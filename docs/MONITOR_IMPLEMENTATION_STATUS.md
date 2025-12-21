# Monitor ROM Implementation Status

**Last Updated:** December 20, 2025

## Current Status

### Commands (14 Complete)

| Command | Description | Status |
|---------|-------------|--------|
| C start end dest | Compare memory regions | DONE |
| D [start] [end] | Dump memory | DONE |
| E [addr] | Examine/modify memory | DONE |
| F start end val | Fill memory with value | DONE |
| G [addr] | Go (execute at address) | DONE |
| H num1 num2 | Hex math (add/subtract) | DONE |
| I port | Input from I/O port | DONE |
| L stor mem [cnt] | Load from storage to memory | DONE |
| M src dst cnt | Move memory (forward copy) | DONE |
| O port value | Output to I/O port | DONE |
| S start end pat | Search memory for pattern | DONE |
| W mem stor [cnt] | Write memory to storage | DONE |
| X [file \| -] | Mount/unmount storage | DONE |
| ? | Help | DONE |
| R | Display/modify registers | DEFERRED |

**Note:** 14 commands now. Storage system complete.

### Completed Infrastructure

- [x] Cold start and initialization
- [x] ROM overlay boot mechanism (hardware-compatible)
- [x] Command parser and dispatch
- [x] Console I/O (CONIN, CONOUT, CONST)
- [x] Print routines (string, hex byte, hex word, CRLF)
- [x] Input routines (READ_LINE, READ_HEX_WORD, READ_HEX_ADDR24)
- [x] Self-modifying I/O stubs for I/O commands
- [x] Workspace layout (LINE_BUFFER, LAST_DUMP_ADDR, STOR_ADDR, etc.)
- [x] Automated test infrastructure (TestConsole device)
- [x] 24-bit storage device (16MB linear addressing)
- [x] File mount/unmount service

### Test Coverage

- 181 CPU instruction tests
- 10 monitor integration tests
- All tests passing

---

## Recent Changes

**December 20, 2025 - Phase 4 Storage System Complete:**
- Added Storage device (ports 0x08-0x0C) with 24-bit addressing
- Added StorageMount service (ports 0x0D-0x0F) for file mounting
- Added READ_HEX_ADDR24 helper for parsing 6-digit hex addresses
- New commands: L (load), W (write), X (mount/unmount)
- Storage addresses support full 24-bit range (16MB per file)
- Monitor version bumped to v0.3

**December 16, 2025 - R Command Deferred:**
- Decision: No current use case for register display
- Will implement when actual debugging need arises
- See "Deferred: R Command" section for implementation notes

**December 16, 2025 - ROM Overlay Boot Mechanism:**
- Implemented authentic S-100 style boot sequence
- ROM appears at both 0x0000 and 0xF000 on reset
- OUT to port 0xFE disables overlay, exposing RAM at 0x0000
- Matches real Altair/IMSAI hardware behavior
- Single ROM file, pure address decoding (no dual binaries)

---

## Deferred: R Command (Registers)

**Status:** Deferred - no current use case

**Rationale:** The R command requires a return mechanism from user programs. Currently G does PCHL and never returns. Implementing R properly requires architectural changes that aren't needed yet.

**When to implement:** When actively debugging a program and wishing you could see registers.

---

## Architecture Notes

### Storage Device Protocol

```
Port 0x08: Address low byte (R/W)
Port 0x09: Address mid byte (R/W)
Port 0x0A: Address high byte (R/W)
Port 0x0B: Data with auto-increment (R/W)
Port 0x0C: Status (R) / Control (W)

Status bits:
  Bit 0: Mounted
  Bit 1: Ready (always 1)
  Bit 7: EOF

Control commands:
  0x00: Reset address to 0
  0x01: Decrement address
  0x02: Flush write buffer
```

### Storage Mount Protocol

```
Port 0x0D: Filename character (W)
Port 0x0E: Control command (W)
Port 0x0F: Status (R)

Control commands:
  0x01: Mount file
  0x02: Unmount
  0x03: Query status

Status codes:
  0x00: OK / Mounted
  0x01: Not found
  0x02: Invalid filename
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
00E7-00E9: STOR_ADDR (3 bytes - 24-bit storage address)
00EA-00FF: Available (22 bytes)
```

### Port Assignments

```
00-02: Console (data out, data in, status)
08-0C: Storage device (24-bit address, data, control)
0D-0F: Storage mount service
FE:    System control (ROM overlay)
FF:    System status
```

---

## Future Phases

- **Phase 5:** Intel HEX loader (H command)
- **Phase 6:** Timer device, interrupts
- **Phase 7:** Assembler/disassembler devices
- **Phase 8:** HTTP client, network time
- **Phase 9:** Claude API integration
- **Phase 10:** Debugger (breakpoints, single-step, R command)
