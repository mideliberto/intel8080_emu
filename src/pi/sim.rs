// pi/sim.rs - The simulated board (PI_DAEMON 13.1): the ARCHITECTURE 6.4 circuit at the
// logic level, behind the daemon's `Gpio` seam. The daemon thread reads and writes the
// register file; the other thread plays the 8080 with `begin`/`wait` (directly, or through
// `Bridge` from a CPU model) and the RESET line. Every handshake obligation is checked on
// the board side; the first violation is recorded, wakes the 8080 thread and panics the
// daemon thread. Used by the tests and by `pi8080d --sim` (PI_DAEMON 16); the fault
// knobs and test helpers ship with it, unused by `--sim`.

use super::*;
use crate::io::IoDevice;
use std::collections::VecDeque;
use std::sync::{Arc, Condvar, Mutex, MutexGuard};
use std::time::{Duration, Instant};

/// One 8080 access: IN, or OUT with its byte.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Access {
    In,
    Out(u8),
}

/// How an access ended.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Done {
    /// ACKed IN: the byte the IN latch captured.
    In(u8),
    /// ACKed OUT.
    Out,
    /// Cut off by RESET, never ACKed.
    Aborted,
}

/// Board knobs, fixed before the daemon starts.
#[derive(Clone, Debug)]
pub struct Knobs {
    /// GPFSEL0-2 at power-on.
    pub fsel: [u32; 3],
    /// The GPSET0/GPCLR0 output latch at power-on.
    pub latch: u32,
    /// REQ stays high this long after the ACK edge, showing the old access.
    pub req_fall: Duration,
    /// The next queued access sets Q no sooner than this after the previous ACK edge.
    pub gap: Duration,
    /// Delivery delay of a RESET falling-edge event (not of its stamp).
    pub release_event_delay: Duration,
    /// ACK reads high whatever is driven.
    pub ack_stuck_high: bool,
    /// `wait` and `wait_paused` panic after this long; None waits forever (`--sim`: READY
    /// has no timeout, ARCHITECTURE 6.4 rule 4).
    pub wait_timeout: Option<Duration>,
}

/// Pins outside the 20 in GPFSEL0-2 hold ALT functions, as on a running Pi (I2C on
/// BCM 0-3, the UART on 14-15), so a write that changes them is caught. Every output latch
/// bit starts high, so turning ACK or LATCH into an output before driving it low is caught.
impl Default for Knobs {
    fn default() -> Self {
        Knobs {
            fsel: [0o4444, 0o44 << 12, 0],
            latch: !0,
            req_fall: Duration::from_nanos(400),
            gap: Duration::from_micros(1),
            release_event_delay: Duration::ZERO,
            ack_stuck_high: false,
            wait_timeout: Some(Duration::from_secs(10)),
        }
    }
}

const SEED: u32 = 0x8080_2026;
const D_MASK: u32 = 0xFF << D_SHIFT;
/// GPFSEL field masks of the pins outside the 20 (BCM 0-3; 14, 15, 18, 19; 28, 29).
const FSEL_OUTSIDE: [u32; 3] = [0o7777 | 0o77 << 30, 0o77 << 12 | 0o77 << 24, 0o77 << 24 | 0o3 << 30];
/// REG1 pull fields of BCM 18, 19 and 28-31.
const PULL_OUTSIDE: u32 = 0xF << 4 | 0xFF << 24;

struct Current {
    port: u8,
    access: Access,
    latched: bool,
}

struct State {
    knobs: Knobs,
    fsel: [u32; 3],
    latch: u32,
    pulls: u32,
    writes: usize,
    // 8080 side
    queue: VecDeque<(u8, Access)>,
    /// The access holding the WAIT flip-flop (Q = 1).
    current: Option<Current>,
    /// The access REQ still shows after its ACK edge, until the instant.
    lag: Option<(Instant, u8, Access)>,
    last_ack_edge: Option<Instant>,
    /// RESET aborted the access in flight and no new one has started.
    aborted: bool,
    done: VecDeque<Done>,
    in_latch: u8,
    reset: bool,
    stuck_d: Option<(u32, bool)>,
    // RESET edge latch: (stamp, delivery) per edge; the previous reset_edge call.
    edges: VecDeque<(Instant, Instant)>,
    last_edge_call: Instant,
    edge_calls: u64,
    // protocol tracking
    d_seen: bool,
    latch_rose_read: Option<bool>,
    ack_rose: Option<(Instant, bool)>,
    /// A GPLEV0 read has shown ACK low since it last fell. The read that ends the step 8
    /// read-back can already show the next request; the daemon samples only after it.
    ack_low_read: bool,
    violation: Option<String>,
    // pause hook
    pause_armed: bool,
    paused: bool,
    rng: u32,
}

