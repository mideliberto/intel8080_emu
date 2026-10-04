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
| Monitor ROM v0.9 (20 commands + Intel HEX loader) | ✅ |
| Host-side debugger (breakpoints, watchpoints, I/O breaks, port trace, trace ring, ROM symbols) | ✅ |
| 378 tests (6 library + 13 host + 139 CPU + 37 device + 56 mailbox + 73 monitor + 18 Pi daemon + 17 debugger + 8 terminal + 4 GAL + 7 netlist), plus 9 `#[ignore]` (4 exercisers, 3 GET time limits, 1 live ASK, 1 measurement) | ✅ |
| Intel HEX loader (Phase 5) | ✅ |
| Service Mailbox (ports 10-13) and `TIME` / T (Phase 6) | ✅ |
| Mailbox `ASM`/`DIS`, A and U (Phase 7) | ✅ |
| Pi daemon `pi8080d` ([PI_DAEMON](docs/PI_DAEMON.md)): every transcript passes through it on a simulated board; `--sim` runs the whole Pi stack with the CPU model before the board exists; static aarch64 binary links | ✅ Code; 🔲 bench |
| RAM test build of the monitor (`rom/monitor_ram.hex`, `G D000`): ROM changes on the board without a burn ([ARCHITECTURE](docs/ARCHITECTURE.md) 2.1) | ✅ |
| Mailbox `GET` and N: HTTP and HTTPS via `curl` on the Pi (Phase 8); Esc aborts N and Q (Phase 12) | ✅ |
| Mailbox `ASK` and Q: Claude via the Messages API, the key on the Pi (Phase 9) | ✅ |
| R: the registers a program left at its `G` return (Phase 10) or at an `RST 6` breakpoint (Phase 12) | ✅ |
| Example programs (`examples/hello`, `examples/memtest`) and the user guide (Phase 11) | ✅ |
| In-circuit ROM burn: `examples/burn` through jumper JP-WE, rehearsed in the emulator with `--jp-we` ([USER_GUIDE](docs/USER_GUIDE.md) 10, Phase 12) | ✅ Code; 🔲 bench |

## Monitor Commands

Command summary: [docs/QUICK_REFERENCE.md](docs/QUICK_REFERENCE.md); full contract: [docs/MONITOR_SPEC.md](docs/MONITOR_SPEC.md).

## Quick Start

    cargo run                      # the monitor prompt; Ctrl-C quits, Ctrl-E opens the debugger
    > ?                            # the command list

    cargo run -- --jp-we           # JP-WE fitted: a program can rewrite the ROM image (ARCHITECTURE 6.10)

Paste `examples/hello.hex` at the prompt, then `G 0100`. Everything else (loading your own programs,
saving to storage, the debugger, the Pi daemon, the RAM test build): [docs/USER_GUIDE.md](docs/USER_GUIDE.md).

## Building

```bash
cargo build
cargo test

# CPU exercisers (8080EXM takes about 20 s)
scripts/fetch_exercisers.sh
cargo test --release --test exerciser -- --ignored --nocapture

# Mailbox GET time limits (about 35 s); the live ASK test skips without a key
cargo test --test mailbox_tests -- --ignored

# Live ASK, once by hand (needs the network and a key)
ANTHROPIC_API_KEY=... cargo test --test mailbox_tests ask_live -- --ignored
```

Mailbox `GET` and `ASK` (the N and Q commands) run `/usr/bin/curl` 8.4.0 or later; macOS ships it. Their tests use a local test server, never the internet or the API, with or without `ANTHROPIC_API_KEY` exported. Q needs `ANTHROPIC_API_KEY` in the environment of `cargo run` (or of `pi8080d`: [docs/PI_DAEMON.md](docs/PI_DAEMON.md) 11); without it Q prints `Service error`.

The Pi daemon `pi8080d` (Linux; with `--sim`, also macOS): cross-build, deploy, `--sim`, and the TCP console are in [docs/USER_GUIDE.md](docs/USER_GUIDE.md) section 9 (one copy; the normative rules are [docs/PI_DAEMON.md](docs/PI_DAEMON.md)).

## ROM Development

Build, test, try on the board without burning, burn and check: [docs/USER_GUIDE.md](docs/USER_GUIDE.md) section 10.

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
        ├── mailbox.rs       # Service Mailbox 10-13: TIME, ASM, DIS, GET, ASK
        ├── ask.rs           # ASK: prompt rules, curl request, SSE reader, wrap (ask_system.txt: the system prompt)
        └── storage.rs       # Storage and mount 08-0F: 24-bit linear storage

