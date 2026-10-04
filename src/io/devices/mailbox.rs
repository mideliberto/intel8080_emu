// mailbox.rs - Service Mailbox (ports 10-13), DEVICE_SPECS 8.
//
// The 8080 writes a text command, executes it, polls status and pops the response.
// TIME, ASM and DIS complete within the execute access. GET and ASK are background
// commands (DEVICE_SPECS 8, Background commands): the worker is a `curl` child process
// that the device spawns at execute, checks only when the 8080 reads IN 12 in BUSY (one
// non-blocking pipe read or one non-blocking wait), and kills and reaps on execute,
// clear and Drop (RESET). No thread: the device stays on the bus thread (PI_DAEMON 1).
// ASK's own parts (its prompt rules, argument list, stdin and the SSE reader) are in
// ask.rs; its worker is spawned here as GET's is (`curl`), and its pipe reads go
// through the reader, which holds text back while a later byte could change it.
//
// The GET worker (DEVICE_SPECS 8, GET client), pinned by `curl_argument_list` below:
//
//   /usr/bin/curl -q -s -f -L --max-redirs 5 --proto =http,https --proto-redir =http,https -g
//                 --connect-timeout 10 --speed-limit 1 --speed-time 30
//                 [-N | --max-filesize 16777215 -o <storage dir>/~FILE] --url <url>
//
//   -q              first: ignore any .curlrc.
//   -s              no progress meter (stderr is /dev/null anyway).
//   -f              a final status of 400 or above: no body, nonzero exit (83).
//   -L, --max-redirs 5, --proto-redir =http,https
//                   follow at most 5 redirects, only to http or https (a sixth: 83).
//   --proto =http,https
//                   http and https only, belt and braces with the 82 check.
//   -g              no URL globbing: [ ] { } are literal.
//   --connect-timeout 10
//                   each connection (DNS, TCP, TLS handshake) at most 10 s (Time limits).
//   --speed-limit 1 --speed-time 30
//                   under 1 byte/s averaged over 30 s fails (Time limits, the stall).
//   -N              stream form: no output buffering, so bytes reach the pipe as they
//                   arrive (without it curl holds about 4 KB, and N prints nothing).
//   --max-filesize 16777215
//                   file form: a body over FFFFFF bytes fails (exit 63) as soon as curl
//                   sees it, chunked included (curl 8.4.0+), so nothing checks after exit.
//   -o ~FILE        file form: the body goes to the temporary file; curl creates no file
//                   when there is no body (304), so the device removes a stale ~FILE first.
//   --url <url>     the 82 check guarantees <url> starts with http, so it is never an option.
//
// stdin and stderr are /dev/null; stdout is a pipe (stream form, made non-blocking) or
// /dev/null (file form). For ASK, stdin is a pipe that carries curl's config (the key and
// the body) and is closed at execute. Every worker's environment is empty (env_clear): no
// proxy variables, no CURL_CA_BUNDLE or SSL_CERT_FILE, no HOME, no ANTHROPIC_API_KEY. On
// Linux a pre_exec sets the worker's cores and PR_SET_PDEATHSIG (PI_DAEMON 1).

use crate::disasm;
use crate::io::devices::ask::{self, AskConfig};
use crate::io::devices::storage;
use crate::io::IoDevice;
use std::collections::VecDeque;
use std::ffi::OsString;
use std::fs::{File, OpenOptions};
use std::io::{ErrorKind, Read, Write};
use std::os::unix::io::AsRawFd;
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdout, Command, Stdio};

const IDLE: u8 = 0x00;
const BUSY: u8 = 0x01;
const AVAIL: u8 = 0x02;
const DONE: u8 = 0x03;
const ERR_UNKNOWN: u8 = 0x80;
const ERR_OVERFLOW: u8 = 0x81;
const ERR_ARGS: u8 = 0x82;
const ERR_SERVICE: u8 = 0x83;

/// The command buffer holds at most this many bytes; exactly this many is accepted.
const COMMAND_MAX: usize = 128;

/// The GET and ASK worker (DEVICE_SPECS 8, GET client, ASK service).
const CURL: &str = "/usr/bin/curl";

/// One check reads at most this many bytes of the worker's output.
const READ_MAX: usize = 4096;

