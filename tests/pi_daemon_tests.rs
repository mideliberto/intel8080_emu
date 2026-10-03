// pi_daemon_tests.rs - pi8080d fault, RESET, startup, stop and console tests (PI_DAEMON
// 13.3). The daemon (`pi::setup_pins` and `pi::serve`) runs on its own thread against the
// simulated board (`pi::sim`); the test thread plays the 8080 with begin/wait and the
// RESET line, and the console client over TCP. No CPU. The board checks every handshake
// obligation (13.1) on every access, so each test also checks those.
//
// "Pulse RESET" is reset(true), 2 ms, reset(false). "After a pass" means edge_calls() has
// risen by 2 since the access completed (a console pass makes exactly one reset_edge call).

use intel8080_emu::io::devices::mailbox;
use intel8080_emu::pi::sim::{Access, Done, Knobs, SimBoard};
use intel8080_emu::pi::{self, ACK, GPFSEL2, GPIO_PUP_PDN_CNTRL_REG1, GPLEV0, LATCH};
use std::io::{Read, Write};
use std::net::{Shutdown, SocketAddr, TcpListener, TcpStream};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering::Relaxed};
use std::sync::Arc;
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

const TIMEOUT: Duration = Duration::from_secs(10);

/// A board, a storage directory, a bound console listener and, once started, the daemon thread.
struct Rig {
    board: SimBoard,
    dir: tempfile::TempDir,
    addr: SocketAddr,
    listener: Option<TcpListener>,
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<Result<(), String>>>,
    /// Give serve a trace file (the default).
    traced: bool,
}

impl Rig {
    fn new(knobs: Knobs) -> Rig {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        Rig {
            board: SimBoard::new(knobs),
            dir: tempfile::tempdir().unwrap(),
            addr: listener.local_addr().unwrap(),
            listener: Some(listener),
            stop: Arc::new(AtomicBool::new(false)),
            thread: None,
            traced: true,
        }
    }

    fn storage(&self) -> PathBuf {
        self.dir.path().join("storage")
    }

    fn trace(&self) -> PathBuf {
        self.dir.path().join("trace.txt")
    }

    fn connect(&self) -> TcpStream {
        let client = TcpStream::connect(self.addr).unwrap();
        client.set_read_timeout(Some(TIMEOUT)).unwrap();
        client
    }

    /// Starts the daemon thread: setup_pins, then serve with the host clock and a trace.
    /// Returns after its first console pass, so a RESET the test makes next happens while
    /// it runs (a thread slow to start would otherwise find the edge after later accesses).
    fn start(&mut self) {
        let (board, storage, trace) = (self.board.clone(), self.storage(), self.trace());
        let (listener, stop, traced) = (self.listener.take().unwrap(), self.stop.clone(), self.traced);
        self.thread = Some(std::thread::spawn(move || {
            let fsel2 = pi::setup_pins(&board)?;
            let trace = traced.then(|| std::fs::File::create(trace).unwrap());
            pi::serve(board, fsel2, &storage, mailbox::local_time, listener, trace, &stop)
        }));
        self.board.wait_edge_calls(1);
    }

    /// Sets the stop flag, joins the daemon, checks the board and returns serve's result
    /// and the trace lines with ` ; xN` stripped from IN/OUT lines (poll counts depend on
    /// timing). RESET lines keep it, so two back-to-back resets never pass as one.
    fn finish(&mut self) -> (Result<(), String>, Vec<String>) {
        self.stop.store(true, Relaxed);
        let result = self.thread.take().unwrap().join();
        self.board.check();
        let text = std::fs::read_to_string(self.trace()).unwrap_or_default();
        let lines = text
            .lines()
            .map(|l| if l.starts_with("RESET") { l } else { l.split(" ; ").next().unwrap() })
            .map(String::from)
            .collect();
        (result.expect("daemon thread panicked"), lines)
    }

    fn inp(&self, port: u8) -> u8 {
        self.board.begin(port, Access::In);
        match self.board.wait() {
            Done::In(v) => v,
            d => panic!("IN {:02X}: {:?}", port, d),
        }
    }

