# Intel 8080 Emulator

An Intel 8080 emulator in Rust with a monitor ROM. Period-appropriate architecture (1975 vintage) connected to modern infrastructure.

**The Vision:** An 8080 that talks to Claude over the API. Internet-connected vintage computing. Not a museum piece—a living system.

**The Mantra:** *"A fool admires complexity, genius admires simplicity."*

## Current Status

| Component | Status |
|-----------|--------|
| CPU core (all 256 opcodes, flags, stack, I/O) | ✅ |
| Memory subsystem with ROM overlay | ✅ |
| Console device | ✅ |
| Storage device (24-bit, 16MB) | ✅ |
| Monitor ROM v0.3 (14 commands) | ✅ |
| 204 tests (14 unit + 180 CPU + 10 integration) | ✅ |
| HTTP / Network | 🔲 Future |
| Claude API integration | 🔲 Future |

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

Storage addresses support up to 6 hex digits (24-bit). The high byte acts as a bank/page selector for organizing data within a single large file.

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
8080 Monitor v0.3
Built: 2025-12-20 ...
Ready.
> 
```

## ROM Development

The monitor ROM uses the AS macro assembler (Alfred Arnold).

```bash
cd rom
make
```

## ROM Overlay Boot

The emulator implements authentic S-100 style boot behavior:

1. CPU starts at PC=0x0000 on reset
2. ROM overlay makes 0x0000 mirror ROM at 0xF000
3. ROM disables overlay via `OUT 0xFE, 0x00`
4. Low memory becomes RAM

This is how real Altair/IMSAI systems booted. One ROM, hardware bank switching. The same mechanism will work on real hardware with a 74LS74 flip-flop.

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
        ├── timer.rs
        └── null.rs

rom/
├── Makefile
├── monitor.asm          # Monitor ROM source
└── monitor.bin          # Compiled ROM (4KB)

storage/                 # Mounted storage files

tests/
├── cpu_tests.rs         # 180 CPU instruction tests
├── monitor_tests.rs     # 10 integration tests
└── common/
    └── mod.rs           # Test utilities
```

## I/O Port Map

| Ports | Device |
|-------|--------|
| 0x00-0x02 | Console |
| 0x08-0x0C | Storage (24-bit address, data, status) |
| 0x0D-0x0F | Storage mount service |
| 0xFE-0xFF | System control |

## The End Goal

The same ROM runs on:
- **Rust emulator** (now) — for development
- **Real 8080 + Pi coprocessor** (future) — for hardware

The 8080 doesn't know the difference. It sends bytes to ports, gets bytes back. Behind those ports: file storage, HTTP, Claude API. The coprocessor handles the complexity.

```
> A What instructions does the 8080 have?
The 8080 has 256 opcodes covering data transfer, arithmetic,
logic, branching, stack operations, and I/O...
```

That's the vision. An 8080 that can ask questions.

## Documentation

Detailed docs live in `docs/`:
- `PROJECT_OVERVIEW.md` — Quick orientation
- `ARCHITECTURE.md` — Memory map, boot sequence
- `DEVICE_SPECS.md` — I/O device protocols
- `QUICK_REFERENCE.md` — Cheat sheets
- `IMPLEMENTATION_ROADMAP.md` — Phases and plans
- `COLLABORATION_LOG.md` — History, decisions, lessons

## License

MIT
