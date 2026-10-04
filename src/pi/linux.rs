// pi/linux.rs - The Linux side of pi8080d (PI_DAEMON 3.3, 5.1, 8): the BCM2711 GPIO
// block through /dev/gpiomem, the RESET line through the GPIO character device (v2
// line-request ioctl, raw through libc), and the NTP-gated TIME clock. Compiled on Linux
// only; `cargo check --target aarch64-unknown-linux-musl --bins` keeps it honest.

use super::Gpio;
use crate::io::devices::mailbox;
use std::fs::{File, OpenOptions};
use std::mem::size_of;
use std::os::fd::{AsRawFd, FromRawFd};
use std::os::unix::fs::OpenOptionsExt;

// Kernel ABI, include/uapi/linux/gpio.h. Hand-written, so pinned at compile time. Every
// field is part of the layout, read or not (hence the dead_code allows).
const GPIO_MAX_NAME_SIZE: usize = 32;
const GPIO_V2_LINES_MAX: usize = 64;
const GPIO_V2_LINE_NUM_ATTRS_MAX: usize = 10;
const GPIO_V2_LINE_FLAG_INPUT: u64 = 1 << 2;
const GPIO_V2_LINE_FLAG_EDGE_RISING: u64 = 1 << 4;
const GPIO_V2_LINE_FLAG_EDGE_FALLING: u64 = 1 << 5;
const GPIO_V2_GET_LINE_IOCTL: u32 = 0xC250_B407; // _IOWR(0xB4, 0x07, 592)
const GPIO_GET_CHIPINFO_IOCTL: u32 = 0x8044_B401; // _IOR(0xB4, 0x01, 68)

#[repr(C)]
#[allow(dead_code)]
struct GpiochipInfo {
    name: [u8; GPIO_MAX_NAME_SIZE],
    label: [u8; GPIO_MAX_NAME_SIZE],
    lines: u32,
}

#[repr(C)]
#[allow(dead_code)]
#[derive(Clone, Copy)]
struct GpioV2LineAttribute {
    id: u32,
    padding: u32,
    value: u64, // union: flags, values, debounce_period_us
}

#[repr(C)]
#[allow(dead_code)]
#[derive(Clone, Copy)]
struct GpioV2LineConfigAttribute {
    attr: GpioV2LineAttribute,
    mask: u64,
}

#[repr(C)]
#[allow(dead_code)]
struct GpioV2LineConfig {
    flags: u64,
    num_attrs: u32,
    padding: [u32; 5],
    attrs: [GpioV2LineConfigAttribute; GPIO_V2_LINE_NUM_ATTRS_MAX],
}

#[repr(C)]
#[allow(dead_code)]
struct GpioV2LineRequest {
    offsets: [u32; GPIO_V2_LINES_MAX],
    consumer: [u8; GPIO_MAX_NAME_SIZE],
    config: GpioV2LineConfig,
    num_lines: u32,
    event_buffer_size: u32,
    padding: [u32; 5],
    fd: i32,
}

#[repr(C)]
#[allow(dead_code)]
#[derive(Clone, Copy)]
struct GpioV2LineEvent {
    timestamp_ns: u64,
    id: u32,
    offset: u32,
    seqno: u32,
    line_seqno: u32,
    padding: [u32; 6],
}

const _: () = assert!(size_of::<GpioV2LineRequest>() == 592 && size_of::<GpioV2LineEvent>() == 48
                      && size_of::<GpiochipInfo>() == 68);

/// The RESET line on the BCM2711 GPIO controller (ARCHITECTURE 6.4).
const RESET_LINE: u32 = 13;
const CHIP_LABEL: &[u8] = b"pinctrl-bcm2711";

fn monotonic_ns() -> u64 {
    // SAFETY: timespec is plain data; clock_gettime writes only to it.
    let mut ts: libc::timespec = unsafe { std::mem::zeroed() };
    unsafe { libc::clock_gettime(libc::CLOCK_MONOTONIC, &mut ts) };
    ts.tv_sec as u64 * 1_000_000_000 + ts.tv_nsec as u64
}

/// The real `Gpio`: the mmapped register block and the RESET line request.
pub struct GpioMem {
    base: *mut u32,
    /// `/dev/gpiomem`, kept open for its lock (`pi::open_gpio`).
    _lock: File,
    /// The line-event fd, after `request_reset`.
    reset: Option<File>,
    /// CLOCK_MONOTONIC of the previous `reset_edge` call (first: the request).
    last: u64,
}

impl GpioMem {
    /// Maps the GPIO block. Refuses anything but a BCM2711 (a Pi 5's RP1 has a different
    /// register model), and refuses if another pi8080d holds the block's lock.
    pub fn open() -> Result<Self, String> {
        let path = "/proc/device-tree/compatible";
        let compatible = std::fs::read(path).map_err(|e| format!("{}: {}", path, e))?;
        if !compatible.split(|&b| b == 0).any(|s| s == b"brcm,bcm2711") {
            return Err(format!("{}: not a BCM2711 (Pi 4B)", path));
        }
        let path = "/dev/gpiomem";
        let file = super::open_gpio(OpenOptions::new().read(true).write(true).custom_flags(libc::O_SYNC), path)?;
        // SAFETY: a fresh shared mapping of the 4 KiB GPIO block.
        let base = unsafe {
            libc::mmap(std::ptr::null_mut(), 4096, libc::PROT_READ | libc::PROT_WRITE, libc::MAP_SHARED,
                       file.as_raw_fd(), 0)
        };
        if base == libc::MAP_FAILED {
            return Err(format!("{}: mmap: {}", path, std::io::Error::last_os_error()));
        }
        Ok(GpioMem { base: base as *mut u32, _lock: file, reset: None, last: 0 })
    }

