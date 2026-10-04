// debugger_tests.rs - The debugger (ARCHITECTURE 7.4) driven through its command parser,
// the way a --script drives it: on small RAM programs for exact formats, and on the
// real ROM with rom/monitor.sym for symbols, breaks, watchpoints, I/O breaks and the trace.

use std::cell::RefCell;
use std::path::Path;
use std::rc::Rc;

use intel8080_emu::debugger::{Debugger, Flow};
use intel8080_emu::disasm::disassemble;
use intel8080_emu::io::build_bus;
use intel8080_emu::io::devices::ask::AskConfig;
use intel8080_emu::io::devices::mailbox;
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
    let (bus, con) = build_bus(dir.path(), mailbox::local_time, AskConfig::default());
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
            Flow::Stay | Flow::Error => out,
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
        assert_eq!(r.dbg.command(&mut r.cpu, line), (Flow::Error, err.to_string()), "{}", line);
    }
    assert_eq!(r.dbg.command(&mut r.cpu, ""), (Flow::Stay, String::new()));
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
    // An `in` break ignores an OUT to its port: the OUT 10 runs past to the OUT FE.
    let mut r = ram(&[0x3E, 0x55, 0xD3, 0x10, 0xDB, 0x02, 0xDB, 0x02, 0xDB, 0x02, 0xD3, 0xFE, 0xDB, 0xFF, 0x76]);
    r.cmd("io 10 in");
    r.cmd("io FE out");
    assert_eq!(r.cmd("c").lines().next(), Some("* io OUT FE 02"));
}

#[test]
fn port_trace_reopened_on_the_same_file_has_only_the_new_trace() {
    // ARCHITECTURE 7.4: `t FILE` truncates FILE, even while it is the file being traced
    // and an `s` that ended without a stop still holds a line back.
    // MVI B,0C / L: IN 02 / DCR B / JNZ L / OUT 10 / IN 02 / IN 02 / HLT
    let mut r = ram(&[0x06, 0x0C, 0xDB, 0x02, 0x05, 0xC2, 0x02, 0x01, 0xD3, 0x10, 0xDB, 0x02, 0xDB, 0x02, 0x76]);
    let path = r.trace_path();
    r.cmd(&format!("t {}", path));
    r.cmd("b 0108");
    r.cmd("c"); // the stop writes IN 02 02 ; x12
    r.cmd("bc");
    r.cmd("s 2"); // OUT 10, IN 02: held back
    r.cmd(&format!("t {}", path));
    r.cmd("s");
    r.cmd("t off");
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "IN 02 02\n");
    // A FILE that cannot be opened changes nothing: the trace goes on.
    let mut r = ram(&[0x06, 0x0C, 0xDB, 0x02, 0x05, 0xC2, 0x02, 0x01, 0xD3, 0x10, 0xDB, 0x02, 0xDB, 0x02, 0x76]);
    r.cmd(&format!("t {}", path));
    r.cmd("s 2");
    let bad = r.dir.path().join("no/such/dir").to_str().unwrap().to_string();
    assert!(r.cmd(&format!("t {}", bad)).starts_with("? "));
    r.cmd("s 3");
    r.cmd("t off");
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "IN 02 02 ; x2\n");
}

#[cfg(unix)]
#[test]
fn port_trace_to_a_pipe() {
    // ARCHITECTURE 7.4: FILE may be a pipe (/dev/stdout piped), which cannot be truncated;
    // re-pointing a running trace at one closes the old trace with its held-back line written.
    // MVI B,0C / L: IN 02 / DCR B / JNZ L / OUT 10 / IN 02 / IN 02 / HLT
    use std::io::Read;
    use std::os::fd::FromRawFd;
    let mut r = ram(&[0x06, 0x0C, 0xDB, 0x02, 0x05, 0xC2, 0x02, 0x01, 0xD3, 0x10, 0xDB, 0x02, 0xDB, 0x02, 0x76]);
    let path = r.trace_path();
    let mut fds = [0; 2];
    assert_eq!(unsafe { libc::pipe(fds.as_mut_ptr()) }, 0);
    let (mut rd, wr) = unsafe { (std::fs::File::from_raw_fd(fds[0]), std::fs::File::from_raw_fd(fds[1])) };
    r.cmd(&format!("t {}", path));
    r.cmd("s 2");
    assert_eq!(r.cmd(&format!("t /dev/fd/{}", fds[1])), "");
    drop(wr); // the trace holds its own write end
    r.cmd("s 3");
    r.cmd("t off");
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "IN 02 02\n");
    let mut piped = String::new();
    rd.read_to_string(&mut piped).unwrap();
    assert_eq!(piped, "IN 02 02\n");
}

