use std::rc::Rc;
use std::cell::RefCell;
use std::path::PathBuf;

use intel8080_emu::Intel8080;
use intel8080_emu::io::devices::console::Console;
use intel8080_emu::io::devices::storage::Storage;
use intel8080_emu::io::devices::storage_mount::StorageMount;

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
        
    // Set up console device on ports 0x00-0x02
    let console = Rc::new(RefCell::new(Console::new()));
    cpu.io_bus_mut().map_port(0x00, console.clone());
    cpu.io_bus_mut().map_port(0x01, console.clone());
    cpu.io_bus_mut().map_port(0x02, console);
    
    // Create storage directory
    std::fs::create_dir_all("./storage/").ok();
    
    // Set up storage device on ports 0x08-0x0C
    let storage = Rc::new(RefCell::new(Storage::new()));
    cpu.io_bus_mut().map_port(0x08, storage.clone());
    cpu.io_bus_mut().map_port(0x09, storage.clone());
    cpu.io_bus_mut().map_port(0x0A, storage.clone());
    cpu.io_bus_mut().map_port(0x0B, storage.clone());
    cpu.io_bus_mut().map_port(0x0C, storage.clone());
    
    // Set up mount service on ports 0x0D-0x0F
    let mount = Rc::new(RefCell::new(StorageMount::new(
        Rc::clone(&storage),
        PathBuf::from("./storage/")
    )));
    cpu.io_bus_mut().map_port(0x0D, mount.clone());
    cpu.io_bus_mut().map_port(0x0E, mount.clone());
    cpu.io_bus_mut().map_port(0x0F, mount);
    
    // Load ROM (mapped at 0xF000, visible at 0x0000 via overlay)
    cpu.load_rom_from_file(std::path::Path::new("rom/monitor.bin"))
        .expect("Failed to load ROM");
    
    // new() left the CPU in its RESET state: PC=0000, ROM overlay set.
    cpu.run();

    // v1 has no interrupt source, so a halt is final (ARCHITECTURE 7.2).
    disable_raw_mode().expect("Failed to disable raw mode");
    println!("\nHLT at PC={:04X}", cpu.pc);
}