    /// Requests RESET (line 13) as an input with both-edge events on the gpiochip labelled
    /// `pinctrl-bcm2711` (PI_DAEMON 5.1). The event fd is non-blocking.
    pub fn request_reset(&mut self) -> Result<(), String> {
        let mut chips: Vec<_> = std::fs::read_dir("/dev").map_err(|e| format!("/dev: {}", e))?
            .filter_map(|e| e.ok().map(|e| e.path()))
            .filter(|p| p.file_name().and_then(|n| n.to_str()).is_some_and(|n| n.starts_with("gpiochip")))
            .collect();
        chips.sort();
        let mut labels = Vec::new();
        for path in chips {
            let chip = match OpenOptions::new().read(true).write(true).open(&path) {
                Ok(chip) => chip,
                Err(e) => {
                    labels.push(format!("{}: {}", path.display(), e));
                    continue;
                }
            };
            // SAFETY: gpiochip_info is plain data, filled by the ioctl.
            let mut info: GpiochipInfo = unsafe { std::mem::zeroed() };
            if unsafe { libc::ioctl(chip.as_raw_fd(), GPIO_GET_CHIPINFO_IOCTL as _, &mut info) } < 0 {
                labels.push(format!("{}: chip info: {}", path.display(), std::io::Error::last_os_error()));
                continue;
            }
            let label = info.label.split(|&b| b == 0).next().unwrap_or(&[]);
            if label != CHIP_LABEL {
                labels.push(format!("{}={}", path.display(), String::from_utf8_lossy(label)));
                continue;
            }
            // SAFETY: the request is plain data; zero means no attributes and default buffer size.
            let mut req: GpioV2LineRequest = unsafe { std::mem::zeroed() };
            req.offsets[0] = RESET_LINE;
            req.consumer[..7].copy_from_slice(b"pi8080d");
            req.config.flags = GPIO_V2_LINE_FLAG_INPUT | GPIO_V2_LINE_FLAG_EDGE_RISING | GPIO_V2_LINE_FLAG_EDGE_FALLING;
            req.num_lines = 1;
            if unsafe { libc::ioctl(chip.as_raw_fd(), GPIO_V2_GET_LINE_IOCTL as _, &mut req) } < 0 {
                return Err(format!("{}: line {} request: {}", path.display(), RESET_LINE, std::io::Error::last_os_error()));
            }
            // SAFETY: the ioctl returned a new fd that we now own.
            let fd = unsafe { File::from_raw_fd(req.fd) };
            let flags = unsafe { libc::fcntl(fd.as_raw_fd(), libc::F_GETFL) };
            if flags < 0 || unsafe { libc::fcntl(fd.as_raw_fd(), libc::F_SETFL, flags | libc::O_NONBLOCK) } < 0 {
                return Err(format!("{}: line {} O_NONBLOCK: {}", path.display(), RESET_LINE, std::io::Error::last_os_error()));
            }
            self.reset = Some(fd);
            self.last = monotonic_ns();
            return Ok(());
        }
        Err(format!("no gpiochip labelled {} (found: {})", String::from_utf8_lossy(CHIP_LABEL), labels.join(", ")))
    }
}

impl Drop for GpioMem {
    fn drop(&mut self) {
        // SAFETY: base is the mapping made in open, unmapped once.
        unsafe { libc::munmap(self.base as *mut libc::c_void, 4096) };
    }
}

impl Gpio for GpioMem {
    fn read(&self, off: usize) -> u32 {
        // SAFETY: off is one of the register offsets in the 4 KiB mapping (PI_DAEMON 3.2).
        unsafe { std::ptr::read_volatile(self.base.add(off / 4)) }
    }

    fn write(&self, off: usize, value: u32) {
        // SAFETY: as read.
        unsafe { std::ptr::write_volatile(self.base.add(off / 4), value) }
    }

    fn reset_edge(&mut self) -> bool {
        let now = monotonic_ns();
        let previous = std::mem::replace(&mut self.last, now);
        let Some(fd) = &self.reset else { return false };
        let mut events = [GpioV2LineEvent { timestamp_ns: 0, id: 0, offset: 0, seqno: 0, line_seqno: 0, padding: [0; 6] }; 16];
        let mut edge = false;
        loop {
            // SAFETY: reads whole events into the array; the fd is non-blocking.
            let n = unsafe { libc::read(fd.as_raw_fd(), events.as_mut_ptr() as *mut libc::c_void, size_of::<[GpioV2LineEvent; 16]>()) };
            if n <= 0 {
                return edge; // EAGAIN: drained
            }
            edge |= events[..n as usize / size_of::<GpioV2LineEvent>()].iter().any(|e| e.timestamp_ns >= previous);
        }
    }
}

/// The Pi's TIME clock (DEVICE_SPECS 8, PI_DAEMON 8): local time, but "not set" unless
/// the kernel reports NTP-synchronized.
pub fn ntp_local_time() -> Option<(u16, u8, u8, u8, u8, u8)> {
    // SAFETY: timex is plain data; modes = 0 only reads the kernel state.
    let mut tx: libc::timex = unsafe { std::mem::zeroed() };
    match unsafe { libc::adjtimex(&mut tx) } {
        -1 | libc::TIME_ERROR => None,
        _ => mailbox::local_time(),
    }
}
