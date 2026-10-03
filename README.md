# Intel 8080 Emulator

An Intel 8080 emulator in Rust with a monitor ROM. Period-appropriate architecture (1975 vintage) connected to modern infrastructure.

**The Vision:** An 8080 that talks to Claude over the API. Internet-connected vintage computing. Not a museum piece—a living system.

**The Mantra:** *"A fool admires complexity, genius admires simplicity."*

## Current Status

| Component | Status |
|-----------|--------|
| CPU core (all 256 opcodes, 8080A flags, interrupt input, HLT; passes TST8080, 8080PRE, CPUTEST, 8080EXM) | ✅ |
| Memory subsystem with ROM overlay | ✅ |
| Console device | ✅ |
| Storage device (24-bit, 16MB) | ✅ |
| Monitor ROM v0.6 (17 commands + Intel HEX loader) | ✅ |
| Host-side debugger (breakpoints, watchpoints, I/O breaks, port trace, trace ring, ROM symbols) | ✅ |
| 300 tests (13 host + 130 CPU + 37 device + 34 mailbox + 44 monitor + 18 Pi daemon + 16 debugger + 8 terminal), plus 4 exercisers (`#[ignore]`) | ✅ |
| Intel HEX loader (Phase 5) | ✅ |
| Service Mailbox (ports 10-13) and `TIME` / T (Phase 6) | ✅ |
| Mailbox `ASM`/`DIS`, A and U (Phase 7) | ✅ |
| Pi daemon `pi8080d` ([PI_DAEMON](docs/PI_DAEMON.md)): every transcript passes through it on a simulated board; `--sim` runs the whole Pi stack with the CPU model before the board exists; static aarch64 binary links | ✅ Code; 🔲 bench |
| RAM test build of the monitor (`rom/monitor_ram.hex`, `G D000`): ROM changes on the board without a burn ([ARCHITECTURE](docs/ARCHITECTURE.md) 2.1) | ✅ |
| Mailbox: HTTP, Claude (Phases 8-9) | 🔲 Future |

## Monitor Commands

```
A addr                - Assemble, one instruction per line ('.' ends)
C start end dest      - Compare memory regions
D [start] [end]       - Dump memory
E [addr]              - Examine/modify memory
F start end val       - Fill memory
G [addr]              - Go (execute at address)
H num1 num2           - Hex math (sum, difference)
I port                - Input from I/O port
L stor mem [cnt]      - Load from storage to memory
M src dst cnt         - Move memory block
O port val            - Output to I/O port
S start end bytes     - Search for pattern
T                     - Show time (YYYY-MM-DD HH:MM:SS)
U addr [cnt]          - Unassemble cnt instructions (default 8)
W mem stor [cnt]      - Write memory to storage
X [file | -]          - Mount/unmount storage
:LLAAAATT..CC         - Intel HEX record (paste at the prompt)
?                     - Help
```

Coming: N (HTTP GET), Q (ask Claude), R (registers). Full contract for every command, argument and message: [docs/MONITOR_SPEC.md](docs/MONITOR_SPEC.md).

## Storage System

24-bit linear-addressed storage with 16MB address space. No sectors, no tracks—just bytes.

```
> X DATA.BIN
Mounted
> L 0 1000 100           ; Load 256 bytes from storage:0x000000 to mem:0x1000
Loaded
> W 2000 10000 80        ; Write 128 bytes from mem:0x2000 to storage:0x010000
Written
> X -
Unmounted
```

Storage addresses take up to 6 hex digits (24-bit). Mounting a missing file creates it. Protocol: [docs/DEVICE_SPECS.md](docs/DEVICE_SPECS.md).

## Building

```bash
cargo build
cargo test

# CPU exercisers (8080EXM takes about 20 s)
scripts/fetch_exercisers.sh
cargo test --release --test exerciser -- --ignored --nocapture
```

The Pi daemon `pi8080d` (Linux; with `--sim`, also macOS) cross-builds on the Mac as a static binary, with the linker Rust ships:

```bash
rustup target add aarch64-unknown-linux-musl     # once
cargo build --release --target aarch64-unknown-linux-musl --bin pi8080d
scp target/aarch64-unknown-linux-musl/release/pi8080d pi:/usr/local/bin/
```

On the Pi: `pi8080d --storage DIR [--listen ADDR:PORT] [--trace FILE] [--sim FILE]`, or the unit `scripts/pi8080d.service`. `--sim rom/monitor.bin` runs the emulated 8080 on a simulated board instead of GPIO, on the Pi or the Mac (`cargo run --bin pi8080d -- --sim rom/monitor.bin --storage /tmp/pi8080d`); the unit drop-in is `scripts/pi8080d-sim.conf` ([docs/PI_DAEMON.md](docs/PI_DAEMON.md) 16). The console is TCP, `127.0.0.1:8080` by default: `ssh -L 8080:localhost:8080 pi`, then `socat -,rawer,escape=0x1d TCP:localhost:8080`. Deployment: [docs/PI_DAEMON.md](docs/PI_DAEMON.md) 11.

