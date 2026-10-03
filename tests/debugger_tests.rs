// debugger_tests.rs - The debugger (ARCHITECTURE 7.4) driven through its command parser,
// the way a --script drives it: on small RAM programs for exact formats, and on the
// real ROM with rom/monitor.sym for symbols, breaks, watchpoints, I/O breaks and the trace.

use std::cell::RefCell;
use std::path::Path;
use std::rc::Rc;

use intel8080_emu::debugger::{Debugger, Flow};
use intel8080_emu::disasm::disassemble;
use intel8080_emu::io::build_bus;
use intel8080_emu::io::devices::console::Console;
use intel8080_emu::Intel8080;

/// Steps a `c` may run before the test calls it a missing stop.
const BUDGET: u64 = 5_000_000;

struct Rig {
    cpu: Intel8080,
    con: Rc<RefCell<Console>>,
    dbg: Debugger,
    dir: tempfile::TempDir,
}

/// The real port map; `program` at 0100 with SP=2000 and no ROM.
fn ram(program: &[u8]) -> Rig {
    let dir = tempfile::tempdir().unwrap();
    let mut cpu = Intel8080::new();
    let (bus, con) = build_bus(dir.path());
    *cpu.io_bus_mut() = bus;
    cpu.load_program(program, 0x0100);
    cpu.sp = 0x2000;
    Rig { cpu, con, dbg: Debugger::new(), dir }
}

/// The real ROM and its symbols, at RESET, before the first instruction.
fn rom() -> Rig {
    let mut r = ram(&[]);
    r.cpu.load_rom_from_file(Path::new("rom/monitor.bin")).unwrap();
    r.cpu.reset();
    r.dbg.load_symbols(&std::fs::read_to_string("rom/monitor.sym").unwrap()).unwrap();
    r
}

impl Rig {
    /// One command line. A `c` runs until the next stop and returns the stop report.
    fn cmd(&mut self, line: &str) -> String {
        let (flow, out) = self.dbg.command(&mut self.cpu, line);
        match flow {
            Flow::Stay => out,
            Flow::Resume => {
                let reason = self.dbg.run(&mut self.cpu, BUDGET);
                let reason = reason.unwrap_or_else(|| panic!("no stop after {:?}, PC={:04X}", line, self.cpu.pc));
                out + &self.dbg.report(&self.cpu, &reason)
            }
            Flow::Quit => panic!("quit"),
        }
    }

    fn sym(&mut self, name: &str) -> u16 {
        u16::from_str_radix(&self.cmd(&format!("sym {}", name))[..4], 16).unwrap()
    }

    fn type_in(&mut self, text: &str) {
        self.con.borrow_mut().push_input(text.as_bytes());
    }

    fn trace_path(&self) -> String {
        self.dir.path().join("trace.txt").to_str().unwrap().to_string()
    }
}

// ---------- Disassembler ----------

#[test]
fn disassembler_matches_the_reference_for_all_256_opcodes() {
    let txt = std::fs::read_to_string("docs/reference/Complete_Intel_8080_Instruction_Set_Reference.txt").unwrap();
    let mut seen = Vec::new();
    for line in txt.lines().filter(|l| l.starts_with("| 0x")) {
        let cols: Vec<&str> = line.split('|').map(|s| s.trim()).collect();
        let op = u8::from_str_radix(&cols[1][2..], 16).unwrap();
        if seen.contains(&op) {
            continue; // listed twice
        }
        seen.push(op);
        let text = cols[3].replace("d16", "1234").replace("a16", "1234").replace("d8", "34").replace("p8", "34");
        let len: u16 = cols[5].parse().unwrap();
        assert_eq!(disassemble([op, 0x34, 0x12], |_| None), (text, len), "opcode {:02X}", op);
    }
    // The branch aliases (ARCHITECTURE 5.4) are not in the reference.
    for (op, text, len) in [(0xCB, "JMP* 1234", 3), (0xD9, "RET*", 1), (0xDD, "CALL* 1234", 3), (0xED, "CALL* 1234", 3), (0xFD, "CALL* 1234", 3)] {
        assert_eq!(disassemble([op, 0x34, 0x12], |_| None), (text.to_string(), len), "opcode {:02X}", op);
        seen.push(op);
    }
    seen.sort();
    assert_eq!(seen, (0..=255).collect::<Vec<u8>>());
}

