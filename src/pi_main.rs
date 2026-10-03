// pi_main.rs - pi8080d (PI_DAEMON 10, 16): arguments, the stop flag, the startup order, then
// pi::serve on the board's GPIO (Linux), or with --sim on the simulated board with the 8080
// model on a second thread (Linux or macOS). Everything else is in src/pi/.

#[cfg(not(unix))]
fn main() {
    eprintln!("pi8080d: Unix only");
    std::process::exit(2);
}

#[cfg(unix)]
fn main() {
    if let Err(e) = daemon::run() {
        eprintln!("pi8080d: {}", e);
        std::process::exit(1);
    }
}

#[cfg(unix)]
mod daemon {
    use intel8080_emu::io::devices::mailbox;
    use intel8080_emu::pi::{self, sim::{Bridge, Knobs, SimBoard}};
    use intel8080_emu::Intel8080;
    use std::cell::RefCell;
    use std::fs::File;
    use std::net::TcpListener;
    use std::path::{Path, PathBuf};
    use std::rc::Rc;
    use std::sync::atomic::{AtomicBool, Ordering::Relaxed};

    const USAGE: &str = "usage: pi8080d --storage DIR [--listen ADDR:PORT] [--trace FILE] [--sim FILE]";

    /// The TIME clock (PI_DAEMON 8): NTP-gated on Linux; under --sim elsewhere, local time.
    #[cfg(target_os = "linux")]
    const CLOCK: mailbox::Clock = pi::linux::ntp_local_time;
    #[cfg(not(target_os = "linux"))]
    const CLOCK: mailbox::Clock = mailbox::local_time;

    /// Set by SIGTERM and SIGINT; `serve` sees it at its next check (PI_DAEMON 4.1).
    static STOP: AtomicBool = AtomicBool::new(false);

    extern "C" fn on_signal(_: libc::c_int) {
        STOP.store(true, Relaxed);
    }

    fn usage() -> ! {
        eprintln!("{}", USAGE);
        std::process::exit(2);
    }

    struct Args {
        listen: String,
        storage: PathBuf,
        trace: Option<PathBuf>,
        sim: Option<PathBuf>,
    }

    pub fn run() -> Result<(), String> {
        let mut listen = "127.0.0.1:8080".to_string();
        let (mut storage, mut trace, mut sim) = (None, None, None);
        let mut args = std::env::args().skip(1);
        while let Some(flag) = args.next() {
            let value = args.next().unwrap_or_else(|| usage());
            match flag.as_str() {
                "--listen" => listen = value,
                "--storage" => storage = Some(PathBuf::from(value)),
                "--trace" => trace = Some(PathBuf::from(value)),
                "--sim" => sim = Some(PathBuf::from(value)),
                _ => usage(),
            }
        }
        let a = Args { listen, storage: storage.unwrap_or_else(|| usage()), trace, sim };
        match &a.sim {
            Some(rom) => simulated(&a, rom),
            None => board(&a),
        }
    }

    /// Startup order (PI_DAEMON 10): the pins are released before anything that can fail late.
    #[cfg(target_os = "linux")]
    fn board(a: &Args) -> Result<(), String> {
        let mut gpio = pi::linux::GpioMem::open()?;
        let fsel2 = pi::setup_pins(&gpio)?;
        gpio.request_reset()?;
        let (listener, trace) = open(a, "")?;
        pi::serve(gpio, fsel2, &a.storage, CLOCK, listener, trace, &STOP)
    }

    #[cfg(not(target_os = "linux"))]
    fn board(_: &Args) -> Result<(), String> {
        eprintln!("pi8080d: Linux only (or --sim FILE)");
        std::process::exit(2);
    }

    /// --sim (PI_DAEMON 16): the same startup with the simulated board in place of GpioMem,
    /// and the 8080 started just before `serve`.
    fn simulated(a: &Args, path: &Path) -> Result<(), String> {
        let rom = std::fs::read(path).map_err(|e| format!("--sim {}: {}", path.display(), e))?;
        if rom.len() != 4096 {
            return Err(format!("--sim {}: {} bytes, not 4096", path.display(), rom.len()));
        }
        let board = SimBoard::new(Knobs { wait_timeout: None, ..Knobs::default() });
        let fsel2 = pi::setup_pins(&board)?;
        let (listener, trace) = open(a, &format!(", board simulated, ROM {}", path.display()))?;
        let cpu_board = board.clone();
        std::thread::spawn(move || run_8080(cpu_board, rom));
        pi::serve(board, fsel2, &a.storage, CLOCK, listener, trace, &STOP)
    }

    /// The 8080 thread (PI_DAEMON 16.2): power-on as main.rs does, the Pi window through
    /// the board, run until a halt, then park for good.
    fn run_8080(board: SimBoard, rom: Vec<u8>) {
        let mut cpu = Intel8080::new();
        cpu.load_rom(&rom);
        let bridge = Rc::new(RefCell::new(Bridge(board)));
        for port in 0x00..=0x6F {
            cpu.io_bus_mut().map_port(port, bridge.clone());
        }
        while !cpu.halted {
            cpu.execute_one();
        }
        eprintln!("pi8080d: 8080 halted at PC={:04X}", cpu.pc);
        loop {
            std::thread::park();
        }
    }

    /// Binds the listener, opens the trace, installs the signal handlers and logs the
    /// startup line (PI_DAEMON 10).
    fn open(a: &Args, board: &str) -> Result<(TcpListener, Option<File>), String> {
        let listener = TcpListener::bind(&a.listen).map_err(|e| format!("--listen {}: {}", a.listen, e))?;
        let trace = match &a.trace {
            Some(path) => Some(File::create(path).map_err(|e| format!("--trace {}: {}", path.display(), e))?),
            None => None,
        };
        let handler = on_signal as extern "C" fn(libc::c_int) as libc::sighandler_t;
        // SAFETY: the handler only stores to an atomic.
        unsafe {
            libc::signal(libc::SIGTERM, handler);
            libc::signal(libc::SIGINT, handler);
        }
        let addr = listener.local_addr().map_err(|e| format!("--listen {}: {}", a.listen, e))?;
        eprintln!("pi8080d: console on {}, storage {}, trace {}{}", addr, a.storage.display(),
                  a.trace.as_ref().map_or("off".to_string(), |p| p.display().to_string()), board);
        Ok((listener, trace))
    }
}
