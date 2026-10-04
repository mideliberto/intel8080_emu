// mailbox_tests.rs - Service Mailbox (ports 10-13) at port level, DEVICE_SPECS 8,
// plus the rules of DEVICE_SPECS 2 that name the mailbox. Written black-box from the
// spec text: each test quotes the sentence it checks.
//
// Every access goes through an IoBus from build_bus (the port map main.rs and the Pi
// daemon use). Tests that need a known time, or an unset clock, replace ports 10-13
// with a mailbox built on an injected clock; that is the only place the device type is
// named.
//
// API used (only `rig_with_clock` names the device):
//   - build_bus(dir, mailbox::local_time) maps one Service Mailbox device at 10, 11, 12, 13, using the host clock.
//   - intel8080_emu::io::devices::mailbox::Mailbox implements IoDevice.
//   - Mailbox::new(clock, storage_dir), clock a plain fn returning Some((year, month, day, hour, minute,
//     second)) of local time, or None for "not set" (TIME gives 83). A plain fn can't
//     capture, so the test clock reads a thread-local (each test runs on its own
//     thread) that `set_clock` changes.
//
// GET (Phase 8, the background command) runs its vectors against the test HTTP server H
// (tests/support/http.rs) through the real `/usr/bin/curl` worker; no test touches the
// internet. The three time-limit rows take 10 s or more and are #[ignore] (run with
// `cargo test -- --ignored`).
//
// Not testable at port level: interrupts (rule 2.7); rule 2.9 (Pi service restart) only
// as "a fresh device reads 00"; the Linux pre_exec (affinity, PDEATHSIG: bench, PI_DAEMON
// 14); a real TLS handshake and DNS failure (they would need the network); curl missing.

use std::cell::{Cell, RefCell};
use std::path::Path;
use std::rc::Rc;
use std::time::{Duration, Instant};

mod support;
use support::http;

use intel8080_emu::io::build_bus;
use intel8080_emu::io::devices::mailbox::{self, Mailbox};
use intel8080_emu::io::IoBus;

type Time = (u16, u8, u8, u8, u8, u8);

thread_local! {
    static NOW: Cell<Option<Time>> = const { Cell::new(None) };
}

/// Set the injected clock: this thread's NOW.
fn set_clock(t: Option<Time>) {
    NOW.with(|n| n.set(t));
}

fn test_clock() -> Option<Time> {
    NOW.with(|n| n.get())
}

const CMD: u8 = 0x10;
const CTL: u8 = 0x11;
const STATUS: u8 = 0x12;
const RESP: u8 = 0x13;

const IDLE: u8 = 0x00;
const BUSY: u8 = 0x01;
const AVAIL: u8 = 0x02;
const DONE: u8 = 0x03;
const E_UNKNOWN: u8 = 0x80;
const E_OVERFLOW: u8 = 0x81;
const E_ARGS: u8 = 0x82;
const E_SERVICE: u8 = 0x83;

const EXECUTE: u8 = 0x01;
const CLEAR: u8 = 0x02;

struct Rig {
    _dir: tempfile::TempDir,
    bus: IoBus,
}

fn rig() -> Rig {
    let dir = tempfile::tempdir().unwrap();
    let (bus, _con) = build_bus(dir.path(), mailbox::local_time);
    Rig { _dir: dir, bus }
}

/// build_bus, then ports 10-13 replaced by a mailbox whose clock reads NOW (set_clock).
fn rig_with_clock() -> Rig {
    let mut r = rig();
    let mb = Rc::new(RefCell::new(Mailbox::new(test_clock, r._dir.path().to_path_buf())));
    for port in 0x10..=0x13 {
        r.bus.map_port(port, mb.clone());
    }
    r
}

const T1: Time = (2026, 10, 2, 14, 30, 5);
const T1_TEXT: &[u8; 19] = b"2026-10-02 14:30:05";
const T2: Time = (2027, 1, 9, 3, 4, 7);
const T2_TEXT: &[u8; 19] = b"2027-01-09 03:04:07";

impl Rig {
    fn inp(&mut self, port: u8) -> u8 {
        self.bus.read(port)
    }
    fn out(&mut self, port: u8, value: u8) {
        self.bus.write(port, value);
    }
    /// IN 12, checked against what TIME, ASM and DIS can ever return.
    /// States: "04-7F | - | Never returned". Transitions: "Commands that complete within
    /// the execute access ... It never reads 01." Error codes: "84-FF | Reserved".
    fn status(&mut self) -> u8 {
        let s = self.inp(STATUS);
        assert!(matches!(s, IDLE | AVAIL | DONE | 0x80..=0x83), "IN 12 = {:02X}", s);
        s
    }
    fn send(&mut self, bytes: &[u8]) {
        for &b in bytes {
            self.out(CMD, b);
        }
    }
    fn clear(&mut self) {
        self.out(CTL, CLEAR);
    }
    /// OUT 11 = 01, then IN 12.
    /// "Commands that complete within the execute access: TIME (Phase 6), ASM and DIS
    /// (Phase 7). Right after execute, IN 12 reads 02 or an error code ... It never reads 01."
    fn execute(&mut self) -> u8 {
        self.out(CTL, EXECUTE);
        let s = self.status();
        assert!(matches!(s, AVAIL | 0x80..=0x83), "right after execute IN 12 = {:02X}", s);
        s
    }
    /// The reference client's resync, send and execute: clear, the bytes, execute.
    fn command(&mut self, bytes: &[u8]) -> u8 {
        self.clear();
        self.send(bytes);
        self.execute()
    }
    /// The reference client's poll loop from AVAIL: IN 13 while IN 12 reads 02.
    /// Returns the bytes and the status that ended the loop.
    fn drain(&mut self) -> (Vec<u8>, u8) {
        let mut got = Vec::new();
        loop {
            let s = self.status();
            if s != AVAIL {
                return (got, s);
            }
            got.push(self.inp(RESP));
            assert!(got.len() <= 1000, "response never ends");
        }
    }
}

/// `NNNN-NN-NN NN:NN:NN`, N a decimal digit (MONITOR_SPEC 6.15, DEVICE_SPECS 8 Commands).
fn is_time_shape(b: &[u8]) -> bool {
    b.len() == 19
        && b.iter().enumerate().all(|(i, &c)| match i {
            4 | 7 => c == b'-',
            10 => c == b' ',
            13 | 16 => c == b':',
            _ => c.is_ascii_digit(),
        })
}

// ---------- Port map and common rules ----------

#[test]
fn ports_10_to_13_are_the_mailbox_and_14_to_6f_stay_unassigned() {
    // Port map: "10 | - | Command byte | Service Mailbox", "11 | - | Control", "12 | Status",
    // "13 | Response byte". Rule 2.3: "Unassigned Pi-window ports (03-07, 14-6F): IN returns FF".
    let mut r = rig();
    assert_eq!(r.inp(STATUS), IDLE, "IN 12 is the mailbox status, not an unassigned FF");
    assert_eq!(r.inp(RESP), 0x00, "IN 13 in IDLE");
    for port in 0x14..=0x6F {
        r.out(port, 0x00);
        assert_eq!(r.inp(port), 0xFF, "port {:02X}", port);
    }
}

#[test]
fn write_only_mailbox_ports_read_ff_with_no_side_effect() {
    // Rule 2.1: "Reading a write-only register (IN 00, 0D, 0E, 10, 11) returns FF and has
    // no side effect." Checked in IDLE, mid-response and DONE.
    set_clock(Some(T1));
    let mut r = rig_with_clock();
    for _ in 0..3 {
        assert_eq!([r.inp(CMD), r.inp(CTL)], [0xFF, 0xFF]);
    }
    assert_eq!(r.status(), IDLE);
    assert_eq!(r.command(b"TIME"), AVAIL);
    assert_eq!(r.inp(RESP), b'2');
    for _ in 0..3 {
        assert_eq!([r.inp(CMD), r.inp(CTL)], [0xFF, 0xFF]);
    }
    let (rest, end) = r.drain();
    assert_eq!((&rest[..], end), (&T1_TEXT[1..], DONE), "IN 10/11 popped or changed the response");
    assert_eq!([r.inp(CMD), r.inp(CTL)], [0xFF, 0xFF]);
    assert_eq!(r.status(), DONE);
}

