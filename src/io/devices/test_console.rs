// test_console.rs - Scripted console for automated testing
//
// Feeds pre-defined input and captures output for verification.

use crate::io::IoDevice;
use std::collections::VecDeque;

pub struct TestConsole {
    input: VecDeque<u8>,
    output: Vec<u8>,
}

impl TestConsole {
    pub fn new(input: &str) -> Self {
        TestConsole {
            input: input.bytes().collect(),
            output: Vec::new(),
        }
    }

    pub fn get_output(&self) -> String {
        String::from_utf8_lossy(&self.output).to_string()
    }

    pub fn output_bytes(&self) -> &[u8] {
        &self.output
    }

    pub fn clear_output(&mut self) {
        self.output.clear();
    }

    pub fn add_input(&mut self, input: &str) {
        self.input.extend(input.bytes());
    }
}

impl IoDevice for TestConsole {
    fn read(&mut self, port: u8) -> u8 {
        match port {
            0x01 => self.input.pop_front().unwrap_or(0),
            0x02 => {
                let mut status = 0x02; // TX always ready
                if !self.input.is_empty() {
                    status |= 0x01; // RX ready
                }
                status
            }
            _ => 0xFF,
        }
    }

    fn write(&mut self, port: u8, value: u8) {
        if port == 0x00 {
            self.output.push(value);
        }
    }
}
