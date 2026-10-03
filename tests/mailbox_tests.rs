// mailbox_tests.rs - Service Mailbox (ports 10-13) at port level, DEVICE_SPECS 8,
// plus the rules of DEVICE_SPECS 2 that name the mailbox. Written black-box from the
// spec text: each test quotes the sentence it checks.
//
// Every access goes through an IoBus from build_bus (the port map main.rs and the Pi
// daemon use). Tests that need a known time, or an unset Pi clock, replace ports 10-13
// with a mailbox built on an injected clock; that is the only place the device type is
// named.
//
// API used (only `rig_with_clock` names the device):
//   - build_bus(dir) maps one Service Mailbox device at 10, 11, 12, 13, using the host clock.
//   - intel8080_emu::io::devices::mailbox::Mailbox implements IoDevice.
//   - Mailbox::new(clock), clock a plain fn returning Some((year, month, day, hour, minute,
//     second)) of local time, or None for "the Pi clock is not set" (TIME gives 83). A
//     plain fn can't capture, so the test clock reads a thread-local (each test runs on
//     its own thread) that `set_clock` changes.
//
// Not testable at port level in Phase 6: BUSY (TIME completes within execute), a request
// that produces bytes later or fails mid-response, an empty response, abort of a running
// background request, interrupts (rule 2.7). Rule 2.9 (Pi service restart) only as
// "a fresh device reads 00".

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use intel8080_emu::io::build_bus;
use intel8080_emu::io::devices::mailbox::Mailbox;
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
    let (bus, _con) = build_bus(dir.path());
    Rig { _dir: dir, bus }
}

/// build_bus, then ports 10-13 replaced by a mailbox whose clock reads NOW (set_clock).
fn rig_with_clock() -> Rig {
    let mut r = rig();
    let mb = Rc::new(RefCell::new(Mailbox::new(test_clock)));
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
    /// IN 12, checked against what Phase 6 can ever return.
    /// States: "04-7F | - | Never returned". Transitions, Phase 6: TIME "never reads 01".
    /// Error codes: "84-FF | Reserved".
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
    /// "Phase 6: TIME completes within the execute access. Right after execute, IN 12
    /// reads 02 or an error code (80-83) ... It never reads 01."
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
    // Command buffer: "Each OUT 10 appends one byte." OUT 10 is not an event in the
    // transitions table, so it changes no state and no response; the bytes wait in the
    // buffer for the next execute (which empties it).
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
fn placeholder_commands_are_unknown_in_phase_6() {
    // Commands lists ASM, DIS, GET and ASK as placeholders for later phases; until then
    // they are unknown words (80), with or without arguments.
    let mut r = rig();
    for command in [&b"ASM"[..], b"ASM NOP", b"DIS", b"DIS 00", b"GET", b"GET x", b"ASK", b"ASK hi"] {
        assert_eq!(r.command(command), E_UNKNOWN, "{:?}", String::from_utf8_lossy(command));
    }
}

// ---------- TIME ----------

#[test]
fn time_is_19_bytes_with_no_line_ending() {
    // Commands: "TIME ... | 19 bytes, YYYY-MM-DD HH:MM:SS: the Pi's local time, 24-hour,
    // zero-padded, with no line ending. Example: 2026-10-02 14:30:05."
    // Command format: "Text responses use CR LF ... except for TIME".
    set_clock(Some(T1));
    let mut r = rig_with_clock();
    assert_eq!(r.command(b"TIME"), AVAIL);
    assert_eq!(r.drain(), (T1_TEXT.to_vec(), DONE));
}

#[test]
fn time_is_zero_padded_24_hour() {
    // Commands: "24-hour, zero-padded".
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
    // Commands: "19 bytes, YYYY-MM-DD HH:MM:SS ... zero-padded". (Reconciler addition: the
    // year range is unspecified. The clock must return in-range fields; the device does
    // not check them. Logged for Mike.)
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
    // Transitions: "Phase 6: TIME completes within the execute access." The response is
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
    // Commands: "If the Pi clock is not set (no NTP sync and no RTC), the result is 83."
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
    // Commands: "the Pi's local time ... The emulator uses the host clock".
    // Transitions, Phase 6: "TIME completes within the execute access. Right after
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
        let (mut bus, _con) = build_bus(dir.path());
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
        let (bus, _con) = build_bus(dir.path());
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
