# 8080 Monitor ROM & Emulator - Complete Project Specification & Implementation Plan

**Version 1.2 | December 2025**

**Changelog:**
- v1.1: Added ROM overlay boot mechanism for hardware compatibility

-----

## EXECUTIVE SUMMARY

This document contains the complete specification and implementation plan for an Intel 8080 emulator with a monitor ROM system. The project combines period-appropriate architecture (1975 vintage) with modern conveniences through an innovative I/O device model.

**Key Features:**

- Full 8080 CPU emulation with cycle-accurate timing
- 4KB Monitor ROM with complete command set
- ROM overlay boot mechanism for hardware compatibility
- Extensible I/O device architecture
- Period-appropriate disk controller (IBM 3740 format)
- Modern features: HTTP client, assembler/disassembler, real-time clock
- Interrupt-driven timing with 8253-compatible timer
- Complete debugging infrastructure

**Current Status:**

- âœ… CPU core (all 256 opcodes, flags, stack, I/O, interrupts)
- âœ… Memory subsystem
- âœ… Basic console device
- âœ… Rust project structure
- âœ… Monitor ROM (11 commands implemented)
- ðŸ”² ROM overlay mechanism (to be implemented)
- ðŸ”² Advanced devices (to be implemented)
- ðŸ”² Debugger (to be implemented)

-----

## TABLE OF CONTENTS

