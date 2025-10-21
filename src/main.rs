use std::rc::Rc;
use std::cell::RefCell;

// If you're using lib.rs:
use intel8080_emu::io::{IoBus, devices::terminal::TerminalOut};
use intel8080_emu::intel8080cpu::Intel8080;


fn main() {
    let mut cpu = Intel8080::new();
 /* 
    // Test program
    let program = [
        0x06, 0x05,     // MVI B, 5
        0x0E, 0x03,     // MVI C, 3
        0x78,           // MOV A,B
        0x81,           // ADD C
        0x76,           // HLT
    ];
*/
    let program = [
    0x3E, 0x48,     // MVI A, 'H'
    0xD3, 0x01,     // OUT 1
    0x3E, 0x69,     // MVI A, 'i'
    0xD3, 0x01,     // OUT 1
    0x3E, 0x0A,     // MVI A, '\n'
    0xD3, 0x01,     // OUT 1
    0x76,           // HLT
];
    
    let terminal = Rc::new(RefCell::new(TerminalOut));
    cpu.io_bus_mut().map_port(0x01, terminal);
    cpu.load_program(&program, 0x0000);
    cpu.run();
    
    println!("Program finished!");
    println!("A={:02X} B={:02X} C={:02X}", 
             cpu.a, cpu.b, cpu.c);
}