rom/
├── Makefile
├── monitor.asm          # Monitor ROM source
├── monitor.bin          # Compiled ROM (4KB)
├── monitor.sym          # ROM labels for the debugger ('AAAA NAME')
└── monitor_ram.hex      # RAM test build at D000 (Intel HEX; paste, then G D000)

examples/
├── Makefile             # make: NAME.hex from NAME.asm (asl + p2hex)
├── burn.asm / .hex      # Programs the ROM from an image in RAM (JP-WE fitted)
├── hello.asm / .hex     # Prints a line, returns to the monitor
└── memtest.asm / .hex   # RAM test over a range

hw/
├── Makefile             # make: glue.jed from glue.pld (galette 0.3.0)
├── glue.pld             # GAL source: the only home of the GAL pinout (ARCHITECTURE 6.2-6.5)
├── glue.jed             # GAL fuse map: burned, and checked by tests/gal_tests.rs
├── board.net.txt        # Board netlist: the only home of pin numbers (HARDWARE_BUILD 2.2)
├── board.kicad.net      # KiCad netlist written from it by tests/netlist_tests.rs (Pcbnew imports this)
├── board.kicad_pro      # KiCad project (minimal), so Pcbnew loads board.kicad_dru
├── board.kicad_pcb      # The board: 160 x 120 mm outline; placement and routing are Mike's
└── board.kicad_dru      # Design rules: fab minimums, rail widths

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
├── USER_GUIDE.md            # Operating the machine (non-normative)
├── QUICK_REFERENCE.md       # Cheat sheet
├── HARDWARE_BUILD.md         # Build plan: BOM, bring-up, Pi platform decisions
├── PARTS_ORDER.md            # What to buy: order lines keyed to the netlist refdes
├── IMPLEMENTATION_ROADMAP.md
├── COLLABORATION_LOG.md
├── reference/               # 8080 instruction set, I/O references
│   └── 8080_HARDWARE.md     # 8080A/8224/8228 pins, levels, bus timing
└── archive/                 # Superseded docs, kept for history

tests/
├── cpu_tests.rs         # CPU: reference-model flags, opcode cycle/length table, branches, wrap
├── device_tests.rs      # Console, storage and mount at port level
├── mailbox_tests.rs     # Service Mailbox at port level (DEVICE_SPECS 8), black-box from the spec
├── debugger_tests.rs   # Debugger: --script runs, breaks, watchpoints, trace (ARCHITECTURE 7.4)
├── gal_tests.rs         # GAL: hw/glue.jed for every input against the emulator's decode and ARCHITECTURE 6.2-6.5
├── netlist_tests.rs     # Board: hw/board.net.txt pad by pad against ARCHITECTURE 6; KiCad netlist, project files, routed board, parts order
├── exerciser.rs         # TST8080, 8080PRE, CPUTEST, 8080EXM under a CP/M shim (#[ignore])
├── monitor_tests.rs     # Strict transcript harness: junk RAM, exact output to each prompt; every transcript also through the Pi daemon, some through pi8080d --sim and the RAM test build
├── pi_daemon_tests.rs   # Pi daemon: RESET, faults, startup, stop, TCP console on the simulated board
├── terminal_tests.rs    # The real binary under a pty: raw mode, key map, Ctrl-C/Ctrl-E, HLT prompt (Unix)
├── support/            # Shared test code: mod.rs, http.rs (local HTTP server for GET and ASK; never the internet or the API)
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
- [`docs/USER_GUIDE.md`](docs/USER_GUIDE.md) — Operating the machine: emulator, Pi daemon, loading, saving, debugging, ROM changes
- [`docs/QUICK_REFERENCE.md`](docs/QUICK_REFERENCE.md) — Cheat sheet
- [`docs/HARDWARE_BUILD.md`](docs/HARDWARE_BUILD.md) — Hardware build plan: BOM, bring-up, sourcing, Pi platform decisions (non-normative)
- [`docs/PARTS_ORDER.md`](docs/PARTS_ORDER.md) — Parts order: MPNs, distributor numbers, spares, lifecycle status, keyed to the netlist refdes (non-normative)
- [`docs/reference/8080_HARDWARE.md`](docs/reference/8080_HARDWARE.md) — 8080A, 8224, 8228 hardware reference (MCS-80 User's Manual)
- [`docs/IMPLEMENTATION_ROADMAP.md`](docs/IMPLEMENTATION_ROADMAP.md) — Phases and plans
- [`docs/COLLABORATION_LOG.md`](docs/COLLABORATION_LOG.md) — History, decisions, lessons
- [`TODO.md`](TODO.md) — Task list

## License

MIT
