// console.rs - Console I/O device (merged terminal output + keyboard input)
//
// Port 0x00: Data - write to output char, read to get input char
// Port 0x01: Status - bit 0 = char available, bit 1 = ready to send

use crate::io::IoDevice;
use std::io::{self, Read, Write};
use std::collections::VecDeque;

pub struct Console {
    input_buffer: VecDeque<u8>,
}

impl Console {
    pub fn new() -> Self {
        Console {
            input_buffer: VecDeque::new(),
        }
    }
    
    /// Queue a character for input (useful for testing or pasting)
    pub fn queue_input(&mut self, c: u8) {
        self.input_buffer.push_back(c);
    }
    
    /// Check if input is available
    pub fn has_input(&self) -> bool {
        !self.input_buffer.is_empty()
    }
}

impl IoDevice for Console {
    fn read(&mut self, port: u8) -> u8 {
        match port {
            0x00 => {
                // Data port - return buffered char or try to read from stdin
                if let Some(c) = self.input_buffer.pop_front() {
                    c
                } else {
                    // Blocking read from stdin
                    let mut buffer = [0u8; 1];
                    match io::stdin().read_exact(&mut buffer) {
                        Ok(_) => buffer[0],
                        Err(_) => 0x00,
                    }
                }
            }
            0x01 => {
                // Status port
                let mut status = 0u8;
                if !self.input_buffer.is_empty() {
                    status |= 0x01;  // Input available
                }
                status |= 0x02;  // Always ready to send (output never blocks)
                status
            }
            _ => 0xFF,
        }
    }
    
    fn write(&mut self, port: u8, value: u8) {
        match port {
            0x00 => {
                // Data port - output character
                print!("{}", value as char);
                io::stdout().flush().unwrap();
            }
            0x01 => {
                // Status port - ignore writes
            }
            _ => {}
        }
    }
}
