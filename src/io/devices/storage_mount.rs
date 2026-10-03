// storage_mount.rs - File mounting service for Storage device
//
// Port 0x0D: Filename char (write only)
// Port 0x0E: Control (write only)
// Port 0x0F: Status (read only)
//
// Control commands:
//   0x01: Mount (open file with accumulated filename)
//   0x02: Unmount
//   0x03: Query mount status
//
// Status codes:
//   0x00: OK / Mounted
//   0x01: File not found (or error opening)
//   0x02: Invalid filename
//   0xFF: Busy (not used, but reserved)

use crate::io::IoDevice;
use super::storage::Storage;
use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;

pub struct StorageMount {
    storage: Rc<RefCell<Storage>>,
    base_path: PathBuf,
    filename_buffer: Vec<u8>,
    status: u8,
}

impl StorageMount {
    pub fn new(storage: Rc<RefCell<Storage>>, base_path: PathBuf) -> Self {
        StorageMount {
            storage,
            base_path,
            filename_buffer: Vec::with_capacity(13),  // 8.3 + null
            status: 0x00,
        }
    }

    fn do_mount(&mut self) {
        // Build filename from buffer
        let filename: String = self.filename_buffer.iter()
            .take_while(|&&c| c != 0)
            .map(|&c| c as char)
            .collect();

        // Validate
        if filename.is_empty() || filename.len() > 12 {
            self.status = 0x02;  // Invalid
            return;
        }

        // Allow only safe characters
        if !filename.chars().all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '-' || c == '_') {
            self.status = 0x02;  // Invalid
            return;
        }

        let path = self.base_path.join(&filename);
        
        match self.storage.borrow_mut().mount(&path) {
            Ok(()) => self.status = 0x00,
            Err(_) => self.status = 0x01,  // Not found / error
        }
    }

    fn do_unmount(&mut self) {
        self.storage.borrow_mut().unmount();
        self.status = 0x00;
    }

    fn do_query(&mut self) {
        self.status = if self.storage.borrow().is_mounted() {
            0x00  // Mounted
        } else {
            0x01  // Not mounted
        };
    }
}

impl IoDevice for StorageMount {
    fn read(&mut self, port: u8) -> u8 {
        match port {
            0x0F => self.status,
            _ => 0xFF,
        }
    }

    fn write(&mut self, port: u8, value: u8) {
        match port {
            0x0D => {  // Filename char
                if value == 0 {
                    // Null terminator - don't add to buffer
                } else if self.filename_buffer.len() < 12 {
                    self.filename_buffer.push(value);
                }
            }
            0x0E => {  // Control
                match value {
                    0x01 => {
                        self.do_mount();
                        self.filename_buffer.clear();
                    }
                    0x02 => self.do_unmount(),
                    0x03 => self.do_query(),
                    _ => {}
                }
            }
            _ => {}
        }
    }
}
