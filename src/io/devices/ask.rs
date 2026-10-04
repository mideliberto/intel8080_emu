// ask.rs - mailbox ASK (DEVICE_SPECS 8, ASK and ASK service): one single-turn Messages
// API request per ASK, run by the mailbox as GET's worker is (a /usr/bin/curl child,
// checked at IN 12). This file holds what is ASK's own: the prompt rules (82), the curl
// argument list and its stdin, the request body, and the reader that turns the SSE
// stream into reply bytes (mapping and wrapping). No network code: curl does that.
//
// The worker's argument list and the reason for each flag are in DEVICE_SPECS 8 (ASK
// client); `argv_never_holds_the_key` pins the list. The key and the body go on curl's
// stdin (`-K -`), never on its command line.

use std::ffi::OsString;

/// The model (DEVICE_SPECS 8, ASK service). Changing it is this edit and a rebuild.
pub const MODEL: &str = "claude-opus-5-5";
/// The Messages API endpoint.
pub const URL: &str = "https://api.anthropic.com/v1/messages";
/// The system prompt, compiled in.
pub const SYSTEM: &str = include_str!("ask_system.txt");

/// A reply line holds at most this many characters.
const WIDTH: usize = 79;

/// What ASK needs from its host: the key (None: every valid ASK gives 83), the endpoint
/// and the total time limit in seconds. main.rs and pi_main.rs fill the key from
/// `ANTHROPIC_API_KEY`; the harnesses pass `AskConfig::default()` (no key).
/// No Debug: it holds the key (PI_DAEMON 10).
#[derive(Clone)]
pub struct AskConfig {
    pub key: Option<String>,
    pub url: String,
    pub total_secs: u32,
}

impl Default for AskConfig {
    fn default() -> Self {
        AskConfig { key: None, url: URL.to_string(), total_secs: 120 }
    }
}

/// The prompt rules (82): only 20h-7Eh, at least one byte that is not a space. Returns
/// the prompt with leading and trailing spaces removed. The 124-byte limit is the
/// command buffer's (81).
pub fn validate(args: &[u8]) -> Result<String, u8> {
    if !args.iter().all(|b| (0x20..=0x7E).contains(b)) {
        return Err(0x82);
    }
    // Every byte is 20-7E, so this can't fail.
    let prompt = std::str::from_utf8(args).unwrap().trim_matches(' ');
    if prompt.is_empty() {
        return Err(0x82);
    }
    Ok(prompt.to_string())
}

/// The worker's argument list (module header). Holds no key and no body.
pub fn argv(cfg: &AskConfig) -> Vec<OsString> {
    let total = cfg.total_secs.to_string();
    ["-q", "-s", "-N", "-f", "--proto", "=http,https", "--connect-timeout", "10", "--max-time", &total,
        "-H", "anthropic-version: 2023-06-01", "-H", "content-type: application/json", "-H", "Expect:",
        "--url", &cfg.url, "-K", "-"]
        .iter()
        .map(OsString::from)
        .collect()
}

/// curl's config on stdin: the key header and the body, each value with `\` and `"`
/// escaped by a backslash.
pub fn stdin(key: &str, body: &str) -> String {
    let quote = |s: &str| s.replace('\\', "\\\\").replace('"', "\\\"");
    format!("header = \"x-api-key: {}\"\ndata-binary = \"{}\"\n", quote(key), quote(body))
}

/// The request body: one user message, streamed.
pub fn body(prompt: &str) -> String {
    serde_json::json!({
        "model": MODEL,
        "max_tokens": 2048,
        "stream": true,
        "output_config": {"effort": "low"},
        "system": SYSTEM,
        "messages": [{"role": "user", "content": prompt}],
    })
    .to_string()
}

/// Turns the SSE stream into reply bytes. `feed` takes whatever one pipe read returned;
/// `finish` flushes at the end. The bytes out never depend on how the stream was split.
#[derive(Default)]
pub struct Reader {
    /// The SSE line so far (up to its LF).
    line: Vec<u8>,
    stop_reason: Option<String>,
    /// None while the reply runs; Some(true) after message_stop (not a refusal);
    /// Some(false) after an error event or a data line that is not JSON. Later bytes are ignored.
    end: Option<bool>,
    wrap: Wrap,
}

