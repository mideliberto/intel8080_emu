// diag_tests.rs - The bring-up step 3 diagnostic image, rom/diag3.bin (HARDWARE_BUILD.md 3.3):
// its bus transfers against a model of the step 3 sequence written here from the spec, every
// HLT address of its table reached through a fault the harness injects, and the `--rom` run
// and fetch trace the bench compares with (ARCHITECTURE 7.4).

use std::path::Path;

use intel8080_emu::cpu::{Intel8080, Transfer, TBLC_CYCLES};

const IMAGE: &str = "rom/diag3.bin";

/// The HLT table of the diag3.asm header and HARDWARE_BUILD.md 3.3, as the PC after the HLT
/// (what the emulator prints).
const OVL1: u16 = 0xF004;
const OVL0: u16 = 0xF005;
const MARCH: [u16; 5] = [0xF006, 0xF007, 0xF008, 0xF009, 0xF00A]; // M1-M5
const PASS: u16 = 0x0118;
const ROM_FAIL: u16 = 0x0119;

/// The march background and its complement (diag3.asm header).
const ZERO: u8 = 0x55;
const ONE: u8 = 0xAA;

/// Steps before a run counts as hung. A good run is about 2.7M.
const STEPS: u64 = 10_000_000;

fn image() -> Vec<u8> {
    std::fs::read(IMAGE).unwrap()
}

/// The step 3 board at RESET: the image, no Pi (no device on any port, so every IN outside
/// FF reads FF), registers and RAM full of junk (ARCHITECTURE 3.1).
fn board() -> Intel8080 {
    let mut cpu = Intel8080::new();
    cpu.load_rom(&image());
    cpu.load_program(&vec![0xA5; 0x10000], 0x0000);
    (cpu.a, cpu.b, cpu.c, cpu.d, cpu.e, cpu.h, cpu.l, cpu.sp) = (0x5A, 0x5A, 0x5A, 0x5A, 0x5A, 0x5A, 0x5A, 0x5A5A);
    cpu.reset();
    cpu
}

struct Run {
    /// PC after the HLT, and HL there.
    pc: u16,
    hl: u16,
    ports: Vec<Transfer>,
    /// Memory transfers in bus order, each with the cycle count at the start of its instruction.
    mem: Vec<(Transfer, u64)>,
}

/// Steps to the HLT. `fault` runs after every step and may change the machine.
fn run(cpu: &mut Intel8080, mut fault: impl FnMut(&mut Intel8080)) -> Run {
    let (mut ports, mut mem) = (Vec::new(), Vec::new());
    for _ in 0..STEPS {
        if cpu.halted {
            return Run { pc: cpu.pc, hl: cpu.get_hl(), ports, mem };
        }
        let start = cpu.cycles;
        cpu.execute_one();
        for &t in cpu.transfers() {
            match t {
                Transfer::In(..) | Transfer::Out(..) => ports.push(t),
                _ => mem.push((t, start)),
            }
        }
        fault(cpu);
    }
    panic!("no HLT in {} steps: PC={:04X}", STEPS, cpu.pc);
}

/// No access to the Pi window, 00-6F (ARCHITECTURE 6.3): with no Pi it stalls the CPU.
fn assert_no_pi_window(r: &Run) {
    for t in &r.ports {
        let (Transfer::In(p, _) | Transfer::Out(p, _)) = *t else { unreachable!() };
        assert!(p >= 0x70, "Pi-window access {:?}", t);
    }
}

/// March C- (van de Goor) over 0000-EFFF with 0 = 55h, 1 = AAh, as bus transfers:
/// up (w0); up (r0,w1); up (r1,w0); down (r0,w1); down (r1,w0); up (r0).
fn march_model() -> Vec<Transfer> {
    use Transfer::{MemRead, MemWrite};
    let up: Vec<u16> = (0x0000..0xF000).collect();
    let down: Vec<u16> = up.iter().rev().copied().collect();
    let mut v: Vec<Transfer> = up.iter().map(|&a| MemWrite(a, ZERO)).collect();
    for (order, read, write) in [(&up, ZERO, ONE), (&up, ONE, ZERO), (&down, ZERO, ONE), (&down, ONE, ZERO)] {
        v.extend(order.iter().flat_map(|&a| [MemRead(a, read), MemWrite(a, write)]));
    }
    v.extend(up.iter().map(|&a| MemRead(a, ZERO)));
    v
}

