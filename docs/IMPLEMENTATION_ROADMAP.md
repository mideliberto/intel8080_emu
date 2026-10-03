# Implementation Roadmap

## Phase Summary

| Phase | Focus | Status |
|-------|-------|--------|
| 1 | Core Monitor | ✅ Complete |
| 2 | Memory Operations | ✅ Complete |
| 3 | Execution & I/O | ✅ Complete |
| 4 | Storage System | ✅ Complete |
| **5** | **Program Loading** | **🔲 Next** |
| 6 | Time | 🔲 Future |
| 7 | Development Tools | 🔲 Future |
| 8 | Internet Services | 🔲 Future |
| 9 | Claude Integration | 🔲 Future |
| 10 | R command (debugger done early) | 🔲 Future |
| 11 | Polish | 🔲 Future |

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

## Phase 5: Program Loading 🔲 NEXT

**Goal:** Load programs into memory by pasting Intel HEX at the prompt. The 8080 parses it in ROM.

**Decided 2026-10-02** (contract: `MONITOR_SPEC.md`, Intel HEX Loader):
- A line whose first non-space character is `:` is one HEX record. No command letter, no loader mode; each line stands alone.
- Types 00 and 01 only. Max 34 data bytes per record. Accepted writes land only in 0100-EEFF.
- Each record is validated in full before any byte is written.
- No sender pacing: the console is a Pi FIFO behind READY (`DEVICE_SPECS.md`).
- Ships as monitor v0.4.

**Tasks:** see TODO.md Next (build order).

**Success Criteria:** every conformance vector in `MONITOR_SPEC.md` (Intel HEX Loader) passes as a cargo test. A pasted HEX file loads, verifiable with `D`.

---

## Phase 6: Time

**Goal:** The 8080 knows what time it is.

**Tasks:**
- [ ] Service Mailbox device (Rust), ports 0x10-0x13 (`DEVICE_SPECS.md`, Service Mailbox)
- [ ] `TIME` mailbox command
- [ ] ROM mailbox client (the `DEVICE_SPECS.md` reference client)
- [ ] T command (`MONITOR_SPEC.md`)

**Success Criteria:**
- `T` prints `YYYY-MM-DD HH:MM:SS` then CR LF. Tests match the shape `NNNN-NN-NN NN:NN:NN`.
- On a mailbox failure `T` prints `Service error`.

No timer in v1. The Pi keeps wall-clock time via NTP. A periodic interrupt source is Someday, decided when a consumer appears.

---

## Phase 7: Development Tools

Assemble and unassemble via mailbox `ASM` / `DIS`; A and U commands. Designed when the phase starts.

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
