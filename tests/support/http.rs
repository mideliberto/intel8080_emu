// http.rs - the test HTTP server `H` of the GET conformance vectors (DEVICE_SPECS 8)
// and the N vectors (MONITOR_SPEC 6.18.1).
//
// A std TcpListener on 127.0.0.1:0, one thread per connection, canned responses by
// path. A connection whose first bytes are not `GET ` is closed at once (so an https
// request fails fast). /hang and /drip send their bytes, then block on read until the
// client closes; they report the request and the close on a channel. Every other route
// reports on a second channel: the request when its head is read, the close after the
// response is written, the server's side shut and the client's close seen (so a test can
// wait for curl instead of sleeping). No test touches the internet.
//
// Routes: /hello (`Hello` CR LF), /lf (`a` LF `b` LF), /bin (00-FF), /chunk (100 KiB
// of `pattern`, chunked), /empty, /304, /rN (a redirect to /r(N-1); /r0 is 200 `ok`),
// /404 and /500 (with a body), /max (FFFFFF bytes of `pattern`), /over (1000000h bytes
// of `pattern`, chunked), /hang (accepts, never answers), /drip (`abc` of a promised
// 1000, then silence). Anything else is a 404.
//
// `scripted` is the ASK side of H (DEVICE_SPECS 8, ASK vectors): each connection's
// request is read in full (method, path, headers, the body by Content-Length) and
// reported, then the connection plays its script: chunks written and flushed one by
// one, `Hold` until the test releases it (reporting a client close meanwhile), and the
// close. `sse` writes the Messages API events for a list of text deltas.

use std::io::{ErrorKind, Read, Write};
use std::net::{Shutdown, SocketAddr, TcpListener, TcpStream};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

pub struct Server {
    addr: SocketAddr,
    /// "open PATH" when a /hang or /drip request arrives, "close PATH" when its client closes.
    events: Receiver<String>,
    /// "request PATH" and "close PATH" for every other route.
    served: Receiver<String>,
}

/// Start H. It runs until the test process ends.
pub fn start() -> Server {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    let (tx, events) = mpsc::channel();
    let (served_tx, served) = mpsc::channel();
    thread::spawn(move || {
        for conn in listener.incoming().flatten() {
            let (tx, served_tx) = (tx.clone(), served_tx.clone());
            thread::spawn(move || serve(conn, tx, served_tx));
        }
    });
    Server { addr, events, served }
}

impl Server {
    /// `127.0.0.1:port`, the H of the vectors.
    pub fn host(&self) -> String {
        self.addr.to_string()
    }

    /// `http://H` + `path`.
    pub fn url(&self, path: &str) -> String {
        format!("http://{}{}", self.addr, path)
    }

    /// A /hang or /drip request arrived within 5 s (so killing the worker now closes a
    /// connection the server holds).
    pub fn sees_request(&self) -> bool {
        self.events.recv_timeout(Duration::from_secs(5)).is_ok_and(|e| e.starts_with("open "))
    }

    /// The server sees the close: a /hang or /drip connection reported the client's
    /// close within 5 s.
    pub fn sees_close(&self) -> bool {
        self.events.recv_timeout(Duration::from_secs(5)).is_ok_and(|e| e.starts_with("close "))
    }

    /// A route other than /hang and /drip reported `event` ("request PATH" or "close
    /// PATH") within 10 s; earlier reports are skipped.
    pub fn sees_served(&self, event: &str) -> bool {
        let deadline = Instant::now() + Duration::from_secs(10);
        while let Some(left) = deadline.checked_duration_since(Instant::now()) {
            match self.served.recv_timeout(left) {
                Ok(e) if e == event => return true,
                Ok(_) => {}
                Err(_) => return false,
            }
        }
        false
    }
}

/// `S` of the connect-limit vector: accepts and holds every connection, reading nothing,
/// so a TLS handshake never gets an answer.
pub fn silent() -> SocketAddr {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    thread::spawn(move || {
        let held: Vec<TcpStream> = listener.incoming().flatten().collect();
        drop(held);
    });
    addr
}