#[test]
fn disassembler_names_address_operands_only() {
    let name = |w: u16| (w == 0xF12E).then(|| "SKIP_SPACES".to_string());
    assert_eq!(disassemble([0xCD, 0x2E, 0xF1], name), ("CALL SKIP_SPACES".to_string(), 3));
    assert_eq!(disassemble([0x3A, 0x2E, 0xF1], name), ("LDA SKIP_SPACES".to_string(), 3));
    assert_eq!(disassemble([0xCD, 0x2F, 0xF1], name), ("CALL F12F".to_string(), 3));
    assert_eq!(disassemble([0x21, 0x2E, 0xF1], name), ("LXI H,F12E".to_string(), 3));
    assert_eq!(disassemble([0x3E, 0x2E, 0xF1], name), ("MVI A,2E".to_string(), 2));
    assert_eq!(disassemble([0xDB, 0x02, 0xF1], name), ("IN 02".to_string(), 2));
}

// ---------- Formats, stepping, the ring (RAM programs) ----------

/// LXI H,1234 / MVI A,0D / PUSH H / LDA 0300 / JMP* 0100 (the CB alias)
const LOOP: [u8; 12] = [0x21, 0x34, 0x12, 0x3E, 0x0D, 0xE5, 0x3A, 0x00, 0x03, 0xCB, 0x00, 0x01];

#[test]
fn registers_disassembly_memory_and_steps() {
    let mut r = ram(&LOOP);
    assert_eq!(r.cmd("r"), "PC=0100 SP=2000 A=00 F=02 ----- BC=0000 DE=0000 HL=0000 INTE=0 OVL=1\n");
    assert_eq!(
        r.cmd("u 0100 5"),
        "0100  21 34 12  LXI H,1234\n\
         0103  3E 0D     MVI A,0D\n\
         0105  E5        PUSH H\n\
         0106  3A 00 03  LDA 0300\n\
         0109  CB 00 01  JMP* 0100\n"
    );
    // s counts instructions; it prints the registers and the next instruction.
    assert_eq!(
        r.cmd("s 3"),
        "PC=0106 SP=1FFE A=0D F=02 ----- BC=0000 DE=0000 HL=1234 INTE=0 OVL=1\n0106  3A 00 03  LDA 0300\n"
    );
    assert_eq!(r.cmd("s").lines().next(), Some("PC=0109 SP=1FFE A=00 F=02 ----- BC=0000 DE=0000 HL=1234 INTE=0 OVL=1"));
    assert_eq!(r.cmd("u").lines().next(), Some("0109  CB 00 01  JMP* 0100"), "u defaults to PC");
    // The ring has the registers before each step, oldest first.
    assert_eq!(
        r.cmd("ring"),
        "0100  21 34 12  LXI H,1234         A=00 F=02 BC=0000 DE=0000 HL=0000 SP=2000\n\
         0103  3E 0D     MVI A,0D           A=00 F=02 BC=0000 DE=0000 HL=1234 SP=2000\n\
         0105  E5        PUSH H             A=0D F=02 BC=0000 DE=0000 HL=1234 SP=2000\n\
         0106  3A 00 03  LDA 0300           A=0D F=02 BC=0000 DE=0000 HL=1234 SP=1FFE\n"
    );
    assert_eq!(r.cmd("ring 1"), "0106  3A 00 03  LDA 0300           A=0D F=02 BC=0000 DE=0000 HL=1234 SP=1FFE\n");
    // m: the monitor's D line format, whole lines covering LEN bytes.
    assert_eq!(r.cmd("m 1FF8 10"), "1FF8: 00 00 00 00 00 00 34 12  00 00 00 00 00 00 00 00  ......4.........\n");
    assert_eq!(r.cmd("m 1FF8 11").lines().nth(1).unwrap_or("").get(..6), Some("2008: "));
    assert_eq!(r.cmd("m 1FF8").lines().count(), 4, "default length 40");
    // The flags field: S Z A P C.
    r.cpu.flags = 0xD7;
    assert!(r.cmd("r").contains(" F=D7 SZAPC "));
    // The ring keeps the last 256 steps.
    r.cmd("s 200");
    assert_eq!(r.cmd("ring").lines().count(), 256);
}

