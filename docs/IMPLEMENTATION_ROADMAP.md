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
| 8 | Internet Services | 🔲 Future |
| 9 | Claude Integration | 🔲 Future |
| 10 | R command (debugger done early) | 🔲 Future |
| 11 | Polish | 🔲 Future |
| - | Pi daemon track (parallel) | 🔲 Specified 2026-10-03 |

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
- R command (registers) - needs return mechanism, implement when debugging requires it

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

**Specified 2026-10-03:** `docs/PI_DAEMON.md`, the fourth normative spec (decisions: COLLABORATION_LOG Key Decisions, "Pi Daemon Specified"). Not started; the task list is in `TODO.md` (Current).

**Success criteria:**
- `cargo test` on any OS: every transcript through the daemon on the simulated board, its trace equal to the emulator's port sequence (PI_DAEMON 13.2), and the 13.3 fault, RESET, startup, stop and console tests.
- The aarch64 musl `cargo check` and `cargo clippy` gate passes (PI_DAEMON 2).
- On the bench: the PI_DAEMON 14 checks, at bring-up steps 5-8 (`HARDWARE_BUILD.md` 3).

---

## Phase 8: Internet Services

HTTP via mailbox `GET`; N command. Large bodies go to a storage file. Designed when the phase starts, including how a hung request ends.

---

## Phase 9: Claude Integration 🎯

The 8080 talks to Claude via mailbox `ASK`; Q command. The API key lives on the coprocessor, never in ROM. Designed when the phase starts.

---

## Phase 10: R Command

The host-side debugger was pulled forward and shipped before Phase 5 (2026-10-03, ARCHITECTURE 7.4): Ctrl-E / `--debug` / `--script`, breakpoints, step, registers, memory, disassembly with ROM symbols, watchpoints, I/O breaks, the port trace and the trace ring.

What is left: the monitor's `R` command (registers, deferred from Phase 3), which needs the `G` return contract to capture them. Not in the debugger v1, each to be decided when a need shows up: a debugger reset (it must reset the devices too, ARCHITECTURE 3.1), writing registers or memory, conditional breakpoints.

---

## Phase 11: Polish & Documentation

Detailed help, self-test, state save/load, example programs, documentation.

---

## The End State

An 8080 system that:
1. Runs the same ROM on emulator and real hardware
2. Stores data to SD card / cloud
3. Fetches data from the internet
4. Talks to Claude for assistance
5. Debugs itself (with emulator help)

The 8080 code is simple. The coprocessor handles complexity. That's the whole point.
