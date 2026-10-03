# Monitor Specification

Normative. This is the contract for everything the monitor ROM shows the user: the banner, the prompt, line input, the argument grammar, every command, the messages, the Intel HEX loader, the `G` return contract and the ROM routine contracts. It replaces the deleted `MONITOR_IMPLEMENTATION_STATUS.md`.

**Scope.** Monitor ROM v0.5 (v0.3 plus Phase 5, the HEX loader, and Phase 6, the `T` command) and the 2026-10-02 decisions. Phases 7-10 get one-line placeholders (Future Commands).

**Elsewhere (one fact, one home):**
- `ARCHITECTURE.md`: the memory map, workspace layout, stack page, reset and boot sequence, WARM entry code, the ROM overlay, the hardware interface (READY, the Pi window), and Host-Side Conveniences (the host key map, Ctrl-C quit, the Ctrl-E debugger hotkey).
- `DEVICE_SPECS.md`: every port protocol (console, storage, mount, Service Mailbox, system control) and the READY contract as software sees it.
- `TODO.md`: the work queue and every known difference between the code and this spec.

**Status of this spec.** All decisions are made (2026-10-02; COLLABORATION_LOG Key Decisions). Where the code differs from this spec, the code is wrong, and the difference goes in `TODO.md`. Sections 1-6, 8, 9 and 11 are implemented (2026-10-03), and section 7 (the HEX loader, Phase 5, v0.4) too. 6.15 (`T`, Phase 6, v0.5) is implemented (2026-10-03).

---

## 0. Conventions

- MUST and MUST NOT are requirements. Everything else is descriptive.
- `<CR>` = 0Dh, `<LF>` = 0Ah, `<BS>` = 08h, `<DEL>` = 7Fh, `<SP>` = 20h, `<NUL>` = 00h.
- "Prints `X`" means the bytes of `X` followed by `<CR><LF>`. Every message in this spec ends with `<CR><LF>`.
- All numeric output is uppercase hex and zero-padded: bytes take 2 digits, addresses and words take 4. There is no `h` suffix and no `0x` prefix.
- Addresses are 16-bit. "Wraps" means modulo 10000h, so FFFF + 1 = 0000.
- The ROM relies on nothing that exists only in the emulator. Every behavior in this spec MUST work unchanged on a real 8080 with the Pi behind the I/O ports.

---

## 1. Banner and Prompt

### 1.1 Banner

The banner is printed once, after cold start (`ARCHITECTURE.md`, Boot Sequence):

```
<CR><LF>
8080 Monitor v<version><CR><LF>
Built: <date> <time><CR><LF>
Ready.<CR><LF>
```

- `<version>` is `0.5` today (`MSG_BANNER` in `rom/monitor.asm`; Phase 6). A phase that changes the command set bumps it in the same commit.
- `<date>` and `<time>` are the assembler's `DATE` and `TIME` at build time.
- Tests MUST NOT match on the version, date or time. A banner check matches only `8080 Monitor v`.
- **Input across reset.** RESET empties the console input FIFO (`DEVICE_SPECS.md`, Rules Common to All Ports), so the monitor never sees bytes typed before a reset. Bytes that arrive after the reset are ordinary input and are processed after the banner. The ROM has no input-drain loop.

### 1.2 Prompt

The prompt is the two bytes `> ` (3Eh 20h). No `<CR><LF>` comes before it. Every command's output ends with `<CR><LF>`, so the prompt starts a new line. The exception is a program returning through `G` (G Return Contract): its last output may leave the cursor mid-line.

---

## 2. Line Input (READ_LINE)

The monitor reads one line at a time into LINE_BUFFER (`ARCHITECTURE.md`, Workspace Layout). The buffer holds 80 bytes: **79 characters plus a terminating `<NUL>`**.