impl Reader {
    /// Feed stream bytes; final reply bytes are appended to `out`.
    pub fn feed(&mut self, bytes: &[u8], out: &mut Vec<u8>) {
        for &b in bytes {
            if b != b'\n' {
                self.line.push(b);
                continue;
            }
            let mut line = std::mem::take(&mut self.line);
            if line.last() == Some(&b'\r') {
                line.pop();
            }
            if self.end.is_none() {
                if let Some(data) = line.strip_prefix(b"data:") {
                    self.event(data, out);
                }
            }
        }
    }

    /// One `data:` line: one JSON object, read by its type. Unknown types are ignored.
    fn event(&mut self, data: &[u8], out: &mut Vec<u8>) {
        let Ok(v) = serde_json::from_slice::<serde_json::Value>(data) else {
            self.end = Some(false);
            return;
        };
        match v["type"].as_str() {
            Some("content_block_delta") if v["delta"]["type"] == "text_delta" => {
                for c in v["delta"]["text"].as_str().unwrap_or("").chars() {
                    map(c, &mut |b| self.wrap.push(b, out));
                }
            }
            Some("message_delta") => {
                if let Some(reason) = v["delta"]["stop_reason"].as_str() {
                    self.stop_reason = Some(reason.to_string());
                }
            }
            Some("message_stop") => self.end = Some(self.stop_reason.as_deref() != Some("refusal")),
            Some("error") => self.end = Some(false),
            _ => {}
        }
    }

    /// The end of the stream, however it ended: the held-back text goes out (trailing
    /// spaces and line breaks dropped). True if the reply ended with message_stop and
    /// was not a refusal.
    pub fn finish(&mut self, out: &mut Vec<u8>) -> bool {
        self.wrap.flush(out);
        self.end == Some(true)
    }

    /// Characters held back (the current line from its last run of spaces) and line
    /// breaks pending.
    #[cfg(test)]
    fn held(&self) -> (usize, usize) {
        (self.wrap.line.len() - self.wrap.sent, self.wrap.breaks)
    }
}

/// The mapping table (DEVICE_SPECS 8, ASK, Mapping): `emit` gets 20h-7Eh, or LF for a
/// line break.
fn map(c: char, emit: &mut impl FnMut(u8)) {
    let s = match c {
        ' '..='~' | '\n' => return emit(c as u8),
        '\t' | '\u{A0}' | '\u{2002}'..='\u{200A}' | '\u{202F}' => " ",
        '\0'..='\u{1F}' | '\u{7F}' | '\u{200B}'..='\u{200D}' | '\u{FEFF}' => "",
        '\u{2018}' | '\u{2019}' | '\u{201A}' | '\u{2032}' => "'",
        '\u{201C}' | '\u{201D}' | '\u{201E}' | '\u{2033}' => "\"",
        '\u{2010}' | '\u{2011}' | '\u{2013}' | '\u{2014}' | '\u{2212}' => "-",
        '\u{2026}' => "...",
        '\u{2022}' | '\u{B7}' => "*",
        '\u{D7}' => "x",
        '\u{2192}' => "->",
        '\u{2190}' => "<-",
        _ => "?",
    };
    s.bytes().for_each(emit);
}

/// The wrapping (DEVICE_SPECS 8, ASK, Wrapping), incremental: a byte goes out as soon as
/// no later text can change it.
#[derive(Default)]
struct Wrap {
    /// The current output line: `line[..sent]` is out, the rest held back.
    line: Vec<u8>,
    sent: usize,
    /// The current input line has a non-space character.
    text: bool,
    /// Some line has had text: line breaks count from then on.
    started: bool,
    /// Line breaks not yet followed by text.
    breaks: usize,
}

