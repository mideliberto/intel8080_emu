use crate::io::IoDevice;
use std::io::{self, Write};

pub struct TerminalOut;

impl IoDevice for TerminalOut {
    fn read(&mut self, _port: u8) -> u8 {
        0xFF  // not readable
    }
    
    fn write(&mut self, _port: u8, value: u8) {
        // write byte as ASCII to stdout
        print!("{}", value as char);
        io::stdout().flush().unwrap();
    }
}