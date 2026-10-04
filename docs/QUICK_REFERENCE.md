# Quick Reference

Cheat sheet. Not normative. How to operate the machine (running, loading, saving, debugging): [USER_GUIDE.md](USER_GUIDE.md).

Every section links to the doc that owns the detail, and where this page and that doc disagree, the doc wins:

- [ARCHITECTURE.md](ARCHITECTURE.md): memory map, workspace, boot, overlay, CPU contract, hardware interface, host-side keys.
- [DEVICE_SPECS.md](DEVICE_SPECS.md): every port protocol and the READY contract.
- [MONITOR_SPEC.md](MONITOR_SPEC.md): line input, argument grammar, commands, messages, HEX loader, `G` return.
- [PI_DAEMON.md](PI_DAEMON.md): the Pi software behind the ports (`pi8080d`): bus loop, RESET, TCP console, build, deployment.

This page shows decided behavior. Where the code does not match yet, the change is pending in `TODO.md`.

---

## Monitor Commands

Detail: [MONITOR_SPEC.md](MONITOR_SPEC.md) (Commands, Intel HEX Loader).

| Cmd | Syntax | Description |
|-----|--------|-------------|
| A | `A addr` | Assemble, one line per prompt; only `.` ends (mailbox `ASM`) |
| C | `C start end dest` | Compare memory; prints each mismatch |
| D | `D [start [end]]` | Dump memory; default 128 bytes from the last dump address |
| E | `E [addr]` | Examine/modify; `CR` stores and advances, `.` exits |
| F | `F start end val` | Fill memory |
| G | `G [addr]` | Go; bare `G` = 0100. Program returns with `RET` (G pushes G_RETURN, which saves the registers for R); plant F7 (`RST 6`) to stop with `BRK aaaa` (MONITOR_SPEC 8.1) |
| H | `H num1 num2` | Hex math: prints sum and difference |
| I | `I port` | Input from port |
| L | `L stor mem [cnt]` | Load from storage (24-bit `stor`); `cnt` default 0100 |
| M | `M src dst cnt` | Move memory; overlap-safe (memmove) |
| N | `N url [> file]` | HTTP GET (to the console, or to a storage file); `Service error` on any failure (mailbox `GET`); Esc aborts (`Aborted`) |
| O | `O port val` | Output to port |
| Q | `Q text` | Ask Claude; the answer in plain ASCII lines of at most 79 characters; `Service error` on any failure (mailbox `ASK`); Esc aborts (`Aborted`) |
| R | `R` | Registers A, F, BC, DE, HL saved at the last `G` return (`RET`) or `RST 6` break |
| S | `S start end b1 [.. b8]` | Search for 1-8 bytes |
| T | `T` | Show time (mailbox `TIME`); `Service error` on a mailbox failure |
| U | `U addr [cnt]` | Unassemble `cnt` instructions, default 8 (mailbox `DIS`) |
| W | `W mem stor [cnt]` | Write to storage (24-bit `stor`), then flush; `cnt` default 0100 |
| X | `X [file \| -]` | `X` query, `X name` mount, `X -` unmount |
| ? | `?` | Help |
| : | `:LLAAAATT..CC` | Intel HEX record, auto-detected at the prompt |

### Argument rules

- Ranges are inclusive. `end < start` prints `Invalid range`.
- Words take 1-4 hex digits, storage addresses 1-6. More digits is an error, never a truncation.
- Byte arguments parse as a word and must be at most FF (`00AA` is fine, `1AA` is an error).
- L/W/M/U count 0 prints `Invalid range`.
- Tokens after the last argument are ignored.
- A command that reports an argument error writes no memory outside the workspace and no I/O port.

### HEX loader

- Accepts types 00 (data) and 01 (EOF, prints `Loaded`). Max 34 data bytes per record.
- Rejects any record that writes outside 0100-EEFF (0100-CFFF in the RAM test build).
- Validates the whole record (length, syntax, checksum, type, range) before writing a byte.

### Future commands

None. Each new command gets its own `MONITOR_SPEC.md` 6.x section in the phase that adds it. Until a letter ships it prints `Unknown command. Type ? for help.`

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
| 10-13 | Service Mailbox | Implemented |
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

### Service Mailbox (10-13)

| Port | Dir | Function |
|------|-----|----------|
| 10 | W | Append command byte (128-byte buffer) |
| 11 | W | 01 execute (aborts any running request), 02 clear |
| 12 | R | 00 idle, 01 busy, 02 byte available, 03 done, 80-FF error |
| 13 | R | Pop response byte (00 with no side effect outside "available") |

- Errors: 80 unknown/empty command, 81 buffer overflow, 82 bad args, 83 service failed.
- `TIME` returns 19 bytes, `YYYY-MM-DD HH:MM:SS`, Pi local time, no line ending.
- `ASM` returns 1-3 binary bytes of machine code. `DIS` returns a length byte, the instruction line and CR LF. Both give 82 on a bad argument, never 83 (DEVICE_SPECS 8).
- `GET url` streams the body unchanged (BUSY between bytes); `GET url > FILE` writes it to `FILE` in the storage directory and returns the length as 6 hex digits. 83 on a network failure, an HTTP status of 400+, more than 5 redirects, a 10 s connect or a 30 s stall (DEVICE_SPECS 8, GET).
- `ASK prompt` streams Claude's reply as ASCII lines of at most 79 characters, CR LF between them and none after the last. 82 for an empty prompt or a byte outside 20-7E; 83 with no API key on the Pi, on a network or API failure, a refusal, or after 120 s (DEVICE_SPECS 8, ASK).

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
| 0000-007F | Unused except 0030-0032, the RST 6 break vector G writes. No API table |
| 0080-00FF | Monitor workspace |
| 0100-EEFF | User programs |
| EF00-EFFF | Monitor stack page (SP = F000) |
| F000-FFFF | ROM (4 KB). Writes do nothing unless jumper JP-WE is fitted (never in normal use; a burn with `examples/burn`, USER_GUIDE 10) |

User programs do their own I/O through the ports. ROM routine addresses are not an API.

**RAM test build** (ARCHITECTURE 2.1): `rom/monitor_ram.hex`, the monitor at D000 for testing ROM changes on the board without a burn. Paste it at the resident prompt, then `G D000` (banner ends ` RAM`); `G F000` goes back. While it runs, D000-EEFF is its own: the HEX loader takes 0100-CFFF, and F, M and L refuse the image.

**`pi8080d --sim rom/monitor.bin --storage DIR`** (PI_DAEMON 16): the Pi daemon with the CPU model on a simulated board, on a Pi or the Mac, before the board exists.

---

## Workspace Layout

Detail: [ARCHITECTURE.md](ARCHITECTURE.md) (Workspace Layout).

| Address | Size | Purpose |
|---------|------|---------|
| 0080-00CF | 80 | LINE_BUFFER (79 chars + NUL) |
| 00D0-00D1 | 2 | Free |
| 00D2-00D3 | 2 | LAST_DUMP_ADDR |
| 00D4-00D5 | 2 | LAST_EXAM_ADDR |
| 00D6-00D8 | 3 | IO_IN_STUB (`IN pp` / `RET`) |
| 00D9-00DB | 3 | IO_OUT_STUB (`OUT pp` / `RET`) |
| 00DC-00E3 | 8 | SEARCH_PATTERN |
| 00E4 | 1 | SEARCH_LENGTH |
| 00E5-00E6 | 2 | SEARCH_END |
| 00E7-00E9 | 3 | STOR_ADDR (24-bit: lo, mid, hi) |
| 00EA-00F1 | 8 | REGS |
| 00F2-00FF | 14 | Free |

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