| Byte received | Action | Echo |
|---|---|---|
| `<CR>` (0Dh) | End of line. Store `<NUL>`. | `<CR><LF>` |
| `<LF>` (0Ah) | End of line. Store `<NUL>`. | `<CR><LF>` |
| `<BS>` (08h) or `<DEL>` (7Fh) | If the line is non-empty, remove the last character. If it is empty, ignore the byte. | `<BS><SP><BS>` when a character was removed, nothing otherwise |
| 20h-7Eh, 80h-FFh | If fewer than 79 characters are stored, append the byte. Otherwise discard it. | The byte itself when stored, nothing when discarded |
| Any other byte below 20h (NUL, Tab, Esc, 03h, 05h, ...) | Ignored | Nothing |

Requirements:
- `<DEL>` MUST behave exactly like `<BS>`.
- Characters after the 79th are discarded silently. There is no bell and no overflow message, and the truncated line is processed as it stands.
- READ_LINE does not fold case. Case folding happens per field (Command Dispatch, Argument Grammar).
- **CR and LF.** `<CR>` and `<LF>` each end a line. READ_LINE keeps no state between lines, so a `<CR><LF>` pair produces the line plus one empty line, and the empty line re-prompts (Command Dispatch). A pasted file with CRLF line ends therefore shows an extra `> ` after each line.
- **Host-reserved keys.** In the emulator the host consumes Ctrl-C (quit) and Ctrl-E (debugger hotkey), maps Enter to `<CR>`, and maps the other keys per the key map in `ARCHITECTURE.md` (Host-Side Conveniences). On hardware the Pi delivers every byte, and READ_LINE and E ignore 03h and 05h like any other control byte. No monitor command can be aborted from the keyboard. Reset is the only way out of a running command or a `G` program.
- READ_LINE MUST NOT depend on input timing. Nothing paces the sender: the console is a Pi FIFO behind READY (`DEVICE_SPECS.md`, Console), so a paste of any length arrives intact.

---

## 3. Command Dispatch

After READ_LINE returns:

1. Leading spaces are skipped.
2. An empty line (nothing, or only spaces) prints the prompt again, with no message.
3. The first non-space character selects the command. `a`-`z` fold to `A`-`Z`.
4. `:` selects the Intel HEX loader (Intel HEX Loader). It is not a command letter. Leading spaces before `:` are allowed.
5. The recognized characters are `C D E F G H I L M O S T W X ?` and `:`. `T` came with Phase 6.
6. Anything else prints `Unknown command. Type ? for help.` This includes the letters reserved for later phases (Future Commands) until they are implemented.

Arguments start right after the command character. The space between the letter and the first argument is optional, so `D0200` is the same as `D 0200`.

---

## 4. Argument Grammar

### 4.1 Tokens

- Arguments are separated by one or more spaces (20h). There is no other separator.
- An argument token is a maximal run of non-space characters.
- A numeric token MUST consist only of hex digits (`0-9 A-F a-f`) and MUST end at a space or at the end of the line. Any other character in the token makes the token invalid. `G 01ZZ`, `E 02ZZ`, `D 0200,0300` and `F 0200 02G0 AA` are all errors.
- An optional argument is absent only when skipping spaces reaches the end of the line. An optional argument that is present but invalid is an error. It is never treated as absent.
- Tokens after the last argument a command takes are ignored. `D 0200 0210 junk` dumps 0200-0210.

### 4.2 Numeric forms

| Form | Digits | Value range | Error |
|---|---|---|---|
| word (address, count) | 1-4 | 0000-FFFF | 5 or more digits |
| addr24 (storage address) | 1-6 | 000000-FFFFFF | 7 or more digits |
| byte (fill value, port, I/O value, search byte) | 1-4, parsed as a word | 00-FF | 5 or more digits, or a value above FF. Leading zeros are allowed: `00AA` = AA, `1AA` is an error |

Silent truncation is forbidden. `F 10200 1020F 1AA` is an error, not a fill of 0200-020F with AA.

### 4.3 Ranges and counts

- **Ranges** (`start end`) are inclusive. `end < start` is an error and prints `Invalid range`. `end = start` covers one byte.
- **Counts** for L and W run from 0001 to FFFF. An omitted count is 0100 (256). A count of 0 prints `Invalid range`. A present but invalid count is an error (`Invalid hex value`), never the default.
- **Count** for M runs from 0001 to FFFF. A count of 0 prints `Invalid range`.

### 4.4 Validation order and side effects

