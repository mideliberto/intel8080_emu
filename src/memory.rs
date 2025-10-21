pub trait Memory {
    fn read(&mut self, addr: u16) -> u8;
    fn write(&mut self, addr: u16, value: u8);
}

pub struct FlatMemory {
    ram: [u8; 65536],
}

impl FlatMemory {
    pub fn new() -> Self {
        Self { ram: [0; 65536] }
    }
}

impl Memory for FlatMemory {
    fn read(&mut self, addr: u16) -> u8 {
        self.ram[addr as usize]
    }
    
    fn write(&mut self, addr: u16, value: u8) {
        self.ram[addr as usize] = value;
    }
}