# Implementation Roadmap

## Phase Summary

| Phase | Focus | Status |
|-------|-------|--------|
| 1 | Core Monitor | ✅ Complete |
| 2 | Memory Operations | ✅ Complete |
| 3 | Execution & I/O | ✅ Complete |
| 4 | Storage System | ✅ Complete |
| 5 | Program Loading | ✅ Complete |
| 6 | Time | ✅ Complete |
| 7 | Development Tools | ✅ Complete |
| 8 | Internet Services | ✅ Complete |
| 9 | Claude Integration | ✅ Complete |
| 10 | R command (debugger done early) | ✅ Complete |
| 11 | Polish | ✅ Complete |
| - | Pi daemon track (parallel) | 🟡 Code done 2026-10-03, bench pending |

---

## Phase 1: Core Monitor ✅

**Goal:** Basic monitor with memory operations

**Delivered:**
- ROM skeleton (init, console)
- Command parser framework
- Helper functions (SKIP_SPACES, READ_HEX_WORD, etc.)
- D command (memory dump)
- E command (examine/modify)
- G command (execute)

---

## Phase 2: Memory Operations ✅

**Goal:** Complete memory manipulation suite

**Delivered:**
- F command (fill)
- M command (move)
- S command (search)
- C command (compare)
- H command (hex arithmetic)

---

## Phase 3: Execution & I/O ✅

**Goal:** Program execution and I/O control

**Delivered:**
- I command (input from port)
- O command (output to port)

**Deferred:**
- R command (registers) - needs return mechanism, implement when debugging requires it (shipped in Phase 10)

---

## Phase 4: Storage System ✅

**Goal:** 24-bit linear-addressed storage with file mounting

**Delivered:**

Rust:
- Storage device (ports 0x08-0x0C) with 24-bit addressing
- StorageMount service (ports 0x0D-0x0F)
- Unit tests with tempfile

ROM:
- X command (mount/unmount/query)
- L command (load from storage to memory)
- W command (write memory to storage)
- READ_HEX_ADDR24 helper for 6-digit hex parsing
- STOR_ADDR workspace (3 bytes at 0x00E7)

**Features:**
- 16MB address space (24-bit, linear)
- Auto-increment on read/write
- Filename validation per DEVICE_SPECS 7: uppercased, 1-12 chars of `A-Z 0-9 . - _`, not `.` or `..`. Storage and mount are one device since 2026-10-03 (`src/io/devices/storage.rs`).

---

## Phase 5: Program Loading ✅ COMPLETE (2026-10-03)

**Goal:** Load programs into memory by pasting Intel HEX at the prompt. The 8080 parses it in ROM.

**Decided 2026-10-02** (contract: `MONITOR_SPEC.md`, Intel HEX Loader):
- A line whose first non-space character is `:` is one HEX record. No command letter, no loader mode; each line stands alone.
- Types 00 and 01 only. Max 34 data bytes per record. Accepted writes land only in 0100-EEFF.
- Each record is validated in full before any byte is written.
- No sender pacing: the console is a Pi FIFO behind READY (`DEVICE_SPECS.md`).
- Ships as monitor v0.4.

**Done:** `HEX_RECORD` in `rom/monitor.asm`, two passes over LINE_BUFFER (pass 1: MONITOR_SPEC 7.2 steps 1-4; pass 2: steps 5-6, then the write). Tests: `tests/transcripts/hex.txt` (every 7.5 vector plus edges), `hex_records_are_validated_before_any_write` (debugger watchpoints and I/O breaks) and `hex_guard_sweep`. +289 bytes. From the line to WARM, a 16-byte record takes 20.0k-23.1k cycles (9.8-11.3 ms at 2.048 MHz, before READY wait states), 9.2k-12.3k of them in the loader; a 34-byte record 36.6k-43.1k (17.9-21.1 ms). The spread is the number of A-F digits (TO_HEX_DIGIT: 34 cycles for 0-9, 80 for A-F).

