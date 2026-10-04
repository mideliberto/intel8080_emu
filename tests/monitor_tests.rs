// monitor_tests.rs - Monitor ROM integration tests: a strict transcript harness.
//
// Boot (ARCHITECTURE 3.1): RAM 0000-EFFF is filled with JUNK before reset, and the
// registers, flags and SP are set to junk after it, so the ROM can't lean on zeroed
// RAM, registers or a preset SP.
//
// A step types its input, then runs until the input is consumed (as many IN 01 as bytes
// typed), the OUT 00 bytes since the step began end with the "> " prompt, and nothing more
// is printed for 2,000 cycles. The output must match the transcript exactly. An
// exhausted cycle budget or a HLT is a failure.
//
// Two paths, one rule (PI_DAEMON 13.2): Local maps build_bus on the CPU's IoBus and talks
// to the Console directly; Daemon maps a Bridge on 00-6F that performs each access as a
// REQ/ACK handshake on the simulated board (pi::sim) against pi::serve on its own
// thread, and talks to the console over TCP. every_transcript_through_the_daemon plays
// every transcript both ways.
//
// Transcripts live in tests/transcripts/*.txt, so the same files can drive the
// hardware over the Pi TCP console. Format, one item per line:
//   # comment             ignored, as are blank lines
//   > text                type `text` CR. The expected output starts with the echo `text` CR LF.
//                         A lone `>` types an empty line.
//   < text                type `text` exactly (no CR added, no echo assumed)
//   anything else         one expected output line; CR LF is appended
// Escapes, in all three: \r \n \\ \xHH. In expected output only, \d matches any
// decimal digit (T prints the time, matched by shape). Write a trailing space as \x20, an
// output line that starts with "> " as \x3E\x20, one that starts with '#' as \x23,
// and an empty output line as a trailing \r\n on the line before it.
// Transcripts only display memory they wrote first: on hardware RAM is random.

use std::cell::RefCell;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::Path;
use std::rc::Rc;
use std::sync::atomic::{AtomicBool, Ordering::Relaxed};
use std::sync::Arc;
use std::time::{Duration, Instant};

use intel8080_emu::cpu::Transfer;
use intel8080_emu::debugger::Debugger;
use intel8080_emu::io::build_bus;
use intel8080_emu::io::devices::console::Console;
use intel8080_emu::io::devices::ask::AskConfig;
use intel8080_emu::io::devices::mailbox::{self, Mailbox};
use intel8080_emu::io::{IoBus, IoDevice};
use intel8080_emu::pi;
use intel8080_emu::pi::sim::{Bridge, Knobs, SimBoard};
use intel8080_emu::Intel8080;

mod support;
use support::http;

/// HLT. A NOP-like byte (00, or A5 = ANA L) would slide execution into F000 and
/// boot the ROM even with the overlay missing. RST 0 would jump back to 0000.
/// HLT stops the CPU at the first junk byte it executes, and a halt fails the step.
const JUNK: u8 = 0x76;
/// Pushes before `LXI SP` land in ROM and are lost, so a return through them crashes.
const JUNK_SP: u16 = 0x0000;
/// Cycles per step. `C 0000 FFFF` is the slowest command and needs about 10M.
const BUDGET: u64 = 30_000_000;
/// A step of an N server row (MONITOR_SPEC 6.18.1) waits on the network, not on the
/// 8080, so it is bounded by wall-clock time instead of BUDGET.
const NET_DEADLINE: Duration = Duration::from_secs(10);

/// Where the console is: the Console itself, or the daemon's TCP client.
enum Side {
    Local(Rc<RefCell<Console>>),
    Daemon(TcpStream),
}

struct Mon {
    cpu: Intel8080,
    side: Side,
    dir: tempfile::TempDir,
    /// Every IN and OUT since power-on, in order.
    ports: Vec<Transfer>,
    /// Steps end at NET_DEADLINE instead of BUDGET (N server rows).
    net: bool,
}

/// Power on with junk RAM and the port map main.rs uses (build_bus); returns the
/// monitor and what boot printed up to the first prompt.
fn power_on(overlay: bool) -> (Mon, Result<Vec<u8>, String>) {
    let dir = tempfile::tempdir().unwrap();
    let (bus, con) = build_bus(dir.path(), mailbox::local_time, AskConfig::default());
    start(bus, Side::Local(con), dir, overlay)
}

/// Power on with `bus` as the port map.
fn start(bus: IoBus, side: Side, dir: tempfile::TempDir, overlay: bool) -> (Mon, Result<Vec<u8>, String>) {
    let mut cpu = Intel8080::new();
    cpu.load_program(&vec![JUNK; 0xF000], 0x0000);
    *cpu.io_bus_mut() = bus;
    cpu.load_rom_from_file(Path::new("rom/monitor.bin")).unwrap();
    cpu.reset();
    (cpu.a, cpu.b, cpu.c, cpu.d, cpu.e, cpu.h, cpu.l) = (JUNK, JUNK, JUNK, JUNK, JUNK, JUNK, JUNK);
    cpu.flags = 0xD7;
    cpu.sp = JUNK_SP;
    if !overlay {
        cpu.rom_overlay_enabled = false;
    }
    let mut m = Mon { cpu, side, dir, ports: Vec::new(), net: false };
    let banner = m.step(b"");
    (m, banner)
}

fn boot() -> Mon {
    booted(power_on(true))
}

/// The monitor after a boot that printed the banner.
fn booted((m, banner): (Mon, Result<Vec<u8>, String>)) -> Mon {
    let banner = banner.unwrap_or_else(|e| panic!("boot: {}", e));
    let b = show(&banner);
    assert!(b.starts_with("\\r\\n8080 Monitor v"), "{}", b);
    assert!(b.ends_with("\\r\\nReady.\\r\\n"), "{}", b);
    m
}

/// Bytes as transcript text: printable ASCII as is, everything else escaped.
fn show(bytes: &[u8]) -> String {
    let mut s = String::new();
    for &b in bytes {
        match b {
            b'\r' => s.push_str("\\r"),
            b'\n' => s.push_str("\\n"),
            b'\\' => s.push_str("\\\\"),
            0x20..=0x7E => s.push(b as char),
            _ => s.push_str(&format!("\\x{:02X}", b)),
        }
    }
    s
}

/// Transcript text as bytes; None is `\d`, any decimal digit.
fn unescape(text: &str) -> Vec<Option<u8>> {
    let mut out = Vec::new();
    let mut it = text.bytes();
    while let Some(b) = it.next() {
        if b != b'\\' {
            out.push(Some(b));
            continue;
        }
        match it.next() {
            Some(b'r') => out.push(Some(b'\r')),
            Some(b'n') => out.push(Some(b'\n')),
            Some(b'\\') => out.push(Some(b'\\')),
            Some(b'd') => out.push(None),
            Some(b'x') => {
                let hex = [it.next().unwrap(), it.next().unwrap()];
                out.push(Some(u8::from_str_radix(std::str::from_utf8(&hex).unwrap(), 16).unwrap()));
            }
            e => panic!("bad escape \\{:?} in {:?}", e.map(|c| c as char), text),
        }
    }
    out
}

/// Typed transcript text: no `\d`.
fn typed(text: &str) -> Vec<u8> {
    unescape(text).into_iter().map(|b| b.unwrap_or_else(|| panic!("\\d in input {:?}", text))).collect()
}

impl Mon {
    /// The Console (local path only).
    fn con(&self) -> &Rc<RefCell<Console>> {
        match &self.side {
            Side::Local(con) => con,
            Side::Daemon(_) => panic!("no local console on the daemon path"),
        }
    }

    /// Type `input`, run to the next prompt, return what was printed (prompt stripped).
    fn step(&mut self, input: &[u8]) -> Result<Vec<u8>, String> {
        match &mut self.side {
            Side::Local(con) => {
                con.borrow_mut().take_output();
                con.borrow_mut().push_input(input);
            }
            Side::Daemon(client) => client.write_all(input).unwrap(),
        }
        let start = self.cpu.cycles;
        let started = Instant::now();
        // Since the step began: IN 01 count, OUT 00 bytes.
        let (mut read, mut out) = (0, Vec::new());
        let mut quiet_since = None;
        loop {
            if self.cpu.halted {
                return Err(format!("HLT at PC={:04X} after {:?}", self.cpu.pc, show(input)));
            }
            let over = if self.net { started.elapsed() > NET_DEADLINE } else { self.cpu.cycles - start > BUDGET };
            if over {
                let budget = if self.net { format!("{:?}", NET_DEADLINE) } else { format!("{} cycles", BUDGET) };
                return Err(format!("no prompt within {} after {:?}, PC={:04X}, output {:?}",
                    budget, show(input), self.cpu.pc, show(&out)));
            }
            self.cpu.execute_one();
            for &t in self.cpu.transfers() {
                match t {
                    Transfer::In(0x01, _) => read += 1,
                    Transfer::Out(0x00, b) => out.push(b),
                    _ => {}
                }
                if matches!(t, Transfer::In(..) | Transfer::Out(..)) {
                    self.ports.push(t);
                }
            }
            let at_prompt = read >= input.len() && out.ends_with(b"> ");
            // At a prompt with no input left, it must stay quiet: a dump line can end in "> ".
            match quiet_since {
                Some((cycles, n)) if n == out.len() && at_prompt => {
                    if self.cpu.cycles - cycles > 2_000 {
                        break;
                    }
                }
                _ => quiet_since = if at_prompt { Some((self.cpu.cycles, out.len())) } else { None },
            }
        }
        let got = match &mut self.side {
            Side::Local(con) => con.borrow_mut().take_output(),
            Side::Daemon(client) => {
                let mut got = vec![0; out.len()];
                client.read_exact(&mut got).map_err(|e| format!("console client after {:?}: {}", show(input), e))?;
                got
            }
        };
        Ok(got[..got.len() - 2].to_vec())
    }

    /// Type one command line; return its output with the echo stripped.
    fn run(&mut self, line: &str) -> String {
        let out = self.step(format!("{}\r", line).as_bytes()).unwrap_or_else(|e| panic!("{}", e));
        let out = show(&out);
        let echo = format!("{}\\r\\n", line);
        out.strip_prefix(&echo).unwrap_or_else(|| panic!("echo mismatch: {:?}", out)).to_string()
    }

    /// Play tests/transcripts/<name>.txt. Every step must match exactly.
    fn play(&mut self, name: &str) {
        for (line, input, expected) in transcript(name) {
            let at = format!("tests/transcripts/{}.txt:{}", name, line);
            let got = self.step(&input).unwrap_or_else(|e| panic!("{}: {}", at, e));
            check(&at, &got, &expected);
        }
    }

    /// Run until PC = `addr` (input already typed).
    fn run_to(&mut self, addr: u16) {
        let start = self.cpu.cycles;
        while self.cpu.pc != addr {
            assert!(!self.cpu.halted && self.cpu.cycles - start < BUDGET, "never reached {:04X}", addr);
            self.cpu.execute_one();
        }
    }

    fn mem(&mut self, addr: u16, n: usize) -> Vec<u8> {
        (0..n).map(|i| self.cpu.read_byte(addr.wrapping_add(i as u16))).collect()
    }
}

/// tests/transcripts/<name>.txt as steps: (line number, the bytes typed, the expected
/// output up to the prompt, which is not included; None is `\d`).
fn transcript(name: &str) -> Vec<(usize, Vec<u8>, Vec<Option<u8>>)> {
    let path = format!("tests/transcripts/{}.txt", name);
    let text = std::fs::read_to_string(&path).unwrap();
    let mut steps: Vec<(usize, Vec<u8>, Vec<Option<u8>>)> = Vec::new();
    for (i, line) in text.lines().enumerate() {
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if line == ">" || line.starts_with("> ") {
            let line = typed(line.get(2..).unwrap_or(""));
            let echo = line.iter().copied().chain(*b"\r\n").map(Some).collect();
            steps.push((i + 1, [&line[..], b"\r"].concat(), echo));
        } else if let Some(line) = line.strip_prefix("< ") {
            steps.push((i + 1, typed(line), Vec::new()));
        } else {
            let step = steps.last_mut().unwrap_or_else(|| panic!("{}:{}: output before input", path, i + 1));
            step.2.extend(unescape(line));
            step.2.extend([Some(b'\r'), Some(b'\n')]);
        }
    }
    steps
}

/// Fails unless `got` is `expected`. A digit where the transcript has \d shows as \d, so
/// the two strings compare.
fn check(at: &str, got: &[u8], expected: &[Option<u8>]) {
    let want: String = expected.iter().map(|e| e.map_or("\\d".to_string(), |b| show(&[b]))).collect();
    let seen: String = got.iter().enumerate().map(|(i, &b)| match expected.get(i) {
        Some(None) if b.is_ascii_digit() => "\\d".to_string(),
        _ => show(&[b]),
    }).collect();
    assert_eq!(seen, want, "{}", at);
}

/// A ROM label's address, from rom/monitor.sym (built and committed with monitor.bin).
fn sym(name: &str) -> u16 {
    let text = std::fs::read_to_string("rom/monitor.sym").unwrap();
    let line = text.lines().find(|l| l.get(5..) == Some(name)).unwrap_or_else(|| panic!("no symbol {}", name));
    u16::from_str_radix(&line[..4], 16).unwrap()
}

// ---------- Boot ----------

#[test]
fn boot_banner() {
    let (_m, banner) = power_on(true);
    let b = show(&banner.unwrap());
    // MONITOR_SPEC 1.1: tests never match the version, date or time.
    let lines: Vec<&str> = b.split("\\r\\n").collect();
    assert_eq!(lines.len(), 5, "{}", b);
    assert_eq!(lines[0], "");
    assert!(lines[1].starts_with("8080 Monitor v"), "{}", b);
    assert!(lines[2].starts_with("Built: "), "{}", b);
    assert_eq!(lines[3], "Ready.");
    assert_eq!(lines[4], "");
}

#[test]
fn boot_assumes_nothing() {
    let mut m = boot();
    // Boot wrote nothing in the user area (ARCHITECTURE 1: 0100-EEFF only on command).
    assert!(m.mem(0x0100, 0xEE00).iter().all(|&b| b == JUNK));
    // The overlay is off and 0000-007F was left alone: D shows RAM junk, not ROM.
    assert!(!m.cpu.rom_overlay_enabled);
    // Not even the RST 6 vector: only G writes 0030-0032 (MONITOR_SPEC 8.1).
    assert!(m.mem(0x0000, 0x80).iter().all(|&b| b == JUNK));
    assert_eq!(m.run("D 0000 000F"),
        "0000: 76 76 76 76 76 76 76 76  76 76 76 76 76 76 76 76  vvvvvvvvvvvvvvvv\\r\\n");
    // Bare D and bare E start at 0000 after cold start (LAST_DUMP_ADDR, LAST_EXAM_ADDR).
    let mut m = boot();
    assert!(m.run("D").starts_with("0000: 76 "));
    assert_eq!(show(&m.step(b"E\r.").unwrap()), "E\\r\\n0000: 76-\\r\\n");
}

#[test]
fn boot_io_is_out_fe_then_console_output() {
    // ARCHITECTURE 3.2 rule 2: from reset to the first prompt the ROM runs OUT FE and
    // OUT 00 and nothing else. CONOUT does not poll (MONITOR_SPEC 11). Then the prompt
    // waits in CONIN, polling IN 02.
    let (m, banner) = power_on(true);
    let banner = banner.unwrap();
    assert_eq!(m.ports[0], Transfer::Out(0xFE, 0x00));
    let printed: Vec<u8> = m.ports[1..].iter().map_while(|t| match t {
        Transfer::Out(0x00, v) => Some(*v),
        _ => None,
    }).collect();
    assert_eq!(printed, [&banner[..], b"> "].concat());
    let rest = &m.ports[1 + printed.len()..];
    assert!(!rest.is_empty() && rest.iter().all(|t| *t == Transfer::In(0x02, 0x02)), "{:?}", &rest[..rest.len().min(4)]);
}

#[test]
fn boot_fails_without_overlay() {
    // Guards the junk choice: with zeroed RAM this booted fine (a NOP slide into F000).
    let (m, banner) = power_on(false);
    let e = banner.expect_err("booted without the overlay");
    assert!(e.starts_with("HLT at PC=0001"), "{}", e);
    assert!(m.con().borrow().output().is_empty());
}

// ---------- Transcripts ----------

