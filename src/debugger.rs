// debugger.rs - The host-side debugger (ARCHITECTURE 7.4): commands, stop conditions,
// the trace ring and the port trace. main.rs owns the prompt and the terminal.

use std::collections::VecDeque;
use std::fmt::Write as _;
use std::fs::File;
use std::io::{BufWriter, Write};

use crate::cpu::{Intel8080, Transfer};
use crate::disasm;

/// Steps kept in the trace ring, and how many of them a stop report shows.
const RING: usize = 256;
const REPORT_RING: usize = 8;

const HELP: &str = "c | s [n] | r | m addr [len] | u [addr] [n] | b addr | w addr[-end] [r|w] | \
io port [in|out] | bl | bc [addr] | t file|off | ring [n] | sym addr | ? | q";

/// What the prompt does after a command.
#[derive(Debug, PartialEq, Eq)]
pub enum Flow {
    Stay,
    Resume,
    Quit,
    /// The command was bad and changed nothing; the output is its `? message` line.
    Error,
}

/// One step as the ring records it: where, the bytes there, and the registers before it ran.
struct Step {
    pc: u16,
    bytes: [u8; 3],
    a: u8,
    f: u8,
    bc: u16,
    de: u16,
    hl: u16,
    sp: u16,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Cond {
    Break(u16),
    Watch { lo: u16, hi: u16, read: bool, write: bool },
    Io { port: u8, input: bool, output: bool },
}

/// The port trace file. A run of identical lines is held back and written once.
struct Trace {
    file: BufWriter<File>,
    line: String,
    count: u64,
}

impl Trace {
    fn add(&mut self, line: String) {
        if line == self.line {
            self.count += 1;
            return;
        }
        self.write_pending();
        self.line = line;
        self.count = 1;
    }

    /// Writes the held-back line, then flushes. A trace write error is ignored: it costs trace lines only.
    fn write_pending(&mut self) {
        if self.count > 1 {
            let _ = writeln!(self.file, "{} ; x{}", self.line, self.count);
        } else if self.count == 1 {
            let _ = writeln!(self.file, "{}", self.line);
        }
        self.count = 0;
        let _ = self.file.flush();
    }
}

impl Drop for Trace {
    fn drop(&mut self) {
        self.write_pending();
    }
}

#[derive(Default)]
pub struct Debugger {
    symbols: Vec<(u16, String)>, // sorted by address, file order within one address
    conds: Vec<Cond>,            // in the order set
    ring: VecDeque<Step>,
    trace: Option<Trace>,
    resume: bool, // the next step ignores a breakpoint at PC
}

impl Debugger {
    pub fn new() -> Self {
        Self::default()
    }

    /// Loads `AAAA NAME` lines (rom/monitor.sym). Returns how many.
    pub fn load_symbols(&mut self, text: &str) -> Result<usize, String> {
        for line in text.lines().filter(|l| !l.trim().is_empty()) {
            match line.split_whitespace().collect::<Vec<_>>()[..] {
                [addr, name] if addr.len() == 4 => {
                    let addr = u16::from_str_radix(addr, 16).map_err(|_| format!("bad symbol line: {}", line))?;
                    self.symbols.push((addr, name.to_ascii_uppercase()));
                }
                _ => return Err(format!("bad symbol line: {}", line)),
            }
        }
        self.symbols.sort_by_key(|s| s.0);
        Ok(self.symbols.len())
    }

    /// Runs up to `steps` steps. Returns the stop reason, or None when the steps ran
    /// out or the CPU is halted. Every step is recorded in the ring and the trace.
    pub fn run(&mut self, cpu: &mut Intel8080, steps: u64) -> Option<String> {
        for _ in 0..steps {
            if cpu.halted {
                return None;
            }
            if !std::mem::take(&mut self.resume) && self.conds.contains(&Cond::Break(cpu.pc)) {
                return Some(format!("break {}", self.location(cpu.pc)));
            }
            if self.ring.len() == RING {
                self.ring.pop_front();
            }
            self.ring.push_back(Step {
                pc: cpu.pc,
                bytes: peek3(cpu, cpu.pc),
                a: cpu.a,
                f: cpu.flags,
                bc: cpu.get_bc(),
                de: cpu.get_de(),
                hl: cpu.get_hl(),
                sp: cpu.sp,
            });
            cpu.execute_one();
            if let Some(reason) = self.check(cpu) {
                return Some(reason);
            }
        }
        None
    }