/// The body bytes of /chunk, /max and /over: byte i is i mod 251.
pub fn pattern(n: usize) -> Vec<u8> {
    (0..n).map(|i| (i % 251) as u8).collect()
}

fn serve(mut conn: TcpStream, events: Sender<String>, served: Sender<String>) {
    let mut head = Vec::new();
    let mut buf = [0u8; 1024];
    while !head.windows(4).any(|w| w == b"\r\n\r\n") {
        match conn.read(&mut buf) {
            Ok(n @ 1..) => head.extend(&buf[..n]),
            _ => return,
        }
        if !b"GET ".starts_with(&head[..head.len().min(4)]) {
            return; // not GET (a TLS hello): close at once
        }
    }
    let path = String::from_utf8_lossy(&head[4..]).split(' ').next().unwrap_or("").to_string();
    match path.as_str() {
        "/hang" => return hold(conn, &path, &events),
        "/drip" => {
            let _ = conn.write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 1000\r\nConnection: close\r\n\r\nabc");
            return hold(conn, &path, &events);
        }
        _ => {}
    }
    let _ = served.send(format!("request {}", path));
    // Write errors are the client's business (curl stops reading /over): ignore them.
    let _ = match path.as_str() {
        "/hello" => respond(&mut conn, "200 OK", b"Hello\r\n"),
        "/lf" => respond(&mut conn, "200 OK", b"a\nb\n"),
        "/bin" => respond(&mut conn, "200 OK", &(0..=255).collect::<Vec<u8>>()),
        "/chunk" => chunked(&mut conn, &pattern(100 * 1024)),
        "/empty" => respond(&mut conn, "200 OK", b""),
        "/304" => conn.write_all(b"HTTP/1.1 304 Not Modified\r\nConnection: close\r\n\r\n"),
        "/404" => respond(&mut conn, "404 Not Found", b"no such page\r\n"),
        "/500" => respond(&mut conn, "500 Internal Server Error", b"broken\r\n"),
        "/max" => respond(&mut conn, "200 OK", &pattern(0xFF_FFFF)),
        "/over" => chunked(&mut conn, &pattern(0x100_0000)),
        "/r0" => respond(&mut conn, "200 OK", b"ok"),
        p => match p.strip_prefix("/r").and_then(|n| n.parse::<u32>().ok()) {
            Some(n) => conn.write_all(format!(
                "HTTP/1.1 302 Found\r\nLocation: /r{}\r\nContent-Length: 0\r\nConnection: close\r\n\r\n", n - 1).as_bytes()),
            None => respond(&mut conn, "404 Not Found", b""),
        },
    };
    let _ = conn.shutdown(Shutdown::Write);
    while let Ok(1..) = conn.read(&mut buf) {}
    let _ = served.send(format!("close {}", path));
}

fn respond(conn: &mut TcpStream, status: &str, body: &[u8]) -> std::io::Result<()> {
    conn.write_all(format!("HTTP/1.1 {}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n", status, body.len()).as_bytes())?;
    conn.write_all(body)
}

fn chunked(conn: &mut TcpStream, body: &[u8]) -> std::io::Result<()> {
    conn.write_all(b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\nConnection: close\r\n\r\n")?;
    for chunk in body.chunks(16 * 1024) {
        conn.write_all(format!("{:x}\r\n", chunk.len()).as_bytes())?;
        conn.write_all(chunk)?;
        conn.write_all(b"\r\n")?;
    }
    conn.write_all(b"0\r\n\r\n")
}

/// Report the request, block until the client closes, then report the close.
fn hold(mut conn: TcpStream, path: &str, events: &Sender<String>) {
    let _ = events.send(format!("open {}", path));
    let mut buf = [0u8; 256];
    while let Ok(1..) = conn.read(&mut buf) {}
    let _ = events.send(format!("close {}", path));
}

/// One step of a scripted connection.
pub enum Step {
    /// Write these bytes and flush.
    Send(Vec<u8>),
    /// Wait until the test calls `release`, or the client closes (reported); the script
    /// then goes on either way.
    Hold,
}

/// A request as the server read it.
#[derive(Debug)]
pub struct Request {
    pub method: String,
    pub path: String,
    /// Names lowercased, values as sent.
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
}

impl Request {
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers.iter().find(|(n, _)| n == name).map(|(_, v)| v.as_str())
    }
}

