# User Guide

How to operate the machine. Not normative. This guide holds procedures; every fact behind one is
owned by the spec section it names, and where this guide and a spec disagree, the spec wins. Every
monitor session below that shows output is copied from the transcript it names. Command syntax at a
glance: [QUICK_REFERENCE.md](QUICK_REFERENCE.md).

---

## 1. Three Ways to Run the Same ROM

The same `rom/monitor.bin` runs on all three. The 8080 cannot tell them apart.

| Where | What runs the 8080 | Console | Use it for |
|---|---|---|---|
| Emulator (`cargo run`) | The CPU model, on your machine | Your terminal | Writing and debugging programs and ROM changes |
| `pi8080d --sim` | The CPU model, inside the Pi daemon | TCP, as on the board | Testing the Pi side (daemon, unit, storage, clock) before the board exists |
| The board | An 8080A, with `pi8080d` on the Pi behind the I/O ports | TCP to the Pi | The real thing |

Specs: [ARCHITECTURE.md](ARCHITECTURE.md) (memory, boot, hardware), [DEVICE_SPECS.md](DEVICE_SPECS.md)
(ports), [MONITOR_SPEC.md](MONITOR_SPEC.md) (commands), [PI_DAEMON.md](PI_DAEMON.md) (the Pi side).

---

## 2. The Emulator

```
cargo run                       # from the repo root: rom/monitor.bin, storage files in storage/
cargo run -- --debug            # start stopped at the debugger prompt
cargo run -- --script FILE      # run debugger commands from FILE first
cargo run -- --jp-we            # JP-WE fitted: writes to F000-FFFF program the ROM image (in memory only)
```

- **Ctrl-C** quits. **Ctrl-E** stops the 8080 and opens the debugger (section 8). The 8080 never sees
  either key. Other keys: ARCHITECTURE 7.1.
- **Piped input** goes to the console byte for byte: `printf 'H 1234 0001\r' | cargo run`. At the end
  of the input the monitor waits at its prompt; the run ends on HLT or Ctrl-C (ARCHITECTURE 7.2).
  Input queued behind an `N` or `Q` that reaches the network is discarded while its request runs
  (MONITOR_SPEC 2), so pipe nothing after one.
- The emulator runs at host speed, not 2 MHz. Cycle counts in the specs are for the board.

---

## 3. Using the Monitor

At power-on the monitor prints its banner, `8080 Monitor v<version>` and the build time
(MONITOR_SPEC 1.1), then the `> ` prompt.

- `?` lists every command. QUICK_REFERENCE has the same list with syntax; MONITOR_SPEC 6 is the
  contract for each.
- Numbers are hex and ranges are inclusive (MONITOR_SPEC 4).
- What every message means: MONITOR_SPEC 5.
- Which memory is yours: ARCHITECTURE 1 (the user area).
- To stop a command or program that does not return: RESET on the board, Ctrl-C in the emulator,
  restart the daemon under `--sim`. Esc stops a running `N` or `Q` (section 7.2); no other monitor
  command stops for a key (MONITOR_SPEC 2).

---

## 4. Programs

### 4.1 The rules

A program started with `G` follows the G return contract (MONITOR_SPEC 8): it ends with `RET`, with
SP where `G` left it. Console I/O is DEVICE_SPECS 4; `examples/hello.asm` shows the output side. A program calls nothing in the ROM, because ROM addresses move between builds
(MONITOR_SPEC 9): copy the routine instead. Do not end with `HLT`: on the board the 8080 then stops
until RESET.

### 4.2 Assemble on the host

The monitor's HEX loader takes Intel HEX typed or pasted at the prompt (MONITOR_SPEC 7). With the AS
assembler (`asl`), the one the ROM uses:

```
asl -cpu 8080 prog.asm
p2hex prog.p prog.hex -F Intel -l 16
```

`-l 16` keeps each record within what the loader accepts (MONITOR_SPEC 7). `examples/Makefile` does
both steps for every `.asm` in `examples/`.

### 4.3 Load it

- **Emulator:** paste the `.hex` file into the terminal at the `> ` prompt. The last record prints
  `Loaded`. Any other message rejected that record: fix it and paste it again.
- **Board or `--sim`:** paste the `.hex` into your socat session (section 9.3). TCP flow control paces
  the paste; nothing is lost (PI_DAEMON 7.2).