/// Local time as (year, month, day, hour, minute, second), or None when the clock is
/// not set (83). Fields must be in range (year 0-9999, month 1-12, ...); the device
/// does not check them. A plain fn, so a test can pass a fixed or a failing clock. The device
/// formats it, so the 19-byte response is the same whatever supplies the time.
pub type Clock = fn() -> Option<(u16, u8, u8, u8, u8, u8)>;

pub struct Mailbox {
    command: Vec<u8>,
    overflow: bool,
    status: u8,
    response: VecDeque<u8>,
    clock: Clock,
    /// The storage directory: `GET > FILE` writes there.
    dir: PathBuf,
    /// The running background request, if any.
    request: Option<Request>,
    /// The status after the last response byte is popped once the request has ended:
    /// DONE, or for ASK 83 (the end comes after the last byte, DEVICE_SPECS 8, ASK).
    end: u8,
    /// ASK's key, endpoint and total limit.
    ask: AskConfig,
}

/// A running GET or ASK: its worker and where its output goes.
struct Request {
    child: Child,
    /// Stream form and ASK: the worker's stdout, non-blocking; None once it reached EOF.
    out: Option<ChildStdout>,
    /// File form: (`~FILE`, `FILE`), both in the storage directory.
    file: Option<(PathBuf, PathBuf)>,
    /// ASK: the stream goes through the reader, not straight to the response.
    reader: Option<ask::Reader>,
}

impl Mailbox {
    /// Power-on state: IDLE, command buffer and response empty, overflow flag clear,
    /// no request running.
    pub fn new(clock: Clock, dir: PathBuf, ask: AskConfig) -> Self {
        Mailbox { command: Vec::new(), overflow: false, status: IDLE, response: VecDeque::new(), clock, dir,
            request: None, end: DONE, ask }
    }

    /// Aborts any running request, then takes the buffer as the new request; the old
    /// response is discarded.
    fn execute(&mut self) {
        self.abort();
        let command = std::mem::take(&mut self.command);
        let result = if std::mem::take(&mut self.overflow) { Err(ERR_OVERFLOW) } else { self.run(&command) };
        self.response.clear();
        self.end = DONE;
        match result {
            Ok(Some(bytes)) => {
                self.status = if bytes.is_empty() { DONE } else { AVAIL };
                self.response = bytes.into();
            }
            Ok(None) => self.status = BUSY,
            Err(code) => self.status = code,
        }
    }

    /// The command word is the bytes before the first 20h, matched exactly; the argument
    /// string is everything after it (None when there is no 20h). Ok(None): a background
    /// request started.
    fn run(&mut self, command: &[u8]) -> Result<Option<Vec<u8>>, u8> {
        let (word, args) = match command.iter().position(|&b| b == b' ') {
            Some(i) => (&command[..i], Some(&command[i + 1..])),
            None => (command, None),
        };
        match (word, args) {
            (b"TIME", None) => (self.clock)()
                .map(|(y, mo, d, h, mi, s)| format!("{:04}-{:02}-{:02} {:02}:{:02}:{:02}", y, mo, d, h, mi, s).into_bytes())
                .ok_or(ERR_SERVICE)
                .map(Some),
            (b"ASM", Some(args)) => std::str::from_utf8(args).ok().and_then(disasm::assemble).ok_or(ERR_ARGS).map(Some),
            (b"DIS", Some(args)) => dis(args).ok_or(ERR_ARGS).map(Some),
            (b"GET", Some(args)) => self.get(args).map(|()| None),
            (b"ASK", Some(args)) => self.ask(args).map(|()| None),
            (b"TIME" | b"ASM" | b"DIS" | b"GET" | b"ASK", _) => Err(ERR_ARGS),
            _ => Err(ERR_UNKNOWN),
        }
    }

