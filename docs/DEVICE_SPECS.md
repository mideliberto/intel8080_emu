# I/O Device Specifications

Normative. Every I/O port the 8080 can see, at register level. Where the emulator or the ROM does something different, the code is wrong. Known deltas are tracked in `TODO.md` ("Review findings" and "Decided, to implement").

**Scope (one fact, one home):**
- This file covers the port map, what every register returns and does, and the READY contract as software sees it.
- `ARCHITECTURE.md` covers port-range ownership, the memory map, reset and boot, CPU behavior (including interrupts), the circuits (WAIT flip-flop and handshake, Pi data path, overlay 74HCT74, decode, reset wiring, level translation, clock, power) and host-side emulator conveniences, including the host key map.
- `MONITOR_SPEC.md` covers monitor commands, messages, line input and the HEX loader. This file names a ROM routine only where it is the reference client of a protocol.

**Conventions:** Port numbers and values are hex. "R" means `IN` and "W" means `OUT`. "Ignored" means no state changes. All decisions in this file were made by Mike on or before 2026-10-02 and are binding.

---

## 1. Port Map

| Port | R | W | Device | Behind READY | Status |
|------|---|---|--------|--------------|--------|
| 00 | - | Console data out | Console | yes | Implemented |
| 01 | Console data in | - | Console | yes | Implemented |
| 02 | Console status | - | Console | yes | Implemented |
| 03-07 | unassigned | unassigned | Pi window | yes | - |
| 08 | Address bits 0-7 | Address bits 0-7 | Storage | yes | Implemented |
| 09 | Address bits 8-15 | Address bits 8-15 | Storage | yes | Implemented |
| 0A | Address bits 16-23 | Address bits 16-23 | Storage | yes | Implemented |
| 0B | Data (auto-increment) | Data (auto-increment) | Storage | yes | Implemented |
| 0C | Status | Control | Storage | yes | Implemented |
| 0D | - | Filename char | Mount | yes | Implemented |
| 0E | - | Command | Mount | yes | Implemented |
| 0F | Status | - | Mount | yes | Implemented |
| 10 | - | Command byte | Service Mailbox | yes | Implemented |
| 11 | - | Control | Service Mailbox | yes | Implemented |
| 12 | Status | - | Service Mailbox | yes | Implemented |
| 13 | Response byte | - | Service Mailbox | yes | Implemented |
| 14-6F | unassigned | unassigned | Pi window | yes | - |
| 70-FD | unmapped | unmapped | local chips (none fitted) | no | - |
| FE | unmapped | Overlay off | System control | no | Implemented |
| FF | System status | unmapped | System control | no | Implemented |

**The Pi window is 00-6F.** The console is a Pi FIFO device, so the window starts at 00. The Pi serves every port in the window, assigned or not, under READY (section 3). Ports 70-FF are local logic.

---

## 2. Rules Common to All Ports

