// http.rs - the test HTTP server `H` of the GET conformance vectors (DEVICE_SPECS 8)
// and the N vectors (MONITOR_SPEC 6.18.1).
//
// A std TcpListener on 127.0.0.1:0, one thread per connection, canned responses by
// path. A connection whose first bytes are not `GET ` is closed at once (so an https
// request fails fast). /hang and /drip send their bytes, then block on read until the
// client closes; they report the request and the close on a channel. No test touches the internet.
//
// Routes: /hello (`Hello` CR LF), /lf (`a` LF `b` LF), /bin (00-FF), /chunk (100 KiB
// of `pattern`, chunked), /empty, /304, /rN (a redirect to /r(N-1); /r0 is 200 `ok`),
// /404 and /500 (with a body), /max (FFFFFF bytes of `pattern`), /over (1000000h bytes
// of `pattern`, chunked), /hang (accepts, never answers), /drip (`abc` of a promised
// 1000, then silence). Anything else is a 404.

use std::io::{Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;
use std::time::Duration;

pub struct Server {
    addr: SocketAddr,
    /// "open PATH" when a /hang or /drip request arrives, "close PATH" when its client closes.
    events: Receiver<String>,
}

/// Start H. It runs until the test process ends.
pub fn start() -> Server {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    let (tx, events) = mpsc::channel();
    thread::spawn(move || {
        for conn in listener.incoming().flatten() {
            let tx = tx.clone();
            thread::spawn(move || serve(conn, tx));
        }
    });
    Server { addr, events }
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

fn serve(mut conn: TcpStream, events: Sender<String>) {
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
        "/hang" => hold(conn, &path, &events),
        "/drip" => {
            let _ = conn.write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 1000\r\nConnection: close\r\n\r\nabc");
            hold(conn, &path, &events)
        }
        p => match p.strip_prefix("/r").and_then(|n| n.parse::<u32>().ok()) {
            Some(n) => conn.write_all(format!(
                "HTTP/1.1 302 Found\r\nLocation: /r{}\r\nContent-Length: 0\r\nConnection: close\r\n\r\n", n - 1).as_bytes()),
            None => respond(&mut conn, "404 Not Found", b""),
        },
    };
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
fn hold(mut conn: TcpStream, path: &str, events: &Sender<String>) -> std::io::Result<()> {
    let _ = events.send(format!("open {}", path));
    let mut buf = [0u8; 256];
    while let Ok(1..) = conn.read(&mut buf) {}
    let _ = events.send(format!("close {}", path));
    Ok(())
}