    /// GET: the grammar (82), then the worker. A worker that cannot start gives 83.
    fn get(&mut self, args: &[u8]) -> Result<(), u8> {
        let tokens: Vec<&[u8]> = args.split(|&b| b == b' ').filter(|t| !t.is_empty()).collect();
        let (url, name) = match tokens[..] {
            [url] => (url, None),
            [url, b">", file] => (url, Some(storage::file_name(file).ok_or(ERR_ARGS)?)),
            _ => return Err(ERR_ARGS),
        };
        let rest = url.strip_prefix(b"http://").or_else(|| url.strip_prefix(b"https://")).ok_or(ERR_ARGS)?;
        if rest.is_empty() || !url.iter().all(|b| (0x21..=0x7E).contains(b)) {
            return Err(ERR_ARGS);
        }
        // Every byte is 21-7E, so this can't fail.
        let url = std::str::from_utf8(url).unwrap();
        let file = name.map(|name| (self.dir.join(format!("~{}", name)), self.dir.join(name)));
        if let Some((tmp, _)) = &file {
            let _ = std::fs::remove_file(tmp); // a leftover from a restart or power loss
        }
        let mut cmd = curl(curl_args(url, file.as_ref().map(|(tmp, _)| tmp.as_path())));
        cmd.stdin(Stdio::null()).stdout(if file.is_some() { Stdio::null() } else { Stdio::piped() });
        let mut child = cmd.spawn().map_err(|_| ERR_SERVICE)?;
        let out = child.stdout.take();
        self.start(Request { child, out, file, reader: None })
    }

    /// ASK: the prompt rules (82), the key (83 with no worker), then the worker, with its
    /// config written to its stdin and the pipe closed.
    fn ask(&mut self, args: &[u8]) -> Result<(), u8> {
        let prompt = ask::validate(args)?;
        let key = self.ask.key.as_deref().ok_or(ERR_SERVICE)?;
        let mut cmd = curl(ask::argv(&self.ask));
        cmd.stdin(Stdio::piped()).stdout(Stdio::piped());
        let mut child = cmd.spawn().map_err(|_| ERR_SERVICE)?;
        if let Some(mut input) = child.stdin.take() {
            // Under 16 KiB, so it never waits on an empty pipe; an error is curl's exit status's business.
            let _ = input.write_all(ask::stdin(key, &ask::body(&prompt)).as_bytes());
        }
        let out = child.stdout.take();
        self.start(Request { child, out, file: None, reader: Some(ask::Reader::default()) })
    }

    /// The worker is running: make its pipe non-blocking and go to BUSY.
    fn start(&mut self, mut request: Request) -> Result<(), u8> {
        if request.out.as_ref().is_some_and(|out| !nonblocking(out)) {
            request.kill();
            return Err(ERR_SERVICE);
        }
        self.request = Some(request);
        Ok(())
    }

    /// One check of the running request, at IN 12 in BUSY: one non-blocking read of the
    /// worker's output, or one non-blocking wait for its exit (after EOF, or in the file form).
    fn check(&mut self) {
        let Some(request) = &mut self.request else { return };
        if let Some(out) = &mut request.out {
            let mut buf = [0u8; READ_MAX];
            match out.read(&mut buf) {
                Ok(0) => request.out = None, // EOF: wait for the exit below
                Ok(n) => {
                    match &mut request.reader {
                        Some(reader) => {
                            let mut text = Vec::new();
                            reader.feed(&buf[..n], &mut text);
                            self.response.extend(text);
                        }
                        None => self.response.extend(&buf[..n]),
                    }
                    if !self.response.is_empty() {
                        self.status = AVAIL;
                    }
                    return;
                }
                Err(e) if matches!(e.kind(), ErrorKind::WouldBlock | ErrorKind::Interrupted) => return,
                // For ASK this drops the held-back text; a pipe read doesn't fail this way in practice.
                Err(_) => return self.fail(),
            }
        }
        match request.child.try_wait() {
            Ok(None) => {}
            Ok(Some(exit)) if request.reader.is_some() => {
                // ASK: the held-back text, then DONE or 83 after its last byte.
                let mut reader = request.reader.take().unwrap();
                self.request = None;
                let mut text = Vec::new();
                let end = if reader.finish(&mut text) && exit.success() { DONE } else { ERR_SERVICE };
                self.response.extend(text);
                if self.response.is_empty() {
                    self.status = end;
                } else {
                    self.status = AVAIL;
                    self.end = end;
                }
            }
            Ok(Some(exit)) if exit.success() => {
                let file = self.request.take().and_then(|r| r.file);
                self.finish(file);
            }
            _ => self.fail(), // try_wait errors: as above, not seen in practice
        }
    }