#[test]
fn every_transcript_through_the_daemon() {
    // PI_DAEMON 13.2: every transcript, each from a fresh power-on, with ports 00-6F served
    // by the daemon on the simulated board and the console over TCP. Then the daemon's
    // trace must equal the CPU's own port sequence for 00-6F, collapsed by the
    // ARCHITECTURE 7.3 repeat rule, line for line.
    let mut names: Vec<String> = std::fs::read_dir("tests/transcripts").unwrap()
        .filter_map(|e| e.unwrap().file_name().into_string().ok()?.strip_suffix(".txt").map(String::from))
        .collect();
    names.sort();
    assert!(names.len() >= 18, "{:?}", names);
    for name in names {
        let board = SimBoard::new(Knobs::default());
        let dir = tempfile::tempdir().unwrap();
        let logs = tempfile::tempdir().unwrap();
        let trace = logs.path().join("trace.txt");
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        // Connected before the daemon starts, so the banner is never discarded for want of a client.
        let client = TcpStream::connect(listener.local_addr().unwrap()).unwrap();
        client.set_read_timeout(Some(Duration::from_secs(10))).unwrap();
        let stop = Arc::new(AtomicBool::new(false));
        let daemon = {
            let (board, storage, trace, stop) = (board.clone(), dir.path().to_path_buf(), trace.clone(), stop.clone());
            std::thread::spawn(move || {
                let fsel2 = pi::setup_pins(&board)?;
                let trace = std::fs::File::create(trace).unwrap();
                pi::serve(board, fsel2, &storage, mailbox::local_time, AskConfig::default(), listener, Some(trace), &stop)
            })
        };
        let mut bus = IoBus::new();
        let bridge = Rc::new(RefCell::new(Bridge(board.clone())));
        for port in 0x00..=0x6F {
            bus.map_port(port, bridge.clone());
        }
        let mut m = booted(start(bus, Side::Daemon(client), dir, true));
        m.play(&name);
        stop.store(true, Relaxed);
        let result = daemon.join();
        board.check();
        assert_eq!(result.expect("daemon thread panicked"), Ok(()), "{}", name);
        let mut want: Vec<(String, usize)> = Vec::new();
        for t in &m.ports {
            let line = match *t {
                Transfer::In(p @ 0x00..=0x6F, v) => format!("IN {:02X} {:02X}", p, v),
                Transfer::Out(p @ 0x00..=0x6F, v) => format!("OUT {:02X} {:02X}", p, v),
                _ => continue,
            };
            match want.last_mut() {
                Some((last, n)) if *last == line => *n += 1,
                _ => want.push((line, 1)),
            }
        }
        let want: Vec<String> = want.into_iter().map(|(l, n)| if n > 1 { format!("{} ; x{}", l, n) } else { l }).collect();
        let got: Vec<String> = std::fs::read_to_string(&trace).unwrap().lines().map(String::from).collect();
        if let Some(i) = (0..want.len().max(got.len())).find(|&i| want.get(i) != got.get(i)) {
            panic!("{}: trace line {} is {:?}, the CPU's port sequence has {:?}", name, i + 1, got.get(i), want.get(i));
        }
    }
}

// ---------- pi8080d --sim (PI_DAEMON 16.5) ----------

/// A pi8080d process, killed if the test fails before it exits.
#[cfg(unix)]
struct Daemon(std::process::Child);