/// The board, shared by the daemon thread (through `Gpio`) and the test thread.
#[derive(Clone)]
pub struct SimBoard(Arc<(Mutex<State>, Condvar)>);

impl SimBoard {
    pub fn new(knobs: Knobs) -> Self {
        let state = State {
            fsel: knobs.fsel,
            latch: knobs.latch,
            pulls: 0x5555_5555,
            writes: 0,
            queue: VecDeque::new(),
            current: None,
            lag: None,
            last_ack_edge: None,
            aborted: false,
            done: VecDeque::new(),
            in_latch: 0,
            reset: false,
            stuck_d: None,
            edges: VecDeque::new(),
            last_edge_call: Instant::now(),
            edge_calls: 0,
            d_seen: false,
            latch_rose_read: None,
            ack_rose: None,
            ack_low_read: true,
            violation: None,
            pause_armed: false,
            paused: false,
            rng: SEED,
            knobs,
        };
        SimBoard(Arc::new((Mutex::new(state), Condvar::new())))
    }

    fn lock(&self) -> MutexGuard<'_, State> {
        self.0 .0.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// Waits on the board until `ready` holds, a violation is recorded or `wait_timeout` passes.
    fn wait_for<T>(&self, what: &str, mut ready: impl FnMut(&mut State) -> Option<T>) -> T {
        let mut st = self.lock();
        let deadline = st.knobs.wait_timeout.map(|t| Instant::now() + t);
        loop {
            if let Some(v) = &st.violation {
                panic!("board violation: {} (seed {:08X})", v, SEED);
            }
            if let Some(t) = ready(&mut st) {
                return t;
            }
            let now = Instant::now();
            if deadline.is_some_and(|d| now >= d) {
                panic!("{}: timed out after {:?}; board: {} (seed {:08X})", what, st.knobs.wait_timeout.unwrap(),
                       st.describe(), SEED);
            }
            st.advance(now);
            // Short waits: a queued access starts on time even if the daemon is parked.
            let nap = deadline.map_or(MS, |d| (d - now).min(MS));
            st = self.0 .1.wait_timeout(st, nap).unwrap().0;
        }
    }

    /// Queues an access from the 8080 side.
    pub fn begin(&self, port: u8, access: Access) {
        let mut st = self.lock();
        st.queue.push_back((port, access));
        st.advance(Instant::now());
    }

    /// The oldest completion. Panics on a recorded violation or after `wait_timeout`.
    pub fn wait(&self) -> Done {
        self.wait_for("wait", |st| st.done.pop_front())
    }

    /// Sets the RESET level. On: clears Q and aborts the access in flight. Either way an
    /// edge event is queued, stamped now (a falling one delivered `release_event_delay` later).
    pub fn reset(&self, on: bool) {
        let mut st = self.lock();
        if st.reset == on {
            return;
        }
        let now = Instant::now();
        st.reset = on;
        let delay = if on { Duration::ZERO } else { st.knobs.release_event_delay };
        st.edges.push_back((now, now + delay));
        if on {
            st.lag = None;
            if st.current.take().is_some() {
                st.aborted = true;
                st.done.push_back(Done::Aborted);
            }
        } else {
            st.advance(now);
        }
        self.0 .1.notify_all();
    }

    /// Pulse RESET (PI_DAEMON 13.3): on, 2 ms, off.
    pub fn pulse_reset(&self) {
        self.reset(true);
        std::thread::sleep(Duration::from_millis(2));
        self.reset(false);
    }