    /// The worker exited 0. Stream form: DONE (the response is empty in BUSY). File form:
    /// fsync `~FILE` (created empty if curl made none: an empty body), rename it over
    /// `FILE`, fsync the directory, respond with the length as 6 hex digits.
    fn finish(&mut self, file: Option<(PathBuf, PathBuf)>) {
        let Some((tmp, file)) = file else {
            self.status = DONE;
            return;
        };
        let install = || -> std::io::Result<u64> {
            let f = OpenOptions::new().write(true).create(true).truncate(false).open(&tmp)?;
            f.sync_all()?;
            let len = f.metadata()?.len();
            std::fs::rename(&tmp, &file)?;
            File::open(&self.dir)?.sync_all()?;
            Ok(len)
        };
        match install() {
            Ok(len) => {
                self.response = format!("{:06X}", len).into_bytes().into();
                self.status = AVAIL;
            }
            Err(_) => {
                let _ = std::fs::remove_file(&tmp);
                self.status = ERR_SERVICE;
            }
        }
    }

    /// The request failed: 83, unread bytes discarded, the worker and `~FILE` gone.
    fn fail(&mut self) {
        self.abort();
        self.response.clear();
        self.status = ERR_SERVICE;
    }

    /// Abort a running request (execute, clear, Drop): kill and reap the worker, remove
    /// its temporary file. Nothing it did becomes visible.
    fn abort(&mut self) {
        if let Some(mut request) = self.request.take() {
            request.kill();
        }
    }
}

impl Request {
    fn kill(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
        if let Some((tmp, _)) = &self.file {
            let _ = std::fs::remove_file(tmp);
        }
    }
}

impl Drop for Mailbox {
    fn drop(&mut self) {
        self.abort();
    }
}

/// A worker: `/usr/bin/curl` with `args`, an empty environment, stderr /dev/null, and on
/// Linux its cores and lifetime. The caller sets stdin and stdout.
fn curl(args: Vec<OsString>) -> Command {
    let mut cmd = Command::new(CURL);
    cmd.args(args).env_clear().stderr(Stdio::null());
    #[cfg(target_os = "linux")]
    worker_cores_and_lifetime(&mut cmd);
    cmd
}

/// The GET worker's argument list (module header).
fn curl_args(url: &str, tmp: Option<&Path>) -> Vec<OsString> {
    let mut args: Vec<OsString> = ["-q", "-s", "-f", "-L", "--max-redirs", "5", "--proto", "=http,https",
        "--proto-redir", "=http,https", "-g", "--connect-timeout", "10", "--speed-limit", "1", "--speed-time", "30"]
        .iter()
        .map(OsString::from)
        .collect();
    match tmp {
        None => args.push("-N".into()),
        Some(tmp) => args.extend(["--max-filesize".into(), "16777215".into(), "-o".into(), tmp.as_os_str().to_owned()]),
    }
    args.extend(["--url".into(), url.into()]);
    args
}

/// Makes the stream-form pipe non-blocking, so a check never waits.
fn nonblocking(out: &ChildStdout) -> bool {
    let fd = out.as_raw_fd();
    // SAFETY: fcntl on a pipe fd this process owns.
    unsafe {
        let flags = libc::fcntl(fd, libc::F_GETFL);
        flags >= 0 && libc::fcntl(fd, libc::F_SETFL, flags | libc::O_NONBLOCK) >= 0
    }
}

/// PI_DAEMON 1, the worker's cores and lifetime: affinity to the online CPUs outside
/// this thread's mask (left alone when that set is empty), and PR_SET_PDEATHSIG so the
/// worker dies with the thread that spawned it. The mask is read here, before the fork.
#[cfg(target_os = "linux")]
fn worker_cores_and_lifetime(cmd: &mut Command) {
    use std::os::unix::process::CommandExt;
    let size = std::mem::size_of::<libc::cpu_set_t>();
    // SAFETY: cpu_set_t is plain data; sched_getaffinity and sysconf only read and write it.
    let (mut own, mut others): (libc::cpu_set_t, libc::cpu_set_t) = unsafe { (std::mem::zeroed(), std::mem::zeroed()) };
    let online = unsafe { libc::sysconf(libc::_SC_NPROCESSORS_ONLN) }.max(0) as usize;
    let mut any = false;
    if unsafe { libc::sched_getaffinity(0, size, &mut own) } == 0 {
        for cpu in 0..online.min(libc::CPU_SETSIZE as usize) {
            if !unsafe { libc::CPU_ISSET(cpu, &own) } {
                unsafe { libc::CPU_SET(cpu, &mut others) };
                any = true;
            }
        }
    }
    // SAFETY: the closure runs in the child between fork and exec and makes only
    // async-signal-safe calls (sched_setaffinity, prctl); failures are ignored.
    unsafe {
        cmd.pre_exec(move || {
            if any {
                libc::sched_setaffinity(0, size, &others);
            }
            libc::prctl(libc::PR_SET_PDEATHSIG, libc::SIGKILL);
            Ok(())
        });
    }
}

