// monitor_tests.rs - Monitor ROM integration tests: a strict transcript harness.
//
// Boot (ARCHITECTURE 3.1): RAM 0000-EFFF is filled with JUNK before reset, and the
// registers, flags and SP are set to junk after it, so the ROM can't lean on zeroed
// RAM, registers or a preset SP.
//
// A step types its input, then runs until the input is consumed, the output ends
// with the "> " prompt and nothing more is printed. The output must match the
// transcript exactly. An exhausted cycle budget or a HLT is a failure.
//
// Transcripts live in tests/transcripts/*.txt, so the same files can drive the
// hardware over the Pi TCP console. Format, one item per line:
//   # comment             ignored, as are blank lines
//   > text                type `text` CR. The expected output starts with the echo `text` CR LF.
//                         A lone `>` types an empty line.
//   < text                type `text` exactly (no CR added, no echo assumed)
//   anything else         one expected output line; CR LF is appended
// Escapes, in all three: \r \n \\ \xHH. Write a trailing space as \x20, an
// output line that starts with "> " as \x3E\x20, one that starts with '#' as \x23,
// and an empty output line as a trailing \r\n on the line before it.
// Transcripts only display memory they wrote first: on hardware RAM is random.

use std::cell::RefCell;
use std::path::Path;
use std::rc::Rc;

use intel8080_emu::io::build_bus;
use intel8080_emu::io::devices::console::Console;
use intel8080_emu::io::IoDevice;
use intel8080_emu::Intel8080;

/// HLT. A NOP-like byte (00, or A5 = ANA L) would slide execution into F000 and
/// boot the ROM even with the overlay missing. RST 0 would jump back to 0000.
/// HLT stops the CPU at the first junk byte it executes, and a halt fails the step.
const JUNK: u8 = 0x76;
/// Pushes before `LXI SP` land in ROM and are lost, so a return through them crashes.
const JUNK_SP: u16 = 0x0000;
/// Cycles per step. `C 0000 FFFF` is the slowest command and needs about 10M.
const BUDGET: u64 = 30_000_000;

struct Mon {
    cpu: Intel8080,
    con: Rc<RefCell<Console>>,
    dir: tempfile::TempDir,
}

/// Power on with junk RAM and the port map main.rs uses (build_bus); returns the
/// monitor and what boot printed up to the first prompt.
fn power_on(overlay: bool) -> (Mon, Result<Vec<u8>, String>) {
    let mut cpu = Intel8080::new();
    cpu.load_program(&vec![JUNK; 0xF000], 0x0000);
    let dir = tempfile::tempdir().unwrap();
    let (bus, con) = build_bus(dir.path());
    *cpu.io_bus_mut() = bus;
    cpu.load_rom_from_file(Path::new("rom/monitor.bin")).unwrap();
    cpu.reset();
    (cpu.a, cpu.b, cpu.c, cpu.d, cpu.e, cpu.h, cpu.l) = (JUNK, JUNK, JUNK, JUNK, JUNK, JUNK, JUNK);
    cpu.flags = 0xD7;
    cpu.sp = JUNK_SP;
    if !overlay {
        cpu.rom_overlay_enabled = false;
    }
    let mut m = Mon { cpu, con, dir };
    let banner = m.step(b"");
    (m, banner)
}

fn boot() -> Mon {
    let (m, banner) = power_on(true);
    let banner = banner.unwrap_or_else(|e| panic!("boot: {}", e));
    let b = show(&banner);
    assert!(b.starts_with("\\r\\n8080 Monitor v"), "{}", b);
    assert!(b.ends_with("\\r\\nReady.\\r\\n"), "{}", b);
    m
}

/// Bytes as transcript text: printable ASCII as is, everything else escaped.
fn show(bytes: &[u8]) -> String {
    let mut s = String::new();
    for &b in bytes {
        match b {
            b'\r' => s.push_str("\\r"),
            b'\n' => s.push_str("\\n"),
            b'\\' => s.push_str("\\\\"),
            0x20..=0x7E => s.push(b as char),
            _ => s.push_str(&format!("\\x{:02X}", b)),
        }
    }
    s
}

fn unescape(text: &str) -> Vec<u8> {
    let mut out = Vec::new();
    let mut it = text.bytes();
    while let Some(b) = it.next() {
        if b != b'\\' {
            out.push(b);
            continue;
        }
        match it.next() {
            Some(b'r') => out.push(b'\r'),
            Some(b'n') => out.push(b'\n'),
            Some(b'\\') => out.push(b'\\'),
            Some(b'x') => {
                let hex = [it.next().unwrap(), it.next().unwrap()];
                out.push(u8::from_str_radix(std::str::from_utf8(&hex).unwrap(), 16).unwrap());
            }
            e => panic!("bad escape \\{:?} in {:?}", e.map(|c| c as char), text),
        }
    }
    out
}