    /// Forces D bit `bit` (0-7) to `level` at the pins, or releases it.
    pub fn stick_d(&self, stuck: Option<(u32, bool)>) {
        self.lock().stuck_d = stuck;
    }

    /// The daemon's next GPLEV0 read that shows a new request computes its value, then
    /// parks until `resume`.
    pub fn pause_next_request(&self) {
        self.lock().pause_armed = true;
    }

    /// Blocks until the daemon is parked by the pause hook.
    pub fn wait_paused(&self) {
        self.wait_for("wait_paused", |st| st.paused.then_some(()));
    }

    pub fn resume(&self) {
        self.lock().paused = false;
        self.0 .1.notify_all();
    }

    /// `reset_edge()` calls so far.
    pub fn edge_calls(&self) -> u64 {
        self.lock().edge_calls
    }

    /// Blocks until `edge_calls()` reaches `n`.
    pub fn wait_edge_calls(&self, n: u64) {
        self.wait_for("wait_edge_calls", |st| (st.edge_calls >= n).then_some(()));
    }

    /// Completions not yet taken by `wait`. Reads the count without waiting on the board.
    pub fn completed(&self) -> usize {
        self.lock().done.len()
    }

    /// Register writes so far.
    pub fn writes(&self) -> usize {
        self.lock().writes
    }

    /// A register as the daemon would read it, without the side effects of a daemon read.
    pub fn peek(&self, off: usize) -> u32 {
        let mut st = self.lock();
        st.register(off)
    }

    /// Fails the test with the recorded violation, if any.
    pub fn check(&self) {
        if let Some(v) = &self.lock().violation {
            panic!("board violation: {} (seed {:08X})", v, SEED);
        }
    }

    /// Records a violation, wakes the test thread and panics the daemon thread.
    fn violate(&self, mut st: MutexGuard<'_, State>, what: String) -> ! {
        let msg = format!("{}; board: {} (seed {:08X})", what, st.describe(), SEED);
        st.violation.get_or_insert(msg.clone());
        drop(st);
        self.0 .1.notify_all();
        panic!("board violation: {}", msg);
    }
}

impl State {
    fn describe(&self) -> String {
        format!("fsel {:08X} {:08X} {:08X}, latch {:08X}, reset {}, current {:?}, queued {}, done {}, aborted {}",
            self.fsel[0], self.fsel[1], self.fsel[2], self.latch, self.reset,
            self.current.as_ref().map(|c| (c.port, c.access)), self.queue.len(), self.done.len(), self.aborted)
    }

    fn output(&self, pin: u32) -> bool {
        self.fsel[pin as usize / 10] >> (pin % 10 * 3) & 7 == 1
    }

    fn d_output(&self) -> bool {
        (20..28).any(|p| self.output(p))
    }

    fn random(&mut self) -> u32 {
        let mut x = self.rng;
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        self.rng = x;
        x
    }

    /// Starts the head of the queue if the WAIT flip-flop can be set now.
    fn advance(&mut self, now: Instant) {
        if self.current.is_some() || self.reset || self.queue.is_empty() {
            return;
        }
        if self.last_ack_edge.is_some_and(|t| now < t + self.knobs.gap) {
            return;
        }
        let (port, access) = self.queue.pop_front().unwrap();
        self.current = Some(Current { port, access, latched: false });
        self.aborted = false;
    }

    /// ACK or LATCH at the pin: its latch bit when an output, else the 4.7k pull-down.
    fn pin(&self, bit: u32) -> bool {
        if bit == ACK && self.knobs.ack_stuck_high {
            return true;
        }
        self.output(bit.trailing_zeros()) && self.latch & bit != 0
    }