    fn out(&self, port: u8, v: u8) {
        self.board.begin(port, Access::Out(v));
        assert_eq!(self.board.wait(), Done::Out, "OUT {:02X} {:02X}", port, v);
    }

    fn mount(&self, name: &[u8]) {
        for &b in name {
            self.out(0x0D, b);
        }
        self.out(0x0E, 0x01);
        assert_eq!(self.inp(0x0F), 0x00);
    }

    /// Polls IN 02 until input is waiting.
    fn wait_input(&self) {
        let deadline = Instant::now() + TIMEOUT;
        while self.inp(0x02) != 0x03 {
            assert!(Instant::now() < deadline, "no console input arrived");
        }
    }

    fn file(&self, name: &str) -> Vec<u8> {
        std::fs::read(self.storage().join(name)).unwrap()
    }
}

fn recv(client: &mut TcpStream, n: usize) -> Vec<u8> {
    let mut buf = vec![0; n];
    client.read_exact(&mut buf).unwrap();
    buf
}

fn resets(trace: &[String]) -> usize {
    trace.iter().filter(|l| *l == "RESET").count()
}

// ---------- RESET ----------

#[test]
fn reset_pulse_mid_in_is_never_acked_and_resets_devices() {
    let mut rig = Rig::new(Knobs::default());
    let mut client = rig.connect();
    rig.start();
    rig.mount(b"A.BIN");
    rig.out(0x11, 0x01); // execute an empty command: ERROR 80
    assert_eq!(rig.inp(0x12), 0x80);
    client.write_all(b"xyz").unwrap();
    rig.wait_input();
    rig.board.pause_next_request();
    rig.board.begin(0x0C, Access::In);
    rig.board.wait_paused();
    rig.board.pulse_reset();
    // The pulse has come and gone: only REQ low at an abort-rule read shows it (5.2 table
    // row 3). The 8080 issues nothing until the daemon has handled it (its reset_edge call
    // after the release), so the edge latch at step 5 can't stand in for that read.
    let calls = rig.board.edge_calls();
    rig.board.resume();
    rig.board.wait_edge_calls(calls + 1);
    assert_eq!(rig.board.wait(), Done::Aborted);
    assert_eq!(rig.inp(0x0C) & 0x01, 0x00, "storage still mounted");
    assert_eq!(rig.inp(0x0F), 0x01);
    assert_eq!(rig.inp(0x02), 0x02, "console FIFO kept its input");
    assert_eq!(rig.inp(0x01), 0x00);
    assert_eq!(rig.inp(0x12), 0x00);
    let (result, trace) = rig.finish();
    assert_eq!(result, Ok(()));
    assert_eq!(resets(&trace), 1, "{:?}", trace);
    let at = trace.iter().position(|l| l == "RESET").unwrap();
    assert_eq!(trace[at - 1], "IN 02 03", "the paused access has a trace line");
    assert_eq!(trace[at + 1..], ["IN 0C 82", "IN 0F 01", "IN 02 02", "IN 01 00", "IN 12 00"]);
}

#[test]
fn reset_pulse_mid_out_is_never_acked_and_resets_devices() {
    let mut rig = Rig::new(Knobs::default());
    let mut client = rig.connect();
    rig.start();
    rig.mount(b"A.BIN");
    rig.board.pause_next_request();
    rig.board.begin(0x0B, Access::Out(0x5A));
    rig.board.wait_paused();
    rig.board.pulse_reset();
    let calls = rig.board.edge_calls();
    rig.board.resume();
    rig.board.wait_edge_calls(calls + 1);
    assert_eq!(rig.board.wait(), Done::Aborted);
    assert_eq!(rig.inp(0x0C) & 0x01, 0x00, "storage still mounted");
    rig.out(0x00, b'k');
    assert_eq!(recv(&mut client, 1), b"k", "the client was dropped");
    let (result, trace) = rig.finish();
    assert_eq!(result, Ok(()));
    assert_eq!(resets(&trace), 1, "{:?}", trace);
    assert!(!trace.iter().any(|l| l.starts_with("OUT 0B")), "{:?}", trace);
}