pub struct Scripted {
    addr: SocketAddr,
    requests: Receiver<Request>,
    /// One message per client close seen during a `Hold`.
    closes: Receiver<()>,
    release: Sender<()>,
}

/// Start a scripted server. Connection i plays `scripts[i]`, the last script for every
/// connection after it; the connection closes when its script ends.
pub fn scripted(scripts: Vec<Vec<Step>>) -> Scripted {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    let (request_tx, requests) = mpsc::channel();
    let (close_tx, closes) = mpsc::channel();
    let (release, held) = mpsc::channel();
    let held = Arc::new(Mutex::new(held));
    let scripts: Vec<Arc<Vec<Step>>> = scripts.into_iter().map(Arc::new).collect();
    thread::spawn(move || {
        for (i, conn) in listener.incoming().flatten().enumerate() {
            let (request_tx, close_tx, held) = (request_tx.clone(), close_tx.clone(), held.clone());
            let script = scripts[i.min(scripts.len() - 1)].clone();
            thread::spawn(move || play(conn, &script, &request_tx, &close_tx, &held));
        }
    });
    Scripted { addr, requests, closes, release }
}

impl Scripted {
    /// `http://H` + `path`.
    pub fn url(&self, path: &str) -> String {
        format!("http://{}{}", self.addr, path)
    }

    /// The next request, within 5 s.
    pub fn request(&self) -> Request {
        self.requests.recv_timeout(Duration::from_secs(5)).expect("no request within 5 s")
    }

    /// No request arrived within 300 ms.
    pub fn no_request(&self) -> bool {
        self.requests.recv_timeout(Duration::from_millis(300)).is_err()
    }

    /// A held connection saw its client close within `ms`.
    pub fn sees_close_within(&self, ms: u64) -> bool {
        self.closes.recv_timeout(Duration::from_millis(ms)).is_ok()
    }

    /// Let a `Hold` go on.
    pub fn release(&self) {
        self.release.send(()).unwrap();
    }
}

/// The head of a 200 event stream; the body ends at the close.
pub fn sse_head() -> Step {
    Step::Send(b"HTTP/1.1 200 OK\r\ncontent-type: text/event-stream\r\nConnection: close\r\n\r\n".to_vec())
}

/// One SSE event.
pub fn event(json: &str) -> Step {
    let ty = json.split("\"type\":\"").nth(1).and_then(|t| t.split('"').next()).unwrap_or("");
    Step::Send(format!("event: {}\ndata: {}\n\n", ty, json).into_bytes())
}

