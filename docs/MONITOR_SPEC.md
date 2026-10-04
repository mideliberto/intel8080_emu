# Monitor Specification

Normative. This is the contract for everything the monitor ROM shows the user: the banner, the prompt, line input, the argument grammar, every command, the messages, the Intel HEX loader, the `G` return contract and the ROM routine contracts. It replaces the deleted `MONITOR_IMPLEMENTATION_STATUS.md`.

**Scope.** Monitor ROM v0.9 (v0.3 plus Phase 5, the HEX loader, Phase 6, the `T` command, Phase 7, the `A` and `U` commands, Phase 8, the `N` command, Phase 9, the `Q` command, Phase 10, the R command and the register capture at the G return, and Phase 12, Esc for N and Q and the RST 6 breakpoint, 8.1) and the 2026-10-02 decisions.

**Elsewhere (one fact, one home):**
- `ARCHITECTURE.md`: the memory map, workspace layout, stack page, reset and boot sequence, WARM entry code, the ROM overlay, the hardware interface (READY, the Pi window), and Host-Side Conveniences (the host key map, Ctrl-C quit, the Ctrl-E debugger hotkey).
- `DEVICE_SPECS.md`: every port protocol (console, storage, mount, Service Mailbox, system control) and the READY contract as software sees it.
- `TODO.md`: the work queue and every known difference between the code and this spec.

**Status of this spec.** All decisions are made (2026-10-02, 2026-10-03 and 2026-10-04; COLLABORATION_LOG Key Decisions). Where the code differs from this spec, the code is wrong, and the difference goes in `TODO.md`. Sections 1-6, 8, 9 and 11 are implemented (2026-10-03), and section 7 (the HEX loader, Phase 5, v0.4) too. 6.15 (`T`, Phase 6, v0.5) is implemented (2026-10-03). 6.16 and 6.17 (`A` and `U`, Phase 7, v0.6) and the section 9 mailbox client are implemented (2026-10-03). 6.18 (`N`, Phase 8, v0.7) is specified (2026-10-03); implemented (2026-10-03). 6.19 (`Q`, Phase 9, v0.8) is specified (2026-10-03); implemented (2026-10-03). 6.20 (`R`, Phase 10, v0.9) and the G_RETURN capture in section 8 are implemented (2026-10-03). Esc in 6.18 and 6.19 (Phase 12) is specified (2026-10-04); implemented (2026-10-04). 8.1 (RST 6 breakpoints, Phase 12) is specified (2026-10-04); implemented (2026-10-04).

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

- `<version>` is `0.9` today (`MSG_BANNER` in `rom/monitor.asm`; Phase 10). A phase that changes the command set bumps it in the same commit.
- `1.0` is set by the 1.0 candidate commit (`IMPLEMENTATION_ROADMAP.md`, The End State, Monitor 1.0).
  It changes no command and no message. The release is the tagged commit whose burned image passed,
  not the first image that says `1.0`.
- The RAM test build (`ARCHITECTURE.md` 2.1) prints `8080 Monitor v<version> RAM` on that line; nothing else in the banner differs. A test MAY check the ` RAM` marker, but still MUST NOT match the version, date or time.
- `<date>` and `<time>` are the assembler's `DATE` and `TIME` at build time.
- Tests MUST NOT match on the version, date or time. A banner check matches only `8080 Monitor v`.
- **Input across reset.** RESET empties the console input FIFO (`DEVICE_SPECS.md`, Rules Common to All Ports), so the monitor never sees bytes typed before a reset. Bytes that arrive after the reset are ordinary input and are processed after the banner. The ROM has no input-drain loop.

### 1.2 Prompt

The prompt is the two bytes `> ` (3Eh 20h). No `<CR><LF>` comes before it. Every command's output ends with `<CR><LF>`, so the prompt starts a new line. The exception is a program returning through `G` (G Return Contract): its last output may leave the cursor mid-line. After an `RST 6` (8.1) the `BRK` line may start mid-line, but the prompt still follows its `<CR><LF>`.

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
- **CR and LF.** `<CR>` and `<LF>` each end a line. READ_LINE keeps no state between lines, so a `<CR><LF>` pair produces the line plus one empty line, and the empty line re-prompts (Command Dispatch). A pasted file with CRLF line ends therefore shows an extra `> ` after each line. After `N` or `Q`, the Esc check (6.18) can eat the `<LF>` of a `<CR><LF>` terminal, so that extra `> ` may not appear; whether it does depends on timing.
- **Host-reserved keys.** In the emulator the host consumes Ctrl-C (quit) and Ctrl-E (debugger hotkey), maps Enter to `<CR>`, and maps the other keys per the key map in `ARCHITECTURE.md` (Host-Side Conveniences). On hardware the Pi delivers every byte, and READ_LINE and E ignore 03h and 05h like any other control byte. Esc (1Bh) aborts `N` and `Q` while their request runs (6.18, 6.19). Nothing else is aborted from the keyboard: reset is the only way out of any other running command or a `G` program.
- READ_LINE MUST NOT depend on input timing. Nothing paces the sender: the console is a Pi FIFO behind READY (`DEVICE_SPECS.md`, Console), so a paste of any length arrives intact. **Exception:** type-ahead that arrives while `N` or `Q` has a request running is discarded (6.18, Esc), so a paste or script MUST wait for the prompt after an `N` or `Q` before sending more.

---

## 3. Command Dispatch

After READ_LINE returns:

1. Leading spaces are skipped.
2. An empty line (nothing, or only spaces) prints the prompt again, with no message.
3. The first non-space character selects the command. `a`-`z` fold to `A`-`Z`.
4. `:` selects the Intel HEX loader (Intel HEX Loader). It is not a command letter. Leading spaces before `:` are allowed.
5. The recognized characters are `A C D E F G H I L M N O Q R S T U W X ?` and `:`. `T` came with Phase 6, `A` and `U` with Phase 7, `N` with Phase 8, `Q` with Phase 9, and `R` with Phase 10.
6. Anything else prints `Unknown command. Type ? for help.`

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
- **Count** for U is a number of instructions, 0001 to FFFF. An omitted count is 0008. A count of 0 prints `Invalid range`. A present but invalid count is an error (`Invalid hex value`), never the default.

### 4.4 Validation order and side effects

