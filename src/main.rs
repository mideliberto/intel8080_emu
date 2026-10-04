// main.rs - The host side of the emulator (ARCHITECTURE 7): terminal, key map, run loop,
// debugger entry and prompt.

use std::cell::RefCell;
use std::io::{IsTerminal, Read, Write};
use std::path::Path;
use std::sync::mpsc::{self, RecvTimeoutError};
use std::time::Duration;

use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use crossterm::terminal::{disable_raw_mode, enable_raw_mode};

use intel8080_emu::debugger::{Debugger, Flow};
use intel8080_emu::io::build_bus;
use intel8080_emu::io::devices::ask::AskConfig;
use intel8080_emu::io::devices::mailbox;
use intel8080_emu::io::devices::console::Console;
use intel8080_emu::Intel8080;

const BUILD_TIMESTAMP: &str = env!("BUILD_TIMESTAMP");

/// Host input is read at least this often, in executed steps (ARCHITECTURE 7.2).
const PUMP_STEPS: u64 = 10_000;

/// How long a pump waits for host input when the 8080 is polling an empty console
/// (ARCHITECTURE 7.2, `idle_waits`), so an idle 8080 does not spin a host core.
const IDLE_WAIT: Duration = Duration::from_millis(1);

#[derive(Debug, PartialEq)]
enum Exit {
    Halted,
    Quit,
    /// A `--script` line was bad (ARCHITECTURE 7.4): exit status 2.
    BadScript,
}

/// Host keys that never reach the console (ARCHITECTURE 7.1).
#[derive(Debug, PartialEq)]
enum Signal {
    Quit,
    Debug,
}

const USAGE: &str = "usage: intel8080 [--debug] [--script FILE]";