#[test]
fn port_trace_repeat_rule() {
    // ARCHITECTURE 7.3: a run of N > 1 identical lines is `<line> ; xN`, N in decimal;
    // a single line has no annotation; a run ends at the first different line.
    // MVI B,0C / L: IN 02 / DCR B / JNZ L / OUT 10 / IN 02 / IN 02 / HLT
    let mut r = ram(&[0x06, 0x0C, 0xDB, 0x02, 0x05, 0xC2, 0x02, 0x01, 0xD3, 0x10, 0xDB, 0x02, 0xDB, 0x02, 0x76]);
    let path = r.trace_path();
    r.cmd(&format!("t {}", path));
    r.cmd("b 010C");
    r.cmd("c");
    // A stop writes the held-back line.
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "IN 02 02 ; x12\nOUT 10 02\nIN 02 02\n");
    r.cmd("s 2");
    r.cmd("t off");
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "IN 02 02 ; x12\nOUT 10 02\nIN 02 02\nIN 02 02\n");
}

// ---------- The real ROM ----------

#[test]
fn workspace_symbols_match_architecture_1_1() {
    // rom/monitor.asm declares the workspace as labels (ORG 80h + DS), so monitor.sym has
    // one line per named row of the table, at its address.
    let doc = std::fs::read_to_string("docs/ARCHITECTURE.md").unwrap();
    let table = doc.split("### 1.1 Workspace Layout").nth(1).unwrap().split("\n\n").nth(2).unwrap();
    let mut want = Vec::new();
    for row in table.lines().skip(2) {
        let cols: Vec<&str> = row.split('|').map(str::trim).collect();
        let name = cols[3].split(' ').next().unwrap();
        if name != "free" {
            want.push(format!("{} {}", &cols[1][..4], name));
        }
    }
    assert_eq!(want.len(), 10, "{}", table);
    let sym = std::fs::read_to_string("rom/monitor.sym").unwrap();
    let got: Vec<&str> = sym.lines().filter(|l| l < &"0100").collect();
    assert_eq!(got, want);
    // Named in the debugger; a user-area address is never NAME+n of the workspace.
    let mut r = rom();
    assert_eq!(r.cmd("sym stor_addr+2"), "00E9 STOR_ADDR+2\n");
    assert_eq!(r.cmd("sym 00FF"), "00FF REGS+15\n");
    assert_eq!(r.cmd("sym 0100"), "0100\n");
    assert_eq!(r.cmd("sym EFFF"), "EFFF\n");
    assert_eq!(r.cmd("sym 007F"), "007F\n");
    r.cmd("b BOOT_CONTINUE");
    r.cmd("c");
    assert!(r.cmd("u BOOT_CONTINUE 5").contains("  SHLD LAST_DUMP_ADDR\n"));
}

#[test]
fn break_at_a_rom_symbol() {
    let mut r = rom();
    let cmd_dump = r.sym("CMD_DUMP");
    let read_word = r.sym("READ_HEX_WORD");
    assert_eq!(r.cmd("sym cmd_dump+3"), format!("{:04X} CMD_DUMP+3\n", cmd_dump + 3));
    assert!(r.cmd("sym cmd_dump+100").starts_with(&format!("{:04X}", cmd_dump + 0x100)), "an offset above FF");
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
    let (lo, hi) = (read_word as u8, (read_word >> 8) as u8);
    assert_eq!(lines[11], format!("{:04X}  CD {:02X} {:02X}  CALL READ_HEX_WORD", cmd_dump, lo, hi));
    // One step into the call.
    let step = r.cmd("s");
    assert!(step.starts_with(&format!("PC={:04X} ", read_word)), "{}", step);
    assert!(step.contains("\nREAD_HEX_WORD:\n"), "{}", step);
    // Back at the prompt the dump has been printed.
    r.cmd("b MAIN_LOOP");
    assert_eq!(r.cmd("c").lines().next(), Some(format!("* break {:04X} MAIN_LOOP", r.sym("MAIN_LOOP")).as_str()));
    let out = String::from_utf8(r.con.borrow_mut().take_output()).unwrap();
    assert!(out.ends_with("D 0200 020F\r\n0200: 00 00 00 00 00 00 00 00  00 00 00 00 00 00 00 00  ................\r\n"), "{:?}", out);
}

#[test]
fn watchpoint_catches_a_load_wrapping_into_the_workspace() {
    // L's destination is not guarded and wraps (MONITOR_SPEC 6.8): 0101 bytes at FF80 run
    // through the ROM, past FFFF, and their last byte lands on the workspace at 0080.
    let mut r = rom();
    let cl_loop = r.sym("CL_LOOP");
    r.type_in("X T.BIN\rL 0 FF80 0101\r");
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
    // The mount side (DEVICE_SPECS 7, MONITOR_SPEC 6.13): the resync query, the name, mount, status.
    let mount: Vec<&str> = trace.lines().filter(|l| [" 0D ", " 0E ", " 0F "].iter().any(|p| l.contains(p))).collect();
    assert_eq!(mount, ["OUT 0E 03", "OUT 0D 41", "OUT 0D 42", "OUT 0E 01", "IN 0F 00"]);
    assert!(r.dir.path().join("AB").exists());
}