1. Arguments are parsed left to right. Within an argument, the digit-count check and (for byte forms) the value check are part of parsing it.
2. The first argument that is missing or invalid prints its command's syntax message (Messages), and the command stops.
3. The `Invalid range` checks (end < start, count 0) run only after every argument has parsed. Example: `F 0300 0200 ZZ` prints `Invalid hex value`.
4. A command that reports an argument or syntax error (`Invalid address`, `Invalid hex value`, `Invalid port/value`, `Invalid range`) MUST NOT have written memory outside the workspace and the stack page (EF00-EFFF), and MUST NOT have written any I/O port. L and W read port 0Ch to check "mounted" before they parse (sections 6.8 and 6.12). That is a read, and it is allowed.
5. Device-reported errors (`Storage error`, `Invalid filename`, `Mount failed`) are printed after the port accesses that produced them. Rule 4 does not apply to them.

---

## 5. Messages

These are the exact strings. Each is printed with a trailing `<CR><LF>`.

| String | Printed by |
|---|---|
| `Unknown command. Type ? for help.` | Dispatch: unrecognized first character |
| `Invalid address` | D, E, G: an argument is invalid |
| `Invalid hex value` | C, F, H, M, S, L, W: an argument is missing or invalid |
| `Invalid port/value` | I, O: an argument is missing, invalid, or above FF |
| `Invalid range` | C, D (two arguments), F, S: end < start. L, W, M: count 0 |
| `No storage mounted` | L, W: storage status bit 0 = 0 before parsing. X query: not mounted |
| `Storage error` | L, W: storage status bit 0 = 0 after the transfer (W: after the flush) |
| `Mounted` | X: the mount succeeded, or the query reports mounted |
| `Unmounted` | `X -` |
| `Invalid filename` | X: mount status 02h |
| `Mount failed` | X: any other nonzero mount status |
| `Loaded` | L: transfer complete and still mounted. HEX: EOF record (type 01) accepted |
| `Written` | W: transfer and flush complete and still mounted |
| `Record too long` | HEX: LL above 22h (34). Checked before the line length, so it also fires on short junk such as `:23` |
| `Bad record` | HEX: syntax error (non-hex character, short line, trailing characters) |
| `Checksum error` | HEX: checksum mismatch |
| `Bad record type` | HEX: type other than 00 or 01 |
| `Address out of range` | HEX: a type 00 record would write outside 0100-EEFF |
| `Service error` | T (Phase 6): mailbox status 00 after execute, or 80-FF |

- The ROM MUST NOT contain `File not found`. Mount creates missing files, so that message can never be true.
- Strings new since v0.3: `Invalid range`, `Mount failed`, `Storage error`, the five HEX errors, and `Service error` (Phase 6). The HEX EOF record reuses `Loaded`.

---

## 6. Commands

Notation: `[x]` is optional. Numeric forms follow 4.2. "No output" means that only the next prompt follows.

### 6.1 C: Compare

`C start end dest`

- Compares each byte in `start..end` with the byte at the same offset from `dest`.
- For each mismatch, in ascending order, prints `AAAA:XX BBBB:YY`: the first-range address and byte, then the second-range address and byte.
- If the ranges are identical, there is no output.
- `dest` addresses wrap past FFFF. `C 0000 FFFF dest` compares 65536 bytes.
- C's own stack use changes bytes in the stack page (EF00-EFFF) while it runs. A compare whose ranges cover them reports those bytes, as S reports its own pattern copy (6.11).
- Errors: `Invalid hex value` for a missing or invalid argument. `Invalid range` for end < start.

### 6.2 D: Dump

`D [start [end]]`

| Form | Range |
|---|---|
| `D` | From LAST_DUMP_ADDR to min(LAST_DUMP_ADDR + 7Fh, FFFF) |
| `D start` | From start to min(start + 7Fh, FFFF) |
| `D start end` | From start to end |

