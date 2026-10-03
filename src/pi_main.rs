// pi_main.rs - pi8080d (PI_DAEMON 10): arguments, the stop flag, the startup order, then
// pi::serve. Everything else is in src/pi/. Linux only.

#[cfg(not(target_os = "linux"))]
fn main() {
    eprintln!("pi8080d: Linux only");
    std::process::exit(2);
}

#[cfg(target_os = "linux")]
fn main() {
    if let Err(e) = daemon::run() {
        eprintln!("pi8080d: {}", e);
        std::process::exit(1);
    }
}

#[cfg(target_os = "linux")]
mod daemon {
    use intel8080_emu::pi::{self, linux::{ntp_local_time, GpioMem}};
    use std::fs::File;
    use std::net::TcpListener;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicBool, Ordering::Relaxed};

    const USAGE: &str = "usage: pi8080d --storage DIR [--listen ADDR:PORT] [--trace FILE]";

    /// Set by SIGTERM and SIGINT; `serve` sees it at its next check (PI_DAEMON 4.1).
    static STOP: AtomicBool = AtomicBool::new(false);

    extern "C" fn on_signal(_: libc::c_int) {
        STOP.store(true, Relaxed);
    }

    fn usage() -> ! {
        eprintln!("{}", USAGE);
        std::process::exit(2);
    }

    pub fn run() -> Result<(), String> {
        let mut listen = "127.0.0.1:8080".to_string();
        let mut storage = None;
        let mut trace_path = None;
        let mut args = std::env::args().skip(1);
        while let Some(flag) = args.next() {
            let value = args.next().unwrap_or_else(|| usage());
            match flag.as_str() {
                "--listen" => listen = value,
                "--storage" => storage = Some(PathBuf::from(value)),
                "--trace" => trace_path = Some(PathBuf::from(value)),
                _ => usage(),
            }
        }
        let storage = storage.unwrap_or_else(|| usage());

        // Startup order (PI_DAEMON 10): the pins are released before anything that can fail late.
        let mut gpio = GpioMem::open()?;
        let fsel2 = pi::setup_pins(&gpio)?;
        gpio.request_reset()?;
        let listener = TcpListener::bind(&listen).map_err(|e| format!("--listen {}: {}", listen, e))?;
        let trace = match &trace_path {
            Some(path) => Some(File::create(path).map_err(|e| format!("--trace {}: {}", path.display(), e))?),
            None => None,
        };
        let handler = on_signal as extern "C" fn(libc::c_int) as libc::sighandler_t;
        // SAFETY: the handler only stores to an atomic.
        unsafe {
            libc::signal(libc::SIGTERM, handler);
            libc::signal(libc::SIGINT, handler);
        }
        eprintln!("pi8080d: console on {}, storage {}, trace {}", listen, storage.display(),
                  trace_path.as_ref().map_or("off".to_string(), |p| p.display().to_string()));
        pi::serve(gpio, fsel2, &storage, ntp_local_time, listener, trace, &STOP)
    }
}
