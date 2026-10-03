// mailbox.rs - Service Mailbox (ports 10-13), DEVICE_SPECS 8.
//
// The 8080 writes a text command, executes it, polls status and pops the response.
// TIME, ASM and DIS complete within the execute access, so there is no background
// worker and the device never reports BUSY (01). A command that takes time (Phase 8)
// brings the worker, BUSY and the abort of a running request.

use crate::disasm;
use crate::io::IoDevice;
use std::collections::VecDeque;

const IDLE: u8 = 0x00;
const AVAIL: u8 = 0x02;
const DONE: u8 = 0x03;
const ERR_UNKNOWN: u8 = 0x80;
const ERR_OVERFLOW: u8 = 0x81;
const ERR_ARGS: u8 = 0x82;
const ERR_SERVICE: u8 = 0x83;

/// The command buffer holds at most this many bytes; exactly this many is accepted.
const COMMAND_MAX: usize = 128;

/// Local time as (year, month, day, hour, minute, second), or None when the clock is
/// not set (83). Fields must be in range (year 0-9999, month 1-12, ...); the device
/// does not check them. A plain fn, so a test can pass a fixed or a failing clock. The device
/// formats it, so the 19-byte response is the same whatever supplies the time.
pub type Clock = fn() -> Option<(u16, u8, u8, u8, u8, u8)>;

pub struct Mailbox {
    command: Vec<u8>,
    overflow: bool,
    status: u8,
    response: VecDeque<u8>,
    clock: Clock,
}

impl Mailbox {
    /// Power-on state: IDLE, command buffer and response empty, overflow flag clear.
    pub fn new(clock: Clock) -> Self {
        Mailbox { command: Vec::new(), overflow: false, status: IDLE, response: VecDeque::new(), clock }
    }

    /// Takes the buffer as the new request; the old response is discarded.
    fn execute(&mut self) {
        let command = std::mem::take(&mut self.command);
        let result = if std::mem::take(&mut self.overflow) { Err(ERR_OVERFLOW) } else { self.run(&command) };
        match result {
            Ok(bytes) => {
                self.status = if bytes.is_empty() { DONE } else { AVAIL };
                self.response = bytes.into();
            }
            Err(code) => {
                self.status = code;
                self.response.clear();
            }
        }
    }

    /// The command word is the bytes before the first 20h, matched exactly; the argument
    /// string is everything after it (None when there is no 20h).
    fn run(&self, command: &[u8]) -> Result<Vec<u8>, u8> {
        let (word, args) = match command.iter().position(|&b| b == b' ') {
            Some(i) => (&command[..i], Some(&command[i + 1..])),
            None => (command, None),
        };
        match (word, args) {
            (b"TIME", None) => (self.clock)()
                .map(|(y, mo, d, h, mi, s)| format!("{:04}-{:02}-{:02} {:02}:{:02}:{:02}", y, mo, d, h, mi, s).into_bytes())
                .ok_or(ERR_SERVICE),
            (b"ASM", Some(args)) => std::str::from_utf8(args).ok().and_then(disasm::assemble).ok_or(ERR_ARGS),
            (b"DIS", Some(args)) => dis(args).ok_or(ERR_ARGS),
            (b"TIME" | b"ASM" | b"DIS", _) => Err(ERR_ARGS),
            _ => Err(ERR_UNKNOWN),
        }
    }
}

impl IoDevice for Mailbox {
    fn read(&mut self, port: u8) -> u8 {
        match port {
            0x12 => self.status,
            0x13 if self.status == AVAIL => {
                let byte = self.response.pop_front().unwrap_or(0x00);
                if self.response.is_empty() {
                    self.status = DONE;
                }
                byte
            }
            0x13 => 0x00,
            _ => 0xFF,
        }
    }

    fn write(&mut self, port: u8, value: u8) {
        match (port, value) {
            (0x10, _) if self.command.len() < COMMAND_MAX => self.command.push(value),
            (0x10, _) => self.overflow = true,
            (0x11, 0x01) => self.execute(),
            (0x11, 0x02) => *self = Mailbox::new(self.clock),
            _ => {}
        }
    }
}

/// DIS: `AAAA B0 B1 B2` (exactly 13 bytes) -> the length byte, the DIS line, CR LF.
fn dis(args: &[u8]) -> Option<Vec<u8>> {
    if args.len() != 13 || [4, 7, 10].iter().any(|&i| args[i] != b' ') {
        return None;
    }
    // from_str_radix alone would take a leading '+'.
    let hex = |r: std::ops::Range<usize>| -> Option<u16> {
        let digits = std::str::from_utf8(&args[r]).ok()?;
        digits.bytes().all(|b| b.is_ascii_hexdigit()).then(|| u16::from_str_radix(digits, 16).unwrap())
    };
    let bytes = [hex(5..7)? as u8, hex(8..10)? as u8, hex(11..13)? as u8];
    let (line, len) = disasm::line(hex(0..4)?, bytes, |_| None);
    Some([&[len as u8][..], line.as_bytes(), b"\r\n"].concat())
}

/// The host's local time (the emulator uses the host clock, DEVICE_SPECS 8). None if
/// the time is before 1970, doesn't fit time_t (32-bit time_t after 2038) or
/// localtime_r fails; the host has no "clock not set".
pub fn local_time() -> Option<(u16, u8, u8, u8, u8, u8)> {
    let secs = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).ok()?.as_secs();
    // time_t by inference: libc deprecates naming it on musl (the Pi daemon's target).
    let secs = secs.try_into().ok()?;
    // SAFETY: tm is plain data; localtime_r writes only to it and returns null on failure.
    let mut tm: libc::tm = unsafe { std::mem::zeroed() };
    if unsafe { libc::localtime_r(&secs, &mut tm) }.is_null() {
        return None;
    }
    let year = u16::try_from(tm.tm_year + 1900).ok().filter(|&y| y <= 9999)?; // > 9999 is "not set" (DEVICE_SPECS 8)
    Some((year, tm.tm_mon as u8 + 1, tm.tm_mday as u8, tm.tm_hour as u8, tm.tm_min as u8, tm.tm_sec as u8))
}