impl Mon {
    /// Type `input`, run to the next prompt, return what was printed (prompt stripped).
    fn step(&mut self, input: &[u8]) -> Result<Vec<u8>, String> {
        self.con.borrow_mut().take_output();
        self.con.borrow_mut().push_input(input);
        let start = self.cpu.cycles;
        let mut quiet_since = None;
        loop {
            if self.cpu.halted {
                return Err(format!("HLT at PC={:04X} after {:?}", self.cpu.pc, show(input)));
            }
            if self.cpu.cycles - start > BUDGET {
                return Err(format!("no prompt within {} cycles after {:?}, PC={:04X}, output {:?}",
                    BUDGET, show(input), self.cpu.pc, show(self.con.borrow().output())));
            }
            self.cpu.execute_one();
            let mut con = self.con.borrow_mut();
            let at_prompt = con.read(0x02) & 0x01 == 0 && con.output().ends_with(b"> ");
            let len = con.output().len();
            drop(con);
            // At a prompt with no input left, it must stay quiet: a dump line can end in "> ".
            match quiet_since {
                Some((cycles, n)) if n == len && at_prompt => {
                    if self.cpu.cycles - cycles > 2_000 {
                        break;
                    }
                }
                _ => quiet_since = if at_prompt { Some((self.cpu.cycles, len)) } else { None },
            }
        }
        let out = self.con.borrow().output().to_vec();
        Ok(out[..out.len() - 2].to_vec())
    }

    /// Type one command line; return its output with the echo stripped.
    fn run(&mut self, line: &str) -> String {
        let out = self.step(format!("{}\r", line).as_bytes()).unwrap_or_else(|e| panic!("{}", e));
        let out = show(&out);
        let echo = format!("{}\\r\\n", line);
        out.strip_prefix(&echo).unwrap_or_else(|| panic!("echo mismatch: {:?}", out)).to_string()
    }

    /// Play tests/transcripts/<name>.txt. Every step must match exactly.
    fn play(&mut self, name: &str) {
        let path = format!("tests/transcripts/{}.txt", name);
        let text = std::fs::read_to_string(&path).unwrap();
        let mut steps: Vec<(usize, Vec<u8>, Vec<u8>)> = Vec::new();
        for (i, line) in text.lines().enumerate() {
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            if line == ">" || line.starts_with("> ") {
                let typed = unescape(line.get(2..).unwrap_or(""));
                let mut echo = typed.clone();
                echo.extend_from_slice(b"\r\n");
                steps.push((i + 1, [&typed[..], b"\r"].concat(), echo));
            } else if line.starts_with("< ") {
                steps.push((i + 1, unescape(&line[2..]), Vec::new()));
            } else {
                let step = steps.last_mut().unwrap_or_else(|| panic!("{}:{}: output before input", path, i + 1));
                step.2.extend(unescape(line));
                step.2.extend_from_slice(b"\r\n");
            }
        }
        for (line, input, expected) in steps {
            let got = self.step(&input).unwrap_or_else(|e| panic!("{}:{}: {}", path, line, e));
            assert_eq!(show(&got), show(&expected), "{}:{}", path, line);
        }
    }

    fn mem(&mut self, addr: u16, n: usize) -> Vec<u8> {
        (0..n).map(|i| self.cpu.read_byte(addr.wrapping_add(i as u16))).collect()
    }
}

// ---------- Boot ----------

#[test]
fn boot_banner() {
    let (_m, banner) = power_on(true);
    let b = show(&banner.unwrap());
    // MONITOR_SPEC 1.1: tests never match the version, date or time.
    let lines: Vec<&str> = b.split("\\r\\n").collect();
    assert_eq!(lines.len(), 5, "{}", b);
    assert_eq!(lines[0], "");
    assert!(lines[1].starts_with("8080 Monitor v"), "{}", b);
    assert!(lines[2].starts_with("Built: "), "{}", b);
    assert_eq!(lines[3], "Ready.");
    assert_eq!(lines[4], "");
}