#[test]
fn reset_and_reboot_during_a_slow_access() {
    // 5.2 table row 4: the pulse comes and goes while the device call runs, and the
    // rebooted 8080 has a new request up by the time the daemon reaches step 5. Only the
    // edge latch can tell.
    let mut rig = Rig::new(Knobs::default());
    let mut client = rig.connect();
    rig.start();
    rig.mount(b"A.BIN");
    rig.board.pause_next_request();
    rig.board.begin(0x0B, Access::Out(0x5A));
    rig.board.wait_paused();
    rig.board.pulse_reset();
    rig.board.begin(0x00, Access::Out(b'B'));
    rig.board.resume();
    assert_eq!(rig.board.wait(), Done::Aborted);
    assert_eq!(rig.board.wait(), Done::Out);
    assert_eq!(recv(&mut client, 1), b"B");
    assert_eq!(rig.inp(0x0C) & 0x01, 0x00, "storage still mounted");
    let (result, trace) = rig.finish();
    assert_eq!(result, Ok(()));
    let at = trace.iter().position(|l| l == "RESET").unwrap();
    assert_eq!(trace[at - 1], "IN 0F 00");
    assert_eq!(trace[at + 1..], ["OUT 00 42", "IN 0C 82"]);
}

#[test]
fn reset_pulse_during_in_readback_does_not_hang() {
    let mut rig = Rig::new(Knobs::default());
    rig.start();
    rig.board.stick_d(Some((1, false))); // IN 02 = 02 never reads back
    rig.board.begin(0x02, Access::In);
    std::thread::sleep(Duration::from_millis(5));
    rig.board.pulse_reset();
    rig.board.stick_d(None);
    assert_eq!(rig.board.wait(), Done::Aborted);
    assert_eq!(rig.inp(0x0F), 0x01);
    let (result, trace) = rig.finish();
    assert_eq!(result, Ok(()));
    assert_eq!(trace, ["RESET", "IN 0F 01"]);
}

#[test]
fn reset_held_blocks_service_until_release() {
    let mut rig = Rig::new(Knobs::default());
    let mut client = rig.connect();
    rig.start();
    rig.board.pause_next_request();
    rig.board.begin(0x00, Access::Out(b'h'));
    rig.board.wait_paused();
    rig.board.reset(true);
    rig.board.resume();
    assert_eq!(rig.board.wait(), Done::Aborted);
    std::thread::sleep(Duration::from_millis(20));
    rig.board.reset(false);
    assert_eq!(rig.inp(0x0F), 0x01);
    // The aborted OUT 00 went to the old console, which RESET dropped.
    rig.out(0x00, b'z');
    assert_eq!(recv(&mut client, 1), b"z");
    let (result, trace) = rig.finish();
    assert_eq!(result, Ok(()));
    assert_eq!(trace, ["RESET", "IN 0F 01", "OUT 00 7A"]);
}

#[test]
fn reset_while_idle_resets_devices() {
    let mut rig = Rig::new(Knobs::default());
    let mut client = rig.connect();
    rig.start();
    client.write_all(b"ab").unwrap();
    rig.wait_input();
    // The FIFO holds input, so the daemon leaves these in the socket: RESET discards them.
    client.write_all(b"cd").unwrap();
    std::thread::sleep(Duration::from_millis(5));
    rig.board.pulse_reset();
    assert_eq!(rig.inp(0x02), 0x02);
    rig.board.wait_edge_calls(rig.board.edge_calls() + 2);
    assert_eq!(rig.inp(0x02), 0x02, "bytes left in the socket at RESET reached the FIFO");
    let (result, trace) = rig.finish();
    assert_eq!(result, Ok(()));
    assert_eq!(resets(&trace), 1, "{:?}", trace);
}

#[test]
fn a_late_release_event_does_not_reset_twice() {
    let mut rig = Rig::new(Knobs { release_event_delay: Duration::from_millis(5), ..Knobs::default() });
    let mut client = rig.connect();
    rig.start();
    rig.board.pulse_reset();
    rig.out(0x00, b'a');
    std::thread::sleep(Duration::from_millis(10));
    rig.out(0x00, b'b');
    assert_eq!(recv(&mut client, 2), b"ab");
    let (result, trace) = rig.finish();
    assert_eq!(result, Ok(()));
    assert_eq!(trace, ["RESET", "OUT 00 61", "OUT 00 62"]);
}

