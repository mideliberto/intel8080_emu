# System Architecture

## Memory Map

### Overview

```
0x0000-0x003F   RST Vector Table (64 bytes, RAM after boot)
0x0040-0x007F   Jump Table (64 bytes, RAM after boot)
0x0080-0x00FF   System Workspace (128 bytes, RAM)
0x0100-0xEFFF   User Program Area (59,904 bytes)
0xF000-0xFFFF   Monitor ROM (4,096 bytes)
```

**Rationale:** Clean separation. Vectors at bottom, ROM at top, everything else is playground. 0x0100 is a nice round start address for user code.

### Detailed Memory Map

```
+----------------+----------------------------------------------+
| Address Range  | Description                                  |
+----------------+----------------------------------------------+
| 0x0000-0x0007  | RST 0 Vector - JMP COLD_START                |
| 0x0008-0x000F  | RST 1 Vector - JMP CONOUT_IMPL               |
| 0x0010-0x0017  | RST 2 Vector - JMP CONIN_IMPL                |
| 0x0018-0x001F  | RST 3 Vector - JMP CONST_IMPL                |
| 0x0020-0x0027  | RST 4 Vector (Reserved)                      |
| 0x0028-0x002F  | RST 5 Vector (Reserved)                      |
| 0x0030-0x0037  | RST 6 Vector (Reserved)                      |
| 0x0038-0x003F  | RST 7 Vector - JMP TIMER_ISR                 |
+----------------+----------------------------------------------+
| 0x0040-0x0042  | API: CONOUT (JMP)                            |
| 0x0043-0x0045  | API: CONIN (JMP)                             |
| 0x0046-0x0048  | API: CONST (JMP)                             |
| 0x0049-0x004B  | API: PRINT_STRING (JMP)                      |
| 0x004C-0x004E  | API: PRINT_HEX_BYTE (JMP)                    |
| 0x004F-0x0051  | API: PRINT_HEX_WORD (JMP)                    |
| 0x0052-0x0054  | API: READ_HEX_WORD (JMP)                     |
| 0x0055-0x0057  | API: SKIP_SPACES (JMP)                       |
| 0x0058-0x005A  | API: STORAGE_READ (JMP)                      |
| 0x005B-0x005D  | API: STORAGE_WRITE (JMP)                     |
| 0x005E-0x007F  | API: Additional entries / padding            |
+----------------+----------------------------------------------+
| 0x0080-0x00CF  | LINE_BUFFER (80 bytes)                       |
| 0x00D0-0x00D1  | BUFFER_PTR (2 bytes)                         |
| 0x00D2-0x00D3  | LAST_DUMP_ADDR (2 bytes)                     |
| 0x00D4-0x00D5  | LAST_EXAM_ADDR (2 bytes)                     |
| 0x00D6-0x00D8  | IO_IN_STUB (3 bytes)                         |
| 0x00D9-0x00DB  | IO_OUT_STUB (3 bytes)                        |
| 0x00DC-0x00E3  | SEARCH_PATTERN (8 bytes)                     |
| 0x00E4         | SEARCH_LENGTH (1 byte)                       |
| 0x00E5-0x00E6  | SEARCH_END (2 bytes)                         |
| 0x00E7-0x00E9  | STOR_ADDR (3 bytes, 24-bit)                  |
| 0x00EA-0x00FF  | Available (22 bytes)                         |
+----------------+----------------------------------------------+
| 0x0100-0xEFFF  | USER PROGRAM AREA                            |
+----------------+----------------------------------------------+
| 0xF000-0xFFFF  | MONITOR ROM                                  |
+----------------+----------------------------------------------+
```

---

## ROM Overlay Boot Mechanism

### The Problem

The 8080 starts execution at 0x0000 on reset. Our ROM lives at 0xF000. We need vectors at low addresses for interrupts, but RAM is undefined at power-on.

### The Solution

ROM overlay with hardware bank switching. On reset, ROM appears at *two* address ranges:

```
              RESET STATE (overlay enabled)
+----------------+---------------------------+
| 0x0000-0x0FFF  | ROM (mirror of F000)      |
| 0x1000-0xEFFF  | RAM                       |
| 0xF000-0xFFFF  | ROM (primary)             |
+----------------+---------------------------+

              RUN STATE (overlay disabled)
+----------------+---------------------------+
| 0x0000-0xEFFF  | RAM                       |
| 0xF000-0xFFFF  | ROM                       |
+----------------+---------------------------+
```