#[test]
fn boot_assumes_nothing() {
    let mut m = boot();
    // The overlay is off and 0000-007F was left alone: D shows RAM junk, not ROM.
    assert!(!m.cpu.rom_overlay_enabled);
    assert_eq!(m.run("D 0000 000F"),
        "0000: 76 76 76 76 76 76 76 76  76 76 76 76 76 76 76 76  vvvvvvvvvvvvvvvv\\r\\n");
    // Bare D and bare E start at 0000 after cold start (LAST_DUMP_ADDR, LAST_EXAM_ADDR).
    let mut m = boot();
    assert!(m.run("D").starts_with("0000: 76 "));
    assert_eq!(show(&m.step(b"E\r.").unwrap()), "E\\r\\n0000: 76-\\r\\n");
}

#[test]
fn boot_fails_without_overlay() {
    // Guards the junk choice: with zeroed RAM this booted fine (a NOP slide into F000).
    let (m, banner) = power_on(false);
    let e = banner.expect_err("booted without the overlay");
    assert!(e.starts_with("HLT at PC=0001"), "{}", e);
    assert!(m.con.borrow().output().is_empty());
}

// ---------- Transcripts ----------

#[test]
fn dispatch() {
    boot().play("dispatch");
}

#[test]
fn line_input() {
    boot().play("line_input");
}

#[test]
fn help() {
    boot().play("help");
}

#[test]
fn hex_math() {
    boot().play("hex_math");
}

#[test]
fn dump() {
    let mut m = boot();
    m.play("dump");
    // A line at FFF8 wraps to 0000-0007, and the next D starts at 0008 (MONITOR_SPEC 6.2).
    // Not a transcript: FFF8-FFFF is ROM padding.
    m.run("F 0000 0007 33");
    let line = m.run("D FFF8 FFFF");
    assert!(line.starts_with("FFF8: ") && line.ends_with("33333333\\r\\n"), "{}", line);
    assert_eq!(line.matches("\\r\\n").count(), 1, "{}", line);
    assert!(m.run("D").starts_with("0008: "));
}

#[test]
fn examine() {
    let mut m = boot();
    m.play("examine");
    assert_eq!(m.mem(0x01FF, 9), [0x00, 0x05, 0x07, 0x00, 0x02, 0x00, 0x0A, 0x00, 0x00]);
}

#[test]
fn fill() {
    let mut m = boot();
    m.play("fill");
    assert_eq!(m.mem(0x01FF, 18), [&[0x00][..], &[0xAA; 16], &[0x00]].concat());
}

#[test]
fn move_block() {
    let mut m = boot();
    m.play("move");
    assert_eq!(m.mem(0x02FF, 18), [&[0x00][..], &[0x11; 8], &[0x22; 8], &[0x00]].concat());
}

#[test]
fn compare() {
    boot().play("compare");
}

#[test]
fn search() {
    boot().play("search");
}

#[test]
fn io_ports() {
    boot().play("io");
}

#[test]
fn storage() {
    let mut m = boot();
    m.play("storage");
    let disk = std::fs::read(m.dir.path().join("TEST.BIN")).unwrap();
    assert_eq!(disk.len(), 0x123457);
    assert_eq!(disk[..4], [0xA0, 0xA1, 0xA2, 0xA3]);
    assert_eq!(disk[4..0x100], [0x5A; 0xFC]);
    assert_eq!(disk[0x100..0x10002], vec![0x00; 0xFF02]);
    assert_eq!(disk[0x10002..0x10005], [0xA0, 0xA1, 0xA2]);
    assert_eq!(m.cpu.io_bus_mut().read(0x0C) & 0x01, 0, "X - left the file mounted");
}

// ---------- G (not a transcript: the program ends in HLT, not at a prompt) ----------

#[test]
fn go_runs_code() {
    for (line, at) in [("G 0300", 0x0300u16), ("G", 0x0100)] {
        let mut m = boot();
        // MVI A,'!' / OUT 00 / HLT
        for (i, b) in [0x3E, b'!', 0xD3, 0x00, 0x76].iter().enumerate() {
            m.run(&format!("F {:04X} {:04X} {:02X}", at + i as u16, at + i as u16, b));
        }
        m.con.borrow_mut().take_output();
        m.con.borrow_mut().push_input(format!("{}\r", line).as_bytes());
        let start = m.cpu.cycles;
        while !m.cpu.halted && m.cpu.cycles - start < BUDGET {
            m.cpu.execute_one();
        }
        assert_eq!(m.cpu.pc, at + 5, "{}: halted at the wrong place", line);
        assert_eq!(show(m.con.borrow().output()), format!("{}\\r\\n!", line));
    }
}