#[test]
fn diag3_is_a_4k_image_burn_refuses() {
    let img = image();
    assert_eq!(img.len(), 4096);
    // F000 (0000 through the overlay) jumps into F000-FFFF at once (ARCHITECTURE 3.2 req. 1).
    assert_eq!(img[0], 0xC3);
    assert_eq!(img[2], 0xF0);
    // Not a monitor image (ARCHITECTURE 3.2 req. 7): examples/burn refuses it, so it cannot
    // replace the monitor in-circuit by mistake. It goes on with the external programmer.
    assert!(img[0] != 0x31 || img[6] != 0xF0);
}

#[test]
fn diag3_passes_on_a_good_board() {
    use Transfer::{In, MemRead, MemWrite, Out};
    let img = image();
    let mut cpu = board();
    let r = run(&mut cpu, |_| {});
    assert_eq!(r.pc, PASS, "HL={:04X}", r.hl);
    assert!(!cpu.rom_overlay_enabled);

    // Step 3: IN FF reads 1, OUT FE, IN FF reads 0; the window check; nothing else.
    assert_eq!(r.ports, [In(0xFF, 1), Out(0xFE, 1), In(0xFF, 0), Out(0x70, 0), In(0x70, 0xFF), Out(0xFD, 0xFF), In(0xFD, 0xFF)]);
    assert_no_pi_window(&r);

    // The march, transfer for transfer.
    let march = march_model();
    let got: Vec<Transfer> = r.mem.iter().map(|&(t, _)| t).collect();
    assert_eq!(got[..march.len()], march[..]);

    // Then the copy: ROM bytes to 0100 up, in order.
    let mut i = march.len();
    let mut copied = 0u16;
    while let (MemRead(src, v), MemWrite(dst, w)) = (got[i], got[i + 1]) {
        if dst != 0x0100 + copied {
            break;
        }
        assert!(src >= 0xF000 && v == img[(src - 0xF000) as usize] && w == v, "copy {:?} {:?}", got[i], got[i + 1]);
        copied += 1;
        i += 2;
    }
    assert!(copied > 0);
    let rloop: Vec<u8> = (0..copied).map(|n| cpu.read_byte(0x0100 + n)).collect();
    assert_eq!(rloop.last(), Some(&0x76), "the copy ends with the fail HLT");

    // Then the write-back loop: each ROM byte read, written back, read again, F000 to FFFF.
    let wb: Vec<Transfer> = (0..0x1000u16)
        .flat_map(|n| {
            let (a, v) = (0xF000 + n, img[n as usize]);
            [MemRead(a, v), MemWrite(a, v), MemRead(a, v)]
        })
        .collect();
    assert_eq!(got[i..], wb[..]);

    // The page-close wait (ARCHITECTURE 6.10 rule 3): from the end of each write (the last T
    // of MOV M,A, 7 T) to the second read (after the 4 T fetch of MOV A,M), >= tBLC at the
    // fastest legal clock, 313 T. The emulator's tBLC (307) is at 2.048 MHz.
    for n in 0..0x1000 {
        let (w, r2) = (r.mem[i + 3 * n + 1].1, r.mem[i + 3 * n + 2].1);
        assert!(r2 + 4 - (w + 7) >= 313, "byte {:03X}: {} T", n, r2 + 4 - (w + 7));
    }
}

#[test]
fn diag3_cycle_counts_match_the_header() {
    // The header's T-states per byte, counted by hand from the 8080 instruction timings
    // (docs/reference): M0 34, M1-M4 55, M5 48; the write-back 386. Measured here between
    // the first transfer of one element and the first of the next (each element starts with
    // a 10 T LXI H).
    let mut cpu = board();
    let r = run(&mut cpu, |_| {});
    let start = |k: usize| r.mem[k].1;
    let n = 0xF000u64;
    // Element e's first transfer: M0 makes n, M1-M4 2n each.
    let m = |e: usize| if e == 0 { 0 } else { n as usize * (2 * e - 1) };
    assert_eq!(start(m(1)) - start(m(0)), n * 34 + 10, "M0");
    for e in 1..5 {
        assert_eq!(start(m(e + 1)) - start(m(e)), n * 55 + 10, "M{}", e);
    }
    let wb = r.mem.len() - 3 * 0x1000;
    assert_eq!(start(r.mem.len() - 3) - start(wb), 4095 * 386, "write-back");
    // M5, then the copy's set-up (LXI H, LXI D, MVI B: 27 T) to its first read.
    assert_eq!(start(m(5) + n as usize) - start(m(5)), n * 48 + 27, "M5");
    // The whole run, to the end of the pass HLT: the prologue to the march's first write
    // (138 T), the march (302 T per byte, plus 5 LXI H), the copy (25 bytes of 39 T, set-up
    // and JMP), the write-back (LXI H, 386 T per byte, the HLT). About 9.8 s at 2.048 MHz.
    assert_eq!(cpu.cycles, 138 + n * 302 + 50 + 27 + 25 * 39 + 10 + 10 + 4096 * 386 + 7);
}