impl Wrap {
    fn push(&mut self, b: u8, out: &mut Vec<u8>) {
        if b == b'\n' {
            self.flush(out);
            self.text = false;
            if self.started {
                self.breaks += 1;
            }
            return;
        }
        if b == b' ' {
            // A run of spaces past column 80 is removed whole by a cut, or trailing.
            if self.line.len() <= WIDTH {
                self.line.push(b);
            }
        } else {
            if !self.text {
                self.text = true;
                if self.started {
                    out.extend(b"\r\n".repeat(self.breaks));
                }
                self.started = true;
                self.breaks = 0;
            }
            self.line.push(b);
            if self.line.len() > WIDTH {
                self.cut(out);
            }
        }
        // Out: everything before the last run of spaces; no cut can fall there now.
        let safe = match self.line.iter().rposition(|&c| c == b' ') {
            Some(i) => self.line[..i].iter().rposition(|&c| c != b' ').map_or(0, |j| j + 1),
            None => self.line.len(),
        };
        if safe > self.sent {
            out.extend(&self.line[self.sent..safe]);
            self.sent = safe;
        }
    }

    /// The line is over 79 characters and ends in a non-space: cut at the last space at index
    /// 1-79 with a non-space before it, removing its run; else after the 79th character,
    /// removing the spaces that follow.
    fn cut(&mut self, out: &mut Vec<u8>) {
        let line = &self.line;
        let space = (1..=WIDTH).rev().find(|&i| line[i] == b' ' && line[..i].iter().any(|&c| c != b' '));
        let (end, rest) = match space {
            Some(i) => {
                let start = line[..i].iter().rposition(|&c| c != b' ').unwrap() + 1;
                (start, i + line[i..].iter().position(|&c| c != b' ').unwrap())
            }
            None => (WIDTH, WIDTH + line[WIDTH..].iter().position(|&c| c != b' ').unwrap()),
        };
        out.extend(&self.line[self.sent..end]);
        out.extend(b"\r\n");
        self.line.drain(..rest);
        self.sent = 0;
    }