#[test]
fn writes_to_status_and_response_ports_are_ignored() {
    // Rule 2.2: "Writing a read-only register (OUT 01, 02, 0F, 12, 13) is ignored."
    set_clock(Some(T1));
    let mut r = rig_with_clock();
    for v in [0x00, 0x01, 0x02, 0x03, 0x80, 0xFF] {
        r.out(STATUS, v);
        r.out(RESP, v);
    }
    assert_eq!(r.status(), IDLE);
    // Nor do they reach the command buffer: TIME alone still executes.
    r.send(b"TIME");
    r.out(STATUS, b'X');
    r.out(RESP, b'X');
    assert_eq!(r.execute(), AVAIL);
    assert_eq!(r.inp(RESP), b'2');
    for v in [0x00, 0x01, 0x02, 0x03, 0x80, 0xFF] {
        r.out(STATUS, v);
        r.out(RESP, v);
    }
    assert_eq!(r.drain(), (T1_TEXT[1..].to_vec(), DONE));
}

#[test]
fn in_12_never_has_a_side_effect() {
    // Rule 2.5: "Reads with side effects: only IN 01, IN 0B and IN 13 in the AVAIL state
    // (pops the mailbox response). Every other IN has no side effect and can be repeated."
    set_clock(Some(T1));
    let mut r = rig_with_clock();
    for _ in 0..10 {
        assert_eq!(r.status(), IDLE);
    }
    assert_eq!(r.command(b"TIME"), AVAIL);
    for _ in 0..10 {
        assert_eq!(r.status(), AVAIL);
    }
    assert_eq!(r.drain(), (T1_TEXT.to_vec(), DONE), "repeated IN 12 consumed a byte");
    for _ in 0..10 {
        assert_eq!(r.status(), DONE);
    }
    assert_eq!(r.command(b"time"), E_UNKNOWN);
    for _ in 0..10 {
        assert_eq!(r.status(), E_UNKNOWN);
    }
}

#[test]
fn undefined_control_values_change_nothing() {
    // Registers: "11 | W | Control: 01 = execute, 02 = clear. Other values are ignored".
    // Rule 2.6: "Undefined values written to a command or control register (0C, 0E, 11)
    // change nothing".
    let others: Vec<u8> = (0x00..=0xFF).filter(|v| *v != EXECUTE && *v != CLEAR).collect();
    set_clock(Some(T1));
    let mut r = rig_with_clock();

    // IDLE with a half-sent command: neither executed, cleared nor appended to.
    r.clear();
    r.send(b"TI");
    for &v in &others {
        r.out(CTL, v);
        assert_eq!(r.status(), IDLE, "OUT 11,{:02X} in IDLE", v);
    }
    r.send(b"ME");
    assert_eq!(r.execute(), AVAIL, "the buffer is no longer exactly TIME");

    // AVAIL mid-response: the response is neither restarted nor discarded.
    let first: Vec<u8> = (0..5).map(|_| r.inp(RESP)).collect();
    for &v in &others {
        r.out(CTL, v);
        assert_eq!(r.status(), AVAIL, "OUT 11,{:02X} in AVAIL", v);
    }
    let (rest, end) = r.drain();
    assert_eq!(([&first[..], &rest[..]].concat(), end), (T1_TEXT.to_vec(), DONE));

    // DONE persists.
    for &v in &others {
        r.out(CTL, v);
        assert_eq!(r.status(), DONE, "OUT 11,{:02X} in DONE", v);
    }

    // ERROR persists.
    assert_eq!(r.command(b"NOPE"), E_UNKNOWN);
    for &v in &others {
        r.out(CTL, v);
        assert_eq!(r.status(), E_UNKNOWN, "OUT 11,{:02X} in ERROR", v);
    }

    // The overflow flag survives them too.
    r.clear();
    r.send(&[b'A'; 129]);
    for &v in &others {
        r.out(CTL, v);
    }
    assert_eq!(r.execute(), E_OVERFLOW);
}

// ---------- States ----------

#[test]
fn power_on_is_idle_with_an_empty_buffer() {
    // States: "00 | IDLE | No request. Set at power-on, RESET and after clear".
    // Reading 13: "IDLE, BUSY, DONE, ERROR | 00 | None".
    // Command buffer: "Power-on and RESET: empty, flag clear."
    let mut r = rig();
    assert_eq!(r.status(), IDLE);
    assert_eq!(r.inp(RESP), 0x00);
    assert_eq!(r.status(), IDLE, "IN 13 in IDLE changed the state");
    // An execute with nothing sent sees an empty buffer: an empty command word is 80.
    assert_eq!(r.execute(), E_UNKNOWN, "power-on buffer was not empty, or the flag was set");
}

#[test]
fn response_bytes_then_done() {
    // States: "02 | AVAIL | At least one response byte is ready at port 13",
    // "03 | DONE | The request has finished and every response byte has been read".
    // Transitions: "IN 13 | AVAIL | AVAIL if another byte is ready; otherwise BUSY if the
    // request is still running; otherwise DONE".
    // Reading 13: "AVAIL | The next response byte | Consumed."
    set_clock(Some(T1));
    let mut r = rig_with_clock();
    assert_eq!(r.command(b"TIME"), AVAIL);
    for (i, &want) in T1_TEXT.iter().enumerate() {
        assert_eq!(r.status(), AVAIL, "before byte {}", i);
        assert_eq!(r.inp(RESP), want, "byte {}", i);
    }
    assert_eq!(r.status(), DONE, "after the 19th byte");
}

#[test]
fn done_and_error_persist_and_in_13_reads_00_there() {
    // Transitions: "DONE and ERROR persist until the next execute or clear."
    // Reading 13: "IDLE, BUSY, DONE, ERROR | 00 | None".
    set_clock(Some(T1));
    let mut r = rig_with_clock();
    assert_eq!(r.command(b"TIME"), AVAIL);
    r.drain();
    for _ in 0..5 {
        assert_eq!(r.inp(RESP), 0x00);
        assert_eq!(r.status(), DONE);
    }
    for (cmd, code) in [(&b""[..], E_UNKNOWN), (&[b'A'; 129][..], E_OVERFLOW), (&b"TIME "[..], E_ARGS)] {
        assert_eq!(r.command(cmd), code);
        for _ in 0..5 {
            assert_eq!(r.inp(RESP), 0x00);
            assert_eq!(r.status(), code);
        }
    }
    set_clock(None);
    assert_eq!(r.command(b"TIME"), E_SERVICE);
    for _ in 0..5 {
        assert_eq!(r.inp(RESP), 0x00);
        assert_eq!(r.status(), E_SERVICE);
    }
}

#[test]
fn appending_command_bytes_does_not_change_the_state() {
    // Command buffer: "OUT 10 appends in every state. In AVAIL, DONE or ERROR it changes
    // no status and no response ... the bytes wait in the buffer for the next execute."
    set_clock(Some(T1));
    let mut r = rig_with_clock();
    assert_eq!(r.command(b"TIME"), AVAIL);
    let head: Vec<u8> = (0..3).map(|_| r.inp(RESP)).collect();
    r.send(b"TI");
    assert_eq!(r.status(), AVAIL);
    let (rest, end) = r.drain();
    assert_eq!(([&head[..], &rest[..]].concat(), end), (T1_TEXT.to_vec(), DONE));
    r.send(b"ME");
    assert_eq!(r.status(), DONE);
    set_clock(Some(T2));
    assert_eq!(r.execute(), AVAIL, "the bytes sent during AVAIL and DONE were lost");
    assert_eq!(r.drain(), (T2_TEXT.to_vec(), DONE));
    // In ERROR too.
    assert_eq!(r.command(b"X"), E_UNKNOWN);
    r.send(b"TIME");
    assert_eq!(r.status(), E_UNKNOWN);
    assert_eq!(r.execute(), AVAIL);
}

// ---------- Transitions: execute ----------

