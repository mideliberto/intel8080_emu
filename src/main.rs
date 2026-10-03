// main.rs - The host side of the emulator (ARCHITECTURE 7): terminal, key map, run loop.

use std::cell::RefCell;
use std::io::{IsTerminal, Read, Write};
use std::path::Path;
use std::sync::mpsc;
use std::time::Duration;

use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use crossterm::terminal::{disable_raw_mode, enable_raw_mode};

use intel8080_emu::io::build_bus;
use intel8080_emu::io::devices::console::Console;
use intel8080_emu::Intel8080;

const BUILD_TIMESTAMP: &str = env!("BUILD_TIMESTAMP");

/// Host input is read at least this often, in executed steps (ARCHITECTURE 7.2).
const PUMP_STEPS: u32 = 10_000;

#[derive(Debug, PartialEq)]
enum Exit {
    Halted,
    Quit,
}

fn main() {
    println!("8080 Emulator");
    println!("Built: {}", BUILD_TIMESTAMP);

    // new() leaves the CPU in its RESET state: PC=0000, ROM overlay set.
    let mut cpu = Intel8080::new();
    let (bus, console) = build_bus(Path::new("storage"));
    *cpu.io_bus_mut() = bus;
    cpu.load_rom_from_file(Path::new("rom/monitor.bin")).expect("Failed to load ROM");

    let mut stdout = std::io::stdout();
    let exit = if std::io::stdin().is_terminal() && enable_raw_mode().is_ok() {
        let default_hook = std::panic::take_hook();
        std::panic::set_hook(Box::new(move |info| {
            let _ = disable_raw_mode();
            default_hook(info);
        }));
        let exit = run_loop(&mut cpu, &console, || pump(std::iter::from_fn(host_key)), &mut stdout);
        let _ = disable_raw_mode();
        exit
    } else {
        // Not a terminal: stdin bytes go to the console unmapped. Ctrl-C is the shell's SIGINT.
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
        run_loop(&mut cpu, &console, || Some(rx.try_iter().flatten().collect()), &mut stdout)
    };

    // v1 has no interrupt source, so a halt is final (ARCHITECTURE 7.2).
    match exit {
        Exit::Halted => println!("\nHLT at PC={:04X}", cpu.pc),
        Exit::Quit => println!(),
    }
}