#[test]
fn watchpoint_stop_report_is_exact() {
    let mut r = ram(&LOOP);
    r.cmd("w 1FFE w");
    // PUSH H writes 1FFF (12), then 1FFE (34); the report names the transfer that hit.
    assert_eq!(
        r.cmd("c"),
        "* watch write 1FFE 34\n\
         0100  21 34 12  LXI H,1234         A=00 F=02 BC=0000 DE=0000 HL=0000 SP=2000\n\
         0103  3E 0D     MVI A,0D           A=00 F=02 BC=0000 DE=0000 HL=1234 SP=2000\n\
         0105  E5        PUSH H             A=0D F=02 BC=0000 DE=0000 HL=1234 SP=2000\n\
         PC=0106 SP=1FFE A=0D F=02 ----- BC=0000 DE=0000 HL=1234 INTE=0 OVL=1\n\
         0106  3A 00 03  LDA 0300\n"
    );
    // A read watchpoint sees data reads, not opcode or operand fetches.
    r.cmd("bc");
    r.cmd("w 0100-0300 r");
    assert_eq!(r.cmd("c").lines().next(), Some("* watch read 0300 00"));
    assert!(r.cmd("r").starts_with("PC=0109 "));
    // Out of range, or the wrong kind: no stop.
    r.cmd("bc");
    r.cmd("w 0400 r");
    r.cmd("w 0300 w");
    r.cmd("b 0109");
    assert_eq!(r.cmd("c").lines().next(), Some("* break 0109"));
    // Each pass pushes 2 bytes lower; a watchpoint there sees the high byte first.
    r.cmd("bc");
    r.cmd("w 1FFA-1FFB");
    assert_eq!(r.cmd("c").lines().next(), Some("* watch write 1FFB 12"));
}

#[test]
fn breakpoints_resume_and_list() {
    let mut r = ram(&LOOP);
    r.cmd("b 0105");
    assert_eq!(r.cmd("c").lines().next(), Some("* break 0105"));
    assert!(r.cmd("r").starts_with("PC=0105 SP=2000 "), "stopped before PUSH ran");
    // c from a breakpoint runs it, then stops there on the next pass.
    let report = r.cmd("c");
    assert_eq!(report.lines().next(), Some("* break 0105"));
    let lines: Vec<&str> = report.lines().collect();
    assert_eq!(lines.len(), 1 + 7 + 2, "7 steps in the ring so far: {}", report);
    assert!(lines[7].starts_with("0103  3E 0D "), "{}", report);
    assert!(r.cmd("r").starts_with("PC=0105 SP=1FFE "));
    // s stops at a breakpoint it reaches, but not at the one it starts on.
    r.cmd("b 0109");
    assert_eq!(r.cmd("s 10").lines().next(), Some("* break 0109"));
    r.cmd("w 0080-00FF w");
    r.cmd("io 0E out");
    r.cmd("io 01");
    r.cmd("w 0300 r");
    r.cmd("b 0105");
    assert_eq!(r.cmd("bl"), "b 0105\nb 0109\nw 0080-00FF w\nio 0E out\nio 01\nw 0300 r\n");
    assert_eq!(r.cmd("bc 0105"), "");
    assert_eq!(r.cmd("bc 0105"), "? no breakpoint at 0105\n");
    assert_eq!(r.cmd("bl"), "b 0109\nw 0080-00FF w\nio 0E out\nio 01\nw 0300 r\n");
    r.cmd("bc");
    assert_eq!(r.cmd("bl"), "");
}