## Running

```bash
cargo run
```

You'll see:
```
8080 Emulator
Built: 2026-10-02 19:20:00        <- emulator build time (build.rs)

8080 Monitor v0.6
Built: 10/02/2026 19:13:00        <- ROM assembly time (asl DATE/TIME)
Ready.
> 
```

Ctrl-C quits the emulator. Ctrl-E opens the debugger. Host key mapping: [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md). With stdin piped instead of a terminal, its bytes go straight to the console. A piped run ends only on HLT (`HLT at PC=xxxx`) or Ctrl-C; at end of input the monitor just waits at its prompt. A `--script` run also prints `HLT at PC=xxxx` and exits on a halt; see Debugger for how else it ends. In an interactive terminal run with no `--script`, HLT opens the debugger prompt instead (`q` quits). Example: `printf 'F 0200 0200 76\rG 0200\r' | cargo run`.

## Debugger

Host-side; the 8080 never sees it. Ctrl-E stops the CPU and opens a `dbg>` prompt; `cargo run -- --debug` starts stopped; `cargo run -- --script FILE` runs debugger commands from FILE first (each echoed as `dbg> ...`; a bad line exits with status 2). ROM labels from `rom/monitor.sym` work anywhere an address does, as `NAME` or `NAME+n`. Numbers are hex.

```
c                  continue              s [n]            step n instructions
r                  registers             m addr [len]     memory (D format)
u [addr] [n]       disassemble           b addr           breakpoint
w addr[-end] [r|w] watchpoint            io port [in|out] break on IN/OUT
bl / bc [addr]     list / clear breaks   t file|off       port trace to a file
ring [n]           last n steps          sym addr         address and label
?                  command summary       q                quit
```

A stop prints the reason, the last 8 steps from the trace ring, the registers and the next instruction:

```
dbg> b CMD_DUMP
dbg> c
> D 0200 020F
* break F28B CMD_DUMP
...
F05E  FE 44     CPI 44             A=44 F=12 BC=0B0D DE=0000 HL=0081 SP=F000
F060  CA 8B F2  JZ CMD_DUMP        A=44 F=56 BC=0B0D DE=0000 HL=0081 SP=F000
PC=F28B SP=F000 A=44 F=56 -ZAP- BC=0B0D DE=0000 HL=0081 INTE=0 OVL=0
CMD_DUMP:
F28B  CD 59 F1  CALL SKIP_SPACES
```

Full contract (formats, watchpoint and trace semantics): [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) section 7.4.

## ROM Development

The monitor ROM uses the AS macro assembler (Alfred Arnold).

```bash
cd rom
make          # monitor.bin, monitor.sym (debugger symbols) and monitor_ram.hex
              # (RAM test build at D000, ARCHITECTURE 2.1); commit all three
```

## ROM Overlay Boot

S-100 style boot: RESET starts the CPU at 0x0000 with the ROM at 0xF000 mirrored there for reads. The ROM jumps into F000+, then any `OUT 0xFE` turns the mirror off and low memory becomes RAM. On hardware it is one 74HCT74 half. Detail: [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md).

## Project Structure