#[test]
fn execute_takes_the_buffer_and_empties_it() {
    // Transitions: "OUT 11 = 01 (execute) | any | ... Take the command buffer as the new
    // request and empty the buffer."
    // Command buffer: "Clear and execute both empty the buffer."
    set_clock(Some(T1));
    let mut r = rig_with_clock();
    assert_eq!(r.command(b"TIME"), AVAIL);
    assert_eq!(r.execute(), E_UNKNOWN, "second execute with nothing sent: empty word");
    r.send(b"TIME");
    assert_eq!(r.execute(), AVAIL, "the failed execute left bytes in the buffer");
    assert_eq!(r.drain(), (T1_TEXT.to_vec(), DONE));
}

#[test]
fn execute_from_every_state() {
    // Transitions: "OUT 11 = 01 (execute) | any | Abort any running request (see Abort).
    // ... Go to BUSY, or directly to AVAIL, DONE or ERROR when the result is available
    // within the access".
    set_clock(Some(T1));
    // From IDLE.
    let mut r = rig_with_clock();
    r.send(b"TIME");
    assert_eq!(r.execute(), AVAIL);
    // From AVAIL (mid-response), DONE and ERROR, each to a fresh full response.
    for setup in ["avail", "done", "error"] {
        let mut r = rig_with_clock();
        match setup {
            "avail" => {
                assert_eq!(r.command(b"TIME"), AVAIL);
                r.inp(RESP);
            }
            "done" => {
                assert_eq!(r.command(b"TIME"), AVAIL);
                assert_eq!(r.drain().1, DONE);
            }
            _ => assert_eq!(r.command(b"BAD"), E_UNKNOWN),
        }
        r.send(b"TIME");
        assert_eq!(r.execute(), AVAIL, "from {}", setup);
        assert_eq!(r.drain(), (T1_TEXT.to_vec(), DONE), "from {}", setup);
    }
}

#[test]
fn execute_mid_response_discards_the_old_response() {
    // Transitions: "Abort completes within the OUT 11 access and never waits on the
    // network. The device marks the old request cancelled, and nothing a cancelled
    // request produces is ever delivered."
    set_clock(Some(T1));
    let mut r = rig_with_clock();
    assert_eq!(r.command(b"TIME"), AVAIL);
    assert_eq!((0..7).map(|_| r.inp(RESP)).collect::<Vec<_>>(), &T1_TEXT[..7]);
    // A new TIME: only the new response, from its first byte.
    set_clock(Some(T2));
    r.send(b"TIME");
    assert_eq!(r.execute(), AVAIL);
    assert_eq!(r.drain(), (T2_TEXT.to_vec(), DONE));
    // A failing request: no byte of the old response is left at 13.
    assert_eq!(r.command(b"TIME"), AVAIL);
    r.inp(RESP);
    r.send(b"time");
    assert_eq!(r.execute(), E_UNKNOWN);
    assert_eq!(r.inp(RESP), 0x00);
    assert_eq!(r.status(), E_UNKNOWN);
}

// ---------- Transitions: clear ----------

#[test]
fn clear_from_every_state_goes_idle() {
    // Transitions: "OUT 11 = 02 (clear) | any | IDLE. Abort any running request, discard
    // the response and empty the command buffer."
    // States: "00 | IDLE | ... Set at ... after clear".
    set_clock(Some(T1));
    for setup in ["idle", "avail", "done", "error"] {
        let mut r = rig_with_clock();
        match setup {
            "idle" => {}
            "avail" => {
                assert_eq!(r.command(b"TIME"), AVAIL);
                r.inp(RESP);
            }
            "done" => {
                assert_eq!(r.command(b"TIME"), AVAIL);
                assert_eq!(r.drain().1, DONE);
            }
            _ => assert_eq!(r.command(b"TIME UTC"), E_ARGS),
        }
        r.send(b"XX");
        r.clear();
        assert_eq!(r.status(), IDLE, "clear from {}", setup);
        assert_eq!(r.inp(RESP), 0x00, "clear from {} left a response byte", setup);
        assert_eq!(r.status(), IDLE);
        // The command buffer is empty: XX is gone.
        r.send(b"TIME");
        assert_eq!(r.execute(), AVAIL, "clear from {} kept the command buffer", setup);
        assert_eq!(r.drain(), (T1_TEXT.to_vec(), DONE));
    }
}

#[test]
fn clear_twice_is_still_idle() {
    // Transitions: "OUT 11 = 02 (clear) | any | IDLE."
    let mut r = rig();
    r.clear();
    r.clear();
    assert_eq!(r.status(), IDLE);
    assert_eq!(r.execute(), E_UNKNOWN, "buffer empty after two clears");
}

// ---------- Command buffer ----------

#[test]
fn exactly_128_bytes_are_accepted() {
    // Command buffer: "The buffer holds at most 128 bytes. Exactly 128 bytes is accepted."
    // Accepted means parsed: 128 bytes of an unknown word give 80, and TIME plus 124 bytes
    // of argument give 82, not 81.
    let mut r = rig();
    assert_eq!(r.command(&[b'A'; 128]), E_UNKNOWN);
    let mut time_args = b"TIME ".to_vec();
    time_args.resize(128, b'X');
    assert_eq!(r.command(&time_args), E_ARGS);
    let mut spaces = b"TIME".to_vec();
    spaces.resize(128, b' ');
    assert_eq!(r.command(&spaces), E_ARGS);
}

#[test]
fn byte_129_overflows_and_execute_gives_81() {
    // Command buffer: "Bytes beyond 128 are dropped and an overflow flag is set; the next
    // execute then fails with 81."
    // Error codes: "81 | Command buffer overflowed (more than 128 bytes)".
    set_clock(Some(T1));
    let mut r = rig_with_clock();
    assert_eq!(r.command(&[b'A'; 129]), E_OVERFLOW);
    // 81 wins over every parse result: a TIME word, a valid-looking prefix, anything.
    let mut long_time = b"TIME".to_vec();
    long_time.resize(129, b' ');
    assert_eq!(r.command(&long_time), E_OVERFLOW);
    let mut time_then_junk = b"TIME".to_vec();
    time_then_junk.resize(129, 0x00);
    assert_eq!(r.command(&time_then_junk), E_OVERFLOW);
    assert_eq!(r.command(&vec![b'A'; 100_000]), E_OVERFLOW);
    // No response bytes from a failed request.
    assert_eq!(r.inp(RESP), 0x00);
}

#[test]
fn clear_and_execute_reset_the_overflow_flag() {
    // Command buffer: "Clear and execute both reset the flag."
    set_clock(Some(T1));
    let mut r = rig_with_clock();
    // Clear resets it.
    r.send(&[b'A'; 200]);
    r.clear();
    r.send(b"TIME");
    assert_eq!(r.execute(), AVAIL, "clear left the overflow flag set");
    assert_eq!(r.drain(), (T1_TEXT.to_vec(), DONE));
    // Execute resets it: the 81 execute, then TIME with no clear in between.
    r.send(&[b'A'; 200]);
    assert_eq!(r.execute(), E_OVERFLOW);
    r.send(b"TIME");
    assert_eq!(r.execute(), AVAIL, "execute left the overflow flag set");
    assert_eq!(r.drain(), (T1_TEXT.to_vec(), DONE));
}

#[test]
fn overflow_drops_the_extra_bytes_rather_than_shifting() {
    // Command buffer: "Bytes beyond 128 are dropped". Observable only through the next
    // request after the 81: it must start from an empty buffer (execute empties it), so
    // no dropped byte reappears.
    set_clock(Some(T1));
    let mut r = rig_with_clock();
    let mut bytes = vec![b'A'; 128];
    bytes.extend_from_slice(b"TIME");
    assert_eq!(r.command(&bytes), E_OVERFLOW);
    assert_eq!(r.execute(), E_UNKNOWN, "dropped bytes were kept as the next command");
}

// ---------- Command format ----------

#[test]
fn command_word_is_exact_and_case_sensitive() {
    // Command format: "The command word is the bytes before the first 20h, or the whole
    // buffer when it contains no 20h. It is matched exactly and case-sensitively against
    // the uppercase names below. An empty word, a lowercase word or an unknown word gives 80."
    // "There is no terminator: execute ends the command."
    // Error codes: "80 | Unknown or empty command word".
    let mut r = rig();
    let unknown: [&[u8]; 18] = [
        b"", b" ", b" TIME", b"  TIME", b"time", b"Time", b"tIME", b"TIM", b"TIMES", b"XTIME",
        b"TIME\r", b"TIME\r\n", b"TIME\0", b"\0TIME", b"TIME\t", b"T", b"NOPE", b"\xD4IME",
    ];
    for cmd in unknown {
        assert_eq!(r.command(cmd), E_UNKNOWN, "{:?}", String::from_utf8_lossy(cmd));
        assert_eq!(r.inp(RESP), 0x00);
    }
    assert_eq!(r.command(b"TIME"), AVAIL);
}

