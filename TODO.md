# TODO

## Open Decisions (Mike decides before anyone codes against them)
- The 2026-10-02 set closed 2026-10-02 and the 2026-10-03 sets closed 2026-10-03; see COLLABORATION_LOG Key Decisions. The specs are docs/ARCHITECTURE.md, docs/DEVICE_SPECS.md, docs/MONITOR_SPEC.md, docs/PI_DAEMON.md.
- [x] Pi daemon spec, 9 questions (build-bus-clock, gpio-seam, reset-check-cost, cdev-interface, listen-default, cross-build, measure-mode, fourth-normative-doc, console-input-arrival, device-send-wording): closed 2026-10-03, Mike accepted every recommendation (Key Decisions, "Pi Daemon Specified"). Written into docs/PI_DAEMON.md, ARCHITECTURE 6.4/6.6/7.4, DEVICE_SPECS 3/4/8/10, HARDWARE_BUILD 3/5.
- [x] Phase 7 spec, 10 questions (Q-ALIAS, Q-HSUFFIX, Q-REGNUM, Q-ADDR-DEFAULT, Q-U-ARGS, Q-U-BADCOUNT, Q-A-LOOP, Q-DIS-SHAPE, Q-READY, Q-LINE-FN): closed 2026-10-03, Mike accepted every recommendation (Key Decisions, "Phase 7 Specified"). Written into DEVICE_SPECS 8, MONITOR_SPEC 6.16-6.17, and the cross-doc edits.
- [x] Idle wait slowed compute-bound programs 15x (2026-10-03): closed 2026-10-03, the wait now also requires an `IN 02` read in the last pump interval (ARCHITECTURE 7.2); the 26M-step loop is back to no-wait speed.
- [x] Debugger NAME+n only within one memory-map region (2026-10-03): accepted by Mike 2026-10-03 (ARCHITECTURE 7.4 Location).
- [x] HEX guard wording: closed 2026-10-03, ARCHITECTURE 1 now says "would write into either range".
- [x] HEX "nothing is written on any failure": closed 2026-10-03, MONITOR_SPEC 7.2 now says nothing outside the stack page (option B, matches the ROM).
- [x] HEX 7.1 vs READ_LINE control characters: closed 2026-10-03, documented, not rejected. MONITOR_SPEC 7.1 now says control characters never reach the stored line; `hex.txt` loads a record with an embedded Tab, Esc and NUL and one corrected with BS and DEL.
- [x] HEX `Line too long` wording: closed 2026-10-03, renamed `Record too long` (ROM, MONITOR_SPEC 5/7, transcripts).
- [x] MONITOR_SPEC 6.15 "No injectable clock is needed": closed 2026-10-03, 6.15 now says transcripts match the shape (they run on hardware) and device-level tests may inject a clock.
- [x] Pi daemon `TIME` clock: closed 2026-10-03, 64-bit Raspberry Pi OS; set = kernel NTP-synchronized (`adjtimex()` not `TIME_ERROR`), else 83; local time per the Pi's TZ set at install (DEVICE_SPECS 8 TIME clock, HARDWARE_BUILD 5).
- [x] Phase 6 literal readings (same-line `Service error`, placeholders 80, NUL on 10 appended, OUT 10 in AVAIL/DONE/ERROR, 81 > 80/82 > 83, year padding and > 9999 = 83, clock field contract): confirmed normative 2026-10-03, written into MONITOR_SPEC 6.15 and DEVICE_SPECS 8.
- [ ] MONITOR_SPEC 6.17 U cost (2026-10-03, docs vs code; not changed): 6.17 says "One line costs about 9,500 cycles (4.6 ms at 2.048 MHz ...), so the default 8 lines take about 37 ms". Measured on v0.6 in the emulator (cycle deltas between `U 0200 n` and `U 0200 n+1`): about 4,450 (NOP) to 5,600 (LXI SP,EFFE) cycles per line (2.2-2.7 ms); 8 NOP lines about 36k cycles (about 18 ms), excluding the command line. The 6.16 A figure (about 3,800 cycles for `MVI A,0D`) matches: 3,760 measured. Mike: replace the 6.17 figure with the measurement?

