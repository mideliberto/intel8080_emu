# Monitor ROM Implementation Status

**Last Updated:** December 16, 2025

## Current Status

### Commands (11 Complete)

| Command | Description | Status |
|---------|-------------|--------|
| C start end dest | Compare memory regions | DONE |
| D [start] [end] | Dump memory | DONE |
| E [addr] | Examine/modify memory | DONE |
| F start end val | Fill memory with value | DONE |
| G [addr] | Go (execute at address) | DONE |
| H num1 num2 | Hex math (add/subtract) | DONE |
| I port | Input from I/O port | DONE |
| M src dst cnt | Move memory (forward copy) | DONE |
| O port value | Output to I/O port | DONE |
| S start end pat | Search memory for pattern | DONE |
| ? | Help | DONE |
| R | Display/modify registers | DEFERRED |

**Note:** 11 commands exceeds DDT's original 10-command set. Monitor is feature-complete for basic use.

### Completed Infrastructure

- [x] Cold start and initialization
- [x] ROM overlay boot mechanism (hardware-compatible)
- [x] Command parser and dispatch
- [x] Console I/O (CONIN, CONOUT, CONST)
- [x] Print routines (string, hex byte, hex word, CRLF)
- [x] Input routines (READ_LINE, READ_HEX_WORD)
- [x] Self-modifying I/O stubs for I/O commands
- [x] Workspace layout (LINE_BUFFER, LAST_DUMP_ADDR, etc.)
- [x] Automated test infrastructure (TestConsole device)

### Test Coverage

- 181 CPU instruction tests
- 10 monitor integration tests
- All tests passing

---

## Recent Changes

**December 16, 2025 - R Command Deferred:**
- Decision: No current use case for register display
- Monitor already exceeds DDT feature parity (11 vs 10 commands)
- Will implement when actual debugging need arises
- See "Deferred: R Command" section for implementation notes

**December 16, 2025 - ROM Overlay Boot Mechanism:**
- Implemented authentic S-100 style boot sequence
- ROM appears at both 0x0000 and 0xF000 on reset
- OUT to port 0xFE disables overlay, exposing RAM at 0x0000
- Matches real Altair/IMSAI hardware behavior
- Single ROM file, pure address decoding (no dual binaries)

**December 16, 2025 - Automated Testing:**
- Created TestConsole device for scripted monitor testing
- 10 integration tests covering all commands
- Tests verify boot, commands, and overlay behavior

**December 2025 - Added S (Search) command:**
- Searches memory for byte patterns (1-8 bytes)
- Syntax: S start end b1 [b2 ... b8]
- Pattern stored in workspace at SEARCH_PATTERN (8 bytes)

**December 2025 - Added F, M, C commands:**
- F (Fill): Fills memory range with byte value
- M (Move): Forward-only block copy
- C (Compare): Shows differences as addr1:val1 addr2:val2

**December 2025 - DUMP command edge cases:**
- Fixed infinite loop when dumping to FFFF
- Fixed overflow handling for high memory addresses

---

## Deferred: R Command (Registers)

**Status:** Deferred - no current use case

**Rationale:** The R command requires a return mechanism from user programs. Currently G does PCHL and never returns. Implementing R properly requires architectural changes that aren't needed yet.

**When to implement:** When actively debugging a program and wishing you could see registers.

**Implementation requirements (for future reference):**

1. **Register save area** (12 bytes in workspace):
   - A, F, BC, DE, HL, SP, PC
   - Location: 00E7-00F2 (available workspace)

2. **Return mechanism** - one of:
   - RST 7 handler saves registers and returns to monitor
   - CALL to fixed address (e.g., 0x0038)
   - Breakpoint via RST with saved PC

3. **G command changes:**
   - Restore registers from save area before PCHL
   - Or: save current SP, set user SP, transfer control

4. **R command itself:**
   - Display: A=xx F=xx BC=xxxx DE=xxxx HL=xxxx SP=xxxx PC=xxxx
   - Optional: modify registers interactively

**Estimated effort:** 4 components, ~100-150 lines of assembly

---

## Architecture Notes

### ROM Overlay Boot Sequence

```
1. Reset: PC=0x0000, overlay enabled
2. CPU reads from 0x0000 -> gets ROM (mirrored from 0xF000)
3. ROM executes: LXI SP / DI / JMP BOOT_CONTINUE
4. PC now at 0xF00B (in ROM address space)
5. OUT 0xFE, 0x00 -> overlay disabled
6. 0x0000-0x0FFF now RAM
7. Normal initialization continues
```

### I/O Stub Mechanism

I and O commands use self-modifying code because 8080's IN/OUT require literal port numbers.

```
IO_IN_STUB   EQU 00D6H   ; 3 bytes: IN xx / RET
IO_OUT_STUB  EQU 00D9H   ; 3 bytes: OUT xx / RET
```

### Workspace Memory Map

```
0080-00CF: LINE_BUFFER (80 bytes)
00D0-00D1: BUFFER_PTR
00D2-00D3: LAST_DUMP_ADDR
00D4-00D5: LAST_EXAM_ADDR
00D6-00D8: IO_IN_STUB
00D9-00DB: IO_OUT_STUB
00DC-00E3: SEARCH_PATTERN (8 bytes)
00E4:      SEARCH_LENGTH (1 byte)
00E5-00E6: SEARCH_END (2 bytes)
00E7-00FF: Available (25 bytes) - reserved for future register save area
```

### Port Assignments

```
00: Console data out
01: Console data in
02: Console status
FE: System control (ROM overlay)
FF: System status
```

---

## Future Phases

- **Disk System:** X (mount), L (list), B (boot)
- **Program Loading:** Intel HEX loader
- **Timing:** Timer device, T command
- **Development Tools:** Assembler/disassembler devices
- **Internet Services:** HTTP, time sync
- **Debugger:** Breakpoints, single-step, trace (includes R command)
