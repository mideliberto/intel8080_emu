# Intel 8080 Emulator

An Intel 8080 emulator in Rust with a monitor ROM. Period-appropriate architecture (1975 vintage) connected to modern infrastructure.

**The Vision:** An 8080 that talks to Claude over the API. Internet-connected vintage computing. Not a museum piece—a living system.

**The Mantra:** *"A fool admires complexity, genius admires simplicity."*

## Current Status

| Component | Status |
|-----------|--------|
| CPU core (documented opcodes + undocumented NOPs, flags, stack, I/O; known bugs in TODO.md Review findings) | ✅ |
| Memory subsystem with ROM overlay | ✅ |
| Console device | ✅ |
| Storage device (24-bit, 16MB) | ✅ |
| Monitor ROM v0.3 (14 commands) | ✅ |
| 204 tests (14 unit + 180 CPU + 10 integration) | ✅ |
| Intel HEX loader (Phase 5) | 🔲 Next |
| Service Mailbox: time, HTTP, Claude (Phases 6-9) | 🔲 Future |

## Monitor Commands

```
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
W mem stor [cnt]      - Write memory to storage
X [file | -]          - Mount/unmount storage
?                     - Help
```

Coming: `:` Intel HEX records pasted at the prompt (Phase 5), then T (time), A/U (assemble/unassemble), N (HTTP GET), Q (ask Claude), R (registers). Full contract for every command, argument and message: [docs/MONITOR_SPEC.md](docs/MONITOR_SPEC.md).

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
```

## Running

```bash
cargo run
```

You'll see:
```
8080 Emulator
Built: 2026-10-02 19:20:00        <- emulator build time (build.rs)

8080 Monitor v0.3
Built: 10/02/2026 19:13:00        <- ROM assembly time (asl DATE/TIME)
Ready.
> 
```

Ctrl-C quits the emulator. Host key mapping: [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md).

## ROM Development

The monitor ROM uses the AS macro assembler (Alfred Arnold).

```bash
cd rom
make
```

## ROM Overlay Boot

S-100 style boot: RESET starts the CPU at 0x0000 with the ROM at 0xF000 mirrored there for reads. The ROM jumps into F000+, then any `OUT 0xFE` turns the mirror off and low memory becomes RAM. On hardware it is one 74HCT74 half. Detail: [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md).

## Project Structure

```
src/
├── main.rs              # Entry point
├── lib.rs               # Library exports
├── cpu.rs               # 8080 CPU emulation
├── memory.rs            # Memory trait
├── registers.rs         # Register enums, flags
└── io/
    ├── mod.rs
    ├── bus.rs           # I/O port mapping
    ├── device.rs        # IoDevice trait
    └── devices/
        ├── console.rs       # Terminal I/O
        ├── storage.rs       # 24-bit linear storage
        ├── storage_mount.rs # File mounting service
        ├── test_console.rs  # Scripted testing
        ├── timer.rs         # Interim timer (deletion pending, TODO.md)
        └── null.rs

rom/
├── Makefile
├── monitor.asm          # Monitor ROM source
└── monitor.bin          # Compiled ROM (4KB)

examples/
└── hello.asm            # Example 8080 program

scripts/
└── zip_source.sh

storage/                 # Mounted storage files

docs/
├── ARCHITECTURE.md          # Normative: memory map, boot, overlay, CPU, hardware
├── DEVICE_SPECS.md          # Normative: port protocols, READY
├── MONITOR_SPEC.md          # Normative: monitor commands, HEX loader
├── QUICK_REFERENCE.md       # Cheat sheet
├── HARDWARE_BUILD.md         # Build plan: BOM, bring-up, Pi service
├── IMPLEMENTATION_ROADMAP.md
├── COLLABORATION_LOG.md
├── reference/               # 8080 instruction set, I/O references
│   └── 8080_HARDWARE.md     # 8080A/8224/8228 pins, levels, bus timing
└── archive/                 # Superseded docs, kept for history

tests/
├── cpu_tests.rs         # 180 CPU instruction tests
├── monitor_tests.rs     # 10 integration tests
└── common/
    └── mod.rs           # Test utilities
```

## I/O Port Map

Ports 0x00-0x6F are the Pi window: console 0x00-0x02, storage 0x08-0x0C, mount 0x0D-0x0F, Service Mailbox 0x10-0x13 (Phase 6). Every access waits on READY until the Pi completes it. 0xFE/0xFF are local overlay control and status. Register-level detail: [docs/DEVICE_SPECS.md](docs/DEVICE_SPECS.md).

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

Working docs:
- [`docs/QUICK_REFERENCE.md`](docs/QUICK_REFERENCE.md) — Cheat sheet
- [`docs/HARDWARE_BUILD.md`](docs/HARDWARE_BUILD.md) — Hardware build plan: BOM, bring-up, sourcing, Pi service (non-normative)
- [`docs/reference/8080_HARDWARE.md`](docs/reference/8080_HARDWARE.md) — 8080A, 8224, 8228 hardware reference (MCS-80 User's Manual)
- [`docs/IMPLEMENTATION_ROADMAP.md`](docs/IMPLEMENTATION_ROADMAP.md) — Phases and plans
- [`docs/COLLABORATION_LOG.md`](docs/COLLABORATION_LOG.md) — History, decisions, lessons
- [`TODO.md`](TODO.md) — Task list

## License

MIT
