# Intel 8080 Emulator

A cycle-counted Intel 8080 emulator with monitor ROM, written in Rust.

## Project Status

- ✅ CPU core (all 256 opcodes)
- ✅ Flag handling (S, Z, AC, P, C)
- ✅ I/O device framework
- ✅ 182 unit tests passing
- 🔲 Monitor ROM
- 🔲 Disk support
- 🔲 Timer/interrupts

## Building

```bash
cargo build
cargo test
```

## Running

```bash
cargo run
```

## ROM Development

The monitor ROM is developed in the `rom/` directory using the AS macro assembler.

```bash
cd rom
make
```

## Project Structure

```
src/
├── lib.rs          # Library exports
├── main.rs         # Entry point
├── cpu.rs          # 8080 CPU emulation
├── registers.rs    # Register enums and constants
├── memory.rs       # Memory trait
└── io/
    ├── mod.rs
    ├── bus.rs      # I/O port mapping
    ├── device.rs   # IoDevice trait
    └── devices/
        ├── console.rs
        ├── disk.rs
        ├── timer.rs
        └── null.rs

rom/
├── Makefile
└── monitor.asm     # Monitor ROM source

tests/
└── cpu_tests.rs    # CPU instruction tests
```

## The Mantra

> "A fool admires complexity, genius admires simplicity."

Keep it simple.