/// Runs until the CPU halts or `input` returns None (quit). `input` returns the
/// console bytes that arrived since its last call; it is called before the first
/// step and then every PUMP_STEPS steps. Console output is drained to `out` as often.
fn run_loop(
    cpu: &mut Intel8080,
    console: &RefCell<Console>,
    mut input: impl FnMut() -> Option<Vec<u8>>,
    out: &mut impl Write,
) -> Exit {
    loop {
        // OUT 00 never waits: a host write error loses the bytes, like a missing terminal.
        let _ = out.write_all(&console.borrow_mut().take_output());
        let _ = out.flush();
        if cpu.halted {
            return Exit::Halted;
        }
        match input() {
            Some(bytes) => console.borrow_mut().push_input(&bytes),
            None => return Exit::Quit,
        }
        for _ in 0..PUMP_STEPS {
            if cpu.halted {
                break;
            }
            cpu.execute_one();
        }
    }
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

/// Maps keys to console bytes in order. None: Ctrl-C was pressed.
fn pump(keys: impl Iterator<Item = KeyEvent>) -> Option<Vec<u8>> {
    let mut bytes = Vec::new();
    for key in keys {
        bytes.extend(map_key(key)?);
    }
    Some(bytes)
}

/// The host key map (ARCHITECTURE 7.1). None is Ctrl-C (quit); an empty Vec is a dropped key.
fn map_key(key: KeyEvent) -> Option<Vec<u8>> {
    let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
    let alt = key.modifiers.contains(KeyModifiers::ALT);
    Some(match key.code {
        KeyCode::Enter => vec![0x0D],
        KeyCode::Backspace => vec![0x08],
        KeyCode::Tab => vec![0x09],
        KeyCode::Esc => vec![0x1B],
        KeyCode::Char(c) if ctrl && !alt && c.is_ascii_alphabetic() => match c.to_ascii_lowercase() {
            'c' => return None,
            'e' => vec![], // reserved for the Phase 10 debugger hotkey
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
            (ctrl('e'), &[]),
            (ctrl('E'), &[]),
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
            assert_eq!(map_key(*k).as_deref(), Some(*bytes), "{:?}", k);
        }
        assert_eq!(map_key(ctrl('c')), None);
        assert_eq!(map_key(ctrl('C')), None);
    }

    /// A CPU with no ROM running `program` at 0000, on the real port map.
    fn machine(program: &[u8]) -> (Intel8080, std::rc::Rc<RefCell<Console>>, tempfile::TempDir) {
        let dir = tempfile::tempdir().unwrap();
        let mut cpu = Intel8080::new();
        let (bus, console) = build_bus(dir.path());
        *cpu.io_bus_mut() = bus;
        cpu.load_program(program, 0x0000);
        (cpu, console, dir)
    }

    /// Each call of the returned input source pumps the next batch of keys.
    /// Plays `batches`, then empty pumps. Panics if the run loop is still going 100 pumps
    /// later, so a regression fails the test instead of hanging it.
    fn script(batches: Vec<Vec<KeyEvent>>) -> impl FnMut() -> Option<Vec<u8>> {
        let mut batches = batches.into_iter();
        let mut idle = 0;
        move || match batches.next() {
            Some(batch) => pump(batch.into_iter()),
            None => {
                idle += 1;
                assert!(idle <= 100, "run loop did not exit within 100 pumps after the script");
                Some(vec![])
            }
        }
    }

    #[test]
    fn scripted_keys_reach_in_01() {
        // IN 01 / MOV B,A / IN 01 / MOV C,A / IN 01 / MOV D,A / IN 01 / MOV E,A / HLT
        let (mut cpu, console, _dir) =
            machine(&[0xDB, 0x01, 0x47, 0xDB, 0x01, 0x4F, 0xDB, 0x01, 0x57, 0xDB, 0x01, 0x5F, 0x76]);
        let keys = script(vec![vec![ctrl('a'), key(KeyCode::Esc, KeyModifiers::NONE), ch('\u{e9}')]]);
        assert_eq!(run_loop(&mut cpu, &console, keys, &mut Vec::new()), Exit::Halted);
        assert_eq!((cpu.b, cpu.c, cpu.d, cpu.e), (0x01, 0x1B, 0xC3, 0xA9));
        assert_eq!(cpu.pc, 0x000D, "PC is the address after the HLT");
    }

    #[test]
    fn ctrl_c_quits_a_program_that_never_reads_the_console() {
        // JMP 0000
        let (mut cpu, console, _dir) = machine(&[0xC3, 0x00, 0x00]);
        let keys = script(vec![vec![], vec![ch('x'), ctrl('c')]]);
        assert_eq!(run_loop(&mut cpu, &console, keys, &mut Vec::new()), Exit::Quit);
        assert_eq!(cpu.cycles, 10 * PUMP_STEPS as u64, "quit within one pump interval");
        assert_eq!(console.borrow_mut().read(0x02), 0x02, "Ctrl-C reached the FIFO");
    }

    #[test]
    fn output_is_drained_8_bit_transparent() {
        // MVI A,80h / OUT 00 / MVI A,0Ah / OUT 00 / HLT
        let (mut cpu, console, _dir) = machine(&[0x3E, 0x80, 0xD3, 0x00, 0x3E, 0x0A, 0xD3, 0x00, 0x76]);
        let mut out = Vec::new();
        assert_eq!(run_loop(&mut cpu, &console, script(vec![]), &mut out), Exit::Halted);
        assert_eq!(out, [0x80, 0x0A]);
        assert!(console.borrow().output().is_empty());
    }
}