- LAST_DUMP_ADDR is 0000 after cold start.
- Output is whole 16-byte lines. Line *k* starts at `start + 16k`. Lines are not aligned to 16, and the last line may show bytes past `end`.
- Line format:
  ```
  0200: 41 42 43 44 45 46 47 48  49 4A 4B 4C 4D 4E 4F 50  ABCDEFGHIJKLMNOP
  ```
  Each line is: the address, `:`, a space; 16 × (byte, space), with one extra space after the 8th byte; one more space; 16 characters, where 20h-7Eh print as themselves and every other byte prints as `.`; then `<CR><LF>`.
- Bytes inside a line wrap: a line that starts at FFF8 shows FFF8-FFFF and then 0000-0007.
- **Termination.** After each line, the next line start is `line start + 10h`. Dumping stops when that addition carries past FFFF, or when the next line start is greater than `end`. `D 0000 F000` prints F01h lines (3841). `D 0000 FFFF` prints 1000h lines (4096).
- When the dump finishes, LAST_DUMP_ADDR is set to the next line start (wrapped). After `D FFF8 FFFF` it is 0008.
- Errors: `Invalid address` for an invalid argument. `Invalid range` for end < start. LAST_DUMP_ADDR does not change on an error.

### 6.3 E: Examine and modify

`E [addr]`

- A bare `E` starts at LAST_EXAM_ADDR, which is 0000 after cold start.
- For each address, E prints `AAAA: XX-` (the address, `:`, a space, the current byte, `-`). It then reads keys directly, not through READ_LINE:

| Key | Action | Echo |
|---|---|---|
| Hex digit (`0-9 A-F a-f`), fewer than 2 entered | Append the digit | The key as typed |
| Hex digit, 2 already entered | Ignored | Nothing |
| `<BS>` or `<DEL>` | Remove the last entered digit, if there is one | `<BS><SP><BS>` if a digit was removed |
| `<CR>` | If 1 or 2 digits were entered, store the value at the address. Then advance the address by 1 (FFFF wraps to 0000). | `<CR><LF>`, then the next `AAAA: XX-` line |
| `.` | Exit. Pending digits are discarded, not stored. LAST_EXAM_ADDR is set to the address currently displayed. | `<CR><LF>` (the `.` itself is not echoed) |
| `<LF>` | Ignored | Nothing |
| `-` | Ignored | Nothing |
| Anything else | Ignored | Nothing |

- Two digits do not advance on their own. Only `<CR>` stores and advances, so a CRLF terminal advances exactly once per Enter.
- One digit is a complete value: `5 <CR>` stores 05.
- E does not read the byte back to verify it. Storing to a ROM address (F000-FFFF) has no effect.
- Error: `Invalid address` for an invalid `addr`.

Example: `E 0200`, then `1` `2` `<CR>` `3` `4` `<CR>` `.`, stores 0200=12 and 0201=34 and leaves LAST_EXAM_ADDR = 0202.

### 6.4 F: Fill

`F start end byte`

- Writes `byte` to every address in `start..end`. F never wraps: with `end = FFFF` it stops after FFFF.
- No output.
- F does not guard the workspace (0080-00FF) or the stack page (EF00-EFFF). Filling either has undefined results.
- Errors: `Invalid hex value` for a missing or invalid argument, or for `byte` above FF. `Invalid range` for end < start.

### 6.5 G: Go

`G [addr]`

- A bare `G` jumps to 0100.
- `G addr` jumps to `addr`. Any address is allowed.
- G pushes the WARM address before it jumps. The G Return Contract (section 8) applies.
- Error: `Invalid address` when `addr` is present but invalid. Nothing executes.

### 6.6 H: Hex math

`H a b`

- Prints `SSSS DDDD`, where SSSS = (a + b) mod 10000h and DDDD = (a − b) mod 10000h.
- Example: `H 1234 0111` prints `1345 1123`.
- Error: `Invalid hex value` for a missing or invalid argument.

### 6.7 I: Input

`I port`

- Executes `IN port` and prints the byte read as `XX`.
- The read has the port's side effects (`DEVICE_SPECS.md`, Rules Common to All Ports): `I 01` pops a console byte, `I 0B` advances the storage address, `I 13` pops a mailbox byte.
- Error: `Invalid port/value` when the argument is missing, invalid, or above FF.

### 6.8 L: Load from storage

`L stor mem [count]`

