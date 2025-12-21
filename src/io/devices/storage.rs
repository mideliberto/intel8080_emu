// storage.rs - Linear-addressed storage device
//
// 24-bit addressing = 16MB address space
// No sectors, no tracks, no banks. Just bytes.
//
// Port 0x08: Address low byte
// Port 0x09: Address mid byte  
// Port 0x0A: Address high byte
// Port 0x0B: Data (read/write with auto-increment)
// Port 0x0C: Read = Status, Write = Control
//
// Status bits:
//   Bit 0: Mounted (1 = file mounted)
//   Bit 1: Ready (always 1)
//   Bit 7: EOF (address >= file size)
//
// Control commands:
//   0x00: Reset address to 0
//   0x01: Decrement address
//   0x02: Flush write buffer

use crate::io::IoDevice;
use std::fs::{File, OpenOptions};
use std::io::{Read, Write, Seek, SeekFrom};
use std::path::PathBuf;

pub struct Storage {
    file: Option<File>,
    address: u32,       // 24-bit, stored in 32 for convenience
    file_size: u32,
}

impl Storage {
    pub fn new() -> Self {
        Storage {
            file: None,
            address: 0,
            file_size: 0,
        }
    }

    /// Mount a file for storage operations
    pub fn mount(&mut self, path: &PathBuf) -> Result<(), String> {
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .open(path)
            .map_err(|e| e.to_string())?;

        let metadata = file.metadata().map_err(|e| e.to_string())?;
        self.file_size = metadata.len() as u32;
        self.file = Some(file);
        self.address = 0;
        Ok(())
    }

    /// Unmount current file
    pub fn unmount(&mut self) {
        if let Some(ref mut f) = self.file {
            let _ = f.flush();
        }
        self.file = None;
        self.file_size = 0;
        self.address = 0;
    }

    pub fn is_mounted(&self) -> bool {
        self.file.is_some()
    }

    fn read_data(&mut self) -> u8 {
        if let Some(ref mut file) = self.file {
            if file.seek(SeekFrom::Start(self.address as u64)).is_ok() {
                let mut buf = [0u8; 1];
                if file.read_exact(&mut buf).is_ok() {
                    self.increment_address();
                    return buf[0];
                }
            }
        }
        0xFF
    }

    fn write_data(&mut self, value: u8) {
        if let Some(ref mut file) = self.file {
            if file.seek(SeekFrom::Start(self.address as u64)).is_ok() {
                let _ = file.write_all(&[value]);
                // Expand file size tracking if we wrote past end
                if self.address >= self.file_size {
                    self.file_size = self.address + 1;
                }
                self.increment_address();
            }
        }
    }

    fn flush(&mut self) {
        if let Some(ref mut file) = self.file {
            let _ = file.flush();
        }
    }

    fn increment_address(&mut self) {
        // 24-bit wrap
        self.address = (self.address.wrapping_add(1)) & 0x00FF_FFFF;
    }

    fn decrement_address(&mut self) {
        // 24-bit wrap
        self.address = (self.address.wrapping_sub(1)) & 0x00FF_FFFF;
    }
}

impl IoDevice for Storage {
    fn read(&mut self, port: u8) -> u8 {
        match port {
            0x08 => self.address as u8,                         // ADDR_LO
            0x09 => (self.address >> 8) as u8,                  // ADDR_MID
            0x0A => (self.address >> 16) as u8,                 // ADDR_HI
            0x0B => self.read_data(),                           // DATA
            0x0C => {                                           // STATUS
                let mut status = 0x02;  // Bit 1: always ready
                if self.is_mounted() {
                    status |= 0x01;     // Bit 0: mounted
                }
                if self.address >= self.file_size {
                    status |= 0x80;     // Bit 7: EOF
                }
                status
            }
            _ => 0xFF,
        }
    }

