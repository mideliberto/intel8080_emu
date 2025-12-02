/// I/O Device trait - keep it simple!
/// Only 2 methods. If you're tempted to add more, re-read the mantra.
pub trait IoDevice {
    fn read(&mut self, port: u8) -> u8;
    fn write(&mut self, port: u8, value: u8);
}
