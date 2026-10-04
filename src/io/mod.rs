mod bus;
mod device;
pub mod devices;

pub use bus::IoBus;
pub use device::IoDevice;

use devices::console::Console;
use devices::mailbox::{self, Mailbox};
use devices::storage::Storage;
use std::cell::RefCell;
use std::path::Path;
use std::rc::Rc;

/// The port map (DEVICE_SPECS 1): every device in its power-on state on a fresh bus.
/// Device reset is calling this again. `clock` is the mailbox TIME clock: the emulator
/// passes `mailbox::local_time`, the Pi daemon its NTP-gated one (PI_DAEMON 6, 8).
/// The mailbox gets `storage_dir` too: `GET > FILE` writes there (DEVICE_SPECS 8, GET).
/// Returns the console so the host can feed and drain it.
pub fn build_bus(storage_dir: &Path, clock: mailbox::Clock) -> (IoBus, Rc<RefCell<Console>>) {
    let mut bus = IoBus::new();
    let console = Rc::new(RefCell::new(Console::new()));
    for port in 0x00..=0x02 {
        bus.map_port(port, console.clone());
    }
    let storage = Rc::new(RefCell::new(Storage::new(storage_dir.to_path_buf())));
    for port in 0x08..=0x0F {
        bus.map_port(port, storage.clone());
    }
    let mailbox = Rc::new(RefCell::new(Mailbox::new(clock, storage_dir.to_path_buf())));
    for port in 0x10..=0x13 {
        bus.map_port(port, mailbox.clone());
    }
    (bus, console)
}