1. Arguments are parsed left to right. Within an argument, the digit-count check and (for byte forms) the value check are part of parsing it.
2. The first argument that is missing or invalid prints its command's syntax message (Messages), and the command stops.
3. The `Invalid range` checks (end < start, count 0) run only after every argument has parsed. Example: `F 0300 0200 ZZ` prints `Invalid hex value`.
4. A command that reports an argument or syntax error (`Invalid address`, `Invalid hex value`, `Invalid port/value`, `Invalid range`) MUST NOT have written memory outside the workspace and the stack page (EF00-EFFF), and MUST NOT have written any I/O port. L and W read port 0Ch to check "mounted" before they parse (sections 6.8 and 6.12). That is a read, and it is allowed.
5. Device-reported errors (`Storage error`, `Invalid filename`, `Mount failed`, `Service error`, `Invalid instruction`) are printed after the port accesses that produced them. Rule 4 does not apply to them.

---

## 5. Messages

These are the exact strings. Each is printed with a trailing `<CR><LF>`.

| String | Printed by |
|---|---|
| `Unknown command. Type ? for help.` | Dispatch: unrecognized first character |
| `Invalid address` | D, E, G: an argument is invalid. A, U: `addr` missing or invalid |
| `Invalid hex value` | C, F, H, M, S, L, W: an argument is missing or invalid. U: `count` invalid |
| `Invalid port/value` | I, O: an argument is missing, invalid, or above FF |
| `Invalid range` | C, D (two arguments), F, S: end < start. L, W, M, U: count 0 |
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
| `Service error` | T, U, N, Q: mailbox status 00 after execute, or 80-FF (for N and Q, 82 too: a bad URL or file name, or an empty or invalid question). U: also DONE before the length byte. A: status 00 after execute, or 80-FF except 82; A then prompts the same address again |
| `Invalid instruction` | A (Phase 7): mailbox status 82, so the line does not assemble; A prompts the same address again |
| `Aborted` | N, Q: Esc while the request runs (6.18). Bytes already printed stay on their line, with no `<CR><LF>` before the message, as `Service error` |
| `BRK aaaa` | A program started by `G` executed `RST 6` at aaaa (8.1). Not preceded by `<CR><LF>`: it starts where the program left the cursor |

- The ROM MUST NOT contain `File not found`. Mount creates missing files, so that message can never be true.
- Strings new since v0.3: `Invalid range`, `Mount failed`, `Storage error`, the five HEX errors, `Service error` (Phase 6), `Invalid instruction` (Phase 7), and `Aborted` and `BRK` (Phase 12). The HEX EOF record reuses `Loaded`.

---

## 6. Commands

Notation: `[x]` is optional. Numeric forms follow 4.2. "No output" means that only the next prompt follows.

These are the ROM's commands. The RAM test build (`ARCHITECTURE.md` 2.1) differs only where 2.1 says: its banner, its HEX guard top, and an F/M/L guard on its own image.

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
- E does not read the byte back to verify it. Storing to a ROM address (F000-FFFF) has no effect (JP-WE open, `ARCHITECTURE.md` 6.10).
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
- G pushes the return address (G_RETURN, section 8) before it jumps. The G Return Contract (section 8) applies.
- Before it pushes G_RETURN, G writes `JMP BRK_ENTRY` at 0030-0032 (8.1). A `G` whose argument fails to parse writes nothing.
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
  A addr           - Assemble
  C start end dest - Compare memory
  D [start] [end]  - Dump memory
  E [addr]         - Examine/modify
  F start end val  - Fill memory
  G [addr]         - Go (execute)
  H num1 num2      - Hex math (+/-)
  I port           - Input from port
  L stor mem [cnt] - Load from storage
  M src dst cnt    - Move memory
  N url [> file]   - HTTP GET
  O port value     - Output to port
  Q text           - Ask Claude
  R                - Registers
  S start end pat  - Search memory
  T                - Show time
  U addr [cnt]     - Unassemble
  W mem stor [cnt] - Write to storage
  X [file | -]     - Mount/unmount storage
  :LLAAAATT..CC    - Intel HEX record
  ?                - Help