/// A text delta event.
pub fn text(t: &str) -> Step {
    event(&format!(r#"{{"type":"content_block_delta","index":0,"delta":{{"type":"text_delta","text":{}}}}}"#, json_string(t)))
}

/// The events before the first text: message_start and a text block's start.
pub fn sse_start() -> Vec<Step> {
    vec![
        sse_head(),
        event(r#"{"type":"message_start","message":{"id":"msg_test","type":"message","role":"assistant","content":[]}}"#),
        event(r#"{"type":"content_block_start","index":0,"content_block":{"type":"text","text":""}}"#),
    ]
}

/// The end: the block's stop, message_delta with `stop_reason`, message_stop.
pub fn sse_stop(stop_reason: &str) -> Vec<Step> {
    vec![
        event(r#"{"type":"content_block_stop","index":0}"#),
        event(&format!(r#"{{"type":"message_delta","delta":{{"stop_reason":"{}"}},"usage":{{"output_tokens":1}}}}"#, stop_reason)),
        event(r#"{"type":"message_stop"}"#),
    ]
}

/// A whole reply: one text delta per element of `texts`, then `stop_reason`.
pub fn sse(texts: &[&str], stop_reason: &str) -> Vec<Step> {
    let mut steps = sse_start();
    steps.extend(texts.iter().map(|t| text(t)));
    steps.extend(sse_stop(stop_reason));
    steps
}

/// An HTTP error status with an API error body.
pub fn status(code: u16) -> Vec<Step> {
    let body = r#"{"type":"error","error":{"type":"api_error","message":"test"}}"#;
    vec![Step::Send(format!("HTTP/1.1 {} Error\r\ncontent-type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
        code, body.len(), body).into_bytes())]
}

/// `t` as a JSON string, quotes included.
fn json_string(t: &str) -> String {
    let mut s = String::from("\"");
    for c in t.chars() {
        match c {
            '"' => s += "\\\"",
            '\\' => s += "\\\\",
            c if (c as u32) < 0x20 => s += &format!("\\u{:04x}", c as u32),
            c => s.push(c),
        }
    }
    s + "\""
}

fn play(mut conn: TcpStream, script: &[Step], requests: &Sender<Request>, closes: &Sender<()>, held: &Mutex<Receiver<()>>) {
    let Some(request) = read_request(&mut conn) else { return };
    let _ = requests.send(request);
    for step in script {
        match step {
            // Write errors are the client's business (it may be gone): ignore them.
            Step::Send(bytes) => {
                let _ = conn.write_all(bytes).and_then(|()| conn.flush());
            }
            Step::Hold => {
                let held = held.lock().unwrap();
                conn.set_read_timeout(Some(Duration::from_millis(20))).unwrap();
                let mut buf = [0u8; 256];
                loop {
                    match held.recv_timeout(Duration::from_millis(20)) {
                        Ok(()) => break,
                        Err(mpsc::RecvTimeoutError::Disconnected) => return,
                        Err(mpsc::RecvTimeoutError::Timeout) => {}
                    }
                    match conn.read(&mut buf) {
                        Ok(0) => {
                            let _ = closes.send(());
                            break;
                        }
                        Err(e) if matches!(e.kind(), ErrorKind::WouldBlock | ErrorKind::TimedOut) => {}
                        Ok(_) => {}
                        Err(_) => {
                            let _ = closes.send(());
                            break;
                        }
                    }
                }
            }
        }
    }
}

/// The request line, the headers and a Content-Length body; None if the client closed first.
fn read_request(conn: &mut TcpStream) -> Option<Request> {
    let mut data = Vec::new();
    let mut buf = [0u8; 4096];
    let head_end = loop {
        if let Some(i) = data.windows(4).position(|w| w == b"\r\n\r\n") {
            break i;
        }
        match conn.read(&mut buf) {
            Ok(n @ 1..) => data.extend(&buf[..n]),
            _ => return None,
        }
    };
    let head = String::from_utf8_lossy(&data[..head_end]).to_string();
    let mut lines = head.split("\r\n");
    let mut first = lines.next()?.split(' ');
    let (method, path) = (first.next()?.to_string(), first.next()?.to_string());
    let headers: Vec<(String, String)> = lines
        .filter_map(|l| l.split_once(':'))
        .map(|(n, v)| (n.trim().to_ascii_lowercase(), v.trim().to_string()))
        .collect();
    let len: usize = headers.iter().find(|(n, _)| n == "content-length").map_or(0, |(_, v)| v.parse().unwrap());
    let mut body = data[head_end + 4..].to_vec();
    while body.len() < len {
        match conn.read(&mut buf) {
            Ok(n @ 1..) => body.extend(&buf[..n]),
            _ => return None,
        }
    }
    Some(Request { method, path, headers, body })
}