impl IoDevice for Mailbox {
    fn read(&mut self, port: u8) -> u8 {
        match port {
            0x12 => {
                if self.status == BUSY {
                    self.check();
                }
                self.status
            }
            0x13 if self.status == AVAIL => {
                let byte = self.response.pop_front().unwrap_or(0x00);
                if self.response.is_empty() {
                    // Still running: back to BUSY, without a check (only IN 12 checks).
                    self.status = if self.request.is_some() { BUSY } else { self.end };
                }
                byte
            }
            0x13 => 0x00,
            _ => 0xFF,
        }
    }

    fn write(&mut self, port: u8, value: u8) {
        match (port, value) {
            (0x10, _) if self.command.len() < COMMAND_MAX => self.command.push(value),
            (0x10, _) => self.overflow = true,
            (0x11, 0x01) => self.execute(),
            // The old value's Drop aborts a running request.
            (0x11, 0x02) => *self = Mailbox::new(self.clock, self.dir.clone(), self.ask.clone()),
            _ => {}
        }
    }
}

/// DIS: `AAAA B0 B1 B2` (exactly 13 bytes) -> the length byte, the DIS line, CR LF.
fn dis(args: &[u8]) -> Option<Vec<u8>> {
    if args.len() != 13 || [4, 7, 10].iter().any(|&i| args[i] != b' ') {
        return None;
    }
    // from_str_radix alone would take a leading '+'.
    let hex = |r: std::ops::Range<usize>| -> Option<u16> {
        let digits = std::str::from_utf8(&args[r]).ok()?;
        digits.bytes().all(|b| b.is_ascii_hexdigit()).then(|| u16::from_str_radix(digits, 16).unwrap())
    };
    let bytes = [hex(5..7)? as u8, hex(8..10)? as u8, hex(11..13)? as u8];
    let (line, len) = disasm::line(hex(0..4)?, bytes, |_| None);
    Some([&[len as u8][..], line.as_bytes(), b"\r\n"].concat())
}

/// The host's local time (the emulator uses the host clock, DEVICE_SPECS 8). None if
/// the time is before 1970, doesn't fit time_t (32-bit time_t after 2038) or
/// localtime_r fails; the host has no "clock not set".
pub fn local_time() -> Option<(u16, u8, u8, u8, u8, u8)> {
    let secs = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).ok()?.as_secs();
    // time_t by inference: libc deprecates naming it on musl (the Pi daemon's target).
    let secs = secs.try_into().ok()?;
    // SAFETY: tm is plain data; localtime_r writes only to it and returns null on failure.
    let mut tm: libc::tm = unsafe { std::mem::zeroed() };
    if unsafe { libc::localtime_r(&secs, &mut tm) }.is_null() {
        return None;
    }
    let year = u16::try_from(tm.tm_year + 1900).ok().filter(|&y| y <= 9999)?; // > 9999 is "not set" (DEVICE_SPECS 8)
    Some((year, tm.tm_mon as u8 + 1, tm.tm_mday as u8, tm.tm_hour as u8, tm.tm_min as u8, tm.tm_sec as u8))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn curl_argument_list() {
        // The GET worker's flags (module header): the time limits, redirects, schemes and
        // the size limit live here, so this test is what pins them.
        let common = "-q -s -f -L --max-redirs 5 --proto =http,https --proto-redir =http,https -g \
            --connect-timeout 10 --speed-limit 1 --speed-time 30";
        // Element by element: "--max-redirs 5" as one argument would not match.
        let text = |args: Vec<OsString>| args.into_iter().map(|a| a.into_string().unwrap()).collect::<Vec<_>>();
        let expect = |tail: &str| common.split_whitespace().chain(tail.split_whitespace()).map(String::from).collect::<Vec<_>>();
        assert_eq!(text(curl_args("http://h/x", None)), expect("-N --url http://h/x"));
        assert_eq!(text(curl_args("https://h/", Some(Path::new("/s/~BOOK.TXT")))),
            expect("--max-filesize 16777215 -o /s/~BOOK.TXT --url https://h/"));
    }
}
