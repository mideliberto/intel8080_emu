# Implementation Roadmap

## Phase Summary

| Phase | Focus | Status |
|-------|-------|--------|
| 1 | Core Monitor | ✅ Complete |
| 2 | Memory Operations | ✅ Complete |
| 3 | Execution & I/O | ✅ Complete |
| **4** | **Storage System** | **🔲 Current** |
| 5 | Program Loading | 🔲 Future |
| 6 | Timing System | 🔲 Future |
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

## Phase 4: Storage System 🔲 CURRENT

**Goal:** Linear-addressed storage operations

**Tasks:**

Rust:
- [ ] Storage device (ports 0x08-0x0B)
- [ ] StorageMount device (ports 0x0C-0x0F)
- [ ] Unit tests

ROM:
- [ ] X command (mount file)
- [ ] L command (load from storage to memory)
- [ ] W command (write memory to storage)

**Success Criteria:**
```
> X TEST.BIN
Mounted: 04 pages
> L 0 1000
Loaded
> D 1000
1000: 00 01 02 03...
> E 1000 FF
> W 1000 0
Written
```

**Details:** See PHASE4_STORAGE_PLAN.md

---

## Phase 5: Program Loading

**Goal:** Load programs into memory

**Tasks:**
- [ ] Intel HEX loader (H command)
- [ ] Checksum validation
- [ ] Type 00 (data) and Type 01 (EOF) records

**Success Criteria:**
```
> H
:10010000...
:00000001FF
Loaded 256 bytes at 0100
```

---

## Phase 6: Timing System

**Goal:** Real-time clock and interrupts

**Tasks:**
- [ ] TimeDevice (Rust) - ports 0x60-0x6F
- [ ] Timer8253 (Rust) - ports 0x70-0x73
- [ ] TIMER_ISR in ROM
- [ ] T command (show time)
- [ ] TI command (init timer)
- [ ] TS command (timer status)

**Success Criteria:**
- Interrupts fire at configured rate
- Software clock updates
- T command shows current time

---

## Phase 7: Development Tools

**Goal:** Assembly and disassembly via I/O devices

**Tasks:**
- [ ] DisassemblerDevice (Rust) - ports 0x20-0x27
- [ ] AssemblerDevice (Rust) - ports 0x28-0x2F
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
- [ ] HTTPDevice (Rust) - ports 0x40-0x4F
- [ ] HTTP GET support
- [ ] Response streaming (chunked for 8080's memory)
- [ ] N G command (HTTP GET)
- [ ] N T command (get network time)

**Success Criteria:**
```
> N T
2025-12-19 14:32:07
> N G http://example.com/
<!doctype html>...
```

---

## Phase 9: Claude Integration 🎯

**Goal:** The 8080 talks to Claude

**Tasks:**
- [ ] ClaudeDevice (Rust) - ports 0x38-0x3F
- [ ] API key management (config file, not in ROM)
- [ ] System prompt with project context
- [ ] Request/response buffering
- [ ] A command (ask Claude)

**Device Protocol:**
```
0x38: Prompt char (write)
0x39: Command (write): 01=send, 02=clear
0x3A: Status (read): 00=idle, 01=waiting, 02=ready, 80+=error
0x3B: Response char (read)
```

**Success Criteria:**
```
> A What is 6502 vs 8080?
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
- [ ] Emulator command parser (`:` prefix)
- [ ] :bp, :step, :trace commands
- [ ] R command (register display) - deferred from Phase 3

**Success Criteria:**
```
:bp 1000
Breakpoint set at 1000
> G 100
Break at 1000
:step
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
