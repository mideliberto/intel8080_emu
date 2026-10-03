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

use intel8080_emu::cpu::Transfer;
use intel8080_emu::debugger::Debugger;
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
    /// Every IN and OUT since power-on, in order.
    ports: Vec<Transfer>,
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
    let mut m = Mon { cpu, con, dir, ports: Vec::new() };
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
            self.ports.extend(self.cpu.transfers().iter().filter(|t| matches!(t, Transfer::In(..) | Transfer::Out(..))));
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

    /// Run until PC = `addr` (input already typed).
    fn run_to(&mut self, addr: u16) {
        let start = self.cpu.cycles;
        while self.cpu.pc != addr {
            assert!(!self.cpu.halted && self.cpu.cycles - start < BUDGET, "never reached {:04X}", addr);
            self.cpu.execute_one();
        }
    }

    fn mem(&mut self, addr: u16, n: usize) -> Vec<u8> {
        (0..n).map(|i| self.cpu.read_byte(addr.wrapping_add(i as u16))).collect()
    }
}

/// A ROM label's address, from rom/monitor.sym (built and committed with monitor.bin).
fn sym(name: &str) -> u16 {
    let text = std::fs::read_to_string("rom/monitor.sym").unwrap();
    let line = text.lines().find(|l| l.get(5..) == Some(name)).unwrap_or_else(|| panic!("no symbol {}", name));
    u16::from_str_radix(&line[..4], 16).unwrap()
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
    // Boot wrote nothing in the user area (ARCHITECTURE 1: 0100-EEFF only on command).
    assert!(m.mem(0x0100, 0xEE00).iter().all(|&b| b == JUNK));
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
fn boot_io_is_out_fe_then_console_output() {
    // ARCHITECTURE 3.2 rule 2: from reset to the first prompt the ROM runs OUT FE and
    // OUT 00 and nothing else. CONOUT does not poll (MONITOR_SPEC 11). Then the prompt
    // waits in CONIN, polling IN 02.
    let (m, banner) = power_on(true);
    let banner = banner.unwrap();
    assert_eq!(m.ports[0], Transfer::Out(0xFE, 0x00));
    let printed: Vec<u8> = m.ports[1..].iter().map_while(|t| match t {
        Transfer::Out(0x00, v) => Some(*v),
        _ => None,
    }).collect();
    assert_eq!(printed, [&banner[..], b"> "].concat());
    let rest = &m.ports[1 + printed.len()..];
    assert!(!rest.is_empty() && rest.iter().all(|t| *t == Transfer::In(0x02, 0x02)), "{:?}", &rest[..rest.len().min(4)]);
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
fn dump_counts_lines_to_ffff() {
    // MONITOR_SPEC 6.2 Termination: stop when the next line start carries past FFFF
    // or passes end. Not a transcript: thousands of lines of junk RAM and ROM.
    let mut m = boot();
    let lines = |out: String| -> Vec<String> { out.split("\\r\\n").filter(|l| !l.is_empty()).map(|l| l[..6].to_string()).collect() };
    let d = lines(m.run("D 0000 F000"));
    assert_eq!((d.len(), d[0].as_str(), d[0xF00].as_str()), (0xF01, "0000: ", "F000: "));
    let d = lines(m.run("D 0000 FFFF"));
    assert_eq!((d.len(), d[0xFFF].as_str()), (0x1000, "FFF0: "));
    assert_eq!(lines(m.run("D"))[0], "0000: ");
    // One argument caps the end at FFFF. The last line wraps, and D goes on from 0001.
    assert_eq!(lines(m.run("D FFE1")), ["FFE1: ", "FFF1: "]);
    assert_eq!(lines(m.run("D"))[0], "0001: ");
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
    let mut m = boot();
    m.play("compare");
    // C 0000 FFFF compares 65536 bytes (MONITOR_SPEC 6.1): the last pair is FFFF (ROM
    // padding) against 0000 (00 after compare.txt). Not a transcript: the ROM and stack
    // page mismatches are thousands of lines, and the stack page ones are C's own stack use
    // (MONITOR_SPEC 6.1).
    let out = m.run("C 0000 FFFF 0001");
    assert!(out.ends_with("\\r\\nFFFF:FF 0000:00\\r\\n"), "{}", &out[out.len().saturating_sub(80)..]);
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
fn go() {
    boot().play("go");
}

#[test]
fn hex_loader() {
    boot().play("hex");
}

/// An Intel HEX type 00 record of `len` copies of `data` at `addr`, checksum correct.
fn hex_data_record(len: u8, addr: u16, data: u8) -> String {
    let mut bytes = vec![len, (addr >> 8) as u8, addr as u8, 0x00];
    bytes.extend(std::iter::repeat(data).take(len as usize));
    let sum = bytes.iter().fold(0u8, |s, b| s.wrapping_add(*b));
    bytes.push(sum.wrapping_neg());
    bytes.iter().fold(":".to_string(), |s, b| s + &format!("{:02X}", b))
}

#[test]
fn hex_guard_sweep() {
    // MONITOR_SPEC 7.2 step 6 for every high byte, at the low bytes and lengths where
    // the carry into the high byte changes: accepted iff AAAA >= 0100 and
    // AAAA + LL <= EF00 without wrap. An accepted record writes exactly its bytes.
    let mut m = boot();
    for hi in 0..=0xFFu16 {
        for lo in [0x00u16, 0x01, 0xDE, 0xDF, 0xFF] {
            for len in [0x01u8, 0x02, 0x21, 0x22] {
                let addr = hi << 8 | lo;
                let data = (hi as u8) ^ lo as u8 ^ len;
                let ok = addr >= 0x0100 && addr as u32 + len as u32 <= 0xEF00;
                let out = m.run(&hex_data_record(len, addr, data));
                assert_eq!(out, if ok { "" } else { "Address out of range\\r\\n" }, "{:04X} {:02X}", addr, len);
                if ok {
                    assert!(m.mem(addr, len as usize).iter().all(|&b| b == data), "{:04X} {:02X}", addr, len);
                }
            }
        }
    }
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
    assert!(!m.dir.path().join("ATEST.BIN").exists(), "X kept a stale name character");
}

#[test]
fn mount_failed() {
    // Mount status 01 (open failed) prints Mount failed (MONITOR_SPEC 6.13). A directory
    // with the name can't be opened as a file (DEVICE_SPECS 7, Mount step 4).
    let mut m = boot();
    std::fs::create_dir(m.dir.path().join("SUB")).unwrap();
    assert_eq!(m.run("X SUB"), "Mount failed\\r\\n");
    assert_eq!(m.run("X"), "No storage mounted\\r\\n");
}

#[test]
fn load_and_write_port_sequences() {
    // MONITOR_SPEC 6.8, 6.12: status, then the address, the data, the flush (W), and
    // status again. The flush is not observable in the file; it is on the port.
    let mut m = boot();
    m.run("F 0200 0202 5A");
    assert_eq!(m.run("X T.BIN"), "Mounted\\r\\n");
    let storage = |m: &Mon, from: usize| -> Vec<Transfer> {
        m.ports[from..].iter().filter(|t| matches!(t, Transfer::In(0x08..=0x0C, _) | Transfer::Out(0x08..=0x0C, _))).copied().collect()
    };
    use Transfer::{In, Out};
    let n = m.ports.len();
    assert_eq!(m.run("W 0200 012345 3"), "Written\\r\\n");
    assert_eq!(storage(&m, n), [In(0x0C, 0x83), Out(0x08, 0x45), Out(0x09, 0x23), Out(0x0A, 0x01),
        Out(0x0B, 0x5A), Out(0x0B, 0x5A), Out(0x0B, 0x5A), Out(0x0C, 0x02), In(0x0C, 0x83)]);
    let n = m.ports.len();
    assert_eq!(m.run("L 012345 0300 3"), "Loaded\\r\\n");
    assert_eq!(storage(&m, n), [In(0x0C, 0x83), Out(0x08, 0x45), Out(0x09, 0x23), Out(0x0A, 0x01),
        In(0x0B, 0x5A), In(0x0B, 0x5A), In(0x0B, 0x5A), In(0x0C, 0x83)]);
    assert_eq!(m.mem(0x0300, 3), [0x5A; 3]);
}

#[test]
fn storage_error_when_the_file_goes_away_mid_transfer() {
    // L and W read status 0C after the transfer (W: after the flush) and print Storage
    // error when bit 0 is 0 (MONITOR_SPEC 6.8, 6.12). The device unmounts on a host I/O
    // error or a Pi service restart (DEVICE_SPECS 6); neither can be caused from here,
    // so the host unmounts it in the middle of the copy loop.
    for (line, at) in [("L 0 0200 10", "CL_LOOP"), ("W 0200 0 10", "CW_LOOP")] {
        let mut m = boot();
        assert_eq!(m.run("X T.BIN"), "Mounted\\r\\n");
        m.con.borrow_mut().push_input(format!("{}\r", line).as_bytes());
        m.run_to(sym(at));
        m.cpu.io_bus_mut().write(0x0E, 0x02);
        assert_eq!(show(&m.step(b"").unwrap()), "Storage error\\r\\n", "{}", line);
    }
}

// ---------- G entry (not a transcript: the program is a HLT) ----------

#[test]
fn go_entry_contract() {
    // MONITOR_SPEC 8: on entry SP = EFFE, the word there is WARM, interrupts are off and
    // the overlay is off. The return through it is in go.txt. The error before G leaves
    // two pushes and a return address behind: WARM must reset SP.
    for (line, at) in [("G 0300", 0x0300u16), ("G", 0x0100)] {
        let mut m = boot();
        m.run(&format!("F {:04X} {:04X} 76", at, at));
        assert_eq!(m.run("C 0200 0100 0300"), "Invalid range\\r\\n");
        m.con.borrow_mut().push_input(format!("{}\r", line).as_bytes());
        let start = m.cpu.cycles;
        while !m.cpu.halted && m.cpu.cycles - start < BUDGET {
            m.cpu.execute_one();
        }
        assert_eq!(m.cpu.pc, at + 1, "{}: halted at the wrong place", line);
        assert_eq!((m.cpu.sp, m.cpu.read_word(0xEFFE)), (0xEFFE, sym("WARM")), "{}", line);
        assert!(!m.cpu.interrupts_enabled && !m.cpu.rom_overlay_enabled);
    }
}

// ---------- Argument errors, under the debugger ----------

#[test]
fn argument_errors_write_no_port_and_no_memory_outside_the_workspace() {
    // MONITOR_SPEC 4.4 rule 4. The debugger stops on any write to 0000-007F, 0100-EEFF or
    // F000-FFFF and on an OUT to any port but the console; each line must instead reach
    // WARM having printed one argument error. The stack page is the monitor's.
    let lines = [
        "C", "C 0200", "C 0200 0210 ZZ", "C 0210 0200 0300", "C 10200 0210 0300",
        "D ZZ", "D 0300 0200", "D 0200 02G0",
        "E ZZ", "E 10200",
        "F 0200 0300", "F 0300 0200 AA", "F 0300 0200 ZZ", "F 0200 0300 100", "F 10200 1020F 1AA", "F 0200 02G0 AA",
        "G ZZ", "G 01ZZ", "G 10100",
        "H 1", "H 10000 1",
        "I", "I 100",
        "M 0200 0300", "M 0200 0300 0", "M 0200 0300 ZZ",
        "O 08", "O 08 100", "O 100 00", "O 0D 4Z",
        "S 0200 0210", "S 0200 0210 AA ZZ", "S 0300 0200 41", "S 0200 0210 100",
        "L 0", "L 0 0200 0", "L 0 0200 ZZ", "L 1234567 0200", "L 0 10200",
        "W 0200", "W 0200 0 0", "W 0200 0 ZZ", "W 0200 1000000",
    ];
    let errors = ["Invalid address", "Invalid hex value", "Invalid port/value", "Invalid range"];
    let mut m = boot();
    assert_eq!(m.run("X T.BIN"), "Mounted\\r\\n");
    let mut dbg = Debugger::new();
    dbg.load_symbols(&std::fs::read_to_string("rom/monitor.sym").unwrap()).unwrap();
    let watches = ["b WARM", "w 0000-007F w", "w 0100-EEFF w", "w F000-FFFF w"].map(String::from);
    for cmd in watches.into_iter().chain((0x01..=0xFF).map(|p| format!("io {:02X} out", p))) {
        assert_eq!(dbg.command(&mut m.cpu, &cmd).1, "", "{}", cmd);
    }
    for line in lines {
        m.con.borrow_mut().take_output();
        m.con.borrow_mut().push_input(format!("{}\r", line).as_bytes());
        dbg.command(&mut m.cpu, "c");
        let stop = dbg.run(&mut m.cpu, 1_000_000).unwrap_or_else(|| panic!("{}: no stop, PC={:04X}", line, m.cpu.pc));
        assert_eq!(stop, format!("break {:04X} WARM", sym("WARM")), "{}\n{}", line, dbg.report(&m.cpu, &stop));
        // The stop at WARM comes before the prompt, so the next line's output starts with it.
        let out = String::from_utf8(m.con.borrow_mut().take_output()).unwrap();
        let msg = out.trim_start_matches("> ").strip_prefix(&format!("{}\r\n", line)).and_then(|o| o.strip_suffix("\r\n"));
        assert!(msg.is_some_and(|msg| errors.contains(&msg)), "{}: {:?}", line, out);
    }
}

// ---------- HEX loader: validate before write, under the debugger ----------

#[test]
fn hex_records_are_validated_before_any_write() {
    // MONITOR_SPEC 7.2: the whole record is validated before any byte is written, and
    // a failure writes nothing. Once READ_LINE has stored the line (break at HEX_RECORD)
    // the debugger stops on any write outside the stack page, on any OUT but the console,
    // and at WARM. A record that writes nothing must reach WARM having printed its message.
    // A data record must make its first write from HR_WRITE, the step after every check,
    // at AAAA.
    let full = ":22020000000102030405060708090A0B0C0D0E0F101112131415161718191A1B1C1D1E1F2021AB";
    let quiet = [
        (":0100FF00AA56", "Address out of range"),
        (":0200FF001122CC", "Address out of range"),
        (":01EF0000AA66", "Address out of range"),
        (":02EEFF001122DE", "Address out of range"),
        (":01000000AA55", "Address out of range"),
        (":22EEDF00000102030405060708090A0B0C0D0E0F101112131415161718191A1B1C1D1E1F2021E0", "Address out of range"),
        (":11EEF00055555555555555555555555555555555556C", "Address out of range"),
        (":10FFF80011111111111111111111111111111111E9", "Address out of range"),
        (":0100000001FE", "Address out of range"),
        (":020080004142FB", "Address out of range"),
        (":2200CE000000000000000000000000000000000000000000000000000000000000000000000010", "Address out of range"),
        (":020000021000EC", "Bad record type"),
        (":0400000300000100F8", "Bad record type"),
        (":020000040000FA", "Bad record type"),
        (":0400000500000100F6", "Bad record type"),
        (":0401000601020304EB", "Bad record type"),
        (":010100FF55AA", "Bad record type"),
        (":0401000001020304F0", "Checksum error"),
        (":0401000001020G04F1", "Bad record"),
        (":0401000001020304", "Bad record"),
        (":0401000001020304F1 ", "Bad record"),
        (":", "Bad record"),
        (&format!(" {}", full), "Bad record"),
        (":23020000000102030405060708090A0B0C0D0E0F101112131415161718191A1B1C1D1E1F20212288", "Record too long"),
        (":0000000000", ""),
        (":00000001FF", "Loaded"),
        (":0101000112EB", "Loaded"),
    ];
    let writes = [(":01010000AA54", 0x0100u16, 0xAAu8), (":01EEFF00AA68", 0xEEFF, 0xAA), (full, 0x0200, 0x00)];
    let mut m = boot();
    let mut dbg = Debugger::new();
    dbg.load_symbols(&std::fs::read_to_string("rom/monitor.sym").unwrap()).unwrap();
    let mut until = |m: &mut Mon, line: &str| -> String {
        m.con.borrow_mut().take_output();
        m.con.borrow_mut().push_input(format!("{}\r", line).as_bytes());
        for cmd in ["bc", "b HEX_RECORD", "c"] {
            dbg.command(&mut m.cpu, cmd);
        }
        assert_eq!(dbg.run(&mut m.cpu, 1_000_000), Some(format!("break {:04X} HEX_RECORD", sym("HEX_RECORD"))), "{}", line);
        let watches = ["bc", "b WARM", "w 0000-EEFF w", "w F000-FFFF w"].map(String::from);
        for cmd in watches.into_iter().chain((0x01..=0xFF).map(|p| format!("io {:02X} out", p))) {
            assert_eq!(dbg.command(&mut m.cpu, &cmd).1, "", "{}", cmd);
        }
        assert_eq!(dbg.command(&mut m.cpu, "c").1, "");
        dbg.run(&mut m.cpu, 1_000_000).unwrap_or_else(|| panic!("{}: no stop, PC={:04X}", line, m.cpu.pc))
    };
    for (line, msg) in quiet {
        let stop = until(&mut m, line);
        assert_eq!(stop, format!("break {:04X} WARM", sym("WARM")), "{}", line);
        let out = String::from_utf8(m.con.borrow_mut().take_output()).unwrap();
        let echo = &line[..line.len().min(79)];
        let want = if msg.is_empty() { String::new() } else { format!("{}\r\n", msg) };
        assert_eq!(out.trim_start_matches("> ").strip_prefix(&format!("{}\r\n", echo)), Some(want.as_str()), "{}", line);
    }
    for (line, addr, byte) in writes {
        let stop = until(&mut m, line);
        assert_eq!(stop, format!("watch write {:04X} {:02X}", addr, byte), "{}", line);
        assert!((sym("HR_WRITE")..sym("HR_EOF")).contains(&m.cpu.pc), "{}: write from {:04X}", line, m.cpu.pc);
    }
}