fn main() {
    // ARCHITECTURE 7.4, Entry.
    let mut start_stopped = false;
    let mut scripted = false;
    let mut script = Vec::new();
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--debug" => start_stopped = true,
            "--script" if i + 1 < args.len() => {
                i += 1;
                let text = std::fs::read_to_string(&args[i]).unwrap_or_else(|e| {
                    eprintln!("{}: {}", args[i], e);
                    std::process::exit(2);
                });
                let lines = text.lines().map(str::trim).filter(|l| !l.is_empty() && !l.starts_with('#'));
                script.extend(lines.map(String::from));
                start_stopped = true;
                scripted = true;
            }
            _ => {
                eprintln!("{}", USAGE);
                std::process::exit(2);
            }
        }
        i += 1;
    }

    println!("8080 Emulator");
    println!("Built: {}", BUILD_TIMESTAMP);

    // new() leaves the CPU in its RESET state: PC=0000, ROM overlay set.
    let mut cpu = Intel8080::new();
    // ASK's key (DEVICE_SPECS 8, ASK service): read once; unset or empty is no key.
    let key = std::env::var("ANTHROPIC_API_KEY").ok().filter(|k| !k.is_empty());
    let (bus, console) = build_bus(Path::new("storage"), mailbox::local_time, AskConfig { key, ..AskConfig::default() });
    *cpu.io_bus_mut() = bus;
    cpu.load_rom_from_file(Path::new("rom/monitor.bin")).expect("Failed to load ROM");
    let mut dbg = Debugger::new();
    if let Ok(text) = std::fs::read_to_string("rom/monitor.sym") {
        dbg.load_symbols(&text).expect("rom/monitor.sym");
    }

    let mut stdout = std::io::stdout();
    let terminal = std::io::stdin().is_terminal() && enable_raw_mode().is_ok();
    if terminal {
        let default_hook = std::panic::take_hook();
        std::panic::set_hook(Box::new(move |info| {
            let _ = disable_raw_mode();
            default_hook(info);
        }));
    }
    let halt_stops = halt_prompts(terminal, scripted);
    let mut script = script.into_iter();
    let mut stop = |cpu: &mut Intel8080, dbg: &mut Debugger, reason: &str| {
        if terminal {
            let _ = disable_raw_mode();
        }
        let flow = prompt(cpu, dbg, reason, &mut script, terminal);
        if terminal && flow == Flow::Resume {
            let _ = enable_raw_mode();
        }
        flow
    };
    let first = if start_stopped { stop(&mut cpu, &mut dbg, "start") } else { Flow::Resume };
    let exit = if first == Flow::Error {
        Exit::BadScript
    } else if first != Flow::Resume {
        Exit::Quit
    } else if terminal {
        let keys = |wait| {
            let _ = event::poll(wait);
            pump(std::iter::from_fn(host_key))
        };
        run_loop(&mut cpu, &console, &mut dbg, keys, &mut stop, halt_stops, &mut stdout)
    } else {
        // Not a terminal: stdin bytes go to the console unmapped. Ctrl-C is the shell's.
        let (tx, rx) = mpsc::channel();
        std::thread::spawn(move || {
            let mut buf = [0u8; 4096];
            let mut stdin = std::io::stdin().lock();
            while let Ok(n @ 1..) = stdin.read(&mut buf) {
                if tx.send(buf[..n].to_vec()).is_err() {
                    break;
                }
            }
        });
        let bytes = |wait| {
            let mut bytes = match rx.recv_timeout(wait) {
                Ok(bytes) => bytes,
                Err(RecvTimeoutError::Timeout) => vec![],
                // End of input: runs end on HLT or Ctrl-C (7.2), so wait like an idle terminal.
                Err(RecvTimeoutError::Disconnected) => {
                    std::thread::sleep(wait);
                    vec![]
                }
            };
            bytes.extend(rx.try_iter().flatten());
            (bytes, None)
        };
        run_loop(&mut cpu, &console, &mut dbg, bytes, &mut stop, halt_stops, &mut stdout)
    };
    if terminal {
        let _ = disable_raw_mode();
    }

    // v1 has no interrupt source, so a halt is final (ARCHITECTURE 7.2).
    match exit {
        Exit::Halted => println!("\nHLT at PC={:04X}", cpu.pc),
        Exit::Quit => println!(),
        Exit::BadScript => {
            drop(dbg); // writes the pending port trace lines
            drop(cpu); // the mailbox kills a running GET worker; storage flushes
            std::process::exit(2);
        }
    }
}

/// Whether a halt opens the debugger prompt (ARCHITECTURE 7.2): only in an interactive
/// run. A piped or scripted run prints `HLT at PC=xxxx` and exits.
fn halt_prompts(terminal: bool, scripted: bool) -> bool {
    terminal && !scripted
}

/// The debugger prompt (ARCHITECTURE 7.4): prints the stop report, then runs commands
/// from the script, then from the terminal. Returns Resume (back to the 8080), Quit,
/// or Error (a bad script line; a bad terminal line only prints its `? message`).
fn prompt(
    cpu: &mut Intel8080,
    dbg: &mut Debugger,
    reason: &str,
    script: &mut impl Iterator<Item = String>,
    terminal: bool,
) -> Flow {
    print!("{}", dbg.report(cpu, reason));
    loop {
        let (line, scripted) = match script.next() {
            Some(line) => {
                println!("dbg> {}", line);
                (line, true)
            }
            None if terminal => {
                print!("dbg> ");
                let _ = std::io::stdout().flush();
                let mut line = String::new();
                match std::io::stdin().read_line(&mut line) {
                    Ok(1..) => (line, false),
                    _ => return Flow::Quit, // end of input is q
                }
            }
            None => return Flow::Quit,
        };
        let (flow, out) = dbg.command(cpu, &line);
        print!("{}", out);
        let _ = std::io::stdout().flush();
        match flow {
            Flow::Stay => {}
            Flow::Error if !scripted => {}
            flow => return flow,
        }
    }
}