```

- The `:LLAAAATT..CC` line shipped with Phase 5, the `T` line with Phase 6, the `A` and `U` lines with Phase 7, the `N` line with Phase 8, the `Q` line with Phase 9, and the `R` line with Phase 10. Each line ships in the same commit as its feature.
- Arguments after `?` are ignored.

### 6.15 T: Time

`T`

1. Runs the mailbox command `TIME` through the mailbox client (section 9; `DEVICE_SPECS.md`, Service Mailbox, Reference client): clear (OUT 11h ← 02h), send `T` `I` `M` `E` to OUT 10h, execute (OUT 11h ← 01h), then poll IN 12h.
2. Each response byte read from IN 13h is printed to the console as it arrives, except that an LF (0Ah) prints as `<CR><LF>`. TIME's response has no LF, so T's output does not change. The loop is shared with N (6.18) and Q (6.19).
3. On status 03h (DONE), prints `<CR><LF>`.
4. On status 00h after execute (Pi service restarted) or 80h-FFh, prints `Service error` then `<CR><LF>`. Any response bytes already printed stay on the same line, with no `<CR><LF>` before the message: an error after `2026-` prints `2026-Service error`.

- Arguments are ignored (4.1). T sends exactly `TIME`.
- The successful output is one line, `YYYY-MM-DD HH:MM:SS` (the Pi's local time), followed by `<CR><LF>`.
- T uses ports 10h-13h.
- T makes no Esc check (6.18): TIME's response is never BUSY and has no LF, so the shared loop never checks for T, and type-ahead after T is kept.
- T transcripts match the shape `NNNN-NN-NN NN:NN:NN` (N = decimal digit), never a value, so they run unchanged on hardware against the Pi's clock. Device-level and emulator tests may inject a clock (`DEVICE_SPECS.md`, TIME clock) to check exact values, padding and the clock-not-set error.

### 6.16 A: Assemble

`A addr`

Assembles one instruction per line into memory, starting at `addr`. The assembler runs on the Pi: mailbox `ASM` (`DEVICE_SPECS.md`, Service Mailbox, ASM).

1. Parse `addr` (word, required). If it is missing or invalid, print `Invalid address` and stop. No port is written.
2. Print the prompt `AAAA: `: the current address, `:`, then a space. No `<CR><LF>` comes before it.
3. Read one line with READ_LINE (section 2: up to 79 characters, BS and DEL, echo, `<CR>` or `<LF>` ends it).
4. Skip leading spaces. If nothing is left, go back to step 2 with the same address: nothing is sent and nothing else is printed. If the first non-space character is `.`, A ends and the `> ` prompt follows; the rest of the line is ignored.
5. Otherwise run the mailbox command `ASM <text>`. `<text>` runs from the first non-space character to the end of the stored line, trailing spaces included. The sequence is: clear (OUT 11h ← 02h); `A` `S` `M` `<SP>` and then each character of `<text>` to OUT 10h; execute (OUT 11h ← 01h); poll IN 12h.
6. Write each response byte to the current address, then advance the address by 1 (FFFF wraps to 0000). On DONE, go back to step 2 with the advanced address.
7. On status 82, the line does not assemble. Print `Invalid instruction`, write nothing, and go back to step 2 with the same address.
8. On status 00 after execute (the Pi restarted) or any other status from 80h to FFh, print `Service error` and go back to step 2 with the address this line started at. ASM never fails after its first byte (`DEVICE_SPECS.md`, ASM), so only a Pi restart can leave part of a line written; typing the line again rewrites it.

- **Only `.` ends A.** An empty line, a line of only spaces, and every failure prompt again. Pasted source therefore never falls through to the command dispatcher, where an instruction would run as a command (`XCHG` as `X CHG`, which mounts a file). A file sent with `<CR><LF>` line ends gives an empty line after every line, which only prompts again. With a dead or Phase 6 Pi service every line prints `Service error`; type `.` to leave.
- A success prints nothing more. The next prompt's address shows how many bytes were written.
- **Syntax:** one instruction in the ASM grammar (`DEVICE_SPECS.md` 8, ASM).
- **No guard**, as E: writes to F000-FFFF have no effect (JP-WE open, `ARCHITECTURE.md` 6.10), but the address still advances. Writing to the workspace (0080-00FF) or the stack page (EF00-EFFF) has undefined results, as F.
- A keeps no state. When it ends, the address is lost; `A addr` starts again anywhere. A changes neither LAST_DUMP_ADDR nor LAST_EXAM_ADDR.
- Tokens after `addr` are ignored (4.1).
- Lines may be pasted. Nothing paces the sender (section 2). Each line costs one mailbox round trip, about 3,800 cycles for `MVI A,0D` (1.9 ms at 2.048 MHz, before READY wait states).
- A uses ports 10h-13h, and only after a line other than an empty one or `.` is entered.

Example:

```
> A 0200
0200: MVI A,0D
0202: lxi h , 1234
0205:
0205: JMP 0200
0208: MOV A,Q
Invalid instruction
0208: .
>
```

This stores 0200-0207 = 3E 0D 21 34 12 C3 00 02.

### 6.17 U: Unassemble

`U addr [count]`

Disassembles `count` instructions starting at `addr`. The disassembler runs on the Pi: mailbox `DIS` (`DEVICE_SPECS.md`, Service Mailbox, DIS).

1. Parse `addr` (word, required), then `count` (word, optional, default 0008; 4.3). If `addr` is missing or invalid, print `Invalid address`. If `count` is present but invalid, print `Invalid hex value`. A count of 0 prints `Invalid range`. No port is written until both have parsed.
2. For each instruction, run the mailbox command `DIS AAAA B0 B1 B2`. AAAA is the current address and B0 B1 B2 are the bytes at the address, the address + 1 and the address + 2 (wrapping past FFFF), each in uppercase hex. The sequence is: clear (OUT 11h ← 02h); `DIS `, the 4 address digits, then `<SP>` and 2 digits for each byte, to OUT 10h (17 bytes); execute (OUT 11h ← 01h); poll IN 12h.
3. The first response byte is the instruction's length, L (01-03). U prints every following byte to the console as it arrives: that is the instruction line and its `<CR><LF>`.
4. On DONE, the address advances by L (FFFF wraps to 0000). Then the next instruction follows.
5. On status 00 at any point after execute (the Pi restarted), on 80h-FFh, or on DONE before the length byte, print `Service error` and end U. Bytes of the current line already printed stay on that line, with no `<CR><LF>` before the message, as for T (6.15).

- **Line format:** U prints the `DIS` line (`DEVICE_SPECS.md` 8, DIS) unchanged. The ROM has no opcode table: it learns each instruction's length from the length byte.
- U only reads memory. It reads three bytes for every instruction, whatever its length. Memory reads have no side effects on this machine. F000-FFFF reads as the ROM. U over the stack page shows U's own stack use, as C does (6.1).
- U keeps no state and changes neither LAST_DUMP_ADDR nor LAST_EXAM_ADDR. Tokens after `count` are ignored (4.1).
- U wraps: `U FFFF 2` lists FFFF, then the address after it modulo 10000h.
- U uses ports 10h-13h. One line costs about 4,450 cycles for `NOP` to 5,600 for `LXI SP,EFFE` (2.2-2.7 ms at 2.048 MHz, before READY wait states), so the default 8 lines take about 18-22 ms; the whole `U addr` command, with its command line and setup, takes about 20-25 ms (measured 41,967 and 51,037 cycles).
- **Round trip:** U's text field (everything after the bytes field), typed at an A prompt, assembles to the bytes U showed, except the R2 aliases (`DEVICE_SPECS.md` 8, Round-trip properties).

Example, after the 6.16 example:

```
> U 0200 3
0200  3E 0D     MVI A,0D
0202  21 34 12  LXI H,1234
0205  C3 00 02  JMP 0200
>
```

### 6.17.1 A and U conformance vectors

Phase 7 tests MUST cover every row. "Prompts" lists the `AAAA: ` prompts A prints, in order. RAM is set by `F` or `A` first, because transcripts only display memory they wrote.

- **Where each row lives.** Rows marked *scripted* are Rust tests in `tests/monitor_tests.rs`: they map the test-local `ScriptedMailbox` (statuses, bytes) at 10-13 with `map_mailbox`, as `scripted` does for T, A and U, and the statuses and bytes given are its script. `Mailbox` gets no test knob: it is the code the Pi runs. Rows marked *ports* use the real `Mailbox` from `build_bus` and check the port sequence. Rows marked *rule 4* join the `lines` list of `argument_errors_write_no_port_and_no_memory_outside_the_workspace`. Every other row is a transcript.
- **An A dialog in a transcript** is one step in the `<` form, every line typed with `\r`, ending in the `.` line, followed by the expected lines. Example: `< A 0200\rMVI A,0D\r.\r`, then `A 0200`, `0200: MVI A,0D`, `0202: .`. The step ends at the `> ` after `.`, so neither the transcript player nor the daemon end-to-end test changes.

| Input | Expected output | Memory and port effect |
|---|---|---|
| `A 0200`, `MVI A,0D`, empty line, `.` | prompts `0200: `, `0202: `, `0202: ` | 0200-0201 = 3E 0D |
| `A 0200`, `  lxi h , 1234`, `.` | prompts `0200: `, `0203: ` | 0200-0202 = 21 34 12 |
| `A 0200`, `NOP*`, `call* 0005`, `   . junk` | prompts `0200: `, `0201: `, `0204: ` | 0200-0203 = 08 DD 05 00 |
| `A 0200`, `MOV A,Q`, `NOP`, `.` | `0200: `, `Invalid instruction`, `0200: `, `0201: ` | 0200 = 00 |
| `A 0200`, a line of only spaces, `.` | prompts `0200: `, `0200: ` | none; no mailbox port written |
| Paste: `A 0200`, `NOP`, empty line, `XCHG`, empty line, `ADD B`, `.` | prompts `0200: `, `0201: `, `0201: `, `0202: `, `0202: `, `0203: ` | 0200-0202 = 00 EB 80; nothing mounted or created (`X` never runs) |
| `a 0200` (lowercase), `RST 7`, `.` | prompts `0200: `, `0201: ` | 0200 = FF |
| `A 0200 junk`, `NOP`, `.` | as `A 0200` (4.1) | 0200 = 00 |
| `A FFFF`, `CALL 1234`, `.` | prompts `FFFF: `, `0002: ` | FFFF unchanged (ROM), 0000-0001 = 34 12 |
| *rule 4:* `A` | `Invalid address` | no port written |
| *rule 4:* `A 02G0`, `A 10000` | `Invalid address` (each) | no port written |
| *ports:* `A 0200`, `MVI A,0D`, `.` | prompts `0200: `, `0202: ` | OUT 11 02; OUT 10 41 53 4D 20 4D 56 49 20 41 2C 30 44 (`ASM MVI A,0D`); OUT 11 01; IN 12 02, IN 13 3E, IN 12 02, IN 13 0D, IN 12 03 |
| *scripted* [83]: `A 0200`, `NOP`, `.` | `0200: `, `Service error`, `0200: ` | 0200 unchanged |
| *scripted* [00]: `A 0200`, `NOP`, `.` | `0200: `, `Service error`, `0200: ` | 0200 unchanged |
| *scripted* [80] (an old Pi service): `A 0200`, `NOP`, `.` | `0200: `, `Service error`, `0200: ` | 0200 unchanged |
| *scripted* [82]: `A 0200`, `NOP`, `.` | `0200: `, `Invalid instruction`, `0200: ` | 0200 unchanged |
| *scripted* statuses [02 00 02 02 02 03], bytes [21 21 34 12] (a restart after one byte, then a retry): `A 0200`, `LXI H,1234`, `LXI H,1234`, `.` | `0200: `, `Service error`, `0200: `, `0203: ` | 0200-0202 = 21 34 12 |
| `F 0200 0207 00`, then `A 0200` with the lines of the 6.16 example, then `U 0200 3` | `0200  3E 0D     MVI A,0D` / `0202  21 34 12  LXI H,1234` / `0205  C3 00 02  JMP 0200` | none |
| `F 0200 020F 76`, then `U 0200` | 8 lines, `0200  76        HLT` through `0207  76        HLT` | none |
| *ports:* `U 0200 1` with 0200 = 3E 0D 21 | `0200  3E 0D     MVI A,0D` | OUT 11 02; OUT 10 `DIS 0200 3E 0D 21` (17 bytes); OUT 11 01; then IN 12 / IN 13 pairs: 02 (the length), then the 24 line bytes and 0D 0A; then IN 12 03 |
| `F 0200 0200 08`, `U 0200 1`; `F 0200 0202 DD`, `U 0200 1` | `0200  08        NOP*`; `0200  DD DD DD  CALL* DDDD` | none |
| `A 0000`, `JMP 1234`, `.`, then `U FFFF 2` | `FFFF  FF        RST 7` / `0000  C3 34 12  JMP 1234` | 0000-0002 = C3 34 12. FFFF is in the ROM's unused tail, which the image fills with FF (`ARCHITECTURE.md` 2) |
| *rule 4:* `U`, `U 02G0` | `Invalid address` (each) | no port written |
| *rule 4:* `U 0200 ZZ`, `U 0200 10000` | `Invalid hex value` (each) | no port written |
| *rule 4:* `U 0200 0` | `Invalid range` | no port written |
| *scripted* [83]: `U 0200 2` | `Service error` | — |
| *scripted* [00]: `U 0200 2` | `Service error` | — |
| *scripted* statuses [02 02 02 02 02 02 83], bytes [02 30 32 30 30 20]: `U 0200 2` | `0200 Service error` (the 5 bytes `0200 ` then the message, on one line) | — |
| *scripted* statuses [02 02 02 02 02 02 00], bytes [02 30 32 30 30 20]: `U 0200 2` | `0200 Service error` | — |
| *scripted* [03] (DONE with an empty response): `U 0200 1` | `Service error` | — |
| **Identity:** for every opcode xx, with 0200-0202 = xx 01 02, `U 0200 1` | the output string (`.1`) of `Debugger::new().command(&mut m.cpu, "u 0200 1")`, no symbols loaded, with its LF replaced by CR LF | none |
| **Round trip:** for every opcode xx, `U 0200 1`, then `A 0300` with U's text field, `.` | none extra | 0300.. = 0200.. for the instruction's length, except the R2 aliases (`DEVICE_SPECS.md` 8) |

### 6.18 N: Net (HTTP GET)

`N url [> file]`

Fetches `url` through the mailbox `GET` (`DEVICE_SPECS.md` 8, GET) and prints the body, or, with `> file`, writes the body to the storage file `file` and prints its length.

1. Run the mailbox command `GET <text>`, where `<text>` is the stored line after the `N`, verbatim: leading and trailing spaces included, case unchanged. The sequence is: clear (OUT 11h ← 02h); `G` `E` `T` `<SP>`, then each byte of `<text>`, to OUT 10h; execute (OUT 11h ← 01h); then poll IN 12h. N parses nothing; the device splits the URL, `>` and the file name, and ignores the extra spaces.
2. Print each response byte as it arrives, as T does (6.15): an LF prints as `<CR><LF>`, every other byte unchanged.
3. On DONE, print `<CR><LF>`.
4. On status 00 after execute (the Pi restarted) or 80h-FFh, print `Service error` then `<CR><LF>`. Bytes already printed stay on their line, with no `<CR><LF>` before the message, as T. This covers 82 (no URL, a URL not starting `http://` or `https://`, a bad file name or a malformed `> file`) and 83 (network, HTTP status 400 or above, the time limits).