#[cfg(unix)]
impl Drop for Daemon {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

/// Kills the daemon, then fails with `msg` and everything it wrote to stderr: a board
/// violation panics the daemon, and the client sees only a closed console.
#[cfg(unix)]
fn daemon_failed(daemon: Daemon, log: std::thread::JoinHandle<String>, msg: String) -> ! {
    drop(daemon);
    panic!("{}\npi8080d stderr:\n{}", msg, log.join().unwrap_or_default());
}

#[cfg(unix)]
#[test]
fn sim_mode_plays_transcripts_over_tcp() {
    // The built binary with --sim and the shipped ROM, a fresh process per run, the test a
    // TCP console client as on the board (HARDWARE_BUILD 3 step 6). The 8080 boots as the
    // daemon starts, so the client may connect mid-banner or after it was discarded
    // (PI_DAEMON 16.3): it types H 0 0 and skips everything up to that command's prompt.
    // The last run is the RAM-image workflow (ARCHITECTURE 2.1): paste rom/monitor_ram.hex,
    // G D000, one guarded F; its expected output is the same steps on the local path.
    use std::io::{BufRead, BufReader};
    use std::process::{Command, Stdio};
    for name in ["hex_math", "storage", "assemble", "ram image"] {
        let steps: Vec<(String, Vec<u8>, Vec<Option<u8>>)> = if name == "ram image" {
            let (lines, _) = ram_image();
            let mut m = boot();
            let mut inputs: Vec<(String, Vec<u8>)> = lines.iter().enumerate()
                .map(|(i, l)| (format!("rom/monitor_ram.hex:{}", i + 1), format!("{}\r", l).into_bytes()))
                .collect();
            inputs.push(("G D000".to_string(), b"G D000\r".to_vec()));
            inputs.push(("F CFFF D000 00".to_string(), b"F CFFF D000 00\r".to_vec()));
            inputs.into_iter().map(|(at, input)| {
                let out = m.step(&input).unwrap_or_else(|e| panic!("{} on the local path: {}", at, e));
                (at, input, out.into_iter().map(Some).collect())
            }).collect()
        } else {
            transcript(name).into_iter()
                .map(|(line, input, expected)| (format!("tests/transcripts/{}.txt:{}", name, line), input, expected))
                .collect()
        };
        let dir = tempfile::tempdir().unwrap();
        let trace = dir.path().join("trace.txt");
        // No key, whatever the developer exported: cargo test never reaches the API (PI_DAEMON 13.2).
        let mut child = Command::new(env!("CARGO_BIN_EXE_pi8080d"))
            .env_remove("ANTHROPIC_API_KEY")
            .args(["--sim", "rom/monitor.bin", "--listen", "127.0.0.1:0", "--storage"])
            .arg(dir.path().join("storage"))
            .arg("--trace")
            .arg(&trace)
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        // Drained to the end: the daemon logs to stderr, and a closed pipe would fail it.
        let mut stderr = BufReader::new(child.stderr.take().unwrap());
        let daemon = Daemon(child);
        let mut line = String::new();
        stderr.read_line(&mut line).unwrap();
        let log = std::thread::spawn(move || {
            let mut rest = String::new();
            let _ = stderr.read_to_string(&mut rest);
            rest
        });
        let addr = line.strip_prefix("pi8080d: console on ").and_then(|l| l.split(',').next())
            .unwrap_or_else(|| panic!("startup line: {:?}", line));
        assert!(line.trim_end().ends_with(", ask off, board simulated, ROM rom/monitor.bin"), "{:?}", line);
        let mut client = TcpStream::connect(addr).unwrap();
        client.set_read_timeout(Some(Duration::from_secs(10))).unwrap();
        client.write_all(b"H 0 0\r").unwrap();
        let mut seen = Vec::new();
        while !seen.ends_with(b"H 0 0\r\n0000 0000\r\n> ") {
            let mut buf = [0u8; 256];
            match client.read(&mut buf) {
                Ok(n) if n > 0 => seen.extend_from_slice(&buf[..n]),
                r => daemon_failed(daemon, log, format!("{}: console {:?} after {:?}", name, r, show(&seen))),
            }
        }
        for (at, input, expected) in steps {
            let at = format!("{} over pi8080d --sim", at);
            client.write_all(&input).unwrap();
            let mut got = vec![0; expected.len() + 2];
            if let Err(e) = client.read_exact(&mut got) {
                daemon_failed(daemon, log, format!("{}: {}", at, e));
            }
            assert_eq!(show(&got[expected.len()..]), "> ", "{}: no prompt", at);
            check(&at, &got[..expected.len()], &expected);
        }
        // SAFETY: kill(2) on our own child's pid.
        assert_eq!(unsafe { libc::kill(daemon.0.id() as libc::pid_t, libc::SIGTERM) }, 0);
        let mut daemon = daemon;
        let status = daemon.0.wait().unwrap();
        if !status.success() {
            daemon_failed(daemon, log, format!("{}: {}", name, status));
        }
        let trace = std::fs::read_to_string(&trace).unwrap();
        assert_eq!(trace.lines().next(), Some("OUT 00 0D"), "{}: the banner's first byte", name);
        assert!(trace.lines().any(|l| l.starts_with("IN 01 ")), "{}: no console input in the trace", name);
    }
}

// ---------- RAM test build (ARCHITECTURE 2.1) ----------

const RAM_BASE: u16 = 0xD000;

/// rom/monitor_ram.hex: its lines, and the bytes its data records write from D000. Checks
/// the shape ARCHITECTURE 2.1 gives it: 16-byte records, contiguous from D000, one EOF last.
fn ram_image() -> (Vec<String>, Vec<u8>) {
    let text = std::fs::read_to_string("rom/monitor_ram.hex").unwrap();
    let lines: Vec<String> = text.lines().map(String::from).collect();
    let mut image = Vec::new();
    for (i, l) in lines.iter().enumerate() {
        let b: Vec<u8> = (1..l.len()).step_by(2).map(|j| u8::from_str_radix(&l[j..j + 2], 16).unwrap()).collect();
        let (len, addr, kind) = (b[0] as usize, u16::from_be_bytes([b[1], b[2]]), b[3]);
        if i + 1 == lines.len() {
            assert_eq!(kind, 1, "the last record is not EOF: {}", l);
            break;
        }
        assert!(kind == 0 && len <= 16 && addr == RAM_BASE + image.len() as u16, "record {}: {}", i + 1, l);
        image.extend_from_slice(&b[4..4 + len]);
    }
    (lines, image)
}

/// The `Built: MM/DD/YYYY` bytes of an image.
fn built_date(image: &[u8]) -> &[u8] {
    let at = image.windows(7).position(|w| w == b"Built: ").expect("no Built: in the image");
    &image[at..at + 17]
}

#[test]
fn ram_build_runs_the_transcripts() {
    // ARCHITECTURE 2.1: the resident ROM loads the RAM image through its HEX loader, G D000
    // starts it, a transcript runs on it, the image is intact afterwards, and G F000 returns
    // to the ROM. `hex` and `search` write D000-EEFF by design; ram/guard covers the RAM
    // build's guards.
    let (lines, image) = ram_image();
    let rom = std::fs::read("rom/monitor.bin").unwrap();
    // Both come from one `make`: a hex left from an earlier day is stale. One from the same
    // day is not caught.
    assert_eq!(show(built_date(&image)), show(built_date(&rom)), "rom/monitor_ram.hex is stale: cd rom && make");
    let paste: Vec<u8> = lines.iter().flat_map(|l| [l.as_bytes(), b"\r"].concat()).collect();
    let loaded = lines.iter().map(|l| format!("{}\r\n", l)).collect::<Vec<_>>().join("> ") + "Loaded\r\n";
    let mut names: Vec<String> = std::fs::read_dir("tests/transcripts").unwrap()
        .filter_map(|e| e.unwrap().file_name().into_string().ok()?.strip_suffix(".txt").map(String::from))
        .filter(|n| n != "hex" && n != "search")
        .collect();
    names.sort();
    assert!(names.len() >= 16, "{:?}", names);
    names.push("ram/guard".to_string());
    for name in names {
        let mut m = boot();
        assert_eq!(show(&m.step(&paste).unwrap()), show(loaded.as_bytes()), "{}: loading the RAM image", name);
        assert!(m.mem(RAM_BASE, image.len()) == image, "{}: the loaded image differs from the file", name);
        let banner = m.run("G D000");
        let first = banner.split("\\r\\n").nth(1).unwrap_or("");
        assert!(banner.starts_with("\\r\\n8080 Monitor v") && first.ends_with(" RAM"), "{}: {}", name, banner);
        m.play(&name);
        assert!(m.mem(RAM_BASE, image.len()) == image, "{} wrote the running RAM image", name);
        let banner = m.run("G F000");
        let first = banner.split("\\r\\n").nth(1).unwrap_or("");
        assert!(banner.starts_with("\\r\\n8080 Monitor v") && !first.ends_with(" RAM"), "{}: {}", name, banner);
    }
}

#[test]
fn dispatch() {
    boot().play("dispatch");
}

#[test]
fn line_input() {
    boot().play("line_input");
}

#[test]
fn help() {
    boot().play("help");
}

#[test]
fn registers() {
    boot().play("registers");
}

#[test]
fn breakpoint() {
    boot().play("breakpoint");
}

#[test]
fn regs_are_written_only_by_a_g_return() {
    // MONITOR_SPEC 6.20.1: cold start does not write REGS, so R first shows the harness's
    // junk RAM, and RAM survives RESET (ARCHITECTURE 3.1), so a capture outlives one.
    // Not a transcript: hardware RAM differs, and a transcript can't RESET.
    let mut m = boot();
    assert_eq!(m.run("R"), "A=76 F=76 BC=7676 DE=7676 HL=7676\\r\\n");
    m.run(":0F030000215644E5F1010D0B113412218100C982");
    m.run("G 0300");
    // The first command after a G return runs on WARM's stack, not in the workspace.
    assert_eq!(m.run("S 0300 030E F1"), "0304\\r\\n");
    m.cpu.reset();
    let banner = m.step(b"");
    let mut m = booted((m, banner));
    assert_eq!(m.run("R"), "A=44 F=56 BC=0B0D DE=1234 HL=0081\\r\\n");
}

#[test]
fn hex_math() {
    boot().play("hex_math");
}

#[test]
fn dump() {
    let mut m = boot();
    m.play("dump");
    // A line at FFF8 wraps to 0000-0007, and the next D starts at 0008 (MONITOR_SPEC 6.2).
    // Not a transcript: FFF8-FFFF is ROM padding.
    m.run("F 0000 0007 33");
    let line = m.run("D FFF8 FFFF");
    assert!(line.starts_with("FFF8: ") && line.ends_with("33333333\\r\\n"), "{}", line);
    assert_eq!(line.matches("\\r\\n").count(), 1, "{}", line);
    assert!(m.run("D").starts_with("0008: "));
}

#[test]
fn dump_counts_lines_to_ffff() {
    // MONITOR_SPEC 6.2 Termination: stop when the next line start carries past FFFF
    // or passes end. Not a transcript: thousands of lines of junk RAM and ROM.
    let mut m = boot();
    let lines = |out: String| -> Vec<String> { out.split("\\r\\n").filter(|l| !l.is_empty()).map(|l| l[..6].to_string()).collect() };
    let d = lines(m.run("D 0000 F000"));
    assert_eq!((d.len(), d[0].as_str(), d[0xF00].as_str()), (0xF01, "0000: ", "F000: "));
    let d = lines(m.run("D 0000 FFFF"));
    assert_eq!((d.len(), d[0xFFF].as_str()), (0x1000, "FFF0: "));
    assert_eq!(lines(m.run("D"))[0], "0000: ");
    // One argument caps the end at FFFF. The last line wraps, and D goes on from 0001.
    assert_eq!(lines(m.run("D FFE1")), ["FFE1: ", "FFF1: "]);
    assert_eq!(lines(m.run("D"))[0], "0001: ");
}

#[test]
fn examine() {
    let mut m = boot();
    m.play("examine");
    assert_eq!(m.mem(0x01FF, 9), [0x00, 0x05, 0x07, 0x00, 0x02, 0x00, 0x0A, 0x00, 0x00]);
}

#[test]
fn fill() {
    let mut m = boot();
    m.play("fill");
    assert_eq!(m.mem(0x01FF, 18), [&[0x00][..], &[0xAA; 16], &[0x00]].concat());
}

#[test]
fn move_block() {
    let mut m = boot();
    m.play("move");
    assert_eq!(m.mem(0x02FF, 18), [&[0x00][..], &[0x11; 8], &[0x22; 8], &[0x00]].concat());
}

#[test]
fn compare() {
    let mut m = boot();
    m.play("compare");
    // C 0000 FFFF compares 65536 bytes (MONITOR_SPEC 6.1): the last pair is FFFF (ROM
    // padding) against 0000 (00 after compare.txt). Not a transcript: the ROM and stack
    // page mismatches are thousands of lines, and the stack page ones are C's own stack use
    // (MONITOR_SPEC 6.1).
    let out = m.run("C 0000 FFFF 0001");
    assert!(out.ends_with("\\r\\nFFFF:FF 0000:00\\r\\n"), "{}", &out[out.len().saturating_sub(80)..]);
}

#[test]
fn search() {
    boot().play("search");
}

#[test]
fn io_ports() {
    boot().play("io");
}

#[test]
fn go() {
    boot().play("go");
}

#[test]
fn jp_we_fitted_lets_a_ram_program_write_the_rom() {
    // ARCHITECTURE 6.10: a program in RAM writes A5 to FFFE, waits at least tBLC (480 T),
    // toggle-polls I/O6 until two reads agree, and returns. With JP-WE fitted at the prompt,
    // for any tWC and both readings of the page-load window, the ROM holds the byte. With
    // JP-WE open (the default) nothing changes. Not a transcript: on the board it would
    // rewrite the ROM.
    // LXI H,FFFE / MVI M,A5 / MVI B,20 / DCR B / JNZ 0307 / MOV A,M / XRA M / ANI 40 / JNZ 030B / RET
    let prog = [0x21, 0xFE, 0xFF, 0x36, 0xA5, 0x06, 0x20, 0x05, 0xC2, 0x07, 0x03,
        0x7E, 0xAE, 0xE6, 0x40, 0xC2, 0x0B, 0x03, 0xC9];
    let line = |b: &str| format!("FFF0: FF FF FF FF FF FF FF FF  FF FF FF FF FF FF {} FF  ................\\r\\n", b);
    for twc in [1, 20_480, 65_535] {
        for cells in [false, true] {
            let mut m = boot();
            m.poke(0x0300, &prog);
            m.cpu.fit_jp_we(twc);
            m.cpu.set_load_window_cells(cells);
            assert_eq!(m.run("G 0300"), "", "tWC {} cells {}", twc, cells);
            assert_eq!(m.run("D FFF0 FFFF"), line("A5"), "tWC {} cells {}", twc, cells);
        }
    }
    let mut m = boot();
    m.poke(0x0300, &prog);
    assert_eq!(m.run("G 0300"), "");
    assert_eq!(m.run("D FFF0 FFFF"), line("FF"));
}

#[test]
fn jp_we_k1_probe() {
    // HARDWARE_BUILD 6, K-1: the record writes FFFF's own byte back, reads it twice at once
    // (inside tBLC), stores bit 6 of the XOR at 0380, then waits out the page load and
    // toggle-polls. 40: I/O6 toggled inside tBLC (the model's default reading). 00: it did
    // not (the literal reading), or JP-WE is open. The ROM never changes. The record is the
    // one in the doc, the one pasted on the board.
    let doc = std::fs::read_to_string("docs/HARDWARE_BUILD.md").unwrap();
    let record = doc.lines().map(str::trim).find(|l| l.starts_with(':') && l.get(3..7) == Some("0300")).expect("no K-1 record");
    let rom = std::fs::read("rom/monitor.bin").unwrap();
    for (fitted, cells, want) in [(true, false, 0x40), (true, true, 0x00), (false, false, 0x00)] {
        let mut m = boot();
        assert_eq!(m.run(record), "");
        if fitted {
            m.cpu.fit_jp_we(20_480);
            m.cpu.set_load_window_cells(cells);
        }
        assert_eq!(m.run("G 0300"), "");
        assert_eq!(m.mem(0x0380, 1), [want], "fitted {} cells {}", fitted, cells);
        assert!(m.mem(0xF000, 0x1000) == rom);
    }
}

#[test]
fn hex_loader() {
    boot().play("hex");
}

/// An Intel HEX type 00 record of `len` copies of `data` at `addr`, checksum correct.
fn hex_data_record(len: u8, addr: u16, data: u8) -> String {
    let mut bytes = vec![len, (addr >> 8) as u8, addr as u8, 0x00];
    bytes.extend(std::iter::repeat_n(data, len as usize));
    let sum = bytes.iter().fold(0u8, |s, b| s.wrapping_add(*b));
    bytes.push(sum.wrapping_neg());
    bytes.iter().fold(":".to_string(), |s, b| s + &format!("{:02X}", b))
}

#[test]
fn hex_guard_sweep() {
    // MONITOR_SPEC 7.2 step 6 for every high byte, at the low bytes and lengths where
    // the carry into the high byte changes: accepted iff AAAA >= 0100 and
    // AAAA + LL <= EF00 without wrap. An accepted record writes exactly its bytes.
    let mut m = boot();
    for hi in 0..=0xFFu16 {
        for lo in [0x00u16, 0x01, 0xDE, 0xDF, 0xFF] {
            for len in [0x01u8, 0x02, 0x21, 0x22] {
                let addr = hi << 8 | lo;
                let data = (hi as u8) ^ lo as u8 ^ len;
                let ok = addr >= 0x0100 && addr as u32 + len as u32 <= 0xEF00;
                let out = m.run(&hex_data_record(len, addr, data));
                assert_eq!(out, if ok { "" } else { "Address out of range\\r\\n" }, "{:04X} {:02X}", addr, len);
                if ok {
                    assert!(m.mem(addr, len as usize).iter().all(|&b| b == data), "{:04X} {:02X}", addr, len);
                }
            }
        }
    }
}

#[test]
fn storage() {
    let mut m = boot();
    m.play("storage");
    let disk = std::fs::read(m.dir.path().join("TEST.BIN")).unwrap();
    assert_eq!(disk.len(), 0x123457);
    assert_eq!(disk[..4], [0xA0, 0xA1, 0xA2, 0xA3]);
    assert_eq!(disk[4..0x100], [0x5A; 0xFC]);
    assert_eq!(disk[0x100..0x10002], vec![0x00; 0xFF02]);
    assert_eq!(disk[0x10002..0x10005], [0xA0, 0xA1, 0xA2]);
    assert_eq!(m.cpu.io_bus_mut().read(0x0C) & 0x01, 0, "X - left the file mounted");
    assert!(!m.dir.path().join("ATEST.BIN").exists(), "X kept a stale name character");
}

#[test]
fn mount_failed() {
    // Mount status 01 (open failed) prints Mount failed (MONITOR_SPEC 6.13). A directory
    // with the name can't be opened as a file (DEVICE_SPECS 7, Mount step 4).
    let mut m = boot();
    std::fs::create_dir(m.dir.path().join("SUB")).unwrap();
    assert_eq!(m.run("X SUB"), "Mount failed\\r\\n");
    assert_eq!(m.run("X"), "No storage mounted\\r\\n");
}

#[test]
fn load_and_write_port_sequences() {
    // MONITOR_SPEC 6.8, 6.12: status, then the address, the data, the flush (W), and
    // status again. The flush is not observable in the file; it is on the port.
    let mut m = boot();
    m.run("F 0200 0202 5A");
    assert_eq!(m.run("X T.BIN"), "Mounted\\r\\n");
    let storage = |m: &Mon, from: usize| -> Vec<Transfer> {
        m.ports[from..].iter().filter(|t| matches!(t, Transfer::In(0x08..=0x0C, _) | Transfer::Out(0x08..=0x0C, _))).copied().collect()
    };
    use Transfer::{In, Out};
    let n = m.ports.len();
    assert_eq!(m.run("W 0200 012345 3"), "Written\\r\\n");
    assert_eq!(storage(&m, n), [In(0x0C, 0x83), Out(0x08, 0x45), Out(0x09, 0x23), Out(0x0A, 0x01),
        Out(0x0B, 0x5A), Out(0x0B, 0x5A), Out(0x0B, 0x5A), Out(0x0C, 0x02), In(0x0C, 0x83)]);
    let n = m.ports.len();
    assert_eq!(m.run("L 012345 0300 3"), "Loaded\\r\\n");
    assert_eq!(storage(&m, n), [In(0x0C, 0x83), Out(0x08, 0x45), Out(0x09, 0x23), Out(0x0A, 0x01),
        In(0x0B, 0x5A), In(0x0B, 0x5A), In(0x0B, 0x5A), In(0x0C, 0x83)]);
    assert_eq!(m.mem(0x0300, 3), [0x5A; 3]);
}

#[test]
fn storage_error_when_the_file_goes_away_mid_transfer() {
    // L and W read status 0C after the transfer (W: after the flush) and print Storage
    // error when bit 0 is 0 (MONITOR_SPEC 6.8, 6.12). The device unmounts on a host I/O
    // error or a Pi service restart (DEVICE_SPECS 6); neither can be caused from here,
    // so the host unmounts it in the middle of the copy loop.
    for (line, at) in [("L 0 0200 10", "CL_LOOP"), ("W 0200 0 10", "CW_LOOP")] {
        let mut m = boot();
        assert_eq!(m.run("X T.BIN"), "Mounted\\r\\n");
        m.con().borrow_mut().push_input(format!("{}\r", line).as_bytes());
        m.run_to(sym(at));
        m.cpu.io_bus_mut().write(0x0E, 0x02);
        assert_eq!(show(&m.step(b"").unwrap()), "Storage error\\r\\n", "{}", line);
    }
}

#[test]
#[ignore]
fn w_command_cycles() {
    // PI_DAEMON 12.2 method 2: the emulator side of the whole-command overhead measurement.
    // Mean Pi overhead per access = (wall time at the TCP client - cycles x 488.28 ns) / accesses.
    let mut m = boot();
    assert_eq!(m.run("X CONF.BIN"), "Mounted\\r\\n");
    let (cycles, n) = (m.cpu.cycles, m.ports.len());
    assert_eq!(m.run("W F000 0 1000"), "Written\\r\\n");
    let accesses = m.ports[n..].iter().filter(|t| matches!(t, Transfer::In(0x00..=0x6F, _) | Transfer::Out(0x00..=0x6F, _))).count();
    println!("W F000 0 1000: {} cycles, {} Pi accesses", m.cpu.cycles - cycles, accesses);
}

// ---------- T: Time (MONITOR_SPEC 6.15, DEVICE_SPECS 8) ----------

#[test]
fn time() {
    boot().play("time");
}

impl Mon {
    /// Replace the mailbox at 10-13 (build_bus maps one with the host clock).
    fn map_mailbox(&mut self, device: Rc<RefCell<dyn IoDevice>>) {
        for port in 0x10..=0x13 {
            self.cpu.io_bus_mut().map_port(port, device.clone());
        }
    }
}

/// `NNNN-NN-NN NN:NN:NN`, N a decimal digit. 6.15: "T transcripts match the shape
/// NNNN-NN-NN NN:NN:NN (N = decimal digit), never a value".
fn is_time_shape(b: &[u8]) -> bool {
    b.len() == 19
        && b.iter().enumerate().all(|(i, &c)| match i {
            4 | 7 => c == b'-',
            10 => c == b' ',
            13 | 16 => c == b':',
            _ => c.is_ascii_digit(),
        })
}

#[test]
fn mailbox() {
    boot().play("mailbox");
}

#[test]
fn t_runs_the_reference_client() {
    // 6.15 step 1: "Runs the mailbox command TIME through the mailbox client (section 9;
    // DEVICE_SPECS.md, Service Mailbox, Reference client): clear (OUT 11h <- 02h), send T I
    // M E to OUT 10h, execute (OUT 11h <- 01h), then poll IN 12h."
    // Step 2: "Each response byte read from IN 13h is printed to the console as it arrives,
    // except that an LF (0Ah) prints as <CR><LF>. TIME's response has no LF, so T's output
    // does not change."
    // Step 3: "On status 03h (DONE), prints <CR><LF>."
    // "T sends exactly TIME." "T uses ports 10h-13h."
    // The clear comes first (DEVICE_SPECS 8, resync rule), so a half-sent command left in
    // the device by O 10 does not reach T.
    use Transfer::{In, Out};
    let mut m = boot();
    m.run("O 10 58");
    m.run("O 10 20");
    for line in ["T", "T UTC"] {
        let n = m.ports.len();
        let out = m.run(line);
        let seen: Vec<Transfer> = m.ports[n..].iter().filter(|t| !matches!(t, In(0x01, _) | In(0x02, _))).copied().collect();
        let resp: Vec<u8> = seen.iter().filter_map(|t| match t { In(0x13, b) => Some(*b), _ => None }).collect();
        assert!(is_time_shape(&resp), "{}: {:?}", line, show(&resp));
        let mut want: Vec<Transfer> = format!("{}\r\n", line).bytes().map(|b| Out(0x00, b)).collect();
        want.extend([Out(0x11, 0x02), Out(0x10, b'T'), Out(0x10, b'I'), Out(0x10, b'M'), Out(0x10, b'E'), Out(0x11, 0x01)]);
        for &b in &resp {
            want.extend([In(0x12, 0x02), In(0x13, b), Out(0x00, b)]);
        }
        want.extend([In(0x12, 0x03), Out(0x00, 0x0D), Out(0x00, 0x0A), Out(0x00, b'>'), Out(0x00, b' ')]);
        assert_eq!(seen, want, "{}", line);
        assert_eq!(out, format!("{}\\r\\n", show(&resp)), "{}", line);
    }
    // T leaves the mailbox DONE with nothing left to read (DEVICE_SPECS 8: "DONE and ERROR
    // persist until the next execute or clear"; IN 13 outside AVAIL reads 00).
    assert_eq!(m.run("I 12"), "03\\r\\n");
    assert_eq!(m.run("I 13"), "00\\r\\n");
    assert_eq!(m.run("I 12"), "03\\r\\n");
    // T works from ERROR too (execute from "any" state).
    m.run("O 11 01");
    assert_eq!(m.run("I 12"), "80\\r\\n");
    let out = m.run("T");
    assert!(is_time_shape(out.strip_suffix("\\r\\n").unwrap().as_bytes()), "{:?}", out);
}

/// A scripted mailbox at 10-13, standing in for anything a Pi could answer, including
/// BUSY, a failure mid-response and a restart, which TIME itself never produces.
/// IN 12 reads 00 until an execute. After it, each IN 12 returns the next of `statuses`
/// (the last one repeats), and IN 13 pops `bytes` while the last status read was 02.
struct ScriptedMailbox {
    statuses: std::collections::VecDeque<u8>,
    bytes: std::collections::VecDeque<u8>,
    status: u8,
    executed: bool,
}

impl IoDevice for ScriptedMailbox {
    fn read(&mut self, port: u8) -> u8 {
        match port {
            0x12 if self.executed => {
                if let Some(s) = self.statuses.pop_front() {
                    self.status = s;
                }
                self.status
            }
            0x12 => 0x00,
            0x13 if self.executed && self.status == 0x02 => self.bytes.pop_front().unwrap_or(0x00),
            0x13 => 0x00,
            _ => 0xFF,
        }
    }
    fn write(&mut self, port: u8, value: u8) {
        if port == 0x11 && value == 0x01 {
            self.executed = true;
        }
    }
}

/// Boot and put a ScriptedMailbox at 10-13.
fn scripted(statuses: &[u8], bytes: &[u8]) -> Mon {
    let mut m = boot();
    let stub = Rc::new(RefCell::new(ScriptedMailbox {
        statuses: statuses.iter().copied().collect(),
        bytes: bytes.iter().copied().collect(),
        status: 0x00,
        executed: false,
    }));
    m.map_mailbox(stub);
    m
}

#[test]
fn t_prints_service_error() {
    // 6.15 step 4: "On status 00h after execute (Pi service restarted) or 80h-FFh, prints
    // Service error then <CR><LF>. Any response bytes already printed stay on the same
    // line, with no <CR><LF> before the message: an error after 2026- prints 2026-Service error."
    // Messages (5): "Service error | T, U, N: mailbox status 00 after execute, or 80-FF".
    for status in [0x00, 0x80, 0x81, 0x82, 0x83, 0x84, 0xC0, 0xFF] {
        assert_eq!(scripted(&[status], b"").run("T"), "Service error\\r\\n", "status {:02X}", status);
    }
    // A failure mid-response (DEVICE_SPECS 8: "The request fails (including mid-response)
    // | BUSY, AVAIL | ERROR"): the bytes printed so far stay, then the message.
    assert_eq!(scripted(&[0x02, 0x02, 0x02, 0x02, 0x02, 0x83], b"2026-").run("T"), "2026-Service error\\r\\n");
    // A Pi restart mid-response reads 00 (DEVICE_SPECS 2.9).
    assert_eq!(scripted(&[0x02, 0x00], b"2").run("T"), "2Service error\\r\\n");
}

#[test]
fn t_handles_every_status_the_reference_client_does() {
    // DEVICE_SPECS 8 reference client, MB_GET: 01 polls again; "byte" (02: IN 13, T prints
    // it), "done" (03: T prints CR LF), "failed" (00 or 80-FF: Service error).
    // "BUSY can follow AVAIL in the middle of a response ... One polling loop handles every
    // case." "A response may be empty." "Response bytes can be any value 00-FF. The end is
    // marked by status, not by a terminator."
    assert_eq!(scripted(&[0x01, 0x01, 0x01, 0x02, 0x01, 0x01, 0x02, 0x01, 0x03], b"AB").run("T"), "AB\\r\\n");
    assert_eq!(scripted(&[0x03], b"").run("T"), "\\r\\n");
    assert_eq!(scripted(&[0x02, 0x02, 0x02, 0x02, 0x03], &[0x00, 0x0D, 0xFF, 0x7F]).run("T"), "\\x00\\r\\xFF\\x7F\\r\\n");
}

#[test]
fn t_with_the_pi_clock_not_set_prints_service_error() {
    // DEVICE_SPECS 8, TIME: "If the clock is not set, the result is 83." 6.15 step 4:
    // 80h-FFh prints Service error.
    let mut m = boot();
    let mb = Rc::new(RefCell::new(Mailbox::new(|| None, m.dir.path().to_path_buf(), AskConfig::default())));
    m.map_mailbox(mb);
    assert_eq!(m.run("T"), "Service error\\r\\n");
    assert_eq!(m.run("I 12"), "83\\r\\n");
}

// ---------- N (MONITOR_SPEC 6.18, 6.18.1) ----------
//
// Server rows use the real Mailbox from build_bus and the test HTTP server H
// (tests/support/http.rs), its address typed into the line, on a wall-clock deadline.
// The /hello body is `Hello` CR LF (the DEVICE_SPECS 8 vectors), so N prints `Hello`, the
// body's CR, the body's LF as CR LF (step 2), then CR LF on DONE (step 3).

/// Boot for server rows: steps on NET_DEADLINE.
fn net() -> Mon {
    let mut m = boot();
    m.net = true;
    m
}

const HELLO: &str = "Hello\\r\\r\\n\\r\\n";

#[test]
fn n_runs_the_mailbox_client() {
    // 6.18.1 ports row: "OUT 11 02; OUT 10 GET then  http://H/hello (the space after N
    // included); OUT 11 01; then IN 12 / IN 13 pairs reading Hello, an IN 12 that may
    // read 01 before any of them, and a final IN 12 03". Step 1: "N parses nothing".
    use Transfer::{In, Out};
    let h = http::start();
    let mut m = net();
    let n = m.ports.len();
    assert_eq!(m.run(&format!("N {}", h.url("/hello"))), HELLO);
    let seen: Vec<Transfer> = m.mailbox_ports(n).into_iter().filter(|&t| t != In(0x12, 0x01)).collect();
    let mut want = vec![Out(0x11, 0x02)];
    want.extend(format!("GET  {}", h.url("/hello")).bytes().map(|b| Out(0x10, b)));
    want.push(Out(0x11, 0x01));
    for &b in b"Hello\r\n" {
        want.extend([In(0x12, 0x02), In(0x13, b)]);
    }
    want.push(In(0x12, 0x03));
    assert_eq!(seen, want);
}

#[test]
fn n_prints_the_body() {
    // 6.18.1 server rows. Step 2: "an LF prints as <CR><LF>, every other byte unchanged."
    // "The space after N is optional (section 3). n works."
    let h = http::start();
    let mut m = net();
    assert_eq!(m.run(&format!("N {}", h.url("/lf"))), "a\\r\\nb\\r\\n\\r\\n");
    assert_eq!(m.run(&format!("N{}", h.url("/hello"))), HELLO);
    assert_eq!(m.run(&format!("n {}", h.url("/hello"))), HELLO);
    assert_eq!(m.run(&format!("N {}", h.url("/404"))), "Service error\\r\\n");
}

#[test]
fn n_to_a_file_then_x_and_l() {
    // 6.18.1: "N http://H/hello > book.txt, then X BOOK.TXT, L 0 0200 7, D 0200 0206 |
    // 000007, Mounted, Loaded, the dump shows Hello.. | BOOK.TXT = Hello 0D 0A".
    let h = http::start();
    let mut m = net();
    assert_eq!(m.run(&format!("N {} > book.txt", h.url("/hello"))), "000007\\r\\n");
    assert_eq!(std::fs::read(m.dir.path().join("BOOK.TXT")).unwrap(), b"Hello\r\n");
    assert_eq!(m.run("X BOOK.TXT"), "Mounted\\r\\n");
    assert_eq!(m.run("L 0 0200 7"), "Loaded\\r\\n");
    let dump = m.run("D 0200 0206");
    // D prints the whole 16-byte line; the bytes after 0206 are junk RAM.
    assert!(dump.starts_with("0200: 48 65 6C 6C 6F 0D 0A") && dump.contains("  Hello.."), "{}", dump);
}

#[test]
fn n_bad_arguments_print_service_error() {
    // 6.18.1: "N, N ftp://x, N http://x > .. | Service error (each) | mailbox ports
    // written; nothing in the storage directory". Step 4: "This covers 82".
    let mut m = boot();
    for line in ["N", "N ftp://x", "N http://x > .."] {
        let n = m.ports.len();
        assert_eq!(m.run(line), "Service error\\r\\n", "{}", line);
        assert!(m.mailbox_ports(n).contains(&Transfer::Out(0x11, 0x01)), "{}: no execute", line);
        assert_eq!(m.run("I 12"), "82\\r\\n", "{}", line);
    }
    assert_eq!(std::fs::read_dir(m.dir.path()).unwrap().count(), 0);
}

#[test]
fn n_handles_every_status_the_reference_client_does() {
    // 6.18.1 scripted rows.
    assert_eq!(scripted(&[0x01, 0x01, 0x02, 0x01, 0x02, 0x03], b"AB").run("N x"), "AB\\r\\n");
    assert_eq!(scripted(&[0x01, 0x01, 0x83], b"").run("N x"), "Service error\\r\\n");
    assert_eq!(scripted(&[0x00], b"").run("N x"), "Service error\\r\\n");
    assert_eq!(scripted(&[0x02, 0x02, 0x01, 0x83], b"ab").run("N x"), "abService error\\r\\n");
    assert_eq!(scripted(&[0x02, 0x03], b"\n").run("N x"), "\\r\\n\\r\\n");
}

// ---------- Q (MONITOR_SPEC 6.19, 6.19.1) ----------
//
// Server rows map a real Mailbox with a test key and the scripted side of H as its
// endpoint (DEVICE_SPECS 8, ASK vectors); nothing here reaches the API.

/// Boot for Q server rows: a mailbox with the test key and H's endpoint.
fn ask_net(h: &http::Scripted) -> Mon {
    let mut m = net();
    let ask = AskConfig { key: Some("sk-ant-test".to_string()), url: h.url("/v1/messages"), total_secs: 120 };
    let mb = Rc::new(RefCell::new(Mailbox::new(mailbox::local_time, m.dir.path().to_path_buf(), ask)));
    m.map_mailbox(mb);
    m
}

#[test]
fn q_runs_the_mailbox_client() {
    // 6.19.1 ports row: "Q hi, reply Hello LF world | Hello 0D 0D 0A world 0D 0A | OUT 11 02;
    // OUT 10 ASK then  hi; OUT 11 01; IN 12 01 any number of times; IN 12 02 / IN 13 pairs;
    // IN 12 03". Step 1: "Q checks nothing; the device trims the spaces."
    use Transfer::{In, Out};
    let h = http::scripted(vec![http::sse(&["Hello
world"], "end_turn")]);
    let mut m = ask_net(&h);
    let n = m.ports.len();
    assert_eq!(m.run("Q hi"), "Hello\\r\\r\\nworld\\r\\n");
    let seen: Vec<Transfer> = m.mailbox_ports(n).into_iter().filter(|&t| t != In(0x12, 0x01)).collect();
    let mut want = vec![Out(0x11, 0x02)];
    want.extend(b"ASK  hi".iter().map(|&b| Out(0x10, b)));
    want.push(Out(0x11, 0x01));
    for &b in b"Hello\r\nworld" {
        want.extend([In(0x12, 0x02), In(0x13, b)]);
    }
    want.push(In(0x12, 0x03));
    assert_eq!(seen, want);
    let body: serde_json::Value = serde_json::from_slice(&h.request().body).unwrap();
    assert_eq!(body["messages"][0]["content"], "hi");
}

#[test]
fn q_failure_after_part_of_a_reply() {
    // 6.19.1: "Q hi, reply Hel, then the connection closes | HelService error". Step 4:
    // "Bytes already printed stay on their line, with no <CR><LF> before the message".
    let mut script = http::sse_start();
    script.push(http::text("Hel"));
    let h = http::scripted(vec![script]);
    let mut m = ask_net(&h);
    assert_eq!(m.run("Q hi"), "HelService error\\r\\n");
    assert_eq!(m.run("I 12"), "83\\r\\n");
}

#[test]
fn q_without_a_question() {
    // 6.19.1: "Q, Q   , q (ask.txt) | Service error (each) | mailbox ports written; no
    // request". Here with a key and a server, so "no request" is checked too.
    let h = http::scripted(vec![http::sse(&["no"], "end_turn")]);
    let mut m = ask_net(&h);
    for line in ["Q", "Q   ", "q"] {
        let n = m.ports.len();
        assert_eq!(m.run(line), "Service error\\r\\n", "{:?}", line);
        assert!(m.mailbox_ports(n).contains(&Transfer::Out(0x11, 0x01)), "{:?}: no execute", line);
    }
    assert!(h.no_request());
    boot().play("ask");
}

// ---------- Esc (MONITOR_SPEC 6.18, 6.19) ----------
//
// ScriptedMailbox ignores the clear, so scripted rows assert the OUT 11 02 port event;
// the real-mailbox row reads the IDLE state after it.

/// One step of `input` on a scripted mailbox: the output, the monitor, and where the
/// step's ports start.
fn esc_step(statuses: &[u8], bytes: &[u8], input: &[u8]) -> (String, Mon, usize) {
    let mut m = scripted(statuses, bytes);
    let n = m.ports.len();
    let out = show(&m.step(input).unwrap_or_else(|e| panic!("{}", e)));
    (out, m, n)
}

#[test]
fn esc_aborts_n_and_q_scripted() {
    use Transfer::{In, Out};
    // 6.18 Esc: checked on each busy pass; OUT 11h <- 02h (clear), then Aborted.
    let (out, m, n) = esc_step(&[0x01], b"", b"N x\r\x1B");
    assert_eq!(out, "N x\\r\\nAborted\\r\\n");
    let mb = m.mailbox_ports(n);
    let exec = mb.iter().position(|&t| t == Out(0x11, 0x01)).unwrap();
    let after = &mb[exec + 1..];
    assert_eq!(after.last(), Some(&Out(0x11, 0x02)), "{:?}", after);
    assert!(after[..after.len() - 1].iter().all(|&t| t == In(0x12, 0x01)), "{:?}", after);
    // An Esc anywhere among the waiting bytes aborts, and every byte waiting with it is
    // discarded: behind a CR LF terminal's LF or type-ahead, and ahead of an arrow key's
    // rest or a typed command. One prompt, nothing runs, the FIFO empty.
    for input in [&b"N x\r\n\x1B"[..], b"N x\rq\x1B", b"N x\r\x1B[A", b"N x\r\x1BH 1 1\r"] {
        let (out, m, _) = esc_step(&[0x01], b"", input);
        assert_eq!(out, "N x\\r\\nAborted\\r\\n", "{:?}", show(input));
        assert!(!m.con().borrow().has_input(), "{:?}", show(input));
    }
    // Nothing typed: a check reads status first and pops no byte (4 IN 01: the line).
    let (out, m, n) = esc_step(&[0x01, 0x01, 0x01, 0x02, 0x03], b"A", b"N x\r");
    assert_eq!(out, "N x\\r\\nA\\r\\n");
    assert_eq!(m.ports[n..].iter().filter(|t| matches!(t, In(0x01, _))).count(), 4);
    // Checked at each LF, after it prints (as CR LF): the message starts a line. The endless
    // 00 after the body has no LF.
    let (out, _, _) = esc_step(&[0x02], b"ab\ncd\n", b"N x\r\x1B");
    assert_eq!(out, "N x\\r\\nab\\r\\nAborted\\r\\n");
    // A CR LF body (every ASK reply): the body's CR is out before the LF check, and the
    // LF still prints, so the message does not overprint the line.
    let (out, _, _) = esc_step(&[0x02], b"Hello world\r\nsecond\r\n", b"Q x\r\x1B");
    assert_eq!(out, "Q x\\r\\nHello world\\r\\r\\nAborted\\r\\n");
    // Every other waiting byte is discarded, all of it (the drain): nothing runs after N,
    // one prompt, the FIFO empty.
    let body = b"a\nb\nc\n";
    let st = [0x02, 0x02, 0x02, 0x02, 0x02, 0x02, 0x03];
    for input in [&b"N x\rq"[..], b"N x\rH 1 1\r"] {
        let (out, m, _) = esc_step(&st, body, input);
        assert_eq!(out, "N x\\r\\na\\r\\nb\\r\\nc\\r\\n\\r\\n", "{:?}", show(input));
        assert!(!m.con().borrow().has_input(), "{:?}", show(input));
    }
    // A CR LF terminal: the busy check eats the LF, so no second prompt.
    let (out, _, _) = esc_step(&[0x01, 0x02, 0x02, 0x02, 0x02, 0x02, 0x03], b"HELLO", b"N x\r\n");
    assert_eq!(out, "N x\\r\\nHELLO\\r\\n");
    // 6.15: T never checks (TIME is never busy and has no LF). The Esc reaches the prompt,
    // which ignores it.
    let (out, _, _) = esc_step(&[0x02, 0x02, 0x02, 0x03], b"12:", b"T\r\x1B");
    assert_eq!(out, "T\\r\\n12:\\r\\n");
}

#[test]
fn esc_with_the_real_mailbox() {
    // Bulk replay (PI_DAEMON 7.1): T and a Q that fails at execute read no keyboard, so
    // all four lines of one step run.
    let mut m = boot();
    let out = show(&m.step(b"T\rI 12\rQ\rI 12\r").unwrap());
    let rest = out.strip_prefix("T\\r\\n").unwrap_or_else(|| panic!("{}", out));
    assert!(is_time_shape(&rest.as_bytes()[..19]), "{}", out);
    assert_eq!(&rest[19..], "\\r\\n> I 12\\r\\n03\\r\\n> Q\\r\\nService error\\r\\n> I 12\\r\\n82\\r\\n");
    // Esc while /hang is busy: Aborted, and the clear left the mailbox IDLE (DEVICE_SPECS 8).
    let h = http::start();
    let mut m = net();
    let url = h.url("/hang");
    let out = show(&m.step(format!("N {}\r\x1B", url).as_bytes()).unwrap());
    assert_eq!(out, format!("N {}\\r\\nAborted\\r\\n", url));
    assert_eq!(m.run("I 12"), "00\\r\\n");
}

// ---------- A and U (MONITOR_SPEC 6.16, 6.17, 6.17.1) ----------

#[test]
fn assemble() {
    boot().play("assemble");
}

#[test]
fn unassemble() {
    boot().play("unassemble");
}

impl Mon {
    /// Type `input` exactly (an A dialog); return everything printed, escaped.
    fn dialog(&mut self, input: &str) -> String {
        show(&self.step(input.as_bytes()).unwrap_or_else(|e| panic!("{}", e)))
    }

    /// Write RAM directly (load_program would move PC).
    fn poke(&mut self, addr: u16, bytes: &[u8]) {
        for (i, &b) in bytes.iter().enumerate() {
            self.cpu.write_byte(addr.wrapping_add(i as u16), b);
        }
    }

    /// The mailbox port accesses (10-13) from `from` on.
    fn mailbox_ports(&self, from: usize) -> Vec<Transfer> {
        self.ports[from..].iter().filter(|t| matches!(t, Transfer::In(0x10..=0x13, _) | Transfer::Out(0x10..=0x13, _))).copied().collect()
    }
}

#[test]
fn a_runs_the_mailbox_client() {
    // 6.17.1 ports row: "A 0200, MVI A,0D, . | prompts 0200: , 0202: | OUT 11 02; OUT 10 41
    // 53 4D 20 4D 56 49 20 41 2C 30 44 (ASM MVI A,0D); OUT 11 01; IN 12 02, IN 13 3E, IN 12
    // 02, IN 13 0D, IN 12 03".
    use Transfer::{In, Out};
    let mut m = boot();
    let n = m.ports.len();
    assert_eq!(m.dialog("A 0200\rMVI A,0D\r.\r"), "A 0200\\r\\n0200: MVI A,0D\\r\\n0202: .\\r\\n");
    let mut want = vec![Out(0x11, 0x02)];
    want.extend(b"ASM MVI A,0D".iter().map(|&b| Out(0x10, b)));
    want.extend([Out(0x11, 0x01), In(0x12, 0x02), In(0x13, 0x3E), In(0x12, 0x02), In(0x13, 0x0D), In(0x12, 0x03)]);
    assert_eq!(m.mailbox_ports(n), want);
    assert_eq!(m.mem(0x0200, 2), [0x3E, 0x0D]);
    // 6.16 step 5: <text> runs from the first non-space character to the end of the stored
    // line, trailing spaces included.
    let n = m.ports.len();
    assert_eq!(m.dialog("A 0200\r  MVI A,0D  \r.\r"), "A 0200\\r\\n0200:   MVI A,0D  \\r\\n0202: .\\r\\n");
    let mut want = vec![Out(0x11, 0x02)];
    want.extend(b"ASM MVI A,0D  ".iter().map(|&b| Out(0x10, b)));
    want.extend([Out(0x11, 0x01), In(0x12, 0x02), In(0x13, 0x3E), In(0x12, 0x02), In(0x13, 0x0D), In(0x12, 0x03)]);
    assert_eq!(m.mailbox_ports(n), want);
    // 6.16 step 4 and "A uses ports 10h-13h, and only after a line other than an empty one
    // or . is entered": empty lines, lines of spaces (CR, LF and CR LF ends) and '.' send nothing.
    let n = m.ports.len();
    m.dialog("A 0200\r\r  \n\r\n \r\n . x\r");
    assert_eq!(m.mailbox_ports(n), []);
    assert_eq!(m.mem(0x0200, 2), [0x3E, 0x0D]);
}

#[test]
fn a_failures_prompt_the_same_address_again() {
    // 6.16 steps 7-8 and the 6.17.1 scripted rows. JUNK at 0200 shows "unchanged".
    for (status, msg) in [(0x83, "Service error"), (0x00, "Service error"), (0x80, "Service error"),
        (0x81, "Service error"), (0xFF, "Service error"), (0x82, "Invalid instruction")] {
        let mut m = scripted(&[status], b"");
        assert_eq!(m.dialog("A 0200\rNOP\r.\r"),
            format!("A 0200\\r\\n0200: NOP\\r\\n{}\\r\\n0200: .\\r\\n", msg), "{:02X}", status);
        assert_eq!(m.mem(0x0200, 1), [JUNK], "{:02X}", status);
    }
    // A restart after one byte, then a retry: the retry rewrites the line from its start.
    let mut m = scripted(&[0x02, 0x00, 0x02, 0x02, 0x02, 0x03], &[0x21, 0x21, 0x34, 0x12]);
    assert_eq!(m.dialog("A 0200\rLXI H,1234\rLXI H,1234\r.\r"),
        "A 0200\\r\\n0200: LXI H,1234\\r\\nService error\\r\\n0200: LXI H,1234\\r\\n0203: .\\r\\n");
    assert_eq!(m.mem(0x0200, 4), [0x21, 0x34, 0x12, JUNK]);
    // BUSY between bytes (MB_GET polls again).
    let mut m = scripted(&[0x01, 0x02, 0x01, 0x01, 0x02, 0x01, 0x03], &[0x3E, 0x0D]);
    assert_eq!(m.dialog("A 0200\rMVI A,0D\r.\r"), "A 0200\\r\\n0200: MVI A,0D\\r\\n0202: .\\r\\n");
    assert_eq!(m.mem(0x0200, 3), [0x3E, 0x0D, JUNK]);
}

#[test]
fn u_runs_the_mailbox_client() {
    // 6.17.1 ports row: "U 0200 1 with 0200 = 3E 0D 21 | 0200  3E 0D     MVI A,0D | OUT 11
    // 02; OUT 10 DIS 0200 3E 0D 21 (17 bytes); OUT 11 01; then IN 12 / IN 13 pairs: 02 (the
    // length), then the 24 line bytes and 0D 0A; then IN 12 03".
    use Transfer::{In, Out};
    let mut m = boot();
    m.poke(0x0200, &[0x3E, 0x0D, 0x21]);
    let n = m.ports.len();
    assert_eq!(m.run("U 0200 1"), "0200  3E 0D     MVI A,0D\\r\\n");
    let mut want = vec![Out(0x11, 0x02)];
    want.extend(b"DIS 0200 3E 0D 21".iter().map(|&b| Out(0x10, b)));
    want.push(Out(0x11, 0x01));
    for &b in [&[0x02][..], b"0200  3E 0D     MVI A,0D\r\n"].concat().iter() {
        want.extend([In(0x12, 0x02), In(0x13, b)]);
    }
    want.push(In(0x12, 0x03));
    assert_eq!(m.mailbox_ports(n), want);
}

#[test]
fn u_prints_service_error() {
    // 6.17 step 5 and the 6.17.1 scripted rows: "On status 00 at any point after execute
    // (the Pi restarted), on 80h-FFh, or on DONE before the length byte, print Service
    // error and end U. Bytes of the current line already printed stay on that line".
    let line = [0x02, 0x30, 0x32, 0x30, 0x30, 0x20];
    let cases: [(&[u8], &[u8], &str, &str); 8] = [
        (&[0x83], &[], "U 0200 2", "Service error"),
        (&[0x00], &[], "U 0200 2", "Service error"),
        (&[0x82], &[], "U 0200 2", "Service error"),
        (&[0x02, 0x02, 0x02, 0x02, 0x02, 0x02, 0x83], &line, "U 0200 2", "0200 Service error"),
        (&[0x02, 0x02, 0x02, 0x02, 0x02, 0x02, 0x00], &line, "U 0200 2", "0200 Service error"),
        (&[0x03], &[], "U 0200 1", "Service error"),
        // A failure on the last line still prints the message.
        (&[0x02, 0x02, 0x83], &[0x01, b'X'], "U 0200 1", "XService error"),
        // A failure on the second line: the first line stands.
        (&[0x02, 0x02, 0x02, 0x03, 0x02, 0x00], &[0x01, b'A', b'\n', 0x01], "U 0200 3", "A\\nService error"),
    ];
    for (statuses, bytes, cmd, want) in cases {
        assert_eq!(scripted(statuses, bytes).run(cmd), format!("{}\\r\\n", want), "{:?}", statuses);
    }
}

#[test]
fn u_counts_instructions_past_ff() {
    // 4.3: "Count for U is a number of instructions, 0001 to FFFF." Not a transcript: 256 lines.
    let mut m = boot();
    m.run("F 0200 03FF 00");
    let out = m.run("U 0200 100");
    let lines: Vec<&str> = out.split("\\r\\n").filter(|l| !l.is_empty()).collect();
    assert_eq!((lines.len(), lines[0], lines[255]), (256, "0200  00        NOP", "02FF  00        NOP"));
}

#[test]
fn u_lines_are_the_debugger_lines() {
    // 6.17.1 Identity: "for every opcode xx, with 0200-0202 = xx 01 02, U 0200 1 | the output
    // string (.1) of Debugger::new().command(&mut m.cpu, "u 0200 1"), no symbols loaded, with
    // its LF replaced by CR LF".
    let mut m = boot();
    for xx in 0..=255u8 {
        m.poke(0x0200, &[xx, 0x01, 0x02]);
        let want = Debugger::new().command(&mut m.cpu, "u 0200 1").1.replace('\n', "\r\n");
        assert_eq!(m.run("U 0200 1"), show(want.as_bytes()), "{:02X}", xx);
    }
}

#[test]
fn u_text_typed_into_a_gives_the_bytes_back() {
    // 6.17.1 Round trip: "for every opcode xx, U 0200 1, then A 0300 with U's text field, . |
    // 0300.. = 0200.. for the instruction's length, except the R2 aliases". R2: NOP* is 08,
    // CALL* is DD.
    let mut m = boot();
    for xx in 0..=255u8 {
        m.poke(0x0200, &[xx, 0x01, 0x02]);
        let out = m.run("U 0200 1");
        let text = out[16..].strip_suffix("\\r\\n").unwrap();
        let len = (out[6..14].trim().len() + 1) / 3;
        m.poke(0x0300, &[JUNK; 3]);
        let next = format!("{:04X}", 0x0300 + len);
        assert_eq!(m.dialog(&format!("A 0300\r{}\r.\r", text)),
            format!("A 0300\\r\\n0300: {}\\r\\n{}: .\\r\\n", text, next), "{:02X}", xx);
        let first = match xx {
            0x10 | 0x18 | 0x20 | 0x28 | 0x30 | 0x38 => 0x08,
            0xED | 0xFD => 0xDD,
            _ => xx,
        };
        assert_eq!(m.mem(0x0300, len), [first, 0x01, 0x02][..len], "{:02X} {}", xx, text);
    }
}

// ---------- G entry (not a transcript: the program is a HLT) ----------

#[test]
fn go_entry_contract() {
    // MONITOR_SPEC 8: on entry SP = EFFE, the word there is G_RETURN, interrupts are off and
    // the overlay is off. The return through it is in go.txt. The error before G leaves
    // two pushes and a return address behind: WARM must reset SP. 8.1: every G, bare G
    // too, has written JMP BRK_ENTRY at 0030 by then.
    let brk = sym("BRK_ENTRY");
    for (line, at) in [("G 0300", 0x0300u16), ("G", 0x0100)] {
        let mut m = boot();
        m.run(&format!("F {:04X} {:04X} 76", at, at));
        assert_eq!(m.run("C 0200 0100 0300"), "Invalid range\\r\\n");
        m.con().borrow_mut().push_input(format!("{}\r", line).as_bytes());
        let start = m.cpu.cycles;
        while !m.cpu.halted && m.cpu.cycles - start < BUDGET {
            m.cpu.execute_one();
        }
        assert_eq!(m.cpu.pc, at + 1, "{}: halted at the wrong place", line);
        assert_eq!((m.cpu.sp, m.cpu.read_word(0xEFFE)), (0xEFFE, sym("G_RETURN")), "{}", line);
        assert!(!m.cpu.interrupts_enabled && !m.cpu.rom_overlay_enabled);
        assert_eq!(m.mem(0x0030, 3), [0xC3, brk as u8, (brk >> 8) as u8], "{}", line);
    }
}

#[test]
fn ret_and_break_leave_interrupts_enabled() {
    // MONITOR_SPEC 8 and 8.1: neither G_RETURN nor BRK_ENTRY executes DI, and RST 6 does not
    // clear INTE, so a program that ran EI is back at the prompt with INTE still set.
    for (prog, out) in [([0xFBu8, 0xC9], "G 0300\\r\\n"), ([0xFB, 0xF7], "G 0300\\r\\nBRK 0301\\r\\n")] {
        let mut m = boot();
        m.poke(0x0300, &prog);
        assert_eq!(show(&m.step(b"G 0300\r").unwrap()), out);
        assert!(m.cpu.interrupts_enabled, "{}", out);
    }
}

// ---------- Argument errors, under the debugger ----------

#[test]
fn argument_errors_write_no_port_and_no_memory_outside_the_workspace() {
    // MONITOR_SPEC 4.4 rule 4. The debugger stops on any write to 0000-007F, 0100-EEFF or
    // F000-FFFF and on an OUT to any port but the console; each line must instead reach
    // WARM having printed one argument error. The stack page is the monitor's.
    let lines = [
        "C", "C 0200", "C 0200 0210 ZZ", "C 0210 0200 0300", "C 10200 0210 0300",
        "D ZZ", "D 0300 0200", "D 0200 02G0",
        "E ZZ", "E 10200",
        "F 0200 0300", "F 0300 0200 AA", "F 0300 0200 ZZ", "F 0200 0300 100", "F 10200 1020F 1AA", "F 0200 02G0 AA",
        "G ZZ", "G 01ZZ", "G 10100",
        "H 1", "H 10000 1",
        "I", "I 100",
        "M 0200 0300", "M 0200 0300 0", "M 0200 0300 ZZ",
        "O 08", "O 08 100", "O 100 00", "O 0D 4Z",
        "S 0200 0210", "S 0200 0210 AA ZZ", "S 0300 0200 41", "S 0200 0210 100",
        "L 0", "L 0 0200 0", "L 0 0200 ZZ", "L 1234567 0200", "L 0 10200",
        "W 0200", "W 0200 0 0", "W 0200 0 ZZ", "W 0200 1000000",
        "A", "A 02G0", "A 10000",
        "U", "U 02G0", "U 0200 ZZ", "U 0200 10000", "U 0200 0",
    ];
    let errors = ["Invalid address", "Invalid hex value", "Invalid port/value", "Invalid range"];
    let mut m = boot();
    assert_eq!(m.run("X T.BIN"), "Mounted\\r\\n");
    let mut dbg = Debugger::new();
    dbg.load_symbols(&std::fs::read_to_string("rom/monitor.sym").unwrap()).unwrap();
    let watches = ["b WARM", "w 0000-007F w", "w 0100-EEFF w", "w F000-FFFF w"].map(String::from);
    for cmd in watches.into_iter().chain((0x01..=0xFF).map(|p| format!("io {:02X} out", p))) {
        assert_eq!(dbg.command(&mut m.cpu, &cmd).1, "", "{}", cmd);
    }
    for line in lines {
        m.con().borrow_mut().take_output();
        m.con().borrow_mut().push_input(format!("{}\r", line).as_bytes());
        dbg.command(&mut m.cpu, "c");
        let stop = dbg.run(&mut m.cpu, 1_000_000).unwrap_or_else(|| panic!("{}: no stop, PC={:04X}", line, m.cpu.pc));
        assert_eq!(stop, format!("break {:04X} WARM", sym("WARM")), "{}\n{}", line, dbg.report(&m.cpu, &stop));
        // The stop at WARM comes before the prompt, so the next line's output starts with it.
        let out = String::from_utf8(m.con().borrow_mut().take_output()).unwrap();
        let msg = out.trim_start_matches("> ").strip_prefix(&format!("{}\r\n", line)).and_then(|o| o.strip_suffix("\r\n"));
        assert!(msg.is_some_and(|msg| errors.contains(&msg)), "{}: {:?}", line, out);
    }
}

// ---------- HEX loader: validate before write, under the debugger ----------

#[test]
fn hex_records_are_validated_before_any_write() {
    // MONITOR_SPEC 7.2: the whole record is validated before any byte is written, and
    // a failure writes nothing. Once READ_LINE has stored the line (break at HEX_RECORD)
    // the debugger stops on any write outside the stack page, on any OUT but the console,
    // and at WARM. A record that writes nothing must reach WARM having printed its message.
    // A data record must make its first write from HR_WRITE, the step after every check,
    // at AAAA.
    let full = ":22020000000102030405060708090A0B0C0D0E0F101112131415161718191A1B1C1D1E1F2021AB";
    let quiet = [
        (":0100FF00AA56", "Address out of range"),
        (":0200FF001122CC", "Address out of range"),
        (":01EF0000AA66", "Address out of range"),
        (":02EEFF001122DE", "Address out of range"),
        (":01000000AA55", "Address out of range"),
        (":22EEDF00000102030405060708090A0B0C0D0E0F101112131415161718191A1B1C1D1E1F2021E0", "Address out of range"),
        (":11EEF00055555555555555555555555555555555556C", "Address out of range"),
        (":10FFF80011111111111111111111111111111111E9", "Address out of range"),
        (":0100000001FE", "Address out of range"),
        (":020080004142FB", "Address out of range"),
        (":2200CE000000000000000000000000000000000000000000000000000000000000000000000010", "Address out of range"),
        (":020000021000EC", "Bad record type"),
        (":0400000300000100F8", "Bad record type"),
        (":020000040000FA", "Bad record type"),
        (":0400000500000100F6", "Bad record type"),
        (":0401000601020304EB", "Bad record type"),
        (":010100FF55AA", "Bad record type"),
        (":0401000001020304F0", "Checksum error"),
        (":0401000001020G04F1", "Bad record"),
        (":0401000001020304", "Bad record"),
        (":0401000001020304F1 ", "Bad record"),
        (":", "Bad record"),
        (&format!(" {}", full), "Bad record"),
        (":23020000000102030405060708090A0B0C0D0E0F101112131415161718191A1B1C1D1E1F20212288", "Record too long"),
        (":0000000000", ""),
        (":00000001FF", "Loaded"),
        (":0101000112EB", "Loaded"),
    ];
    let writes = [(":01010000AA54", 0x0100u16, 0xAAu8), (":01EEFF00AA68", 0xEEFF, 0xAA), (full, 0x0200, 0x00)];
    let mut m = boot();
    let mut dbg = Debugger::new();
    dbg.load_symbols(&std::fs::read_to_string("rom/monitor.sym").unwrap()).unwrap();
    let mut until = |m: &mut Mon, line: &str| -> String {
        m.con().borrow_mut().take_output();
        m.con().borrow_mut().push_input(format!("{}\r", line).as_bytes());
        for cmd in ["bc", "b HEX_RECORD", "c"] {
            dbg.command(&mut m.cpu, cmd);
        }
        assert_eq!(dbg.run(&mut m.cpu, 1_000_000), Some(format!("break {:04X} HEX_RECORD", sym("HEX_RECORD"))), "{}", line);
        let watches = ["bc", "b WARM", "w 0000-EEFF w", "w F000-FFFF w"].map(String::from);
        for cmd in watches.into_iter().chain((0x01..=0xFF).map(|p| format!("io {:02X} out", p))) {
            assert_eq!(dbg.command(&mut m.cpu, &cmd).1, "", "{}", cmd);
        }
        assert_eq!(dbg.command(&mut m.cpu, "c").1, "");
        dbg.run(&mut m.cpu, 1_000_000).unwrap_or_else(|| panic!("{}: no stop, PC={:04X}", line, m.cpu.pc))
    };
    for (line, msg) in quiet {
        let stop = until(&mut m, line);
        assert_eq!(stop, format!("break {:04X} WARM", sym("WARM")), "{}", line);
        let out = String::from_utf8(m.con().borrow_mut().take_output()).unwrap();
        let echo = &line[..line.len().min(79)];
        let want = if msg.is_empty() { String::new() } else { format!("{}\r\n", msg) };
        assert_eq!(out.trim_start_matches("> ").strip_prefix(&format!("{}\r\n", echo)), Some(want.as_str()), "{}", line);
    }
    for (line, addr, byte) in writes {
        let stop = until(&mut m, line);
        assert_eq!(stop, format!("watch write {:04X} {:02X}", addr, byte), "{}", line);
        assert!((sym("HR_WRITE")..sym("HR_EOF")).contains(&m.cpu.pc), "{}: write from {:04X}", line, m.cpu.pc);
    }
}

// ---------- Example programs (examples/, USER_GUIDE 5) ----------

#[test]
fn example_hello() {
    boot().play("example_hello");
}

#[test]
fn example_memtest() {
    boot().play("example_memtest");
}

#[test]
fn example_burn() {
    boot().play("example_burn");
}

#[test]
fn example_tictac() {
    boot().play("example_tictac");
}

/// The reference for examples/tictac: plain minimax, no pruning. A board is 9 squares,
/// 0 free, 1 X, 2 O.
const TTT_LINES: [[usize; 3]; 8] = [[0, 1, 2], [3, 4, 5], [6, 7, 8], [0, 3, 6], [1, 4, 7], [2, 5, 8], [0, 4, 8], [2, 4, 6]];

fn ttt_won(b: &[u8; 9], p: u8) -> bool {
    TTT_LINES.iter().any(|l| l.iter().all(|&i| b[i] == p))
}

/// The score of `p` taking square `sq`, for `p`: a win scores the squares still free
/// plus one (sooner is better), a full board 0, any other move minus the best reply.
fn ttt_score(b: &mut [u8; 9], sq: usize, p: u8) -> i32 {
    b[sq] = p;
    let free = b.iter().filter(|&&c| c == 0).count() as i32;
    let s = if ttt_won(b, p) {
        free + 1
    } else if free == 0 {
        0
    } else {
        let free: Vec<usize> = (0..9).filter(|&i| b[i] == 0).collect();
        -free.into_iter().map(|i| ttt_score(b, i, 3 - p)).max().unwrap()
    };
    b[sq] = 0;
    s
}

/// The board as tictac prints it (transcript text): X, O, or the square's digit.
fn ttt_show(b: &[u8; 9]) -> String {
    let cell = |i: usize| match b[i] { 1 => 'X', 2 => 'O', _ => (b'1' + i as u8) as char };
    (0..3).map(|r| format!("{} {} {}\\r\\n", cell(3 * r), cell(3 * r + 1), cell(3 * r + 2))).collect()
}

/// tictac's BOARD (0103-010B: 0 free, 1 X, 4 O) and FREE (010C, the free squares).
fn ttt_memory(b: &[u8; 9]) -> Vec<u8> {
    let free = b.iter().filter(|&&c| c == 0).count() as u8;
    b.iter().map(|&c| [0, 1, 4][c as usize]).chain([free]).collect()
}

/// Every game from `b`, X to move: tictac sits at its move prompt with `b` on its board,
/// unless `again` (a game just ended, Again? is up). Each X move is tried in turn from
/// the same position, restored by writing BOARD and FREE back. Returns (games, won).
fn ttt_explore(m: &mut Mon, b: [u8; 9], again: &mut bool, best: &mut std::collections::HashMap<[u8; 9], Vec<usize>>) -> (u32, u32) {
    let at_again = "Again? (Y/N)\\r\\n";
    let saved = ttt_memory(&b);
    let (mut games, mut won) = (0, 0);
    for sq in (0..9).filter(|&i| b[i] == 0) {
        if *again {
            assert_eq!(m.run("Y"), format!("You are X. Type 1-9.\\r\\n{}", ttt_show(&[0; 9])));
            *again = false;
            m.poke(0x0103, &saved);
        }
        assert_eq!(m.mem(0x0103, 10), saved, "{:?}: BOARD and FREE", b);
        let mut b = b;
        b[sq] = 1;
        let out = m.run(&(sq + 1).to_string());
        assert!(!ttt_won(&b, 1), "the program lost: {:?}", b);
        if b.iter().all(|&c| c != 0) {
            assert_eq!(out, format!("{}Draw\\r\\n{}", ttt_show(&b), at_again), "{:?}", b);
            (games, *again) = (games + 1, true);
            continue;
        }
        let moves = best.entry(b).or_insert_with(|| {
            let mut t = b;
            let s: Vec<i32> = (0..9).map(|i| if t[i] == 0 { ttt_score(&mut t, i, 2) } else { i32::MIN }).collect();
            let max = *s.iter().max().unwrap();
            (0..9).filter(|&i| s[i] == max).collect()
        });
        let reply = out.strip_prefix("I play ").and_then(|r| r.get(..1)).and_then(|d| d.parse::<usize>().ok());
        let o = reply.unwrap_or_else(|| panic!("no reply to {:?}: {}", b, out)) - 1;
        assert!(moves.contains(&o), "{:?}: played {}, best {:?}", b, o + 1, moves);
        assert_eq!(o, moves[0], "{:?}: a tie goes to the lowest square", b);
        b[o] = 2;
        let shown = format!("I play {}\\r\\n{}", o + 1, ttt_show(&b));
        if ttt_won(&b, 2) {
            assert_eq!(out, format!("{}I win\\r\\n{}", shown, at_again), "{:?}", b);
            (games, won, *again) = (games + 1, won + 1, true);
        } else {
            assert_eq!(out, shown, "{:?}", b);
            let (g, w) = ttt_explore(m, b, again, best);
            (games, won) = (games + g, won + w);
        }
    }
    (games, won)
}

#[test]
fn tictac_never_loses_and_plays_optimal_moves() {
    // examples/tictac against every sequence of human moves (all of them, no sampling).
    // Every reply must be a move the Rust minimax scores best, so the program never loses,
    // takes the fastest win and puts off a loss. Each board it prints and each result line
    // must match the game. One session: a branch already played is not replayed from the
    // start; its position is written back into BOARD and FREE at the move prompt, where
    // they are tictac's whole game state.
    let mut m = boot();
    for record in std::fs::read_to_string("examples/tictac.hex").unwrap().lines() {
        m.run(record);
    }
    assert_eq!(m.run("G 0100"), format!("You are X. Type 1-9.\\r\\n{}", ttt_show(&[0; 9])));
    let mut again = false;
    let (games, won) = ttt_explore(&mut m, [0; 9], &mut again, &mut std::collections::HashMap::new());
    assert!(again);
    assert_eq!(show(&m.step(b"N").unwrap()), "N\\r\\n");
    assert!(games > 9 * 7 && won > 0, "{} games, {} won", games, won);
}

#[test]
fn example_ed() {
    boot().play("example_ed");
}

#[test]
fn example_life() {
    boot().play("example_life");
}

/// Conway's Life on a 32x16 torus (B3/S23), counted the textbook way: the 8 neighbours.
fn life_step(g: &[[bool; 32]; 16]) -> [[bool; 32]; 16] {
    let mut next = [[false; 32]; 16];
    for r in 0..16 {
        for c in 0..32 {
            let mut n = 0;
            for dr in [15, 0, 1] {
                for dc in [31, 0, 1] {
                    if (dr, dc) != (0, 0) && g[(r + dr) % 16][(c + dc) % 32] {
                        n += 1;
                    }
                }
            }
            next[r][c] = n == 3 || (n == 2 && g[r][c]);
        }
    }
    next
}

#[test]
fn life_matches_a_reference_model() {
    // examples/life.asm against a Rust model with the seed its header states: an
    // R-pentomino at rows 6-8, columns 20-22, and a glider at rows 1-3, columns 2-4.
    // 128 generations, every 32nd printed.
    let mut g = [[false; 32]; 16];
    for (r, c) in [(1, 3), (2, 4), (3, 2), (3, 3), (3, 4)] {
        g[r][c] = true;
    }
    // The model's wrap: a lone glider moves one row and one column every 4 generations, so
    // after 128 it has gone 32 of each, once round the torus across and twice down.
    let mut alone = g;
    for _ in 0..128 {
        alone = life_step(&alone);
    }
    assert!(alone == g, "the model's glider did not come home");
    for (r, c) in [(6, 21), (6, 22), (7, 20), (7, 21), (8, 21)] {
        g[r][c] = true;
    }
    let mut want = String::new();
    for gen in 0..=128 {
        if gen % 32 == 0 {
            want += &format!("Gen {:03}\\r\\n", gen);
            for row in &g {
                want.extend(row.iter().map(|&a| if a { '#' } else { '.' }));
                want += "\\r\\n";
            }
        }
        g = life_step(&g);
    }
    let mut m = boot();
    for record in std::fs::read_to_string("examples/life.hex").unwrap().lines() {
        m.run(record);
    }
    assert_eq!(m.run("G 0100"), want);
}

#[test]
fn example_mandel() {
    boot().play("example_mandel");
}

/// examples/mandel.asm's picture, from the same integer algorithm in Rust: 4.12 fixed
/// point, exact 32-bit squares, escape when x2 + y2 > 4.0, arithmetic shifts, 16-bit wrap.
fn mandel_model() -> String {
    let ramp = b".,:;-~=+*xoO%&$@";
    let mut s = String::new();
    for row in 0..19 {
        let cy: i16 = -9 * 539 + row * 539;
        for col in 0..40 {
            let cx: i16 = -8192 + col * 256;
            let (mut x, mut y) = (cx, cy);
            let mut ch = b' ';
            for &escaped in ramp {
                // One pass per iteration: MAXIT = 16 = the ramp's length.
                let (xx, yy) = (x as i32 * x as i32, y as i32 * y as i32);
                if (xx as u32).wrapping_add(yy as u32) > 4 << 24 {
                    ch = escaped;
                    break;
                }
                let xy = x as i32 * y as i32;
                (x, y) = ((((xx - yy) >> 12) as i16).wrapping_add(cx), ((xy >> 11) as i16).wrapping_add(cy));
            }
            s.push(ch as char);
        }
        s.push_str("\r\n");
    }
    s
}

/// A booted monitor with examples/mandel.hex pasted.
fn mandel() -> Mon {
    let mut m = boot();
    for record in std::fs::read_to_string("examples/mandel.hex").unwrap().lines() {
        m.run(record);
    }
    m
}

#[test]
fn mandel_matches_the_model() {
    // The model against known points first: -2 and 0 (row 9, columns 0 and 32) are in the
    // set, -2-1.18i (the corner) escapes at once (|c| > 2).
    let pic = mandel_model();
    let rows: Vec<&str> = pic.split("\r\n").collect();
    assert_eq!((&rows[9][..1], &rows[9][32..33], &rows[0][..1]), (" ", " ", "."));
    // About 17M cycles: inside BUDGET.
    assert_eq!(mandel().run("G 0100"), show(pic.as_bytes()));
}

#[test]
fn mandel_mul_is_exact() {
    // examples/mandel.asm's MUL (D:E:H:L = DE * BC, signed) against i32 multiply, called
    // directly: every pair of the edge values, then pseudo-random pairs. The picture only
    // reaches |operand| < 7000H; this covers 8000H and all four sign cases.
    let mut m = mandel();
    let code = m.mem(0x0100, 0x0300);
    let mul = 0x0100 + code.windows(3).position(|w| w == [0x7A, 0xA8, 0xF5]).expect("MUL: MOV A,D; XRA B; PUSH PSW") as u16;
    let edges: [i16; 14] = [0, 1, -1, 2, -2, 0x7FFF, -0x8000, -0x7FFF, 0x1000, -0x1000, 0x00FF, 0x0100, -0x0100, 0x5A5A];
    let mut pairs: Vec<(i16, i16)> = edges.iter().flat_map(|&a| edges.iter().map(move |&b| (a, b))).collect();
    let mut seed: u32 = 1;
    for _ in 0..1000 {
        seed = seed.wrapping_mul(1_103_515_245).wrapping_add(12_345);
        pairs.push(((seed >> 16) as i16, seed as i16));
    }
    const BACK: u16 = 0xCFF0; // never executed
    for (a, b) in pairs {
        [m.cpu.d, m.cpu.e] = a.to_be_bytes();
        [m.cpu.b, m.cpu.c] = b.to_be_bytes();
        m.cpu.sp -= 2;
        let sp = m.cpu.sp;
        m.poke(sp, &BACK.to_le_bytes());
        m.cpu.pc = mul;
        m.run_to(BACK);
        let got = i32::from_be_bytes([m.cpu.d, m.cpu.e, m.cpu.h, m.cpu.l]);
        assert_eq!(got, a as i32 * b as i32, "{} * {}", a, b);
    }
}

#[test]
fn example_pi() {
    boot().play("example_pi");
}

/// The first 100 decimals of pi as published: OEIS A000796, the decimal expansion of pi.
/// Typed from there, never from the emulator's output.
const PI_DECIMALS: &str =
    "1415926535897932384626433832795028841971693993751058209749445923078164062862089986280348253421170679";

/// A booted monitor with examples/pi.hex pasted.
fn pi_loaded() -> Mon {
    let mut m = boot();
    for record in std::fs::read_to_string("examples/pi.hex").unwrap().lines() {
        m.run(record);
    }
    m
}

#[test]
fn pi_prints_the_published_digits() {
    // The transcript's expected output is this: "3." and the published decimals, 50 to a line.
    let want = format!("3.\\r\\n{}\\r\\n{}\\r\\n", &PI_DECIMALS[..50], &PI_DECIMALS[50..]);
    assert_eq!(pi_loaded().run("G 0100"), want);
}

#[test]
fn pi_needs_its_carry() {
    // The 100 decimals cover the held-digit carry: the spigot makes a 10 at decimal 32.
    // With its test (CPI 10; JZ CARRY) made CPI 11, the 10 prints as ':' and decimal 31
    // stays the 4 the spigot made, so the output leaves the published digits there.
    let mut m = pi_loaded();
    let code = m.mem(0x0100, 0x0200);
    let at = code.windows(3).position(|w| w == [0xFE, 0x0A, 0xCA]).expect("no CPI 10; JZ in examples/pi.hex");
    m.poke(0x0101 + at as u16, &[11]);
    let got = m.run("G 0100").replace("\\r\\n", "");
    assert!(got.starts_with(&format!("3.{}4:", &PI_DECIMALS[..30])), "{}", got);
}

#[test]
fn example_rpn() {
    boot().play("example_rpn");
}

/// examples/rpn.asm's rules in i128: what it prints for `line` (escaped as `show` does),
/// with `stack` updated. An error prints its message and drops the rest of the line;
/// the token that failed leaves the stack as it was.
fn rpn_model(stack: &mut Vec<i128>, line: &str) -> String {
    const MAX: i128 = 9_999_999_999_999_999;
    let mut out = String::new();
    let b = line.as_bytes();
    let mut i = 0;
    while i < b.len() {
        let c = b[i].to_ascii_lowercase();
        i += 1;
        let n = stack.len();
        let err = match c {
            b'0'..=b'9' => {
                let mut v = (c - b'0') as i128;
                while i < b.len() && b[i].is_ascii_digit() && v <= MAX {
                    v = v * 10 + (b[i] - b'0') as i128;
                    i += 1;
                }
                if v > MAX {
                    Some("Overflow")
                } else if n == 4 {
                    Some("Stack overflow")
                } else {
                    stack.push(v);
                    None
                }
            }
            b' ' => None,
            b'+' | b'-' | b'*' | b'/' if n < 2 => Some("Stack underflow"),
            b'/' if stack[n - 1] == 0 => Some("Divide by zero"),
            b'+' | b'-' | b'*' | b'/' => {
                let (y, x) = (stack[n - 2], stack[n - 1]);
                let r = match c { b'+' => y + x, b'-' => y - x, b'*' => y * x, _ => y / x };
                if r.abs() > MAX {
                    Some("Overflow")
                } else {
                    stack.truncate(n - 2);
                    stack.push(r);
                    None
                }
            }
            b'p' if n == 0 => Some("Stack underflow"),
            b'p' => {
                out += &format!("{}\\r\\n", stack[n - 1]);
                None
            }
            b's' => {
                stack.iter().for_each(|v| out += &format!("{}\\r\\n", v));
                None
            }
            b'c' => {
                stack.clear();
                None
            }
            _ => Some("Bad input"),
        };
        if let Some(e) = err {
            out += e;
            out += "\\r\\n";
            break;
        }
    }
    out
}

/// xorshift64: the operands for rpn_matches_i128.
struct Rng(u64);

impl Rng {
    fn below(&mut self, n: u64) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0 % n
    }

    /// `len` digits, nines-heavy so additions carry through DAA, sometimes behind leading zeros.
    fn number(&mut self, len: u64) -> String {
        let mut s = if self.below(10) == 0 { "000".to_string() } else { String::new() };
        for _ in 0..len {
            let d = match self.below(10) { 0..=3 => 9, 4 => 0, _ => self.below(10) };
            s.push((b'0' + d as u8) as char);
        }
        s
    }
}

#[test]
fn rpn_matches_i128() {
    // examples/rpn.asm against an independent model (rpn_model): 200 operations on
    // pseudo-random operands, each line ending in `s`, so the whole stack is compared with
    // i128 arithmetic after every one. Results chain, so signs mix; multiplier lengths
    // mostly fit 16 digits, and the rest overflow.
    let mut m = boot();
    for record in std::fs::read_to_string("examples/rpn.hex").unwrap().lines() {
        m.run(record);
    }
    assert_eq!(m.run("G 0100"), "RPN calculator\\r\\n");
    let mut rng = Rng(0x2545_F491_4F6C_DD1D);
    let mut stack: Vec<i128> = Vec::new();
    let (mut ops, mut negative, mut errors) = (0, 0, 0);
    while ops < 200 {
        let top = stack.last().map_or(1, |v| v.unsigned_abs().to_string().len() as u64);
        let line = if stack.is_empty() || rng.below(10) == 0 {
            let len = 1 + rng.below(16);
            format!("c {} s", rng.number(len))
        } else if stack.len() < 3 && rng.below(8) == 0 {
            let len = 1 + rng.below(16);
            format!("{} s", rng.number(len))
        } else {
            ops += 1;
            let op = ["+", "-", "*", "/"][rng.below(4) as usize];
            let len = match op {
                "*" if rng.below(5) > 0 => 1 + rng.below(17u64.saturating_sub(top).max(1)),
                "/" => 1 + rng.below(top),
                _ => 1 + rng.below(16),
            };
            let x = if rng.below(25) == 0 { "0".to_string() } else { rng.number(len) };
            format!("{} {} s", x, op)
        };
        let want = rpn_model(&mut stack, &line);
        assert_eq!(m.run(&line), want, "{:?}", line);
        negative += want.contains('-') as u32;
        errors += (want.contains("flow") || want.contains("zero")) as u32;
    }
    assert!(negative >= 20 && errors >= 10, "{} negative, {} errors", negative, errors);
    assert_eq!(m.run("q"), "");
}

#[test]
fn example_sieve() {
    boot().play("example_sieve");
}

#[test]
fn sieve_matches_a_model() {
    // The expected output from an independent model, not from the emulator: trial division,
    // a different algorithm from the program's sieve. pi(8192) = 1028 is published (OEIS A007053).
    let primes: Vec<u32> = (2..8192u32).filter(|&n| (2..n).take_while(|d| d * d <= n).all(|d| n % d != 0)).collect();
    assert_eq!(primes.len(), 1028);
    let last: Vec<String> = primes[primes.len() - 10..].iter().map(|p| p.to_string()).collect();
    let want = format!("{} primes below 8192\\r\\nLast ten: {}\\r\\nSum: {}\\r\\n",
        primes.len(), last.join(" "), primes.iter().sum::<u32>());
    let mut m = boot();
    for record in std::fs::read_to_string("examples/sieve.hex").unwrap().lines() {
        m.run(record);
    }
    assert_eq!(m.run("G 0100"), want);
}

#[test]
fn example_hanoi() {
    boot().play("example_hanoi");
}

#[test]
fn hanoi_matches_a_rust_model() {
    // examples/hanoi.asm against a model that shares nothing with it: the 4-disk move list
    // from a Rust recursion, the counts as 2^n - 1. And the stack, seen from the CPU: the
    // program's RET leaves SP at F000, so it ran with SP = EFFE where G left it (MONITOR_SPEC
    // 8), and the recursion went 16 frames deep: HANOI(n > 1) is a 6-byte frame, and
    // HANOI(16) is entered at EFFC, so HANOI(1) is entered at EFFC - 6 * 15 = EFA2.
    fn hanoi(n: u32, from: char, to: char, via: char, out: &mut String) {
        if n > 0 {
            hanoi(n - 1, from, via, to, out);
            out.push_str(&format!("{}: {}->{}\r\n", n, from, to));
            hanoi(n - 1, via, to, from, out);
        }
    }
    let mut want = String::from("G 0100\r\nHanoi, 4 disks, A to C:\r\n");
    hanoi(4, 'A', 'C', 'B', &mut want);
    want.push_str("Moves for n disks:\r\n");
    for n in 1..=16 {
        want.push_str(&format!("n={}: {}\r\n", n, (1u32 << n) - 1));
    }
    want.push_str("Stack OK\r\n");
    let mut m = boot();
    for record in std::fs::read_to_string("examples/hanoi.hex").unwrap().lines() {
        m.run(record);
    }
    m.con().borrow_mut().push_input(b"G 0100\r");
    m.run_to(0x0100);
    assert_eq!(m.cpu.sp, 0xEFFE, "SP on entry");
    let sp = m.cpu.sp;
    let back = m.mem(sp, 2);
    let back = u16::from_le_bytes([back[0], back[1]]);
    let (start, mut low) = (m.cpu.cycles, m.cpu.sp);
    while m.cpu.pc != back {
        assert!(!m.cpu.halted && m.cpu.cycles - start < BUDGET, "no return, PC={:04X}", m.cpu.pc);
        m.cpu.execute_one();
        low = low.min(m.cpu.sp);
    }
    assert_eq!(m.cpu.sp, 0xF000, "SP after the RET");
    assert_eq!(low, 0xEFA2, "lowest SP");
    assert_eq!(show(&m.con().borrow_mut().take_output()), show(want.as_bytes()));
    assert_eq!(m.step(b"").unwrap(), b"", "the monitor prompt after the return");
}

/// A booted monitor with examples/burn.hex pasted and `image` at 1000, its default source.
fn burner(image: &[u8]) -> Mon {
    let mut m = boot();
    for record in std::fs::read_to_string("examples/burn.hex").unwrap().lines() {
        m.run(record);
    }
    m.poke(0x1000, image);
    m
}

/// rom/monitor.bin with a change the banner shows (`Ready.` becomes `REady.`) and one in
/// the last page (the final padding byte becomes 5A).
fn changed_rom() -> Vec<u8> {
    let mut rom = std::fs::read("rom/monitor.bin").unwrap();
    let at = rom.windows(6).position(|w| w == b"Ready.").unwrap();
    rom[at + 1] = b'E';
    rom[0xFFF] = 0x5A;
    rom
}

impl Mon {
    /// Type `input` and run `cycles` with no prompt expected (a program that never
    /// returns); return what was printed.
    fn run_for(&mut self, input: &[u8], cycles: u64) -> Vec<u8> {
        self.con().borrow_mut().push_input(input);
        let start = self.cpu.cycles;
        while self.cpu.cycles - start < cycles {
            assert!(!self.cpu.halted, "HLT at PC={:04X}", self.cpu.pc);
            self.cpu.execute_one();
            self.ports.extend(self.cpu.transfers().iter().filter(|t| matches!(t, Transfer::In(..) | Transfer::Out(..))));
        }
        self.con().borrow_mut().take_output()
    }
}

#[test]
fn burn_programs_a_changed_image() {
    // ARCHITECTURE 6.10: with JP-WE fitted, examples/burn writes the image to F000-FFFF and
    // jumps to F000: the new monitor cold-starts, and its banner shows the change. For any
    // tWC and both readings of the page-load window (6.10, Emulator). Once from an
    // unaligned source (SRC = 1234), so page offsets come from the destination.
    let new = changed_rom();
    let mut runs: Vec<(u64, bool, u16)> = Vec::new();
    for twc in [1, 20_480, 65_535] {
        runs.extend([(twc, false, 0x1000), (twc, true, 0x1000)]);
    }
    runs.push((20_480, false, 0x1234));
    for (twc, cells, src) in runs {
        let at = format!("tWC {} cells {} SRC {:04X}", twc, cells, src);
        let mut m = burner(&[]);
        m.poke(src, &new);
        m.poke(0x0103, &src.to_le_bytes());
        m.cpu.fit_jp_we(twc);
        m.cpu.set_load_window_cells(cells);
        let banner = m.run("G 0100");
        assert!(banner.starts_with("\\r\\n8080 Monitor v") && banner.ends_with("\\r\\nREady.\\r\\n"), "{}: {}", at, banner);
        assert!(m.mem(0xF000, 0x1000) == new, "{}: the ROM is not the image", at);
        assert_eq!(m.run("D FFF0 FFFF"), "FFF0: FF FF FF FF FF FF FF FF  FF FF FF FF FF FF FF 5A  ...............Z\\r\\n", "{}", at);
    }
}

#[test]
fn burn_page_loads_meet_tblc_and_close_before_the_poll() {
    // ARCHITECTURE 6.10 rule 4: 64 runs of 64 writes, one per 64-byte page in order, at most
    // 75 cycles apart start to start (tBLC 150 us at the slowest clock, tCY 2.0 us), with no
    // port access inside a run. Rule 3's exception: from the end of a page's last write to
    // the next read of the chip, at least 313 cycles (tBLC at the fastest clock, tCY
    // 0.48 us). And the program never runs ROM code until its final JMP F000.
    let new = changed_rom();
    let mut m = burner(&[]);
    m.poke(0x1234, &new);
    m.poke(0x0103, &[0x34, 0x12]);
    m.cpu.fit_jp_we(20_480);
    m.con().borrow_mut().push_input(b"G 0100\r");
    m.run_to(0x0100);
    // (start, end, transfer) for every write or read of the chip and every IN or OUT.
    let mut seen: Vec<(u64, u64, Transfer)> = Vec::new();
    while m.cpu.pc < 0xF000 {
        let start = m.cpu.cycles;
        assert!(start < BUDGET, "no JMP F000");
        m.cpu.execute_one();
        for &t in m.cpu.transfers() {
            if matches!(t, Transfer::MemWrite(0xF000.., _) | Transfer::MemRead(0xF000.., _) | Transfer::In(..) | Transfer::Out(..)) {
                seen.push((start, m.cpu.cycles, t));
            }
        }
    }
    assert_eq!(m.cpu.pc, 0xF000, "left the program other than by JMP F000");
    // A page load: the writes between two reads of the chip.
    let mut runs: Vec<Vec<usize>> = Vec::new();
    let mut open = false;
    for (i, s) in seen.iter().enumerate() {
        match s.2 {
            Transfer::MemWrite(..) if open => runs.last_mut().unwrap().push(i),
            Transfer::MemWrite(..) => {
                runs.push(vec![i]);
                open = true;
            }
            Transfer::MemRead(..) => open = false,
            _ => {}
        }
    }
    assert_eq!(runs.len(), 64, "page loads");
    for (page, run) in runs.iter().enumerate() {
        assert_eq!(run.len(), 64, "page {}: bytes in the load", page);
        for (n, &i) in run.iter().enumerate() {
            assert!(matches!(seen[i].2, Transfer::MemWrite(a, _) if a == 0xF000 + (page * 64 + n) as u16), "page {} byte {}: {:?}", page, n, seen[i].2);
            if n > 0 {
                let gap = seen[i].0 - seen[run[n - 1]].0;
                assert!(gap <= 75, "page {} byte {}: {} cycles after the last", page, n, gap);
            }
        }
        let (first, last) = (run[0], run[63]);
        assert!(seen[first..last].iter().all(|s| !matches!(s.2, Transfer::In(..) | Transfer::Out(..))), "page {}: a port access in the page load", page);
        let read = seen[last..].iter().find(|s| matches!(s.2, Transfer::MemRead(..))).expect("no poll");
        assert!(read.0 - seen[last].1 >= 313, "page {}: read {} cycles after the last write", page, read.0 - seen[last].1);
    }
    let banner = m.step(b"").unwrap();
    assert!(show(&banner).ends_with("\\r\\nREady.\\r\\n"), "{}", show(&banner));
    assert!(m.mem(0xF000, 0x1000) == new);
}

#[test]
fn burn_with_jp_we_open_fails_at_the_first_differing_byte() {
    // The likely user error (ARCHITECTURE 6.10): nothing is written, I/O6 never toggles, the
    // poll ends at once, and verify stops at the first byte that differs, after that page's
    // writes and no later. It prints the address by OUT 00 and spins in the program, with no
    // port access, until RESET, which boots the old ROM. A raised byte after five identical
    // pages, at page offset 3F; a lowered byte with S clear in the last page. The failure
    // path runs no ROM code (the ROM may be half new): PC stays below F000 throughout.
    let rom = std::fs::read("rom/monitor.bin").unwrap();
    assert_eq!(rom[0xFC1], 0xFF, "the last page is padding");
    for (offset, value, writes) in [(0x17F, rom[0x17F].wrapping_add(1), 6 * 64), (0xFC1, 0x5A, 4096)] {
        let mut image = rom.clone();
        image[offset] = value;
        let mut m = burner(&image);
        m.con().borrow_mut().push_input(b"G 0100\r");
        m.run_to(0x0100);
        m.con().borrow_mut().take_output();
        let from = m.ports.len();
        let mut n = 0;
        while !m.con().borrow().output().ends_with(b"\r\n") {
            assert!(m.cpu.cycles < BUDGET, "{:04X}: no message", offset);
            assert!(m.cpu.pc < 0xF000, "{:04X}: ROM code at {:04X}", offset, m.cpu.pc);
            m.cpu.execute_one();
            n += m.cpu.transfers().iter().filter(|t| matches!(t, Transfer::MemWrite(0xF000.., _))).count();
            m.ports.extend(m.cpu.transfers().iter().filter(|t| matches!(t, Transfer::In(..) | Transfer::Out(..))));
        }
        assert_eq!(show(&m.con().borrow_mut().take_output()), format!("Burn failed {:04X}\\r\\n", 0xF000 + offset));
        assert_eq!(n, writes, "{:04X}: writes before the failure", offset);
        assert!(m.ports[from..].iter().all(|t| matches!(t, Transfer::Out(0x00, _))), "{:04X}: {:?}", offset, &m.ports[from..]);
        let ports = m.ports.len();
        assert_eq!(m.run_for(b"", 1_000_000), b"");
        assert_eq!(m.ports.len(), ports, "a port access while spinning");
        assert!((0x0100..0x0200).contains(&m.cpu.pc), "PC={:04X}", m.cpu.pc);
        assert!(m.mem(0xF000, 0x1000) == rom);
        m.cpu.reset();
        let banner = m.step(b"");
        booted((m, banner));
    }
}

#[test]
fn burn_full_verify_catches_a_page_changed_after_its_verify() {
    // After the 64 pages, burn verifies all 4096 bytes before it jumps to F000. A byte
    // changed once every page has verified (the chip idle) and the full verify has read
    // F000 is caught there, in the first page or the last. The failure path runs no ROM
    // code: PC stays below F000 until the message ends.
    let new = changed_rom();
    for offset in [0x010, 0xFFF] {
        let mut m = burner(&new);
        m.cpu.fit_jp_we(1);
        m.con().borrow_mut().push_input(b"G 0100\r");
        m.run_to(0x0100);
        m.con().borrow_mut().take_output();
        let mut writes = 0;
        while !(writes == 4096 && m.cpu.transfers().contains(&Transfer::MemRead(0xF000, new[0]))) {
            assert!(m.cpu.cycles < BUDGET, "the full verify never started");
            m.cpu.execute_one();
            writes += m.cpu.transfers().iter().filter(|t| matches!(t, Transfer::MemWrite(0xF000.., _))).count();
        }
        let mut chip = new.clone();
        chip[offset] ^= 0xFF;
        m.cpu.load_rom(&chip);
        while !m.con().borrow().output().ends_with(b"\r\n") {
            assert!(m.cpu.cycles < BUDGET, "{:04X}: no message", offset);
            assert!(m.cpu.pc < 0xF000, "{:04X}: ROM code at {:04X}", offset, m.cpu.pc);
            m.cpu.execute_one();
        }
        assert_eq!(show(&m.con().borrow_mut().take_output()), format!("Burn failed {:04X}\\r\\n", 0xF000 + offset));
    }
}

#[test]
fn burn_refuses_a_ram_build_image() {
    // Byte 6 of the RAM test build is D0 (ARCHITECTURE 3.2): with JP-WE fitted, burn prints
    // `Not a ROM image`, returns, and the ROM is unchanged. Junk, an erased image, and each
    // byte wrong alone, below and above the value checked, are in the transcript.
    let (_, image) = ram_image();
    let rom = std::fs::read("rom/monitor.bin").unwrap();
    let mut m = burner(&image);
    m.cpu.fit_jp_we(20_480);
    assert_eq!(m.run("G 0100"), "Not a ROM image\\r\\n");
    assert!(m.mem(0xF000, 0x1000) == rom);
}

#[test]
fn burn_checks_the_source_first() {
    // The image, SRC to SRC+0FFF, must lie in 0200-EEFF: a ROM image at either end burns. Just
    // outside, below and above, at F000 (the ROM, a ROM image itself) and wrapping past FFFF,
    // burn prints `Bad source` with JP-WE fitted and the bytes the image check reads right
    // (the first 7, so nothing lands in the stack page), and the ROM is unchanged. The
    // transcript has the same bounds with JP-WE open.
    let new = changed_rom();
    let rom = std::fs::read("rom/monitor.bin").unwrap();
    for src in [0x0200u16, 0xDF00, 0x01FF, 0xDF01, 0xF000, 0xFFFF] {
        let ok = src == 0x0200 || src == 0xDF00;
        let mut m = burner(&[]);
        if src < 0xF000 {
            m.poke(src, if ok { &new } else { &new[..7] });
        }
        m.poke(0x0103, &src.to_le_bytes());
        m.cpu.fit_jp_we(20_480);
        let out = m.run("G 0100");
        if ok {
            assert!(out.ends_with("\\r\\nREady.\\r\\n"), "SRC {:04X}: {}", src, out);
            assert!(m.mem(0xF000, 0x1000) == new, "SRC {:04X}: the ROM is not the image", src);
        } else {
            assert_eq!(out, "Bad source\\r\\n", "SRC {:04X}", src);
            assert!(m.mem(0xF000, 0x1000) == rom, "SRC {:04X}: the ROM changed", src);
        }
    }
}

#[test]
fn burn_of_the_same_image_is_a_dry_run() {
    // JP-WE open and the image already in the ROM (M F000 1000 1000): every page verifies, and
    // the ROM monitor cold-starts, unchanged. Also when started from the RAM test build
    // (ARCHITECTURE 2.1): the JMP F000 lands in the ROM monitor.
    let rom = std::fs::read("rom/monitor.bin").unwrap();
    let (lines, _) = ram_image();
    for ram in [false, true] {
        let mut m = burner(&[]);
        if ram {
            for l in &lines {
                m.run(l);
            }
            assert!(m.run("G D000").split("\\r\\n").nth(1).unwrap().ends_with(" RAM"));
        }
        assert_eq!(m.run("M F000 1000 1000"), "");
        let banner = m.run("G 0100");
        let first = banner.split("\\r\\n").nth(1).unwrap_or("");
        assert!(banner.starts_with("\\r\\n8080 Monitor v") && !first.ends_with(" RAM"), "RAM build {}: {}", ram, banner);
        assert!(m.mem(0xF000, 0x1000) == rom);
    }
}

#[test]
fn cold_start_layout_is_what_burn_checks() {
    // ARCHITECTURE 3.2: the image starts LXI SP,F000 / DI / JMP BOOT_CONTINUE, so byte 0 is
    // 31 and byte 6 is the JMP's high byte: F0 in the ROM build, D0 in the RAM test build.
    let rom = std::fs::read("rom/monitor.bin").unwrap();
    assert_eq!(rom[..7], [0x31, 0x00, 0xF0, 0xF3, 0xC3, 0x07, 0xF0]);
    assert_eq!(ram_image().1[..7], [0x31, 0x00, 0xF0, 0xF3, 0xC3, 0x07, 0xD0]);
}

#[test]
fn memtest_default_range() {
    // The range a user runs: 0200-EEFF as assembled, about 13.1M cycles (under BUDGET).
    // Local only: the transcript uses a short range so it also runs on the daemon path and
    // under the RAM test build, where the default range would overwrite the running monitor.
    let mut m = boot();
    for record in std::fs::read_to_string("examples/memtest.hex").unwrap().lines() {
        m.run(record);
    }
    assert_eq!(m.run("G 0100"), "RAM OK\\r\\n");
}

#[test]
fn memtest_catches_an_alias_and_a_stuck_bit() {
    // The header's claim, over 0200-7FFF. `fault(cpu, address, value)` runs after each RAM
    // write and may write RAM.
    let run = |fault: &dyn Fn(&mut Intel8080, u16, u8)| {
        let mut m = boot();
        for record in std::fs::read_to_string("examples/memtest.hex").unwrap().lines() {
            m.run(record);
        }
        m.cpu.write_byte(0x0106, 0x7F); // LAST = 7FFF, as E 0105 sets it
        m.con().borrow_mut().take_output();
        m.con().borrow_mut().push_input(b"G 0100\r");
        let start = m.cpu.cycles;
        let mut out = Vec::new();
        while !out.ends_with(b"> ") {
            assert!(!m.cpu.halted && m.cpu.cycles - start < BUDGET, "{}", show(&out));
            m.cpu.execute_one();
            for t in m.cpu.transfers().to_vec() {
                match t {
                    Transfer::MemWrite(a, v) => fault(&mut m.cpu, a, v),
                    Transfer::Out(0x00, b) => out.push(b),
                    _ => {}
                }
            }
        }
        show(&out)
    };
    assert_eq!(run(&|_, _, _| {}), "G 0100\\r\\nRAM OK\\r\\n> ");
    // An address line fault: 1234 and 5234 are one cell (A14 lost there). Pass 1 writes 26
    // (34 XOR 12) at 1234, then 66 (34 XOR 52) over it at 5234; 1234 reads back wrong. A
    // pattern without the high byte writes 34 at both and misses it.
    let alias = |cpu: &mut Intel8080, a: u16, v: u8| match a {
        0x1234 => cpu.write_byte(0x5234, v),
        0x5234 => cpu.write_byte(0x1234, v),
        _ => {}
    };
    assert_eq!(run(&alias), "G 0100\\r\\nFAIL 1234\\r\\n> ");
    // D3 stuck at 0 in one cell: pass 1 writes 66 there (45 XOR 23, bit 3 clear) and passes;
    // only the complement pass, 99, sees it.
    let stuck = |cpu: &mut Intel8080, a: u16, v: u8| {
        if a == 0x2345 {
            cpu.write_byte(a, v & !0x08);
        }
    };
    assert_eq!(run(&stuck), "G 0100\\r\\nFAIL 2345\\r\\n> ");
}

#[test]
fn examples_match_their_hex() {
    // examples/NAME.hex is what a user pastes; tests/transcripts/example_NAME.txt pastes the
    // same records, in order, as its `> :` lines. The .hex is built from NAME.asm by
    // `cd examples && make` and committed, as rom/monitor_ram.hex is.
    let mut n = 0;
    for e in std::fs::read_dir("examples").unwrap() {
        let path = e.unwrap().path();
        if path.extension() != Some("hex".as_ref()) {
            continue;
        }
        let name = path.file_stem().unwrap().to_str().unwrap().to_string();
        let hex = std::fs::read_to_string(&path).unwrap();
        let script = std::fs::read_to_string(format!("tests/transcripts/example_{}.txt", name))
            .unwrap_or_else(|e| panic!("examples/{}.hex has no transcript: {}", name, e));
        let pasted: Vec<&str> = script.lines().filter_map(|l| l.strip_prefix("> ")).filter(|l| l.starts_with(':')).collect();
        let records: Vec<&str> = hex.lines().collect();
        assert_eq!(pasted, records, "examples/{}.hex", name);
        n += 1;
    }
    assert!(n >= 2, "{} examples", n);
}


// ---------- examples/ed.asm against a model ----------

/// examples/ed.asm's buffer, BUF (0500) to LIMIT (D000), from its header.
const ED_CAP: usize = 0xD000 - 0x0500;

/// A model of examples/ed.asm, written from its header comment and DEVICE_SPECS 6 and 7,
/// not from the program's output: what one session prints from the first "*" to the
/// echo of q, and the storage directory as the device leaves it.
#[derive(Default)]
struct EdModel {
    text: Vec<u8>,
    prev: u8,
    files: std::collections::BTreeMap<String, Vec<u8>>,
    out: Vec<u8>,
}

impl EdModel {
    /// A line as the program reads it (MONITOR_SPEC 2, but the LF right after a CR is skipped).
    fn line(&mut self, input: &mut impl Iterator<Item = u8>) -> Vec<u8> {
        let mut line = Vec::new();
        loop {
            let b = input.next().expect("the input ends before q");
            let prev = std::mem::replace(&mut self.prev, b);
            match b {
                b'\n' if prev == b'\r' => {}
                b'\r' | b'\n' => {
                    self.out.extend(b"\r\n");
                    return line;
                }
                0x08 | 0x7F => {
                    if line.pop().is_some() {
                        self.out.extend(b"\x08 \x08");
                    }
                }
                0x00..=0x1F => {}
                _ if line.len() < 79 => {
                    line.push(b);
                    self.out.push(b);
                }
                _ => {}
            }
        }
    }

    /// DEVICE_SPECS 7 mount: fold, validate, create a missing file empty.
    fn mount(&mut self, name: &[u8]) -> Option<String> {
        let name = String::from_utf8(name.to_ascii_uppercase()).ok()?;
        let valid = (1..=12).contains(&name.len()) && name != "." && name != ".."
            && name.bytes().all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || b".-_".contains(&c));
        if valid {
            self.files.entry(name.clone()).or_default();
        }
        valid.then_some(name)
    }

    /// Where line n starts; the end of the text for the last line + 1.
    fn start_of(&self, n: usize) -> Option<usize> {
        let mut at = 0;
        for _ in 1..n {
            if at == self.text.len() {
                return None;
            }
            at += self.text[at..].iter().position(|&b| b == b'\n').unwrap() + 1;
        }
        (n > 0).then_some(at)
    }

    fn run(&mut self, input: &[u8]) {
        let mut input = input.iter().copied();
        self.text.clear();
        self.prev = b'\r';
        loop {
            self.out.push(b'*');
            let line = self.line(&mut input);
            let skip = |s: &[u8]| s.iter().position(|&c| c != b' ').unwrap_or(s.len());
            let s = &line[skip(&line)..];
            let Some(&c) = s.first() else { continue };
            let arg = &s[1 + skip(&s[1..])..];
            let num = || -> Option<usize> {
                let digits = arg.iter().take_while(|c| c.is_ascii_digit()).count();
                let n: usize = std::str::from_utf8(&arg[..digits]).unwrap().parse().ok()?;
                (digits > 0 && n <= 0xFFFF && skip(&arg[digits..]) == arg.len() - digits).then_some(n)
            };
            let done = match c | 0x20 {
                b'q' if arg.is_empty() => return,
                b'a' if arg.is_empty() => {
                    self.input_mode(self.text.len(), &mut input);
                    Some(())
                }
                b'i' => num().and_then(|n| self.start_of(n)).map(|at| self.input_mode(at, &mut input)),
                b'd' => num().and_then(|n| self.start_of(n)).filter(|&at| at < self.text.len()).map(|at| {
                    let end = at + self.text[at..].iter().position(|&b| b == b'\n').unwrap() + 1;
                    self.text.drain(at..end);
                }),
                b'p' if arg.is_empty() => {
                    for (i, l) in self.text.split_inclusive(|&b| b == b'\n').enumerate() {
                        self.out.extend(format!("{} ", i + 1).bytes());
                        self.out.extend(&l[..l.len() - 1]);
                        self.out.extend(b"\r\n");
                    }
                    Some(())
                }
                b'c' if arg.is_empty() => {
                    self.text.clear();
                    Some(())
                }
                b'w' => self.mount(arg).map(|name| {
                    // From address 0, the text and the 1A marker; a file is never shortened.
                    let bytes = [&self.text[..], &[0x1A]].concat();
                    let file = self.files.get_mut(&name).unwrap();
                    if file.len() < bytes.len() {
                        file.resize(bytes.len(), 0);
                    }
                    file[..bytes.len()].copy_from_slice(&bytes);
                    self.out.extend(format!("{}\r\n", self.text.len()).bytes());
                }),
                b'r' => self.mount(arg).and_then(|name| {
                    let start = self.text.len();
                    let mut kept = start;
                    // A byte that does not fit drops the line it is in, then "?".
                    for &b in self.files[&name].iter().chain(&[0x1A]) {
                        if b == 0x1A {
                            break;
                        }
                        if self.text.len() == ED_CAP {
                            self.text.truncate(kept);
                            return None;
                        }
                        self.text.push(b);
                        if b == b'\n' {
                            kept = self.text.len();
                        }
                    }
                    // A last line with no LF gets one, if there is room for it.
                    if self.text.len() != kept {
                        if self.text.len() == ED_CAP {
                            self.text.truncate(kept);
                            return None;
                        }
                        self.text.push(b'\n');
                    }
                    let added = self.text.len() - start;
                    self.out.extend(format!("{}\r\n", added).bytes());
                    (added > 0).then_some(())
                }),
                _ => None,
            };
            if done.is_none() {
                // r of nothing printed its 0: take it back, the program prints only "?".
                if self.out.ends_with(b"\n0\r\n") {
                    self.out.truncate(self.out.len() - 3);
                }
                self.out.extend(b"?\r\n");
            }
        }
    }

    fn input_mode(&mut self, mut at: usize, input: &mut impl Iterator<Item = u8>) {
        loop {
            let mut line = self.line(input);
            if line == b"." {
                return;
            }
            if self.text.len() + line.len() + 1 > ED_CAP {
                self.out.extend(b"?\r\n");
                continue;
            }
            line.push(b'\n');
            self.text.splice(at..at, line.iter().copied());
            at += line.len();
        }
    }
}

/// A booted monitor with examples/ed.hex pasted.
fn ed() -> Mon {
    let mut m = boot();
    for record in std::fs::read_to_string("examples/ed.hex").unwrap().lines() {
        m.run(record);
    }
    m
}

impl Mon {
    /// One editor session, G 0100 to q: it must print what the model prints, and leave the
    /// storage directory as the model does.
    fn ed_session(&mut self, model: &mut EdModel, input: &[u8]) {
        model.out.clear();
        model.run(input);
        let got = self.step(&[b"G 0100\r", input].concat()).unwrap_or_else(|e| panic!("{}", e));
        assert_eq!(show(&got), show(&[&b"G 0100\r\n"[..], &model.out].concat()));
        let disk: std::collections::BTreeMap<String, Vec<u8>> = std::fs::read_dir(self.dir.path()).unwrap()
            .map(|e| e.unwrap())
            .map(|e| (e.file_name().into_string().unwrap(), std::fs::read(e.path()).unwrap()))
            .collect();
        assert_eq!(disk, model.files);
    }
}

#[test]
fn ed_matches_its_model() {
    // Line input, every command, every error (65537 wraps to line 1 without the overflow
    // check), names the device folds or refuses, a write over a longer file, and a host file
    // with CR LF, a blank line, a NUL and no final LF.
    let mut m = ed();
    let mut model = EdModel::default();
    let long = "x".repeat(100);
    let session = format!(concat!(
        "a\rone\rtwo\x08\x08o\rthree\r{}\r\ttab\x1Aignored\rfour\r\nfive\nsix\r\r\nét\u{FF}\r.\r\n",
        "P\r  p  \ri 1\rzero\r.\ri 007\rseven\r.\ri 12\rlast\r.\ri 15\r",
        "d 0\rd 65535\rd 65536\rd 65537\rd 99999\rd 1x\rd\rd 1 2\rd 2\rd 13\rd 12\rp junk\r\r   \rz\r\x08\rp\r",
        "w lower.txt\rw\rw A.B.C \rw ABCDEFGHIJKLM\rw ..\rw GOOD-1_x.Y\r",
        "c\rr LOWER.TXT\rr lower.txt\rp\rr EMPTY\rd 3\rw LOWER.TXT\rc\rr LOWER.TXT\rp\rq x\rq\r"), long);
    m.ed_session(&mut model, session.as_bytes());
    std::fs::write(m.dir.path().join("HOST.TXT"), b"alpha\r\nbeta\n\n\x00gamma").unwrap();
    model.files.insert("HOST.TXT".into(), b"alpha\r\nbeta\n\n\x00gamma".to_vec());
    // A stale name character (O 0D) must not reach the mount: the resync query clears it.
    m.run("O 0D 41");
    m.ed_session(&mut model, b"r host.txt\rp\ra\rmore\r.\rw HOST.TXT\rq\r");
}

#[test]
fn ed_fills_its_buffer() {
    // Typed lines until the buffer is full (each further line prints "?"), block moves of
    // the whole buffer (d 1, i 1), then r of a file larger than the buffer: the lines that
    // fit, then "?". Nothing at or above LIMIT (D000) is written.
    let mut m = ed();
    let mut model = EdModel::default();
    let mut s = b"a\r".to_vec();
    for i in 0..700 {
        s.extend(format!("{:03} {}", i, "abcdefghij".repeat(8))[..79].bytes().chain(*b"\r"));
    }
    s.extend(b".\rw BIG\rq\r");
    let above = m.mem(0xD000, 0x1F00);
    m.ed_session(&mut model, &s);
    // 80-byte lines: 649 fit in 0500-CFFF (51968 bytes), the other 51 print "?".
    assert_eq!(model.out.windows(4).filter(|w| w == b"\n?\r\n").count(), 51);
    assert!(model.out.ends_with(format!("*w BIG\r\n{}\r\n*q\r\n", 649 * 80).as_bytes()));
    // One step has BUDGET cycles: the moves of the whole buffer get a session of their own.
    // One line of 80 out, one of 4 in, one of 80 out.
    m.ed_session(&mut model, b"r BIG\rd 1\ri 1\rnew\r.\rd 649\rw BIG\rq\r");
    assert!(model.out.ends_with(format!("*w BIG\r\n{}\r\n*q\r\n", 649 * 80 - 80 + 4 - 80).as_bytes()));
    let big: Vec<u8> = (0..9000).flat_map(|i| format!("line {}\n", i).into_bytes()).collect();
    std::fs::write(m.dir.path().join("BIGR"), &big).unwrap();
    model.files.insert("BIGR".into(), big);
    m.ed_session(&mut model, b"r BIGR\rr BIG\rw OUT\rq\r");
    // A buffer typed full to its last byte (CFFF) is written and read back whole: 649 lines
    // of 80 and one of 48 are ED_CAP bytes, and one more line does not fit.
    let mut s = b"a\r".to_vec();
    for _ in 0..649 {
        s.extend([b'x'; 79].iter().chain(b"\r"));
    }
    s.extend(b"yyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyy\rz\r.\rw FULL\rq\r");
    m.ed_session(&mut model, &s);
    assert!(model.out.ends_with(format!("y\r\nz\r\n?\r\n.\r\n*w FULL\r\n{}\r\n*q\r\n", ED_CAP).as_bytes()));
    m.ed_session(&mut model, b"r FULL\rw TWO\rq\r");
    assert!(model.out.ends_with(format!("*r FULL\r\n{}\r\n*w TWO\r\n{0}\r\n*q\r\n", ED_CAP).as_bytes()));
    // The edges with no marker: ED_CAP bytes ending in LF fit; ED_CAP - 1 bytes with no
    // final LF fill the buffer exactly, the LF r adds at CFFF; ED_CAP bytes with no final LF
    // leave no room for that LF, so the last line is dropped and r prints "?".
    let exact: Vec<u8> = (0..ED_CAP).map(|i| if i % 80 == 79 || i == ED_CAP - 1 { b'\n' } else { b'e' }).collect();
    let edge: Vec<u8> = (0..ED_CAP - 1).map(|i| if i % 80 == 79 { b'\n' } else { b'g' }).collect();
    let over: Vec<u8> = (0..ED_CAP).map(|i| if i % 80 == 79 { b'\n' } else { b'o' }).collect();
    for (name, file) in [("EXACT", exact), ("EDGE", edge), ("OVER", over)] {
        std::fs::write(m.dir.path().join(name), &file).unwrap();
        model.files.insert(name.into(), file);
    }
    m.ed_session(&mut model, b"r EXACT\rc\rr EDGE\rw OUT\rc\rr OVER\rw OUT\rq\r");
    assert!(model.out.ends_with(format!("*r EXACT\r\n{}\r\n*c\r\n*r EDGE\r\n{0}\r\n*w OUT\r\n{0}\r\n*c\r\n*r OVER\r\n?\r\n*w OUT\r\n{}\r\n*q\r\n",
        ED_CAP, ED_CAP / 80 * 80).as_bytes()));
    assert!(m.mem(0xD000, 0x1F00) == above, "ed wrote at or above D000");
}

#[test]
fn ed_drops_a_line_cut_by_a_storage_error() {
    // A host read error (DEVICE_SPECS 6): the failed read returns FF, then the device
    // unmounts. The host file shrinks to 8 bytes after the mount, under the device's cached
    // size, so reading byte 8 fails. The whole line before it stays; "wo" and the FF go.
    let mut m = ed();
    let path = m.dir.path().join("CUT.TXT");
    std::fs::write(&path, b"hello\nworld\n").unwrap();
    m.con().borrow_mut().take_output();
    m.con().borrow_mut().push_input(b"G 0100\rr CUT.TXT\rp\rq\r");
    let start = m.cpu.cycles;
    let mut out = Vec::new();
    while !out.ends_with(b"> ") {
        assert!(!m.cpu.halted && m.cpu.cycles - start < BUDGET, "{}", show(&out));
        m.cpu.execute_one();
        for &t in m.cpu.transfers() {
            match t {
                Transfer::Out(0x0E, 0x01) => std::fs::OpenOptions::new().write(true).open(&path).unwrap().set_len(8).unwrap(),
                Transfer::Out(0x00, b) => out.push(b),
                _ => {}
            }
        }
    }
    assert_eq!(show(&out), "G 0100\\r\\n*r CUT.TXT\\r\\n?\\r\\n*p\\r\\n1 hello\\r\\n*q\\r\\n> ");
}