1. **Reading a write-only register** (`IN` 00, 0D, 0E, 10, 11) returns FF and has no side effect.
2. **Writing a read-only register** (`OUT` 01, 02, 0F, 12, 13) is ignored.
3. **Unassigned Pi-window ports** (03-07, 14-6F): `IN` returns FF and `OUT` is ignored. The Pi still completes the READY handshake, so the access never hangs.
4. **Unmapped ports outside the Pi window** (70-FD, `IN` FE, `OUT` FF): `OUT` is ignored. The value `IN` returns is undefined on hardware, because nothing drives the bus. Software MUST NOT depend on it. The emulator returns FF (`src/io/bus.rs:26`). (`IN` FF is the system status port, section 5.)
5. **Reads with side effects:** only `IN 01` (pops the console FIFO), `IN 0B` (advances the storage address) and `IN 13` in the AVAIL state (pops the mailbox response). Every other `IN` has no side effect and can be repeated. The monitor's `I` command triggers the same side effects.
6. **Undefined values** written to a command or control register (0C, 0E, 11) change nothing, with one exception: every write to 0E, whatever the value, clears the filename buffer (section 7).
7. **No device raises an interrupt.** Hardware v1 has no interrupt source. The interrupt input and its future tick source are in `ARCHITECTURE.md` (Interrupts).
8. **RESET** (power-on or the reset button) returns the whole machine to its power-on state, RAM excepted:
   - It clears the WAIT flip-flop, so READY is high and no Pi request is pending, and it sets the overlay flip-flop (section 5). Circuit: `ARCHITECTURE.md` (Reset).
   - The Pi sees RESET on an edge-latched GPIO input. It drops any request in flight without raising ACK, and returns devices to their power-on state when RESET is released. The next request it serves is a new access.
   - Every Pi device returns to its power-on state: storage unmounted (flushed durably and closed, address 000000), filename buffer empty, mount status 01, mailbox IDLE with the command buffer and response cleared and any running request aborted, console input FIFO empty, console output buffer empty.
   - An access cut off by RESET may or may not have taken effect. Its device state is then reset as above.
   - No ROM code is involved.
   - **Emulator:** RESET happens only at process start, when `build_bus` (`src/io/mod.rs`) creates every device in its power-on state. Any future host-side reset (Phase 10) MUST also return every device to its power-on state, by calling `build_bus` again.
9. **Pi service restart:** if the Pi's device service restarts (crash, update or reboot) while the 8080 runs, every Pi device returns to its power-on state and the 8080 is not notified. Clients detect this by checking status:
   - storage: status bit 0 drops to 0 (section 6);
   - mailbox: status reads 00 after an execute (section 8).

---

## 3. READY Contract (Software View)

The circuit is in `ARCHITECTURE.md` (Pi Window and READY). This section is the contract software relies on.

1. **One instruction, one access.** Each `IN` or `OUT` to a port in 00-6F is exactly one device access. Accesses are never lost, merged or repeated, no matter how closely they follow each other. The access completes before the next instruction runs: a write's effects are applied and a read's value is final. Example: after `OUT 0Eh` with 01, the next `IN 0Fh` returns the result of that mount.
2. **No byte-level busy polling.** Outside the mailbox, no status bit reports "not ready". Storage status bit 1 and console status bit 1 always read 1. Mount status FF ("busy") is reserved and never returned.
3. **Pi obligations:**
   - Release an access only after the operation has completed.
   - Sense pending requests by level, so that an access already waiting when the device service starts is still serviced.
   - Sample port, direction and OUT data from a GPIO read in which REQ is high (`ARCHITECTURE.md`, Pi Window and READY).
   - After raising ACK, read it back high and wait at least 500 ns before treating REQ as a new access. Never wait for REQ to go low.
   - Separate dependent GPIO steps (drive data, LATCH, ACK) with a read-back of the GPIO level register.
   - On RESET, drop any request in flight without raising ACK. Check for a latched RESET immediately before every ACK (rule 2.8; `ARCHITECTURE.md`, Reset).
   - Do only bounded local work under READY: console byte transfer, storage address, data and control operations (including fsync and filling a past-EOF gap), and mount commands (open, create, fsync or close a local file). "Bounded" means the operation always finishes. It does not mean it is fast: an fsync, or a write at FFFFFF in an empty file, can hold READY for seconds.
   - Never wait on the network, an external service or user input while holding READY. Unbounded work (anything behind the mailbox) runs in the background and reports through mailbox status (section 8).
4. **No timeout.** The 8080 waits as long as the access is pending. A dead Pi stalls the 8080 until RESET. Until the Pi's device service is running, the first Pi-window access stalls and then completes once the Pi services it. At boot that access is the banner's first `OUT 00`. The stalled access MUST NOT complete with a floating bus (`ARCHITECTURE.md`, Pi Window and READY, rule 3 and Power and boot independence). No ROM code handles the stall.
5. **Timing:** on hardware every Pi-window access costs 10 T-states plus at least one wait state. T3 starts 0.4-0.9 us after the Pi's ACK, and the Pi's service time comes on top (on the order of microseconds on a busy-polling Pi 4, est). Software MUST NOT depend on how long an `IN` or `OUT` takes. The emulator models no wait states: `IN` and `OUT` take 10 T-states, and device effects are applied within the instruction.
6. **Hardware is the target.** No protocol in this file may rely on emulator-only timing or behavior.

