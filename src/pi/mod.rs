// pi/mod.rs - pi8080d, the Pi daemon (docs/PI_DAEMON.md): the emulator's port map behind
// GPIO instead of behind the CPU model. Portable: the bus loop, RESET handling, the TCP
// console and the trace run in `cargo test` on any OS, and under `pi8080d --sim`, against
// the simulated board (sim.rs). Only the register mapping, the RESET line and adjtimex
// are Linux code (linux.rs).

use crate::debugger::Trace;
use crate::io::devices::console::Console;
use crate::io::devices::mailbox;
use crate::io::{build_bus, IoBus};
use std::cell::RefCell;
use std::fs::File;
use std::io::{ErrorKind, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::Path;
use std::rc::Rc;
use std::sync::atomic::{AtomicBool, Ordering::Relaxed};
use std::time::{Duration, Instant};

#[cfg(target_os = "linux")]
pub mod linux;
pub mod sim;

/// The Pi's view of the board: the BCM2711 GPIO register block and the RESET edge latch.
pub trait Gpio {
    /// 32-bit read of the register at byte offset `off` (volatile on hardware).
    fn read(&self, off: usize) -> u32;
    /// 32-bit write of the register at byte offset `off` (volatile on hardware).
    fn write(&self, off: usize, value: u32);
    /// True if RESET changed level at or after the previous call (the first call: since
    /// the line was requested). Edges from before the previous call are drained and ignored.
    fn reset_edge(&mut self) -> bool;
}

// BCM2711 GPIO registers, byte offsets (PI_DAEMON 3.2). The daemon writes no others.
pub const GPFSEL0: usize = 0x00;
pub const GPFSEL1: usize = 0x04;
pub const GPFSEL2: usize = 0x08;
pub const GPSET0: usize = 0x1C;
pub const GPCLR0: usize = 0x28;
pub const GPLEV0: usize = 0x34;
pub const GPIO_PUP_PDN_CNTRL_REG1: usize = 0xE8;

// Signals in GPLEV0, GPSET0 and GPCLR0: bit n is BCM n (ARCHITECTURE 6.4).
/// A0-A6 are BCM 4-10: `port = (lev >> A_SHIFT) & 7F`.
pub const A_SHIFT: u32 = 4;
/// 1 = OUT (/I/OR high), 0 = IN.
pub const DIR: u32 = 1 << 11;
pub const REQ: u32 = 1 << 12;
pub const RESET: u32 = 1 << 13;
pub const ACK: u32 = 1 << 16;
pub const LATCH: u32 = 1 << 17;
/// D0-D7 are BCM 20-27: `(lev >> D_SHIFT) & FF`.
pub const D_SHIFT: u32 = 20;
/// The 20 pins of the ARCHITECTURE 6.4 map: BCM 4-13, 16, 17, 20-27.
pub const PINS: u32 = 0x0FF3_3FF0;

/// GPFSEL2 with D0-D7 (BCM 20-27) as inputs and as outputs; BCM 28-29 as found at startup.
#[derive(Clone, Copy, Debug)]
pub struct Fsel2 {
    pub input: u32,
    pub output: u32,
}

/// Blanking after ACK reads back high (ARCHITECTURE 6.4 rule 2).
const BLANK: Duration = Duration::from_nanos(500);
/// The console pass interval, the RESET edge-latch gate (PI_DAEMON 5.1) and the D read-back report delay.
const MS: Duration = Duration::from_millis(1);
/// Console input read per pass, and output sent per pass (PI_DAEMON 7.2).
const INPUT_CHUNK: usize = 4096;
const OUTPUT_CHUNK: usize = 16 * 1024;

/// Pin setup (PI_DAEMON 3.3): the first thing the daemon does to the board. Releases
/// D0-D7, drives ACK and LATCH low, and refuses a board it can't serve.
pub fn setup_pins<G: Gpio>(gpio: &G) -> Result<Fsel2, String> {
    let fsel = [gpio.read(GPFSEL0), gpio.read(GPFSEL1), gpio.read(GPFSEL2)];
    for pin in (4..=13).chain(16..=17).chain(20..=27) {
        if fsel[pin / 10] >> (pin % 10 * 3) & 7 > 1 {
            return Err(format!("BCM {} is in an ALT function", pin));
        }
    }
    let input = fsel[2] & !0x00FF_FFFF;
    let fsel2 = Fsel2 { input, output: input | 0x0024_9249 };
    gpio.write(GPFSEL2, fsel2.input);
    gpio.write(GPCLR0, ACK | LATCH);
    let mut pulls = gpio.read(GPIO_PUP_PDN_CNTRL_REG1);
    for pin in [16, 17, 20, 21, 22, 23, 24, 25, 26, 27] {
        let shift = (pin - 16) * 2;
        pulls = pulls & !(3 << shift) | 2 << shift; // 10 = pull-down
    }
    gpio.write(GPIO_PUP_PDN_CNTRL_REG1, pulls);
    // GPFSEL1: BCM 10-13 input (fields 0-3), 16 and 17 output (fields 6-7: 000 or 001
    // after step 1, so OR sets them).
    gpio.write(GPFSEL1, fsel[1] & !0o7777 | 0o11 << 18);
    // GPFSEL0: BCM 4-9 input (fields 4-9).
    gpio.write(GPFSEL0, fsel[0] & !(0o777777 << 12));
    let lev = gpio.read(GPLEV0);
    if lev & ACK != 0 {
        return Err("BCM 16 (ACK) reads high after drive low".to_string());
    }
    if lev & LATCH != 0 {
        return Err("BCM 17 (LATCH) reads high after drive low".to_string());
    }
    Ok(fsel2)
}

/// Serves the board until `stop` is set (PI_DAEMON 4): the bus loop, RESET handling, the
/// console pass and the trace, all on the calling thread, which owns the IoBus. Returns
/// Ok(()) on stop, Err only for an error that ends service.
pub fn serve<G: Gpio>(gpio: G, fsel2: Fsel2, storage: &Path, clock: mailbox::Clock,
                      listener: TcpListener, trace: Option<File>, stop: &AtomicBool)
                      -> Result<(), String> {
    listener.set_nonblocking(true).map_err(|e| format!("console listener: {}", e))?;
    let (bus, console) = build_bus(storage, clock);
    let now = Instant::now();
    let mut s = Service {
        gpio, fsel2, storage, clock, bus, console, listener,
        client: None,
        pending: Vec::new(),
        sent: 0,
        trace: trace.map(Trace::new),
        stop,
        last_pass: now,
        last_edge: now,
    };
    while s.access().is_ok() {}
    // Stop (4.1): an access not yet ACKed stays un-ACKed; dropping the bus flushes storage.
    s.gpio.write(GPFSEL2, s.fsel2.input);
    s.gpio.write(GPCLR0, LATCH | ACK);
    if let Some(trace) = &mut s.trace {
        trace.write_pending();
    }
    Ok(())
}

/// `stop` was set: unwind to `serve`.
struct Stopped;

/// The console client (PI_DAEMON 7.1). `eof`: it half-closed, so it is no longer read.
struct Client {
    stream: TcpStream,
    eof: bool,
}

struct Service<'a, G: Gpio> {
    gpio: G,
    fsel2: Fsel2,
    storage: &'a Path,
    clock: mailbox::Clock,
    bus: IoBus,
    console: Rc<RefCell<Console>>,
    listener: TcpListener,
    client: Option<Client>,
    /// Console output taken from the console and not yet sent: `pending[sent..]`.
    pending: Vec<u8>,
    sent: usize,
    trace: Option<Trace>,
    stop: &'a AtomicBool,
    last_pass: Instant,
    /// The last `reset_edge()` call.
    last_edge: Instant,
}

