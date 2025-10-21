use crate::io::IoDevice;

pub struct NullDevice;

impl IoDevice for NullDevice {
    fn read(&mut self, _port: u8) -> u8 {
        0xFF  // floating bus
    }
    
    fn write(&mut self, _port: u8, _value: u8) {
        // does nothing
    }
}