1. Read the storage status (port 0Ch). If bit 0 = 0, print `No storage mounted` and stop. The arguments are not parsed.
2. Parse `stor` (addr24), then `mem` (word), then `count` (word, default 0100). Apply the count check (4.3). No port is written until all three have parsed.
3. Set the storage address to `stor` (ports 08h-0Ah). Then read `count` bytes from the data port (0Bh) into `mem`, `mem+1`, and so on. Memory addresses wrap past FFFF.
4. Read the storage status (port 0Ch). If bit 0 = 1, print `Loaded`. If bit 0 = 0, print `Storage error`: the device unmounted the file after a host I/O error, or the Pi service restarted (`DEVICE_SPECS.md`, Storage).

- Reads past the end of the file return FFh and advance the address (`DEVICE_SPECS.md`, Storage). That is not an error.
- The destination is not guarded.
- Errors: `Invalid hex value` when an argument is invalid or `stor` or `mem` is missing. `Invalid range` for a count of 0.

### 6.9 M: Move

`M src dst count`

- Copies `count` bytes from `src` to `dst`. Addresses wrap past FFFF.
- **Overlap.** After M, `dst..dst+count-1` MUST hold the original contents of `src..src+count-1` for any overlap, as long as neither range wraps past FFFF. The copy runs backward (highest address first) when `dst > src`, and forward otherwise.
- No output.
- Errors: `Invalid hex value` for a missing or invalid argument. `Invalid range` for a count of 0.

### 6.10 O: Output

`O port byte`

- Executes `OUT port` with `byte`. No output.
- `O FE xx` disables the overlay, which is already disabled after boot (`ARCHITECTURE.md`, ROM Overlay).
- Error: `Invalid port/value` when either argument is missing, invalid, or above FF. No port is written on an error.

### 6.11 S: Search

`S start end b1 [b2 ... b8]`

- For every address A in `start..end`, in ascending order, compares the pattern with the bytes at A, A+1, and so on. The pattern may extend past `end`, and pattern bytes past FFFF wrap to 0000. Candidate addresses never wrap: `S 0000 FFFF ...` tests every address exactly once.
- Each match prints `AAAA`.
- If nothing matches, there is no output.
- The pattern has 1 to 8 bytes. Tokens after the 8th byte are ignored (4.1).
- The workspace (0080-00FF) holds LINE_BUFFER and S's own copy of the pattern. A range that covers that copy always reports it as a match.
- Errors: `Invalid hex value` when start or end is missing or invalid, there are no pattern bytes, a pattern token is invalid, or a pattern byte is above FF. `Invalid range` for end < start.

Example: with 0100-EEFF zeroed, `F 0500 0502 77` followed by `S 0100 EEFF 77 77 77` prints exactly `0500`.

### 6.12 W: Write to storage

`W mem stor [count]`

1. Read the storage status (port 0Ch). If bit 0 = 0, print `No storage mounted` and stop.
2. Parse `mem` (word), then `stor` (addr24), then `count` (word, default 0100). Apply the count check (4.3). No port is written until all three have parsed.
3. Set the storage address to `stor`. Write `count` bytes from `mem`, `mem+1`, and so on (wrapping) to the data port. Then write 02h (flush) to port 0Ch.
4. Read the storage status (port 0Ch). If bit 0 = 1, print `Written`. If bit 0 = 0, print `Storage error`: a host I/O error on a write or on the flush unmounted the file, or the Pi service restarted (`DEVICE_SPECS.md`, Storage).

- Errors: as for L.

### 6.13 X: Mount, unmount, query

| Form | Port sequence | Output |
|---|---|---|
| `X` | OUT 0Eh ← 03h; IN 0Fh | 00h: `Mounted`. Anything else: `No storage mounted` |
| `X -` | OUT 0Eh ← 02h | `Unmounted` |
| `X name` | OUT 0Eh ← 03h (query; clears the name buffer); each name character to OUT 0Dh; OUT 0Eh ← 01h; IN 0Fh | 00h: `Mounted`. 02h: `Invalid filename`. Any other nonzero value: `Mount failed` |