#[test]
fn time_with_any_argument_string_is_82() {
    // Commands: "TIME | 6 | TIME only. Any other buffer whose command word is TIME (for
    // example `TIME ` or `TIME UTC`) gives 82".
    // Command format: "The argument string is everything after the first 20h, passed
    // verbatim and possibly empty."
    // Error codes: "82 | Bad or missing arguments for a known command".
    let mut r = rig();
    let with_args: [&[u8]; 7] = [b"TIME ", b"TIME  ", b"TIME UTC", b"TIME utc", b"TIME \0", b"TIME \r\n", b"TIME TIME"];
    for cmd in with_args {
        assert_eq!(r.command(cmd), E_ARGS, "{:?}", String::from_utf8_lossy(cmd));
        assert_eq!(r.inp(RESP), 0x00);
    }
}

#[test]
fn command_bytes_are_any_value() {
    // Command buffer: "Each OUT 10 appends one byte." No byte value is special (unlike
    // OUT 0D, which ignores 00): every value counts toward the 128 and toward the word.
    let mut r = rig();
    let all: Vec<u8> = (0x00..=0x7F).collect();
    assert_eq!(r.command(&all), E_UNKNOWN, "128 bytes 00-7F: word is 00-1F, accepted");
    let mut over = all.clone();
    over.push(0x00);
    assert_eq!(r.command(&over), E_OVERFLOW, "a 129th byte of 00 still overflows");
}

#[test]
fn placeholder_commands_are_unknown() {
    // Command format: "A placeholder word in the Commands table (ASK) is unknown until
    // its phase ships, so it gives 80, with or without arguments."
    let mut r = rig();
    for command in [&b"ASK"[..], b"ASK hi"] {
        assert_eq!(r.command(command), E_UNKNOWN, "{:?}", String::from_utf8_lossy(command));
    }
}

// ---------- TIME ----------

#[test]
fn time_is_19_bytes_with_no_line_ending() {
    // Commands: "TIME ... | 19 bytes, YYYY-MM-DD HH:MM:SS: local time, 24-hour, every
    // field zero-padded, with no line ending. Example: 2026-10-02 14:30:05."
    // Command format: "Text responses use CR LF ... except for TIME".
    set_clock(Some(T1));
    let mut r = rig_with_clock();
    assert_eq!(r.command(b"TIME"), AVAIL);
    assert_eq!(r.drain(), (T1_TEXT.to_vec(), DONE));
}

#[test]
fn time_is_zero_padded_24_hour() {
    // Commands: "24-hour, every field zero-padded".
    let cases: [(Time, &[u8; 19]); 6] = [
        ((2026, 1, 2, 3, 4, 5), b"2026-01-02 03:04:05"),
        ((2026, 12, 31, 23, 59, 59), b"2026-12-31 23:59:59"),
        ((2027, 1, 1, 0, 0, 0), b"2027-01-01 00:00:00"),
        ((2026, 10, 3, 12, 0, 0), b"2026-10-03 12:00:00"),
        ((2026, 10, 3, 13, 0, 0), b"2026-10-03 13:00:00"),
        ((2028, 2, 29, 9, 9, 9), b"2028-02-29 09:09:09"),
    ];
    for (t, text) in cases {
        set_clock(Some(t));
        let mut r = rig_with_clock();
        assert_eq!(r.command(b"TIME"), AVAIL);
        assert_eq!(r.drain(), (text.to_vec(), DONE), "{:?}", t);
    }
}

#[test]
fn time_year_is_zero_padded_to_four_digits() {
    // Commands: "A year below 1000 is zero-padded to four digits (0999-01-02 03:04:05)."
    // TIME clock: "it returns year 0-9999 ... The device does not range-check the fields."
    set_clock(Some((999, 1, 2, 3, 4, 5)));
    let mut r = rig_with_clock();
    assert_eq!(r.command(b"TIME"), AVAIL);
    assert_eq!(r.drain(), (b"0999-01-02 03:04:05".to_vec(), DONE));
    set_clock(Some((9999, 12, 31, 23, 59, 59)));
    assert_eq!(r.command(b"TIME"), AVAIL);
    assert_eq!(r.drain(), (b"9999-12-31 23:59:59".to_vec(), DONE));
}

#[test]
fn time_is_taken_at_execute() {
    // Transitions: "Commands that complete within the execute access: TIME". The response is
    // fixed then; the clock moving while the bytes are read changes nothing.
    set_clock(Some(T1));
    let mut r = rig_with_clock();
    assert_eq!(r.command(b"TIME"), AVAIL);
    let head: Vec<u8> = (0..10).map(|_| r.inp(RESP)).collect();
    set_clock(Some(T2));
    let (rest, end) = r.drain();
    assert_eq!(([&head[..], &rest[..]].concat(), end), (T1_TEXT.to_vec(), DONE));
    set_clock(None);
    assert_eq!(r.status(), DONE);
}

#[test]
fn time_with_the_clock_not_set_is_83() {
    // Commands: "If the clock is not set, the result is 83." TIME clock: "Not set: TIME gives 83."
    // Error codes: "83 | Service failed (no network, API error, clock not set, host error)".
    // States: "80-FF | ERROR | The request failed. The value is the error code".
    set_clock(None);
    let mut r = rig_with_clock();
    assert_eq!(r.command(b"TIME"), E_SERVICE);
    assert_eq!(r.inp(RESP), 0x00);
    assert_eq!(r.status(), E_SERVICE);
    // Parsing comes first: a bad buffer is still 80/82 with no clock.
    assert_eq!(r.command(b"TIME "), E_ARGS);
    assert_eq!(r.command(b"time"), E_UNKNOWN);
    // Once the clock is set, TIME works.
    set_clock(Some(T1));
    assert_eq!(r.command(b"TIME"), AVAIL);
    assert_eq!(r.drain(), (T1_TEXT.to_vec(), DONE));
}

/// Seconds since 0000-03-01 for a civil date and time (no time zone: both sides are local).
fn seconds(b: &[u8]) -> i64 {
    let n = |r: std::ops::Range<usize>| std::str::from_utf8(&b[r]).unwrap().parse::<i64>().unwrap();
    let (y, m, d) = (n(0..4), n(5..7), n(8..10));
    let (y, m) = if m <= 2 { (y - 1, m + 9) } else { (y, m - 3) };
    let days = y * 365 + y / 4 - y / 100 + y / 400 + (153 * m + 2) / 5 + d - 1;
    days * 86400 + n(11..13) * 3600 + n(14..16) * 60 + n(17..19)
}

/// The host's local time now, as the shell's `date` prints it.
fn host_local_now() -> Vec<u8> {
    let out = std::process::Command::new("date").arg("+%Y-%m-%d %H:%M:%S").output().unwrap();
    out.stdout.trim_ascii_end().to_vec()
}

#[test]
fn time_through_build_bus_is_the_host_local_clock() {
    // Commands: "local time". TIME clock: "Emulator: the host's local time (localtime_r)."
    // Transitions: "Commands that complete within the execute access: TIME ... Right after
    // execute, IN 12 reads 02 ... It never reads 01." (checked in execute()).
    let mut r = rig();
    let before = host_local_now();
    assert_eq!(r.command(b"TIME"), AVAIL);
    let after = host_local_now();
    let (got, end) = r.drain();
    assert_eq!(end, DONE);
    assert!(is_time_shape(&got), "{:?}", String::from_utf8_lossy(&got));
    let t = seconds(&got);
    let m = |r: std::ops::Range<usize>| std::str::from_utf8(&got[r]).unwrap().parse::<u32>().unwrap();
    assert!((1..=12).contains(&m(5..7)) && (1..=31).contains(&m(8..10)), "{:?}", String::from_utf8_lossy(&got));
    assert!(m(11..13) <= 23 && m(14..16) <= 59 && m(17..19) <= 60, "{:?}", String::from_utf8_lossy(&got));
    assert!(seconds(&before) <= t && t <= seconds(&after),
        "TIME {:?} is not the host's local time ({:?}..{:?})",
        String::from_utf8_lossy(&got), String::from_utf8_lossy(&before), String::from_utf8_lossy(&after));
}

