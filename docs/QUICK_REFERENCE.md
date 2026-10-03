# Quick Reference

Cheat sheet. Not normative. Every section links to the doc that owns the detail, and where this page and that doc disagree, the doc wins:

- [ARCHITECTURE.md](ARCHITECTURE.md): memory map, workspace, boot, overlay, CPU contract, hardware interface, host-side keys.
- [DEVICE_SPECS.md](DEVICE_SPECS.md): every port protocol and the READY contract.
- [MONITOR_SPEC.md](MONITOR_SPEC.md): line input, argument grammar, commands, messages, HEX loader, `G` return.

This page shows decided behavior. Where the code does not match yet, the change is pending in `TODO.md`.

---

## Monitor Commands

Detail: [MONITOR_SPEC.md](MONITOR_SPEC.md) (Commands, Intel HEX Loader).

| Cmd | Syntax | Description |
|-----|--------|-------------|
| C | `C start end dest` | Compare memory; prints each mismatch |
| D | `D [start [end]]` | Dump memory; default 128 bytes from the last dump address |
| E | `E [addr]` | Examine/modify; `CR` stores and advances, `.` exits |
| F | `F start end val` | Fill memory |
| G | `G [addr]` | Go; bare `G` = 0100. Program returns with `RET` (G pushes WARM) |
| H | `H num1 num2` | Hex math: prints sum and difference |
| I | `I port` | Input from port |
| L | `L stor mem [cnt]` | Load from storage (24-bit `stor`); `cnt` default 0100 |
| M | `M src dst cnt` | Move memory; overlap-safe (memmove) |
| O | `O port val` | Output to port |
| S | `S start end b1 [.. b8]` | Search for 1-8 bytes |
| W | `W mem stor [cnt]` | Write to storage (24-bit `stor`), then flush; `cnt` default 0100 |
| X | `X [file \| -]` | `X` query, `X name` mount, `X -` unmount |
| ? | `?` | Help |
| : | `:LLAAAATT..CC` | Intel HEX record, auto-detected at the prompt (Phase 5) |

### Argument rules

- Ranges are inclusive. `end < start` prints `Invalid range`.
- Words take 1-4 hex digits, storage addresses 1-6. More digits is an error, never a truncation.
- Byte arguments parse as a word and must be at most FF (`00AA` is fine, `1AA` is an error).
- L/W count 0 or M count 0 prints `Invalid range`.
- Tokens after the last argument are ignored.
- A command that reports an argument error writes no memory outside the workspace and no I/O port.

### HEX loader

- Accepts types 00 (data) and 01 (EOF, prints `Loaded`). Max 34 data bytes per record.
- Rejects any record that writes outside 0100-EEFF.
- Validates the whole record (length, syntax, checksum, type, range) before writing a byte.

### Future commands

| Cmd | Phase | Description |
|-----|-------|-------------|
| T | 6 | Show time (mailbox `TIME`) |
| A | 7 | Assemble (mailbox `ASM`) |
| U | 7 | Unassemble (mailbox `DIS`) |
| N | 8 | HTTP GET (mailbox `GET`) |
| Q | 9 | Ask Claude (mailbox `ASK`) |
| R | 10 | Registers |

Until a letter ships it prints `Unknown command. Type ? for help.`

---

## Host Keys (Emulator Only)

Detail: [ARCHITECTURE.md](ARCHITECTURE.md) (Host-Side Conveniences).

| Key | Effect |
|-----|--------|
| Ctrl-C | Quits the emulator. Never reaches the 8080 |
| Ctrl-E | Stops the CPU and opens the `dbg>` prompt (ARCHITECTURE 7.4). Never reaches the 8080 |
| Enter / Backspace / Tab / Esc | 0D / 08 / 09 / 1B |
| Other Ctrl-A..Ctrl-Z | 01..1A |
| Printable ASCII | Its byte |
| Non-ASCII | UTF-8 bytes in order |
| Arrows, F-keys, other | Dropped |

---

## Port Map

Detail: [DEVICE_SPECS.md](DEVICE_SPECS.md).

| Port | Device | Status |
|------|--------|--------|
| 00-02 | Console (Pi FIFO) | Implemented |
| 03-07 | Unassigned (Pi window) | - |
| 08-0C | Storage | Implemented |
| 0D-0F | Storage mount | Implemented |
| 10-13 | Service Mailbox | Phase 6 |
| 14-6F | Unassigned (Pi window) | - |
| 70-FD | Unmapped (local, none fitted) | - |
| FE | System control (W) | Implemented |
| FF | System status (R) | Implemented |

- **Pi window = 00-6F.** Every access waits on READY until the Pi completes it. No timeout. No byte-level busy polling.
- Unassigned Pi-window ports: `IN` returns FF, `OUT` is ignored.
- `IN` from 70-FD is undefined on hardware; the emulator returns FF. Software must not depend on it.
- **RESET** returns every Pi device to its power-on state (RAM excepted).

