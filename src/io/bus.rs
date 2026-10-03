use crate::io::IoDevice;
use std::rc::Rc;
use std::cell::RefCell;

pub struct IoBus {
    ports: [Option<Rc<RefCell<dyn IoDevice>>>; 256],
}

impl IoBus {
    pub fn new() -> Self {
        IoBus {
            ports: [(); 256].map(|_| None),
        }
    }
    
    /// Panics on FE and FF: the CPU owns them (overlay control and status) and
    /// would never pass them to a device.
    pub fn map_port(&mut self, port: u8, device: Rc<RefCell<dyn IoDevice>>) {
        assert!(port < 0xFE, "port {:02X} belongs to the CPU", port);
        self.ports[port as usize] = Some(device);
    }
    
    pub fn read(&mut self, port: u8) -> u8 {
        match &self.ports[port as usize] {
            Some(device) => device.borrow_mut().read(port),
            None => 0xFF,
        }
    }
    
    pub fn write(&mut self, port: u8, value: u8) {
        if let Some(device) = &self.ports[port as usize] {
            device.borrow_mut().write(port, value);
        }
    }
}