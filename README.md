```markdown
# Intel 8080 Emulator

A cycle-counted Intel 8080 emulator with monitor ROM, written in Rust.

## Project Status

- ✅ CPU core (all 256 opcodes)
- ✅ Flag handling (S, Z, AC, P, C)
- ✅ I/O device framework
- ✅ 181 unit tests passing
- ✅ Monitor ROM (v0.2)
  - C (compare memory)
  - D (dump memory)
  - E (examine/modify)
  - F (fill memory)
  - G (go/execute)
  - H (hex math)
  - I (input from port)
  - M (move memory)
  - O (output to port)
  - ? (help)
- 🔲 Additional monitor commands (S, R)
- 🔲 Disk support
- 🔲 Timer/interrupts
- 🔲 Internet services

## Building

```bash
cargo build
cargo test
```

## Running

```bash
cargo run
```

Starts the monitor ROM. You'll see:
```
8080 Monitor v0.2
Ready.
> 
```

## ROM Development

The monitor ROM is in `rom/` using the AS macro assembler.

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
├── registers.rs    # Register enums
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
├── monitor.asm     # Monitor ROM source
└── monitor.bin     # Compiled ROM

tests/
├── cpu_tests.rs    # CPU instruction tests
└── common/
    └── mod.rs      # Test utilities

examples/
└── hello.asm       # Example program
```

## The Mantra

> "A fool admires complexity, genius admires simplicity."

Keep it simple.
```