### Console (00-02)

| Port | Dir | Function |
|------|-----|----------|
| 00 | W | Byte to terminal. Never waits; discarded if no terminal or buffer full |
| 01 | R | Pop input FIFO (00 if empty) |
| 02 | R | Status: bit 0 = RX ready, bit 1 = TX ready (always 1) |

### Storage (08-0C)

| Port | Dir | Function |
|------|-----|----------|
| 08 / 09 / 0A | R/W | Address bits 0-7 / 8-15 / 16-23 |
| 0B | R/W | Data; every access advances the address, mounted or not |
| 0C | R | Status: bit 0 mounted, bit 1 ready (always 1), bit 7 EOF |
| 0C | W | Control: 00 address = 0, 01 address - 1, 02 flush (fsync) |

- Status examples: 82 not mounted, 03 mounted inside the file, 83 mounted at/past EOF.
- Past-EOF read returns FF. Unmounted read returns FF, write is discarded.
- A host I/O error on a data read, a data write or a flush unmounts the file.

### Mount (0D-0F)

| Port | Dir | Function |
|------|-----|----------|
| 0D | W | Append filename character |
| 0E | W | 01 mount, 02 unmount, 03 query. Any write clears the filename buffer |
| 0F | R | 00 ok / mounted, 01 open failed / not mounted, 02 invalid name. 01 at power-on |

- Names: 1-12 of `A-Z 0-9 . - _`; the device uppercases. More than 12 gives 02.
- Mount creates a missing file. A failed mount leaves nothing mounted. Files over 16 MB fail with 01.

### Service Mailbox (10-13, Phase 6)

| Port | Dir | Function |
|------|-----|----------|
| 10 | W | Append command byte (128-byte buffer) |
| 11 | W | 01 execute (aborts any running request), 02 clear |
| 12 | R | 00 idle, 01 busy, 02 byte available, 03 done, 80-FF error |
| 13 | R | Pop response byte (00 with no side effect outside "available") |

- Errors: 80 unknown/empty command, 81 buffer overflow, 82 bad args, 83 service failed.
- `TIME` returns 19 bytes, `YYYY-MM-DD HH:MM:SS`, Pi local time, no line ending.

### System Control (FE-FF)

| Port | Dir | Function |
|------|-----|----------|
| FE | W | Any value disables the ROM overlay. Only RESET re-enables it |
| FF | R | Bit 0 = overlay enabled. Mask bits 1-7 |

No software reset, no halt command.

---

## Memory Map

Detail: [ARCHITECTURE.md](ARCHITECTURE.md) (Memory Map).

| Range | Contents |
|-------|----------|
| 0000-007F | Unused. No RST vectors, no API table |
| 0080-00FF | Monitor workspace |
| 0100-EEFF | User programs |
| EF00-EFFF | Monitor stack page (SP = F000) |
| F000-FFFF | ROM (4 KB) |

User programs do their own I/O through the ports. ROM routine addresses are not an API.

---

## Workspace Layout

Detail: [ARCHITECTURE.md](ARCHITECTURE.md) (Workspace Layout).

| Address | Size | Purpose |
|---------|------|---------|
| 0080-00CF | 80 | LINE_BUFFER (79 chars + NUL) |
| 00D0-00D1 | 2 | Free (unused `BUFFER_PTR` equate, `rom/monitor.asm:52`, deletion pending, TODO.md) |
| 00D2-00D3 | 2 | LAST_DUMP_ADDR |
| 00D4-00D5 | 2 | LAST_EXAM_ADDR |
| 00D6-00D8 | 3 | IO_IN_STUB (`IN pp` / `RET`) |
| 00D9-00DB | 3 | IO_OUT_STUB (`OUT pp` / `RET`) |
| 00DC-00E3 | 8 | SEARCH_PATTERN |
| 00E4 | 1 | SEARCH_LENGTH |
| 00E5-00E6 | 2 | SEARCH_END |
| 00E7-00E9 | 3 | STOR_ADDR (24-bit: lo, mid, hi) |
| 00EA-00FF | 22 | Free |

---

## Boot Sequence

Detail: [ARCHITECTURE.md](ARCHITECTURE.md) (Reset and Boot, ROM Overlay).

```
1. RESET: PC=0000, overlay on (ROM mirrored at 0000-0FFF for reads), Pi devices at power-on state
2. LXI SP,F000 / DI / JMP to absolute F0xx
3. OUT FE -> overlay off
4. Init workspace + I/O stubs, print banner
5. WARM: SP=F000 -> MAIN_LOOP
```

---

## Flags Register

Detail: [ARCHITECTURE.md](ARCHITECTURE.md) (CPU Behavioral Contract).

```
bit  7  6  5  4   3  2  1  0
     S  Z  0  AC  0  P  1  CY
```