```
src/
├── main.rs              # Host side: terminal, key map, run loop, debugger prompt
├── pi_main.rs           # pi8080d: arguments, signals, startup order, --sim
├── lib.rs               # Library exports
├── cpu.rs               # 8080 CPU emulation
├── debugger.rs          # Debugger: commands, breaks, watchpoints, trace ring, port trace
├── disasm.rs            # Opcode table: disassembler, DIS line, single-line assembler
├── registers.rs         # Register enums, flags
├── pi/
│   ├── mod.rs           # Pi daemon: Gpio trait, pin setup, bus loop, RESET, TCP console
│   ├── sim.rs           # SimBoard (the 8080 board at the GPIO register level) and Bridge: tests and --sim
│   └── linux.rs         # GpioMem (/dev/gpiomem, RESET line ioctls), NTP-gated clock
└── io/
    ├── mod.rs           # build_bus: the port map (devices in power-on state)
    ├── bus.rs           # I/O port mapping
    ├── device.rs        # IoDevice trait
    └── devices/
        ├── console.rs       # Console 00-02: input FIFO, output buffer
        ├── mailbox.rs       # Service Mailbox 10-13: TIME, ASM, DIS
        └── storage.rs       # Storage and mount 08-0F: 24-bit linear storage

rom/
├── Makefile
├── monitor.asm          # Monitor ROM source
├── monitor.bin          # Compiled ROM (4KB)
├── monitor.sym          # ROM labels for the debugger ('AAAA NAME')
└── monitor_ram.hex      # RAM test build at D000 (Intel HEX; paste, then G D000)

examples/
└── hello.asm            # Example 8080 program

scripts/
├── fetch_exercisers.sh  # Downloads the exercisers to tests/data/exercisers (pinned SHA-256)
├── pi8080d.service      # systemd unit for the Pi daemon
├── pi8080d-sim.conf     # systemd drop-in: pi8080d --sim
└── zip_source.sh

storage/                 # Mounted storage files

docs/
├── ARCHITECTURE.md          # Normative: memory map, boot, overlay, CPU, hardware
├── DEVICE_SPECS.md          # Normative: port protocols, READY
├── MONITOR_SPEC.md          # Normative: monitor commands, HEX loader
├── PI_DAEMON.md             # Normative: the Pi daemon (bus loop, RESET, TCP console, build, deploy)
├── QUICK_REFERENCE.md       # Cheat sheet
├── HARDWARE_BUILD.md         # Build plan: BOM, bring-up, Pi platform decisions
├── IMPLEMENTATION_ROADMAP.md
├── COLLABORATION_LOG.md
├── reference/               # 8080 instruction set, I/O references
│   └── 8080_HARDWARE.md     # 8080A/8224/8228 pins, levels, bus timing
└── archive/                 # Superseded docs, kept for history

tests/
├── cpu_tests.rs         # CPU: reference-model flags, opcode cycle/length table, branches, wrap
├── device_tests.rs      # Console, storage and mount at port level
├── mailbox_tests.rs     # Service Mailbox at port level (DEVICE_SPECS 8), black-box from the spec
├── exerciser.rs         # TST8080, 8080PRE, CPUTEST, 8080EXM under a CP/M shim (#[ignore])
├── monitor_tests.rs     # Strict transcript harness: junk RAM, exact output to each prompt; every transcript also through the Pi daemon, some through pi8080d --sim and the RAM test build
├── pi_daemon_tests.rs   # Pi daemon: RESET, faults, startup, stop, TCP console on the simulated board
├── terminal_tests.rs    # The real binary under a pty: raw mode, key map, Ctrl-C/Ctrl-E, HLT prompt (Unix)
└── transcripts/         # Monitor transcripts (data; also meant for hardware over the Pi console); ram/ only for the RAM test build
```

## I/O Port Map

Ports 0x00-0x6F are the Pi window: console 0x00-0x02, storage 0x08-0x0C, mount 0x0D-0x0F, Service Mailbox 0x10-0x13. Every access waits on READY until the Pi completes it. 0xFE/0xFF are local overlay control and status. Register-level detail: [docs/DEVICE_SPECS.md](docs/DEVICE_SPECS.md).

## The End Goal

The same ROM runs on:
- **Rust emulator** (now) — for development
- **Real 8080 + Pi coprocessor** (future) — for hardware

The 8080 doesn't know the difference. It sends bytes to ports, gets bytes back. Behind those ports: file storage, HTTP, Claude API. The coprocessor handles the complexity.

```
> Q What instructions does the 8080 have?
The 8080 has 256 opcodes covering data transfer, arithmetic,
logic, branching, stack operations, and I/O...
```

That's the vision. An 8080 that can ask questions.

## Documentation

Normative specs (code that differs from them is tracked in `TODO.md`):
- [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md) — Memory map, boot, overlay, CPU contract, hardware interface, host-side keys
- [`docs/DEVICE_SPECS.md`](docs/DEVICE_SPECS.md) — I/O port protocols, READY contract
- [`docs/MONITOR_SPEC.md`](docs/MONITOR_SPEC.md) — Monitor commands, line input, HEX loader, `G` return
- [`docs/PI_DAEMON.md`](docs/PI_DAEMON.md) — The Pi daemon `pi8080d`: bus loop, RESET, TCP console, TIME clock, build, deployment, tests

Working docs:
- [`docs/QUICK_REFERENCE.md`](docs/QUICK_REFERENCE.md) — Cheat sheet
- [`docs/HARDWARE_BUILD.md`](docs/HARDWARE_BUILD.md) — Hardware build plan: BOM, bring-up, sourcing, Pi platform decisions (non-normative)
- [`docs/reference/8080_HARDWARE.md`](docs/reference/8080_HARDWARE.md) — 8080A, 8224, 8228 hardware reference (MCS-80 User's Manual)
- [`docs/IMPLEMENTATION_ROADMAP.md`](docs/IMPLEMENTATION_ROADMAP.md) — Phases and plans
- [`docs/COLLABORATION_LOG.md`](docs/COLLABORATION_LOG.md) — History, decisions, lessons
- [`TODO.md`](TODO.md) — Task list

## License

MIT
