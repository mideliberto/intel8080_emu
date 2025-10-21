use crate::io::IoDevice;
use std::fs::{File, OpenOptions};
use std::io::{Read, Write, Seek, SeekFrom};

pub struct DiskDevice {
    file: File,
    address: u16,  // current seek position
}

impl DiskDevice {
    pub fn new(path: &str) -> std::io::Result<Self> {
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .open(path)?;
        
        Ok(DiskDevice {
            file,
            address: 0,
        })
    }
}

impl IoDevice for DiskDevice {
    fn read(&mut self, port: u8) -> u8 {
        match port {
            0x20 => {  // data port - read and auto-increment
                let _ = self.file.seek(SeekFrom::Start(self.address as u64));
                let mut buffer = [0; 1];
                match self.file.read_exact(&mut buffer) {
                    Ok(_) => {
                        self.address = self.address.wrapping_add(1);
                        buffer[0]
                    }
                    Err(_) => 0x00,
                }
            }
            0x21 => {  // address low byte
                (self.address & 0xFF) as u8
            }
            0x22 => {  // address high byte
                (self.address >> 8) as u8
            }
            _ => 0xFF,
        }
    }
    
    fn write(&mut self, port: u8, value: u8) {
        match port {
            0x20 => {  // data port - write and auto-increment
                let _ = self.file.seek(SeekFrom::Start(self.address as u64));
                let _ = self.file.write_all(&[value]);
                let _ = self.file.flush();
                self.address = self.address.wrapping_add(1);
            }
            0x21 => {  // address low byte
                self.address = (self.address & 0xFF00) | (value as u16);
            }
            0x22 => {  // address high byte
                self.address = (self.address & 0x00FF) | ((value as u16) << 8);
            }
            _ => {}
        }
    }
}