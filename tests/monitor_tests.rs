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
use std::time::Duration;

use intel8080_emu::cpu::Transfer;
use intel8080_emu::debugger::Debugger;
use intel8080_emu::io::build_bus;
use intel8080_emu::io::devices::console::Console;
use intel8080_emu::io::devices::mailbox::{self, Mailbox};
use intel8080_emu::io::{IoBus, IoDevice};
use intel8080_emu::pi;
use intel8080_emu::pi::sim::{Bridge, Knobs, SimBoard};
use intel8080_emu::Intel8080;

/// HLT. A NOP-like byte (00, or A5 = ANA L) would slide execution into F000 and
/// boot the ROM even with the overlay missing. RST 0 would jump back to 0000.
/// HLT stops the CPU at the first junk byte it executes, and a halt fails the step.
const JUNK: u8 = 0x76;
/// Pushes before `LXI SP` land in ROM and are lost, so a return through them crashes.
const JUNK_SP: u16 = 0x0000;
/// Cycles per step. `C 0000 FFFF` is the slowest command and needs about 10M.
const BUDGET: u64 = 30_000_000;

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
}

/// Power on with junk RAM and the port map main.rs uses (build_bus); returns the
/// monitor and what boot printed up to the first prompt.
fn power_on(overlay: bool) -> (Mon, Result<Vec<u8>, String>) {
    let dir = tempfile::tempdir().unwrap();
    let (bus, con) = build_bus(dir.path(), mailbox::local_time);
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
    let mut m = Mon { cpu, side, dir, ports: Vec::new() };
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
        // Since the step began: IN 01 count, OUT 00 bytes.
        let (mut read, mut out) = (0, Vec::new());
        let mut quiet_since = None;
        loop {
            if self.cpu.halted {
                return Err(format!("HLT at PC={:04X} after {:?}", self.cpu.pc, show(input)));
            }
            if self.cpu.cycles - start > BUDGET {
                return Err(format!("no prompt within {} cycles after {:?}, PC={:04X}, output {:?}",
                    BUDGET, show(input), self.cpu.pc, show(&out)));
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
                pi::serve(board, fsel2, &storage, mailbox::local_time, listener, Some(trace), &stop)
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
        let mut child = Command::new(env!("CARGO_BIN_EXE_pi8080d"))
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
        assert!(line.trim_end().ends_with("board simulated, ROM rom/monitor.bin"), "{:?}", line);
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
    // Step 2: "Each response byte read from IN 13h is printed to the console as it arrives."
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
    // Messages (5): "Service error | T, U: mailbox status 00 after execute, or 80-FF".
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
    let mb = Rc::new(RefCell::new(Mailbox::new(|| None)));
    m.map_mailbox(mb);
    assert_eq!(m.run("T"), "Service error\\r\\n");
    assert_eq!(m.run("I 12"), "83\\r\\n");
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
    // MONITOR_SPEC 8: on entry SP = EFFE, the word there is WARM, interrupts are off and
    // the overlay is off. The return through it is in go.txt. The error before G leaves
    // two pushes and a return address behind: WARM must reset SP.
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
        assert_eq!((m.cpu.sp, m.cpu.read_word(0xEFFE)), (0xEFFE, sym("WARM")), "{}", line);
        assert!(!m.cpu.interrupts_enabled && !m.cpu.rom_overlay_enabled);
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
