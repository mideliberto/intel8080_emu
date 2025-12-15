# Monitor ROM Implementation Status

**Last Updated:** December 2025

## Current Status

### Completed Commands

| Command | Description | Status |
|---------|-------------|--------|
| C start end dest | Compare memory regions | ✅ Done |
| D [start] [end] | Dump memory | ✅ Done |
| E [addr] | Examine/modify memory | ✅ Done |
| F start end val | Fill memory with value | ✅ Done |
| G [addr] | Go (execute at address) | ✅ Done |
| H num1 num2 | Hex math (add/subtract) | ✅ Done |
| I port | Input from I/O port | ✅ Done |
| M src dst cnt | Move memory (forward copy) | ✅ Done |
| O port value | Output to I/O port | ✅ Done |
| ? | Help | ✅ Done |

### Completed Infrastructure

- [x] Cold start and initialization
- [x] Command parser and dispatch
- [x] Console I/O (CONIN, CONOUT, CONST)
- [x] Print routines (string, hex byte, hex word, CRLF)
- [x] Input routines (READ_LINE, READ_HEX_WORD)
- [x] Self-modifying I/O stubs for I/O commands
- [x] Workspace layout (LINE_BUFFER, LAST_DUMP_ADDR, etc.)

### Recent Changes

**December 2025 - Added F, M, C commands:**
- F (Fill): Fills memory range with byte value, handles FFFF wrap-around
- M (Move): Forward-only block copy. Overlapping regions where dest > source produce undefined results (documented limitation, matches DDT behavior)
- C (Compare): Compares two memory regions, shows differences as `addr1:val1 addr2:val2`

**December 2025 - DUMP command edge cases:**
- Fixed infinite loop when dumping to FFFF (D F000 FFFF would wrap from 0000 and never terminate)
- Fixed one-arg overflow: `D FF80` now caps end at FFFF instead of wrapping to 00FF
- Fixed no-arg continuation overflow after dumping high memory

Root cause: 16-bit address arithmetic wraps at FFFF→0000, and INX doesn't set flags. Solution: detect wrap-around (H=00 when D>=F0) and use DAD carry flag to cap computed end addresses.

---

## Remaining Monitor Commands

### Recommended Implementation Order

#### 1. S (Search)
**Complexity:** Moderate  
**Syntax:** `S start end byte [byte...]`  
**Description:** Search memory for byte pattern.

```
> S 0 FFFF C3 00 F0   ; Find all JMP F000 instructions
```

#### 2. R (Registers) - ARCHITECTURAL DECISION REQUIRED
**Complexity:** Requires design thought  
**Syntax:** `R`  
**Description:** Display CPU registers.

**Problem:** Current G command does `PCHL` and never returns. For R to be meaningful, need to decide:

1. How do user programs return to monitor?
   - RST instruction?
   - CALL to fixed address?
   - Breakpoint mechanism?

2. Where is register state saved?
   - Dedicated workspace area
   - Stack frame

3. Should G change to preserve return capability?

**Recommendation:** Defer until core monitor is feature-complete. Then design a proper breakpoint/return mechanism that supports both R and future debugger features.

---

## Architecture Notes

### I/O Stub Mechanism

I and O commands use self-modifying code in RAM because 8080's IN/OUT instructions require literal port numbers (unlike Z80's `IN A,(C)`).

Workspace locations:
```
IO_IN_STUB   EQU 00D6H   ; 3 bytes: IN xx / RET
IO_OUT_STUB  EQU 00D9H   ; 3 bytes: OUT xx / RET
```

Initialized during COLD_START, patched at runtime by I/O commands.

### Workspace Memory Map

```
0080-00CF: LINE_BUFFER (80 bytes)
00D0-00D1: BUFFER_PTR
00D2-00D3: LAST_DUMP_ADDR
00D4-00D5: LAST_EXAM_ADDR
00D6-00D8: IO_IN_STUB
00D9-00DB: IO_OUT_STUB
00DC-00FF: Available for future use
```

### 16-bit Address Boundary Handling

When computing end addresses or iterating through memory:
- DAD sets carry on overflow - use this to cap at FFFF
- INX does NOT set flags - cannot detect wrap directly
- For loops ending at FFFF, detect wrap by checking if high byte went from Fxh to 00h

### Move Command Design Decision

The M command uses forward-only copying. This means overlapping regions where dest > source will produce undefined results. This matches the behavior of CP/M DDT and most 8080-era monitors. The rationale:
- Saves code space
- Simpler implementation
- Most users aren't copying overlapping regions
- Bidirectional copy on 8080 requires painful register juggling

---

## Future Phases (from main spec)

After core monitor commands complete:

- **Disk System:** X (mount), L (list), B (boot)
- **Program Loading:** Intel HEX loader
- **Timing:** Timer device, T command
- **Development Tools:** Assembler/disassembler devices
- **Internet Services:** HTTP, time sync
- **Debugger:** Breakpoints, single-step, trace