/// True when a read between sampling a request and ACK shows the access is gone: RESET
/// high, or REQ low, which only RESET can cause before ACK (4, abort rule).
fn aborted(s: u32) -> bool {
    s & RESET != 0 || s & REQ == 0
}

impl<G: Gpio> Service<'_, G> {
    fn check_stop(&self) -> Result<(), Stopped> {
        if self.stop.load(Relaxed) {
            Err(Stopped)
        } else {
            Ok(())
        }
    }

    fn reset_edge(&mut self) -> bool {
        self.last_edge = Instant::now();
        self.gpio.reset_edge()
    }

    /// One pass of the loop body, steps 1-8 (PI_DAEMON 4).
    fn access(&mut self) -> Result<(), Stopped> {
        self.check_stop()?;
        let s = self.gpio.read(GPLEV0);
        if s & RESET != 0 {
            return self.reset();
        }
        if s & REQ == 0 {
            if self.last_pass.elapsed() >= MS {
                self.last_pass = Instant::now();
                if self.console_pass() {
                    return self.reset();
                }
            }
            return Ok(());
        }
        let port = (s >> A_SHIFT & 0x7F) as u8;
        let out = s & DIR != 0;
        let v = if out {
            let v = (s >> D_SHIFT) as u8;
            self.bus.write(port, v);
            v
        } else {
            let v = self.bus.read(port);
            if !self.drive(v)? {
                return self.reset();
            }
            v
        };
        // Step 5: the RESET check before ACK, the edge latch behind the 1 ms gate (5.1).
        let s = self.gpio.read(GPLEV0);
        if aborted(s) || (self.last_edge.elapsed() > MS && self.reset_edge()) {
            return self.reset();
        }
        self.gpio.write(GPSET0, ACK);
        while self.gpio.read(GPLEV0) & ACK == 0 {
            self.check_stop()?;
        }
        let t = Instant::now();
        if let Some(trace) = &mut self.trace {
            trace.add(format!("{} {:02X} {:02X}", if out { "OUT" } else { "IN" }, port, v));
        }
        while t.elapsed() < BLANK {}
        self.gpio.write(GPCLR0, ACK);
        while self.gpio.read(GPLEV0) & ACK != 0 {
            self.check_stop()?;
        }
        Ok(())
    }

    /// IN step 4: drive `v` into the IN latch and release D0-D7 (ARCHITECTURE 6.4, IN
    /// cycle 1-4). False if the access was aborted; D0-D7 and LATCH are then left to `reset`.
    fn drive(&mut self, v: u8) -> Result<bool, Stopped> {
        if aborted(self.gpio.read(GPLEV0)) {
            return Ok(false);
        }
        // Values before direction, so no wrong byte is ever driven.
        self.gpio.write(GPCLR0, (!v as u32 & 0xFF) << D_SHIFT);
        self.gpio.write(GPSET0, (v as u32) << D_SHIFT);
        self.gpio.write(GPFSEL2, self.fsel2.output);
        let start = Instant::now();
        let mut reported = false;
        loop {
            let s = self.gpio.read(GPLEV0);
            if aborted(s) {
                return Ok(false);
            }
            if (s >> D_SHIFT) as u8 == v {
                break;
            }
            if !reported && start.elapsed() > MS {
                eprintln!("pi8080d: D read-back mismatch: drove {:02X}, read {:02X}", v, (s >> D_SHIFT) as u8);
                reported = true;
            }
            self.check_stop()?;
        }
        if aborted(self.gpio.read(GPLEV0)) {
            return Ok(false);
        }
        self.gpio.write(GPSET0, LATCH);
        loop {
            let s = self.gpio.read(GPLEV0);
            if aborted(s) {
                return Ok(false);
            }
            if s & LATCH != 0 {
                break;
            }
            self.check_stop()?;
        }
        self.gpio.write(GPCLR0, LATCH);
        self.gpio.write(GPFSEL2, self.fsel2.input);
        Ok(true)
    }

    /// RESET handling (PI_DAEMON 5.2): drop the access in flight unACKed, wait for the
    /// release, then rebuild every device.
    fn reset(&mut self) -> Result<(), Stopped> {
        self.gpio.write(GPFSEL2, self.fsel2.input);
        self.gpio.write(GPCLR0, LATCH | ACK);
        if let Some(trace) = &mut self.trace {
            trace.add("RESET".to_string());
        }
        eprintln!("pi8080d: RESET");
        while self.gpio.read(GPLEV0) & RESET != 0 {
            self.check_stop()?;
        }
        // This reset's edges, a late release edge included, are now from before the previous call.
        self.reset_edge();
        self.bus = IoBus::new(); // drops the old devices first: Storage's Drop flushes and closes
        (self.bus, self.console) = build_bus(self.storage, self.clock);
        self.pending.clear();
        self.sent = 0;
        // DEVICE_SPECS 4: bytes still buffered in the transport are discarded.
        let mut buf = [0u8; INPUT_CHUNK];
        while let Some(client) = self.client.as_mut().filter(|c| !c.eof) {
            match client.stream.read(&mut buf) {
                Ok(0) => {
                    eprintln!("pi8080d: console input EOF");
                    client.eof = true;
                }
                Ok(_) => {}
                Err(e) if matches!(e.kind(), ErrorKind::WouldBlock | ErrorKind::Interrupted) => break,
                Err(e) => self.drop_client(&e),
            }
        }
        Ok(())
    }

    fn drop_client(&mut self, e: &std::io::Error) {
        eprintln!("pi8080d: console client dropped: {}", e);
        self.client = None;
        self.pending.clear();
        self.sent = 0;
    }

    /// The console pass (PI_DAEMON 7.3), run while REQ is low. True if the edge latch
    /// shows a RESET pulse missed while idle.
    fn console_pass(&mut self) -> bool {
        if self.reset_edge() {
            return true;
        }
        loop {
            match self.listener.accept() {
                Ok((stream, peer)) => {
                    if let Err(e) = stream.set_nonblocking(true) {
                        eprintln!("pi8080d: console client {}: {}", peer, e);
                        continue;
                    }
                    let _ = stream.set_nodelay(true);
                    eprintln!("pi8080d: console client {} connected", peer);
                    // The old client, if any, is closed and its unsent output discarded.
                    self.client = Some(Client { stream, eof: false });
                    self.pending.clear();
                    self.sent = 0;
                }
                Err(e) if matches!(e.kind(), ErrorKind::WouldBlock | ErrorKind::Interrupted) => break,
                Err(e) => {
                    eprintln!("pi8080d: console accept: {}", e);
                    break;
                }
            }
        }
        let Some(client) = &mut self.client else {
            self.console.borrow_mut().take_output();
            return false;
        };
        // Input only into an empty FIFO: otherwise it waits in the kernel and TCP stops the sender.
        if !client.eof && !self.console.borrow().has_input() {
            let mut buf = [0u8; INPUT_CHUNK];
            match client.stream.read(&mut buf) {
                Ok(0) => {
                    eprintln!("pi8080d: console input EOF");
                    client.eof = true;
                }
                Ok(n) => self.console.borrow_mut().push_input(&buf[..n]),
                Err(e) if matches!(e.kind(), ErrorKind::WouldBlock | ErrorKind::Interrupted) => {}
                Err(e) => {
                    self.drop_client(&e);
                    return false;
                }
            }
        }
        if self.sent == self.pending.len() {
            self.pending = self.console.borrow_mut().take_output();
            self.sent = 0;
        }
        let end = self.pending.len().min(self.sent + OUTPUT_CHUNK);
        if self.sent < end {
            match client.stream.write(&self.pending[self.sent..end]) {
                Ok(n) => self.sent += n,
                Err(e) if matches!(e.kind(), ErrorKind::WouldBlock | ErrorKind::Interrupted) => {}
                Err(e) => self.drop_client(&e),
            }
        }
        false
    }
}