**Success Criteria:** every conformance vector in `MONITOR_SPEC.md` (Intel HEX Loader) passes as a cargo test. A pasted HEX file loads, verifiable with `D`.

---

## Phase 6: Time ✅ COMPLETE (2026-10-03)

**Goal:** The 8080 knows what time it is.

**Tasks:**
- [x] Service Mailbox device (Rust), ports 0x10-0x13 (`DEVICE_SPECS.md`, Service Mailbox): `src/io/devices/mailbox.rs`, mapped by `build_bus`
- [x] `TIME` mailbox command. The clock is a plain fn passed to `Mailbox::new` that returns the date and time fields (the device formats them): `build_bus` passes the host's local time (`localtime_r` through the `libc` crate), tests pass a fixed or a failing one
- [x] ROM mailbox client (the `DEVICE_SPECS.md` reference client), inline in `CMD_TIME`: T is its only user until Phase 7
- [x] T command (`MONITOR_SPEC.md`), help line, monitor v0.5

**Done:** +115 bytes (2456 -> 2571): 56 the client and T, 5 dispatch, 6 the error tail, 16 `Service error`, 32 the help line. Tests: `tests/mailbox_tests.rs` (30 port-level tests written black-box from DEVICE_SPECS 8, each quoting its sentence; exact values on an injected clock, the host-clock value against `date`), `tests/transcripts/time.txt` (T by shape via the `\d` transcript escape), `tests/transcripts/mailbox.txt` (the mailbox through I and O, exact), `t_runs_the_reference_client` (every non-console-poll port access of T), `t_with_the_pi_clock_not_set_prints_service_error` (83), `t_prints_service_error` and `t_handles_every_status_the_reference_client_does` (scripted devices for BUSY, empty and binary responses, 00 after execute, every error code, errors mid-response). ROM mutants of T: 20/20 killed; review campaign over CMD_TIME, dispatch, help, messages and banner: 48/49, the survivor the banner version (1.1: tests never match it).

**Success Criteria:**
- `T` prints `YYYY-MM-DD HH:MM:SS` then CR LF. Tests match the shape `NNNN-NN-NN NN:NN:NN`.
- On a mailbox failure `T` prints `Service error`.

No timer in v1. The Pi keeps wall-clock time via NTP. A periodic interrupt source is Someday, decided when a consumer appears.

---

## Phase 7: Development Tools ✅ COMPLETE (2026-10-03)

**Goal:** Assemble and unassemble on the machine, with the tools running on the Pi.

**Specified 2026-10-03** (decisions: COLLABORATION_LOG Key Decisions, "Phase 7 Specified"): mailbox `ASM` and `DIS` in `DEVICE_SPECS.md` 8; `A` and `U` in `MONITOR_SPEC.md` 6.16-6.17, with the conformance vectors in 6.17.1.

**Tasks:**
- [x] `src/disasm.rs`: `assemble(line) -> Option<Vec<u8>>`, the OPCODES table read backwards (DEVICE_SPECS 8, ASM); `line(addr, bytes, name)` moved out of `Debugger::insn`, so DIS and the debugger share it
- [x] Mailbox `ASM` and `DIS` in `mailbox.rs` (DEVICE_SPECS 8): pure, complete within the execute access, 82 on a bad argument, never 83. No test knob
- [x] ROM: mailbox client MB_SEND/MB_PUT/MB_GET (MONITOR_SPEC 9), T moved onto it with no change to its port sequence; A and U (6.16, 6.17); `Invalid instruction`; help lines; v0.6
- [x] Tests, `tests/mailbox_tests.rs`: every DEVICE_SPECS 8 ASM/DIS vector; R1 and R2 at port level (one loop over `DIS 0000 x y z` and `ASM t` through the rig, no new pub items)
- [x] Tests, `tests/monitor_tests.rs`: the MONITOR_SPEC 6.17.1 rows as marked there. `t_against` became `scripted(statuses, bytes) -> Mon`, so T, A and U tests type into the same scripted rig; the *rule 4* rows join the existing argument-error test; A dialogs in transcripts are single `<` steps