    fn write(&mut self, port: u8, value: u8) {
        match port {
            0x08 => {  // ADDR_LO
                self.address = (self.address & 0x00FFFF00) | (value as u32);
            }
            0x09 => {  // ADDR_MID
                self.address = (self.address & 0x00FF00FF) | ((value as u32) << 8);
            }
            0x0A => {  // ADDR_HI
                self.address = (self.address & 0x0000FFFF) | ((value as u32) << 16);
            }
            0x0B => {  // DATA
                self.write_data(value);
            }
            0x0C => {  // CONTROL
                match value {
                    0x00 => self.address = 0,           // Reset address
                    0x01 => self.decrement_address(),   // Decrement
                    0x02 => self.flush(),               // Flush
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
    use std::io::Write;

    fn temp_file_with_data(data: &[u8]) -> (tempfile::TempDir, PathBuf) {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test.bin");
        let mut file = File::create(&path).unwrap();
        file.write_all(data).unwrap();
        file.flush().unwrap();
        (dir, path)
    }

    #[test]
    fn test_mount_unmount() {
        let (_dir, path) = temp_file_with_data(&[0x41, 0x42, 0x43]);
        let mut storage = Storage::new();
        
        assert!(!storage.is_mounted());
        storage.mount(&path).unwrap();
        assert!(storage.is_mounted());
        storage.unmount();
        assert!(!storage.is_mounted());
    }

    #[test]
    fn test_24bit_address_set_get() {
        let mut storage = Storage::new();
        
        // Set address to 0x123456
        storage.write(0x08, 0x56);  // low
        storage.write(0x09, 0x34);  // mid
        storage.write(0x0A, 0x12);  // high
        
        assert_eq!(storage.read(0x08), 0x56);
        assert_eq!(storage.read(0x09), 0x34);
        assert_eq!(storage.read(0x0A), 0x12);
        assert_eq!(storage.address, 0x123456);
    }

    #[test]
    fn test_read_with_auto_increment() {
        let (_dir, path) = temp_file_with_data(&[0x41, 0x42, 0x43, 0x44]);
        let mut storage = Storage::new();
        storage.mount(&path).unwrap();
        
        // Reset address
        storage.write(0x0C, 0x00);
        
        // Read with auto-increment
        assert_eq!(storage.read(0x0B), 0x41);
        assert_eq!(storage.read(0x0B), 0x42);
        assert_eq!(storage.read(0x0B), 0x43);
        assert_eq!(storage.read(0x0B), 0x44);
        
        // Address should be 4 now
        assert_eq!(storage.address, 4);
    }

    #[test]
    fn test_write_with_auto_increment() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("write_test.bin");
        
        let mut storage = Storage::new();
        storage.mount(&path).unwrap();
        
        // Write bytes
        storage.write(0x0C, 0x00);  // Reset address
        storage.write(0x0B, 0xAA);
        storage.write(0x0B, 0xBB);
        storage.write(0x0C, 0x02);  // Flush
        
        // Read back
        storage.write(0x0C, 0x00);  // Reset address
        assert_eq!(storage.read(0x0B), 0xAA);
        assert_eq!(storage.read(0x0B), 0xBB);
    }

    #[test]
    fn test_status_bits() {
        let mut storage = Storage::new();
        
        // Not mounted: bit 0 = 0, bit 1 = 1 (ready)
        let status = storage.read(0x0C);
        assert_eq!(status & 0x01, 0);   // not mounted
        assert_eq!(status & 0x02, 0x02); // ready
    }

    #[test]
    fn test_eof_status() {
        let (_dir, path) = temp_file_with_data(&[0x41, 0x42]);
        let mut storage = Storage::new();
        storage.mount(&path).unwrap();
        
        // At address 0, file size 2 - not EOF
        let status = storage.read(0x0C);
        assert_eq!(status & 0x80, 0);
        
        // Set address to 2 (past end)
        storage.write(0x08, 0x02);
        let status = storage.read(0x0C);
        assert_eq!(status & 0x80, 0x80);  // EOF
    }

    #[test]
    fn test_address_wrap_at_24bit() {
        let mut storage = Storage::new();
        
        // Set address to 0xFFFFFF
        storage.write(0x08, 0xFF);
        storage.write(0x09, 0xFF);
        storage.write(0x0A, 0xFF);
        
        // Increment should wrap to 0
        storage.increment_address();
        assert_eq!(storage.address, 0);
    }

    #[test]
    fn test_decrement_command() {
        let mut storage = Storage::new();
        
        storage.write(0x08, 0x05);  // address = 5
        storage.write(0x0C, 0x01);  // decrement
        
        assert_eq!(storage.read(0x08), 0x04);
    }
}
