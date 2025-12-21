# 8080 Monitor ROM & Emulator

**The Mantra:** *"A fool admires complexity, genius admires simplicity."*

---

## What Is This?

An Intel 8080 emulator in Rust with a monitor ROM. Period-appropriate architecture (1975 vintage) connected to modern infrastructure.

**The Vision:** An 8080 that talks to Claude over the API. Internet-connected vintage computing. Big data for an 8-bit processor. Not a museum piece - a living system.

**Key Features:**
- Full 8080 CPU emulation (all 256 opcodes, cycle-accurate)
- 4KB Monitor ROM with classic command set
- ROM overlay boot mechanism (hardware-compatible)
- Linear-addressed storage (maps to real SD/EEPROM)
- Extensible I/O device architecture
- Future: HTTP client, Claude API integration, real-time data feeds

**End State:** The same ROM runs on the Rust emulator today and on real 8080 hardware with a Pi coprocessor tomorrow. The 8080 doesn't know the difference.

---

## Current Status

| Component | Status |
|-----------|--------|
| CPU core (256 opcodes, flags, stack, I/O, interrupts) | ✅ Done |
| Memory subsystem | ✅ Done |
| Console device | ✅ Done |
| Rust project structure | ✅ Done |
| Monitor ROM (11 commands) | ✅ Done |
| ROM overlay mechanism | ✅ Done |
| **Storage device** | 🔲 Phase 4 - Current |
| Network / HTTP | 🔲 Future |
| Claude API integration | 🔲 Future |
| Debugger | 🔲 Future |

**Tests:** 191 passing (181 CPU + 10 monitor integration)

---

## Monitor Commands (Implemented)

| Cmd | Syntax | Description |
|-----|--------|-------------|
| C | C start end dest | Compare memory |
| D | D [start] [end] | Dump memory |
| E | E [addr] | Examine/modify |
| F | F start end val | Fill memory |
| G | G [addr] | Go (execute) |
| H | H num1 num2 | Hex math (+/-) |
| I | I port | Input from port |
| M | M src dst cnt | Move memory |
| O | O port value | Output to port |
| S | S start end pat | Search memory |
| ? | ? | Help |

**Deferred:** R (registers) - needs return mechanism, implement when debugging requires it.

---

## Documentation Index

| Document | Purpose | When to Load |
|----------|---------|--------------|
| ARCHITECTURE.md | Memory map, boot, overlay, vectors | Writing memory/boot code |
| DEVICE_SPECS.md | All I/O device protocols | Implementing/debugging devices |
| DESIGN_DECISIONS.md | The 12 key decisions + rationale | Questioning past choices |
| IMPLEMENTATION_ROADMAP.md | Phases and success criteria | Planning sessions |
| QUICK_REFERENCE.md | Cheat sheet tables | Keep open while coding |
| CODE_TEMPLATES.md | Rust structure, Cargo.toml | Reference for project setup |
| MONITOR_IMPLEMENTATION_STATUS.md | Living status of ROM work | ROM development |
| PHASE4_STORAGE_PLAN.md | Storage implementation details | Phase 4 work |

---

## Quick Start

```bash
# Build and test
cargo build
cargo test

# Run the emulator
cargo run

# Build the ROM
cd rom
make
```

---

## Repository

GitHub: https://github.com/mideliberto/intel8080_emu