- The first non-space character after `X` selects the form. If it is `-`, X unmounts and ignores the rest of the line. A name that starts with `-` cannot be mounted from the monitor.
- The leading `OUT 0Eh ← 03h` discards any partial name left in the device by a program or by `O 0D xx`. Its 0Fh result is overwritten by the mount and is not read.
- The name runs to the next space or the end of the line. Characters are sent as typed. The device uppercases the name, validates it, and clears its name buffer on every 0Eh write (`DEVICE_SPECS.md`, Storage Mount). The ROM sends no terminator.
- Mounting a missing file creates it. There is no "not found".
- X does not print the name of the mounted file.

### 6.14 ?: Help

`?`

Prints this text exactly, each line ending in `<CR><LF>`:

```
Commands:
  C start end dest - Compare memory
  D [start] [end]  - Dump memory
  E [addr]         - Examine/modify
  F start end val  - Fill memory
  G [addr]         - Go (execute)
  H num1 num2      - Hex math (+/-)
  I port           - Input from port
  L stor mem [cnt] - Load from storage
  M src dst cnt    - Move memory
  O port value     - Output to port
  S start end pat  - Search memory
  T                - Show time
  W mem stor [cnt] - Write to storage
  X [file | -]     - Mount/unmount storage
  :LLAAAATT..CC    - Intel HEX record
  ?                - Help
```

- The `:LLAAAATT..CC` line shipped with Phase 5 and the `T` line with Phase 6. Each line ships in the same commit as its feature.
- Arguments after `?` are ignored.

### 6.15 T: Time

`T`

1. Runs the mailbox command `TIME` with the reference client in `DEVICE_SPECS.md` (Service Mailbox): clear (OUT 11h ← 02h), send `T` `I` `M` `E` to OUT 10h, execute (OUT 11h ← 01h), then poll IN 12h.
2. Each response byte read from IN 13h is printed to the console as it arrives.
3. On status 03h (DONE), prints `<CR><LF>`.
4. On status 00h after execute (Pi service restarted) or 80h-FFh, prints `Service error`. Any response bytes already printed stay on the line before it.