    /// GPLEV0 as the pins show it now.
    fn level(&mut self, now: Instant) -> u32 {
        if self.lag.is_some_and(|(until, _, _)| now >= until) {
            self.lag = None;
        }
        let shown = match (&self.current, self.lag) {
            (Some(c), _) => Some((c.port, c.access)),
            (None, Some((_, port, access))) => Some((port, access)),
            (None, None) => None,
        };
        let mut lev = 0;
        let (port, dir, data) = match shown {
            Some((port, Access::Out(v))) => (port as u32, true, v as u32),
            Some((port, Access::In)) => (port as u32, false, 0), // pull-downs while the 245 is off
            None => (self.random() & 0x7F, true, self.random() & 0xFF),
        };
        lev |= port << A_SHIFT;
        if dir {
            lev |= DIR;
        }
        if shown.is_some() {
            lev |= REQ;
        }
        if self.reset {
            lev |= RESET;
        }
        for bit in [ACK, LATCH] {
            if self.pin(bit) {
                lev |= bit;
            }
        }
        let mut d = data << D_SHIFT;
        for pin in 20..28 {
            if self.output(pin) {
                d = d & !(1 << pin) | self.latch & 1 << pin;
            }
        }
        if let Some((bit, level)) = self.stuck_d {
            d = d & !(1 << (D_SHIFT + bit)) | (level as u32) << (D_SHIFT + bit);
        }
        lev | d
    }

    fn register(&mut self, off: usize) -> u32 {
        match off {
            GPFSEL0 => self.fsel[0],
            GPFSEL1 => self.fsel[1],
            GPFSEL2 => self.fsel[2],
            GPIO_PUP_PDN_CNTRL_REG1 => self.pulls,
            GPLEV0 => self.level(Instant::now()),
            _ => 0,
        }
    }
}

impl Gpio for SimBoard {
    fn read(&self, off: usize) -> u32 {
        let mut st = self.lock();
        if st.violation.is_some() {
            drop(st);
            panic!("board violation recorded");
        }
        let now = Instant::now();
        st.advance(now);
        let lev = st.register(off);
        if off != GPLEV0 {
            return lev;
        }
        // Read-back bookkeeping.
        if st.d_output() && (lev ^ st.latch) & D_MASK == 0 {
            st.d_seen = true;
        }
        if lev & LATCH != 0 {
            if let Some(read) = &mut st.latch_rose_read {
                *read = true;
            }
        }
        if lev & ACK != 0 {
            if let Some((_, read)) = &mut st.ack_rose {
                *read = true;
            }
        }
        let sampled_next = st.ack_low_read;
        st.ack_low_read = lev & ACK == 0;
        if st.pause_armed && st.current.is_some() && sampled_next && lev & ACK == 0 {
            st.pause_armed = false;
            st.paused = true;
            self.0 .1.notify_all();
            while st.paused {
                st = self.0 .1.wait(st).unwrap_or_else(|e| e.into_inner());
            }
        }
        drop(st);
        // The daemon polls in a tight loop. While nothing is pending, leave a gap in which
        // the test thread can take the lock (std's Mutex is not fair).
        if lev & REQ == 0 {
            std::thread::yield_now();
        }
        lev
    }

