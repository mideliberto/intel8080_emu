pub trait IoDevice {
    fn read(&mut self, port: u8) -> u8;
    fn write(&mut self, port: u8, value: u8);
}