    /// Feeds the last step's transfers to the trace and the stop conditions.
    fn check(&mut self, cpu: &Intel8080) -> Option<String> {
        let mut stop = None;
        for &t in cpu.transfers() {
            if let (Some(trace), Transfer::In(p, v) | Transfer::Out(p, v)) = (&mut self.trace, t) {
                let dir = if matches!(t, Transfer::In(..)) { "IN" } else { "OUT" };
                trace.add(format!("{} {:02X} {:02X}", dir, p, v));
            }
            let hit = self.conds.iter().any(|&c| match (c, t) {
                (Cond::Watch { lo, hi, read, .. }, Transfer::MemRead(a, _)) => read && (lo..=hi).contains(&a),
                (Cond::Watch { lo, hi, write, .. }, Transfer::MemWrite(a, _)) => write && (lo..=hi).contains(&a),
                (Cond::Io { port, input, .. }, Transfer::In(p, _)) => input && p == port,
                (Cond::Io { port, output, .. }, Transfer::Out(p, _)) => output && p == port,
                _ => false,
            });
            if hit && stop.is_none() {
                stop = Some(match t {
                    Transfer::MemRead(a, v) => format!("watch read {:04X} {:02X}", a, v),
                    Transfer::MemWrite(a, v) => format!("watch write {:04X} {:02X}", a, v),
                    Transfer::In(p, v) => format!("io IN {:02X} {:02X}", p, v),
                    Transfer::Out(p, v) => format!("io OUT {:02X} {:02X}", p, v),
                });
            }
        }
        stop
    }

    /// The stop report: reason, ring tail, registers, next instruction. Writes pending trace lines.
    pub fn report(&mut self, cpu: &Intel8080, reason: &str) -> String {
        if let Some(trace) = &mut self.trace {
            trace.write_pending();
        }
        let mut out = format!("* {}\n", reason);
        for step in self.ring.iter().skip(self.ring.len().saturating_sub(REPORT_RING)) {
            out += &self.ring_line(step);
        }
        out + &self.here(cpu)
    }

    /// Registers line and the next instruction.
    fn here(&self, cpu: &Intel8080) -> String {
        format!("{}\n{}", regs_line(cpu), self.listing(cpu.pc, peek3(cpu, cpu.pc)).0)
    }

    /// Runs one command line. Returns what the prompt does next and the output.
    pub fn command(&mut self, cpu: &mut Intel8080, line: &str) -> (Flow, String) {
        let mut out = String::new();
        match self.exec(cpu, line, &mut out) {
            Ok(flow) => (flow, out),
            Err(e) => (Flow::Error, format!("? {}\n", e)),
        }
    }