    /// The input line ends (a line break, or the end of the reply): its held-back text
    /// goes out without its trailing spaces.
    fn flush(&mut self, out: &mut Vec<u8>) {
        let end = self.line.iter().rposition(|&c| c != b' ').map_or(0, |j| j + 1);
        if end > self.sent {
            out.extend(&self.line[self.sent..end]);
        }
        self.line.clear();
        self.sent = 0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn argv_never_holds_the_key() {
        let cfg = AskConfig { key: Some("sk-test-KEY".into()), url: "http://h/v1/messages".into(), total_secs: 120 };
        let args: Vec<String> = argv(&cfg).into_iter().map(|a| a.into_string().unwrap()).collect();
        assert_eq!(args.join(" "), "-q -s -N -f --proto =http,https --connect-timeout 10 --max-time 120 \
            -H anthropic-version: 2023-06-01 -H content-type: application/json -H Expect: --url http://h/v1/messages -K -");
        assert_eq!(args.len(), 20);
        assert!(args.iter().all(|a| !a.contains("sk-test-KEY") && !a.contains(MODEL)));
        let input = stdin("sk-test-KEY", &body("hi"));
        assert!(input.starts_with("header = \"x-api-key: sk-test-KEY\"\ndata-binary = \"{"));
    }

    #[test]
    fn config_fits_an_empty_pipe() {
        // The worst escaping: 124 bytes of `"`, each escaped twice (JSON, then curl's config).
        assert!(stdin("sk-ant-api03-".repeat(10).as_str(), &body(&"\"".repeat(124))).len() < 16384);
    }

    #[test]
    fn map_table() {
        let m = |c: char| {
            let mut v = Vec::new();
            map(c, &mut |b| v.push(b));
            String::from_utf8(v).unwrap()
        };
        // 20-7E: itself. LF: a line break.
        assert_eq!((m(' '), m('A'), m('~'), m('\n')), (" ".into(), "A".into(), "~".into(), "\n".into()));
        // Tab, U+00A0, U+2002-U+200A, U+202F: one space.
        for c in ['\t', '\u{A0}', '\u{2002}', '\u{2005}', '\u{200A}', '\u{202F}'] {
            assert_eq!(m(c), " ", "{:?}", c);
        }
        // CR, every other 00-1F, 7F, U+200B-U+200D, U+FEFF: nothing.
        for c in ['\r', '\0', '\u{1B}', '\u{1F}', '\u{7F}', '\u{200B}', '\u{200C}', '\u{200D}', '\u{FEFF}'] {
            assert_eq!(m(c), "", "{:?}", c);
        }
        for (cs, s) in [(&['\u{2018}', '\u{2019}', '\u{201A}', '\u{2032}'][..], "'"),
                        (&['\u{201C}', '\u{201D}', '\u{201E}', '\u{2033}'][..], "\""),
                        (&['\u{2010}', '\u{2011}', '\u{2013}', '\u{2014}', '\u{2212}'][..], "-"),
                        (&['\u{2026}'][..], "..."),
                        (&['\u{2022}', '\u{B7}'][..], "*"),
                        (&['\u{D7}'][..], "x"),
                        (&['\u{2192}'][..], "->"),
                        (&['\u{2190}'][..], "<-")] {
            for &c in cs {
                assert_eq!(m(c), s, "{:?}", c);
            }
        }
        // Any other character from U+0080 up: `?`.
        for c in ['\u{80}', '\u{9B}', '\u{E9}', '\u{2015}', '\u{1F600}'] {
            assert_eq!(m(c), "?", "{:?}", c);
        }
    }

    /// The SSE bytes of a reply: one text delta per element, then the stop.
    fn sse(texts: &[&str], stop: &str) -> Vec<u8> {
        let mut s = String::from("event: message_start\ndata: {\"type\":\"message_start\",\"message\":{}}\n\n");
        s += "data: {\"type\":\"content_block_start\",\"index\":0,\"content_block\":{\"type\":\"text\",\"text\":\"\"}}\n\n";
        for t in texts {
            let ev = serde_json::json!({"type": "content_block_delta", "index": 0, "delta": {"type": "text_delta", "text": t}});
            s += &format!("event: content_block_delta\r\ndata: {}\r\n\r\n", ev);
        }
        s += &format!("data: {{\"type\":\"message_delta\",\"delta\":{{\"stop_reason\":\"{}\"}}}}\n\n", stop);
        s += "data: {\"type\":\"message_stop\"}\n\n";
        s.into_bytes()
    }

    /// Feeds `stream` in the pieces `splits` gives; checks the held-back bound after each
    /// feed; returns (bytes, ok).
    fn run(stream: &[u8], splits: &[usize]) -> (Vec<u8>, bool) {
        let mut r = Reader::default();
        let mut out = Vec::new();
        let mut at = 0;
        for &end in splits.iter().chain([stream.len()].iter()) {
            r.feed(&stream[at..end], &mut out);
            at = end;
            // Held back: exactly the current line from the start of its last run of spaces.
            let line = &r.wrap.line;
            let from = match line.iter().rposition(|&c| c == b' ') {
                Some(i) => line[..i].iter().rposition(|&c| c != b' ').map_or(0, |j| j + 1),
                None => line.len(),
            };
            assert_eq!(r.held().0, line.len() - from, "held {:?} of {:?}", r.held(), String::from_utf8_lossy(line));
            assert!(r.held().0 <= 80, "held {:?}", r.held());
        }
        let ok = r.finish(&mut out);
        (out, ok)
    }

    #[test]
    fn delivery_does_not_depend_on_splits() {
        let x = |n| "x".repeat(n);
        let streams: Vec<(Vec<u8>, String, bool)> = vec![
            (sse(&["ok"], "end_turn"), "ok".into(), true),
            (sse(&["Hello", ", world"], "end_turn"), "Hello, world".into(), true),
            (sse(&["\n\n  a  \n\nb\n\n"], "end_turn"), "  a\r\n\r\nb".into(), true),
            (sse(&["\u{201C}x\u{201D}\u{2014}\u{2019}\u{2026}\u{1F600}\t\u{1B}[2J"], "end_turn"), "\"x\"-'...? [2J".into(), true),
            (sse(&[&x(100)], "end_turn"), format!("{}\r\n{}", x(79), x(21)), true),
            (sse(&[&"word ".repeat(20)], "end_turn"), format!("{}\r\n{}", ["word"; 16].join(" "), ["word"; 4].join(" ")), true),
            (sse(&[&x(79), " ", "y"], "end_turn"), format!("{}\r\ny", x(79)), true),
            (sse(&["    ", &x(80)], "end_turn"), format!("    {}\r\n{}", x(75), x(5)), true),
            (sse(&["cut"], "max_tokens"), "cut".into(), true),
            (sse(&["abc"], "refusal"), "abc".into(), false),
            // Spaces past column 80 and a long indentation (the held-back bound).
            (sse(&[&format!("a{}b", " ".repeat(200))], "end_turn"), "a\r\nb".into(), true),
            (sse(&[&format!("{}b c", " ".repeat(100))], "end_turn"), format!("{}\r\nb c", " ".repeat(79)), true),
            (sse(&["first line\n", "second"], "end_turn"), "first line\r\nsecond".into(), true),
        ];
        // `Hel`, then the connection closes before message_delta; or an error event there.
        let hel = sse(&["Hel"], "end_turn");
        let at = b"data: {\"type\":\"message_delta";
        let cut_short = hel[..hel.windows(at.len()).position(|w| w == at).unwrap()].to_vec();
        let error = [&cut_short[..],
            b"event: error\ndata: {\"type\":\"error\",\"error\":{\"type\":\"overloaded_error\"}}\n\n"].concat();
        let ignored = [&b"data: {\"type\":\"ping\"}\n\ndata: {\"type\":\"what_is_this\"}\n\n\
            data: {\"type\":\"content_block_start\",\"index\":0,\"content_block\":{\"type\":\"thinking\"}}\n\n\
            data: {\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"thinking_delta\",\"thinking\":\"hmm\"}}\n\n\
            data: {\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"signature_delta\",\"signature\":\"x\"}}\n\n\
            data: {\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"new_delta\",\"text\":\"no\"}}\n\n"[..],
            &sse(&["ok"], "end_turn")].concat();
        // An error event ends the reply even if text and message_stop follow it.
        let error_then_more = [&error[..], &sse(&["lo"], "end_turn")].concat();
        let streams = streams.into_iter().chain([
            (cut_short, "Hel".to_string(), false),
            (error, "Hel".to_string(), false),
            (error_then_more, "Hel".to_string(), false),
            (b"data: not json\n\n".to_vec(), String::new(), false),
            (ignored, "ok".to_string(), true),
        ]);
        for (stream, want, ok) in streams {
            let whole = run(&stream, &[]);
            assert_eq!((String::from_utf8(whole.0.clone()).unwrap(), whole.1), (want.clone(), ok),
                "{}", String::from_utf8_lossy(&stream));
            let bytes: Vec<usize> = (1..stream.len()).collect();
            assert_eq!(run(&stream, &bytes), whole, "one byte at a time: {:?}", want);
            for at in 1..stream.len() {
                assert_eq!(run(&stream, &[at]), whole, "split at {}: {:?}", at, want);
            }
        }
    }

    #[test]
    fn ask_system_lists_every_command() {
        // Every command in the help text starts a line of the system prompt, so a phase
        // that adds a command and forgets to tell Claude fails here.
        let help = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/transcripts/help.txt")).unwrap();
        let list: Vec<&str> = help.lines().skip_while(|l| *l != "Commands:").skip(1)
            .take_while(|l| l.starts_with("  ")).collect();
        assert!(list.len() >= 19, "{:?}", list);
        for l in list {
            let token = l.split_whitespace().next().unwrap();
            assert!(SYSTEM.lines().any(|s| s.starts_with(&format!("  {} ", token))), "ask_system.txt lacks {}", token);
        }
        assert!(SYSTEM.is_ascii() && SYSTEM.lines().all(|l| l.len() < 80));
    }
}
