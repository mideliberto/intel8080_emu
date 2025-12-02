// timer.rs - Simple interval timer device
//
// Port 0x30: Counter low byte (read), Reload low byte (write)
// Port 0x31: Counter high byte (read), Reload high + load counter (write)
// Port 0x32: Status/Control
//            Read:  bit 0 = enabled, bit 1 = interrupt pending
//            Write: bit 0 = enable, bit 1 = acknowledge interrupt

use crate::io::IoDevice;

pub struct Timer {
    counter: u16,
    reload_value: u16,
    enabled: bool,
    pub interrupt_pending: bool,
}

impl Timer {
    pub fn new() -> Self {
        Timer {
            counter: 0,
            reload_value: 0,
            enabled: false,
            interrupt_pending: false,
        }
    }
    
    /// Call this from the CPU execution loop to advance the timer
    pub fn tick(&mut self, cycles: u64) {
        if !self.enabled || self.reload_value == 0 {
            return;
        }
        
        let cycles = cycles as u16;
        if self.counter > cycles {
            self.counter -= cycles;
        } else {
            // Timer expired - reload and trigger interrupt
            self.counter = self.reload_value;
            self.interrupt_pending = true;
        }
    }
    
    /// Check and clear interrupt
    pub fn check_interrupt(&mut self) -> bool {
        if self.interrupt_pending {
            self.interrupt_pending = false;
            true
        } else {
            false
        }
    }
}

impl IoDevice for Timer {
    fn read(&mut self, port: u8) -> u8 {
        match port {
            0x30 => (self.counter & 0xFF) as u8,
            0x31 => (self.counter >> 8) as u8,
            0x32 => {
                let mut status = 0u8;
                if self.enabled { status |= 0x01; }
                if self.interrupt_pending { status |= 0x02; }
                status
            }
            _ => 0xFF,
        }
    }
    
    fn write(&mut self, port: u8, value: u8) {
        match port {
            0x30 => {
                // Reload value low byte
                self.reload_value = (self.reload_value & 0xFF00) | (value as u16);
            }
            0x31 => {
                // Reload value high byte - also loads counter
                self.reload_value = (self.reload_value & 0x00FF) | ((value as u16) << 8);
                self.counter = self.reload_value;
            }
            0x32 => {
                // Control register
                self.enabled = (value & 0x01) != 0;
                if value & 0x02 != 0 {
                    self.interrupt_pending = false;  // Acknowledge interrupt
                }
            }
            _ => {}
        }
    }
}