#[test]
fn diag3_jp_we_fitted_stops_at_f000() {
    // A /WE path despite JP-WE open, modelled by the fitted JP-WE (ARCHITECTURE 6.10): the
    // first write-back starts a write cycle of F000's own byte, the read after the wait is a
    // polling read, and the loop stops at F000. Under both readings of the page-load window.
    // tWC on a real part is milliseconds; the read comes 24 cycles after tBLC, so any tWC
    // above that is caught.
    let img = image();
    for cells in [false, true] {
        for twc in [1000, 20_480, 65_535] {
            let mut cpu = board();
            cpu.fit_jp_we(twc);
            cpu.set_load_window_cells(cells);
            let r = run(&mut cpu, |_| {});
            assert_eq!((r.pc, r.hl), (ROM_FAIL, 0xF000), "cells {} tWC {}", cells, twc);
            let rom_writes: Vec<_> = r.mem.iter().filter(|(t, _)| matches!(t, Transfer::MemWrite(a, _) if *a >= 0xF000)).collect();
            assert_eq!(rom_writes.len(), 1);
            assert_eq!(rom_writes[0].0, Transfer::MemWrite(0xF000, img[0]));
            assert_no_pi_window(&r);
            cpu.cycles += 2 * (TBLC_CYCLES + twc);
            assert_eq!(cpu.read_byte(0xF000), img[0], "the byte rewritten with its own value");
        }
    }
}

#[test]
fn diag3_overlay_faults() {
    use Transfer::{In, Out};
    // The overlay flip-flop not set by RESET: IN FF reads 0 (the CPU is started at F000,
    // since 0000 would be RAM).
    let mut cpu = board();
    cpu.rom_overlay_enabled = false;
    cpu.pc = 0xF000;
    let r = run(&mut cpu, |_| {});
    assert_eq!(r.pc, OVL1);
    assert_eq!(r.ports, [In(0xFF, 0)]);

    // Stuck set: OUT FE does not clear it.
    let mut cpu = board();
    let r = run(&mut cpu, |cpu| cpu.rom_overlay_enabled = true);
    assert_eq!(r.pc, OVL0);
    assert_eq!(r.ports, [In(0xFF, 1), Out(0xFE, 1), In(0xFF, 1)]);
}

/// Runs with a RAM fault: after each write to `cell`, `fault(cell, value, nth write)`
/// may write RAM.
fn faulty_ram(cell: u16, mut fault: impl FnMut(&mut Intel8080, u8, usize)) -> Run {
    let mut cpu = board();
    let mut writes = 0;
    let r = run(&mut cpu, |cpu| {
        let w = cpu.transfers().iter().find_map(|t| match *t {
            Transfer::MemWrite(a, v) if a == cell => Some(v),
            _ => None,
        });
        if let Some(v) = w {
            writes += 1;
            fault(cpu, v, writes);
        }
    });
    assert_no_pi_window(&r);
    r
}

#[test]
fn diag3_march_faults_stop_at_their_element() {
    // A cell that loses its value after its k-th write fails the read element that follows
    // write k: M1 to M5, each at its own HLT, with HL at the cell. One cell per RAM region:
    // under the overlay, both sides of A15 (the two RAM chips), the top.
    for (k, cell) in [0x0000u16, 0x0FFF, 0x7FFF, 0x8000, 0xEFFF].into_iter().enumerate() {
        let r = faulty_ram(cell, |cpu, _, n| {
            if n == k + 1 {
                cpu.write_byte(cell, 0x00);
            }
        });
        assert_eq!((r.pc, r.hl), (MARCH[k], cell), "lost after write {}", k + 1);
    }
    // An address fault: writes to 1234 also land in 5234 (A14 lost for that cell). M1 writes
    // 1 at 1234 and reads 5234 later expecting 0.
    let r = faulty_ram(0x1234, |cpu, v, _| cpu.write_byte(0x5234, v));
    assert_eq!((r.pc, r.hl), (MARCH[0], 0x5234));
    // D3 stuck at 0 in one cell: 55h has bit 3 clear, AAh set, so M2 reads A2h.
    let r = faulty_ram(0x2345, |cpu, v, _| cpu.write_byte(0x2345, v & !0x08));
    assert_eq!((r.pc, r.hl), (MARCH[1], 0x2345));
}