Hardware conformance test (monitor only, with stress-ng loading the Pi's other cores):

1. `X CONF.BIN`
2. `W F000 0 1000`
3. `I 0A`, `I 09`, `I 08` print 00 10 00.
4. `L 0 2000 1000`
5. `I 0A`, `I 09`, `I 08` print 00 10 00.
6. `C F000 FFFF 2000` prints nothing.

A lost or repeated `OUT 0B` or `IN 0B` moves the final address. Stale OUT data, such as the 10h status byte, shows up as a C mismatch.

---

## 4. Console (Ports 00-02)

The console is a Pi FIFO device behind READY. The terminal connects to the Pi; the transport between them is Pi configuration, invisible to the 8080 (`ARCHITECTURE.md`, What Is Local and What Is the Pi). There is no UART chip, no ROM UART init and no input overrun. Nothing paces input: pasted text, including Intel HEX, arrives at any rate and is never lost.

### Registers

| Port | Dir | Function |
|------|-----|----------|
| 00 | W | Send one byte to the terminal |
| 01 | R | Pop one byte from the input FIFO |
| 02 | R | Status |

### Status (IN 02)

| Bit | Meaning |
|-----|---------|
| 0 | RX ready: 1 when the input FIFO holds at least one byte |
| 1 | TX ready: always 1 |
| 2-7 | Always 0 |

`IN 02` therefore returns 02 when no input is waiting and 03 when input is waiting.

### Behavior

- **OUT 00:** the byte goes to the terminal unchanged: 8-bit transparent, no translation, no CR/LF insertion. `OUT 00` never waits on the terminal. When no terminal is attached, or the Pi's output buffer is full, the byte is discarded. The Pi's output buffer holds at least 2 MiB, more than any single monitor command prints (`C 0000 FFFF` prints up to 1,114,112 bytes). Bytes are discarded only when it is full. The emulator's buffer is 2 MiB (`OUTPUT_CAP`, `src/io/devices/console.rs`); the host run loop drains it to stdout.
- **IN 01 with input waiting** returns the oldest byte and removes it from the FIFO. A byte is "waiting" from the moment it reaches the Pi. Arrival never depends on whether the 8080 has read `IN 02`.
- **IN 01 with the FIFO empty** returns 00 and changes nothing.
- **Input bytes** are delivered in arrival order and unchanged, including control characters (00-1F, 7F) and bytes 80-FF. The device delivers every byte the terminal sends.
- **No echo:** the device never echoes input. 8080 software does all echoing (`MONITOR_SPEC.md`, Line Input (READ_LINE)).
- **Input FIFO:** MUST NOT drop bytes. Its capacity is a Pi implementation detail. When the FIFO is full, the Pi stops the terminal with out-of-band flow control: RTS/CTS, or the transport's own (USB, TCP). In-band XON/XOFF is forbidden, because 11 and 13 are data.
- **Power-on and RESET:** the input FIFO is empty, status reads 02, and console output not yet sent to the terminal is discarded. Bytes received before RESET is released are discarded, so the ROM needs no input drain at boot.
- **Emulator:** host keys become FIFO bytes through `ARCHITECTURE.md` (Host Key Map), the only home for host-reserved keys.

### Reference client

```asm
CONOUT: OUT     00H             ; never waits; TX ready is always 1
        RET
CONIN:  IN      02H
        ANI     01H
        JZ      CONIN
        IN      01H
        RET
```

This is the monitor's CONOUT and CONIN (`MONITOR_SPEC.md`, Hardware Constraints on the ROM).

---

## 5. System Control (Ports FE-FF)

This is local glue logic, not a Pi port, so it adds no READY wait. The circuit is in `ARCHITECTURE.md` (Overlay Glue).

| Port | Dir | Function |
|------|-----|----------|
| FE | W | Any value: disable the ROM overlay |
| FF | R | Bit 0 = overlay flip-flop (1 = enabled). Bits 1-7 are undefined and software MUST mask them. The emulator returns 0 in bits 1-7 |

- **OUT FE** with any value 00-FF clears the overlay flip-flop. Writing it again has no further effect.
- **Re-enabling:** only RESET sets the flip-flop. Software cannot re-enable it.
- There is **no halt command** (use `HLT`) and **no software reset** (use the reset line).
- **IN FE and OUT FF** are unmapped (rule 2.4).
- The memory effects of the overlay are in `ARCHITECTURE.md` (ROM Overlay).

---

## 6. Storage (Ports 08-0C)

One mounted file, addressed linearly with 24 bits (16 MB). There are no sectors, tracks or banks.

### Registers

| Port | R | W |
|------|---|---|
| 08 | Address bits 0-7 | Set address bits 0-7 |
| 09 | Address bits 8-15 | Set address bits 8-15 |
| 0A | Address bits 16-23 | Set address bits 16-23 |
| 0B | Read the byte at the address, then address += 1 | Write the byte at the address, then address += 1 |
| 0C | Status | Control |

### Address register

- There is one 24-bit register. Each write to 08, 09 or 0A replaces only that byte and takes effect immediately; there is no commit step. The writes may come in any order.
- `IN 08`, `IN 09` and `IN 0A` return the current address bytes, including the effect of auto-increment.
- Increment wraps FFFFFF to 000000. Decrement (control 01) wraps 000000 to FFFFFF.
- The address is set to 000000 at power-on and RESET, by every mount command (successful or failed), by unmount, by control 00, and when an I/O error unmounts the file. Query leaves it unchanged.

### Data port (0B)

Every `IN 0B` and every `OUT 0B` advances the address by exactly 1, mounted or not and whatever the EOF state. The one exception is an access that hits a host I/O error (see Durability and errors).

| Condition | IN 0B returns | OUT 0B effect |
|-----------|---------------|---------------|
| Mounted, address < size | The byte at the address | Overwrites the byte at the address |
| Mounted, address >= size (past EOF) | FF | Writes the byte. Size becomes address+1. Any gap between the old size and the address reads back as 00 |
| Not mounted | FF | The byte is discarded |

- **Read-your-writes:** a read returns the last byte written to that address, whether or not it has been flushed.
- **Maximum file size** is 0x1000000 bytes (16 MB), the most the address can reach.

### Status (IN 0C)

| Bit | Meaning |
|-----|---------|
| 0 | Mounted: 1 when a file is mounted |
| 1 | Ready: always 1 (the READY wait-state replaces polling) |
| 2-6 | Always 0 |
| 7 | EOF: 1 when address >= file size. Always 1 when nothing is mounted, because the size is then 0 |

Example values: 82 (not mounted), 03 (mounted, inside the file), 83 (mounted, at or past EOF).

### Control (OUT 0C)

| Value | Function |
|-------|----------|
| 00 | Address = 000000 |
| 01 | Address = address - 1 (wraps) |
| 02 | Flush: when the access completes, every byte written so far is durable (fsync on the Pi) |
| other | Ignored |

### Durability and errors

- Writes MAY be buffered. They are durable only after a flush (control 02), an unmount, a mount command (a mount unmounts the previous file first), or a RESET. Flush, unmount, and the unmount step of a mount each fsync.
- **Errors:** a host I/O error on a data read (other than past EOF), a data write or a flush is handled in two steps:
  1. The access completes. A failed read returns FF. The address does not advance.
  2. The device then unmounts the file: status bit 0 drops to 0 and the address becomes 000000. Later accesses follow the "Not mounted" row.
- A client detects failure by reading status bit 0 after its transfer (after its final flush, for writes). The same check catches a Pi service restart in the middle of a transfer (rule 2.9).
- `L` and `W` perform this check and print `Storage error` when bit 0 is 0 (`MONITOR_SPEC.md`, L: Load from storage and W: Write to storage).
- An fsync error during unmount, or during the unmount step of a mount, is not reported: 0F still reads 00 for unmount. Software that needs certainty flushes and checks bit 0 before it unmounts.
- The error path cannot be exercised by `cargo test` (no fault injection), and neither can fsync. Both are verified by review and on hardware. The emulator fsyncs with `File::sync_all`, and dropping the device (process exit, device reset) unmounts it the same way.

### Power-on

Not mounted, address 000000, status 82.

---

## 7. Storage Mount (Ports 0D-0F)

Selects the file that storage (section 6) addresses. All files sit in one flat storage directory, with no subdirectories. In the emulator that directory is `./storage/`; on the Pi it is a configured directory on the SD card. The device service creates the storage directory at startup if it is missing. If the directory does not exist and cannot be created, every mount returns 01 (the emulator creates it in `Storage::new`, `src/io/devices/storage.rs`).

### Registers

| Port | Dir | Function |
|------|-----|----------|
| 0D | W | Append one filename character |
| 0E | W | Command. Every write clears the filename buffer after the command runs |
| 0F | R | Status of the last command |

### Filename buffer (0D)

- Each `OUT 0D` appends one byte. A 00 byte is ignored. Software sees no length limit, but the device MUST remember at least whether more than 12 characters arrived.
- Every `OUT 0E`, whatever the value, empties the buffer after its command runs. A command therefore always sees exactly the characters sent since the previous `OUT 0E`, or since power-on, RESET or a Pi service restart (rules 2.8 and 2.9).
- **Resync rule:** a client MUST write `OUT 0Eh` = 03 (query) before it sends the first name character. This discards any half-sent name left by an aborted earlier client or by `O 0D`.
- Power-on: empty.

### Commands (0E)

| Value | Command | Effect | 0F afterwards |
|-------|---------|--------|---------------|
| 01 | Mount | See below | 00, 01 or 02 |
| 02 | Unmount | Flushes durably (fsync) and closes any mounted file; address = 000000. Allowed when nothing is mounted | 00 |
| 03 | Query | No change | 00 if mounted, 01 if not |
| other | - | Clears the buffer and does nothing else | Unchanged |

### Mount (command 01), in order

1. Unmount the current file, if any: flush durably, close it, and set the address to 000000. A failed mount leaves nothing mounted.
2. Convert `a`-`z` in the name to `A`-`Z`. Host files whose names contain lowercase letters cannot be reached (on a case-sensitive host filesystem such as the Pi's ext4; a case-insensitive one, such as default macOS APFS, matches them anyway).
3. Validate the name. Any failure sets status 02, and nothing is mounted:
   - The name is 1-12 characters long. A name of 13 or more returns 02 and is never truncated.
   - Every character is `A`-`Z`, `0`-`9`, `.`, `-` or `_`.
   - The name is not `.` or `..`.
   - The 8.3 form is a convention only and is not checked: `A.B.C` and `LONGNAME1234` are valid.
4. Open the file read/write in the storage directory. If it does not exist, create it empty. There is no "not found".
   - On success: status 00, address 000000, size = the file's length.
   - On a host failure (permission denied, a directory with that name, a missing storage directory, a disk error), or when the file is larger than 0x1000000 bytes (16 MB): status 01, and nothing is mounted.
5. Clear the filename buffer, as after every 0E write.

Mounting the name that is already mounted closes and reopens the file: status 00, address 000000.

### Status codes (IN 0F)

| Value | After Mount | After Unmount | After Query |
|-------|-------------|---------------|-------------|
| 00 | Mounted | Done | A file is mounted |
| 01 | Open failed (host error or file too large) | - | Nothing mounted |
| 02 | Invalid filename | - | - |
| FF | Reserved; never returned (READY) | | |

`IN 0F` has no side effect and returns the last result until the next command. The power-on value is 01, which is what a query would return.

### Reference client

```asm
        ; HL -> name, NUL-terminated
        MVI     A,03H
        OUT     0EH             ; query: clears the name buffer (resync)
SEND:   MOV     A,M
        ORA     A
        JZ      GO
        OUT     0DH
        INX     H
        JMP     SEND
GO:     MVI     A,01H
        OUT     0EH             ; mount; the buffer is cleared afterwards
        IN      0FH             ; 00 ok, 01 open failed, 02 invalid name
```

The monitor's `X name` follows this sequence and prints `Invalid filename` for 02 and `Mount failed` for any other nonzero status (`MONITOR_SPEC.md`, X: Mount, unmount, query).

---

## 8. Service Mailbox (Ports 10-13)

One device carries every Pi service. The 8080 writes a text command and reads back a byte stream. The Pi handles TLS, DNS, JSON, NTP and the API key. The device logic is Rust behind `IoDevice`: the emulator bus calls it, and on the Pi a GPIO front end calls the same code. Its first use is Phase 6 `TIME`.

### Registers

| Port | Dir | Function |
|------|-----|----------|
| 10 | W | Append one byte to the command buffer |
| 11 | W | Control: 01 = execute, 02 = clear. Other values are ignored |
| 12 | R | Status |
| 13 | R | Pop one response byte |

### States

| Status (IN 12) | State | Meaning |
|----------------|-------|---------|
| 00 | IDLE | No request. Set at power-on, RESET and after clear |
| 01 | BUSY | A request is running and its next response byte is not ready yet |
| 02 | AVAIL | At least one response byte is ready at port 13 |
| 03 | DONE | The request has finished and every response byte has been read |
| 80-FF | ERROR | The request failed. The value is the error code |
| 04-7F | - | Never returned |

### Transitions

| Event | From | To |
|-------|------|----|
| `OUT 11` = 01 (execute) | any | Abort any running request (see Abort). Take the command buffer as the new request and empty the buffer. Go to BUSY, or directly to AVAIL, DONE or ERROR when the result is available within the access |
| The request produces a byte | BUSY | AVAIL |
| `IN 13` | AVAIL | AVAIL if another byte is ready; otherwise BUSY if the request is still running; otherwise DONE |
| The request completes with nothing left to read | BUSY | DONE |
| The request fails (including mid-response) | BUSY, AVAIL | ERROR. Unread response bytes are discarded |
| `OUT 11` = 02 (clear) | any | IDLE. Abort any running request, discard the response and empty the command buffer |

- BUSY can follow AVAIL in the middle of a response, for streamed results (Phase 8 and later). One polling loop handles every case.
- DONE and ERROR persist until the next execute or clear.
- A response may be empty.
- Response bytes can be any value 00-FF. The end is marked by status, not by a terminator.
- **Abort** completes within the `OUT 11` access and never waits on the network. The device marks the old request cancelled, and nothing a cancelled request produces is ever delivered.
- **Phase 6:** `TIME` completes within the execute access. Right after execute, `IN 12` reads 02 or an error code (80-83), or 00 after a Pi service restart (rule 2.9). It never reads 01. The emulator needs no background worker until a command that takes time exists (Phase 8).

### Reading 13 in each state

| State | IN 13 returns | Side effect |
|-------|---------------|-------------|
| AVAIL | The next response byte | Consumed. Status changes as in the transitions table |
| IDLE, BUSY, DONE, ERROR | 00 | None |

### Command buffer (10)

- Each `OUT 10` appends one byte.
- The buffer holds at most 128 bytes. Exactly 128 bytes is accepted. Bytes beyond 128 are dropped and an overflow flag is set; the next execute then fails with 81. Clear and execute both reset the flag.
- Clear and execute both empty the buffer. Power-on and RESET: empty, flag clear.
- **Resync rule:** a client MUST write clear (`OUT 11` = 02) before it sends the first command byte. This discards any half-sent command left by an aborted earlier client.

### Command format

- The **command word** is the bytes before the first 20h, or the whole buffer when it contains no 20h. It is matched exactly and case-sensitively against the uppercase names below. An empty word, a lowercase word or an unknown word gives 80.
- The **argument string** is everything after the first 20h, passed verbatim and possibly empty. URLs are case-sensitive.
- There is no terminator: execute ends the command.
- Text responses use CR LF (0D 0A) between lines and after each line, except for `TIME` (below).

### Error codes

| Code | Meaning |
|------|---------|
| 80 | Unknown or empty command word |
| 81 | Command buffer overflowed (more than 128 bytes) |
| 82 | Bad or missing arguments for a known command |
| 83 | Service failed (no network, API error, clock not set, host error) |
| 84-FF | Reserved |

### Commands

| Command | Phase | Exact buffer | Response |
|---------|-------|--------------|----------|
| `TIME` | 6 | `TIME` only. Any other buffer whose command word is `TIME` (for example `TIME ` or `TIME UTC`) gives 82 | 19 bytes, `YYYY-MM-DD HH:MM:SS`: the Pi's local time, 24-hour, zero-padded, with no line ending. Example: `2026-10-02 14:30:05`. If the Pi clock is not set (no NTP sync and no RTC), the result is 83. The emulator uses the host clock |
| `ASM <line>` | 7 | Placeholder | Designed in Phase 7 |
| `DIS <bytes>` | 7 | Placeholder | Designed in Phase 7 |
| `GET <url> [> FILE]` | 8 | Placeholder | Designed in Phase 8, including how it interacts with the mounted file |
| `ASK <prompt>` | 9 | Placeholder | Designed in Phase 9 |

Large results go to storage files, and the 8080 reads them through section 6. How long a request may run, and how a hung request ends, is designed in Phase 8 with `GET`. `TIME` cannot hang.

### Reference client

This is the shared send-and-poll loop. Each command supplies its own byte sink and its own error handling. The monitor's `T` prints each byte to the console, then CR LF, and prints `Service error` when the loop reaches ERR (`MONITOR_SPEC.md`, T: Time).

```asm
        MVI     A,02H
        OUT     11H             ; clear (resync)
        ; ... OUT 10H for each command byte ...
        MVI     A,01H
        OUT     11H             ; execute
POLL:   IN      12H
        CPI     01H
        JC      ERR             ; 00 after execute: Pi restarted (rule 2.9)
        JZ      POLL            ; 01 busy
        CPI     03H
        RZ                      ; 03 done
        JNC     ERR             ; 80+ error (04-7F never returned)
        IN      13H             ; 02 avail
        CALL    SINK            ; per-command byte sink
        JMP     POLL
```

---

## 9. History

Superseded on 2026-10-02 (see COLLABORATION_LOG Key Decisions):
- the earlier per-device register specs (asm/disasm, Claude, HTTP, time, 8253) in this file at git 41f04ce, replaced by the Service Mailbox;
- the console and parallel reservations at 03-07 in `ARCHITECTURE.md` and the UART-chip option in `TODO.md`, replaced by the Pi FIFO console;
- "the sender paces lines" for pasted input, replaced by the Pi FIFO console: nothing paces;
- the 08-6F Pi window, replaced by 00-6F;
- the interim timer at 30-32 and the `OUT FE` = FF cold reset, deleted.

---

## 10. Implementation Map

| Device | Emulator | Hardware |
|--------|----------|----------|
| Port map 00-6F | `build_bus` (`src/io/mod.rs`), used by `main.rs` and every test harness | Pi daemon: the same function |
| Console 00-02 | `src/io/devices/console.rs` (input FIFO and output buffer, no terminal code); the terminal side is `src/main.rs` | Pi, with the terminal connected to the Pi |
| Storage 08-0C, Mount 0D-0F | `src/io/devices/storage.rs`, one device (std::fs) | Pi: the same Rust code, files on its SD card |
| Service Mailbox 10-13 | `src/io/devices/mailbox.rs`. `TIME` reads a clock passed to `Mailbox::new` (a plain fn returning the date and time fields; the device formats the 19 bytes): `build_bus` passes the host's local time (`mailbox::local_time`), tests pass a fixed or a failing one | Pi: the same Rust code behind GPIO, with a clock that reports "not set" (83) |
| System control FE-FF | `src/cpu.rs` | 74HCT74 and decode (`ARCHITECTURE.md`, Overlay Glue) |