// ---------- Bus loop and startup ----------

#[test]
fn a_request_pending_at_start_is_served() {
    let mut rig = Rig::new(Knobs::default());
    let mut client = rig.connect();
    rig.board.begin(0x00, Access::Out(b'Q'));
    rig.start();
    assert_eq!(rig.board.wait(), Done::Out);
    assert_eq!(recv(&mut client, 1), b"Q");
    let (result, trace) = rig.finish();
    assert_eq!(result, Ok(()));
    assert_eq!(trace, ["OUT 00 51"]);
}

#[test]
fn back_to_back_requests_never_wait_for_req_low() {
    // With no gap and no REQ lag, GPLEV0 never shows REQ low between the queued accesses;
    // a daemon that waits for REQ low hangs, and the 10 s timeout fails the test.
    // No trace, and the daemon drains the queue before the first wait(): trace formatting
    // and a test thread woken on every ACK edge would hide a missing 500 ns blanking spin.
    // That catch is timing-dependent (about 19 runs in 20 on the dev Mac).
    let mut rig = Rig::new(Knobs { gap: Duration::ZERO, req_fall: Duration::ZERO, ..Knobs::default() });
    rig.traced = false;
    rig.start();
    for i in 0..500u32 {
        rig.board.begin(0x08, Access::Out(i as u8));
        rig.board.begin(0x08, Access::In);
    }
    let deadline = Instant::now() + TIMEOUT;
    while rig.board.completed() < 1000 && Instant::now() < deadline {
        rig.board.check();
        std::thread::sleep(Duration::from_millis(5));
    }
    for i in 0..500u32 {
        assert_eq!(rig.board.wait(), Done::Out);
        assert_eq!(rig.board.wait(), Done::In(i as u8));
    }
    let (result, _) = rig.finish();
    assert_eq!(result, Ok(()));
}

#[test]
fn startup_releases_d_and_drives_ack_and_latch_low() {
    // A daemon that crashed mid-IN: D0-D7 outputs, ACK and LATCH outputs latched high.
    let knobs = Knobs::default();
    let fsel = [knobs.fsel[0], knobs.fsel[1] | 0o11 << 18, knobs.fsel[2] | 0x0024_9249];
    let mut rig = Rig::new(Knobs { fsel, ..knobs });
    assert_eq!(rig.board.peek(GPLEV0) & (ACK | LATCH), ACK | LATCH);
    pi::setup_pins(&rig.board).unwrap();
    assert_eq!(rig.board.peek(GPFSEL2) & 0x00FF_FFFF, 0, "D0-D7 not inputs");
    assert_eq!(rig.board.peek(GPLEV0) & (ACK | LATCH), 0);
    // Pull-down (10) on ACK, LATCH and D0-D7 (PI_DAEMON 3.3 step 4); the board starts them at pull-up.
    let pulls = rig.board.peek(GPIO_PUP_PDN_CNTRL_REG1);
    for pin in [16, 17, 20, 21, 22, 23, 24, 25, 26, 27] {
        assert_eq!(pulls >> ((pin - 16) * 2) & 3, 0b10, "BCM {} pull, REG1 {:08X}", pin, pulls);
    }
    rig.start();
    assert_eq!(rig.inp(0x0F), 0x01);
    let (result, _) = rig.finish();
    assert_eq!(result, Ok(()));
}

#[test]
fn a_pin_in_an_alt_function_refuses_to_start() {
    let knobs = Knobs::default();
    let board = SimBoard::new(Knobs { fsel: [knobs.fsel[0] | 0o4 << 21, knobs.fsel[1], knobs.fsel[2]], ..knobs });
    let e = pi::setup_pins(&board).unwrap_err();
    assert_eq!(e, "BCM 7 is in an ALT function");
    assert_eq!(board.writes(), 0);
}

#[test]
fn ack_stuck_high_refuses_to_start() {
    let board = SimBoard::new(Knobs { ack_stuck_high: true, ..Knobs::default() });
    assert_eq!(pi::setup_pins(&board).unwrap_err(), "BCM 16 (ACK) reads high after drive low");
}