- **Stream form** `N url`: the body, then `<CR><LF>`. Bytes 80h-FFh and control bytes go to the terminal unchanged. A UTF-8 page shows correctly on a UTF-8 terminal; a binary file shows as garbage, so fetch binaries with `> file`.
- **File form** `N url > file`: prints the 6-digit length (`0012AB`) and `<CR><LF>`. Then `X file` mounts it and `L` loads it. If `file` is already mounted, mount it again to see the new contents (`DEVICE_SPECS.md` 8, GET).
- **Length:** READ_LINE stores 79 characters, so `url`, ` > ` and `file` share the 77 after `N `. A program that drives the mailbox itself has 124 bytes.
- **Time limits.** A request that cannot connect or stalls ends by the device's time limits (`DEVICE_SPECS.md` 8, GET, Time limits) and prints `Service error`. At 155 cycles a byte (248 for an LF, which includes the Esc check; counted in the emulator, before READY wait states) the monitor prints about 13 KB/s at 2.048 MHz, so a 700 KB book takes about a minute: fetch it to a file.
- **Esc aborts.** The shared loop (6.15) checks the keyboard in two places only: on every pass of its own BUSY wait, and at each response LF, before that LF prints. A check reads every byte waiting in the console FIFO (status first: an empty FIFO pops nothing). If any of them is Esc (1Bh), the ROM clears the mailbox (OUT 11h <- 02h), which aborts the request (`DEVICE_SPECS.md` 8, Abort), prints `Aborted` then `<CR><LF>`, and prompts. Every other byte is discarded. Bytes already printed stay on their line, with no `<CR><LF>` before the message (`abAborted`); the LF that saw the Esc does not print.
  - **Stream form:** stopped while BUSY and at each LF. On the board a fast server keeps every status read at 02 (AVAIL), so only the LF check stops it; a body with no LF (binary, minified HTML or JSON) cannot be stopped in stream form: fetch it with `> file`. `Aborted` means the ROM stopped before it saw the request end; the body may already have been complete.
  - **File form:** nothing prints while the request runs, so it is BUSY until its end, and every Esc in that time aborts. Whenever `Aborted` prints, `file` is untouched (the device removes its temporary file). An Esc typed after the request ends (during the length digits, which have no LF) is not seen; it is ignored at the next prompt (section 2) and `file` holds the new body.
  - **Type-ahead is discarded** while the request runs (section 2, the exception). A key that sends an Esc-prefixed sequence (an arrow or function key, Alt-x) also aborts; any rest of the sequence that is not yet waiting reaches the next prompt.
  - A, U and T never check: A and U do not use the shared loop, and T's response is never BUSY and has no LF.