**Done:** +322 bytes (2571 -> 2893, 1203 free): dispatch 10, T -25 (56 -> 31 on the shared client), MB_SEND/MB_PUT 13, MB_HEX 23, MB_GET 19, A 81, U 100, the command strings 15, help lines 64, `Invalid instruction` 22. Measured at 2.048 MHz before READY wait states: an A line of `MVI A,0D` 3,760 cycles from prompt to prompt (READ_LINE and echo 2,776, the mailbox round trip 984); a U line about 4,450-5,600 cycles (8 lines about 36k, 18 ms, after the command line). Tests: `tests/transcripts/assemble.txt` and `unassemble.txt` (every 6.17.1 transcript row, the paste regression with LF and CR LF ends and a mid-paste failure, then `X` finds nothing mounted), 7 Rust tests in `monitor_tests.rs` (both *ports* rows, every *scripted* row, a 256-instruction U, Identity and Round trip over all 256 opcodes), 4 in `mailbox_tests.rs` (every ASM/DIS vector, bytes outside 20-7E, the 21-30 byte DIS response, R1 and R2 over 768 cases).

**Success criteria:**
- `A 0200`, then `MVI A,0D`, `JMP 0200`, `.`; `U 0200 2` prints `0200  3E 0D     MVI A,0D` and `0202  C3 00 02  JMP 0200`.
- For every opcode, U's text typed into A gives back the same bytes, except the R2 aliases (DEVICE_SPECS 8).
- Pasting source with blank lines into A never runs a monitor command.
- `t_runs_the_reference_client` passes unchanged.

---

## Pi Daemon Track (parallel to the phases)

**Goal:** `pi8080d`, the software that runs the emulator's port map behind GPIO on the Pi 4B, so the same ROM runs on the real board.

**Specified 2026-10-03:** `docs/PI_DAEMON.md`, the fourth normative spec (decisions: COLLABORATION_LOG Key Decisions, "Pi Daemon Specified"). **Code done 2026-10-03** (`src/pi/`, `src/pi_main.rs`; the static musl binary links). What is left is the bench.

**Success criteria:**
- [x] `cargo test` on any OS: every transcript through the daemon on the simulated board, its trace equal to the emulator's port sequence (PI_DAEMON 13.2), and the 13.3 fault, RESET, startup, stop and console tests.
- [x] The aarch64 musl `cargo check` and `cargo clippy` gate passes (PI_DAEMON 2).
- [x] `pi8080d --sim` (PI_DAEMON 16, 2026-10-03): the whole Pi stack on the simulated board with the CPU model, so the daemon, unit, console, storage, clock and trace run on a Pi before the board exists; the built binary plays transcripts over TCP in `cargo test`.
- [x] RAM test build (ARCHITECTURE 2.1, 2026-10-03): `rom/monitor_ram.hex` at D000, loaded through the resident HEX loader and run with `G D000`, so ROM changes run on the board without a burn.
- [ ] On the bench: the PI_DAEMON 14 checks, at bring-up steps 5-8 (`HARDWARE_BUILD.md` 3).

---

## Phase 8: Internet Services ✅ COMPLETE (2026-10-03)

**Goal:** The 8080 fetches from the web: mailbox `GET`, the first background command, and the `N` command.

**Specified 2026-10-03** (decisions: COLLABORATION_LOG Key Decisions, "Phase 8 Specified"): `GET`, Background commands and the vectors in `DEVICE_SPECS.md` 8; `N` in `MONITOR_SPEC.md` 6.18, with the vectors in 6.18.1; the worker's process, cores and lifetime in `PI_DAEMON.md` 1 and 11.