// ---------- Stop ----------

#[test]
fn stop_while_reset_is_held_returns_and_flushes() {
    let mut rig = Rig::new(Knobs::default());
    rig.start();
    rig.mount(b"A.BIN");
    rig.out(0x0B, 0x5A);
    rig.board.reset(true);
    std::thread::sleep(Duration::from_millis(5));
    let file = rig.storage().join("A.BIN");
    let (result, trace) = rig.finish();
    assert_eq!(result, Ok(()));
    assert_eq!(std::fs::read(file).unwrap(), [0x5A]);
    assert_eq!(trace.last().map(String::as_str), Some("RESET"));
}

#[test]
fn stop_with_a_stuck_d_bit_returns_and_flushes() {
    let mut rig = Rig::new(Knobs::default());
    rig.start();
    rig.mount(b"A.BIN");
    rig.out(0x0B, 0x5A);
    rig.board.stick_d(Some((1, false)));
    rig.board.begin(0x02, Access::In);
    std::thread::sleep(Duration::from_millis(5));
    let board = rig.board.clone();
    let (result, _) = rig.finish();
    assert_eq!(result, Ok(()));
    assert_eq!(rig.file("A.BIN"), [0x5A]);
    assert_eq!(board.peek(GPFSEL2) & 0x00FF_FFFF, 0, "D0-D7 not inputs");
    assert_eq!(board.peek(GPLEV0) & (ACK | LATCH), 0);
}

// ---------- Console ----------

#[test]
fn a_new_client_replaces_the_old() {
    let mut rig = Rig::new(Knobs::default());
    let mut one = rig.connect();
    rig.start();
    rig.out(0x00, b'a');
    assert_eq!(recv(&mut one, 1), b"a");
    let mut two = rig.connect();
    rig.out(0x00, b'b');
    assert_eq!(recv(&mut two, 1), b"b");
    let mut rest = Vec::new();
    one.read_to_end(&mut rest).unwrap();
    assert_eq!(rest, b"", "client 1 got more after client 2 connected");
    let (result, _) = rig.finish();
    assert_eq!(result, Ok(()));
}

#[test]
fn output_with_no_client_is_discarded() {
    let mut rig = Rig::new(Knobs::default());
    rig.start();
    rig.out(0x00, b'x');
    rig.board.wait_edge_calls(rig.board.edge_calls() + 2);
    let mut client = rig.connect();
    rig.out(0x00, b'y');
    assert_eq!(recv(&mut client, 1), b"y");
    let (result, _) = rig.finish();
    assert_eq!(result, Ok(()));
}

#[test]
fn half_closed_client_still_receives_output() {
    let mut rig = Rig::new(Knobs::default());
    let mut client = rig.connect();
    rig.start();
    client.write_all(b"xyz").unwrap();
    client.shutdown(Shutdown::Write).unwrap();
    let mut got = Vec::new();
    for _ in 0..3 {
        rig.wait_input();
        got.push(rig.inp(0x01));
    }
    assert_eq!(got, b"xyz");
    rig.out(0x00, b'a');
    assert_eq!(recv(&mut client, 1), b"a");
    let (result, _) = rig.finish();
    assert_eq!(result, Ok(()));
}

#[test]
fn input_arrives_in_order_and_unchanged() {
    // 100 KiB through a FIFO the daemon refills 4 KiB at a time, only when it is empty:
    // TCP flow control holds the rest, and nothing is lost.
    let mut rig = Rig::new(Knobs::default());
    let mut client = rig.connect();
    rig.start();
    let sent: Vec<u8> = (0..100 * 1024u32).map(|i| (i * 7 + i / 256) as u8).collect();
    let data = sent.clone();
    let writer = std::thread::spawn(move || client.write_all(&data).map(|_| client));
    let mut got = Vec::with_capacity(sent.len());
    while got.len() < sent.len() {
        rig.wait_input();
        got.push(rig.inp(0x01));
    }
    let _client = writer.join().unwrap().unwrap();
    assert!(got == sent, "input changed in transit");
    let (result, _) = rig.finish();
    assert_eq!(result, Ok(()));
}