    fn exec(&mut self, cpu: &mut Intel8080, line: &str, out: &mut String) -> Result<Flow, String> {
        let words: Vec<&str> = line.split_whitespace().collect();
        let Some((cmd, args)) = words.split_first() else {
            return Ok(Flow::Stay);
        };
        let cmd = cmd.to_ascii_lowercase();
        match (cmd.as_str(), args) {
            ("c", []) => {
                self.resume = true;
                return Ok(Flow::Resume);
            }
            ("q", []) => return Ok(Flow::Quit),
            ("?", []) => *out += &format!("{}\n", HELP),
            ("s", [] | [_]) => {
                let n = args.first().map(|a| number(a, 4)).transpose()?.unwrap_or(1);
                if cpu.halted {
                    *out += &format!("{}\n", regs_line(cpu));
                } else {
                    self.resume = true;
                    *out += &match self.run(cpu, n as u64) {
                        Some(reason) => self.report(cpu, &reason),
                        None => self.here(cpu),
                    };
                }
            }
            ("r", []) => *out += &format!("{}\n", regs_line(cpu)),
            ("m", [a, rest @ ..]) if rest.len() <= 1 => {
                let addr = self.address(a)?;
                let len = rest.first().map(|l| number(l, 4)).transpose()?.unwrap_or(0x40);
                for line in 0..(len as u32).div_ceil(16) {
                    let at = addr.wrapping_add(line as u16 * 16);
                    let bytes: Vec<u8> = (0..16).map(|i| cpu.read_byte(at.wrapping_add(i))).collect();
                    let hex: Vec<String> = bytes.iter().map(|b| format!("{:02X}", b)).collect();
                    let ascii: String =
                        bytes.iter().map(|&b| if (0x20..=0x7E).contains(&b) { b as char } else { '.' }).collect();
                    *out += &format!("{:04X}: {}  {}  {}\n", at, hex[..8].join(" "), hex[8..].join(" "), ascii);
                }
            }
            ("u", rest) if rest.len() <= 2 => {
                let mut addr = rest.first().map(|a| self.address(a)).transpose()?.unwrap_or(cpu.pc);
                let n = rest.get(1).map(|n| number(n, 4)).transpose()?.unwrap_or(8);
                for _ in 0..n {
                    let (text, len) = self.listing(addr, peek3(cpu, addr));
                    *out += &text;
                    addr = addr.wrapping_add(len);
                }
            }
            ("b", [a]) => self.set(Cond::Break(self.address(a)?)),
            ("w", [range, rest @ ..]) if rest.len() <= 1 => {
                let (lo, hi) = match range.split_once('-') {
                    Some((lo, hi)) => (self.address(lo)?, self.address(hi)?),
                    None => (self.address(range)?, self.address(range)?),
                };
                if hi < lo {
                    return Err("end < start".into());
                }
                let (read, write) = match rest.first().map(|k| k.to_ascii_lowercase()).as_deref() {
                    None => (true, true),
                    Some("r") => (true, false),
                    Some("w") => (false, true),
                    Some(k) => return Err(format!("not r or w: {}", k)),
                };
                self.set(Cond::Watch { lo, hi, read, write });
            }
            ("io", [p, rest @ ..]) if rest.len() <= 1 => {
                let port = number(p, 2)? as u8;
                let (input, output) = match rest.first().map(|k| k.to_ascii_lowercase()).as_deref() {
                    None => (true, true),
                    Some("in") => (true, false),
                    Some("out") => (false, true),
                    Some(k) => return Err(format!("not in or out: {}", k)),
                };
                self.set(Cond::Io { port, input, output });
            }
            ("bl", []) => {
                for &c in &self.conds {
                    *out += &match c {
                        Cond::Break(a) => format!("b {}\n", self.symbolic(a)),
                        Cond::Watch { lo, hi, read, write } => format!(
                            "w {:04X}{}{}\n",
                            lo,
                            if hi != lo { format!("-{:04X}", hi) } else { String::new() },
                            if !read { " w" } else if !write { " r" } else { "" }
                        ),
                        Cond::Io { port, input, output } => format!(
                            "io {:02X}{}\n",
                            port,
                            if !input { " out" } else if !output { " in" } else { "" }
                        ),
                    };
                }
            }
            ("bc", []) => self.conds.clear(),
            ("bc", [a]) => {
                let addr = self.address(a)?;
                let n = self.conds.len();
                self.conds.retain(|&c| c != Cond::Break(addr));
                if self.conds.len() == n {
                    return Err(format!("no breakpoint at {:04X}", addr));
                }
            }
            ("t", [f]) if f.eq_ignore_ascii_case("off") => self.trace = None,
            ("t", [f]) => {
                let file = File::create(f).map_err(|e| format!("{}: {}", f, e))?;
                self.trace = Some(Trace { file: BufWriter::new(file), line: String::new(), count: 0 });
            }
            ("ring", [] | [_]) => {
                let n = args.first().map(|n| number(n, 4)).transpose()?.map_or(RING, |n| n as usize);
                for step in self.ring.iter().skip(self.ring.len().saturating_sub(n)) {
                    *out += &self.ring_line(step);
                }
            }
            ("sym", [a]) => *out += &format!("{}\n", self.location(self.address(a)?)),
            ("c" | "q" | "?" | "s" | "r" | "m" | "u" | "b" | "w" | "io" | "bl" | "bc" | "t" | "ring" | "sym", _) => {
                return Err(format!("usage: {}", HELP));
            }
            _ => return Err(format!("unknown command: {}", cmd)),
        }
        Ok(Flow::Stay)
    }

    fn set(&mut self, c: Cond) {
        if !self.conds.contains(&c) {
            self.conds.push(c);
        }
    }

