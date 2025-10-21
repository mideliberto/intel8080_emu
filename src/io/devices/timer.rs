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
    
    pub fn tick(&mut self, cycles: u8) {
        if !self.enabled {
            return;
        }
        
        if self.counter > cycles as u16 {
            self.counter -= cycles as u16;
        } else {
            // Timer expired - reload and trigger interrupt
            self.counter = self.reload_value;
            self.interrupt_pending = true;
        }
    }
    
    pub fn read_port(&self, port: u8) -> u8 {
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
    
    pub fn write_port(&mut self, port: u8, value: u8) {
        match port {
            0x30 => {  // reload value low
                self.reload_value = (self.reload_value & 0xFF00) | (value as u16);
            }
            0x31 => {  // reload value high - writing this loads counter
                self.reload_value = (self.reload_value & 0x00FF) | ((value as u16) << 8);
                self.counter = self.reload_value;
            }
            0x32 => {  // control register
                self.enabled = (value & 0x01) != 0;
                if value & 0x02 != 0 {
                    self.interrupt_pending = false;  // acknowledge interrupt
                }
            }
            _ => {}
        }
    }
}
