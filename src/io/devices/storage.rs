// storage.rs - Storage (ports 08-0C) and mount (0D-0F): one device, DEVICE_SPECS 6-7.
//
// One file from one flat directory, addressed linearly with 24 bits. No sectors,
// no tracks, no banks. Just bytes.

use crate::io::IoDevice;
use std::fs::{File, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::PathBuf;

/// The address reaches 0xFFFFFF, so a file can hold 0x1000000 bytes.
const MAX_SIZE: u64 = 0x100_0000;

/// The mount name rule (DEVICE_SPECS 7, Mount steps 2-3): `a`-`z` fold to `A`-`Z`, then
/// 1-12 characters of `A-Z 0-9 . - _`, not `.` or `..`. None is "invalid" (02). Mount and
/// the mailbox `GET > FILE` (DEVICE_SPECS 8, GET) both use it.
pub fn file_name(name: &[u8]) -> Option<String> {
    let name = name.to_ascii_uppercase();
    let valid = !name.is_empty()
        && name.len() <= 12
        && name != b"."
        && name != b".."
        && name.iter().all(|&c| c.is_ascii_uppercase() || c.is_ascii_digit() || b".-_".contains(&c));
    // Valid names are ASCII, so from_utf8 can't fail.
    valid.then(|| String::from_utf8(name).unwrap())
}

pub struct Storage {
    dir: PathBuf,
    file: Option<File>,
    size: u32,
    address: u32,
    /// Filename bytes since the last OUT 0E. Holds at most 13: 13 means "too long".
    name: Vec<u8>,
    mount_status: u8,
}

impl Storage {
    /// Power-on state. Creates `dir` if it is missing; if that fails, every mount
    /// returns 01 because the open fails.
    pub fn new(dir: PathBuf) -> Self {
        let _ = std::fs::create_dir_all(&dir);
        Storage { dir, file: None, size: 0, address: 0, name: Vec::new(), mount_status: 0x01 }
    }

    /// Flushes durably and closes. An fsync error here is not reported (DEVICE_SPECS 6).
    fn unmount(&mut self) {
        if let Some(file) = self.file.take() {
            let _ = file.sync_all();
        }
        self.size = 0;
        self.address = 0;
    }

    fn mount(&mut self) -> u8 {
        self.unmount();
        let Some(name) = file_name(&self.name) else {
            return 0x02;
        };
        let path = self.dir.join(name);
        let Ok(file) = OpenOptions::new().read(true).write(true).create(true).truncate(false).open(path) else {
            return 0x01;
        };
        match file.metadata() {
            Ok(m) if m.len() <= MAX_SIZE => {
                self.size = m.len() as u32;
                self.file = Some(file);
                0x00
            }
            _ => 0x01,
        }
    }

    fn advance(&mut self) {
        self.address = (self.address + 1) & 0xFF_FFFF;
    }

    fn read_data(&mut self) -> u8 {
        let Some(file) = &mut self.file else {
            self.advance();
            return 0xFF;
        };
        if self.address >= self.size {
            self.advance();
            return 0xFF;
        }
        let mut buf = [0u8];
        match file.seek(SeekFrom::Start(self.address as u64)).and_then(|_| file.read_exact(&mut buf)) {
            Ok(()) => {
                self.advance();
                buf[0]
            }
            Err(_) => {
                self.unmount();
                0xFF
            }
        }
    }

    fn write_data(&mut self, value: u8) {
        let Some(file) = &mut self.file else {
            self.advance();
            return;
        };
        match file.seek(SeekFrom::Start(self.address as u64)).and_then(|_| file.write_all(&[value])) {
            Ok(()) => {
                self.size = self.size.max(self.address + 1);
                self.advance();
            }
            Err(_) => self.unmount(),
        }
    }

    fn flush(&mut self) {
        if let Some(file) = &self.file {
            if file.sync_all().is_err() {
                self.unmount();
            }
        }
    }
}

impl Drop for Storage {
    fn drop(&mut self) {
        self.unmount();
    }
}

impl IoDevice for Storage {
    fn read(&mut self, port: u8) -> u8 {
        match port {
            0x08 => self.address as u8,
            0x09 => (self.address >> 8) as u8,
            0x0A => (self.address >> 16) as u8,
            0x0B => self.read_data(),
            0x0C => 0x02 | self.file.is_some() as u8 | ((self.address >= self.size) as u8) << 7,
            0x0F => self.mount_status,
            _ => 0xFF,
        }
    }

    fn write(&mut self, port: u8, value: u8) {
        match port {
            0x08 => self.address = (self.address & 0xFF_FF00) | value as u32,
            0x09 => self.address = (self.address & 0xFF_00FF) | (value as u32) << 8,
            0x0A => self.address = (self.address & 0x00_FFFF) | (value as u32) << 16,
            0x0B => self.write_data(value),
            0x0C => match value {
                0x00 => self.address = 0,
                0x01 => self.address = self.address.wrapping_sub(1) & 0xFF_FFFF,
                0x02 => self.flush(),
                _ => {}
            },
            0x0D => {
                if value != 0 && self.name.len() <= 12 {
                    self.name.push(value);
                }
            }
            0x0E => {
                match value {
                    0x01 => self.mount_status = self.mount(),
                    0x02 => {
                        self.unmount();
                        self.mount_status = 0x00;
                    }
                    0x03 => self.mount_status = if self.file.is_some() { 0x00 } else { 0x01 },
                    _ => {}
                }
                self.name.clear();
            }
            _ => {}
        }
    }
}