    /// A number, NAME or NAME+n (ARCHITECTURE 7.4, Arguments).
    fn address(&self, tok: &str) -> Result<u16, String> {
        if tok.chars().all(|c| c.is_ascii_hexdigit()) {
            return number(tok, 4);
        }
        let (name, offset) = match tok.split_once('+') {
            Some((name, n)) => (name, number(n, 4)?),
            None => (tok, 0),
        };
        match self.symbols.iter().find(|s| s.1.eq_ignore_ascii_case(name)) {
            Some(s) => Ok(s.0.wrapping_add(offset)),
            None => Err(format!("unknown symbol: {}", name)),
        }
    }

    /// The nearest symbol at or below `addr` in its memory-map region (ARCHITECTURE 1),
    /// so a user-area address is never named after the workspace.
    fn nearest(&self, addr: u16) -> Option<&(u16, String)> {
        self.symbols.iter().rev().find(|s| s.0 <= addr && region(s.0) == region(addr))
    }

    /// NAME, NAME+n with the nearest symbol, or AAAA.
    fn symbolic(&self, addr: u16) -> String {
        match self.nearest(addr) {
            Some((a, name)) if *a == addr => name.clone(),
            Some((a, name)) => format!("{}+{:X}", name, addr - a),
            None => format!("{:04X}", addr),
        }
    }

    /// AAAA, or AAAA and its symbolic form.
    fn location(&self, addr: u16) -> String {
        if self.nearest(addr).is_some() {
            format!("{:04X} {}", addr, self.symbolic(addr))
        } else {
            format!("{:04X}", addr)
        }
    }

    fn name_at(&self, addr: u16) -> Option<String> {
        self.symbols.iter().find(|s| s.0 == addr).map(|s| s.1.clone())
    }

    /// The instruction line, preceded by a NAME: line for each symbol at `addr`.
    fn listing(&self, addr: u16, bytes: [u8; 3]) -> (String, u16) {
        let mut text = String::new();
        for (_, name) in self.symbols.iter().filter(|s| s.0 == addr) {
            let _ = writeln!(text, "{}:", name);
        }
        let (line, len) = disasm::line(addr, bytes, |w| self.name_at(w));
        (text + &line + "\n", len)
    }

    fn ring_line(&self, s: &Step) -> String {
        format!(
            "{:<34} A={:02X} F={:02X} BC={:04X} DE={:04X} HL={:04X} SP={:04X}\n",
            disasm::line(s.pc, s.bytes, |w| self.name_at(w)).0,
            s.a,
            s.f,
            s.bc,
            s.de,
            s.hl,
            s.sp
        )
    }
}

/// The memory-map region (ARCHITECTURE 1) holding `addr`: unused, workspace, user, stack, ROM.
fn region(addr: u16) -> u8 {
    match addr {
        0x0000..=0x007F => 0,
        0x0080..=0x00FF => 1,
        0x0100..=0xEEFF => 2,
        0xEF00..=0xEFFF => 3,
        0xF000..=0xFFFF => 4,
    }
}

/// 1 to `digits` hex digits.
fn number(tok: &str, digits: usize) -> Result<u16, String> {
    if tok.is_empty() || tok.len() > digits || !tok.chars().all(|c| c.is_ascii_hexdigit()) {
        return Err(format!("bad number: {}", tok));
    }
    Ok(u16::from_str_radix(tok, 16).unwrap())
}

/// Three bytes from `addr` as the CPU would read them, without a bus transfer.
fn peek3(cpu: &Intel8080, addr: u16) -> [u8; 3] {
    [0, 1, 2].map(|i| cpu.read_byte(addr.wrapping_add(i)))
}

fn regs_line(cpu: &Intel8080) -> String {
    let flags: String = [(0x80, 'S'), (0x40, 'Z'), (0x10, 'A'), (0x04, 'P'), (0x01, 'C')]
        .iter()
        .map(|&(bit, c)| if cpu.flags & bit != 0 { c } else { '-' })
        .collect();
    format!(
        "PC={:04X} SP={:04X} A={:02X} F={:02X} {} BC={:04X} DE={:04X} HL={:04X} INTE={} OVL={}{}",
        cpu.pc,
        cpu.sp,
        cpu.a,
        cpu.flags,
        flags,
        cpu.get_bc(),
        cpu.get_de(),
        cpu.get_hl(),
        cpu.interrupts_enabled as u8,
        cpu.rom_overlay_enabled as u8,
        if cpu.halted { " HLT" } else { "" }
    )
}