#[test]
fn reference_client_reads_time() {
    // Reference client: clear (resync), the command bytes, execute, then poll 12:
    // 00 -> ERR, 01 -> poll, 03 -> done, 80+ -> ERR, 02 -> IN 13 and sink.
    // Command buffer: "Resync rule: a client MUST write clear (OUT 11 = 02) before it sends
    // the first command byte. This discards any half-sent command left by an aborted
    // earlier client." Here a stale half command is left, then the client runs.
    let mut r = rig();
    r.send(b"GARB");
    r.clear();
    r.send(b"TIME");
    r.out(CTL, EXECUTE);
    let mut sink = Vec::new();
    let mut polls = 0;
    let end = loop {
        polls += 1;
        assert!(polls < 1000, "poll loop never ended");
        match r.inp(STATUS) {
            0x00 => break Err(0x00),
            0x01 => continue,
            0x02 => sink.push(r.inp(RESP)),
            0x03 => break Ok(()),
            s => break Err(s),
        }
    };
    assert_eq!(end, Ok(()));
    assert!(is_time_shape(&sink), "{:?}", String::from_utf8_lossy(&sink));
    assert!(!sink.contains(&0x0D) && !sink.contains(&0x0A));
}

// ---------- ASM and DIS ----------

impl Rig {
    /// The reference client end to end: the status right after execute, and the response
    /// read to DONE (empty when the status is an error, after checking IN 13 reads 00).
    fn ask(&mut self, command: &[u8]) -> (u8, Vec<u8>) {
        match self.command(command) {
            AVAIL => {
                let (got, end) = self.drain();
                assert_eq!(end, DONE, "{:?}", String::from_utf8_lossy(command));
                (AVAIL, got)
            }
            s => {
                assert_eq!(self.inp(RESP), 0x00, "{:?}", String::from_utf8_lossy(command));
                (s, Vec::new())
            }
        }
    }
}

#[test]
fn asm_conformance_vectors() {
    // ASM and DIS conformance vectors: "Device-level tests MUST cover every row."
    // Transitions: "ASM and DIS can give 80-82 and never 83." ASM: "Response: the machine
    // code, 1-3 bytes, binary ... There is no line ending."
    let ok: [(&str, &[u8]); 19] = [
        ("ASM MVI A,0D", &[0x3E, 0x0D]),
        ("ASM mvi a,0d", &[0x3E, 0x0D]),
        ("ASM   MVI   A , 0D  ", &[0x3E, 0x0D]),
        ("ASM MVI A,D", &[0x3E, 0x0D]),
        ("ASM MVI A,000D", &[0x3E, 0x0D]),
        ("ASM MVI A,00AA", &[0x3E, 0xAA]),
        ("ASM LXI H,1", &[0x21, 0x01, 0x00]),
        ("ASM LXI SP,EFFE", &[0x31, 0xFE, 0xEF]),
        ("ASM CALL 0005", &[0xCD, 0x05, 0x00]),
        ("ASM MOV A,M", &[0x7E]),
        ("ASM HLT", &[0x76]),
        ("ASM IN 02", &[0xDB, 0x02]),
        ("ASM POP PSW", &[0xF1]),
        ("ASM RST 7", &[0xFF]),
        ("ASM xchg", &[0xEB]),
        ("ASM NOP*", &[0x08]),
        ("ASM JMP* 0200", &[0xCB, 0x00, 0x02]),
        ("ASM RET*", &[0xD9]),
        ("ASM call* 1234", &[0xDD, 0x34, 0x12]),
    ];
    let mut r = rig();
    for (cmd, code) in ok {
        assert_eq!(r.ask(cmd.as_bytes()), (AVAIL, code.to_vec()), "{}", cmd);
    }
    let bad = [
        "ASM", "ASM ", "ASM    ",
        "ASM MVI A,100", "ASM MVI A,1AA",
        "ASM LXI H,10000", "ASM LXI H,00001", "ASM MVI A,0000D",
        "ASM MVI A,0DH", "ASM MVI A,0x0D", "ASM MVI A,+D", "ASM MVI A,-1",
        "ASM MOV M,M", "ASM LDAX H", "ASM PUSH SP", "ASM LXI PSW,0",
        "ASM RST 8", "ASM RST 07",
        "ASM JMP", "ASM NOP 00",
        "ASM MVI A,", "ASM MOV A B", "ASM MOV A,,B", "ASM MOV A,B,C",
        "ASM MVI\tA,0D",
        "ASM NOP ;c", "ASM LABEL: NOP", "ASM DB 00",
    ];
    for cmd in bad {
        assert_eq!(r.ask(cmd.as_bytes()), (E_ARGS, Vec::new()), "{:?}", cmd);
    }
    assert_eq!(r.ask(b"asm NOP"), (E_UNKNOWN, Vec::new()));
    // ASM: "Length: the 128-byte buffer leaves 124 bytes for <line>. A longer command gives 81."
    let mut long = b"ASM ".to_vec();
    long.extend([b' '; 125]);
    assert_eq!(r.ask(&long), (E_OVERFLOW, Vec::new()), "129 bytes");
    let mut full = b"ASM NOP".to_vec();
    full.extend([b' '; 121]);
    assert_eq!(r.ask(&full), (AVAIL, vec![0x00]), "128 bytes, trailing spaces ignored");
}

#[test]
fn asm_rejects_every_byte_outside_20_to_7e() {
    // ASM: "Characters: only 20h-7Eh. Any other byte, including Tab, CR, LF, 00 and 80-FF,
    // gives 82."
    let mut r = rig();
    for b in (0x00..0x20).chain(0x7F..=0xFF) {
        for cmd in [[&b"ASM NOP"[..], &[b]].concat(), [&b"ASM "[..], &[b], b"NOP"].concat()] {
            assert_eq!(r.ask(&cmd), (E_ARGS, Vec::new()), "{:02X}", b);
        }
    }
}

#[test]
fn dis_conformance_vectors() {
    // ASM and DIS conformance vectors. DIS: "Response: one length byte L, binary 01, 02 or
    // 03, the instruction's length. Then the instruction line, then CR LF."
    let ok: [(&str, u8, &str); 11] = [
        ("DIS 0100 3E 0D 00", 2, "0100  3E 0D     MVI A,0D"),
        ("DIS 0100 00 FF FF", 1, "0100  00        NOP"),
        ("DIS 0100 c3 00 f0", 3, "0100  C3 00 F0  JMP F000"),
        ("DIS FFFF CD 34 12", 3, "FFFF  CD 34 12  CALL 1234"),
        ("DIS 0200 31 FE EF", 3, "0200  31 FE EF  LXI SP,EFFE"),
        ("DIS 0100 FB 00 00", 1, "0100  FB        EI"),
        ("DIS 0200 DB 02 00", 2, "0200  DB 02     IN 02"),
        ("DIS 0200 FF 00 00", 1, "0200  FF        RST 7"),
        ("DIS 0100 08 FF FF", 1, "0100  08        NOP*"),
        ("DIS 0100 DD 00 01", 3, "0100  DD 00 01  CALL* 0100"),
        ("DIS abcd 00 00 00", 1, "ABCD  00        NOP"),
    ];
    let mut r = rig();
    for (cmd, len, line) in ok {
        let want = [&[len][..], line.as_bytes(), b"\r\n"].concat();
        assert_eq!(r.ask(cmd.as_bytes()), (AVAIL, want), "{}", cmd);
    }
    let bad = [
        "DIS", "DIS ", "DIS 0100 3E 0D", "DIS 100 3E 0D 00", "DIS 0100 3E 0D 00 ", "DIS 0100  3E 0D 00",
        "DIS 0100 3E 0D 0G", "DIS 0100 +E 0D 00", "DIS 0100 3E 0D 00 00", "DIS 01000 3E 0D 0",
        "DIS +100 3E 0D 00", "DIS 0100\t3E 0D 00",
    ];
    for cmd in bad {
        assert_eq!(r.ask(cmd.as_bytes()), (E_ARGS, Vec::new()), "{:?}", cmd);
    }
    assert_eq!(r.ask(b"DIS 0100 3E 0D \xC0\xC0"), (E_ARGS, Vec::new()));
    assert_eq!(r.ask(b"dis 0100 00 00 00"), (E_UNKNOWN, Vec::new()));
    // The line length range: "The line is 18-27 bytes ..., and the response is 21-30 bytes."
    for x in 0..=255u8 {
        let (_, resp) = r.ask(format!("DIS 0000 {:02X} 34 12", x).as_bytes());
        assert!((21..=30).contains(&resp.len()), "{:02X}: {:?}", x, String::from_utf8_lossy(&resp));
    }
}