## Current
- [x] Apply alignment package (archived: docs/archive/HANDOFF_2026-10.md)
- [x] Untrack build artifacts: `git rm --cached rom/monitor.lst rom/monitor.p`
- [x] Spec/state review (2026-10-02). Decisions in COLLABORATION_LOG Key Decisions.
- [x] Rebuild stale `rom/monitor.bin` (it predated `X -` unmount)
- [x] Close open decisions; solidify spec into 3 normative docs (2026-10-02)
- [x] Code review: emulator + ROM against the new specs (steps A-E, 2026-10-03)
- [x] Spec the debugger (ARCHITECTURE Host-Side Conveniences), then build it before Phase 5: core + watchpoints + I/O break/trace + trace ring + ROM symbols; line prompt + script mode. Done 2026-10-03: ARCHITECTURE 7.4, `src/debugger.rs`, `src/disasm.rs`, `rom/monitor.sym`, `tests/debugger_tests.rs`
- [x] Hardware-alignment pass (2026-10-02): design buildable with one hard fix (WAIT set via STSTB, not SYNC). Spec edits in ARCHITECTURE section 6 and DEVICE_SPECS; BOM and bring-up in docs/HARDWARE_BUILD.md
- [x] 8080A/8224/8228 hardware reference: docs/reference/8080_HARDWARE.md
- [x] Implement the 2026-10-03 decisions (2026-10-03): idle wait in the pump; HLT opens the prompt in an interactive run; a bad `--script` line exits 2; workspace labels in `monitor.asm` (ROM bytes identical but DATE/TIME, `monitor.sym` gains 9 names); the 7.3 repeat rule; MONITOR_SPEC 4.4 rule 4 and 6.1 wording
- [x] Specs integrated (2026-10-03): docs/PI_DAEMON.md installed as the fourth normative doc; Phase 7 spec text in DEVICE_SPECS 8 and MONITOR_SPEC; cross-doc edits; Key Decisions. Docs only.
- [ ] Pi daemon `pi8080d` per docs/PI_DAEMON.md (moved from Someday 2026-10-03). In order: `build_bus(storage_dir, clock)` with main.rs and the three harnesses passing `mailbox::local_time` (PI_DAEMON 6); `debugger::Trace` made `pub(crate)` (9); `src/pi/mod.rs` (the `Gpio` trait, register and pin constants, `setup_pins`, `serve`, RESET handling, console pass, stop), `src/pi/linux.rs` (`GpioMem`: mmap, the v2 line request found by the `pinctrl-bcm2711` label, the kernel-ABI const asserts, `ntp_local_time`), `src/pi_main.rs`, `default-run` in Cargo.toml (2); `tests/sim/mod.rs` (SimBoard, 13.1), `tests/pi_daemon_tests.rs` (13.3), `every_transcript_through_the_daemon` with `Mon.side` (13.2) and the `#[ignore]` `w_command_cycles` helper (12.2)
- [ ] Mike: add the PI_DAEMON 2 aarch64 musl `cargo check`/`cargo clippy` gate to CLAUDE.md (Rules, Build, End of Session step 1) before the first `src/pi/` commit. Left to Mike because it changes CLAUDE.md's rules
- [ ] Verify the cross build (PI_DAEMON 2; not a decision): `rustup target add aarch64-unknown-linux-musl`, `.cargo/config.toml` linker = `rust-lld`, `cargo build --release --target aarch64-unknown-linux-musl --bin pi8080d`; then on the Pi, local time under musl reads `/etc/localtime` (PI_DAEMON 14). Fallback: build natively on the Pi
- [x] Loose ends (2026-10-03): `cargo clippy --all-targets` clean (Default for Intel8080, Debugger, IoBus, Console; a `MountCase` alias in `device_tests.rs`). Real-terminal tests `tests/terminal_tests.rs` (8): the binary under a pty via `rexpect` (Unix-only dev-dependency), wrapped in `sh` so each test checks the exit status and `stty -g` before = after; raw mode boot, echo and run, Ctrl-C mid `JMP $`, Ctrl-E / bad line / `c`, interactive HLT / `q`, Backspace 7F -> 08 at `IN 01` and in a line edit, a piped `--script c` run leaving the terminal mode alone. No pty: prints `skipped: no pty` and passes; libtest captures that line, so the 266 count looks the same either way (`cargo test --test terminal_tests -- --nocapture` shows it). cargo-mutants `src/main.rs`: 8 missed -> 0 (65 caught, 5 timeouts = arg-loop hangs and a quit/resume swap that hangs the piped tests, 6 unviable)

