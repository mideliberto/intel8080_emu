// console.rs - Console I/O device
//
// Port 0x00: Data Out   - write to output char
// Port 0x01: Data In    - read to get input char
// Port 0x02: Status     - bit 0 = RX ready, bit 1 = TX ready

use crate::io::IoDevice;
use crossterm::event::{poll, read, Event, KeyCode, KeyEvent, KeyEventKind};
use crossterm::event::{KeyModifiers};
use crossterm::terminal::disable_raw_mode;
use std::collections::VecDeque;
use std::io::Write;
use std::time::Duration;

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
            0x01 => self.input_buffer.pop_front().unwrap_or(0),
            0x02 => {
                // Drain all pending events into buffer
                while poll(Duration::from_millis(1)).unwrap_or(false) {
                    if let Ok(event) = read() {
                        if let Event::Key(key_event) = event {
                            if key_event.kind == KeyEventKind::Press {  // ADD THIS
                                if let Some(c) = key_to_byte(key_event) {
                                    self.input_buffer.push_back(c);
                                }
                            }
                        }
                    }
                }

                // Status purely reflects buffer state
                let mut status = 0x02; // TX always ready
                if !self.input_buffer.is_empty() {
                    status |= 0x01;
                }
                status
            }
            _ => 0xFF,
        }
    }

    fn write(&mut self, port: u8, value: u8) {
        if port == 0x00 {
            print!("{}", value as char);
            std::io::stdout().flush().ok();
        }
    }
}

fn key_to_byte(key_event: KeyEvent) -> Option<u8> {
    match key_event.code {
        KeyCode::Char('c') if key_event.modifiers.contains(KeyModifiers::CONTROL) => {
            // Restore terminal before exit
            let _ = disable_raw_mode();
            std::process::exit(0);
        }
        KeyCode::Char(c) => Some(c as u8),
        KeyCode::Enter => Some(0x0D),
        KeyCode::Backspace => Some(0x08),
        _ => None,
    }
}