/// Runs until the CPU halts (unless `halt_stops`), `input` signals quit, or `prompt`
/// returns anything but Resume. `input(wait)` waits up to `wait` for host input and
/// returns the console bytes that arrived since its last call, and a host signal that
/// came after them. Each pump calls it with no wait, then, when `idle_waits`, once more
/// with IDLE_WAIT. A pump runs before the first step and then every PUMP_STEPS steps.
/// Console output is drained to `out` as often. Every step goes through the debugger; a debugger stop, Ctrl-E, or a halt with
/// `halt_stops` calls `prompt` with the reason, after a CR LF if the console output so
/// far does not end with LF.
fn run_loop(
    cpu: &mut Intel8080,
    console: &RefCell<Console>,
    dbg: &mut Debugger,
    mut input: impl FnMut(Duration) -> (Vec<u8>, Option<Signal>),
    mut prompt: impl FnMut(&mut Intel8080, &mut Debugger, &str) -> Flow,
    halt_stops: bool,
    out: &mut impl Write,
) -> Exit {
    let mut last = b'\n';
    loop {
        last = drain(console, out).unwrap_or(last);
        let stop = if cpu.halted {
            if !halt_stops {
                return Exit::Halted;
            }
            Some("halt".to_string())
        } else {
            let (mut bytes, mut signal) = input(Duration::ZERO);
            let polled = console.borrow_mut().take_polled();
            if idle_waits(bytes.is_empty() && signal.is_none(), !console.borrow().has_input(), polled) {
                (bytes, signal) = input(IDLE_WAIT);
            }
            console.borrow_mut().push_input(&bytes);
            match signal {
                Some(Signal::Quit) => return Exit::Quit,
                Some(Signal::Debug) => Some("ctrl-e".to_string()),
                None => dbg.run(cpu, PUMP_STEPS),
            }
        };
        if let Some(reason) = stop {
            last = drain(console, out).unwrap_or(last);
            if last != b'\n' {
                let _ = out.write_all(b"\r\n");
                let _ = out.flush();
                last = b'\n';
            }
            match prompt(cpu, dbg, &reason) {
                Flow::Resume => {}
                Flow::Error => return Exit::BadScript,
                Flow::Stay | Flow::Quit => return Exit::Quit,
            }
        }
    }
}

/// Whether a pump waits IDLE_WAIT for host input (ARCHITECTURE 7.2): only when it
/// brought nothing, the console input FIFO is empty, and the 8080 read IN 02 during the
/// last pump interval. A compute-bound program does not poll, so it never waits.
fn idle_waits(brought_nothing: bool, fifo_empty: bool, polled: bool) -> bool {
    brought_nothing && fifo_empty && polled
}

/// OUT 00 never waits: a host write error loses the bytes, like a missing terminal.
/// Returns the last byte drained.
fn drain(console: &RefCell<Console>, out: &mut impl Write) -> Option<u8> {
    let bytes = console.borrow_mut().take_output();
    let _ = out.write_all(&bytes);
    let _ = out.flush();
    bytes.last().copied()
}

/// The next pending key press, without waiting.
fn host_key() -> Option<KeyEvent> {
    while event::poll(Duration::ZERO).unwrap_or(false) {
        if let Ok(Event::Key(key)) = event::read() {
            if key.kind == KeyEventKind::Press {
                return Some(key);
            }
        }
    }
    None
}

/// Maps keys to console bytes in order, up to the first host signal. Keys after
/// the signal are not consumed.
fn pump(keys: impl Iterator<Item = KeyEvent>) -> (Vec<u8>, Option<Signal>) {
    let mut bytes = Vec::new();
    for key in keys {
        match map_key(key) {
            Ok(b) => bytes.extend(b),
            Err(signal) => return (bytes, Some(signal)),
        }
    }
    (bytes, None)
}

