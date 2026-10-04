// terminal_tests.rs - The real binary under a pseudo-terminal (ARCHITECTURE 7.1, 7.2, 7.4).
//
// These reach what the run-loop tests in src/main.rs can't: crossterm's raw mode, its
// key decoding, the terminal poll, and the hand-off between raw mode and the line-mode
// `dbg>` prompt. Each test starts the binary in a temp dir (its own `storage/`, a copy of
// rom/monitor.bin and monitor.sym) under `sh`, which prints `stty -g` before and after
// the run and the exit status in between, so every test also checks that the terminal
// mode was restored.
//
// rexpect opens the pty and turns its echo off; everything echoed below comes from the
// ROM. Unix only. A test that can't open a pty prints `skipped: no pty` and passes.

#![cfg(unix)]

use std::process::Command;

use rexpect::session::{spawn_command, PtySession};

const TIMEOUT_MS: u64 = 10_000;
const PROMPT: &str = "\r\n> ";

struct Run {
    p: PtySession,
    stty: String,
    _dir: tempfile::TempDir,
}

/// Starts `run` under `sh` in the pty, where "$0" is the binary. None when no pty opens.
fn spawn(run: &str) -> Option<Run> {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir(dir.path().join("rom")).unwrap();
    for f in ["monitor.bin", "monitor.sym"] {
        std::fs::copy(format!("rom/{}", f), dir.path().join("rom").join(f)).unwrap();
    }
    let mut cmd = Command::new("sh");
    let sh = format!(r#"stty -g; {}; echo "exit=$?"; stty -g"#, run);
    cmd.args(["-c", &sh, env!("CARGO_BIN_EXE_intel8080")]);
    cmd.current_dir(dir.path());
    cmd.env_remove("ANTHROPIC_API_KEY"); // cargo test never reaches the API (PI_DAEMON 13.2)
    let mut p = match spawn_command(cmd, Some(TIMEOUT_MS)) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("skipped: no pty ({})", e);
            return None;
        }
    };
    let stty = p.read_line().unwrap();
    Some(Run { p, stty, _dir: dir })
}

