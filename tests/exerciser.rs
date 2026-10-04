// exerciser.rs - The four standard 8080 exercisers, run under a CP/M shim.
//
// Fetch them first (they are not in the repo): scripts/fetch_exercisers.sh
// Run: cargo test --release --test exerciser -- --ignored --nocapture
// (8080EXM takes about 20 s in release.) A missing .COM skips its test with a message.
//
// The shim is 8080 code, not a Rust hook, so the same bytes run on hardware: load
// SHIM at 0000 and the .COM at 0100, then G 0100. Only BDOS 2 (print E) and 9 (print
// the $-terminated string at DE) are served, by OUT to the console data port. The
// program ends by jumping to 0000, or returning to it; 0000 jumps into the monitor.
// The test stops when PC reaches 0000.

use intel8080_emu::io::build_bus;
use intel8080_emu::io::devices::ask::AskConfig;
use intel8080_emu::io::devices::mailbox;
use intel8080_emu::Intel8080;

const BDOS: u16 = 0xEE00;  // CP/M programs put their stack below the BDOS entry (word at 0006)

/// (address, bytes)
const SHIM: [(u16, &[u8]); 3] = [
    // 0000: JMP F000. Warm boot: restart the monitor. F000 (COLD_START) is the ROM's only
    // fixed address; WARM moves between builds and is not published (ARCHITECTURE 2).
    (0x0000, &[0xC3, 0x00, 0xF0]),
    // 0005: JMP BDOS
    (0x0005, &[0xC3, 0x00, 0xEE]),
    (BDOS, &[
        0x79,             // EE00  MOV  A,C
        0xFE, 0x02,       // EE01  CPI  2
        0xCA, 0x13, 0xEE, // EE03  JZ   EE13
        0xFE, 0x09,       // EE06  CPI  9
        0xC0,             // EE08  RNZ             other functions: ignored
        0x1A,             // EE09  LDAX D          function 9
        0xFE, 0x24,       // EE0A  CPI  '$'
        0xC8,             // EE0C  RZ
        0xD3, 0x00,       // EE0D  OUT  00h
        0x13,             // EE0F  INX  D
        0xC3, 0x09, 0xEE, // EE10  JMP  EE09
        0x7B,             // EE13  MOV  A,E        function 2
        0xD3, 0x00,       // EE14  OUT  00h
        0xC9,             // EE16  RET
    ]),
];

/// Runs `name` to completion; None if the file is absent.
fn run(name: &str, max_cycles: u64) -> Option<String> {
    let path = format!("{}/tests/data/exercisers/{}", env!("CARGO_MANIFEST_DIR"), name);
    let Ok(com) = std::fs::read(&path) else {
        println!("SKIPPED: {} not found; run scripts/fetch_exercisers.sh", path);
        return None;
    };
    let mut cpu = Intel8080::new();
    let dir = tempfile::tempdir().unwrap();
    let (bus, con) = build_bus(dir.path(), mailbox::local_time, AskConfig::default());
    *cpu.io_bus_mut() = bus;
    for (addr, bytes) in SHIM {
        cpu.load_program(bytes, addr);
    }
    cpu.load_program(&com, 0x0100);
    cpu.sp = BDOS - 2;
    cpu.write_word(cpu.sp, 0x0000);  // a final RET goes to 0000
    while cpu.pc != 0x0000 {
        assert!(!cpu.halted, "{}: HLT at PC={:04X}", name, cpu.pc);
        assert!(cpu.cycles < max_cycles, "{}: not done after {} cycles, PC={:04X}\n{}",
            name, max_cycles, cpu.pc, String::from_utf8_lossy(con.borrow().output()));
        cpu.execute_one();
    }
    let out = String::from_utf8_lossy(con.borrow().output()).into_owned();
    println!("== {} ({} cycles)\n{}", name, cpu.cycles, out);
    Some(out)
}

#[test]
#[ignore]
fn tst8080() {
    if let Some(out) = run("TST8080.COM", 10_000_000) {
        assert!(out.contains("CPU IS OPERATIONAL"), "{}", out);
    }
}

#[test]
#[ignore]
fn pre8080() {
    if let Some(out) = run("8080PRE.COM", 10_000_000) {
        assert!(out.contains("Preliminary tests complete") && !out.contains("ERROR"), "{}", out);
    }
}

#[test]
#[ignore]
fn cputest() {
    if let Some(out) = run("CPUTEST.COM", 1_000_000_000) {
        assert!(out.contains("CPU TESTS OK") && !out.contains("CPU FAILED"), "{}", out);
    }
}

#[test]
#[ignore]
fn exm8080() {
    if let Some(out) = run("8080EXM.COM", 50_000_000_000) {
        assert!(out.contains("Tests complete") && !out.contains("ERROR"), "{}", out);
    }
}