**Tasks:**
- [x] `storage.rs`: the mount name rule as one function (`file_name`: fold, check, `Option<String>`), called by mount and by `GET`
- [x] `mailbox.rs`: `Mailbox::new(clock, storage_dir)` (`build_bus` and the harnesses pass it); `GET` grammar (82); the `/usr/bin/curl` worker with `env_clear`, its argument list and flag notes in the module header (`-N` in the stream form: without it curl holds about 4 KB before N prints anything); stream form (non-blocking pipe, 4096-byte reads at `IN 12` only) and file form (remove a stale `~FILE`, curl's size limit, fsync, rename, directory fsync, the 6-digit length); BUSY; abort on execute, clear and `Drop`; on Linux the `pre_exec` (affinity, `PR_SET_PDEATHSIG`). The module header loses "no background worker"
- [x] `mailbox.rs` unit test: the curl argument list, exactly (`curl_argument_list`)
- [x] `main.rs`: `drop(cpu)` before the bad-`--script` `std::process::exit(2)`, so the mailbox aborts its worker (and storage flushes)
- [x] ROM: `N` (dispatch, `CMD_NET` with the `CN_SEND` label Phase 9's Q enters at, `STR_GET`), T's loop prints LF as CR LF and gains `CT_EXEC`; help line; banner v0.7
- [x] Tests: the test HTTP server `H` in `tests/support/http.rs` (shared by the GET and N tests; Phase 9 extends it); `tests/mailbox_tests.rs`: every DEVICE_SPECS 8 GET vector (three `#[ignore]`); `placeholder_commands_are_unknown` keeps only `ASK`
- [x] Tests, `tests/monitor_tests.rs`: every MONITOR_SPEC 6.18.1 row (server rows on a wall-clock deadline); `help.txt` gains the `N` line
- [x] `cargo test -- --ignored` green once (the time limits: 10.0 s connect, 30 s stall before the first byte and mid-body), and the musl `cargo check`/`clippy` gate (the `pre_exec` is Linux-only)

**Done:** +67 bytes (2893 -> 2960, 1136 free), as the sketch measured: dispatch 5, `CMD_NET` 14, `STR_GET` 5, help line 31, T's LF-to-CR LF 12. N prints a body at 155 cycles a byte (206 for an LF), about 13 KB/s at 2.048 MHz before READY wait states. Tests: 15 GET tests and 3 ignored time-limit tests in `mailbox_tests.rs`, 5 N tests in `monitor_tests.rs`, 1 unit test. The 6.18.1 first row's expected output was corrected to what 6.18 steps 2-3 print for the vectors' `/hello` body (`Hello` CR LF: `Hello` CR CR LF, then CR LF), which the file row's `000007` already required. Two more wording changes, both to match the device: the same row's `IN 12` = 01 reads may fall anywhere among the pairs (after the last buffered byte is popped the status is BUSY again), and PI_DAEMON 1 says the device reads the bus thread's mask before the fork.

**Success criteria:**
- [x] `N http://H/hello` against the test server prints `Hello`, and `N http://H/hello > F` then `X F` and `L` load it, in `cargo test`, with no internet.
- [x] Clear, execute and drop kill a running worker within the access (`cargo test`). The time limits are pinned by the argument-list unit test in `cargo test` and shown by the ignored tests (`cargo test -- --ignored`): a request that never answers ends in 83.
- [ ] By hand, not in `cargo test`: `N https://www.gutenberg.org/cache/epub/1342/pg1342.txt > PRIDE.TXT` in the emulator, then `X PRIDE.TXT` and `L`.
- [x] `t_runs_the_reference_client` passes unchanged.

---

## Phase 9: Claude Integration ✅ COMPLETE (2026-10-03)

**Goal:** The 8080 talks to Claude: mailbox `ASK`, the second background command, and the `Q` command. The API key lives on the coprocessor, never in ROM.

**Specified 2026-10-03** (decisions: COLLABORATION_LOG Key Decisions, "Phase 9 Specified"): `ASK`, ASK service and the ASK vectors in `DEVICE_SPECS.md` 8; `Q` in `MONITOR_SPEC.md` 6.19, with the vectors in 6.19.1; the key in `PI_DAEMON.md` 10 and 11.

**Tasks:**
- [x] `ask.rs` and `ask_system.txt`: the prompt rules (82), `MODEL`, `URL`, `AskConfig`, the curl argv and stdin (key and body on stdin, never argv), the body (`serde_json::json!`), the SSE reader, the mapping and the wrapping
- [x] `AskConfig` through `build_bus`, `serve`, `main.rs`, `pi_main.rs` (the key from `ANTHROPIC_API_KEY`, read once); the harnesses pass `AskConfig::default()`; clear keeps it
- [x] `mailbox.rs`: `ASK` beside `GET`, on the same `/usr/bin/curl` spawn (`env_clear`); stdin piped and closed at execute; pipe reads through the reader; DONE or 83 only after the last reply byte
- [x] `serde_json` (the first new crate since `libc`)
- [x] The shared test server's scripted SSE side (`tests/support/http.rs`: recorded requests, `Hold`, `sse`)
- [x] ROM `Q` (dispatch, `CMD_ASK` into N's tail at `CN_SEND`, `STR_ASK`), help line, banner v0.8
- [x] `env_remove("ANTHROPIC_API_KEY")` at every spawn of a built binary (four sites), `ask off` checked in the `--sim` startup line
- [x] `placeholder_commands_are_unknown` deleted (ASK is no longer a placeholder)

**Done:** +50 bytes (2960 -> 3010, 1086 free), as the sketch measured: dispatch 5, `CMD_ASK` 7, `STR_ASK` 5, help line 33. Tests: 5 `ask.rs` unit tests, 8 ASK device tests and 1 `#[ignore]` live test in `mailbox_tests.rs`, 3 Q tests and `ask.txt` in `monitor_tests.rs` (also through the daemon and on the RAM build). `serve` takes eight arguments as PI_DAEMON 2 gives them, under `#[allow(clippy::too_many_arguments)]`.

**Success criteria:**
- [x] Every ASK vector and the five `ask.rs` unit tests pass with no network, with or without `ANTHROPIC_API_KEY` exported.
- [x] Every 6.19.1 row.
- [ ] `ask_live_answers_in_plain_ascii` passes once by hand (`ANTHROPIC_API_KEY=... cargo test --test mailbox_tests ask_live -- --ignored`).
- [x] ROM within 4096 (3010).
- [ ] The PI_DAEMON 14 Q row on the bench.

---

## Phase 10: R Command ✅ COMPLETE (2026-10-03)

**Goal:** see a program's registers on the board, where there is no host debugger.

**Specified 2026-10-03** (decisions: COLLABORATION_LOG Key Decisions, "Phase 10 Specified"): `MONITOR_SPEC.md` 6.20 (R) and 8 (G_RETURN), `ARCHITECTURE.md` 1.1 (REGS).

The host-side debugger shipped before Phase 5 (2026-10-03, ARCHITECTURE 7.4). Its "when a need shows up" items (reset, writing registers or memory, conditional breakpoints) are in Someday in `TODO.md`.

**Tasks:**
- [x] ROM: REGS (`ARCHITECTURE.md` 1.1), G_RETURN, `G` pushes it, CMD_REGS and MSG_REGS, dispatch, help line, banner bump; rebuild and commit `monitor.bin`, `monitor.sym`, `monitor_ram.hex`
- [x] Docs: MONITOR_SPEC Scope, Status, 3, 6.5, 6.14, 6.20, 8, 10; ARCHITECTURE 1.1, 2.1, 3.1, 3.2 and 8; QUICK_REFERENCE; README; PI_DAEMON 13.2 cost line re-measured
- [x] Tests: `tests/transcripts/registers.txt` and `help.txt`; `registers` and `regs_are_written_only_by_a_g_return`; `go_entry_contract` expects G_RETURN; `workspace_symbols_match_architecture_1_1` expects 10 names and `00FF REGS+15`
- [x] `src/io/devices/ask_system.txt` lists R (cross-check C10), so `ask_system_lists_every_command` stays green

**Budget:** +106 bytes, measured on a sketch (G_RETURN 10, CMD_REGS 31, MSG_REGS 28, dispatch 5, help line 32).

**Done:** +106 bytes (3010 -> 3116, 980 free), as measured. Tests: `registers.txt` (also through the daemon and on the RAM build), 2 new monitor tests.

**Success criteria:**
- [x] `G` a program that sets A, the flags, BC, DE and HL and returns with `RET`; `R` prints them as `A=xx F=xx BC=xxxx DE=xxxx HL=xxxx`.
- [x] Every 6.20.1 transcript row passes in the emulator, through pi8080d on the simulated board, and on the RAM test build; the Rust rows pass in the emulator.

---

## Phase 11: Polish & Documentation ✅ COMPLETE (2026-10-03)

**Goal:** someone who has never seen the repo can run the machine, load a program, save their work and
find the spec for anything they see, and every doc agrees with the code. No new ROM code.

**Specified 2026-10-03** (decisions: COLLABORATION_LOG Key Decisions, "Phase 11 Specified").

**Cut, with where each need is met instead:**
- **Detailed help.** `?` stays the one-screen list (MONITOR_SPEC 6.14). Detail: `docs/USER_GUIDE.md`
  and MONITOR_SPEC 6. If a later change runs the ROM out of room, moving `MSG_HELP` behind the
  mailbox is the lever (TODO, Someday), not a Phase 11 task.
- **Self-test.** No ROM self-test. RAM: `examples/memtest`. ROM: `W F000 0 1000` to a storage file,
  then `cmp -n 4096` against `rom/monitor.bin` (USER_GUIDE 6.3). CPU: the four exercisers
  (`tests/exerciser.rs`, HARDWARE_BUILD 3 step 8). Before the Pi exists: the step 3 diagnostic image.
- **State save/load.** W and L are save and load (USER_GUIDE 6.2). No host-side emulator snapshot:
  the 8080 could not see it, the board could not do it, and the debugger plus transcripts already
  reproduce any state worth reproducing.

**Tasks:**
- [x] `examples/`: `hello.asm` returns with `RET` (it ended in `HLT`, which breaks the G return
  contract: the emulator exits, the board stops until RESET); new `memtest.asm`; `Makefile`
  (`asl` + `p2hex -F Intel -l 16`); `hello.hex` and `memtest.hex` committed, as `rom/monitor_ram.hex` is,
  so `cargo test` needs no assembler.
- [x] `tests/transcripts/example_hello.txt`, `example_memtest.txt`: paste the `.hex`, run, check the
  output. As transcripts they also run through the daemon and on the RAM test build. `memtest` runs on
  a short range (0200-0FFF, below the RAM build's image) and on F000-F0FF, where the ROM ignores
  writes, for the `FAIL F000` line.
- [x] `tests/monitor_tests.rs`: `example_hello`, `example_memtest`, `memtest_default_range` (the
  default 0200-EEFF, local path only) and `examples_match_their_hex` (each `examples/NAME.hex` equals
  the `> :` lines of `example_NAME.txt`, in order).
- [x] `docs/USER_GUIDE.md` (non-normative), including N, Q and R, written from the MONITOR_SPEC
  sections Phases 8-10 shipped (6.18, 6.19, 6.20). README's Running, Debugger, Storage System and ROM
  Development sections move into it. README keeps the vision, the status table and a short quick start
  that links it. QUICK_REFERENCE links it.
- [x] Consistency pass (the checklist below), after Phase 10 shipped. Every mismatch goes to `TODO.md`.
  A doc is fixed to match the code only where the spec is unambiguous; otherwise the mismatch goes to
  Open Decisions (CLAUDE.md).
- [x] The 1.0 definition in The End State (below). Writing it down is the task; meeting it is not.

**Consistency checklist** (one pass, by hand or by an agent; no new tests):
1. Messages: every string in MONITOR_SPEC 5 is in `rom/monitor.asm`, every ROM message is in the
   table, and `File not found` is not in the ROM.
2. Commands: MONITOR_SPEC 3 rule 5, the ROM dispatch table, `dispatch.txt`, the 6.14 help block,
   `help.txt`, `MSG_HELP` and the QUICK_REFERENCE table name the same set.
3. Ports: DEVICE_SPECS 1, `build_bus`, MONITOR_SPEC 11 and the QUICK_REFERENCE port map agree.
4. Mailbox: the DEVICE_SPECS 8 Commands table and `mailbox.rs` have the same commands, and no
   placeholder row is left.
5. Placeholders gone once Phases 8-10 ship: MONITOR_SPEC Scope, Status, 3 rule 5 and 10;
   DEVICE_SPECS 3.3 and 8; ARCHITECTURE 3.1 (the Phase 10 reset note) and 8; PI_DAEMON 1 and 15;
   QUICK_REFERENCE Future commands; README "Coming"; this file's phase table.
6. Counts: test counts and ROM bytes in README, CLAUDE.md Status and COLLABORATION_LOG Current State
   equal `cargo test` and `make size`, and the PI_DAEMON 13.2 cost line is re-measured with the
   example transcripts.
7. References: every `FILE.md N.N` reference names a section that exists, including those in
   USER_GUIDE.
8. USER_GUIDE: every monitor session with output lines is copied from the transcript it names.

**Done:** 0 ROM bytes (3116, `make size` unchanged). Two example programs with transcripts on every
path, four tests, `docs/USER_GUIDE.md`, the 1.0 definition. The checklist's findings and the host
commands run by hand are in `TODO.md` (Done: Phase 11).

**Success criteria:**
- [x] `cargo test` passes, with both example transcripts on every path and the four example tests.
- [x] USER_GUIDE meets checklist item 8.
- [ ] Its host commands (`cargo run`, `pi8080d --sim`, socat, the `nc` script form, the RAM test build
  load, `cd examples && make`) have been run once by hand from a clean checkout. All but socat, which
  is not installed on the build Mac (`TODO.md`).
- [x] The checklist is done and its findings are fixed or logged.
- [x] `make size` is unchanged by this phase.

---

## The End State

An 8080 system that:
1. Runs the same ROM on emulator and real hardware
2. Stores data to SD card / cloud
3. Fetches data from the internet
4. Talks to Claude for assistance
5. Debugs itself (with emulator help)

The 8080 code is simple. The coprocessor handles complexity. That's the whole point.

### Monitor 1.0

1.0 is a git tag, not a phase. It marks the commit whose burned image passed every item below.

- **Candidate.** After Phase 11, one commit sets the banner version to `1.0` (MONITOR_SPEC 1.1),
  rebuilds `rom/monitor.bin` and changes nothing else. That exact image is burned and checked. A
  candidate that fails is fixed and rebuilt, still `1.0`, still untagged, and every item is run again
  on the new image.
- **Tag.** `v1.0` goes on the commit whose `rom/monitor.bin` passed. Its COLLABORATION_LOG entry
  records the results.

Criteria, all on one candidate image:
1. Phases 8-11 are complete to their success criteria.
2. `cargo test` is green, the four exercisers pass in the emulator, and the aarch64 musl check and
   clippy gate pass.
3. `make size` is at most 4096.
4. On the board, with the candidate burned: HARDWARE_BUILD 3 steps 5-8 pass (steps 0-4 are board
   construction, done once at build), and so do the PI_DAEMON 14 bench checks.
5. Every file in `tests/transcripts/` replays on the board by the step 6 method (Q11-REPLAY). The
   storage directory is empty and the board is fresh from RESET before each file. The daemon runs
   without `ANTHROPIC_API_KEY`, so no replay reaches the API, and the Pi clock is NTP-synced
   (`time.txt`). Output matches under the harness's rules (`tests/monitor_tests.rs` header: `\d`
   matches any digit). The boot port trace equals the emulator's.
6. Live, on the board: the daemon restarted with the key installed, one `Q` prints an answer, and one
   `N` against a real URL prints its body.
7. F000-FFFF as the 8080 reads it equals the candidate's `rom/monitor.bin` (USER_GUIDE 6.3).
8. `TODO.md` has no open decision and no open "code differs from spec" item.