    fn write(&self, off: usize, value: u32) {
        let mut st = self.lock();
        if st.violation.is_some() {
            drop(st);
            panic!("board violation recorded");
        }
        let now = Instant::now();
        st.writes += 1;
        let (ack_was, latch_was) = (st.pin(ACK), st.pin(LATCH));
        match off {
            GPFSEL0 | GPFSEL1 | GPFSEL2 => {
                let i = off / 4;
                if (value ^ st.fsel[i]) & FSEL_OUTSIDE[i] != 0 {
                    self.violate(st, format!("GPFSEL{} write {:08X} changes a pin outside the 20", i, value));
                }
                let was_d_output = st.d_output();
                let old = std::mem::replace(&mut st.fsel[i], value);
                for (bit, name) in [(ACK, "ACK"), (LATCH, "LATCH")] {
                    let pin = bit.trailing_zeros();
                    let shift = pin % 10 * 3;
                    if pin as usize / 10 == i && old >> shift & 7 != 1 && value >> shift & 7 == 1 && st.latch & bit != 0 {
                        self.violate(st, format!("{} newly an output while its latch bit is high", name));
                    }
                }
                if i == 2 {
                    st.d_seen = false;
                    if st.d_output() && !was_d_output {
                        let ok = st.current.as_ref().is_some_and(|c| c.access == Access::In);
                        if !ok {
                            self.violate(st, "D0-D7 set to output while REQ is low or DIR is OUT".to_string());
                        }
                    }
                }
            }
            GPSET0 | GPCLR0 => {
                if value & !PINS != 0 {
                    self.violate(st, format!("GP{}0 write {:08X} touches a pin outside the 20",
                        if off == GPSET0 { "SET" } else { "CLR" }, value));
                }
                if off == GPSET0 {
                    st.latch |= value;
                } else {
                    st.latch &= !value;
                }
                if value & D_MASK != 0 {
                    st.d_seen = false;
                }
            }
            GPIO_PUP_PDN_CNTRL_REG1 => {
                if (value ^ st.pulls) & PULL_OUTSIDE != 0 {
                    self.violate(st, format!("REG1 write {:08X} changes a pin outside the 20", value));
                }
                st.pulls = value;
            }
            _ => self.violate(st, format!("write to register {:02X}", off)),
        }
        // LATCH edges.
        match (latch_was, st.pin(LATCH)) {
            (false, true) => {
                if !st.d_output() || !st.d_seen {
                    self.violate(st, "LATCH rising with D0-D7 not outputs or not read back".to_string());
                }
                let lev = st.level(now);
                st.in_latch = (lev >> D_SHIFT) as u8;
                if let Some(c) = &mut st.current {
                    c.latched = true;
                }
                st.latch_rose_read = Some(false);
            }
            (true, false) => {
                if st.latch_rose_read == Some(false) {
                    self.violate(st, "LATCH falling without a read since it rose".to_string());
                }
                st.latch_rose_read = None;
            }
            _ => {}
        }
        // ACK edges.
        match (ack_was, st.pin(ACK)) {
            (false, true) => {
                if st.d_output() || st.pin(LATCH) {
                    self.violate(st, "ACK rising with D0-D7 outputs or LATCH high".to_string());
                }
                if st.reset {
                    self.violate(st, "ACK rising while RESET is high".to_string());
                }
                let Some(c) = st.current.take() else {
                    let what = if st.aborted { "for an access aborted by RESET" } else { "with no access pending" };
                    self.violate(st, format!("ACK rising {}", what));
                };
                let done = match c.access {
                    Access::In if !c.latched => self.violate(st, "ACK rising on an IN with no LATCH edge".to_string()),
                    Access::In => Done::In(st.in_latch),
                    Access::Out(_) => Done::Out,
                };
                st.done.push_back(done);
                let fall = st.knobs.req_fall;
                st.lag = (!fall.is_zero()).then_some((now + fall, c.port, c.access));
                st.last_ack_edge = Some(now);
                st.ack_rose = Some((now, false));
                st.advance(now);
                self.0 .1.notify_all();
            }
            (true, false) => {
                st.ack_low_read = false;
                if let Some((rose, read)) = st.ack_rose.take() {
                    if !read {
                        self.violate(st, "ACK falling without a read since it rose".to_string());
                    }
                    if now - rose < Duration::from_nanos(500) {
                        self.violate(st, "ACK falling less than 500 ns after it rose".to_string());
                    }
                }
            }
            _ => {}
        }
    }

    fn reset_edge(&mut self) -> bool {
        let mut st = self.lock();
        let now = Instant::now();
        let previous = std::mem::replace(&mut st.last_edge_call, now);
        let mut edge = false;
        while st.edges.front().is_some_and(|&(_, deliver)| deliver <= now) {
            let (stamp, _) = st.edges.pop_front().unwrap();
            edge |= stamp >= previous;
        }
        st.edge_calls += 1;
        self.0 .1.notify_all();
        edge
    }
}

/// The Pi window as the 8080 sees it (PI_DAEMON 13.2, 16.2): an `IoDevice` a CPU model maps
/// on 00-6F. Each access is a REQ/ACK handshake on the board, served by `serve` on another
/// thread.
pub struct Bridge(pub SimBoard);

impl IoDevice for Bridge {
    fn read(&mut self, port: u8) -> u8 {
        self.0.begin(port, Access::In);
        match self.0.wait() {
            Done::In(v) => v,
            d => panic!("IN {:02X}: {:?}", port, d),
        }
    }

    fn write(&mut self, port: u8, value: u8) {
        self.0.begin(port, Access::Out(value));
        assert_eq!(self.0.wait(), Done::Out, "OUT {:02X} {:02X}", port, value);
    }
}