- The space after `N` is optional (section 3). `n` works. Tokens are not checked in the ROM, so section 4's rules do not apply. N has no argument errors and writes ports for every line.
- N uses ports 10h-13h.

Example:

```
> N https://www.gutenberg.org/cache/epub/1342/pg1342.txt > PRIDE.TXT
0B7A2C
> X PRIDE.TXT
Mounted
> L 0 0200 100
Loaded
> N http://192.168.1.10/hello.txt
Hello from the LAN

> N ftp://example.com/
Service error
>
```

(The length shown is illustrative.)

### 6.18.1 N conformance vectors

Phase 8 tests (and Phase 12, for the Esc rows) MUST cover every row. *ports* and *scripted* as in 6.17.1. *server* rows use the real `Mailbox` from `build_bus` and the `DEVICE_SPECS.md` 8 test HTTP server `H` (`tests/support/http.rs`; its address typed into the line). They are Rust tests in `tests/monitor_tests.rs`, because a transcript cannot know the server's port. A *server* row's step waits on the network, not on the 8080, so it is bounded by a 10 s wall-clock deadline (`Instant`) instead of the cycle budget.

| Input | Expected output | Effect |
|---|---|---|
| *ports, server:* `N http://H/hello` (body `Hello` 0D 0A) | `Hello`, then an empty line: bytes 48 65 6C 6C 6F 0D 0D 0A 0D 0A (the body's CR, its LF as CR LF, then CR LF on DONE) | OUT 11 02; OUT 10 `GET ` then ` http://H/hello` (the space after N included); OUT 11 01; then IN 12 / IN 13 pairs reading `Hello` 0D 0A, IN 12 reads of 01 anywhere among them, and a final IN 12 03 |
| *server:* `N http://H/lf` (body `a` 0A `b` 0A) | `a`, `b`, an empty line (bytes 61 0D 0A 62 0D 0A 0D 0A) | — |
| *server:* `Nhttp://H/hello` and `n http://H/hello` | as the first row | — |
| *server:* `N http://H/hello > book.txt`, then `X BOOK.TXT`, `L 0 0200 7`, `D 0200 0206` | `000007`, `Mounted`, `Loaded`, the dump shows `Hello..` | `BOOK.TXT` = `Hello` 0D 0A |
| *server:* `N http://H/404` | `Service error` | — |
| `N`, `N ftp://x`, `N http://x > ..` | `Service error` (each) | mailbox ports written; nothing in the storage directory |
| *scripted* statuses [01 01 02 01 02 03], bytes [41 42]: `N x` | `AB` | — |
| *scripted* statuses [01 01 83]: `N x` | `Service error` | — |
| *scripted* [00]: `N x` | `Service error` | — |
| *scripted* statuses [02 02 01 83], bytes [61 62]: `N x` | `abService error` | — |
| *scripted* statuses [02 03], bytes [0A]: `N x` | an empty line, then an empty line (bytes 0D 0A 0D 0A) | — |
| *scripted* statuses [01]: `N x` CR, then Esc, in one step | `Aborted` | after the execute only IN 12 01 reads, then OUT 11 02; no IN 13 |
| *scripted* statuses [01]: `N x` CR then, in one step, LF Esc; `q` Esc; Esc `[A`; Esc `H 1 1` CR | `Aborted` (each); one prompt, nothing else runs | an Esc behind other waiting bytes aborts, and the bytes waiting after it are discarded; console FIFO empty |
| *scripted* statuses [01 01 01 02 03], bytes [41]: `N x` CR, nothing more | `A` | 4 IN 01 in the step (the line's bytes): a check pops no byte when none waits |
| *scripted* statuses [02], bytes `ab` 0A `cd` 0A then 00 forever: `N x` CR, then Esc | `abAborted` | — |
| *scripted* statuses [02 02 02 02 02 02 03], bytes `a` 0A `b` 0A `c` 0A: `N x` CR `q`, and `N x` CR `H 1 1` CR | `a`, `b`, `c`, an empty line; one prompt, nothing else runs | console FIFO empty |
| *scripted* statuses [01 02 02 02 02 02 03], bytes `HELLO`: `N x` CR LF | `HELLO`, one prompt (the BUSY check ate the LF) | — |
| *scripted* statuses [02 02 02 03], bytes `12:`: `T` CR, then Esc | `12:`; the Esc is ignored at the prompt | — |
| `T` CR `I 12` CR `Q` CR `I 12` CR in one step | the time, `03`, `Service error`, `82`: all four run | — |
| *server:* `N http://H/hang` CR, then Esc, in one step; then `I 12` | `Aborted`, then `00` | the clear left the mailbox IDLE |
| `T` (6.15 rows) | unchanged; `t_runs_the_reference_client` passes unchanged | — |
| `?` | the help text with the `N` line | — |

### 6.19 Q: Ask Claude

`Q text`

Asks Claude one question through the mailbox `ASK` (`DEVICE_SPECS.md` 8, ASK) and prints the answer.

1. Run the mailbox command `ASK <text>`, where `<text>` is the stored line after the `Q`, verbatim. The sequence is: clear (OUT 11h <- 02h); `A` `S` `K` `<SP>`, then each byte of `<text>`, to OUT 10h; execute (OUT 11h <- 01h); then poll IN 12h. Q checks nothing; the device trims the spaces.
2. Print each response byte as it arrives, as T does (6.15). ASK ends lines with CR LF, which the shared loop prints as `<CR><CR><LF>`: one line break on a terminal.
3. On DONE, print `<CR><LF>`.
4. On status 00 after execute (the Pi restarted) or 80h-FFh, print `Service error` then `<CR><LF>`. Bytes already printed stay on their line, with no `<CR><LF>` before the message, as T. This covers 82 (nothing after `Q`, only spaces, or a byte 80h-FFh in the line, which READ_LINE stores: section 2) and 83 (no API key on the Pi, no network, an API error, a refusal, the time limits).

- **Text:** READ_LINE stores 79 characters, so the question has 77 after `Q `. A program that drives the mailbox itself has 124. Case is kept. The space after `Q` is optional (section 3): `Qhello` asks `hello`, and `quit` asks `uit`.
- **The answer** is plain ASCII in lines of at most 79 characters, wrapped by the Pi. Claude is told about this machine and these commands, and to write code as lines the `A` command accepts.
- **Time limits and Esc.** Q ends within 120 s, and within about 10 s when the Pi cannot connect. Esc aborts it, as N (6.18, Esc): while BUSY (connecting, the model thinking) and at each LF of the reply. `HelAborted`, or `Aborted` before the first byte. The clear closes the connection.
- Each Q is independent: Claude does not see earlier questions.
- Q keeps no state, writes no memory, and uses ports 10h-13h.

Example (the answer varies):

```
> Q how do I print a star from a program at 0200?
At A 0200:
MVI A,2A
OUT 00
RET
Then G 0200.
> Q
Service error
>
```

### 6.19.1 Q conformance vectors

Phase 9 tests MUST cover every row. *ports* and *server* as in 6.18.1; *server* rows give the bus a test key and the test server's endpoint. Transcripts never contain an answer: `ask.txt` holds only lines that never leave the device, so it passes with or without a key, on the bench too.

| Input | Expected output | Effect |
|---|---|---|
| *ports, server:* `Q hi`, reply `Hello` LF `world` | `Hello` 0D 0D 0A `world` 0D 0A | OUT 11 02; OUT 10 `ASK ` then ` hi`; OUT 11 01; IN 12 01 any number of times; IN 12 02 / IN 13 pairs; IN 12 03 |
| *server:* `Q hi`, reply `Hel`, then the connection closes | `HelService error` | — |
| `Q`, `Q   `, `q` (`ask.txt`) | `Service error` (each) | mailbox ports written; no request |
| `?` | the help text with the `Q` line | — |

Q shares N's loop and Esc check from the execute on (it enters CMD_NET at `CN_SEND`), so the Esc rows of 6.18.1 cover Q.

### 6.20 R: Registers

`R`

Prints the registers that the last program started by `G` held when it returned with `RET` (section 8) or stopped at an `RST 6` (8.1):

    A=44 F=56 BC=0B0D DE=1234 HL=0081

- The line is `A=`, A, ` F=`, the flags byte, ` BC=`, ` DE=` and ` HL=` with each pair high byte first, then `<CR><LF>`. Bytes take 2 digits and pairs 4 (section 0). The notation is the debugger's (`ARCHITECTURE.md` 7.4, ring line) without SP.
- F is the raw flags byte (S Z 0 AC 0 P 1 CY, bit 7 to bit 0; `ARCHITECTURE.md` 5.1). It is not decoded.
- **When the registers are captured.** Only when a program started by `G` executes `RET` to the return address (section 8), or executes a planted `RST 6` (8.1). They are saved in REGS (`ARCHITECTURE.md` 1.1). Nothing else writes REGS: not cold start, not WARM, not a command, not a `G` that fails to parse (`G ZZ`), not a program that never returns. R itself changes nothing, so R twice prints the same line.
- **Before the first capture** R prints REGS as cold start left it: cold start does not write REGS, and RAM is undefined at power-on. RAM survives RESET (`ARCHITECTURE.md` 3.1), so after a RESET R still shows the last return before it, if there was one. Nothing marks a stale capture.
- **Not shown.** SP: after a `RET` the contract fixes it (EFFEh when `RET` runs, F000h after); after a break the monitor drops it (8.1). PC: after a break the `BRK` message shows it; after a `RET` the monitor cannot know where the `RET` was. INTE: no 8080 instruction reads it.
- **Breakpoints.** Plant `RST 6` (F7) with `E` or `A` where the program should stop (8.1). R then shows the registers at that instruction. In the emulator the host debugger (`ARCHITECTURE.md` 7.4) does more and costs the 8080 nothing.
- **Read-only.** R does not change registers, and G does not load them: a program's registers on entry stay unspecified (section 8).
- R uses only the console (port 00h). It has no error case. Tokens after `R` are ignored (4.1).
- REGS is ordinary workspace RAM (`ARCHITECTURE.md` 1): a program that writes it changes what R prints.

Example (the program at 0300 is `LXI H,4456` / `PUSH H` / `POP PSW` / `LXI B,0B0D` / `LXI D,1234` / `LXI H,0081` / `RET`):

    > :0F030000215644E5F1010D0B113412218100C982
    > G 0300
    > R
    A=44 F=56 BC=0B0D DE=1234 HL=0081
    >

### 6.20.1 R conformance vectors

Phase 10 tests MUST cover every row. Rows marked *Rust* are tests in `tests/monitor_tests.rs` and run in the emulator only; the others are `tests/transcripts/registers.txt`.

| Input | Expected output | Notes |
|---|---|---|
| The 6.20 example, then `R` | `A=44 F=56 BC=0B0D DE=1234 HL=0081` | |
| Then `H 1234 0081`, `G ZZ`, `r junk` | `12B5 11B3`, `Invalid address`, then the same R line | commands and a failed G do not capture; case folds; arguments ignored |
| `:0F03100021FF00E5F101FFFF11008021FFEEC981`, `G 0310`, `R`, `R` | `A=00 F=D7 BC=FFFF DE=8000 HL=EEFF` twice | the next return replaces the capture; POP PSW of FF reads back D7 (`ARCHITECTURE.md` 5.1) |
| `?` | the 6.14 text with the R line | `help.txt` |
| *Rust:* `R` right after cold start; then the 6.20 example, RESET, cold start, `R` | REGS unchanged by cold start; then the 6.20 example's R line | cold start does not write REGS; RAM survives RESET. Not a transcript: RAM at power-on differs on hardware, and a transcript cannot RESET |
| *Rust:* `go_entry_contract` | the word at EFFE is `G_RETURN` | replaces `WARM` |

The capture at a break has its own vectors in 8.1.1.

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

Because of step 6, a record that is accepted writes only inside 0100-EEFF. The RAM test build lowers the top to D000 (`ARCHITECTURE.md` 2.1).

### 7.3 Actions on a valid record

| Record | Action | Output |
|---|---|---|
| Type 00, LL > 0 | Write the data bytes to AAAA, AAAA+1, ... | None |
| Type 00, LL = 0 | Nothing | None |
| Type 01 (EOF) | Nothing. Once the checksum passes, LL, AAAA and any data are ignored | `Loaded` |

- The guard (step 6) applies only to records that write. The standard EOF record `:00000001FF`, at address 0000, is accepted.
- The guard ranges come from the memory map (`ARCHITECTURE.md`, Memory Map): 0000-007F is unused (except 0030-0032, which G writes, 8.1), 0080-00FF is the workspace (including LINE_BUFFER), EF00-EFFF is the monitor stack, and F000-FFFF is ROM. Writes can never reach LINE_BUFFER, so the loader may read data bytes from the buffer while it writes them.
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
- SP = EFFEh, and the word at EFFE is the return address, G_RETURN.
- Interrupts are disabled (the ROM never executes `EI`).
- A, the flags, BC, DE and HL are unspecified.
- The overlay is disabled.
- Console input that READ_LINE has not consumed (for example, the `<LF>` of a CRLF pair) is left in the FIFO for the program.
- 0030-0032 holds `JMP BRK_ENTRY` for breakpoints (8.1). Programs MUST NOT rely on it. The rest of 0000-007F is undefined: no other RST vector, no API table (`ARCHITECTURE.md`, Memory Map).

**To return:** execute `RET` with SP = EFFEh and the word at EFFE intact. `RET` is the only supported exit. The return address is not published.

**G_RETURN** saves A, the flags, BC, DE and HL in REGS (`ARCHITECTURE.md` 1.1), where R (6.20) reads them, then enters WARM. Neither executes `DI`: a program that returns with interrupts enabled leaves them enabled. v1 has no interrupt source (`ARCHITECTURE.md` 5.7).

**WARM** sets SP = F000h and enters MAIN_LOOP, which prints the prompt. It prints no banner and no `<CR><LF>`, and it does not reinitialize the workspace. LAST_DUMP_ADDR and LAST_EXAM_ADDR survive unless the program overwrote them.

### 8.1 Breakpoints (RST 6)

A debugging aid, not an exit. A program started by `G` that executes an `RST 6` (F7) the user planted stops. The monitor saves A, the flags, BC, DE and HL in REGS, as G_RETURN does, prints `BRK aaaa` (aaaa = the address of the F7, the pushed return address minus 1) and enters WARM.

- **Vector.** `G` writes `C3 lo hi` (`JMP BRK_ENTRY`) at 0030-0032 every time, after its argument parses. The address is not published. A program that writes 0030-0032 breaks RST 6 until the next `G`; a program the user put at 0030-0032 is overwritten by `G`. Programs MUST NOT use RST 6 to return: `RET` is the only supported exit (section 8), and a later ROM may move or drop the vector.
- **Planting.** There is no breakpoint command. Write F7 with `E` (6.3) or `A addr` / `RST 6` (6.16). E shows the byte it replaces; put it back with E. The monitor does not remember or restore it.
- **Stack.** Any call depth. `RST` writes the return address at SP-2 and SP-1 of the program's stack, and the handler discards the program's SP. Those two bytes MUST be in the user area or the stack page (program SP 0102-F000). Elsewhere the result is undefined (`ARCHITECTURE.md` 1).
- **No continue.** G does not load registers (6.20), so a break cannot be resumed. Restore the byte and run the program again.
- **Interrupts.** RST 6 does not clear INTE, and BRK_ENTRY executes no DI (as G_RETURN, section 8). RST 7 is not a breakpoint: it is reserved for an interrupt source (`ARCHITECTURE.md` 6.7).

Example (the 6.20 program with F7 planted over its `RET` at 030E):

    > :0F030000215644E5F1010D0B113412218100C982
    > :01030E00F7F7
    > G 0300
    BRK 030E
    > R
    A=44 F=56 BC=0B0D DE=1234 HL=0081
    >

### 8.1.1 Breakpoint conformance vectors

Tests MUST cover every row. All rows are `tests/transcripts/breakpoint.txt`.

| Input | Expected output | Notes |
|---|---|---|
| `:0F030000215644E5F1010D0B113412218100C982`, `:01030E00F7F7` (F7 over the RET), `G 0300`, `R` | `BRK 030E`, then `A=44 F=56 BC=0B0D DE=1234 HL=0081` | the break captures. F=56: Z, AC, P set, CY clear |
| `F 0030 0032 76`, `:07032000310004CDF004C917`, `:1004F0000122111144332166553E5AB7370000F7E7`, `G 0320`, `R` | `BRK 04FF`, then `A=5A F=07 BC=1122 DE=3344 HL=5566` | G repairs a clobbered vector (76 = HLT would halt the run); a break one CALL deep on the program's own stack; CY set (F=07: CY, P set, Z clear); the pushed address 0500 has low byte 00 |
| *Rust:* `boot_assumes_nothing` | 0000-007F still holds the harness's junk after cold start | only G writes the vector |
| *Rust:* `argument_errors_write_no_port_and_no_memory_outside_the_workspace` | `G ZZ`, `G 01ZZ`, `G 10100` write nothing in 0000-007F | the vector is written after the parse |

The two transcript rows together guard the flags: F=56 and F=07 differ in Z, AC and CY in both directions.

Program listings for the transcript:

```
0300: LXI H,4456 / PUSH H / POP PSW / LXI B,0B0D / LXI D,1234 / LXI H,0081 / RET   (the 6.20 example)
030E: F7 planted over the RET
0320: 31 00 04  LXI SP,0400
0323: CD F0 04  CALL 04F0
0326: C9        RET              (never reached)
04F0: 01 22 11  LXI B,1122
04F3: 11 44 33  LXI D,3344
04F6: 21 66 55  LXI H,5566
04F9: 3E 5A     MVI A,5A
04FB: B7        ORA A
04FC: 37        STC              (F = 07)
04FD: 00 00     NOP / NOP
04FF: F7        RST 6            (pushes 0500 at 03FC)
```

---

## 9. ROM Routine Contracts

The header comment above each routine in `rom/monitor.asm` is that routine's contract. Register preservation is descriptive, not an ABI. This spec does not repeat the headers. The rules are:

- The convention is stated once, in the header block of `monitor.asm`: **a register that a routine's header lists neither as an output nor as trashed is preserved.**
- A change to a routine's register behavior MUST update its header in the same commit.
- There is no public API and no jump table. User programs MUST NOT call ROM addresses, because they move between builds.
- READ_HEX_WORD and READ_HEX_ADDR24 implement section 4, and READ_HEX_BYTE (a word whose value is at most FF) builds on READ_HEX_WORD. Their headers MUST state the error cases (no digits, too many digits, a token not ended by a space or NUL) and that they skip leading spaces on entry.
- CMD_COMPARE relies on B surviving PRINT_ADDR, PRINT_HEX_BYTE, PRINT_SPACE, CONOUT and PRINT_CRLF.
- MB_SEND, MB_PUT and MB_GET implement the `DEVICE_SPECS.md` reference client (Service Mailbox), which T, A, U, N and Q use. MB_GET's header MUST state its three outcomes (a byte, done, failed with the status in A) and that callers test CY before Z, and its second entry `MB_GOT` (A = a status already read and not 01), where the shared T/N/Q loop enters after its own BUSY wait. MB_KEY's header MUST state that it reads every waiting console byte, discards each that is not Esc, and on Esc clears the mailbox, prints `Aborted` and enters WARM without returning. CMD_NET's header MUST say that Q (CMD_ASK) enters it at `CN_SEND` with HL = its verb string and DE = the text.

---

## 10. Future Commands (Placeholders)

None. Each new command gets its own 6.x section in the phase that adds it.

Quitting the emulator (Ctrl-C) and the debugger (Ctrl-E) are host-side, not monitor commands (`ARCHITECTURE.md`, Host-Side Conveniences).

---

## 11. Hardware Constraints on the ROM

- Ports the monitor uses on its own: 00h-02h (console), 08h-0Ch (storage), 0Dh-0Fh (mount), 10h-13h (Service Mailbox: `T`, `A`, `U`, `N`, `Q`), and FEh (overlay off at boot). I and O can reach any port. The protocols are in `DEVICE_SPECS.md`.
- The ROM does no console chip initialization. The console is a Pi FIFO device behind READY.
- **CONOUT is `OUT 00h` followed by `RET`.** It MUST NOT poll TX-ready: status bit 1 always reads 1, and OUT 00 never waits on the terminal (`DEVICE_SPECS.md`, Console).
- The only polling loops in the ROM wait for a person or a background service, never for a byte transfer: CONIN and E poll RX-ready (port 02h, bit 0); MB_GET (A, U) polls mailbox status (port 12h); and the shared T/N/Q loop (CT_GET) polls mailbox status itself, with an Esc check (port 02h, then 01h while bytes wait) on each BUSY pass. Every other device access assumes an instant answer, which READY provides on hardware.
- The boot's first Pi-window access is the banner's first `OUT 00h`. If the Pi's device service is not running yet, that access waits under READY with no timeout (`DEVICE_SPECS.md`, READY Contract). No ROM code handles the stall.
- The I and O commands run self-modified `IN` and `OUT` stubs in workspace RAM. That works on any 8080 and needs no special hardware.
- `rom/monitor.bin` MUST be at most 4096 bytes and run at F000h.