/// The emulator binary in an empty directory (so rom/monitor.bin is not there), with the
/// image by absolute path.
fn emulator(args: &[&str], script: &str) -> (Option<i32>, String) {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("s.dbg"), script).unwrap();
    let image = Path::new(env!("CARGO_MANIFEST_DIR")).join(IMAGE);
    let mut all = vec!["--rom", image.to_str().unwrap()];
    all.extend(args);
    run_in(dir.path(), &all)
}

/// How long a run of the binary may take. A piped run ends only on a halt (ARCHITECTURE 7.2),
/// so a regression that never reaches its HLT fails here instead of hanging cargo test.
const DEADLINE: std::time::Duration = std::time::Duration::from_secs(30);

/// The emulator binary run in `dir` with no input. Returns the exit code and stdout.
fn run_in(dir: &Path, args: &[&str]) -> (Option<i32>, String) {
    use std::process::{Command, Stdio};
    // stdout goes to a file, so a child that never exits cannot block on a full pipe.
    let stdout = dir.join("stdout.txt");
    let mut child = Command::new(env!("CARGO_BIN_EXE_intel8080"))
        .env_remove("ANTHROPIC_API_KEY")
        .args(args)
        .current_dir(dir)
        .stdin(Stdio::null())
        .stdout(std::fs::File::create(&stdout).unwrap())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    let start = std::time::Instant::now();
    let status = loop {
        if let Some(status) = child.try_wait().unwrap() {
            break status;
        }
        if start.elapsed() > DEADLINE {
            let _ = child.kill();
            let _ = child.wait();
            panic!("{:?} still running after {:?}; stdout so far:\n{}", args, DEADLINE,
                String::from_utf8_lossy(&std::fs::read(&stdout).unwrap()));
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    };
    (status.code(), String::from_utf8_lossy(&std::fs::read(&stdout).unwrap()).into_owned())
}

#[test]
fn rom_flag_runs_the_image_to_its_pass_hlt() {
    let (code, out) = emulator(&[], "");
    assert_eq!(code, Some(0), "{}", out);
    assert!(out.ends_with("\nHLT at PC=0118\n"), "{}", out);
    // A missing image exits 2, like a missing script.
    let (code, _) = emulator(&["--rom", "missing.bin"], "");
    assert_eq!(code, Some(2));
}

#[test]
fn rom_flag_trace_has_the_bench_fetch_sequence() {
    // The HARDWARE_BUILD.md 3.3 command: the first 256 instructions from the ring. The bench
    // compares their addresses with the analyzer's status A2 rows.
    let (code, out) = emulator(&["--script", "s.dbg"], "s 100\nring\nq\n");
    assert_eq!(code, Some(0), "{}", out);
    let ring: Vec<&str> = out.split("dbg> ring\n").nth(1).unwrap().lines().take_while(|l| !l.is_empty() && !l.starts_with("dbg> ")).collect();
    assert_eq!(ring.len(), 256, "{}", out);
    assert!(ring[0].starts_with("0000  C3 0A F0  JMP F00A "), "{}", ring[0]);
    assert!(ring[1].starts_with("F00A  DB FF     IN FF "), "{}", ring[1]);
    let pcs: Vec<u16> = ring.iter().map(|l| u16::from_str_radix(&l[..4], 16).unwrap()).collect();
    // Landmarks: one fetch from the mirror (0000), then straight-line code through the
    // overlay and window checks, then the M0 loop (F029-F02E) over and over.
    let prologue = [
        0x0000, 0xF00A, 0xF00C, 0xF00E, 0xF011, 0xF013, 0xF015, 0xF017, 0xF01A, 0xF01C, 0xF01E, 0xF020, 0xF022, 0xF024,
        0xF026,
    ];
    assert_eq!(pcs[..prologue.len()], prologue);
    let m0 = [0xF029, 0xF02A, 0xF02B, 0xF02C, 0xF02E];
    for (n, &pc) in pcs[prologue.len()..].iter().enumerate() {
        assert_eq!(pc, m0[n % m0.len()], "ring line {}", prologue.len() + n);
    }
    assert!(ring[4].starts_with("F011  D3 FE     OUT FE "), "{}", ring[4]);
}

#[cfg(unix)]
#[test]
fn bench_trace_recipe_finishes_from_a_terminal() {
    // The HARDWARE_BUILD.md 3.3 recipe as written, run from a terminal: stdin is the pty and
    // stdout goes to the trace file, so a script that runs out leaves the debugger waiting at
    // a `dbg>` prompt nobody sees. `cargo run --` is the binary, /tmp a temp dir.
    let doc = include_str!("../docs/HARDWARE_BUILD.md");
    let block = doc.split("**Emulator trace.** From the repo root:\n\n```\n").nth(1).unwrap();
    let recipe = block.split("\n```").next().unwrap();
    assert_eq!(recipe.lines().count(), 3, "{}", recipe);
    let dir = tempfile::tempdir().unwrap();
    let recipe = recipe.replace("cargo run --", r#""$0""#).replace("/tmp/", &format!("{}/", dir.path().display()));
    let mut cmd = std::process::Command::new("sh");
    cmd.args(["-c", &format!("{}\necho exit=$?", recipe), env!("CARGO_BIN_EXE_intel8080")]);
    cmd.current_dir(env!("CARGO_MANIFEST_DIR"));
    cmd.env_remove("ANTHROPIC_API_KEY");
    let mut p = match rexpect::session::spawn_command(cmd, Some(10_000)) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("skipped: no pty ({})", e);
            return;
        }
    };
    let out = p.exp_string("exit=0").expect("the recipe did not finish");
    let pcs: Vec<&str> = out.lines().map(str::trim).filter(|l| !l.is_empty()).collect();
    assert_eq!(pcs.len(), 256, "{}", out);
    assert_eq!((pcs[0], pcs[1], pcs[255]), ("0000", "F00A", "F029"), "{}", out);
}