#[test]
fn bad_commands_change_nothing() {
    let mut r = ram(&LOOP);
    for (line, err) in [
        ("zz", "? unknown command: zz\n"),
        ("b NOPE", "? unknown symbol: NOPE\n"),
        ("b 12345", "? bad number: 12345\n"),
        ("b CMD+G", "? bad number: G\n"),
        ("w 0200-0100", "? end < start\n"),
        ("w 0200 x", "? not r or w: x\n"),
        ("io 100", "? bad number: 100\n"),
        ("io 01 both", "? not in or out: both\n"),
        ("m 1 2 3", "? usage: c | s [n] | r | m addr [len] | u [addr] [n] | b addr | w addr[-end] [r|w] | io port [in|out] | bl | bc [addr] | t file|off | ring [n] | sym addr | ? | q\n"),
        ("u 1 2 3", "? usage: c | s [n] | r | m addr [len] | u [addr] [n] | b addr | w addr[-end] [r|w] | io port [in|out] | bl | bc [addr] | t file|off | ring [n] | sym addr | ? | q\n"),
        ("w 1 r x", "? usage: c | s [n] | r | m addr [len] | u [addr] [n] | b addr | w addr[-end] [r|w] | io port [in|out] | bl | bc [addr] | t file|off | ring [n] | sym addr | ? | q\n"),
        ("io 1 in x", "? usage: c | s [n] | r | m addr [len] | u [addr] [n] | b addr | w addr[-end] [r|w] | io port [in|out] | bl | bc [addr] | t file|off | ring [n] | sym addr | ? | q\n"),
        ("s 1 2", "? usage: c | s [n] | r | m addr [len] | u [addr] [n] | b addr | w addr[-end] [r|w] | io port [in|out] | bl | bc [addr] | t file|off | ring [n] | sym addr | ? | q\n"),
    ] {
        assert_eq!(r.cmd(line), err, "{}", line);
    }
    assert_eq!(r.cmd(""), "");
    assert_eq!(r.cmd("?"), "c | s [n] | r | m addr [len] | u [addr] [n] | b addr | w addr[-end] [r|w] | io port [in|out] | bl | bc [addr] | t file|off | ring [n] | sym addr | ? | q\n");
    for bad in ["F00 X", "F000", "F000 A B", "G000 A"] {
        assert_eq!(r.dbg.load_symbols(bad), Err(format!("bad symbol line: {}", bad)));
    }
    assert_eq!(r.cmd("bl"), "");
    assert!(r.cmd("R").starts_with("PC=0100 "), "commands are case-insensitive");
    assert_eq!(r.dbg.command(&mut r.cpu, "q").0, Flow::Quit);
}

#[test]
fn port_trace_collapses_repeats_and_sees_fe_ff() {
    // MVI A,55 / OUT 10 / IN 02 x3 / OUT FE / IN FF / HLT
    let mut r = ram(&[0x3E, 0x55, 0xD3, 0x10, 0xDB, 0x02, 0xDB, 0x02, 0xDB, 0x02, 0xD3, 0xFE, 0xDB, 0xFF, 0x76]);
    let path = r.trace_path();
    r.cmd(&format!("t {}", path));
    let out = r.cmd("s 10");
    assert!(out.starts_with("PC=010F SP=2000 A=00 F=02 ----- BC=0000 DE=0000 HL=0000 INTE=0 OVL=0 HLT\n"), "{}", out);
    assert_eq!(r.cmd("t off"), "");
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "OUT 10 55\nIN 02 02 ; x3\nOUT FE 02\nIN FF 00\n");
    assert!(!Path::new("off").exists(), "t off opened a file named off");
    // s on a halted CPU only reports.
    assert_eq!(r.cmd("s"), "PC=010F SP=2000 A=00 F=02 ----- BC=0000 DE=0000 HL=0000 INTE=0 OVL=0 HLT\n");
    assert_eq!(r.cmd("ring").lines().count(), 8);

    let mut r = ram(&[0x3E, 0x55, 0xD3, 0x10, 0xDB, 0x02, 0xDB, 0x02, 0xDB, 0x02, 0xD3, 0xFE, 0xDB, 0xFF, 0x76]);
    r.cmd("io FE");
    assert_eq!(r.cmd("c").lines().next(), Some("* io OUT FE 02"));
    r.cmd("io FF in");
    assert_eq!(r.cmd("c").lines().next(), Some("* io IN FF 00"));
}

// ---------- The real ROM ----------