#[test]
fn dis_then_asm_round_trips_r1_and_r2() {
    // Round-trip properties: "For every x in 00-FF and every operand pair y z in {00 00,
    // FF FF, 34 12}, DIS 0000 x y z responds with a length L and a line; let t be the line
    // from column 16 ... up to the CR LF. Tests MUST check R1 and R2 for all 768 cases."
    // R1: "ASM t responds with L bytes, and DIS 0000 of those bytes (padded with y z)
    // gives t again." R2: "the L bytes ASM t responds with are the first L of x y z, for
    // every opcode except the 8 duplicate aliases. NOP* always assembles to 08 ... and
    // CALL* always assembles to DD".
    let mut r = rig();
    let dis = |r: &mut Rig, b: [u8; 3]| -> (usize, String) {
        let (s, resp) = r.ask(format!("DIS 0000 {:02X} {:02X} {:02X}", b[0], b[1], b[2]).as_bytes());
        assert_eq!(s, AVAIL);
        let line = std::str::from_utf8(&resp[1..]).unwrap().strip_suffix("\r\n").unwrap();
        (resp[0] as usize, line[16..].to_string())
    };
    for x in 0..=255u8 {
        for (y, z) in [(0x00, 0x00), (0xFF, 0xFF), (0x34, 0x12)] {
            let (len, t) = dis(&mut r, [x, y, z]);
            let (s, code) = r.ask(format!("ASM {}", t).as_bytes());
            assert_eq!((s, code.len()), (AVAIL, len), "R1 {:02X} {:02X} {:02X}: {}", x, y, z, t);
            let mut padded = [x, y, z];
            padded[..len].copy_from_slice(&code);
            assert_eq!(dis(&mut r, padded), (len, t.clone()), "R1 {:02X} {:02X} {:02X}", x, y, z);
            let first = match x {
                0x10 | 0x18 | 0x20 | 0x28 | 0x30 | 0x38 => 0x08,
                0xED | 0xFD => 0xDD,
                _ => x,
            };
            assert_eq!(code, [first, y, z][..len], "R2 {:02X} {:02X} {:02X}: {}", x, y, z, t);
        }
    }
}

// ---------- RESET and Pi service restart ----------

#[test]
fn reset_is_rebuilding_the_bus() {
    // Rule 2.8: "Every Pi device returns to its power-on state: ... mailbox IDLE with the
    // command buffer and response cleared and any running request aborted".
    // "Emulator: RESET happens only at process start, when build_bus creates every
    // device in its power-on state. Any future host-side reset MUST also return every
    // device to its power-on state, by calling build_bus again."
    // Rule 2.9: "mailbox: status reads 00 after an execute" (after a Pi service restart,
    // which is the same rebuild).
    let dir = tempfile::tempdir().unwrap();
    for leave in ["avail", "done", "error", "half", "overflow"] {
        let (mut bus, _con) = build_bus(dir.path(), mailbox::local_time);
        for b in b"TIME" {
            bus.write(CMD, *b);
        }
        bus.write(CTL, EXECUTE);
        match leave {
            "avail" => {
                bus.read(RESP);
            }
            "done" => {
                while bus.read(STATUS) == AVAIL {
                    bus.read(RESP);
                }
            }
            "error" => {
                bus.write(CMD, b' ');
                bus.write(CTL, EXECUTE);
            }
            "half" => {
                bus.write(CMD, b'X');
            }
            _ => {
                for _ in 0..200 {
                    bus.write(CMD, b'A');
                }
            }
        }
        // RESET: build_bus again.
        let (bus, _con) = build_bus(dir.path(), mailbox::local_time);
        let mut r = Rig { _dir: tempfile::tempdir().unwrap(), bus };
        assert_eq!(r.status(), IDLE, "after {}", leave);
        assert_eq!(r.inp(RESP), 0x00, "after {}: a response byte survived", leave);
        // Buffer empty and flag clear: TIME with no clear first works.
        r.send(b"TIME");
        assert_eq!(r.execute(), AVAIL, "after {}: buffer or flag survived", leave);
        let (got, end) = r.drain();
        assert!(end == DONE && is_time_shape(&got), "after {}: {:?}", leave, String::from_utf8_lossy(&got));
    }
}

// ---------- GET (DEVICE_SPECS 8: Background commands, GET, GET conformance vectors) ----------
//
// "Final" is the status after polling IN 12 until it is not 01. "Response" is every byte
// read from 13. "A status may read 01 before any IN 12 read. Every polling loop has a 10 s
// Instant deadline and fails with a message naming the row".

impl Rig {
    fn dir(&self) -> &Path {
        self._dir.path()
    }

    /// Clear, the bytes, execute. Returns IN 12 right after execute: "Right after execute,
    /// IN 12 reads 01, 81, 82 or 83" (80 for an unknown word, as for every command).
    fn start(&mut self, bytes: &[u8]) -> u8 {
        self.clear();
        self.send(bytes);
        self.out(CTL, EXECUTE);
        let s = self.inp(STATUS);
        assert!(matches!(s, BUSY | 0x80..=0x83), "{:?}: right after execute IN 12 = {:02X}", String::from_utf8_lossy(bytes), s);
        s
    }

    /// Poll IN 12 until it reads neither 01 nor 02, reading IN 13 at each 02. Returns the
    /// final status and the response. Fails after `secs`.
    fn finish_within(&mut self, row: &str, secs: u64) -> (u8, Vec<u8>) {
        let deadline = Instant::now() + Duration::from_secs(secs);
        let mut got = Vec::new();
        loop {
            match self.inp(STATUS) {
                BUSY => {}
                AVAIL => got.push(self.inp(RESP)),
                s => return (s, got),
            }
            assert!(Instant::now() < deadline, "{}: no end within {} s ({} bytes)", row, secs, got.len());
        }
    }

    fn finish(&mut self, row: &str) -> (u8, Vec<u8>) {
        self.finish_within(row, 10)
    }

    /// The storage directory's file names, sorted.
    fn listing(&self) -> Vec<String> {
        let mut names: Vec<String> = std::fs::read_dir(self.dir()).unwrap().map(|e| e.unwrap().file_name().into_string().unwrap()).collect();
        names.sort();
        names
    }
}

#[test]
fn get_stream_vectors() {
    // Stream form: "The response is the body." Body: "unchanged: no character-set
    // conversion, no line-ending change, any byte 00-FF." Redirects: "followed, at most 5".
    let h = http::start();
    let bin: Vec<u8> = (0..=255).collect();
    let rows: [(String, &[u8]); 5] = [
        (format!("GET http://{}/hello", h.host()), b"Hello\r\n"),
        (format!("GET   http://{}/hello  ", h.host()), b"Hello\r\n"),
        (format!("GET http://{}/lf", h.host()), b"a\nb\n"),
        (format!("GET http://{}/bin", h.host()), &bin),
        (format!("GET http://{}/r5", h.host()), b"ok"),
    ];
    for (command, body) in rows {
        let mut r = rig();
        assert_eq!(r.start(command.as_bytes()), BUSY, "{}", command);
        assert_eq!(r.finish(&command), (DONE, body.to_vec()), "{}", command);
        assert_eq!(r.listing(), Vec::<String>::new(), "{}: storage directory", command);
    }
}

