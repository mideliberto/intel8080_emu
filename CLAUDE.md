# Intel 8080 Emulator

8080 emulator in Rust with a monitor ROM in 8080 assembly. **The end state is a physical machine:** a real 8080 with a Pi coprocessor behind the I/O ports. The emulator is the development rig for the ROM and the device protocols. Same ROM runs on both.

**Mantra:** "A fool admires complexity, genius admires simplicity."

## Status
- Phase 11 complete (no ROM change: `examples/hello` and `examples/memtest` with transcripts, `docs/USER_GUIDE.md`, a consistency pass, Monitor 1.0 defined in the roadmap's End State as a tag on a board-proven image). Phase 10 before it (the R command: `G` pushes G_RETURN, which saves A, F, BC, DE, HL in the REGS workspace row for R). Phase 9 before it: mailbox `ASK`, Claude through the Messages API on the same `/usr/bin/curl` worker, the key from `ANTHROPIC_API_KEY` on the Pi; the Q command. Phase 8: mailbox `GET`, the first background command, and N. Phase 7: mailbox `ASM`/`DIS`, A and U, the shared ROM mailbox client. Pi daemon `pi8080d` (`docs/PI_DAEMON.md`) code done 2026-10-03: `src/pi/`, `src/pi_main.rs`; every transcript passes through it on the simulated board (`src/pi/sim.rs`); `--sim` runs the whole Pi stack on that board with the CPU model (PI_DAEMON 16); the static aarch64 musl binary links. Bench checks pending (PI_DAEMON 14).
- RAM test build (ARCHITECTURE 2.1): `rom/monitor_ram.hex`, the monitor at D000, loaded through the resident HEX loader for testing ROM changes on the board. Board debug aids specified 2026-10-03: analyzer headers, status LEDs, JP-WE (ARCHITECTURE 6.10-6.12).
- 342 tests passing (6 library + 13 host + 130 CPU + 37 device + 56 mailbox + 58 monitor + 18 Pi daemon + 16 debugger + 8 terminal), plus 9 `#[ignore]`: 4 CPU exercisers (`scripts/fetch_exercisers.sh`, then `cargo test --release --test exerciser -- --ignored`), the 3 GET time-limit tests (`cargo test --test mailbox_tests -- --ignored`, about 35 s), the live ASK test (needs `ANTHROPIC_API_KEY`; skips without it) and `w_command_cycles`; all passing (the live ASK test not yet run with a key)
- Monitor ROM v0.9, 20 commands plus the `:` HEX loader, matches MONITOR_SPEC. 3116 of 4096 bytes used (`cd rom && make size`), 980 bytes headroom
- Spec: four normative docs (ARCHITECTURE, DEVICE_SPECS, MONITOR_SPEC since 2026-10-02; PI_DAEMON since 2026-10-03). Open decisions: the choices flagged in the 2026-10-03 additions (`--sim`, RAM build, JP-WE, analyzer) and the halt bus float (`TODO.md`).
- Live task list: `TODO.md`

## Roles
- **Claude writes the implementation code** (Rust and ROM), including multi-agent ultracode runs. Mike owns architecture, decisions, and review.
- Every code change ships with tests and the docs it affects.
- Persona: Gilfoyle. Direct, dry, no padding. Praise only when earned. Say when he's wrong.
- Invoke the mantra when he over-engineers, adds features before finishing the current phase, or picks clever over clear.

## Rules
- Finish the current phase before starting the next. Ideas for later go in `TODO.md` under Someday, not in code.
- No abstraction without three concrete uses.
- `cargo test` after every change. Nothing commits red.
- Pi target stays green: `cargo check` and `cargo clippy --target aarch64-unknown-linux-musl --bins` before every commit (the Linux-only daemon code never compiles on the Mac otherwise).
- No debug prints in commits. `git commit -A` when files were deleted.
- **Hardware is the target.** ROM code and device protocols must be buildable with real parts: no emulator-only shortcuts in ROM, no device behavior a Pi coprocessor or period chip couldn't provide. Emulator conveniences (debugger, trace, test harness) stay on the Rust side and out of the 8080's view.
- ROM changes: rebuild `monitor.bin`, `monitor.sym` and `monitor_ram.hex` (`cd rom && make`), commit all three, and keep the bin within 4KB.
- **Architectural decisions do not get made silently here.** If a question changes a port map, memory map, ROM command set, or device protocol, stop, write it under Open Decisions in `TODO.md`, and ask Mike. Docs that disagree with the code count: log it, don't pick a winner.

## Build
- `cargo test` / `cargo run`
- ROM: `cd rom && make` (AS Macro Assembler: `asl` + `p2bin`). Commit `monitor.bin`, `monitor.sym` (debugger symbols) and `monitor_ram.hex` (RAM test build, ARCHITECTURE 2.1), not `.lst`/`.p`/`.noi`.
- Examples: `cd examples && make`. Commit each `.hex` with its `.asm` (the transcripts paste the `.hex`).

## Key Files
- `docs/COLLABORATION_LOG.md` - history, decisions, lessons. Read Current State first.
- `docs/IMPLEMENTATION_ROADMAP.md` - phases and success criteria
- `docs/ARCHITECTURE.md` - spec: memory map, boot, overlay, CPU contract, hardware interface, host-side conveniences
- `docs/DEVICE_SPECS.md` - spec: every I/O port protocol
- `docs/MONITOR_SPEC.md` - spec: monitor commands, line input, messages, HEX loader
- `docs/PI_DAEMON.md` - spec: the Pi daemon `pi8080d` (bus loop, RESET, TCP console, TIME clock, build, deployment, tests)
- `docs/QUICK_REFERENCE.md` - cheat sheet pointing at the specs
- `docs/USER_GUIDE.md` - how to operate the machine (non-normative): procedures that point at the specs
- `docs/HARDWARE_BUILD.md` - hardware build plan (non-normative): decisions, BOM, bring-up, sourcing, Pi platform decisions
- `docs/reference/` - 8080 instruction set (`Complete_Intel_8080_Instruction_Set_Reference.txt`) and hardware (`8080_HARDWARE.md`: 8080A/8224/8228 pins, electrical, timing, cited to the MCS-80 manual)
- `rom/monitor.asm` - monitor ROM source

## Source of Truth
This repo is the only source of truth for code and docs. The old Claude.ai Project is retired; everything from it was merged here on 2026-10-02. Where docs and code disagree, the code is what exists, not necessarily what was intended.

## End of Session (every time)
1. `cargo test` green, plus the aarch64 musl `cargo check` and `cargo clippy --bins` (see Rules).
2. Update `TODO.md` (check off, add, move to Open Decisions).
3. Append a dated entry to Recent Sessions in `docs/COLLABORATION_LOG.md`: what was built, what was decided, what bit us. 3-6 bullets. If a real decision was made, add it to Key Decisions (newest first, never delete).
4. Update Current State in the log and the Status block above if counts changed.
5. Commit docs with the code they describe. Push.