#[test]
fn break_at_a_rom_symbol() {
    let mut r = rom();
    let cmd_dump = r.sym("CMD_DUMP");
    let skip_spaces = r.sym("SKIP_SPACES");
    assert_eq!(r.cmd("sym cmd_dump+3"), format!("{:04X} CMD_DUMP+3\n", cmd_dump + 3));
    assert_eq!(r.cmd("sym 0100"), "0100\n");
    r.type_in("D 0200 020F\r");
    r.cmd("b CMD_DUMP");
    assert_eq!(r.cmd("bl"), "b CMD_DUMP\n");
    let report = r.cmd("c");
    let lines: Vec<&str> = report.lines().collect();
    assert_eq!(lines.len(), 12, "{}", report);
    assert_eq!(lines[0], format!("* break {:04X} CMD_DUMP", cmd_dump));
    // The ring: the dispatch compared A with 'D' and jumped.
    assert!(lines[7].contains("  CPI 44 ") && lines[7].contains(" A=44 "), "{}", report);
    assert!(lines[8].contains("  JZ CMD_DUMP ") && lines[8].contains(" A=44 "), "{}", report);
    assert!(lines[9].starts_with(&format!("PC={:04X} ", cmd_dump)) && lines[9].contains(" HL=0081 "), "{}", report);
    assert_eq!(lines[10], "CMD_DUMP:");
    let (lo, hi) = (skip_spaces as u8, (skip_spaces >> 8) as u8);
    assert_eq!(lines[11], format!("{:04X}  CD {:02X} {:02X}  CALL SKIP_SPACES", cmd_dump, lo, hi));
    // One step into the call.
    let step = r.cmd("s");
    assert!(step.starts_with(&format!("PC={:04X} ", skip_spaces)), "{}", step);
    assert!(step.contains("\nSKIP_SPACES:\n"), "{}", step);
    // Back at the prompt the dump has been printed.
    r.cmd("b MAIN_LOOP");
    assert_eq!(r.cmd("c").lines().next(), Some(format!("* break {:04X} MAIN_LOOP", r.sym("MAIN_LOOP")).as_str()));
    let out = String::from_utf8(r.con.borrow_mut().take_output()).unwrap();
    assert!(out.ends_with("D 0200 020F\r\n0200: 00 00 00 00 00 00 00 00  00 00 00 00 00 00 00 00  ................\r\n"), "{:?}", out);
}

#[test]
fn watchpoint_catches_a_load_wrapping_into_the_workspace() {
    // L with count 0 moves 65536 bytes (TODO, Review findings): from 0200 up, through
    // FFFF, around to the workspace at 0080. The copy loop uses no stack, so it gets there.
    let mut r = rom();
    let cl_loop = r.sym("CL_LOOP");
    r.type_in("X T.BIN\rL 0 0200 0\r");
    r.cmd("b CL_LOOP");
    r.cmd("c");
    r.cmd("bc");
    r.cmd("w 0080-00FF w");
    let report = r.cmd("c");
    let lines: Vec<&str> = report.lines().collect();
    assert_eq!(lines[0], "* watch write 0080 FF", "{}", report);
    // The culprit is the last ring line: STAX D with DE = 0080.
    assert!(lines[8].starts_with(&format!("{:04X}  12        STAX D ", cl_loop + 2)), "{}", report);
    assert!(lines[8].contains(" DE=0080 "), "{}", report);
    assert!(lines[9].starts_with(&format!("PC={:04X} ", cl_loop + 3)), "{}", report);
}

