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

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::File;
    use std::io::Write;

    fn setup() -> (tempfile::TempDir, Rc<RefCell<Storage>>, StorageMount) {
        let dir = tempfile::tempdir().unwrap();
        let storage = Rc::new(RefCell::new(Storage::new()));
        let mount = StorageMount::new(Rc::clone(&storage), dir.path().to_path_buf());
        (dir, storage, mount)
    }

    #[test]
    fn test_mount_file() {
        let (dir, storage, mut mount) = setup();
        
        // Create a test file
        let path = dir.path().join("TEST.BIN");
        let mut f = File::create(&path).unwrap();
        f.write_all(&[0xDE, 0xAD, 0xBE, 0xEF]).unwrap();
        
        // Send filename
        for c in b"TEST.BIN" {
            mount.write(0x0D, *c);
        }
        
        // Mount command
        mount.write(0x0E, 0x01);
        
        // Check status
        assert_eq!(mount.read(0x0F), 0x00);  // OK
        assert!(storage.borrow().is_mounted());
    }

    #[test]
    fn test_mount_nonexistent_creates_file() {
        let (_dir, storage, mut mount) = setup();
        
        // Send filename for file that doesn't exist
        for c in b"NEW.BIN" {
            mount.write(0x0D, *c);
        }
        
        mount.write(0x0E, 0x01);
        
        // Should succeed (creates file)
        assert_eq!(mount.read(0x0F), 0x00);
        assert!(storage.borrow().is_mounted());
    }

    #[test]
    fn test_invalid_filename() {
        let (_dir, _storage, mut mount) = setup();
        
        // Empty filename
        mount.write(0x0E, 0x01);
        assert_eq!(mount.read(0x0F), 0x02);  // Invalid
    }

    #[test]
    fn test_unmount() {
        let (dir, storage, mut mount) = setup();
        
        // Create and mount a file
        let path = dir.path().join("TEST.BIN");
        File::create(&path).unwrap();
        
        for c in b"TEST.BIN" {
            mount.write(0x0D, *c);
        }
        mount.write(0x0E, 0x01);
        assert!(storage.borrow().is_mounted());
        
        // Unmount
        mount.write(0x0E, 0x02);
        assert!(!storage.borrow().is_mounted());
    }

    #[test]
    fn test_query_status() {
        let (_dir, _storage, mut mount) = setup();
        
        // Query when not mounted
        mount.write(0x0E, 0x03);
        assert_eq!(mount.read(0x0F), 0x01);  // Not mounted
    }

    #[test]
    fn test_filename_with_bad_chars() {
        let (_dir, _storage, mut mount) = setup();
        
        // Path traversal attempt
        for c in b"../etc/passwd" {
            mount.write(0x0D, *c);
        }
        mount.write(0x0E, 0x01);
        
        assert_eq!(mount.read(0x0F), 0x02);  // Invalid
    }
}
