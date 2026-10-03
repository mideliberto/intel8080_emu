// console.rs - Console (ports 00-02), DEVICE_SPECS 4.
//
// An input FIFO and an output buffer. The host side (main.rs, the test harnesses,
// later the Pi daemon) fills the one and drains the other; the device never
// touches a terminal.

use crate::io::IoDevice;
use std::collections::VecDeque;

/// Output beyond this many undrained bytes is discarded (DEVICE_SPECS 4: at least 2 MiB).
pub const OUTPUT_CAP: usize = 2 * 1024 * 1024;

pub struct Console {
    input: VecDeque<u8>,
    output: Vec<u8>,
}

impl Console {
    /// Power-on state: both buffers empty.
    pub fn new() -> Self {
        Console { input: VecDeque::new(), output: Vec::new() }
    }

    /// Bytes arriving from the terminal, in order.
    pub fn push_input(&mut self, bytes: &[u8]) {
        self.input.extend(bytes);
    }

    /// Bytes sent with OUT 00 and not yet drained.
    pub fn output(&self) -> &[u8] {
        &self.output
    }

    /// Drains the output buffer.
    pub fn take_output(&mut self) -> Vec<u8> {
        std::mem::take(&mut self.output)
    }
}

impl IoDevice for Console {
    fn read(&mut self, port: u8) -> u8 {
        match port {
            0x01 => self.input.pop_front().unwrap_or(0x00),
            0x02 => 0x02 | !self.input.is_empty() as u8,
            _ => 0xFF,
        }
    }

    fn write(&mut self, port: u8, value: u8) {
        if port == 0x00 && self.output.len() < OUTPUT_CAP {
            self.output.push(value);
        }
    }
}