1. [Design Decisions](#design-decisions)
2. [Memory Architecture](#memory-architecture)
3. [Boot Sequence](#boot-sequence)
4. [I/O Port Map](#io-port-map)
5. [Device Protocols](#device-protocols)
6. [Rust Project Structure](#rust-project-structure)
7. [Assembly ROM Structure](#assembly-rom-structure)
8. [Implementation Plan](#implementation-plan)
9. [Code Templates](#code-templates)
10. [Configuration](#configuration)
11. [Testing Strategy](#testing-strategy)

-----

## DESIGN DECISIONS

### 1. Memory Layout

**Decision:** High ROM (0xF000-0xFFFF) with CP/M-style page zero reservation

```
0x0000-0x003F: RST Vector Table (64 bytes, RAM, copied from ROM)
0x0040-0x007F: BIOS Jump Table (64 bytes, RAM, copied from ROM)
0x0080-0x00FF: System Workspace (128 bytes, RAM)
0x0100-0xEFFF: User Program Area (59,904 bytes)
0xF000-0xFFFF: Monitor ROM (4,096 bytes)
```

**Rationale:** CP/M compatible, clean 0x0100 start address, standard practice

### 2. Boot Mechanism

**Decision:** ROM overlay with bank switching

**Problem:** The 8080 starts execution at address 0x0000 on reset, but our ROM lives at 0xF000. We need vectors in low RAM for interrupts to work, but RAM contents are undefined at power-on.

**Solution:** ROM overlay mechanism

- On reset, ROM appears at BOTH 0x0000-0x0FFF AND 0xF000-0xFFFF
- CPU starts at 0x0000, executing ROM code (which is actually at F000)
- ROM code disables the overlay via OUT to port 0xFE
- Low memory (0x0000-0x0FFF) becomes RAM
- ROM copies vectors from high ROM to low RAM
- System continues with vectors in place

**Hardware Implementation:**
- Flip-flop (74LS74) controls overlay state
- Set on reset (overlay enabled)
- Cleared by write to port 0xFE with value 0x00
- Address decode logic checks flip-flop for 0x0000-0x0FFF access

**Rationale:** This is how real S-100 systems (Altair, IMSAI) handled the boot problem. Maintains CP/M compatibility with full RAM at 0x0000 after boot.

### 3. CPU Speed

**Decision:** Configurable (default 2.0 MHz)

**Configuration:**

```json
{
  "cpu": {
    "speed_mhz": 2.0,
    "cycle_accurate": true
  }
}
```

### 4. Disk Format

**Decision:** Raw binary sectors, IBM 3740 format

```
Sectors per track: 26
Bytes per sector: 128
Tracks: 77
Capacity: 256,256 bytes (~250KB)
Format: Single-density 8" floppy standard
```

### 5. API Entry Points

**Decision:** Hybrid RST + CALL approach

**RST Vectors (1-byte calls):**

- RST 1 (0x0008): CONOUT - Console output
- RST 2 (0x0010): CONIN - Console input
- RST 3 (0x0018): CONST - Console status
- RST 7 (0x0038): Timer interrupt vector

**BIOS Jump Table (0x0040-0x007F):**

- 0x0040: CONOUT
- 0x0043: CONIN
- 0x0046: CONST
- 0x0049: PRINT_STRING
- 0x004C: PRINT_HEX_BYTE
- 0x004F: PRINT_HEX_WORD
- 0x0052: READ_HEX_WORD
- 0x0055: SKIP_SPACES
- 0x0058: DISK_READ
- 0x005B: DISK_WRITE

### 6. Register Preservation

**Decision:** Hybrid model - document what each function trashes

**Rules:**

- Flags: Always trashed
- A: Preserved if input-only, trashed if return value
- HL: Preserved unless itâ€™s a 16-bit return value
- BC, DE: Preserved unless documented as parameters
- **Every function must document its register usage**

### 7. Monitor Commands

**Memory Operations:**

- D [start] [end] - Dump memory
- E [address] - Examine/modify
- F [start] [end] [byte] - Fill
- M [source] [dest] [count] - Move
- S [start] [end] [bytes] - Search
- C [start] [end] [dest] - Compare
- H [num1] [num2] - Hex arithmetic

**Execution:**

- G [address] - Go/execute
- C [address] - Call with return
- R - Registers (DEFERRED - requires return mechanism, implement when needed)

**I/O:**

- I [port] - Input
- O [port] [value] - Output

**Disk:**

- X [drive] [filename] - Mount
- L [drive] - List
- B [address] - Boot

**Development:**

- H - Load Intel HEX

**Network:**

- N W [url] - HTTP GET
- N T - Get time

**Utility:**

- ? - Help
- V - Version
- T - Show time
- TI [freq] - Init timer
- TS - Timer status
- Z - Cold start
- Q - Quit

### 8. Command Parser

**Decision:** Jump table dispatch with shared parsing helpers

**Helpers:**

- SKIP_SPACES
- READ_HEX_WORD
- IS_HEX_DIGIT
- HEX_TO_BINARY
- PRINT_HEX_BYTE/WORD

### 9. User Interface

**Decision:**

- Prompt: Simple â€œ>â€
- Error messages: Descriptive but concise (â€œInvalid addressâ€, not â€œ?â€)
- Startup: Banner + self-test + optional auto-boot
- Console: Echo enabled, basic line editing (BS, Ctrl+U, Ctrl+C)

### 10. File Formats

**Decision:**

- Configuration: JSON (modern choice for vintage system)
- Intel HEX: Validate checksums, basic format only (Type 00, 01)
- Disassembly: Period-appropriate (uppercase, â€œHâ€ suffix, proper spacing)

### 11. Emulator vs ROM

**Decision:** Clear separation with â€œ:â€ prefix

**ROM Commands:** D, E, F, M, S, C, H, G, C, R, I, O, X, L, B, N, T, ?, V, Z, Q
**Emulator Commands:** :bp, :step, :trace, :load, :save, :mount, :state, :speed, :reset, :quit

### 12. Device Architecture

**Decision:** Port-based I/O device model

All complex operations (assembly, disassembly, networking) exposed as I/O devices. ROM code sends/receives bytes through ports; Rust emulator handles complexity.

-----

## MEMORY ARCHITECTURE

### Complete Memory Map

```
â”Œâ”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”
â”‚ 0x0000-0x0007  RST 0 Vector                                 â”‚
â”‚                JMP COLD_START (copied from ROM)             â”‚
â”œâ”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”¤
â”‚ 0x0008-0x000F  RST 1 Vector                                 â”‚
â”‚                JMP CONOUT_IMPL (copied from ROM)            â”‚
â”œâ”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”¤
â”‚ 0x0010-0x0017  RST 2 Vector                                 â”‚
â”‚                JMP CONIN_IMPL (copied from ROM)             â”‚
â”œâ”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”¤
â”‚ 0x0018-0x001F  RST 3 Vector                                 â”‚
â”‚                JMP CONST_IMPL (copied from ROM)             â”‚
â”œâ”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”¤
â”‚ 0x0020-0x0027  RST 4 Vector (Reserved)                      â”‚
â”œâ”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”¤
â”‚ 0x0028-0x002F  RST 5 Vector (Reserved/BDOS)                 â”‚
â”œâ”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”¤
â”‚ 0x0030-0x0037  RST 6 Vector (Reserved)                      â”‚
â”œâ”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”¤
â”‚ 0x0038-0x003F  RST 7 Vector                                 â”‚
â”‚                JMP TIMER_ISR (copied from ROM)              â”‚
â”œâ”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”¤
â”‚ 0x0040-0x0042  BIOS: CONOUT (JMP)                           â”‚
â”œâ”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”¤
â”‚ 0x0043-0x0045  BIOS: CONIN (JMP)                            â”‚
â”œâ”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”¤
â”‚ 0x0046-0x0048  BIOS: CONST (JMP)                            â”‚
â”œâ”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”¤
â”‚ 0x0049-0x004B  BIOS: PRINT_STRING (JMP)                     â”‚
â”œâ”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”¤
â”‚ 0x004C-0x004E  BIOS: PRINT_HEX_BYTE (JMP)                   â”‚
â”œâ”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”¤
â”‚ 0x004F-0x0051  BIOS: PRINT_HEX_WORD (JMP)                   â”‚
â”œâ”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”¤
â”‚ 0x0052-0x0054  BIOS: READ_HEX_WORD (JMP)                    â”‚
â”œâ”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”¤
â”‚ 0x0055-0x0057  BIOS: SKIP_SPACES (JMP)                      â”‚
â”œâ”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”¤
â”‚ 0x0058-0x005A  BIOS: DISK_READ (JMP)                        â”‚
â”œâ”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”¤
â”‚ 0x005B-0x005D  BIOS: DISK_WRITE (JMP)                       â”‚
â”œâ”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”¤
â”‚ 0x005E-0x007F  BIOS: Additional entries / padding           â”‚
â”œâ”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”¤
â”‚ 0x0080-0x00CF  LINE_BUFFER (80 bytes)                       â”‚
â”‚                Command line input buffer                    â”‚
â”œâ”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”¤
â”‚ 0x00D0-0x00D1  TICK_COUNT (16-bit)                          â”‚
â”‚                Timer tick counter                           â”‚
â”œâ”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”¤
â”‚ 0x00D2         SECONDS (8-bit)                              â”‚
â”‚                Software clock seconds                       â”‚
â”œâ”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”¤
â”‚ 0x00D3         MINUTES (8-bit)                              â”‚
â”‚                Software clock minutes                       â”‚
â”œâ”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”¤
â”‚ 0x00D4         HOURS (8-bit)                                â”‚
â”‚                Software clock hours                         â”‚
â”œâ”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”¤
â”‚ 0x00D5-0x00D6  LAST_DUMP_ADDR (16-bit)                      â”‚
â”‚                Last address dumped (for D command)          â”‚
â”œâ”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”¤
â”‚ 0x00D7         CURRENT_DRIVE (8-bit)                        â”‚
â”‚                Currently selected drive (0-3)               â”‚
â”œâ”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”¤
â”‚ 0x00D8         SYSTEM_FLAGS (8-bit)                         â”‚
â”‚                System status flags                          â”‚
â”œâ”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”¤
â”‚ 0x00D9-0x00FF  Available workspace                          â”‚
â”‚                Additional system variables as needed        â”‚
â”œâ”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”¤
â”‚ 0x0100-0xEFFF  USER PROGRAM AREA (59,904 bytes)             â”‚
â”‚                Transient Program Area (TPA)                 â”‚
â”‚                Available for user programs                  â”‚
â”‚                CP/M compatible                              â”‚
â”œâ”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”¤
â”‚ 0xF000-0xFFFF  MONITOR ROM (4,096 bytes)                    â”‚
â”‚                Monitor code, BIOS functions                 â”‚
â”‚                Command handlers, ISRs                       â”‚
â”‚                String constants                             â”‚
â”‚                Vector/BIOS table source                     â”‚
â””â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”˜
```

### ROM Overlay Mechanism

On reset, the ROM is mapped to two address ranges simultaneously:

```
â”Œâ”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”
â”‚                    RESET STATE (overlay enabled)            â”‚
â”œâ”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”¤
â”‚ 0x0000-0x0FFF  ROM (mirror of F000-FFFF, partial)          â”‚
â”‚ 0x1000-0xEFFF  RAM                                          â”‚
â”‚ 0xF000-0xFFFF  ROM (primary location)                       â”‚
â””â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”˜

â”Œâ”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”
â”‚                    RUN STATE (overlay disabled)             â”‚
â”œâ”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”¤
â”‚ 0x0000-0xEFFF  RAM                                          â”‚
â”‚ 0xF000-0xFFFF  ROM                                          â”‚
â””â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”˜
```

**State Transitions:**
- RESET â†’ overlay enabled (ROM at 0000 and F000)
- OUT 0FEh, 00h â†’ overlay disabled (RAM at 0000)
- OUT 0FEh, FFh â†’ cold reset (overlay re-enabled)

### ROM Organization (0xF000-0xFFFF)

```
F000: COLD_START
      - Disable ROM overlay (OUT 0FEh, 00h)
      - Initialize stack (SP = 0xF000)
      - Copy vectors/BIOS table to low RAM
      - Clear workspace
      - Initialize devices
      - Display banner
      - Run self-test
      - Check auto-boot
      - Enter monitor loop

F0XX: MONITOR_LOOP
      - Print prompt
      - Read command line
      - Parse command
      - Dispatch to handler
      - Loop

F1XX: COMMAND HANDLERS
      - CMD_DUMP
      - CMD_EXAMINE
      - CMD_FILL
      - CMD_MOVE
      - CMD_SEARCH
      - CMD_COMPARE
      - CMD_HEX_MATH
      - CMD_GO
      - CMD_CALL
      - CMD_REGISTERS
      - CMD_INPUT
      - CMD_OUTPUT
      - CMD_DISK_MOUNT
      - CMD_DISK_LIST
      - CMD_DISK_BOOT
      - CMD_HEX_LOAD
      - CMD_TIME
      - CMD_TIMER_INIT
      - CMD_TIMER_STATUS
      - CMD_NETWORK
      - CMD_HELP
      - CMD_VERSION

F4XX: BIOS FUNCTIONS
      - CONOUT_IMPL
      - CONIN_IMPL
      - CONST_IMPL
      - PRINT_STRING
      - PRINT_HEX_BYTE
      - PRINT_HEX_WORD
      - PRINT_CRLF
      - READ_LINE
      - READ_HEX_WORD
      - SKIP_SPACES
      - IS_HEX_DIGIT
      - HEX_TO_BINARY
      - BLOCK_MOVE
      - BLOCK_FILL

F6XX: DISK FUNCTIONS
      - DISK_READ
      - DISK_WRITE
      - DISK_SELECT

F7XX: INTERRUPT SERVICE ROUTINES
      - TIMER_ISR
      - RESERVED_RST

F8XX: HELPER FUNCTIONS
      - Command parsing
      - Error handling
      - Utility functions

FAXX: VECTOR TABLE SOURCE
      - RST vector table (64 bytes)
      - BIOS jump table (64 bytes)

FCXX: STRING CONSTANTS
      - Banner text
      - Help text
      - Error messages
      - Command prompts

FFFF: End of ROM
```

-----

## BOOT SEQUENCE

### Power-On / Reset Sequence

```
1. Hardware Reset
   â”œâ”€â”€ CPU PC = 0x0000
   â”œâ”€â”€ ROM overlay flip-flop SET (overlay enabled)
   â””â”€â”€ ROM appears at 0x0000-0x0FFF AND 0xF000-0xFFFF

2. First Instructions (executing from 0x0000, reading ROM)
   â”œâ”€â”€ LXI SP, 0F000h      ; Set up stack below ROM
   â”œâ”€â”€ DI                   ; Disable interrupts
   â””â”€â”€ XRA A / OUT 0FEh    ; Disable overlay â†’ RAM at 0x0000

3. Vector Initialization
   â”œâ”€â”€ Copy RST vectors (0x0000-0x003F) from ROM source
   â”œâ”€â”€ Copy BIOS table (0x0040-0x007F) from ROM source
   â””â”€â”€ Low RAM now contains jump instructions

4. Workspace Initialization
   â”œâ”€â”€ Clear workspace variables
   â”œâ”€â”€ Initialize I/O stubs
   â””â”€â”€ Set default values

5. Device Initialization
   â”œâ”€â”€ Initialize console
   â”œâ”€â”€ Initialize timer (if enabled)
   â””â”€â”€ Initialize other devices

6. User Interface
   â”œâ”€â”€ Display banner
   â”œâ”€â”€ Optional: Run self-test
   â”œâ”€â”€ Optional: Check auto-boot flag
   â””â”€â”€ Enter monitor loop
```

### Cold Start Assembly Code

```asm
COLD_START:
        ; CRITICAL: First instructions run from overlay
        ; ROM at 0x0000 mirrors ROM at 0xF000
        
        LXI     SP,STACK_TOP    ; Stack below ROM
        DI                      ; No interrupts yet!
        
        ; Disable ROM overlay - expose RAM at 0x0000
        XRA     A               ; A = 0x00
        OUT     0FEH            ; SYSTEM_CONTROL: disable overlay
        
        ; Now 0x0000-0x0FFF is RAM (uninitialized)
        ; We're still executing from 0xF000 region
        
        ; Copy vectors from ROM to RAM
        LXI     H,VECTOR_SOURCE ; Source in ROM
        LXI     D,0000H         ; Destination in RAM
        LXI     B,0080H         ; 128 bytes (vectors + BIOS table)
        CALL    BLOCK_COPY
        
        ; Continue with normal initialization...
        CALL    INIT_WORKSPACE
        CALL    INIT_IO_STUBS
        CALL    PRINT_BANNER
        
        EI                      ; Safe to enable interrupts now
        JMP     MONITOR_LOOP

VECTOR_SOURCE:
        ; RST 0 - Cold start
        JMP     COLD_START
        DS      5               ; Pad to 8 bytes
        
        ; RST 1 - Console output  
        JMP     CONOUT_IMPL
        DS      5
        
        ; ... etc for all vectors
```

-----

## I/O PORT MAP

### Complete Port Allocation

```
â”Œâ”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”¬â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”
â”‚  Ports   â”‚  Device                                             â”‚
â”œâ”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”¼â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”¤
â”‚ 00-03    â”‚ Console Serial Interface                            â”‚
â”‚ 04-07    â”‚ (Reserved for Parallel I/O)                         â”‚
â”‚ 08-0F    â”‚ Disk Controller                                     â”‚
â”‚ 10-13    â”‚ Disk Mount Service                                  â”‚
â”‚ 14-1F    â”‚ (Reserved)                                          â”‚
â”‚ 20-27    â”‚ Disassembler Service                                â”‚
â”‚ 28-2F    â”‚ Assembler Service                                   â”‚
â”‚ 30-37    â”‚ (Reserved for Debugger Services)                    â”‚
â”‚ 38-3F    â”‚ (Reserved)                                          â”‚
â”‚ 40-5F    â”‚ Internet Services (HTTP, DNS, Time)                 â”‚
â”‚ 60-6F    â”‚ System Time (Read-only)                             â”‚
â”‚ 70-73    â”‚ Programmable Timer (8253-compatible)                â”‚
â”‚ 74-EF    â”‚ (Available for expansion)                           â”‚
â”‚ F0-FD    â”‚ (Reserved)                                          â”‚
â”‚ FE-FF    â”‚ System Control/Status                               â”‚
â””â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”´â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”˜
```

### Detailed Port Specifications

#### Console (0x00-0x03)

```
0x00: CONSOLE_DATA_OUT    [Write] Send character
0x01: CONSOLE_DATA_IN     [Read]  Receive character
0x02: CONSOLE_STATUS      [Read]  Bit 0: RX ready, Bit 1: TX ready
0x03: CONSOLE_CONTROL     [R/W]   (Reserved)
```

#### Disk Controller (0x08-0x0F)

```
0x08: DISK_COMMAND        [Write] 01h=Read, 02h=Write, 03h=Status
0x09: DISK_STATUS         [Read]  00h=Ready, error codes
0x0A: DISK_TRACK          [R/W]   Track number (0-76)
0x0B: DISK_SECTOR         [R/W]   Sector number (0-25)
0x0C: DISK_DMA_LO         [R/W]   DMA address low byte
0x0D: DISK_DMA_HI         [R/W]   DMA address high byte
0x0E: DISK_DRIVE_SELECT   [R/W]   Drive select (0-3 = A-D)
0x0F: (Reserved)
```

#### Disk Mount Service (0x10-0x13)

```
0x10: DISK_MOUNT_NAME     [Write] Filename character (null-terminated)
0x11: DISK_MOUNT_CONTROL  [Write] 01h=Mount, 02h=Unmount, 03h=Query
0x12: DISK_MOUNT_STATUS   [Read]  00h=OK, 01h=Not Found, 02h=Invalid, FFh=In Progress
0x13: DISK_MOUNT_DRIVE    [R/W]   Drive to operate on (0-3)
```

#### Disassembler (0x20-0x27)

```
0x20: DASM_OPCODE_IN      [Write] Send opcode bytes
0x21: DASM_CONTROL        [Write] 01h=Disassemble, 02h=Reset
0x22: DASM_STATUS         [Read]  Bits 0-6: text length, Bit 7: error
0x23: DASM_TEXT_OUT       [Read]  ASCII text output
0x24-0x27: (Reserved)
```

#### Assembler (0x28-0x2F)

```
0x28: ASM_TEXT_IN         [Write] Assembly text (null-terminated)
0x29: ASM_CONTROL         [Write] 01h=Assemble, 02h=Reset
0x2A: ASM_STATUS          [Read]  Bits 0-3: bytes assembled, Bits 4-7: error
0x2B: ASM_OPCODE_OUT      [Read]  Assembled opcode bytes
0x2C: ASM_ERROR_POS       [Read]  Position of parse error
0x2D-0x2F: (Reserved)
```

#### Internet Services (0x40-0x5F)

**HTTP Client (0x40-0x47):**

```
0x40: HTTP_URL_BYTE       [Write] URL characters (null-terminated)
0x41: HTTP_COMMAND        [Write] 01h=GET, 02h=POST
0x42: HTTP_STATUS         [Read]  HTTP status code (200, 404, etc.)
0x43: HTTP_HEADER_IN      [Read]  Response headers
0x44: HTTP_BODY_IN        [Read]  Response body
0x45: HTTP_BODY_OUT       [Write] POST data
0x46-0x47: (Reserved)
```

**Time Service (0x48-0x4F):**

```
0x48: TIME_COMMAND        [Write] 01h=Get current time
0x49: TIME_YEAR           [Read]  Year (since 1900)
0x4A: TIME_MONTH          [Read]  Month (1-12)
0x4B: TIME_DAY            [Read]  Day (1-31)
0x4C: TIME_HOUR           [Read]  Hour (0-23)
0x4D: TIME_MINUTE         [Read]  Minute (0-59)
0x4E: TIME_SECOND         [Read]  Second (0-59)
0x4F: (Reserved)
```

**DNS Service (0x50-0x57):**

```
0x50: DNS_HOSTNAME_IN     [Write] Hostname bytes (null-terminated)
0x51: DNS_COMMAND         [Write] 01h=Resolve
0x52: DNS_IP_0            [Read]  IP address byte 0
0x53: DNS_IP_1            [Read]  IP address byte 1
0x54: DNS_IP_2            [Read]  IP address byte 2
0x55: DNS_IP_3            [Read]  IP address byte 3
0x56-0x57: (Reserved)
```

#### System Time - Read Only (0x60-0x6F)

**Current Time (0x60-0x67):**

```
0x60: TIME_SECOND         [Read]  Current second (0-59)
0x61: TIME_MINUTE         [Read]  Current minute (0-59)
0x62: TIME_HOUR           [Read]  Current hour (0-23)
0x63: TIME_DAY            [Read]  Day of month (1-31)
0x64: TIME_MONTH          [Read]  Month (1-12)
0x65: TIME_YEAR           [Read]  Years since 1900
0x66: TIME_DOW            [Read]  Day of week (0=Sun, 6=Sat)
0x67: (Reserved)
```

**Uptime Counter (0x68-0x6B):**

```
0x68: UPTIME_SEC          [Read]  Seconds since start
0x69: UPTIME_MIN          [Read]  Minutes since start
0x6A: UPTIME_HOUR         [Read]  Hours since start
0x6B: UPTIME_DAYS         [Read]  Days since start
```

**Millisecond Timer (0x6C-0x6F):**

```
0x6C: TIMER_MS_LO         [Read]  Millisecond timer low byte
0x6D: TIMER_MS_HI         [Read]  Millisecond timer high byte
0x6E: TIMER_CONTROL       [Write] Reset timer (any value)
0x6F: (Reserved)
```

#### Programmable Timer (0x70-0x73)

```
0x70: TIMER_COUNTER_0     [R/W]   Counter 0 (LSB/MSB)
0x71: TIMER_COUNTER_1     [R/W]   Counter 1 (LSB/MSB)
0x72: TIMER_COUNTER_2     [R/W]   Counter 2 (LSB/MSB)
0x73: TIMER_CONTROL       [Write] Control register (8253 format)

Control Register Format:
  Bits 7-6: Counter select (00=0, 01=1, 10=2, 11=read-back)
  Bits 5-4: R/W mode (00=latch, 01=LSB, 10=MSB, 11=LSB then MSB)
  Bits 3-1: Mode (010=rate generator - only mode implemented)
  Bit 0:    BCD (0=binary - only mode supported)
```

#### System Control (0xFE-0xFF)

```
0xFE: SYSTEM_CONTROL      [Write] System control commands
      00h = Disable ROM overlay (expose RAM at 0x0000-0x0FFF)
      01h = Halt CPU
      FFh = Cold reset (re-enable overlay, restart at 0x0000)

0xFF: SYSTEM_STATUS       [Read]  System status / sense switches
      Bit 0: ROM overlay state (1=enabled, 0=disabled)
      Bits 1-7: Reserved / sense switch inputs
```

-----

## DEVICE PROTOCOLS

### System Control Protocol

**Purpose:** Boot control and system management

**Port 0xFE Commands:**

| Value | Function | Description |
|-------|----------|-------------|
| 0x00  | OVERLAY_OFF | Disable ROM overlay, expose RAM at 0x0000-0x0FFF |
| 0x01  | HALT | Halt CPU execution |
| 0xFF  | COLD_RESET | Re-enable overlay, jump to 0x0000 |

**Boot Sequence Example:**

```asm
COLD_START:
        LXI     SP,0F000H       ; Stack below ROM
        DI                      ; Interrupts off
        
        ; Disable overlay - RAM now at 0x0000
        XRA     A               ; A = 0x00
        OUT     0FEH            ; SYSTEM_CONTROL
        
        ; Copy vectors to RAM...
        ; (rest of initialization)
```

**Reading Overlay State:**

```asm
        IN      0FFH            ; SYSTEM_STATUS
        ANI     01H             ; Mask overlay bit
        JZ      OVERLAY_OFF     ; Branch if disabled
```

### Disk Mounter Protocol

**Purpose:** Change mounted disk images at runtime

**Sequence:**

1. Select drive (write to port 0x13)
1. Send filename bytes (write to port 0x10), null-terminated
1. Send mount command (write 0x01 to port 0x11)
1. Poll status (read from port 0x12) until not 0xFF
1. Check result: 0x00 = success, other = error

**Example:**

```asm
; Mount "GAMES.IMG" on drive A
    XRA A               ; Drive A (0)
    OUT 13h             ; Select drive

    LXI H, FILENAME
MOUNT_LOOP:
    MOV A, M
    ORA A
    JZ MOUNT_CMD
    OUT 10h             ; Send character
    INX H
    JMP MOUNT_LOOP

MOUNT_CMD:
    MVI A, 01h          ; Mount command
    OUT 11h

MOUNT_WAIT:
    IN 12h              ; Check status
    CPI 0FFh
    JZ MOUNT_WAIT

    ORA A               ; Check result
    JNZ MOUNT_ERROR

FILENAME: DB 'GAMES.IMG', 0
```

**Error Codes:**

- 0x00: Success
- 0x01: File not found
- 0x02: Invalid filename
- 0xFF: Operation in progress

**Filename Constraints:**

- Max 12 characters
- Characters: [a-zA-Z0-9.-]
- Null-terminated
- Relative to configured disk base path

### Assembler Protocol

**Purpose:** Assemble 8080 mnemonics into opcodes

**Sequence:**

1. Send assembly text (write to port 0x28), null-terminated
1. Send assemble command (write 0x01 to port 0x29)
1. Read status (read from port 0x2A)
- Bits 0-3: number of bytes assembled
- Bits 4-7: error code (0 = success)
1. If success, read opcode bytes (read from port 0x2B)
1. If error, read error position (read from port 0x2C)

**Example:**

```asm
; Assemble "MVI A,42H"
    LXI H, ASM_TEXT
ASM_SEND:
    MOV A, M
    ORA A
    JZ ASM_DO
    OUT 28h             ; Send character
    INX H
    JMP ASM_SEND

ASM_DO:
    MVI A, 01h          ; Assemble command
    OUT 29h

    IN 2Ah              ; Read status
    ANI 0F0h            ; Check error bits
    JNZ ASM_ERROR

    IN 2Ah              ; Get byte count
    ANI 0Fh
    MOV B, A            ; B = byte count

    LXI H, OPCODE_BUF
ASM_READ:
    IN 2Bh              ; Read opcode byte
    MOV M, A
    INX H
    DCR B
    JNZ ASM_READ

ASM_TEXT: DB 'MVI A,42H', 0
OPCODE_BUF: DS 4
```

**Error Codes (bits 4-7):**

- 0x0: Success
- 0x1: Syntax error
- 0x2: Unknown mnemonic
- 0x3: Invalid operand
- 0xF: General error

### Disassembler Protocol

**Purpose:** Disassemble opcodes into mnemonics

**Sequence:**

1. Send opcode bytes (write to port 0x20)
1. Send disassemble command (write 0x01 to port 0x21)
1. Read status (read from port 0x22)
- Bits 0-6: text length
- Bit 7: error flag
1. If success, read ASCII text (read from port 0x23)

**Example:**

```asm
; Disassemble 0x3E 0x42 (MVI A,42H)
    MVI A, 3Eh
    OUT 20h             ; Send opcode
    MVI A, 42h
    OUT 20h             ; Send immediate

    MVI A, 01h          ; Disassemble command
    OUT 21h

    IN 22h              ; Read status
    ANI 80h             ; Check error bit
    JNZ DASM_ERROR

    IN 22h              ; Get text length
    ANI 7Fh
    MOV B, A            ; B = length

    LXI H, TEXT_BUF
DASM_READ:
    IN 23h              ; Read character
    MOV M, A
    INX H
    DCR B
    JNZ DASM_READ

    XRA A               ; Null terminate
    MOV M, A

TEXT_BUF: DS 32
```

**Output Format:**

```
"MVI  A,42H" (period-appropriate: uppercase, H suffix)
```

### HTTP Protocol

**Purpose:** Fetch web content

**Sequence:**

1. Send URL bytes (write to port 0x40), null-terminated
1. Send GET command (write 0x01 to port 0x41)
1. Read HTTP status (read from port 0x42)
1. Read response body bytes (read from port 0x44) until 0x00

**Example:**

```asm
; Fetch http://example.com/
    LXI H, URL_STR
HTTP_SEND_URL:
    MOV A, M
    ORA A
    JZ HTTP_GET
    OUT 40h             ; Send URL byte
    INX H
    JMP HTTP_SEND_URL

HTTP_GET:
    MVI A, 01h          ; GET command
    OUT 41h

    IN 42h              ; Read status code
    CPI 200             ; Check for 200 OK
    JNZ HTTP_ERROR

HTTP_READ:
    IN 44h              ; Read body byte
    ORA A               ; End of data?
    JZ HTTP_DONE
    CALL CONOUT         ; Display character
    JMP HTTP_READ

URL_STR: DB 'http://example.com/', 0
```

### Timer Protocol (8253 Mode 2)

**Purpose:** Generate periodic interrupts

**Initialization:**

1. Calculate count value: CPU_Hz / Desired_Hz
1. Write control byte (port 0x73)
1. Write count LSB (port 0x70)
1. Write count MSB (port 0x70)

**Example:**

```asm
; Initialize for 100Hz interrupts (2MHz CPU)
; Count = 2,000,000 / 100 = 20,000 = 0x4E20

    DI                  ; Disable interrupts

    ; Program Counter 0 for Mode 2
    ; SC=00 (Counter 0), RW=11 (LSB then MSB), M=010 (Mode 2), BCD=0
    MVI A, 00110100b    ; 0x34
    OUT 73h             ; Control register

    ; Set count value
    MVI A, 20h          ; LSB of 20,000
    OUT 70h             ; Counter 0
    MVI A, 4Eh          ; MSB of 20,000
    OUT 70h             ; Counter 0

    EI                  ; Enable interrupts
```

**Reading Counter:**

```asm
; Latch counter value
    MVI A, 00000000b    ; SC=00, latch command
    OUT 73h

; Read latched value
    IN 70h              ; LSB
    MOV L, A
    IN 70h              ; MSB
    MOV H, A
    ; HL now has current count
```

-----

## RUST PROJECT STRUCTURE

### Directory Layout

```
8080-emulator/
â”œâ”€â”€ Cargo.toml
â”œâ”€â”€ config.json
â”œâ”€â”€ README.md
â”œâ”€â”€ LICENSE
â”‚
â”œâ”€â”€ src/
â”‚   â”œâ”€â”€ main.rs
â”‚   â”œâ”€â”€ lib.rs
â”‚   â”‚
â”‚   â”œâ”€â”€ emulator/
â”‚   â”‚   â”œâ”€â”€ mod.rs
â”‚   â”‚   â”œâ”€â”€ cpu.rs
â”‚   â”‚   â”œâ”€â”€ memory.rs
â”‚   â”‚   â””â”€â”€ config.rs
â”‚   â”‚
â”‚   â”œâ”€â”€ devices/
â”‚   â”‚   â”œâ”€â”€ mod.rs
â”‚   â”‚   â”œâ”€â”€ console.rs
â”‚   â”‚   â”œâ”€â”€ disk_controller.rs
â”‚   â”‚   â”œâ”€â”€ disk_mounter.rs
â”‚   â”‚   â”œâ”€â”€ disassembler.rs
â”‚   â”‚   â”œâ”€â”€ assembler.rs
â”‚   â”‚   â”œâ”€â”€ internet.rs
â”‚   â”‚   â”œâ”€â”€ time.rs
â”‚   â”‚   â”œâ”€â”€ timer.rs
â”‚   â”‚   â””â”€â”€ system.rs
â”‚   â”‚
â”‚   â”œâ”€â”€ debugger/
â”‚   â”‚   â”œâ”€â”€ mod.rs
â”‚   â”‚   â”œâ”€â”€ breakpoints.rs
â”‚   â”‚   â”œâ”€â”€ tracer.rs
â”‚   â”‚   â””â”€â”€ commands.rs
â”‚   â”‚
â”‚   â”œâ”€â”€ disassembler/
â”‚   â”‚   â”œâ”€â”€ mod.rs
â”‚   â”‚   â”œâ”€â”€ decoder.rs
â”‚   â”‚   â””â”€â”€ formatter.rs
â”‚   â”‚
â”‚   â””â”€â”€ utils/
â”‚       â”œâ”€â”€ mod.rs
â”‚       â”œâ”€â”€ hex_loader.rs
â”‚       â””â”€â”€ state.rs
â”‚
â”œâ”€â”€ roms/
â”‚   â”œâ”€â”€ monitor.asm
â”‚   â”œâ”€â”€ monitor.bin
â”‚   â””â”€â”€ build.sh
â”‚
â”œâ”€â”€ disks/
â”‚   â”œâ”€â”€ system.img
â”‚   â””â”€â”€ data.img
â”‚
â”œâ”€â”€ examples/
â”‚   â””â”€â”€ hello.asm
â”‚
â”œâ”€â”€ tests/
â”‚   â””â”€â”€ integration_tests.rs
â”‚
â””â”€â”€ docs/
    â”œâ”€â”€ ARCHITECTURE.md
    â”œâ”€â”€ DEVICES.md
    â””â”€â”€ COMMANDS.md
```

### Cargo.toml

```toml
[package]
name = "emulator8080"
version = "1.0.0"
edition = "2021"
authors = ["Your Name <your.email@example.com>"]
description = "Intel 8080 emulator with monitor ROM"
license = "MIT"
build = "build.rs"

[dependencies]
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
reqwest = { version = "0.11", features = ["blocking", "json"] }
chrono = "0.4"
crossterm = "0.27"
clap = { version = "4.0", features = ["derive"] }
log = "0.4"
env_logger = "0.11"
anyhow = "1.0"
thiserror = "1.0"

[dev-dependencies]
criterion = "0.5"
proptest = "1.0"

[[bin]]
name = "emu8080"
path = "src/main.rs"

[lib]
name = "emulator8080"
path = "src/lib.rs"

[profile.release]
opt-level = 3
lto = true
codegen-units = 1
```

-----

## PORT MANAGEMENT STRATEGY

### Single Source of Truth

**Problem:** ROM assembly code and Rust devices both need to know port numbers. Keeping them synchronized manually is error-prone.

**Solution:** Define ports in Rust, generate assembly include file automatically.

### src/devices/ports.rs

```rust
//! I/O Port Definitions
//! 
//! Single source of truth for all device port assignments.
//! This file is used by both Rust devices and assembly ROM
//! (via generated include file).

// Console (0x00-0x03)
pub const CONSOLE_DATA_OUT: u8 = 0x00;
pub const CONSOLE_DATA_IN: u8 = 0x01;
pub const CONSOLE_STATUS: u8 = 0x02;
pub const CONSOLE_CONTROL: u8 = 0x03;

// Disk Controller (0x08-0x0F)
pub const DISK_COMMAND: u8 = 0x08;
pub const DISK_STATUS: u8 = 0x09;
pub const DISK_TRACK: u8 = 0x0A;
pub const DISK_SECTOR: u8 = 0x0B;
pub const DISK_DMA_LO: u8 = 0x0C;
pub const DISK_DMA_HI: u8 = 0x0D;
pub const DISK_DRIVE_SELECT: u8 = 0x0E;

// Disk Mount Service (0x10-0x13)
pub const DISK_MOUNT_NAME: u8 = 0x10;
pub const DISK_MOUNT_CONTROL: u8 = 0x11;
pub const DISK_MOUNT_STATUS: u8 = 0x12;
pub const DISK_MOUNT_DRIVE: u8 = 0x13;

// Disassembler (0x20-0x27)
pub const DASM_OPCODE_IN: u8 = 0x20;
pub const DASM_CONTROL: u8 = 0x21;
pub const DASM_STATUS: u8 = 0x22;
pub const DASM_TEXT_OUT: u8 = 0x23;

// Assembler (0x28-0x2F)
pub const ASM_TEXT_IN: u8 = 0x28;
pub const ASM_CONTROL: u8 = 0x29;
pub const ASM_STATUS: u8 = 0x2A;
pub const ASM_OPCODE_OUT: u8 = 0x2B;
pub const ASM_ERROR_POS: u8 = 0x2C;

// Internet Services - HTTP (0x40-0x47)
pub const HTTP_URL_BYTE: u8 = 0x40;
pub const HTTP_COMMAND: u8 = 0x41;
pub const HTTP_STATUS: u8 = 0x42;
pub const HTTP_HEADER_IN: u8 = 0x43;
pub const HTTP_BODY_IN: u8 = 0x44;
pub const HTTP_BODY_OUT: u8 = 0x45;

// Internet Services - Time (0x48-0x4F)
pub const TIME_COMMAND: u8 = 0x48;
pub const TIME_YEAR: u8 = 0x49;
pub const TIME_MONTH: u8 = 0x4A;
pub const TIME_DAY: u8 = 0x4B;
pub const TIME_HOUR: u8 = 0x4C;
pub const TIME_MINUTE: u8 = 0x4D;
pub const TIME_SECOND: u8 = 0x4E;

// Internet Services - DNS (0x50-0x57)
pub const DNS_HOSTNAME_IN: u8 = 0x50;
pub const DNS_COMMAND: u8 = 0x51;
pub const DNS_IP_0: u8 = 0x52;
pub const DNS_IP_1: u8 = 0x53;
pub const DNS_IP_2: u8 = 0x54;
pub const DNS_IP_3: u8 = 0x55;

// System Time - Read Only (0x60-0x6F)
pub const TIME_SEC: u8 = 0x60;
pub const TIME_MIN: u8 = 0x61;
pub const TIME_HR: u8 = 0x62;
pub const TIME_DAY_R: u8 = 0x63;
pub const TIME_MONTH_R: u8 = 0x64;
pub const TIME_YEAR_R: u8 = 0x65;
pub const TIME_DOW: u8 = 0x66;

pub const UPTIME_SEC: u8 = 0x68;
pub const UPTIME_MIN: u8 = 0x69;
pub const UPTIME_HOUR: u8 = 0x6A;
pub const UPTIME_DAYS: u8 = 0x6B;

pub const TIMER_MS_LO: u8 = 0x6C;
pub const TIMER_MS_HI: u8 = 0x6D;
pub const TIMER_CONTROL: u8 = 0x6E;

// Programmable Timer (0x70-0x73)
pub const TIMER_COUNTER_0: u8 = 0x70;
pub const TIMER_COUNTER_1: u8 = 0x71;
pub const TIMER_COUNTER_2: u8 = 0x72;
pub const TIMER_CTL: u8 = 0x73;

// System Control (0xFE-0xFF)
pub const SYSTEM_CONTROL: u8 = 0xFE;
pub const SYSTEM_STATUS: u8 = 0xFF;
```

### build.rs - Generate Assembly Include File

```rust
//! Build script to generate assembly port definitions
//! 
//! This generates roms/include/ports_generated.asm from
//! src/devices/ports.rs, ensuring ROM and Rust stay synchronized.

use std::fs::File;
use std::io::Write;
use std::path::Path;

// Include the ports module
mod devices {
    pub mod ports {
        include!("src/devices/ports.rs");
    }
}

use devices::ports;

fn main() {
    let out_dir = Path::new("roms/include");
    std::fs::create_dir_all(out_dir).expect("Failed to create roms/include directory");
    
    let out_path = out_dir.join("ports_generated.asm");
    let mut f = File::create(&out_path).expect("Failed to create ports_generated.asm");
    
    writeln!(f, ";â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•").unwrap();
    writeln!(f, "; AUTO-GENERATED PORT DEFINITIONS").unwrap();
    writeln!(f, "; Generated from src/devices/ports.rs").unwrap();
    writeln!(f, "; DO NOT EDIT THIS FILE DIRECTLY").unwrap();
    writeln!(f, ";").unwrap();
    writeln!(f, "; To change port assignments:").unwrap();
    writeln!(f, ";   1. Edit src/devices/ports.rs").unwrap();
    writeln!(f, ";   2. Run 'cargo build'").unwrap();
    writeln!(f, ";   3. Rebuild ROM with updated port definitions").unwrap();
    writeln!(f, ";â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•").unwrap();
    writeln!(f, "").unwrap();
    
    writeln!(f, "; Console (0x{:02X}-0x{:02X})", ports::CONSOLE_DATA_OUT, ports::CONSOLE_CONTROL).unwrap();
    writeln!(f, "CONSOLE_DATA_OUT    EQU {:02X}h", ports::CONSOLE_DATA_OUT).unwrap();
    writeln!(f, "CONSOLE_DATA_IN     EQU {:02X}h", ports::CONSOLE_DATA_IN).unwrap();
    writeln!(f, "CONSOLE_STATUS      EQU {:02X}h", ports::CONSOLE_STATUS).unwrap();
    writeln!(f, "CONSOLE_CONTROL     EQU {:02X}h", ports::CONSOLE_CONTROL).unwrap();
    writeln!(f, "").unwrap();
    
    writeln!(f, "; Disk Controller (0x{:02X}-0x0F)", ports::DISK_COMMAND).unwrap();
    writeln!(f, "DISK_COMMAND        EQU {:02X}h", ports::DISK_COMMAND).unwrap();
    writeln!(f, "DISK_STATUS         EQU {:02X}h", ports::DISK_STATUS).unwrap();
    writeln!(f, "DISK_TRACK          EQU {:02X}h", ports::DISK_TRACK).unwrap();
    writeln!(f, "DISK_SECTOR         EQU {:02X}h", ports::DISK_SECTOR).unwrap();
    writeln!(f, "DISK_DMA_LO         EQU {:02X}h", ports::DISK_DMA_LO).unwrap();
    writeln!(f, "DISK_DMA_HI         EQU {:02X}h", ports::DISK_DMA_HI).unwrap();
    writeln!(f, "DISK_DRIVE_SELECT   EQU {:02X}h", ports::DISK_DRIVE_SELECT).unwrap();
    writeln!(f, "").unwrap();
    
    writeln!(f, "; Disk Mount Service (0x{:02X}-0x{:02X})", ports::DISK_MOUNT_NAME, ports::DISK_MOUNT_DRIVE).unwrap();
    writeln!(f, "DISK_MOUNT_NAME     EQU {:02X}h", ports::DISK_MOUNT_NAME).unwrap();
    writeln!(f, "DISK_MOUNT_CONTROL  EQU {:02X}h", ports::DISK_MOUNT_CONTROL).unwrap();
    writeln!(f, "DISK_MOUNT_STATUS   EQU {:02X}h", ports::DISK_MOUNT_STATUS).unwrap();
    writeln!(f, "DISK_MOUNT_DRIVE    EQU {:02X}h", ports::DISK_MOUNT_DRIVE).unwrap();
    writeln!(f, "").unwrap();
    
    writeln!(f, "; Disassembler (0x{:02X}-0x27)", ports::DASM_OPCODE_IN).unwrap();
    writeln!(f, "DASM_OPCODE_IN      EQU {:02X}h", ports::DASM_OPCODE_IN).unwrap();
    writeln!(f, "DASM_CONTROL        EQU {:02X}h", ports::DASM_CONTROL).unwrap();
    writeln!(f, "DASM_STATUS         EQU {:02X}h", ports::DASM_STATUS).unwrap();
    writeln!(f, "DASM_TEXT_OUT       EQU {:02X}h", ports::DASM_TEXT_OUT).unwrap();
    writeln!(f, "").unwrap();
    
    writeln!(f, "; Assembler (0x{:02X}-0x2F)", ports::ASM_TEXT_IN).unwrap();
    writeln!(f, "ASM_TEXT_IN         EQU {:02X}h", ports::ASM_TEXT_IN).unwrap();
    writeln!(f, "ASM_CONTROL         EQU {:02X}h", ports::ASM_CONTROL).unwrap();
    writeln!(f, "ASM_STATUS          EQU {:02X}h", ports::ASM_STATUS).unwrap();
    writeln!(f, "ASM_OPCODE_OUT      EQU {:02X}h", ports::ASM_OPCODE_OUT).unwrap();
    writeln!(f, "ASM_ERROR_POS       EQU {:02X}h", ports::ASM_ERROR_POS).unwrap();
    writeln!(f, "").unwrap();
    
    writeln!(f, "; Internet - HTTP (0x{:02X}-0x47)", ports::HTTP_URL_BYTE).unwrap();
    writeln!(f, "HTTP_URL_BYTE       EQU {:02X}h", ports::HTTP_URL_BYTE).unwrap();
    writeln!(f, "HTTP_COMMAND        EQU {:02X}h", ports::HTTP_COMMAND).unwrap();
    writeln!(f, "HTTP_STATUS         EQU {:02X}h", ports::HTTP_STATUS).unwrap();
    writeln!(f, "HTTP_HEADER_IN      EQU {:02X}h", ports::HTTP_HEADER_IN).unwrap();
    writeln!(f, "HTTP_BODY_IN        EQU {:02X}h", ports::HTTP_BODY_IN).unwrap();
    writeln!(f, "HTTP_BODY_OUT       EQU {:02X}h", ports::HTTP_BODY_OUT).unwrap();
    writeln!(f, "").unwrap();
    
    writeln!(f, "; Internet - Time Service (0x{:02X}-0x4F)", ports::TIME_COMMAND).unwrap();
    writeln!(f, "TIME_COMMAND        EQU {:02X}h", ports::TIME_COMMAND).unwrap();
    writeln!(f, "TIME_YEAR           EQU {:02X}h", ports::TIME_YEAR).unwrap();
    writeln!(f, "TIME_MONTH          EQU {:02X}h", ports::TIME_MONTH).unwrap();
    writeln!(f, "TIME_DAY            EQU {:02X}h", ports::TIME_DAY).unwrap();
    writeln!(f, "TIME_HOUR           EQU {:02X}h", ports::TIME_HOUR).unwrap();
    writeln!(f, "TIME_MINUTE         EQU {:02X}h", ports::TIME_MINUTE).unwrap();
    writeln!(f, "TIME_SECOND         EQU {:02X}h", ports::TIME_SECOND).unwrap();
    writeln!(f, "").unwrap();
    
    writeln!(f, "; Internet - DNS (0x{:02X}-0x57)", ports::DNS_HOSTNAME_IN).unwrap();
    writeln!(f, "DNS_HOSTNAME_IN     EQU {:02X}h", ports::DNS_HOSTNAME_IN).unwrap();
    writeln!(f, "DNS_COMMAND         EQU {:02X}h", ports::DNS_COMMAND).unwrap();
    writeln!(f, "DNS_IP_0            EQU {:02X}h", ports::DNS_IP_0).unwrap();
    writeln!(f, "DNS_IP_1            EQU {:02X}h", ports::DNS_IP_1).unwrap();
    writeln!(f, "DNS_IP_2            EQU {:02X}h", ports::DNS_IP_2).unwrap();
    writeln!(f, "DNS_IP_3            EQU {:02X}h", ports::DNS_IP_3).unwrap();
    writeln!(f, "").unwrap();
    
    writeln!(f, "; System Time - Read Only (0x{:02X}-0x6F)", ports::TIME_SEC).unwrap();
    writeln!(f, "TIME_SEC            EQU {:02X}h", ports::TIME_SEC).unwrap();
    writeln!(f, "TIME_MIN            EQU {:02X}h", ports::TIME_MIN).unwrap();
    writeln!(f, "TIME_HR             EQU {:02X}h", ports::TIME_HR).unwrap();
    writeln!(f, "TIME_DAY_R          EQU {:02X}h", ports::TIME_DAY_R).unwrap();
    writeln!(f, "TIME_MONTH_R        EQU {:02X}h", ports::TIME_MONTH_R).unwrap();
    writeln!(f, "TIME_YEAR_R         EQU {:02X}h", ports::TIME_YEAR_R).unwrap();
    writeln!(f, "TIME_DOW            EQU {:02X}h", ports::TIME_DOW).unwrap();
    writeln!(f, "").unwrap();
    
    writeln!(f, "UPTIME_SEC          EQU {:02X}h", ports::UPTIME_SEC).unwrap();
    writeln!(f, "UPTIME_MIN          EQU {:02X}h", ports::UPTIME_MIN).unwrap();
    writeln!(f, "UPTIME_HOUR         EQU {:02X}h", ports::UPTIME_HOUR).unwrap();
    writeln!(f, "UPTIME_DAYS         EQU {:02X}h", ports::UPTIME_DAYS).unwrap();
    writeln!(f, "").unwrap();
    
    writeln!(f, "TIMER_MS_LO         EQU {:02X}h", ports::TIMER_MS_LO).unwrap();
    writeln!(f, "TIMER_MS_HI         EQU {:02X}h", ports::TIMER_MS_HI).unwrap();
    writeln!(f, "TIMER_CONTROL       EQU {:02X}h", ports::TIMER_CONTROL).unwrap();
    writeln!(f, "").unwrap();
    
    writeln!(f, "; Programmable Timer (0x{:02X}-0x{:02X})", ports::TIMER_COUNTER_0, ports::TIMER_CTL).unwrap();
    writeln!(f, "TIMER_COUNTER_0     EQU {:02X}h", ports::TIMER_COUNTER_0).unwrap();
    writeln!(f, "TIMER_COUNTER_1     EQU {:02X}h", ports::TIMER_COUNTER_1).unwrap();
    writeln!(f, "TIMER_COUNTER_2     EQU {:02X}h", ports::TIMER_COUNTER_2).unwrap();
    writeln!(f, "TIMER_CTL           EQU {:02X}h", ports::TIMER_CTL).unwrap();
    writeln!(f, "").unwrap();
    
    writeln!(f, "; System Control (0x{:02X}-0x{:02X})", ports::SYSTEM_CONTROL, ports::SYSTEM_STATUS).unwrap();
    writeln!(f, "SYSTEM_CONTROL      EQU {:02X}h", ports::SYSTEM_CONTROL).unwrap();
    writeln!(f, "SYSTEM_STATUS       EQU {:02X}h", ports::SYSTEM_STATUS).unwrap();
    
    println!("cargo:rerun-if-changed=src/devices/ports.rs");
    println!("cargo:warning=Generated {}", out_path.display());
}
```

### Using Ports in Rust Devices

**src/devices/timer.rs (updated):**

```rust
use super::Device;
use super::ports;  // Import port constants
use anyhow::Result;

pub struct Timer8253 {
    // ... fields ...
}

impl Device for Timer8253 {
    fn name(&self) -> &str {
        "Timer8253"
    }
    
    fn ports(&self) -> &[u8] {
        &[
            ports::TIMER_COUNTER_0,
            ports::TIMER_COUNTER_1,
            ports::TIMER_COUNTER_2,
            ports::TIMER_CTL,
        ]
    }
    
    fn read_port(&mut self, port: u8) -> Result<u8> {
        match port {
            ports::TIMER_COUNTER_0 => { /* ... */ },
            ports::TIMER_COUNTER_1 => { /* ... */ },
            ports::TIMER_COUNTER_2 => { /* ... */ },
            _ => Ok(0xFF)
        }
    }
    
    fn write_port(&mut self, port: u8, value: u8) -> Result<()> {
        match port {
            ports::TIMER_COUNTER_0 => { /* ... */ },
            ports::TIMER_COUNTER_1 => { /* ... */ },
            ports::TIMER_COUNTER_2 => { /* ... */ },
            ports::TIMER_CTL => { /* ... */ },
            _ => {}
        }
        Ok(())
    }
}
```

### Using Ports in Assembly ROM

**roms/include/constants.asm (updated):**

```asm
;â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•
; SYSTEM CONSTANTS
;â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•

; Memory map
ROM_BASE        EQU 0F000h
ROM_SIZE        EQU 01000h
VECTOR_TABLE    EQU 00000h
BIOS_TABLE      EQU 00040h
WORKSPACE       EQU 00080h
USER_SPACE      EQU 00100h

; Workspace variables
LINE_BUFFER     EQU 00080h
TICK_COUNT      EQU 000D0h
SECONDS         EQU 000D2h
MINUTES         EQU 000D3h
HOURS           EQU 000D4h

; Character constants
CR              EQU 0Dh
LF              EQU 0Ah
BS              EQU 08h
SPACE           EQU 20h
CTRL_C          EQU 03h

;â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•
; PORT DEFINITIONS (auto-generated from src/devices/ports.rs)
;â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•
.include "ports_generated.asm"
```

**Now ROM code can use the constants:**

```asm
; Console I/O
CONOUT_IMPL:
    PUSH PSW
    OUT CONSOLE_DATA_OUT    ; Uses generated constant
    POP PSW
    RET

; Timer initialization
INIT_TIMER:
    MVI A, 00110100b
    OUT TIMER_CTL           ; Uses generated constant
    MVI A, 20h
    OUT TIMER_COUNTER_0     ; Uses generated constant
    ; ... etc
```

### Build Workflow

1. **Edit ports:** Modify `src/devices/ports.rs`
1. **Build Rust:** `cargo build` â†’ generates `roms/include/ports_generated.asm`
1. **Build ROM:** `cd roms && ./build.sh` â†’ assembles ROM with updated ports
1. **Run:** `cargo run` â†’ emulator loads ROM with synchronized port definitions

### Benefits

âœ… **Single source of truth** - Ports defined once in Rust
âœ… **Automatic synchronization** - Assembly always matches Rust
âœ… **No manual maintenance** - Build script handles generation
âœ… **Type safety** - Rust constants prevent typos
âœ… **Easy to change** - Edit one file, rebuild everything
âœ… **Self-documenting** - Generated file shows current port map

### Future: System Profiles

For CP/M or Space Invaders compatibility, you can later add:

```rust
// src/systems/mod.rs
pub trait SystemProfile {
    fn name(&self) -> &str;
    fn rom_path(&self) -> &str;
    fn port_map(&self) -> PortMap;
    fn create_devices(&self) -> Vec<Box<dyn Device>>;
}

// Different systems with different port layouts
pub struct StandardSystem;    // Uses ports.rs constants
pub struct CPMSystem;          // Different port layout
pub struct SpaceInvadersSystem; // Arcade hardware ports
```

But for now, single port map with generated assembly is the right approach.

### Key Rust Code Templates

See the full artifact for complete code templates including:

- src/main.rs - CLI entry point
- src/emulator/mod.rs - Core emulator loop
- src/devices/mod.rs - Device trait and registry
- src/devices/timer.rs - Full Timer8253 implementation
- Additional device templates

-----

## ASSEMBLY ROM STRUCTURE

### Directory Layout

```
roms/
â”œâ”€â”€ monitor.asm
â”œâ”€â”€ include/
â”‚   â”œâ”€â”€ macros.asm
â”‚   â”œâ”€â”€ constants.asm
â”‚   â””â”€â”€ bios_api.asm
â”‚
â”œâ”€â”€ src/
â”‚   â”œâ”€â”€ init.asm
â”‚   â”œâ”€â”€ vectors.asm
â”‚   â”œâ”€â”€ console.asm
â”‚   â”œâ”€â”€ commands/
â”‚   â”‚   â”œâ”€â”€ dump.asm
â”‚   â”‚   â”œâ”€â”€ examine.asm
â”‚   â”‚   â”œâ”€â”€ fill.asm
â”‚   â”‚   â”œâ”€â”€ move.asm
â”‚   â”‚   â”œâ”€â”€ search.asm
â”‚   â”‚   â”œâ”€â”€ compare.asm
â”‚   â”‚   â”œâ”€â”€ hex_math.asm
â”‚   â”‚   â”œâ”€â”€ go.asm
â”‚   â”‚   â”œâ”€â”€ call.asm
â”‚   â”‚   â”œâ”€â”€ registers.asm
â”‚   â”‚   â”œâ”€â”€ io.asm
â”‚   â”‚   â”œâ”€â”€ disk.asm
â”‚   â”‚   â”œâ”€â”€ hex_loader.asm
â”‚   â”‚   â”œâ”€â”€ time.asm
â”‚   â”‚   â”œâ”€â”€ network.asm
â”‚   â”‚   â””â”€â”€ help.asm
â”‚   â”‚
â”‚   â”œâ”€â”€ parser.asm
â”‚   â”œâ”€â”€ helpers.asm
â”‚   â”œâ”€â”€ print.asm
â”‚   â”œâ”€â”€ isr.asm
â”‚   â””â”€â”€ strings.asm
â”‚
â”œâ”€â”€ build.sh
â””â”€â”€ README.md
```

### Key Assembly Code Templates

See the full artifact for complete assembly code including:

- include/constants.asm - All port and memory definitions
- src/init.asm - Cold start and initialization
- src/parser.asm - Command parser and dispatcher
- src/helpers.asm - Shared helper functions
- src/print.asm - Print utilities
- src/console.asm - Console I/O implementation
- src/commands/dump.asm - Memory dump command
- src/isr.asm - Interrupt service routines
- src/vectors.asm - RST and BIOS jump tables
- src/strings.asm - String constants

-----

## CONFIGURATION

### config.json

```json
{
  "system": {
    "name": "8080 Development System",
    "rom_path": "./roms/monitor.bin",
    "ram_size_kb": 60
  },
  
  "memory": {
    "user_start": "0x0100",
    "user_end": "0xEFFF",
    "rom_start": "0xF000",
    "rom_end": "0xFFFF",
    "vector_table": "0x0000",
    "bios_table": "0x0040",
    "workspace": "0x0080"
  },
  
  "cpu": {
    "speed_mhz": 2.0,
    "cycle_accurate": true
  },
  
  "disk": {
    "format": "ibm3740",
    "sectors_per_track": 26,
    "bytes_per_sector": 128,
    "tracks": 77,
    "base_path": "./disks/",
    "drives": {
      "A": "system.img",
      "B": "data.img",
      "C": "",
      "D": ""
    },
    "auto_save": true
  },
  
  "console": {
    "type": "terminal",
    "echo": true,
    "line_editing": true,
    "control_c_abort": true
  },
  
  "boot": {
    "rom_overlay_enabled": true,
    "auto_boot": false,
    "boot_drive": "A",
    "boot_address": "0x0000"
  },
  
  "devices": {
    "timer_enabled": true,
    "internet_enabled": true,
    "assembler_path": "./asm8080",
    "max_http_response_kb": 1024
  },
  
  "debug": {
    "trace_instructions": false,
    "log_port_io": false,
    "log_interrupts": false,
    "log_overlay_state": false
  }
}
```

-----

## EMULATOR IMPLEMENTATION: ROM OVERLAY

### Memory Subsystem Changes

```rust
pub struct Memory {
    ram: [u8; 65536],
    rom: [u8; 4096],
    rom_overlay_enabled: bool,
}

impl Memory {
    pub fn reset(&mut self) {
        self.rom_overlay_enabled = true;  // Enable on reset
    }
    
    pub fn read(&self, addr: u16) -> u8 {
        if addr >= 0xF000 {
            // Always ROM at F000-FFFF
            self.rom[(addr - 0xF000) as usize]
        } else if addr < 0x1000 && self.rom_overlay_enabled {
            // ROM overlay active: 0000-0FFF mirrors F000-FFFF
            self.rom[addr as usize]
        } else {
            self.ram[addr as usize]
        }
    }
    
    pub fn write(&mut self, addr: u16, value: u8) {
        if addr >= 0xF000 {
            // ROM - ignore writes
        } else if addr < 0x1000 && self.rom_overlay_enabled {
            // Overlay active - ignore writes to 0000-0FFF
        } else {
            self.ram[addr as usize] = value;
        }
    }
    
    pub fn set_overlay(&mut self, enabled: bool) {
        self.rom_overlay_enabled = enabled;
    }
    
    pub fn overlay_enabled(&self) -> bool {
        self.rom_overlay_enabled
    }
}
```

### System Control Device

```rust
pub struct SystemControl {
    memory: Rc<RefCell<Memory>>,
}

impl IoDevice for SystemControl {
    fn read(&mut self, port: u8) -> u8 {
        match port {
            0xFF => {
                // SYSTEM_STATUS
                let overlay = self.memory.borrow().overlay_enabled();
                if overlay { 0x01 } else { 0x00 }
            }
            _ => 0xFF
        }
    }
    
    fn write(&mut self, port: u8, value: u8) {
        match port {
            0xFE => {
                match value {
                    0x00 => {
                        // Disable overlay
                        self.memory.borrow_mut().set_overlay(false);
                    }
                    0x01 => {
                        // Halt - handled by CPU
                    }
                    0xFF => {
                        // Cold reset - re-enable overlay
                        self.memory.borrow_mut().set_overlay(true);
                        // CPU should jump to 0x0000
                    }
                    _ => {}
                }
            }
            _ => {}
        }
    }
}
```

-----

## IMPLEMENTATION PLAN

### Phase 1: Core Monitor (NEXT - Week 1)

**Goal:** Basic monitor with memory operations

- [ ] Assemble ROM skeleton (init, vectors, console)
- [ ] Implement command parser framework
- [ ] Implement helper functions (SKIP_SPACES, READ_HEX_WORD, etc.)
- [ ] Implement D command (memory dump)
- [ ] Implement E command (examine/modify)
- [ ] Implement G command (execute)
- [ ] Test: Dump ROM, examine RAM, execute simple programs

**Deliverable:** Working monitor that can dump/modify memory and execute code

### Phase 2: Memory Operations (Week 2)

**Goal:** Complete memory manipulation suite

- [ ] Implement F command (fill)
- [ ] Implement M command (move)
- [ ] Implement S command (search)
- [ ] Implement C command (compare)
- [ ] Implement H command (hex arithmetic)
- [ ] Test: All memory operations

**Deliverable:** Full memory operation toolset

### Phase 3: Execution & I/O (Week 3)

**Goal:** Program execution and I/O control

- [x] Implement I command (input from port) - DONE
- [x] Implement O command (output to port) - DONE
- [ ] Implement C command (call with return)
- [ ] R command (register display) - DEFERRED until debugger phase
- [ ] Test: Execute programs, test I/O
- [ ] Implement register save/restore mechanism
- [ ] Implement R command (register display) - deferred from Phase 3

**Deliverable:** Complete execution control and I/O access

### Phase 4: Disk System (Week 4)

**Goal:** Storage operations

- [ ] Implement DiskController device (Rust)
- [ ] Implement DiskMounter device (Rust)
- [ ] Implement X command (mount disk)
- [ ] Implement L command (list mounted)
- [ ] Implement B command (boot from disk)
- [ ] Create disk image tools
- [ ] Test: Mount images, read/write sectors, boot

**Deliverable:** Working disk system with multiple drives

### Phase 5: Program Loading (Week 5)

**Goal:** Load programs into memory

- [ ] Implement Intel HEX loader (H command)
- [ ] Validate checksums
- [ ] Handle Type 00 (data) and Type 01 (EOF)
- [ ] Error handling
- [ ] Test: Load HEX files

**Deliverable:** Can load programs from Intel HEX format

### Phase 6: Timing System (Week 6)

**Goal:** Real-time clock and interrupts

- [ ] Implement TimeDevice (Rust) - ports 0x60-0x6F
- [ ] Implement Timer8253 (Rust) - ports 0x70-0x73
- [ ] Implement TIMER_ISR in ROM
- [ ] Implement T command (show time)
- [ ] Implement TI command (init timer)
- [ ] Implement TS command (timer status)
- [ ] Test: Interrupts firing, software clock updating

**Deliverable:** Interrupt-driven timing with software clock

### Phase 7: Development Tools (Week 7)

**Goal:** Assembly and disassembly

- [ ] Implement DisassemblerDevice (Rust)
- [ ] Implement AssemblerDevice (Rust)
- [ ] Integration with your existing assembler
- [ ] Test: Assemble instructions, disassemble code
- [ ] Monitor integration (A, U commands if adding)

**Deliverable:** Can assemble/disassemble 8080 code

### Phase 8: Internet Services (Week 8)

**Goal:** Network connectivity

- [ ] Implement InternetDevice (Rust)
- [ ] HTTP GET support
- [ ] DNS resolution
- [ ] Time sync (NTP-style)
- [ ] Implement N W command (HTTP GET)
- [ ] Implement N T command (get time)
- [ ] Streaming/paging for large responses
- [ ] Test: Fetch webpages, sync time

**Deliverable:** Can access internet from 8080

### Phase 9: Debugger (Week 9)

**Goal:** Advanced debugging features

- [ ] Implement register save/restore mechanism
- [ ] Implement R command (register display) - deferred from Phase 3
- [ ] Implement breakpoint system (Rust)
- [ ] Implement single-step execution
- [ ] Implement instruction trace
- [ ] Emulator command parser (`:` prefix)
- [ ] Implement `:bp`, `:step`, `:trace` commands
- [ ] Test: Debug programs with breakpoints

**Deliverable:** Full debugging infrastructure

### Phase 10: Polish & Documentation (Week 10)

**Goal:** Complete, documented system

- [ ] Implement help system (? command)
- [ ] Implement version command (V)
- [ ] Startup banner
- [ ] Self-test routine
- [ ] Auto-boot support
- [ ] State save/load
- [ ] Write documentation
- [ ] Create example programs
- [ ] Performance optimization

**Deliverable:** Production-ready system with documentation

-----

## TESTING STRATEGY

### Unit Tests

**CPU Tests:**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_mvi_a() {
        let mut cpu = CPU8080::new();
        let mut mem = Memory::new(65536);
        
        mem.write(0, 0x3E);  // MVI A,
        mem.write(1, 0x42);  // 42h
        
        let cycles = cpu.execute(&mut mem, &mut DeviceRegistry::new()).unwrap();
        
        assert_eq!(cpu.a(), 0x42);
        assert_eq!(cpu.pc(), 2);
        assert_eq!(cycles, 7);
    }
}
```

**Device Tests:**

```rust
#[test]
fn test_timer_interrupt() {
    let mut timer = Timer8253::new(2_000_000.0);
    
    // Program for 100Hz (20,000 cycles)
    timer.write_port(0x73, 0x34).unwrap();
    timer.write_port(0x70, 0x20).unwrap();
    timer.write_port(0x70, 0x4E).unwrap();
    
    timer.tick(20000).unwrap();
    
    assert!(timer.has_interrupt());
    assert_eq!(timer.get_interrupt_vector(), Some(0xFF));
}
```

### Integration Tests

```rust
#[test]
fn test_monitor_boot() {
    let config = Config::default();
    let mut emu = Emulator::new(config).unwrap();
    
    assert_eq!(emu.cpu.pc(), 0xF000);
}
```

-----

## QUICK START CHECKLIST

### Before Starting Implementation

- [x] CPU emulator working (all opcodes)
- [x] Memory subsystem working
- [x] Basic console I/O working
- [x] Project structure created
- [ ] Assembler toolchain ready
- [ ] Configuration file created
- [ ] Disk images created (blank)

### Week 1 Goals (Immediate Next Steps)

1. Set up ROM build environment
1. Implement ROM basics (constants, vectors, cold start, console, helpers)
1. Implement first commands (parser, D, E)
1. Test & Debug

### Success Criteria for Week 1

- [ ] ROM assembles without errors
- [ ] Emulator loads ROM and starts at 0xF000
- [ ] Banner displays on startup
- [ ] Prompt appears: â€œ>â€
- [ ] D command dumps memory correctly
- [ ] E command modifies memory correctly

-----

## APPENDIX: QUICK REFERENCE

### Monitor Commands Summary

```
D [start] [end]         - Dump memory
E [addr]                - Examine/modify
F [start] [end] [val]   - Fill
M [src] [dst] [cnt]     - Move
S [start] [end] [...]   - Search
C [start] [end] [dst]   - Compare
H [num1] [num2]         - Hex arithmetic
G [addr]                - Go/execute
C [addr]                - Call
R                       - Registers (DEFERRED)
I [port]                - Input
O [port] [val]          - Output
X [drv] [file]          - Mount disk
L [drv]                 - List disk
B [addr]                - Boot
H                       - Load HEX
N W [url]               - HTTP GET
N T                     - Get time
T                       - Show time
TI [freq]               - Init timer
TS                      - Timer status
?                       - Help
V                       - Version
Z                       - Cold start
Q                       - Quit
```

### Port Map Quick Reference

```
00-03: Console
08-0F: Disk Controller
10-13: Disk Mounter
20-27: Disassembler
28-2F: Assembler
40-5F: Internet
60-6F: System Time
70-73: Timer (8253)
FE:    System Control
FF:    System Status
```

### System Control Commands (Port 0xFE)

```
00h: Disable ROM overlay (expose RAM at 0000-0FFF)
01h: Halt CPU
FFh: Cold reset (re-enable overlay, restart)
```

### Memory Map Quick Reference

```
0000-003F: RST Vectors (RAM after boot)
0040-007F: BIOS Table (RAM after boot)
0080-00FF: Workspace
0100-EFFF: User Programs
F000-FFFF: Monitor ROM
```

### Boot Sequence Quick Reference

```
1. Reset â†’ overlay enabled, PC=0000
2. CPU executes ROM (visible at 0000)
3. ROM does OUT 0FEh, 00h â†’ overlay disabled
4. ROM copies vectors to RAM at 0000
5. System ready
```

-----

**END OF SPECIFICATION**

This document serves as the complete reference for the 8080 Monitor ROM & Emulator project. All design decisions, code templates, protocols, and implementation plans are contained herein.