#[test]
fn get_flow_control() {
    // "Flow control. Streamed output waits in the worker's pipe (64 KiB on Linux) until the
    // 8080 reads it." Row: /chunk, "wait 200 ms before the first IN 12 | 200, chunked, 100
    // KiB of a pattern (more than a pipe holds, so curl blocks) | 03 after the same bytes".
    let h = http::start();
    let mut r = rig();
    r.clear();
    r.send(format!("GET {}", h.url("/chunk")).as_bytes());
    r.out(CTL, EXECUTE);
    std::thread::sleep(Duration::from_millis(200));
    assert_eq!(r.finish("/chunk"), (DONE, http::pattern(100 * 1024)));
}

#[test]
fn get_empty_bodies_go_straight_to_done() {
    // "An empty body goes from BUSY straight to DONE." Status: "Any other final status
    // (2xx, or a 3xx that is not followed, such as 304) delivers its body, which may be empty."
    let h = http::start();
    for path in ["/empty", "/304"] {
        let mut r = rig();
        assert_eq!(r.start(format!("GET {}", h.url(path)).as_bytes()), BUSY);
        assert_eq!(r.finish(path), (DONE, vec![]), "{}: no AVAIL ever", path);
    }
}

#[test]
fn get_failures_are_83_with_no_byte() {
    // Errors: "83 for everything after it: ... DNS, connect or TLS failure, ... a status
    // of 400 or above, too many redirects". Status: "400 or above gives 83, and no byte of
    // its body is delivered."
    let h = http::start();
    let closed = closed_port();
    for command in [
        format!("GET {}", h.url("/r6")),
        format!("GET {}", h.url("/404")),
        format!("GET {}", h.url("/500")),
        format!("GET http://127.0.0.1:{}/", closed),
        format!("GET https://{}/hello", h.host()),
    ] {
        let mut r = rig();
        assert_eq!(r.start(command.as_bytes()), BUSY, "{}", command);
        assert_eq!(r.finish(&command), (E_SERVICE, vec![]), "{}", command);
        assert_eq!(r.inp(RESP), 0x00);
        assert_eq!(r.listing(), Vec::<String>::new(), "{}", command);
    }
}

/// A port nothing listens on: bind one, note it, close it.
fn closed_port() -> u16 {
    std::net::TcpListener::bind("127.0.0.1:0").unwrap().local_addr().unwrap().port()
}

#[test]
fn only_in_12_checks_the_worker() {
    // "The device checks the worker when the 8080 reads IN 12 in BUSY, and at no other
    // time." "IN 13 ... in BUSY it reads 00 with no side effect". Row: /hang; IN 13; IN 12 |
    // IN 13 = 00, then IN 12 = 01.
    let h = http::start();
    let mut r = rig();
    assert_eq!(r.start(format!("GET {}", h.url("/hang")).as_bytes()), BUSY);
    assert_eq!(r.inp(RESP), 0x00);
    assert_eq!(r.inp(STATUS), BUSY);
}

#[test]
fn the_last_pop_goes_busy_without_a_check() {
    // Background commands: "When it pops the last byte read so far, the status goes back to
    // BUSY without a check." /chunk is 100 KB; one check reads at most 4096 bytes, so after
    // popping exactly those the next IN 13 is in BUSY and reads 00.
    let h = http::start();
    let mut r = rig();
    assert_eq!(r.start(format!("GET {}", h.url("/chunk")).as_bytes()), BUSY);
    std::thread::sleep(Duration::from_millis(200));
    assert_eq!(r.inp(STATUS), AVAIL);
    for i in 0..4096usize {
        assert_eq!(r.inp(RESP), (i % 251) as u8, "byte {}", i);
    }
    assert_eq!(r.inp(RESP), 0x00, "the last pop checked the worker");
}

#[test]
fn get_to_a_file() {
    // File form: "The response is the body length as 6 uppercase hex digits, with no line
    // ending". Step 3: "renames it over FILE (created if it does not exist)". "An empty body
    // gives an empty FILE." Step 1: "At execute, the device removes any leftover temporary
    // file for FILE". The name is checked as mount does (folded to uppercase).
    let h = http::start();
    let rows = [
        (format!("GET {} > BOOK.TXT", h.url("/hello")), "BOOK.TXT", &b"000007"[..], &b"Hello\r\n"[..]),
        (format!("GET {}   >   book.txt ", h.url("/hello")), "BOOK.TXT", b"000007", b"Hello\r\n"),
        (format!("GET {} > E.BIN", h.url("/empty")), "E.BIN", b"000000", b""),
    ];
    for (command, name, response, body) in rows {
        let mut r = rig();
        assert_eq!(r.start(command.as_bytes()), BUSY);
        assert_eq!(r.finish(&command), (DONE, response.to_vec()), "{}", command);
        assert_eq!(std::fs::read(r.dir().join(name)).unwrap(), body, "{}", command);
        assert_eq!(r.listing(), [name], "{}: no ~{}", command, name);
    }
    // A stale ~E.BIN is removed before the worker starts, so an empty body is never junk.
    // /304 has no body, so curl creates no file; only the removal keeps the junk out.
    let mut r = rig();
    std::fs::write(r.dir().join("~E.BIN"), b"junk").unwrap();
    r.start(format!("GET {} > E.BIN", h.url("/304")).as_bytes());
    assert_eq!(r.finish("stale ~E.BIN"), (DONE, b"000000".to_vec()));
    assert_eq!(std::fs::read(r.dir().join("E.BIN")).unwrap(), b"");
    assert_eq!(r.listing(), ["E.BIN"]);
}

#[test]
fn get_file_size_limit() {
    // Step 2: "A body longer than FFFFFF bytes fails with 83 as soon as the worker sees it,
    // and the temporary file is removed." Rows /max (FFFFFF bytes) and /over (1000000h, chunked).
    let h = http::start();
    let mut r = rig();
    r.start(format!("GET {} > M.BIN", h.url("/max")).as_bytes());
    assert_eq!(r.finish("/max"), (DONE, b"FFFFFF".to_vec()));
    assert!(std::fs::read(r.dir().join("M.BIN")).unwrap() == http::pattern(0xFF_FFFF), "M.BIN is not the body");
    let mut r = rig();
    r.start(format!("GET {} > O.BIN", h.url("/over")).as_bytes());
    assert_eq!(r.finish("/over"), (E_SERVICE, vec![]));
    assert_eq!(r.listing(), Vec::<String>::new());
}

#[test]
fn a_failed_get_leaves_file_untouched() {
    // "On any failure or abort before that, FILE is untouched and the temporary file is removed."
    let h = http::start();
    let mut r = rig();
    std::fs::write(r.dir().join("KEEP.TXT"), b"old").unwrap();
    r.start(format!("GET {} > KEEP.TXT", h.url("/404")).as_bytes());
    assert_eq!(r.finish("/404 > KEEP.TXT"), (E_SERVICE, vec![]));
    assert_eq!(std::fs::read(r.dir().join("KEEP.TXT")).unwrap(), b"old");
    assert_eq!(r.listing(), ["KEEP.TXT"]);
}

#[test]
fn a_mounted_file_keeps_its_old_contents_until_mounted_again() {
    // "A mounted FILE: the mount keeps reading the file it opened, which is the old
    // contents, until FILE is mounted again." "The mailbox does not consult the storage device."
    let h = http::start();
    let mut r = rig();
    std::fs::write(r.dir().join("F.TXT"), b"old").unwrap();
    let mount = |r: &mut Rig| {
        r.out(0x0E, 0x03);
        for &b in b"F.TXT" {
            r.out(0x0D, b);
        }
        r.out(0x0E, 0x01);
        assert_eq!(r.inp(0x0F), 0x00);
    };
    mount(&mut r);
    r.start(format!("GET {} > F.TXT", h.url("/hello")).as_bytes());
    assert_eq!(r.finish("mounted F.TXT"), (DONE, b"000007".to_vec()));
    assert_eq!(r.inp(0x0B), b'o', "the old mount");
    assert_eq!(std::fs::read(r.dir().join("F.TXT")).unwrap(), b"Hello\r\n");
    mount(&mut r);
    assert_eq!(r.inp(0x0B), b'H');
}

