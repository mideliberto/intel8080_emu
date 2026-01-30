# Intel 8080 Emulator

## What
8080 emulator in Rust with monitor ROM. Learning Rust, building toward real hardware.

## Status
- Phase 4 complete (24-bit storage, 16MB)
- Phase 5 next (Intel HEX loader)
- 191 tests passing
- Monitor ROM v0.3 with 14 commands

## Key Files
- `docs/COLLABORATION_LOG.md` - Full project history, decisions, lessons
- `docs/ARCHITECTURE.md` - Memory map, boot sequence, ROM overlay
- `docs/IMPLEMENTATION_ROADMAP.md` - Phases and success criteria
- `docs/DEVICE_SPECS.md` - I/O port protocols
- `rom/monitor.asm` - Monitor ROM source

## Rules
- I write the implementation code (learning Rust)
- Challenge complexity, invoke the mantra
- Finish current phase before adding features
- `cargo test` frequently
- No debug statements in commits