- **From a script:** `{ cat prog.hex; printf 'G 0100\r'; } | nc localhost 8080`. Leave `nc` running
  until the output you expect has arrived (`Loaded`, then the program's output), then Ctrl-C it.
  After an `N` or `Q`, wait for the prompt before sending more: input that arrives while their
  request runs is discarded (MONITOR_SPEC 2).
  Connect another client only after that: a new client closes the old one, and the old one's unread
  input and unsent output go with it (PI_DAEMON 7.1, 7.2).

### 4.4 Small changes on the machine

`A addr` assembles one instruction per line until a line that is only `.`, and `U addr [count]` lists
instructions (MONITOR_SPEC 6.16, 6.17). Both run on the Pi behind the Service Mailbox, so they need
the daemon or the emulator. To put a three-instruction program at 0200 and run it, type:

```
A 0200
MVI A,41
OUT 00
RET
.
G 0200
```

The exact dialog, prompt by prompt: `tests/transcripts/assemble.txt`.

---

## 5. Example Programs

In `examples/`, each as `.asm` source and a ready `.hex`. Each has a transcript,
`tests/transcripts/example_NAME.txt`, that `cargo test` runs.

| Program | Run | Prints |
|---|---|---|
| `hello` | paste `hello.hex`, `G 0100` | `Hello, 8080!` |
| `memtest` | paste `memtest.hex`, `G 0100` | `RAM OK`, or `FAIL aaaa` at the first bad byte |
| `burn` | the image at 1000, paste `burn.hex`, fit JP-WE, `G 0100` (section 10) | the new monitor's banner, `Not a ROM image`, or `Burn failed aaaa` |

`memtest`'s range, what it catches and how to change the range are in the header of
`examples/memtest.asm`. **Under the RAM test build** (section 10), set the last address to CFFF
before you run it, or it overwrites the running monitor:

```
E 0105
FF
CF
.
```

---

## 6. Storage: Saving and Loading

One storage file is mounted at a time (DEVICE_SPECS 6, 7). Files live in `storage/` (emulator) or the
daemon's `--storage` directory (`/var/lib/pi8080d` on the Pi).

### 6.1 Commands

Mount with `X`, write memory to the file with `W`, load it back with `L`, unmount with `X -`
(MONITOR_SPEC 6.8, 6.12, 6.13). These pairs are copied from `tests/transcripts/storage.txt`, which
runs them in a longer sequence:

```
> X test.bin
Mounted
> W 0200 0
Written
> L 0 0400
Loaded
> X -
Unmounted
```

### 6.2 Save and restore a session

There is no snapshot command. W and L are save and load. Type:

```
X SAVE1.BIN
W 0100 0 EE00           the whole user area, 0100-EEFF
```

and later:

```
X SAVE1.BIN
L 0 0100 EE00
G 0100                  restart your program from its entry
```

Registers and the monitor's workspace are not saved: a program restarts from its entry point.

`Written` means the data reached the Pi's disk (MONITOR_SPEC 5). A power cut or RESET during a `W`
leaves the file part old and part new, and the 8080 cannot tell. To always have one good copy,
alternate between two files (`SAVE1.BIN`, `SAVE2.BIN`).

### 6.3 Check the ROM

The 8080 can read its own ROM, so it can copy it out. Type:

```
X ROM.BIN
W F000 0 1000           F000-FFFF, 4096 bytes
X -
```

Then on the Mac, from the repo:

```
scp pi:/var/lib/pi8080d/ROM.BIN /tmp/ROM.BIN        # emulator: use storage/ROM.BIN
cmp -n 4096 /tmp/ROM.BIN rom/monitor.bin
```

No output means F000-FFFF, as the 8080 reads it, is the committed image. `-n 4096` is there because
`W` never shortens a file: an older, longer `ROM.BIN` keeps its tail.

### 6.4 Files on the host

Storage files are plain files. Copy them in and out with `cp` or `scp`, and look at them with
`xxd`. Do not change a file while it is mounted.

---

## 7. Pi Services

`T`, `A`, `U`, `N` and `Q` run on the Pi through the Service Mailbox (DEVICE_SPECS 8). In the
emulator the same Rust code answers.

### 7.1 Time

`T` prints the Pi's local time. `Service error` from `T` means the Pi's clock is not NTP-synchronized
yet (DEVICE_SPECS 8, TIME).

### 7.2 Internet and Claude

- **`N url`** prints the body of a URL. **`N url > FILE`** writes the body to a storage file instead
  and prints its length; then `X FILE` and `L` bring it into memory. Fetch anything binary or long
  with `> FILE`. Rules: MONITOR_SPEC 6.18.
- **`Q text`** asks Claude one question and prints the answer. Each `Q` stands alone: Claude does not
  see earlier questions. The API key lives on the Pi, never in the ROM (PI_DAEMON 11). Rules:
  MONITOR_SPEC 6.19.
- **Esc** stops a running `N` or `Q`: the request is cancelled and `Aborted` prints after whatever
  was already printed. In the stream form `N` checks for Esc at each line end, so a long body without
  line ends (a binary, minified HTML or JSON) cannot be stopped: fetch those with `> FILE`, which
  Esc always stops, leaving `FILE` untouched. Keys typed while the request runs are thrown away.
  Arrow and function keys start with Esc too: over TCP or piped stdin they abort, and their tail
  (`[A`) may reach the next prompt. Rules: MONITOR_SPEC 6.18.

---

## 8. Debugging

### 8.1 The emulator debugger

Ctrl-E (or `--debug`) opens `dbg>`, and `?` there lists its commands with their syntax. You will use
breakpoints at ROM labels (`rom/monitor.sym`), step and continue, registers, memory, disassembly,
watchpoints on memory and ports, the instruction ring and the port trace. Full contract: ARCHITECTURE
7.4. A `--script` file runs the same commands.

### 8.2 Registers from the monitor

After a program started with `G` returns with `RET`, `R` prints the registers it returned with
(MONITOR_SPEC 6.20). Copied from `tests/transcripts/registers.txt`:

```
> :0F030000215644E5F1010D0B113412218100C982
> G 0300
> R
A=44 F=56 BC=0B0D DE=1234 HL=0081
```

When `R`'s line is captured, and what else changes it: MONITOR_SPEC 6.20.

To stop mid-program, at any call depth, plant a breakpoint (MONITOR_SPEC 8.1):

1. `E aaaa`, note the byte it shows, type `F7` (`RST 6`), Enter, then `.` (`.` alone discards the
   digits: only Enter stores). `A aaaa` / `RST 6` / `.` does the same.
2. `G` the program as usual. When it reaches aaaa the monitor prints `BRK aaaa` and the prompt.
3. `R` shows the registers at the break. SP is not saved, and the program cannot be continued.
4. `E aaaa`, type the noted byte back, Enter, `.`.

Copied from `tests/transcripts/breakpoint.txt` (F7 over the `RET` of the program above):

```
> :0F030000215644E5F1010D0B113412218100C982
> :01030E00F7F7
> G 0300
BRK 030E
> R
A=44 F=56 BC=0B0D DE=1234 HL=0081
```

A breakpoint is for debugging only: a program still ends with `RET` (4.1). In the emulator the
debugger (8.1) does more.

### 8.3 Port traces: emulator against board

Both write the ARCHITECTURE 7.3 format: `t FILE` in the debugger, `pi8080d --trace FILE` on the Pi.
Run the same ROM and input on both and diff them. The filter that makes them comparable is in
ARCHITECTURE 7.4 (Port trace); it drops the empty console polls (`IN 02 02`) that `N` and `Q`
interleave with their BUSY status reads, so a long `N` or `Q` makes a long trace.

---

## 9. The Pi Daemon

`pi8080d` is the emulator's port map behind GPIO instead of behind the CPU model (PI_DAEMON 1).

### 9.1 Without a board: `--sim`

On the Mac, from the repo:

```
cargo run --bin pi8080d -- --sim rom/monitor.bin --storage /tmp/pi8080d
socat -,rawer,escape=0x1d TCP:localhost:8080          # in a second terminal; Ctrl-] quits
```

Press Enter for a prompt: the banner went out before you connected. There is no RESET under `--sim`.
Restarting the daemon is the power cycle (PI_DAEMON 16).

### 9.2 On the Pi

Build on the Mac and copy it over (PI_DAEMON 2, 11):

```
rustup target add aarch64-unknown-linux-musl     # once
cargo build --release --target aarch64-unknown-linux-musl --bin pi8080d
scp target/aarch64-unknown-linux-musl/release/pi8080d pi:/usr/local/bin/
```

Install `scripts/pi8080d.service` as `/etc/systemd/system/pi8080d.service`. The Pi setup it needs
(user, `config.txt`, `cmdline.txt`, time zone, API key) is PI_DAEMON 11. With
`scripts/pi8080d-sim.conf` installed as a drop-in, the same unit runs `--sim` on the Pi (PI_DAEMON 16.4).

### 9.3 The console

The daemon listens on `127.0.0.1:8080`. From the Mac:

```
ssh -L 8080:localhost:8080 pi
socat -,rawer,escape=0x1d TCP:localhost:8080
```

A new connection replaces the old one (PI_DAEMON 7.1). For scripted input, see section 4.3. Ctrl-C
reaches the 8080 as byte 03, which the monitor ignores.

---

## 10. Changing the ROM

```
cd rom && make          # monitor.bin, monitor.sym, monitor_ram.hex; commit all three
make size               # bytes used of 4096
```

1. **Emulator:** `cargo test` runs every transcript against the new `monitor.bin`.
2. **Board, without burning:** the RAM test build (ARCHITECTURE 2.1). With the resident monitor at
   its prompt, paste `rom/monitor_ram.hex` into the socat session, then type `G D000`; from a script,
   use the `nc` form in 4.3 and wait for the banner. The banner line ends ` RAM` (MONITOR_SPEC 1.1).
   While it runs the user area is smaller (ARCHITECTURE 2.1). `G F000` or RESET goes back to the ROM.
3. **Burn in circuit,** with no external programmer: `examples/burn` writes F000-FFFF through
   jumper JP-WE (ARCHITECTURE 6.10). Rehearse it in the emulator first: copy `monitor.bin` to
   `storage/MONITOR.BIN`, `cargo run -- --jp-we`, the same steps (the burned image lives in
   memory only).

   ```
   cd rom && make                                    # on the Mac
   scp monitor.bin pi:/var/lib/pi8080d/MONITOR.BIN   # emulator: cp monitor.bin ../storage/MONITOR.BIN
   ```

   Then at the monitor:

   ```
   X MONITOR.BIN
   L 0 1000 1000           the image to 1000-1FFF
                           paste examples/burn.hex (4.3)
                           fit JP-WE
   G 0100                  about 1 s on the board: type nothing until the banner
                           the new banner: remove JP-WE at its first prompt
   C F000 FFFF 1000        no output: the ROM is the image
   ```

   - **The banner** is the new monitor cold-starting: the burn worked. Remove JP-WE before anything
     else: while it is fitted, any write to F000-FFFF reprograms the ROM (ARCHITECTURE 6.10 rule 1),
     and keys typed during the burn run on the new monitor.
   - **`Not a ROM image`:** nothing was written. Byte 0 of the image is not 31 or byte 6 is not F0
     (ARCHITECTURE 3.2 requirement 7). A RAM test build has D0 there: burn `monitor.bin`.
   - **`Burn failed aaaa`:** the byte at aaaa did not verify, and the program spins until RESET. With
     JP-WE open nothing was written: RESET, fit JP-WE, `G 0100` again (RAM survives RESET,
     ARCHITECTURE 3.1). With JP-WE fitted the ROM is part new, part old: use the external
     programmer (step 4).
   - The image is read from 1000. Elsewhere: `E 0103`, the address low byte first.
4. **External programmer** (HARDWARE_BUILD): the first image, and recovery from a failed burn. Then
   check it (6.3).

---

## 11. When Something Is Wrong

### 11.1 A monitor message

What each message means: MONITOR_SPEC 5. What to do:

| You see | Do |
|---|---|
| `Unknown command. Type ? for help.` | `?` |
| `No storage mounted` | `X NAME` |
| `Storage error` | Check the Pi's disk, then `X NAME` again |
| `Service error` | `T`: wait for NTP. `Q`: check the key (PI_DAEMON 11). Otherwise `journalctl -u pi8080d` |
| `Aborted` | Esc (or an arrow key) during `N` or `Q`. Run it again; fetch a long or binary body with `> FILE` |
| `BRK aaaa` | The program reached an F7 you planted at aaaa (8.2). `R`, then put the byte back: `E aaaa`, the byte, Enter, `.` |
| `Address out of range` on a HEX line | Move the program into the user area; on the RAM test build, `G F000` first |
| `Checksum error`, `Bad record` | Paste that line again |

### 11.2 Everything else

| You see | Why | Do |
|---|---|---|
| Nothing after connecting | The banner went out before you connected | Press Enter |
| No prompt at all on the board | The 8080 waits under READY for the daemon | Start `pi8080d`; check `journalctl -u pi8080d` |
| `HLT at PC=xxxx` and a piped emulator run exits, or `* halt` at `dbg>` in an interactive one | The program executed HLT | End programs with `RET` (4.1); `q` quits the debugger |
| A program never returns | It loops | RESET (board), Ctrl-C (emulator), restart (`--sim`) |