#[test]
fn clear_kills_the_worker() {
    // Background commands: "Execute and clear abort a running request (Abort, above): the
    // device kills the worker and reaps it within the OUT 11 access, discards its output and
    // removes its temporary file."
    let h = http::start();
    let mut r = rig();
    r.start(format!("GET {}", h.url("/hang")).as_bytes());
    assert!(h.sees_request());
    assert_eq!(r.inp(STATUS), BUSY);
    r.clear();
    assert_eq!(r.inp(STATUS), IDLE);
    assert!(h.sees_close(), "/hang: the worker was not killed");

    let mut r = rig();
    r.start(format!("GET {} > H.BIN", h.url("/hang")).as_bytes());
    assert!(h.sees_request());
    r.clear();
    assert_eq!(r.inp(STATUS), IDLE);
    assert!(h.sees_close(), "/hang > H.BIN: the worker was not killed");
    assert_eq!(r.listing(), Vec::<String>::new(), "no H.BIN, no ~H.BIN");

    // /drip: read 3 bytes, then IN 12 = 01; after clear 00, and IN 13 = 00.
    let mut r = rig();
    r.start(format!("GET {}", h.url("/drip")).as_bytes());
    assert!(h.sees_request());
    let deadline = Instant::now() + Duration::from_secs(10);
    let mut got = Vec::new();
    while got.len() < 3 {
        match r.inp(STATUS) {
            BUSY => assert!(Instant::now() < deadline, "/drip: no abc within 10 s"),
            AVAIL => got.push(r.inp(RESP)),
            s => panic!("/drip: status {:02X}", s),
        }
    }
    assert_eq!(got, b"abc");
    assert_eq!(r.inp(STATUS), BUSY);
    r.clear();
    assert_eq!((r.inp(STATUS), r.inp(RESP)), (IDLE, 0x00));
    assert!(h.sees_close(), "/drip: the worker was not killed");
}

#[test]
fn execute_kills_the_worker() {
    // "One request at a time. Execute and clear abort a running request". Row: /hang;
    // execute TIME | the TIME response; the server sees the close.
    let h = http::start();
    let mut r = rig();
    r.start(format!("GET {}", h.url("/hang")).as_bytes());
    assert!(h.sees_request());
    r.send(b"TIME"); // no clear: the execute alone must abort the GET
    assert_eq!(r.execute(), AVAIL);
    let (got, end) = r.drain();
    assert!(end == DONE && is_time_shape(&got), "{:?}", String::from_utf8_lossy(&got));
    assert!(h.sees_close());
}

#[test]
fn the_worker_gets_an_empty_environment() {
    // GET client: "run ... with an empty environment". The emulator binary runs with proxy
    // variables pointing at a closed port; N still reaches H, so curl never saw them.
    // A subprocess, so this test process's environment is untouched.
    use std::io::Write;
    use std::process::{Command, Stdio};
    let h = http::start();
    let proxy = format!("http://127.0.0.1:{}", closed_port());
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir(dir.path().join("rom")).unwrap();
    for f in ["monitor.bin", "monitor.sym"] {
        std::fs::copy(Path::new("rom").join(f), dir.path().join("rom").join(f)).unwrap();
    }
    let mut child = Command::new(env!("CARGO_BIN_EXE_intel8080"))
        .current_dir(dir.path())
        .envs([("http_proxy", &proxy), ("HTTP_PROXY", &proxy), ("ALL_PROXY", &proxy), ("all_proxy", &proxy)])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    // N, then a HLT at 0100 to end the piped run.
    let input = format!("N {}\rF 0100 0100 76\rG 0100\r", h.url("/hello"));
    child.stdin.take().unwrap().write_all(input.as_bytes()).unwrap();
    let out = child.wait_with_output().unwrap();
    let out = String::from_utf8_lossy(&out.stdout);
    assert!(out.contains(&format!("N {}\r\nHello\r\r\n\r\n> ", h.url("/hello"))), "{:?}", out);
}

#[test]
fn reset_kills_the_worker() {
    // "RESET and power-on abort it the same way (rule 2.8). In the emulator and the daemon,
    // dropping the device does it." Row: /hang > D.BIN; drop the device | the server sees
    // the close | no ~D.BIN.
    let h = http::start();
    let mut r = rig();
    r.start(format!("GET {} > D.BIN", h.url("/hang")).as_bytes());
    assert!(h.sees_request());
    let (bus, _con) = build_bus(r.dir(), mailbox::local_time);
    r.bus = bus;
    assert!(h.sees_close());
    assert_eq!(r.listing(), Vec::<String>::new());
}

#[test]
#[ignore = "takes 10 s: cargo test -- --ignored"]
fn get_connect_limit() {
    // Time limits: "each connection (name lookup, TCP connect, TLS handshake) takes at most
    // 10 s, or the request fails with 83." Row: https://S/, S accepts and never answers |
    // 83 after 10-12 s.
    let s = http::silent();
    let mut r = rig();
    let t = Instant::now();
    r.start(format!("GET https://{}/", s).as_bytes());
    assert_eq!(r.finish_within("https://S/", 15), (E_SERVICE, vec![]));
    let secs = t.elapsed().as_secs_f64();
    assert!((10.0..12.0).contains(&secs), "{} s", secs);
}

#[test]
#[ignore = "takes 30 s: cargo test -- --ignored"]
fn get_stall_limit_before_the_first_byte() {
    // "Stall: from the first connect on, including the wait for the response headers, a
    // request that moves less than 1 byte per second averaged over 30 s fails with 83."
    // Row: /hang | 83 after 30-35 s.
    let h = http::start();
    let mut r = rig();
    let t = Instant::now();
    r.start(format!("GET {}", h.url("/hang")).as_bytes());
    assert_eq!(r.finish_within("/hang", 40), (E_SERVICE, vec![]));
    let secs = t.elapsed().as_secs_f64();
    assert!((30.0..35.0).contains(&secs), "{} s", secs);
}

#[test]
#[ignore = "takes 30 s: cargo test -- --ignored"]
fn get_stall_limit_mid_body() {
    // Row: /drip | 03 never; abc, then 83 after 30-40 s. Stream form: "A failure after some
    // bytes (a stall, a dropped connection) is ERROR 83 mid-response".
    let h = http::start();
    let mut r = rig();
    let t = Instant::now();
    r.start(format!("GET {}", h.url("/drip")).as_bytes());
    assert_eq!(r.finish_within("/drip", 45), (E_SERVICE, b"abc".to_vec()));
    let secs = t.elapsed().as_secs_f64();
    assert!((30.0..40.0).contains(&secs), "{} s", secs);
}

#[test]
fn get_grammar_errors_are_82() {
    // Grammar: "Anything else gives 82: no token ..., two tokens (GET url >FILE), a second
    // token other than >, four or more tokens." "<url> begins with http:// or https://
    // exactly (lowercase), has at least one byte after the //, and has every byte in
    // 21h-7Eh." "<FILE> is checked by section 7, Mount steps 2-3."
    let h = http::start();
    let url = |p: &str| h.url(p);
    let mut rows: Vec<Vec<u8>> = vec![
        b"GET".to_vec(), b"GET ".to_vec(), b"GET    ".to_vec(),
        format!("GET ftp://{}/", h.host()).into_bytes(),
        format!("GET HTTP://{}/", h.host()).into_bytes(),
        b"GET http://".to_vec(), b"GET hello".to_vec(),
        format!("GET {}\tb", url("/a")).into_bytes(),
        format!("GET {} b", url("/a")).into_bytes(),
        format!("GET {} >F", url("/a")).into_bytes(),
        format!("GET {} > F G", url("/a")).into_bytes(),
        format!("GET {} < F", url("/a")).into_bytes(),
        format!("GET {} > ", url("/a")).into_bytes(),
        format!("GET {} > BAD/NAME", url("/a")).into_bytes(),
        format!("GET {} > 1234567890123", url("/a")).into_bytes(),
        format!("GET {} > ..", url("/a")).into_bytes(),
    ];
    let mut high = format!("GET {}", url("/")).into_bytes();
    high.push(0x80);
    rows.push(high);
    for command in rows {
        let mut r = rig();
        assert_eq!(r.start(&command), E_ARGS, "{:?}", String::from_utf8_lossy(&command));
        assert_eq!(r.listing(), Vec::<String>::new());
    }
    let mut r = rig();
    assert_eq!(r.start(format!("get {}", url("/hello")).as_bytes()), E_UNKNOWN);
    let mut long = b"GET ".to_vec();
    long.extend([b'a'; 125]);
    assert_eq!(r.start(&long), E_OVERFLOW);
}
