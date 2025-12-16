// monitor_tests.rs - Integration tests for monitor ROM

use std::rc::Rc;
use std::cell::RefCell;
use std::path::Path;

use intel8080_emu::Intel8080;
use intel8080_emu::io::devices::test_console::TestConsole;

fn setup_monitor(input: &str) -> (Intel8080, Rc<RefCell<TestConsole>>) {
    let mut cpu = Intel8080::new();
    
    let console = Rc::new(RefCell::new(TestConsole::new(input)));
    cpu.io_bus_mut().map_port(0x00, console.clone());
    cpu.io_bus_mut().map_port(0x01, console.clone());
    cpu.io_bus_mut().map_port(0x02, console.clone());
    
    cpu.load_rom_from_file(Path::new("rom/monitor.bin"))
        .expect("Failed to load ROM");
    cpu.reset();
    
    (cpu, console)
}

fn run_cycles(cpu: &mut Intel8080, max_cycles: u64) {
    let start = cpu.cycles;
    while cpu.cycles - start < max_cycles && !cpu.halted {
        cpu.execute_one();
    }
}

#[test]
fn test_monitor_boots() {
    let (mut cpu, console) = setup_monitor("");
    run_cycles(&mut cpu, 500_000);
    
    let output = console.borrow().get_output();
    assert!(output.contains("8080 Monitor"), "Should show banner");
    assert!(output.contains(">"), "Should show prompt");
}

#[test]
fn test_dump_rom() {
    let (mut cpu, console) = setup_monitor("D F000 F00F\r");
    run_cycles(&mut cpu, 1_000_000);
    
    let output = console.borrow().get_output();
    assert!(output.contains("F000:"), "Should show address");
    assert!(output.contains("31"), "Should show LXI SP opcode");
}

#[test]
fn test_fill_command() {
    let (mut cpu, _console) = setup_monitor("F 0200 020F AA\r");
    run_cycles(&mut cpu, 1_000_000);
    
    // Verify memory was filled
    for addr in 0x0200..=0x020F {
        assert_eq!(cpu.read_byte(addr), 0xAA, "Address {:04X} should be AA", addr);
    }
}

#[test]
fn test_hex_math() {
    let (mut cpu, console) = setup_monitor("H 1234 0111\r");
    run_cycles(&mut cpu, 1_000_000);
    
    let output = console.borrow().get_output();
    assert!(output.contains("1345"), "Should show sum");
    assert!(output.contains("1123"), "Should show difference");
}

#[test]
fn test_move_command() {
    let (mut cpu, _console) = setup_monitor("F 0200 020F 55\rM 0200 0300 10\r");
    run_cycles(&mut cpu, 2_000_000);
    
    // Verify data was moved
    for addr in 0x0300..=0x030F {
        assert_eq!(cpu.read_byte(addr), 0x55, "Address {:04X} should be 55", addr);
    }
}

#[test]
fn test_search_finds_pattern() {
    // Fill, then search
    let (mut cpu, console) = setup_monitor("F 0500 0502 41\rS 0500 0510 41 41 41\r");
    run_cycles(&mut cpu, 2_000_000);
    
    let output = console.borrow().get_output();
    assert!(output.contains("0500"), "Should find pattern at 0500");
}

#[test]
fn test_compare_identical() {
    // Fill two regions with same value, compare should show no output
    let (mut cpu, console) = setup_monitor("F 0200 020F BB\rF 0300 030F BB\rC 0200 020F 0300\r");
    run_cycles(&mut cpu, 3_000_000);
    
    let output = console.borrow().get_output();
    // Count prompts - should have 4 (initial + after each command)
    let prompt_count = output.matches("> ").count();
    assert!(prompt_count >= 4, "Should complete all commands");
}

#[test]
fn test_io_read_status() {
    let (mut cpu, console) = setup_monitor("I 02\r");
    run_cycles(&mut cpu, 1_000_000);
    
    let output = console.borrow().get_output();
    // Status should show TX ready (02) or TX+RX ready (03)
    assert!(output.contains("02") || output.contains("03"), "Should show status");
}

#[test] 
fn test_help_command() {
    let (mut cpu, console) = setup_monitor("?\r");
    run_cycles(&mut cpu, 1_000_000);
    
    let output = console.borrow().get_output();
    assert!(output.contains("Commands:"), "Should show help header");
    assert!(output.contains("Dump"), "Should list D command");
    assert!(output.contains("Fill"), "Should list F command");
}

#[test]
fn test_overlay_disabled_after_boot() {
    let (mut cpu, _console) = setup_monitor("");
    run_cycles(&mut cpu, 500_000);
    
    // Overlay should be off after boot
    assert!(!cpu.rom_overlay_enabled, "Overlay should be disabled");
    
    // Low memory should be RAM (writable)
    cpu.write_byte(0x0000, 0x42);
    assert_eq!(cpu.read_byte(0x0000), 0x42, "Should write to RAM at 0x0000");
}
