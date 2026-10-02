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
| 10 | Debugger | 🔲 Future |
| 11 | Polish | 🔲 Future |

---

## Phase 1: Core Monitor ✅

**Goal:** Basic monitor with memory operations

**Delivered:**
- ROM skeleton (init, vectors, console)
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
- 16MB address space (24-bit)
- Auto-increment on read/write
- High byte acts as bank/page selector
- Filename validation (8.3, safe chars only)

---

## Phase 5: Program Loading 🔲 NEXT

**Goal:** Load programs into memory by pasting Intel HEX at the prompt. The 8080 parses it in ROM.

**Decided 2026-10-02:**
- A line starting with `:` at the prompt is a HEX record. There is no command letter and no loader mode; each line stands alone.
- Records touching 0x0000-0x00FF or 0xEF00-0xFFFF are rejected.
- Max 34 data bytes per record (the 80-char LINE_BUFFER). A longer line gets "Line too long".
- Types 00 (data) and 01 (EOF) only; 02-05 are errors. Each record is validated in full (length + checksum) before any byte is written.
- Paste speed: on hardware the sender paces lines (per-line delay). The console chip and flow control are hardware-build decisions.

**Tasks:** see TODO.md Next (build order).

**Success Criteria:**
```
> :10010000...
> :00000001FF
```
Each data record is accepted silently or reports an error. Bytes land in memory, verifiable with `D`.

---

## Phase 6: Time

**Goal:** The 8080 knows what time it is.

**Tasks:**
- [ ] Service Mailbox device (Rust) - ports 0x10-0x13 (DEVICE_SPECS.md)
- [ ] ROM mailbox routine: send command, poll, stream response
- [ ] `TIME` mailbox command
- [ ] T command (show time)

**Success Criteria:**
- T command shows current time

Decided 2026-10-02: the 8253, TIMER_ISR, TI/TS commands and the RST 7 vector move to Someday until something needs a periodic interrupt. The Pi keeps wall-clock time via NTP.

---

## Phase 7: Development Tools

**Goal:** Assembly and disassembly via the Service Mailbox

**Tasks:**
- [ ] `ASM` / `DIS` mailbox commands (Rust)
- [ ] A command (assemble line)
- [ ] U command (unassemble/disassemble)

**Success Criteria:**
```
> A 1000
1000: MVI A,42
1002: RET
> U 1000
1000: 3E 42    MVI  A,42H
1002: C9       RET
```

---

## Phase 8: Internet Services

**Goal:** HTTP connectivity from 8080

**Tasks:**
- [ ] `GET <url> [> FILE]` mailbox command
- [ ] Response streaming; large bodies go to a storage file
- [ ] N command (HTTP GET)

**Success Criteria:**
```
> N http://example.com/
<!doctype html>...
```

---

## Phase 9: Claude Integration 🎯

**Goal:** The 8080 talks to Claude

**Tasks:**
- [ ] `ASK <prompt>` mailbox command
- [ ] API key management (config file on the coprocessor, not in ROM)
- [ ] System prompt with project context
- [ ] Q command (ask Claude). `A` stays assemble; decided 2026-10-02

**Success Criteria:**
```
> Q What is 6502 vs 8080?
The 6502 and 8080 are both 8-bit processors from 1975...
```

**Vision:** The 8080 doesn't know it's talking to an AI. It sends bytes to a port, gets bytes back. The magic happens in the coprocessor.

---

## Phase 10: Debugger

**Goal:** Advanced debugging features

**Tasks:**
- [ ] Breakpoint system (Rust side)
- [ ] Single-step execution
- [ ] Instruction trace
- [ ] Emulator command parser (prefix TBD; `:` belongs to Intel HEX)
- [ ] bp, step, trace commands
- [ ] R command (register display) - deferred from Phase 3. G pushes a WARM return (decided 2026-10-02), but R still needs register capture

**Success Criteria:**
```
<prefix>bp 1000
Breakpoint set at 1000
> G 100
Break at 1000
<prefix>step
1001: 3E 42    MVI  A,42H
```

---

## Phase 11: Polish & Documentation

**Goal:** Production-ready system

**Tasks:**
- [ ] Help system (? with detailed help)
- [ ] Version command (V)
- [ ] Self-test routine
- [ ] State save/load
- [ ] Documentation
- [ ] Example programs

**Success Criteria:**
- Clean startup
- Comprehensive help
- Example programs run correctly
- Documentation complete

---

## The End State

An 8080 system that:
1. Runs the same ROM on emulator and real hardware
2. Stores data to SD card / cloud
3. Fetches data from the internet
4. Talks to Claude for assistance
5. Debugs itself (with emulator help)

The 8080 code is simple. The coprocessor handles complexity. That's the whole point.