// ---------- The binary: CLI and --script (7.4, Entry) ----------

/// How long a run of the binary may take. A piped run ends only on a halt (ARCHITECTURE 7.2),
/// so a regression that never reaches its HLT fails here instead of hanging cargo test.
const DEADLINE: std::time::Duration = std::time::Duration::from_secs(30);

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
    // stdout goes to a file, so a child that never exits cannot block on a full pipe.
    let stdout = dir.path().join("stdout.txt");
    let mut child = Command::new(env!("CARGO_BIN_EXE_intel8080"))
        .env_remove("ANTHROPIC_API_KEY") // cargo test never reaches the API (PI_DAEMON 13.2)
        .args(args)
        .current_dir(dir.path())
        .stdin(Stdio::piped())
        .stdout(std::fs::File::create(&stdout).unwrap())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    child.stdin.take().unwrap().write_all(stdin).unwrap();
    let start = std::time::Instant::now();
    let status = loop {
        if let Some(status) = child.try_wait().unwrap() {
            break status;
        }
        if start.elapsed() > DEADLINE {
            let _ = child.kill();
            let _ = child.wait();
            panic!("{:?} still running after {:?}; stdout so far:\n{}", args, DEADLINE,
                String::from_utf8_lossy(&std::fs::read(&stdout).unwrap()));
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    };
    (status.code(), String::from_utf8_lossy(&std::fs::read(&stdout).unwrap()).into_owned())
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
fn a_bad_script_line_exits_2() {
    let (code, out) = emulator(&["--script", "s.dbg"], "r\nb NOPE\nr\n", b"");
    assert_eq!(code, Some(2), "{}", out);
    assert!(out.ends_with("\ndbg> b NOPE\n? unknown symbol: NOPE\n"), "{}", out);
}

#[test]
fn a_piped_or_scripted_halt_exits() {
    // G to a HLT at 0100. Not a terminal: the halt ends the run (ARCHITECTURE 7.2).
    let input = b"F 0100 0100 76\rG 0100\r";
    let (code, out) = emulator(&[], "", input);
    assert_eq!(code, Some(0), "{}", out);
    assert!(out.ends_with("\nHLT at PC=0101\n"), "{}", out);
    let (code, out) = emulator(&["--script", "s.dbg"], "c\n", input);
    assert_eq!(code, Some(0), "{}", out);
    assert!(out.ends_with("\nHLT at PC=0101\n") && !out.contains("* halt"), "{}", out);
}

#[test]
fn jp_we_fits_the_jumper() {
    // ARCHITECTURE 6.10, Emulator: a RAM program writes A5 to FFFE and toggle-polls; D
    // shows the ROM byte. Without --jp-we the write changes nothing. A HLT ends the run.
    let input = b":1303000021FEFF36A5062005C207037EAEE640C20B03C90F\rG 0300\rD FFF0 FFFF\rF 0100 0100 76\rG 0100\r";
    let line = |b: &str| format!("\r\nFFF0: FF FF FF FF FF FF FF FF  FF FF FF FF FF FF {} FF  ................\r\n", b);
    let (code, out) = emulator(&["--jp-we"], "", input);
    assert_eq!(code, Some(0), "{}", out);
    assert!(out.contains(&line("A5")) && out.ends_with("\nHLT at PC=0101\n"), "{}", out);
    let (code, out) = emulator(&[], "", input);
    assert_eq!(code, Some(0), "{}", out);
    assert!(out.contains(&line("FF")), "{}", out);
}

#[test]
fn jp_we_write_cycle_is_tblc_plus_20480_cycles() {
    // ARCHITECTURE 6.10, Emulator: `--jp-we` fits tWC = 20,480 cycles. A RAM program writes
    // A5 to FFFE, then counts DATA polls in BC until the byte reads back:
    // 0300 LXI H,FFFE / MVI M,A5 (at w) / LXI B,0 / L: INX B / MOV A,M / CPI A5 / JNZ L / HLT
    // Poll k's MOV runs at w + 25 + 29(k-1) and reads status while that is below
    // w + 307 + 20480, so k = 717 (2CDh) is the first to read the byte.
    let input = b":1003000021FEFF36A5010000037EFEA5C20803768C\rG 0300\r";
    let (code, out) = emulator(&["--jp-we", "--script", "s.dbg"], "b 030F\nc\n", input);
    assert_eq!(code, Some(0), "{}", out);
    assert!(out.contains("\n* break 030F\n") && out.contains(" A=A5 F=56 -ZAP- BC=02CD "), "{}", out);
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
