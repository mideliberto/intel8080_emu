# System Architecture

## Memory Map

### Overview

```
0x0000-0x007F   Page 0 low (128 bytes, RAM after boot, unused by monitor)
0x0080-0x00FF   System Workspace (128 bytes, RAM)
0x0100-0xEEFF   User Program Area (60,928 bytes)
0xEF00-0xEFFF   Monitor Stack Page (stack grows down from 0xF000)
0xF000-0xFFFF   Monitor ROM (4,096 bytes)
```

**Rationale:** Clean separation. Workspace at the bottom, stack page and ROM at the top, everything else is playground. 0x0100 is a nice round start address for user code.

**No vectors or API table in page 0.** Decided 2026-10-02: there is no public API jump table and no RST vector copy. RST 7 gets a single JMP when something needs interrupts (Someday). Programs exit back to the monitor with `RET` (see G in QUICK_REFERENCE), not `JMP 0000`.

### Detailed Memory Map

```
+----------------+----------------------------------------------+
| Address Range  | Description                                  |
+----------------+----------------------------------------------+
| 0x0000-0x007F  | Unused (uninitialised RAM after boot)        |
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
| 0x0100-0xEEFF  | USER PROGRAM AREA                            |
+----------------+----------------------------------------------+
| 0xEF00-0xEFFF  | MONITOR STACK PAGE (SP starts at 0xF000)     |
+----------------+----------------------------------------------+
| 0xF000-0xFFFF  | MONITOR ROM                                  |
+----------------+----------------------------------------------+
```

The HEX loader (Phase 5) rejects records that touch page 0x00 or 0xEF00-0xFFFF. The other monitor commands (F, M, E, L) do not guard; writing there clobbers the workspace or the stack.

---

## ROM Overlay Boot Mechanism

### The Problem

The 8080 starts execution at 0x0000 on reset. Our ROM lives at 0xF000, and RAM is undefined at power-on.

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
| OUT 0xFE (any value) | Overlay disabled (RAM at 0x0000) |

Decided 2026-10-02: any write to 0xFE disables the overlay; there is no software cold reset (use the reset line). The emulator currently acts only on 0x00 (disable) and 0xFF (cold reset); change pending in TODO.md. The ROM writes 0x00, which works under both.

**Writes during overlay:** decided 2026-10-02 to write through to the RAM underneath (ROM is decoded on MEMR only). The emulator currently drops them (`src/cpu.rs` `write_byte`); change pending. The ROM never writes low RAM before disabling the overlay, so behavior is identical either way.

### Hardware Implementation

For future physical build:
- 74LS74 flip-flop controls overlay state
- Set on reset (overlay enabled)
- Cleared by any write to port 0xFE (no data decode)
- Address decode: when set, MEMR to 0x0000-0x0FFF selects ROM; MEMW always goes to RAM
- IN 0xFF: bit 0 = overlay flip-flop; bits 1-7 undefined

This is how real S-100 systems solved the boot problem. We're using a proven pattern.

---

## Boot Sequence

### Power-On Flow

```
1. RESET
   └─> Overlay enabled, PC = 0x0000

2. CPU executes from 0x0000 (reads ROM via overlay)
   └─> LXI SP, F000h
   └─> DI
   └─> JMP BOOT_CONTINUE  ; Jump to F000+ address space

3. Now executing from 0xF000+ range
   └─> OUT 0FEh, 00h      ; Disable overlay
   └─> 0x0000-0x0FFF is now RAM

4. Initialize workspace (LAST_DUMP_ADDR, LAST_EXAM_ADDR) and I/O stubs

5. Print banner, enter MAIN_LOOP (interrupts stay disabled)
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

        ; Initialize workspace and I/O stubs
        ; ...

        CALL    PRINT_BANNER
        ; falls through to MAIN_LOOP (interrupts stay disabled)
```

**Critical:** The `JMP BOOT_CONTINUE` escapes the overlay region *before* disabling it. Without this, disabling overlay would cause PC to read garbage RAM.

---

## I/O Port Map

### Port Allocation

| Range | Device | Status |
|-------|--------|--------|
| 0x00-0x02 | Console | ✅ Implemented |
| 0x03 | Console Control | Reserved |
| 0x04-0x07 | (Parallel I/O) | Reserved |
| 0x08-0x0C | Storage Device (24-bit) | ✅ Done |
| 0x0D-0x0F | Storage Mount | ✅ Done |
| 0x10-0x13 | Service Mailbox (Pi services: TIME, GET, ASK, ASM, DIS) | Future |
| 0x14-0x6F | (Pi window, unassigned) | - |
| 0x70-0xFD | (Expansion: local chips) | Available |
| 0xFE | System Control | ✅ Implemented |
| 0xFF | System Status | ✅ Implemented |

Decided 2026-10-02:
- **Pi window.** Ports 0x08-0x6F belong to the Pi coprocessor. 0x00-0x07 is the console chip, 0x70-0xFD local chips, 0xFE-0xFF glue logic.
- **One mailbox for all Pi services.** HTTP, Claude, time, assembler and disassembler are text commands through one Service Mailbox (DEVICE_SPECS.md), not per-device register maps. Large results land in storage files.
- **Pi ports use a READY wait-state.** Any I/O to a Pi port holds READY low until the Pi releases it, so protocols stay instant-response from the 8080's point of view.

The emulator also intercepts ports 0x30-0x32 for an interim timer (`src/cpu.rs`). Decided 2026-10-02 to delete it; pending in TODO.md.

---

## RST Vectors

None installed. Interrupts stay disabled. When something needs a periodic interrupt (Someday), the ROM writes a single `JMP` at 0x0038 for RST 7.

---

## ROM Organization

Addresses from the current build (2511 of 4096 bytes used):

```
F000: COLD_START, BOOT_CONTINUE (overlay off, workspace/stub init)
F034: MAIN_LOOP (prompt, READ_LINE, CPI/JZ dispatch)
F0A8: Console I/O (CONOUT, CONIN, CONST), print routines
F10D: Input/parse (READ_LINE, SKIP_SPACES, READ_HEX_WORD, TO_HEX_DIGIT, READ_HEX_ADDR24)
F217: Commands C D E F G H I M O S ?, READ_EXAM_BYTE
F592: Storage commands X, L, W
F6CB: Strings (banner, help, errors)
F9CF-FFFF: Free (0xFF fill)
```