#[test]
fn io_break_and_port_trace_of_a_mount() {
    let mut r = rom();
    r.cmd("b MAIN_LOOP");
    r.cmd("c");
    r.cmd("bc");
    let path = r.trace_path();
    r.cmd(&format!("t {}", path));
    r.type_in("X AB\r");
    r.cmd("io 0E out");
    let report = r.cmd("c");
    let lines: Vec<&str> = report.lines().collect();
    let value = lines[0].strip_prefix("* io OUT 0E ").unwrap_or_else(|| panic!("{}", report));
    assert!(lines[8].contains("  OUT 0E "), "{}", report);
    assert!(lines[9].contains(&format!(" A={} ", value)), "{}", report);
    r.cmd("bc");
    r.cmd("b MAIN_LOOP");
    r.cmd("c");
    r.cmd("t off");
    let trace = std::fs::read_to_string(&path).unwrap();
    // ARCHITECTURE 7.3: IN pp vv / OUT pp vv, uppercase hex, and the repeat annotation.
    let hex2 = |h: &str| h.len() == 2 && h.chars().all(|c| matches!(c, '0'..='9' | 'A'..='F'));
    for line in trace.lines() {
        let (event, repeats) = match line.split_once(" ; x") {
            Some((event, n)) => (event, n.parse::<u64>().unwrap()),
            None => (line, 2),
        };
        let words: Vec<&str> = event.split(' ').collect();
        assert!(repeats > 1 && words.len() == 3 && ["IN", "OUT"].contains(&words[0]) && hex2(words[1]) && hex2(words[2]),
            "bad trace line {:?}", line);
    }
    // The console side: X is read from the FIFO and echoed.
    assert!(trace.contains("\nIN 01 58\n") && trace.contains("\nOUT 00 58\n"), "{}", trace);
    // The mount side (DEVICE_SPECS 7). This list changes when the ROM sends OUT 0E,03 first (TODO).
    let mount: Vec<&str> = trace.lines().filter(|l| [" 0D ", " 0E ", " 0F "].iter().any(|p| l.contains(p))).collect();
    assert_eq!(mount, ["OUT 0D 41", "OUT 0D 42", "OUT 0E 01", "IN 0F 00"]);
    assert!(r.dir.path().join("AB").exists());
}

// ---------- The binary: CLI and --script (7.4, Entry) ----------

/// Runs the emulator binary in a fresh directory holding rom/monitor.bin and rom/monitor.sym,
/// with `stdin` piped in. Returns the exit code and stdout.
fn emulator(args: &[&str], script: &str, stdin: &[u8]) -> (Option<i32>, String) {
    use std::io::Write;
    use std::process::{Command, Stdio};
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir(dir.path().join("rom")).unwrap();
    for f in ["monitor.bin", "monitor.sym"] {
        std::fs::copy(Path::new("rom").join(f), dir.path().join("rom").join(f)).unwrap();
    }
    std::fs::write(dir.path().join("s.dbg"), script).unwrap();
    let mut child = Command::new(env!("CARGO_BIN_EXE_intel8080"))
        .args(args)
        .current_dir(dir.path())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    child.stdin.take().unwrap().write_all(stdin).unwrap();
    let out = child.wait_with_output().unwrap();
    (out.status.code(), String::from_utf8_lossy(&out.stdout).into_owned())
}

#[test]
fn script_runs_echoed_skips_comments_and_quits_when_it_runs_out() {
    let cmd_dump = rom().sym("CMD_DUMP");
    let (code, out) = emulator(&["--script", "s.dbg"], "# a comment\n\n  b CMD_DUMP\nc\nr\n", b"D 0200 020F\r");
    assert_eq!(code, Some(0), "{}", out);
    assert!(out.contains("\n* start\n"), "{}", out);
    assert!(out.contains("\ndbg> b CMD_DUMP\ndbg> c\n"), "{}", out);
    assert!(out.contains(&format!("\r\n* break {:04X} CMD_DUMP\n", cmd_dump)), "{}", out);
    assert!(out.contains(&format!("\ndbg> r\nPC={:04X} ", cmd_dump)), "{}", out);
    assert!(!out.contains("comment") && !out.contains("dbg> \n"), "{}", out);
    assert!(!out.contains("HLT at"), "{}", out);
}

#[test]
fn debug_starts_stopped_and_bad_arguments_exit_2() {
    let (code, out) = emulator(&["--debug"], "", b"");
    assert_eq!(code, Some(0), "{}", out);
    assert!(out.contains("\n* start\n") && out.contains("\n0000  31 00 F0  LXI SP,F000\n"), "{}", out);
    assert_eq!(emulator(&["--bogus"], "", b"").0, Some(2));
    assert_eq!(emulator(&["--script"], "", b"").0, Some(2));
    assert_eq!(emulator(&["--script", "missing.dbg"], "", b"").0, Some(2));
}