- Arguments are ignored (4.1). T sends exactly `TIME`.
- The successful output is one line, `YYYY-MM-DD HH:MM:SS` (the Pi's local time), followed by `<CR><LF>`.
- T uses ports 10h-13h.
- Tests match the shape `NNNN-NN-NN NN:NN:NN` (N = decimal digit), never a value. No injectable clock is needed.

---

## 7. Intel HEX Loader

A line whose first non-space character is `:` is one Intel HEX record. There is no command letter and no loader mode. Each line stands alone, and the monitor keeps no state between records.

### 7.1 Grammar

```
record  = ":" LL AAAA TT data CC <end of stored line>
LL      = byte          ; data length, 00-22h
AAAA    = byte byte     ; load address, high byte first
TT      = byte          ; record type
data    = LL * byte
CC      = byte          ; checksum
byte    = hexdigit hexdigit     ; exactly two, 0-9 A-F a-f
```

- No spaces or other characters may appear inside the record.
- Nothing may follow CC in the stored line. A trailing space is a syntax error.
- The grammar applies to the line as READ_LINE stored it, which is at most 79 characters (section 2):
  - A full 34-byte record is 1 + 2 × (34 + 5) = 79 characters. Leading spaces before `:` are allowed, but they count against the 79. With one leading space, a full-length record loses its last checksum digit and fails with `Bad record`.
  - Characters typed after a full 79-character line are discarded before the loader sees the line. A full-length record followed by trailing characters therefore loads.
  - Control characters never reach the stored line: READ_LINE drops Tab, Esc and every other byte below 20h but CR, LF and BS, and applies BS and DEL (section 2). A record with an embedded Tab, or one corrected with BS, therefore loads.

### 7.2 Validation order

The loader validates the whole record before it writes any byte. The first check that fails prints its message, and the record is discarded. On any failure nothing outside the stack page (EF00-EFFF) is written, not even the workspace.

| Step | Check | Failure message |
|---|---|---|
| 1 | LL is two hex digits | `Bad record` |
| 2 | LL ≤ 22h (34) | `Record too long` |
| 3 | AAAA, TT, the LL data bytes and CC are each two hex digits, and the line ends right after CC | `Bad record` |
| 4 | (LL + AAAA high + AAAA low + TT + all data bytes + CC) mod 100h = 00 | `Checksum error` |
| 5 | TT = 00 or 01. Types 02-05 and every other type fail | `Bad record type` |
| 6 | Type 00 with LL > 0 only: AAAA ≥ 0100h **and** AAAA + LL ≤ EF00h, with the sum computed without wrap (a carry out of bit 15 fails the check) | `Address out of range` |

Because of step 6, a record that is accepted writes only inside 0100-EEFF.

### 7.3 Actions on a valid record

| Record | Action | Output |
|---|---|---|
| Type 00, LL > 0 | Write the data bytes to AAAA, AAAA+1, ... | None |
| Type 00, LL = 0 | Nothing | None |
| Type 01 (EOF) | Nothing. Once the checksum passes, LL, AAAA and any data are ignored | `Loaded` |

- The guard (step 6) applies only to records that write. The standard EOF record `:00000001FF`, at address 0000, is accepted.
- The guard ranges come from the memory map (`ARCHITECTURE.md`, Memory Map): 0000-007F is unused, 0080-00FF is the workspace (including LINE_BUFFER), EF00-EFFF is the monitor stack, and F000-FFFF is ROM. Writes can never reach LINE_BUFFER, so the loader may read data bytes from the buffer while it writes them.
- The loader does not change LAST_DUMP_ADDR or LAST_EXAM_ADDR.

### 7.4 Echo and paste

- READ_LINE echoes the record's characters as they arrive, then `<CR><LF>`.
- A data record that loads prints nothing more. The next prompt follows.
- A pasted file with CRLF line ends shows an extra `> ` after each record (section 2). A bad record in the middle of a paste prints its message, and the records after it are still processed one by one.
- Nothing paces the sender (section 2).

Example (LF-only or CR-only line ends):

```
> :0401000001020304F1
> :0401000001020304F0
Checksum error
> :00000001FF
Loaded
>
```

### 7.5 Conformance vectors

Phase 5 tests MUST cover every row.

| Input line | Expected output | Memory effect |
|---|---|---|
| `:01010000AA54` | none | 0100 = AA |
| `:01EEFF00AA68` | none | EEFF = AA |
| `:0401000001020304f1` (lowercase) | none | 0100-0103 = 01 02 03 04 |
| `   :01010000AA54` (leading spaces) | none | 0100 = AA |
| `:22020000000102030405060708090A0B0C0D0E0F101112131415161718191A1B1C1D1E1F2021AB` (34 bytes, 79 characters) | none | 0200-0221 = 00..21 |
| Same as the previous row, plus a trailing `X` | none | as the previous row (the `X` is discarded by READ_LINE) |
| Same as the 34-byte row, with one leading space | `Bad record` | none |
| `:23020000000102030405060708090A0B0C0D0E0F101112131415161718191A1B1C1D1E1F20212288` (35 bytes) | `Record too long` | none |
| `:0100FF00AA56` | `Address out of range` | none |
| `:01EF0000AA66` | `Address out of range` | none |
| `:11EEF00055555555555555555555555555555555556C` (straddles EEFF/EF00) | `Address out of range` | none |
| `:10FFF80011111111111111111111111111111111E9` (wraps past FFFF) | `Address out of range` | 0000-0007 unchanged |
| `:020000021000EC`, `:0400000300000100F8`, `:020000040000FA`, `:0400000500000100F6` | `Bad record type` (each) | none |
| `:0401000001020304F0` | `Checksum error` | none |
| `:0401000001020G04F1` | `Bad record` | none |
| `:0401000001020304` (no CC) | `Bad record` | none |
| `:0401000001020304F1 ` (trailing space) | `Bad record` | none |
| `:` | `Bad record` | none |
| `:0000000000` (type 00, LL 0, address 0000) | none | none |
| `:00000001FF` | `Loaded` | none |
| `:0101000112EB` (type 01 with an address and data) | `Loaded` | none |
| Three good records, a bad-checksum record, then a good one, all pasted with CRLF | `Checksum error` once; the other four load | as each record specifies |

---

## 8. G Return Contract

This section owns the program-facing contract. The WARM entry code and the stack page are defined in `ARCHITECTURE.md` (Boot Sequence, Memory Map).

**On entry to the program:**
- PC = the target address.
- SP = EFFEh, and the word at EFFE is the WARM address.
- Interrupts are disabled (the ROM never executes `EI`).
- A, the flags, BC, DE and HL are unspecified.
- The overlay is disabled.
- Console input that READ_LINE has not consumed (for example, the `<LF>` of a CRLF pair) is left in the FIFO for the program.
- 0000-007F holds no RST vectors and no API table (`ARCHITECTURE.md`, Memory Map).

**To return:** execute `RET` with SP = EFFEh and the word at EFFE intact. `RET` is the only supported exit. The WARM address is not published.

**WARM** sets SP = F000h and enters MAIN_LOOP, which prints the prompt. It prints no banner and no `<CR><LF>`, and it does not reinitialize the workspace. LAST_DUMP_ADDR and LAST_EXAM_ADDR survive unless the program overwrote them.

---

## 9. ROM Routine Contracts

The header comment above each routine in `rom/monitor.asm` is that routine's contract. Register preservation is descriptive, not an ABI. This spec does not repeat the headers. The rules are:

- The convention is stated once, in the header block of `monitor.asm`: **a register that a routine's header lists neither as an output nor as trashed is preserved.**
- A change to a routine's register behavior MUST update its header in the same commit.
- There is no public API and no jump table. User programs MUST NOT call ROM addresses, because they move between builds.
- READ_HEX_WORD and READ_HEX_ADDR24 implement section 4, and READ_HEX_BYTE (a word whose value is at most FF) builds on READ_HEX_WORD. Their headers MUST state the error cases (no digits, too many digits, a token not ended by a space or NUL) and that they skip leading spaces on entry.
- CMD_COMPARE relies on B surviving PRINT_ADDR, PRINT_HEX_BYTE, PRINT_SPACE, CONOUT and PRINT_CRLF.

---

## 10. Future Commands (Placeholders)

These are placeholders, not designs. Each phase writes its own section here when it starts. Until a letter is implemented, it prints `Unknown command. Type ? for help.`

| Cmd | Phase | Purpose |
|---|---|---|
| A | 7 | Assemble: mailbox `ASM` |
| U | 7 | Unassemble: mailbox `DIS` |
| N | 8 | HTTP GET: mailbox `GET` |
| Q | 9 | Ask Claude: mailbox `ASK` |
| R | 10 | Registers. Blocked on capturing registers at return |

Quitting the emulator (Ctrl-C) and the debugger (Ctrl-E) are host-side, not monitor commands (`ARCHITECTURE.md`, Host-Side Conveniences).

---

## 11. Hardware Constraints on the ROM

- Ports the monitor uses on its own: 00h-02h (console), 08h-0Ch (storage), 0Dh-0Fh (mount), 10h-13h (Service Mailbox, Phase 6 `T`), and FEh (overlay off at boot). I and O can reach any port. The protocols are in `DEVICE_SPECS.md`.
- The ROM does no console chip initialization. The console is a Pi FIFO device behind READY.
- **CONOUT is `OUT 00h` followed by `RET`.** It MUST NOT poll TX-ready: status bit 1 always reads 1, and OUT 00 never waits on the terminal (`DEVICE_SPECS.md`, Console).
- The only polling loops in the ROM wait for a person or a background service, never for a byte transfer: CONIN and E poll RX-ready (port 02h, bit 0), and T polls mailbox status (port 12h). Every other device access assumes an instant answer, which READY provides on hardware.
- The boot's first Pi-window access is the banner's first `OUT 00h`. If the Pi's device service is not running yet, that access waits under READY with no timeout (`DEVICE_SPECS.md`, READY Contract). No ROM code handles the stall.
- The I and O commands run self-modified `IN` and `OUT` stubs in workspace RAM. That works on any 8080 and needs no special hardware.
- `rom/monitor.bin` MUST be at most 4096 bytes and run at F000h.