### State Transitions

| Trigger | Result |
|---------|--------|
| Hardware RESET | Overlay enabled, PC=0x0000 |
| OUT 0xFE, 0x00 | Overlay disabled (RAM at 0x0000) |
| OUT 0xFE, 0xFF | Cold reset (overlay re-enabled) |

### Hardware Implementation

For future physical build:
- 74LS74 flip-flop controls overlay state
- Set on reset (overlay enabled)
- Cleared by write to port 0xFE with value 0x00
- Address decode logic checks flip-flop for 0x0000-0x0FFF access

This is how real S-100 systems solved the boot problem. We're using a proven pattern.

---

## Boot Sequence

### Power-On Flow

```
1. RESET
   â””â”€> Overlay enabled, PC = 0x0000

2. CPU executes from 0x0000 (reads ROM via overlay)
   â””â”€> LXI SP, F000h
   â””â”€> DI
   â””â”€> JMP BOOT_CONTINUE  ; Jump to F000+ address space

3. Now executing from 0xF000+ range
   â””â”€> OUT 0FEh, 00h      ; Disable overlay
   â””â”€> 0x0000-0x0FFF is now RAM

4. Copy vectors from ROM to RAM
   â””â”€> RST vectors at 0x0000-0x003F
   â””â”€> API table at 0x0040-0x007F

5. Initialize workspace, I/O stubs, devices

6. Print banner, enter monitor loop
```

### Cold Start Code

```asm
COLD_START:
        LXI     SP,STACK_TOP        ; Stack below ROM
        DI                          ; No interrupts yet
        JMP     BOOT_CONTINUE       ; Escape overlay region

BOOT_CONTINUE:
        ; Now PC is in 0xF000+ range - safe to disable overlay
        XRA     A                   ; A = 0x00
        OUT     SYSTEM_CONTROL      ; Disable overlay

        ; Initialize workspace, copy vectors, etc.
        ; ...
        
        EI
        JMP     MONITOR_LOOP
```

**Critical:** The `JMP BOOT_CONTINUE` escapes the overlay region *before* disabling it. Without this, disabling overlay would cause PC to read garbage RAM.

---

## I/O Port Map

### Port Allocation

| Range | Device | Status |
|-------|--------|--------|
| 0x00-0x02 | Console | âœ… Implemented |
| 0x03 | Console Control | Reserved |
| 0x04-0x07 | (Parallel I/O) | Reserved |
| 0x08-0x0C | Storage Device (24-bit) | ✅ Done |
| 0x0D-0x0F | Storage Mount | ✅ Done |
| 0x10-0x1F | Network | Future |
| 0x20-0x27 | Disassembler | Future |
| 0x28-0x2F | Assembler | Future |
| 0x30-0x37 | (Debugger) | Reserved |
| 0x38-0x3F | Claude API | Future |
| 0x40-0x5F | Internet (HTTP, DNS, Time) | Future |
| 0x60-0x6F | System Time | Future |
| 0x70-0x73 | Timer (8253) | Future |
| 0x74-0xEF | (Expansion) | Available |
| 0xF0-0xFD | (Reserved) | - |
| 0xFE | System Control | âœ… Implemented |
| 0xFF | System Status | âœ… Implemented |

---

## RST Vectors

| Vector | Address | Purpose |
|--------|---------|---------|
| RST 0 | 0x0000 | Cold start |
| RST 1 | 0x0008 | CONOUT - Console output |
| RST 2 | 0x0010 | CONIN - Console input |
| RST 3 | 0x0018 | CONST - Console status |
| RST 4 | 0x0020 | (Reserved) |
| RST 5 | 0x0028 | (Reserved) |
| RST 6 | 0x0030 | (Reserved) |
| RST 7 | 0x0038 | Timer interrupt |

---

## ROM Organization

```
F000: COLD_START, BOOT_CONTINUE
F0XX: MONITOR_LOOP (prompt, read, parse, dispatch)
F1XX: Command handlers (CMD_DUMP, CMD_EXAMINE, etc.)
F4XX: API functions (CONOUT, CONIN, PRINT_*, etc.)
F6XX: Storage functions (future)
F7XX: Interrupt service routines
F8XX: Helper functions, error handling
FAXX: Vector table source (copied to RAM at boot)
FCXX: String constants (banner, help, errors)
```
