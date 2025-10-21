use crate::io::IoDevice;
use std::io::{self, Read};

pub struct KeyboardInput;

impl IoDevice for KeyboardInput {
    fn read(&mut self, _port: u8) -> u8 {
        let mut buffer = [0; 1];
        io::stdin().read_exact(&mut buffer).unwrap();
        buffer[0]
    }
    
    fn write(&mut self, _port: u8, _value: u8) {
        // not writable
    }
}