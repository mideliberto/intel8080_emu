mod registers;
mod intel8080cpu;  // This is your 8080cpu.rs

use intel8080cpu::Intel8080;

fn main() {
    let mut cpu = Intel8080::new();
    
    // Test program
    let program = [
        0x06, 0x05,     // MVI B, 5
        0x0E, 0x03,     // MVI C, 3
        0x78,           // MOV A,B
        0x81,           // ADD C
        0x76,           // HLT
    ];
    
    cpu.load_program(&program, 0x0000);
    cpu.run();
    
    println!("Program finished!");
    println!("A={:02X} B={:02X} C={:02X}", 
             cpu.a, cpu.b, cpu.c);
}