/// The host key map (ARCHITECTURE 7.1). An empty Vec is a dropped key.
fn map_key(key: KeyEvent) -> Result<Vec<u8>, Signal> {
    let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
    let alt = key.modifiers.contains(KeyModifiers::ALT);
    Ok(match key.code {
        KeyCode::Enter => vec![0x0D],
        KeyCode::Backspace => vec![0x08],
        KeyCode::Tab => vec![0x09],
        KeyCode::Esc => vec![0x1B],
        KeyCode::Char(c) if ctrl && !alt && c.is_ascii_alphabetic() => match c.to_ascii_lowercase() {
            'c' => return Err(Signal::Quit),
            'e' => return Err(Signal::Debug),
            c => vec![c as u8 - b'a' + 1],
        },
        KeyCode::Char(c) if !ctrl && !alt && (matches!(c, ' '..='~') || !c.is_ascii()) => {
            c.to_string().into_bytes()
        }
        _ => vec![],
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use intel8080_emu::io::IoDevice;

    fn key(code: KeyCode, modifiers: KeyModifiers) -> KeyEvent {
        KeyEvent::new(code, modifiers)
    }

    fn ch(c: char) -> KeyEvent {
        key(KeyCode::Char(c), KeyModifiers::NONE)
    }

    fn ctrl(c: char) -> KeyEvent {
        key(KeyCode::Char(c), KeyModifiers::CONTROL)
    }

    #[test]
    fn key_map() {
        let none = KeyModifiers::NONE;
        let cases: &[(KeyEvent, &[u8])] = &[
            (key(KeyCode::Enter, none), &[0x0D]),
            (key(KeyCode::Backspace, none), &[0x08]),
            (key(KeyCode::Tab, none), &[0x09]),
            (key(KeyCode::Esc, none), &[0x1B]),
            (ctrl('a'), &[0x01]),
            (ctrl('A'), &[0x01]),
            (ctrl('d'), &[0x04]),
            (ctrl('f'), &[0x06]),
            (ctrl('s'), &[0x13]),
            (key(KeyCode::Char('Z'), KeyModifiers::CONTROL | KeyModifiers::SHIFT), &[0x1A]),
            (ch(' '), b" "),
            (ch('a'), b"a"),
            (key(KeyCode::Char('A'), KeyModifiers::SHIFT), b"A"),
            (ch('~'), b"~"),
            (ch('\u{e9}'), &[0xC3, 0xA9]),
            (ch('\u{20AC}'), &[0xE2, 0x82, 0xAC]),
            (ch('\x7F'), &[]),
            (ch('\x01'), &[]),
            (ctrl('4'), &[]),
            (key(KeyCode::Char('x'), KeyModifiers::ALT), &[]),
            (key(KeyCode::Char('a'), KeyModifiers::CONTROL | KeyModifiers::ALT), &[]),
            (key(KeyCode::Char('c'), KeyModifiers::CONTROL | KeyModifiers::ALT), &[]),
            (key(KeyCode::Up, none), &[]),
            (key(KeyCode::F(1), none), &[]),
            (key(KeyCode::Delete, none), &[]),
            (key(KeyCode::BackTab, KeyModifiers::SHIFT), &[]),
        ];
        for (k, bytes) in cases {
            assert_eq!(map_key(*k).as_deref(), Ok(*bytes), "{:?}", k);
        }
        assert_eq!(map_key(ctrl('c')), Err(Signal::Quit));
        assert_eq!(map_key(ctrl('C')), Err(Signal::Quit));
        assert_eq!(map_key(ctrl('e')), Err(Signal::Debug));
        assert_eq!(map_key(ctrl('E')), Err(Signal::Debug));
    }

    /// A CPU with no ROM running `program` at 0000, on the real port map.
    fn machine(program: &[u8]) -> (Intel8080, std::rc::Rc<RefCell<Console>>, tempfile::TempDir) {
        let dir = tempfile::tempdir().unwrap();
        let mut cpu = Intel8080::new();
        let (bus, console) = build_bus(dir.path(), mailbox::local_time, AskConfig::default());
        *cpu.io_bus_mut() = bus;
        cpu.load_program(program, 0x0000);
        (cpu, console, dir)
    }

    /// Each pump of the returned input source plays the next batch of keys on its no-wait
    /// call; an idle wait brings nothing. Plays `batches`, then empty pumps. Panics if the
    /// run loop is still going 100 pumps later, so a regression fails the test instead of
    /// hanging it.
    fn script(batches: Vec<Vec<KeyEvent>>) -> impl FnMut(Duration) -> (Vec<u8>, Option<Signal>) {
        let mut batches = batches.into_iter();
        let mut idle = 0;
        move |wait| match if wait.is_zero() { batches.next() } else { Some(vec![]) } {
            Some(batch) => pump(batch.into_iter()),
            None => {
                idle += 1;
                assert!(idle <= 100, "run loop did not exit within 100 pumps after the script");
                (vec![], None)
            }
        }
    }

    /// Runs the loop with a fresh debugger, halts exiting, and no stops expected.
    fn run(cpu: &mut Intel8080, console: &RefCell<Console>, keys: impl FnMut(Duration) -> (Vec<u8>, Option<Signal>), out: &mut Vec<u8>) -> Exit {
        let no_stop = |_: &mut Intel8080, _: &mut Debugger, reason: &str| -> Flow { panic!("stop: {}", reason) };
        run_loop(cpu, console, &mut Debugger::new(), keys, no_stop, false, out)
    }

    #[test]
    fn scripted_keys_reach_in_01() {
        // IN 01 / MOV B,A / IN 01 / MOV C,A / IN 01 / MOV D,A / IN 01 / MOV E,A / HLT
        let (mut cpu, console, _dir) =
            machine(&[0xDB, 0x01, 0x47, 0xDB, 0x01, 0x4F, 0xDB, 0x01, 0x57, 0xDB, 0x01, 0x5F, 0x76]);
        let keys = script(vec![vec![ctrl('a'), key(KeyCode::Esc, KeyModifiers::NONE), ch('\u{e9}')]]);
        assert_eq!(run(&mut cpu, &console, keys, &mut Vec::new()), Exit::Halted);
        assert_eq!((cpu.b, cpu.c, cpu.d, cpu.e), (0x01, 0x1B, 0xC3, 0xA9));
        assert_eq!(cpu.pc, 0x000D, "PC is the address after the HLT");
    }

    #[test]
    fn ctrl_c_quits_a_program_that_never_reads_the_console() {
        // JMP 0000
        let (mut cpu, console, _dir) = machine(&[0xC3, 0x00, 0x00]);
        let keys = script(vec![vec![], vec![ch('x'), ctrl('c')]]);
        assert_eq!(run(&mut cpu, &console, keys, &mut Vec::new()), Exit::Quit);
        assert_eq!(cpu.cycles, 10 * PUMP_STEPS, "quit within one pump interval");
        assert_eq!(console.borrow_mut().read(0x01), b'x');
        assert_eq!(console.borrow_mut().read(0x02), 0x02, "Ctrl-C reached the FIFO");
    }

    #[test]
    fn output_is_drained_8_bit_transparent() {
        // MVI A,80h / OUT 00 / MVI A,0Ah / OUT 00 / HLT
        let (mut cpu, console, _dir) = machine(&[0x3E, 0x80, 0xD3, 0x00, 0x3E, 0x0A, 0xD3, 0x00, 0x76]);
        let mut out = Vec::new();
        assert_eq!(run(&mut cpu, &console, script(vec![]), &mut out), Exit::Halted);
        assert_eq!(out, [0x80, 0x0A]);
        assert!(console.borrow().output().is_empty());
    }

    #[test]
    fn ctrl_e_stops_after_the_keys_before_it() {
        // IN 01 / JMP 0000
        let (mut cpu, console, _dir) = machine(&[0xDB, 0x01, 0xC3, 0x00, 0x00]);
        let keys = script(vec![vec![], vec![ch('x'), ctrl('e'), ch('y')], vec![ctrl('c')]]);
        let mut stops = Vec::new();
        let prompt = |cpu: &mut Intel8080, dbg: &mut Debugger, reason: &str| {
            stops.push((reason.to_string(), cpu.cycles, dbg.command(cpu, "r").1));
            Flow::Resume
        };
        let exit = run_loop(&mut cpu, &console, &mut Debugger::new(), keys, prompt, false, &mut Vec::new());
        assert_eq!(exit, Exit::Quit);
        assert_eq!(stops.len(), 1);
        let (reason, cycles, regs) = &stops[0];
        assert_eq!(reason, "ctrl-e");
        assert_eq!(*cycles, 10 * PUMP_STEPS, "stopped at the pump, before another step");
        assert!(regs.starts_with("PC=0000 "), "{}", regs);
        // 'x', before Ctrl-E, reached the FIFO before the stop.
        assert_eq!(console.borrow_mut().read(0x02) & 0x01, 0x01);
        cpu.execute_one();
        assert_eq!(cpu.a, b'x');
    }

    #[test]
    fn a_stop_report_starts_on_its_own_line() {
        // MVI A,41h / OUT 00 / NOP / MVI A,0Ah / OUT 00 / JMP 0004
        let (mut cpu, console, _dir) =
            machine(&[0x3E, 0x41, 0xD3, 0x00, 0x00, 0x3E, 0x0A, 0xD3, 0x00, 0xC3, 0x04, 0x00]);
        let mut dbg = Debugger::new();
        dbg.command(&mut cpu, "b 0004");
        let mut stops = 0;
        let prompt = |cpu: &mut Intel8080, dbg: &mut Debugger, _: &str| {
            stops += 1;
            dbg.command(cpu, if stops < 2 { "c" } else { "q" }).0
        };
        let mut out = Vec::new();
        assert_eq!(run_loop(&mut cpu, &console, &mut dbg, script(vec![]), prompt, false, &mut out), Exit::Quit);
        // First stop after "A": CR LF added. Second stop after LF: nothing added.
        assert_eq!(out, b"A\r\n\n");
    }

    #[test]
    fn a_debugger_stop_prompts_and_q_quits() {
        // NOP / NOP / JMP 0000
        let (mut cpu, console, _dir) = machine(&[0x00, 0x00, 0xC3, 0x00, 0x00]);
        let mut dbg = Debugger::new();
        dbg.command(&mut cpu, "b 0001");
        let mut stops = Vec::new();
        let prompt = |cpu: &mut Intel8080, dbg: &mut Debugger, reason: &str| {
            stops.push((reason.to_string(), cpu.pc));
            dbg.command(cpu, if stops.len() < 3 { "c" } else { "q" }).0
        };
        let exit = run_loop(&mut cpu, &console, &mut dbg, script(vec![]), prompt, false, &mut Vec::new());
        assert_eq!(exit, Exit::Quit);
        assert_eq!(stops, vec![("break 0001".to_string(), 1); 3]);
    }

    #[test]
    fn idle_waits_only_when_nothing_came_the_fifo_is_empty_and_the_8080_polled() {
        for i in 0..8 {
            let (nothing, empty, polled) = (i & 1 != 0, i & 2 != 0, i & 4 != 0);
            assert_eq!(idle_waits(nothing, empty, polled), i == 7, "{} {} {}", nothing, empty, polled);
        }
    }

    #[test]
    fn a_pump_waits_only_while_the_8080_polls_an_empty_fifo() {
        // IN 02 / IN 01 / JMP 0000: polls and reads one byte per loop.
        let (mut cpu, console, _dir) = machine(&[0xDB, 0x02, 0xDB, 0x01, 0xC3, 0x00, 0x00]);
        let mut waits = Vec::new();
        let mut batches = vec![vec![], vec![ch('a'), ch('b')], vec![], vec![ctrl('c')]].into_iter();
        let keys = |wait: Duration| {
            waits.push((wait, console.borrow().has_input()));
            pump(batches.next().unwrap().into_iter())
        };
        assert_eq!(run(&mut cpu, &console, keys, &mut Vec::new()), Exit::Quit);
        let zero = Duration::ZERO;
        assert_eq!(
            waits,
            [
                (zero, false),      // pump 1: nothing, but no step has run, so no poll yet
                (zero, false),      // pump 2: "ab" arrives
                (zero, false),      // pump 3: the 8080 read both and polled, nothing comes...
                (IDLE_WAIT, false), // ...so it waits, and Ctrl-C arrives
            ]
        );

        // Polling, but a byte stays in the FIFO: no wait.
        // IN 02 / JMP 0000
        let (mut cpu, console, _dir) = machine(&[0xDB, 0x02, 0xC3, 0x00, 0x00]);
        console.borrow_mut().push_input(b"x");
        let mut waits = Vec::new();
        let mut batches = vec![vec![], vec![], vec![ctrl('c')]].into_iter();
        let keys = |wait: Duration| {
            waits.push(wait);
            pump(batches.next().unwrap().into_iter())
        };
        assert_eq!(run(&mut cpu, &console, keys, &mut Vec::new()), Exit::Quit);
        assert_eq!(waits, [Duration::ZERO; 3]);
    }

    #[test]
    fn a_compute_bound_program_never_waits() {
        // LXI B,0000 / DCX B / MOV A,B / ORA C / JNZ 0003 / HLT: 262,144 steps, no I/O.
        let (mut cpu, console, _dir) = machine(&[0x01, 0x00, 0x00, 0x0B, 0x78, 0xB1, 0xC2, 0x03, 0x00, 0x76]);
        let mut pumps = 0;
        let keys = |wait: Duration| {
            assert_eq!(wait, Duration::ZERO, "idle wait in a compute loop");
            pumps += 1;
            (vec![], None)
        };
        assert_eq!(run(&mut cpu, &console, keys, &mut Vec::new()), Exit::Halted);
        assert_eq!(pumps, 27);
    }

    #[test]
    fn halt_prompts_only_in_an_interactive_run() {
        assert!(halt_prompts(true, false));
        assert!(!halt_prompts(true, true), "scripted");
        assert!(!halt_prompts(false, false), "piped");
        assert!(!halt_prompts(false, true));
    }

    #[test]
    fn a_halt_opens_the_prompt_when_asked_and_q_quits() {
        // MVI A,41h / OUT 00 / HLT
        let (mut cpu, console, _dir) = machine(&[0x3E, 0x41, 0xD3, 0x00, 0x76]);
        let mut stops = Vec::new();
        let prompt = |cpu: &mut Intel8080, dbg: &mut Debugger, reason: &str| {
            stops.push((reason.to_string(), dbg.command(cpu, "r").1));
            // c on a halted CPU stops again at once: v1 has nothing to wake it.
            dbg.command(cpu, if stops.len() < 2 { "c" } else { "q" }).0
        };
        let mut out = Vec::new();
        let exit = run_loop(&mut cpu, &console, &mut Debugger::new(), script(vec![]), prompt, true, &mut out);
        assert_eq!(exit, Exit::Quit);
        assert_eq!(out, b"A\r\n", "the report starts on its own line");
        assert_eq!(stops.len(), 2);
        for (reason, regs) in &stops {
            assert_eq!(reason, "halt");
            assert!(regs.starts_with("PC=0005 ") && regs.ends_with(" HLT\n"), "{}", regs);
        }
    }

    #[test]
    fn a_bad_script_line_ends_the_run() {
        // JMP 0000
        let (mut cpu, console, _dir) = machine(&[0xC3, 0x00, 0x00]);
        let mut dbg = Debugger::new();
        dbg.command(&mut cpu, "b 0000");
        let prompt = |_: &mut Intel8080, _: &mut Debugger, _: &str| Flow::Error;
        let exit = run_loop(&mut cpu, &console, &mut dbg, script(vec![]), prompt, false, &mut Vec::new());
        assert_eq!(exit, Exit::BadScript);
    }
}