/// Boots the emulator on the terminal and waits for the first monitor prompt.
fn boot() -> Option<Run> {
    let mut r = spawn(r#""$0""#)?;
    r.p.exp_string("8080 Emulator").unwrap();
    r.p.exp_string("8080 Monitor v").unwrap();
    r.p.exp_string(PROMPT).unwrap();
    Some(r)
}

impl Run {
    /// Sends `bytes`, then expects `out` next, exactly (nothing between).
    fn io(&mut self, bytes: &str, out: &str) {
        self.p.send(bytes).unwrap();
        self.p.flush().unwrap();
        let before = self.p.exp_string(out).unwrap();
        assert_eq!(before, "", "unexpected output before {:?}", out);
    }

    /// Types a monitor command and Enter; expects the echo, `out` and the next prompt.
    fn cmd(&mut self, line: &str, out: &str) {
        self.io(&format!("{}\r", line), &format!("{}\r\n{}> ", line, out));
    }

    /// Waits for the process to end. Checks the exit status and that the terminal
    /// mode is what it was before the run.
    fn finish(mut self, status: i32) {
        self.p.exp_string(&format!("exit={}", status)).unwrap();
        self.p.read_line().unwrap(); // rest of the exit line
        assert_eq!(self.p.read_line().unwrap(), self.stty, "terminal mode not restored");
        self.p.exp_eof().unwrap();
    }
}

#[test]
fn boot_reaches_the_prompt_in_raw_mode() {
    let Some(mut r) = boot() else { return };
    // Non-canonical: each key reaches the ROM, which echoes it, before Enter.
    r.io("H 2 1", "H 2 1");
    // No output processing: the ROM's CR LF arrives as sent (ONLCR would give CR CR LF).
    r.io("\r", "\r\n0003 0001\r\n> ");
    r.io("\x03", "\r\n"); // Ctrl-C: main.rs ends the line it was on
    r.finish(0);
}

#[test]
fn a_typed_command_echoes_and_runs() {
    let Some(mut r) = boot() else { return };
    r.cmd("F 0200 020F 41", "");
    r.cmd("D 0200 020F", "0200: 41 41 41 41 41 41 41 41  41 41 41 41 41 41 41 41  AAAAAAAAAAAAAAAA\r\n");
    r.io("\x03", "\r\n");
    r.finish(0);
}

#[test]
fn ctrl_c_quits_a_running_program_and_restores_the_terminal() {
    let Some(mut r) = boot() else { return };
    // 0200: JMP 0200. The 8080 never reads the console again.
    r.cmd(":03020000C3000236", "");
    r.io("G 0200\r", "G 0200\r\n");
    r.io("\x03", "\r\n");
    r.finish(0);
}

#[test]
fn ctrl_e_opens_the_debugger_and_c_resumes() {
    let Some(mut r) = boot() else { return };
    r.io("\x05", "\r\n* ctrl-e\r\n");
    // The prompt is in line mode (raw mode off), so output processing turns LF into CR LF.
    r.p.exp_string("dbg> ").unwrap();
    // A bad line at the terminal prints its message and prompts again (a script would end).
    r.p.send_line("zz").unwrap();
    r.p.exp_string("? unknown command: zz\r\ndbg> ").unwrap();
    r.p.send_line("c").unwrap();
    // Raw mode is back: the next key is echoed before Enter, and output is unprocessed.
    r.io("H", "H");
    r.io(" 2 1\r", " 2 1\r\n0003 0001\r\n> ");
    r.io("\x03", "\r\n");
    r.finish(0);
}

#[test]
fn an_interactive_hlt_opens_the_debugger_and_q_quits() {
    let Some(mut r) = boot() else { return };
    r.cmd("F 0200 0200 76", "");
    r.io("G 0200\r", "G 0200\r\n* halt\r\n");
    r.p.exp_regex(r"PC=0201 [^\r\n]*\r\n").unwrap();
    r.p.exp_string("dbg> ").unwrap();
    r.p.send_line("q").unwrap();
    r.finish(0);
}

#[test]
fn backspace_reaches_in_01_as_08() {
    let Some(mut r) = boot() else { return };
    // 0200: IN 02 / ANI 01 / JZ 0200 / IN 01 / HLT. The ROM takes 7F as a backspace too,
    // so only IN 01 shows which byte arrived.
    r.cmd(":0A020000DB02E601CA0002DB017612", "");
    r.io("G 0200\r", "G 0200\r\n");
    r.io("\x7F", "* halt\r\n");
    r.p.exp_regex(r"PC=020A [^\r\n]* A=08 ").unwrap();
    r.p.exp_string("dbg> ").unwrap();
    r.p.send_line("q").unwrap();
    r.finish(0);
}

#[test]
fn backspace_and_ctrl_h_reach_the_rom_as_08() {
    let Some(mut r) = boot() else { return };
    // A terminal's Backspace key sends 7F; the key map turns it into 08 (ARCHITECTURE 7.1),
    // which the ROM echoes as BS SP BS. Ctrl-H is 08 too.
    r.io("H 12\x7F34 1\r", "H 12\x08 \x0834 1\r\n0135 0133\r\n> ");
    r.io("H 12\x0834 1\r", "H 12\x08 \x0834 1\r\n0135 0133\r\n> ");
    r.io("\x03", "\r\n");
    r.finish(0);
}

#[test]
fn a_piped_run_leaves_the_terminal_alone() {
    // stdin is a pipe, stdout the terminal: no raw mode, not even when a script resumes.
    let Some(mut r) = spawn(r#"echo c > s.txt; printf 'F 0200 0200 76\rG 0200\r' | "$0" --script s.txt"#) else {
        return;
    };
    r.p.exp_string("dbg> c").unwrap();
    r.p.exp_string("HLT at PC=0201").unwrap();
    r.finish(0);
}
