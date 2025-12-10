use std::rc::Rc;
use std::cell::RefCell;

use intel8080_emu::Intel8080;
use intel8080_emu::io::devices::console::Console;

use crossterm::terminal::{enable_raw_mode, disable_raw_mode};

const BUILD_TIMESTAMP: &str = env!("BUILD_TIMESTAMP");


fn main() {
    println!("8080 Emulator");
    println!("Built: {}", BUILD_TIMESTAMP);
    enable_raw_mode().expect("Failed to enable raw mode");
    
    let default_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let _ = disable_raw_mode();
        default_hook(info);
    }));

    let mut cpu = Intel8080::new();
    
    // Test program: print "Hi\n"
    let program = [
        0x3E, 0x48,     // MVI A, 'H'
        0xD3, 0x00,     // OUT 0 (console data)
        0x3E, 0x69,     // MVI A, 'i'
        0xD3, 0x00,     // OUT 0
        0x3E, 0x0A,     // MVI A, '\n'
        0xD3, 0x00,     // OUT 0
        0x76,           // HLT
    ];
    
    // Set up console device on ports 0x00-0x01
    let console = Rc::new(RefCell::new(Console::new()));
    cpu.io_bus_mut().map_port(0x00, console.clone());
    cpu.io_bus_mut().map_port(0x01, console.clone());
    cpu.io_bus_mut().map_port(0x02, console);
    
    //cpu.load_program(&program, 0x0000);
    cpu.load_program_from_file(std::path::Path::new("/Users/mike/intel8080_emu/rom/monitor.bin"), 0xF000).expect("Failed to load program");
    cpu.run();
    
    println!("Program finished!");
    println!("A={:02X} B={:02X} C={:02X}", cpu.a, cpu.b, cpu.c);
    disable_raw_mode().expect("Failed to disable raw mode");

}