#[test]
fn rom_flag_loads_the_images_own_symbols() {
    // ARCHITECTURE 7.2, ROM image: FILE's .sym, never rom/monitor.sym, which is here too.
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir(dir.path().join("rom")).unwrap();
    std::fs::copy("rom/monitor.sym", dir.path().join("rom/monitor.sym")).unwrap();
    std::fs::copy(IMAGE, dir.path().join("x.bin")).unwrap();
    std::fs::write(dir.path().join("s.dbg"), "sym F00B\n").unwrap();
    let args = ["--rom", "x.bin", "--script", "s.dbg"];
    let (code, out) = run_in(dir.path(), &args);
    assert_eq!(code, Some(0), "{}", out);
    assert!(out.contains("\ndbg> sym F00B\nF00B\n"), "no x.sym: no names\n{}", out);
    std::fs::write(dir.path().join("x.sym"), "F00A START\n").unwrap();
    let (code, out) = run_in(dir.path(), &args);
    assert_eq!(code, Some(0), "{}", out);
    assert!(out.contains("\ndbg> sym F00B\nF00B START+1\n"), "{}", out);
}

#[test]
fn rom_flag_bad_image_or_symbols_exit_2() {
    // ARCHITECTURE 7.2, ROM image and 7.4, Symbols: printed errors and status 2, never a
    // panic (101) or a run with no ROM chip.
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("empty.bin"), b"").unwrap();
    std::fs::write(dir.path().join("big.bin"), vec![0x76; 0x1001]).unwrap();
    std::fs::write(dir.path().join("max.bin"), vec![0x76; 0x1000]).unwrap(); // HLT at 0000
    for args in [&["--rom", "empty.bin"][..], &["--rom", "empty.bin", "--jp-we"], &["--rom", "big.bin"]] {
        assert_eq!(run_in(dir.path(), args).0, Some(2), "{:?}", args);
    }
    let (code, out) = run_in(dir.path(), &["--rom", "max.bin"]);
    assert_eq!(code, Some(0), "{}", out);
    assert!(out.ends_with("\nHLT at PC=0001\n"), "{}", out);
    for sym in [&b"F000 START ; comment\n"[..], b"\xFF\xFE\n"] {
        std::fs::write(dir.path().join("max.sym"), sym).unwrap();
        assert_eq!(run_in(dir.path(), &["--rom", "max.bin"]).0, Some(2), "{:?}", sym);
    }
}