## Done: Phase 7 - Development Tools (2026-10-03)
Budget was +328 (sketch); shipped at +322 (2571 -> 2893).
1. [x] `src/disasm.rs`: `assemble(line)` (the OPCODES table searched from 00; a token must equal the entry's text or be a 1-4 digit hex number, so bad bytes, empty operands and operands with spaces fall out of the match) and `line(addr, bytes, name)`, moved out of `Debugger::insn`; the debugger calls it with its symbol lookup, DIS with `|_| None`. `disassemble` stays public (debugger tests use it). Debugger output and tests unchanged
2. [x] Mailbox `ASM` and `DIS` in `mailbox.rs`: the command word and the argument string split at the first 20h; `TIME`, `ASM`, `DIS` with a missing or bad argument string give 82; never 83. No test knob. Header says TIME, ASM and DIS complete within the execute access
3. [x] ROM: MB_SEND/MB_PUT/MB_GET plus MB_HEX (the reference client, MONITOR_SPEC 9); T on it with its port sequence unchanged (`t_runs_the_reference_client` untouched); A (6.16) and U (6.17); `Invalid instruction`; A and U help lines; banner v0.6; MONITOR_SPEC Scope, 1.1 and Status. Bytes: dispatch 10, T -25, MB_SEND/MB_PUT 13, MB_HEX 23, MB_GET 19, A 81, U 100, command strings 15, help lines 64, `Invalid instruction` 22
4. [x] `tests/mailbox_tests.rs` (+4): every ASM/DIS vector plus 5-digit numbers with leading zeros, the 128/129-byte ASM boundary, every byte outside 20-7E, the 21-30 byte DIS response; R1 and R2 over all 768 cases at port level. `placeholder_commands_are_unknown` with GET and ASK only
5. [x] `tests/monitor_tests.rs` (+9) and transcripts: `assemble.txt`, `unassemble.txt` (every 6.17.1 transcript row; the paste regression with LF and CR LF ends and a mid-paste `Invalid instruction`, then `X` reports nothing mounted; the roadmap success criterion); the ports, scripted, Identity and Round trip rows as Rust tests (`t_against` became `scripted(statuses, bytes) -> Mon`); rule 4 rows in `argument_errors_...`; `help.txt` gains A and U
6. [x] Spec quotes in tests updated (mailbox_tests header, `status`, `execute`, TIME tests; monitor_tests 6.15 step 1, Messages, MB_GET outcomes)
7. [x] Docs: README, CLAUDE.md Status, MONITOR_SPEC Scope/1.1/Status, ARCHITECTURE 7.4 and 8, DEVICE_SPECS 8 and 10 pending notes dropped, roadmap Phase 7 done
8. [x] Mutants: cargo-mutants `disasm.rs` + `mailbox.rs` (excluding `local_time`) 62/62 caught after dropping three redundant checks (`assemble`'s character filter and empty/space operand filter, `number`'s empty check) that left equivalent mutants, and adding `LXI H,00001`/`MVI A,0000D`. ROM: 201 generated mutants over MB_*, A, U, T and dispatch (delete, flip condition, immediate +-1, INX/DCX, STC/CMC, JMP/RET): 4 unviable, 193 killed; 2 survivors fixed by tests (`MOV B,D` in U's count: `u_counts_instructions_past_ff`; `JNZ ERR_SERVICE` after U's last line: a last-line failure row), 2 equivalent (`MVI B,0` before `DAD B` in U, B is already 0 from the CU_BYTE loop; `RC` in TO_HEX_DIGIT, pre-existing)

## Done: Phase 5 - Intel HEX Loader (2026-10-03)
Budget was ~250 bytes, from a 176-byte sketch that left out the five messages and the help line (126 bytes the spec requires). Shipped at +289 (2165 -> 2454): 163 code, 87 the five messages, 39 the help line. Built from three candidate implementations: the two-pass one as the base, grafts and tests from the other two.
1. [x] Fix the vacuous tests (Review findings) and add a monitor test that maps storage and checks `X -` prints `Unmounted`. That test is the guard against shipping a stale bin. (2026-10-02: `tests/transcripts/storage.txt`)
2. [x] Edge tests for what the loader leans on: READ_LINE (full buffer, BS, CR/LF/CRLF) and TO_HEX_DIGIT (lowercase, invalid): `line_input.txt`, `hex_math.txt`. DEL shipped with the READ_LINE DEL fix (2026-10-03).
3. [x] MAIN_LOOP: a line starting with `:` dispatches to the loader (`HEX_RECORD`). `:` is tested first: a paste is many lines.
4. [x] `HEX_PAIR`: exactly 2 digits from (HL) into A, HL+=2, CY on error, trashes B. The checksum sum lives in the caller's loop, not in HEX_PAIR.
5. [x] Pass 1: MONITOR_SPEC 7.2 steps 1-4 (LL syntax, LL <= 22h, every pair + end of line, checksum). No writes.
6. [x] Pass 2: re-read the header, steps 5-6 (type 00/01, guard 0100-EEFF without wrap), then write type 00 data.
7. [x] Type 01 prints `Loaded`; the five HEX messages per MONITOR_SPEC 5.
8. [x] Tests: `tests/transcripts/hex.txt` (every 7.5 vector, CRLF, LF and CR pastes, bad record mid-stream, truncation inside a paste, step-order cases incl. a wrong CC followed by junk, guard edges, LINE_BUFFER targets, LAST_DUMP_ADDR and LAST_EXAM_ADDR untouched), `hex_records_are_validated_before_any_write` (debugger: no memory write and no OUT but the console before HR_WRITE), `hex_guard_sweep` (step 6 on every page, 5,120 records). Mutants of the loader: 18/18 killed.
9. [x] `monitor.bin` v0.4 rebuilt, size recorded, docs updated.

## Done: Phase 6 - Time (2026-10-03)
1. [x] Service Mailbox device `src/io/devices/mailbox.rs`, mapped at 10-13 by `build_bus`: 128-byte buffer with overflow flag (81), execute from any state, clear = power-on state, IN 13 per state, command word before the first 20h, `TIME` exact or 82, empty/lowercase/unknown 80. No worker, no BUSY: TIME completes within the execute access.
2. [x] Clock: `Mailbox::new(clock: fn() -> Option<(u16, u8, u8, u8, u8, u8)>)`, (year, month, day, hour, minute, second); the device formats the 19 bytes, so the padding is device code the Pi daemon shares and tests can reach. A plain fn is the simplest seam: no trait, no Box, the default is `mailbox::local_time` (host `localtime_r` via the `libc` crate, already in the lock through crossterm), and a test passes a fixed or a failing closure. None means "clock not set" (83).
3. [x] ROM: `CMD_TIME` is the DEVICE_SPECS 8 reference client inline (one user until Phase 7), `Service error` tail, dispatch, help line, v0.5. +115 bytes (2456 -> 2571).
4. [x] Tests: `tests/transcripts/time.txt` (new `\d` escape: any decimal digit, expected output only), the reference-client port sequence (`t_runs_the_reference_client`), 83 -> `Service error`, and a scripted device (`t_prints_service_error`, `t_handles_every_status_the_reference_client_does`) for BUSY, an empty response, binary bytes, 00 after execute and an error mid-response. ROM mutants of T: 20/20 killed. Device hand mutants: 13/16 killed; the 3 survivors are equivalent today (an empty response -> DONE is unreachable with TIME; clearing the response on an error and IN 13 outside AVAIL are unobservable because the response is empty whenever the status is not AVAIL).
5. [x] Reconciled with an independent black-box test set written from the specs alone: `tests/mailbox_tests.rs` (28 tests + 1 year-padding test + the placeholder-word test, moved from `device_tests.rs` in review), `tests/transcripts/mailbox.txt`, 5 T tests in `monitor_tests.rs`. No spec disagreement with the implementation. One change: the clock returned 19 preformatted bytes, which made the zero-padding test check its own formatter, so formatting moved into the device (item 2). Duplicates removed: 10 of the 11 implementer mailbox tests in `device_tests.rs` (the placeholder-word case stays there), `time_runs_the_reference_client` and `time_with_the_clock_not_set_is_service_error` (subsumed by the black-box `t_` tests). Device mutants re-run on the merged set: 11/11 killed.
6. [x] Review (ship, all low): dropped `Script`, `time_handles_every_status` and `t_prints_one_time_line` (a 49-mutant ROM campaign kills the same 48 without them; the survivor is the banner version, which 1.1 forbids tests to match); dropped the partial year > 9999 -> 83 guard (field range logged in Open Decisions); `local_time` converts with `time_t::try_from`, so a 32-bit time_t past 2038 gives 83 instead of 1901; the mailbox tests all live in `mailbox_tests.rs`; transcript headers say the Pi clock must be set on hardware.

## Decided, to implement (from the specs; small, not Phase 5)
Emulator:
- [x] `mailbox::local_time` reports "not set" for a year above 9999: done 2026-10-03 (filter in `local_time`; unreachable before 10000, so untested).
- [x] Delete `src/io/devices/timer.rs` and its port hooks in `src/cpu.rs` (ports 0x30-0x32, tick at ~:947). Keep a CPU interrupt input `interrupt(rst)` with 8080A acceptance (EI delay, HLT wake, 11 cycles); tests only.
- [x] HLT: `execute_one` fetches nothing while halted; `run()` returns on halt; main.rs prints `HLT at PC=xxxx`, restores the terminal, exits; `perform_hlt` stops printing.
- [x] Host input pump + Ctrl-C quit in the emulator run loop; host key map per ARCHITECTURE; the run loop returns a quit/halted status (ARCHITECTURE 7.2). Done 2026-10-03: `run_loop`/`map_key` in `src/main.rs`; piped stdin feeds the console unmapped
- [x] Console output 8-bit transparent (was `value as char`, so bytes 80-FF went out as 2 UTF-8 bytes)
- [x] `IoBus::map_port` panics on 0xFE/0xFF (CPU-owned ports) instead of silently never being called
- [x] Power-on: `reset()` = RESET pin only (PC, INTE, halted, overlay, pending interrupt); `new()` calls `reset()`; no zeroed registers or preset SP; the harness fills RAM with junk (ARCHITECTURE 3.1). Harness side done: RAM and A-L = 76, flags = D7, SP = 0000 at boot
- [x] Merge Storage + StorageMount into one device serving 08-0F (drop the `Rc<RefCell<Storage>>`)
- [x] One Console (input VecDeque + output buffer, no crossterm); host polling, key map and Ctrl-C/E in main.rs; delete `test_console.rs` (and the unused `null.rs`)
- [x] `scripts/fetch_exercisers.sh` (pinned SHA-256) + `#[ignore]` `tests/exerciser.rs`. Write the CP/M shim in 8080 assembly, not Rust hooks, so the same bytes run on hardware: 0005 JMP to a print routine at <= EEFF; 0000 JMP F000 (cold start; not HLT, which hangs hardware with no INT source). Goal: 8080EXM all PASS; the exercisers are also the chip-acceptance test. Done 2026-10-03: all four pass. WARM has no fixed address and gets no vector (ARCHITECTURE 2, decided 2026-10-03), so the shim re-enters through cold start.
- [x] Console: 2 MiB output cap, discard on full; clear output on reset (reset rebuilds the Console)
- [x] Device reset = rebuild the IoBus and devices from config (no `reset()` on IoDevice, no Send). Storage `Drop` does `File::sync_all`; flush uses `sync_all` (`File::flush` was a no-op)
- [x] One port-mapping function shared by main.rs and a future Pi daemon binary (one crate, two binaries). Done 2026-10-03: `build_bus` in `src/io/mod.rs`, used by main.rs and all three harnesses
- [x] Strict monitor harness stores transcripts as data files, so the same files drive hardware conformance over the Pi TCP console (`tests/transcripts/*.txt`, format in the `tests/monitor_tests.rs` header)
- [x] Port 0xFE: any write disables overlay; drop 0xFF cold reset (`src/cpu.rs:758-769`). Overlay writes go through to RAM (`src/cpu.rs:212`)
- [x] Storage: every IN/OUT 0B advances the address (mounted or not); any host I/O error unmounts; flush/unmount fsync; host file > 16 MB fails mount with 01. Tests: power-on 0C=82, storage dir created at startup (now in `Storage::new`)
- [x] Mount: names > 12 chars -> 02 (was truncated); failed mount unmounts previous; every OUT 0E (any value) clears the name buffer; uppercase before validate/open; 0F reads 01 at power-on
ROM:
All done 2026-10-03 (step E), with transcripts or monitor tests for each:
- [x] G pushes WARM (`WARM: LXI SP,STACK_TOP` before MAIN_LOOP) before PCHL
- [x] CONOUT = `OUT 00h / RET`. The first Pi access after reset is now the banner's OUT 00 (`boot_io_is_out_fe_then_console_output`)
- [x] X: `OUT 0Eh,03h` before the name; 02 -> "Invalid filename", other nonzero -> "Mount failed"; "File not found" deleted. `io_break_and_port_trace_of_a_mount` has the `OUT 0E 03` line
- [x] L/W: read 0C after transfer (W after flush); bit 0 = 0 -> "Storage error"
- [x] Argument strictness per MONITOR_SPEC (Invalid range for end<start and count 0 incl. M; digit limits; byte args > FF; G garbage)
- [x] M copies backward when dst > src (memmove)
- [x] Delete unused `BUFFER_PTR` equate; `ROM_END` label, `make size` reports used bytes
- [x] `monitor.asm` header: memory map per ARCHITECTURE 1, routine-contract convention stated

## Review findings (2026-10-02)
Every item was reproduced by a scratch test or confirmed by tracing the asm.

### CPU (`src/cpu.rs`)
- [x] AC inverted on SUB/SBB/CMP/SUI/SBI/CPI (:349, :356, :374, :613, :643, :750). Repro: `MVI A,10h; MVI B,01h; SUB B` sets AC=1, expected 0. `tests/cpu_tests.rs:317` and `:2853` assert the wrong value.
- [x] DCR AC inverted (:422). Repro: `MVI B,10h; DCR B` sets AC=1, expected 0. `tests/cpu_tests.rs:421` asserts the wrong value.
- [x] ANA/ANI always clear AC (:362, :622 via `update_flags_logical` :272). Repro: `MVI A,08h; MVI B,00h; ANA B` gives AC=0, expected 1 (8080: AC = bit 3 of A|operand).
- [x] DAA always clears AC (:712). Repro: `MVI A,0Ah; DAA` gives A=10h (correct) and AC=0, expected 1.
- [x] POP PSW keeps flag bits 3 and 5 (`set_psw` :89). Repro: FFh FFh on the stack, `POP PSW; PUSH PSW` pushes flags FFh, expected D7h. `tests/cpu_tests.rs:2605` is vacuous: it writes 00h at the wrong SP.
- [x] Undocumented aliases panic (:944). Repro: `CB 05 00` panics "Unknown opcode", expected JMP. Same for D9 (RET) and DD/ED/FD (CALL).
- [x] EI has no one-instruction delay (:805, interrupt check at the top of `execute_one`). Repro: pending interrupt, `EI; MVI A,1` is taken before MVI runs. Breaks `EI; RET` ISR epilogues.
- [x] HLT is terminal (`run` :284, `handle_interrupt` :290). An interrupt never clears `halted`; `execute_one` keeps executing past HLT. Repro: `76 3E 01 76`, execute_one x2 gives A=01, expected 00.
- [x] Interrupt acknowledge adds 0 cycles (`handle_interrupt` :290). Expected 11 for RST.

### Devices
- [x] `..` and `.` pass validation, mount fails with 0x01 instead of 0x02. Repro: name `..`, mount: status 01. Fixed 2026-10-03.
- [x] Flush and write errors are swallowed (flush had no fsync). W prints "Written" regardless. Device side fixed 2026-10-03 (errors unmount); the W status check is the ROM item above.
- [x] `map_port` silently ineffective for 0x30-0x32, 0xFE and 0xFF: the CPU intercepts them first (`cpu.rs:758-782`). 0x30-0x32 goes away with the timer deletion.
- [x] Ctrl-C can't stop a program that doesn't poll the console. Repro: `G` into `JMP $`, then ^C: the process keeps running. Fixed 2026-10-03 (host pump).
- [x] Control keys mangled. Repro: Ctrl-A arrives as 0x61, Ctrl-S as 0x73; Esc and Tab are dropped. Fixed 2026-10-03 (`map_key`).

### Monitor ROM (`rom/monitor.asm`)
All fixed 2026-10-03 (step E); line numbers refer to the old ROM. Each repro is now a transcript line.
- [x] L/W count 0 moves 65536 bytes (:1351-1358, :1420-1427). Repro: `L 0 0200 0` wipes the workspace; the next `I 02` cold-boots.
- [x] L/W junk count silently means 256 (:1339, :1408). Repro: `L 0 0200 ZZ` loads 256 bytes.
- [x] D wrap heuristic stops early (:750-756). Repro: `D 0 F000` prints one line.
- [x] S same heuristic (:1123-1129). Repro: `F 0500 0502 77`, then `S 0000 FFFF 77 77 77` finds nothing.
- [x] F with end < start fills to FFFF (:847-861). Repro: `F 0300 0200 AA` fills 0300-EFFF, stack included.
- [x] C with end < start does 64K compares (:561-568). Repro: `C 0201 0200 0300` prints thousands of lines.
- [x] Too many hex digits are silently truncated (READ_HEX_WORD :360-415, READ_HEX_ADDR24 :460-530). Repro: `F 10200 1020F 1AA` fills 0200-020F with AA.
- [x] G with a bad argument runs 0100 (:879). Repro: `G ZZ`.
- [x] E: LF counts as "advance", and CR after a 2-digit value skips a byte (:1171, :1195-1198). Repro: `E 0200`, then `12` CR `34` CR `.` writes 0200=12, 0202=34.
- [x] READ_LINE stores DEL (0x7F) as a character (:299-302). Real terminals send DEL for backspace. Repro: `D 020<DEL>0 0200` prints "Invalid address".
- [x] M with an overlapping dest > src corrupts (:953). Repro: `M 0200 0201 0F` smears 0200. Documented as "forward copy" only.
- [x] `make size` always reports 4096 because p2bin pads (`rom/Makefile:34-36`).

### Test gaps
- [x] Vacuous: `test_search_finds_pattern` (`tests/monitor_tests.rs:85`) matches the echoed `F 0500` line; `test_io_read_status` (:107) matches "02" in the banner date; `test_compare_identical` (:95) only counts prompts.
- [x] No monitor test maps storage, so L/W/X in the ROM are untested. This is how the stale bin slipped through.
- [x] Untested: interrupts (RST entry, EI delay, HLT wake, DI blocking), with the interrupt rework. Done 2026-10-03: interrupts and overlay write-through. Done 2026-10-02: cycle counts (all 244 documented opcodes), the overlay at CPU level (reads, IN FF, OUT FE 00), the E command (one-digit entries), every error path that is correct today, range edges. Done 2026-10-03 with the ROM fixes: end < start, FFFF wrap, two-digit E entries.

## Code Review (2026-10-02)
Method: a 16-agent review workflow (CPU first, then ROM and devices) with every finding adversarially verified; real `cargo-mutants`; the four standard 8080 exercisers run under a CP/M shim. Only new findings are listed here; the items above still stand.

### Evidence
- **Exercisers:** TST8080 and 8080PRE pass, with cycle totals matching the reference exactly. CPUTEST fails on `DCR B` AC (flags 46h, expected 56h). 8080EXM fails 11 of 24 groups (aluop nn, aluop reg, daa/cma/stc/cmc, inr/dcr on all 8 targets); every failure is the tracked AC family.
- **cargo-mutants** (CPU + devices): 649 of 932 caught (70.5%). Survivors cluster in perform_alu and the immediate ALU ops (98), dead debug code (32), registers.rs metadata (50) and storage (14).
- **Hand mutation, CPU tests only:** 26% (35% with the monitor tests). **ROM:** 10.6% (7/66), and a strict-transcript harness kills 60/66. After step E (2026-10-03): the implementer's 61 hand mutants of the rewritten ROM all killed. The review's broader campaign (151 valid mutants, every command, parser, RANGE, tails, messages) killed 136; of the 15 survivors 5 are equivalent (DI, stub RET->RNZ, `'z'+1`->`'z'`, ORA->ADD in the E digit insert, bare-D 7F->70), 2 are unreachable with this device (X query and mount status masks; it only returns 00/01/02) and 8 were test gaps (lowercase x fold, E CR-with-no-digits, E `0` CR, E BS mask, M/L/W count low byte 00, C over 65536 bytes). New transcript lines and asserts kill all 8, plus boot `LXI SP` -> EF00 (`boot_assumes_nothing` now checks 0100-EEFF untouched). Before it, after the strict harness shipped: ROM 59/65 hand mutants killed; the 6 survivors are equivalent (DI, `'z'+1`, bare-D 7F->70, E BS mask), unobservable (W flush) or reachable only through end < start (F wrap guard). cargo-mutants `cpu.rs` 647/752; the misses are the tracked AC family, `|`/`^` equivalents, overlay write-through, OUT FE=FF, interrupts and dead debug code.
- **Exhaustive conformance** (all ALU ops x A x v x CY, INR/DCR, DAA, all 256 opcodes for cycles and length, address wrap): the only CPU violations are the ones already tracked. Cycle counts and wrap are correct.

### Tests that don't test (fix before any code change)
- [x] Zeroed RAM is a NOP slide into F000. Removing the overlay or emptying `reset()` passes every test (`cpu.rs:1109`, `memory.rs:12`). Fill RAM with junk (76 or A5) and SP with junk in the monitor harness boot. Add a CPU-level overlay test: read 0100 = rom[100]; write 55; OUT FE nonzero; read 0100 = 55; IN FF bit 0 is 1 then 0. Done 2026-10-02: junk is 76 (HLT), not A5: A5 is ANA L, which slides into F000 like a NOP; `boot_fails_without_overlay` guards it. The write-55 and nonzero-OUT-FE parts ship with the overlay fix.
- [x] Monitor harness: replace the fixed `run_cycles` budget with "run to the Nth prompt, exact transcript between prompts, budget exhausted = failure", plus memory and port effects at both range edges (`tests/monitor_tests.rs:10-30`).
- [x] RST tests are vacuous on the vector (`cpu_tests.rs:842-1035`). Mutant `n*8 -> n*7` survives. Assert PC, SP and the pushed address.
- [x] Conditional CALL/RET taken paths are untested (`cpu_tests.rs:797-830`). `test_rp_not_taken`/`test_rpo_not_taken` actually take the return, because MVI doesn't set flags (`:1549-1594`). Several Ccc tests target the wrong address (`:1414, :1429, :1443, :1458`). Replace with a table: 8 conditions x {Jcc, Ccc, Rcc} x {taken, not}, with flags set explicitly.
- [x] No-assert or off-topic tests: `test_in_instruction`/`test_out_instruction` (`:1769-1792`), `test_mov_all_to_m` (`:2235`), `daa_preserves_flags`, `all_arithmetic_preserve_bit1`. Duplicates: `:1167 = :1912`, among others.
- [x] Monitor tests: `test_dump_rom` (`:42-50`) passes with wrong line length and format, and its "31" check tests ROM layout. `test_move_command`/`test_hex_math` ignore boundaries and format. C, S and I can each be undispatched with the suite green.
- [x] Storage mount tests never check which file opened: ignoring `base_path` passes and writes NEW.BIN/TEST.BIN into the crate root (`storage_mount.rs:60` vs tests `:128-164`). Device protocols: 16 of 18 port-level mutants survive. Write port-level tests with no private-field access. Done 2026-10-02 (`tests/device_tests.rs`): cargo-mutants on storage + mount 74/80; the misses are flush (unobservable), `|` vs `^` on masked bytes (equivalent), and the 12-char truncation (tracked bug).
- [x] `tests/common/mod.rs` is dead, a byte-identical copy of `cpu_tests.rs:1-33`.
- [x] Wanted: a table-driven reference-model flag test, a 256-opcode cycle/length table test, and an exerciser test (see Open Decisions). The scratch prototypes exist and run in under 1 s. Done 2026-10-02: the reference model (exhaustive ALU, INR/DCR, DAA, DAD, rotates; AC masked for the tracked AC family until its fix) and the cycle/length table for the 244 documented opcodes. Done 2026-10-03: the aliases (all 256 opcodes in the table) and `tests/exerciser.rs` (all four pass).

### CPU (`src/cpu.rs`)
- [x] Untested and mutant-proven: DAD never clearing CY (`:455`), ADC/ACI AC carry-in (`:343, :740`), the CY boundaries on ADC/ADI/SBB/SUI/SBI/DAD, S after logical ops (`:276`), and ORI (`test_ori` uses F0|0F, where OR = XOR).
- [x] Interrupt acknowledge is a hand-written CALL 0038 fused with the vector's first instruction in one step (`:290-303, :826-830`). Spec: acknowledge executes `RST n` as its own 11-cycle step. Use `perform_rst`.
- [x] Pattern: the ALU is implemented twice. 8 immediate functions (`:601-647, :736-754`) duplicate `perform_alu` (`:328-385`), which is why the AC bug has 6 copies. Use one `alu(op, v)` for 10AAASSS and 11AAA110. Also: three SZP flag helpers that are the same code, dead DAA carry re-set, and the push/pop sequence copied 8 times. A scratch refactor came out at +71/-455 lines with tests green.
- [x] Dead code: `disassemble_at` (decodes 19 of 256 opcodes), `trace`, `debug_state` (`:958-1080`); about 85% of `registers.rs` (`FLAG_BIT_3`/`FLAG_BIT_5` are constants equal to 0); the `Memory` trait with one impl forcing `&mut self` reads (`memory.rs`). Stale edit-marker comments.

### ROM (`rom/monitor.asm`)
All done 2026-10-03 (step E): the WARM/error-tail/one-parser/RANGE refactor from the review prototypes, applied with the behavior fixes. 2511 -> 2165 bytes.
- [x] S: an invalid pattern token ends the pattern instead of erroring (`:1057-1061`). `S 0200 0210 AA ZZ` searches for AA. Strict parsing alone doesn't fix it.
- [x] S also tests the candidate end+1: the loop compares before it checks `current > end` (`:1081-1129`). Repro: `F 0500 0502 41`, then `S 0400 04FF 41` prints `0500`. Found 2026-10-02 by the strict harness. The RANGE helper rewrite should fix it; add `S 0400 04FF 41` to `search.txt` with the fix.
- [x] E ignores DEL (`:1173-1177`); spec 6.3 says DEL deletes a digit like BS. Fix with READ_LINE's DEL.
- [x] ROM size guard `IF $ > 0FFFFH` (`:1515-1517`) rejects an exactly-4096-byte ROM, and asl already errors past FFFF. Delete it.
- [x] Dead CONST routine (`:194-202`); single-use PRINT_BANNER wrapper.
- [x] Byte budget, all prototyped with golden transcripts identical:
  - 23 copies of a 9-byte error tail plus POP-cleanup chains. With WARM, use one tail per message and delete every cleanup POP.
  - 19 redundant `CALL SKIP_SPACES` (57 bytes).
  - READ_HEX_ADDR24 duplicates READ_HEX_WORD (89 bytes); use one C:D:E parser returning the digit count.
  - TO_HEX_DIGIT 42 -> 19 bytes.
  - The D default paths are duplicated (21 bytes), the 'AAAA:' prefix is copied 4 times (18), and the I/O stub init is 30 bytes of MVI/STA.
- [x] Contracts: READ_HEX_WORD returns the same flags for "absent" and "junk". Adopt Z=absent / C=invalid (7 peek sites collapse; fixes G, L/W count and S). One RANGE helper (4 uses: C, D, F, S) fixes all four tracked range bugs for -3 net bytes and deletes both `CPI 0F0H` heuristics.

### Devices
- [x] Console and TestConsole duplicate the protocol. The shipped Console has zero coverage and pulls crossterm into the device layer the Pi reuses. Fixed 2026-10-03: one Console, tested at port level.
- [x] StorageMount holds `Rc<RefCell<Storage>>`: it is !Send and splits one Pi device across two structs. Fixed 2026-10-03: one device.
- [x] The W flush (OUT 0C <- 02) can't be observed by any test. Done 2026-10-03: the monitor harness records every port access, and `load_and_write_port_sequences` pins `OUT 0C 02` before the status read. Durability itself is still unobservable (no fault injection).
- [x] IN 02 costs about 1.2 ms of wall time (a 1 ms poll), so console output is throttled to about 850 chars/s. Fixed 2026-10-03: `poll(Duration::ZERO)` every 10,000 steps in the host pump.

## Blocked
- [ ] R command - needs register capture on return (Phase 10)

## Someday
- [ ] Phase 8: HTTP GET (mailbox `GET`). Its background worker must not move the IoBus off the Pi daemon's bus thread (devices are not `Send`, PI_DAEMON 1)
- [ ] Phase 9: Claude (mailbox `ASK`, Q command)
- [ ] Phase 10: R command (the debugger shipped early, ARCHITECTURE 7.4)
- [ ] 8253 timer, TIMER_ISR, RST 7 vector: when something needs a periodic interrupt
- [ ] Hardware prototype: see docs/HARDWARE_BUILD.md (BOM, 9-step bring-up)
- [ ] Pi-assisted hardware single-step (needs A8-A15 on the Pi; not in v1)
- [ ] Interrupt tick source (Pi GPIO vs 8254) when something needs a periodic interrupt
