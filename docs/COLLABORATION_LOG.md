# Collaboration Log

The "memory" that maintains continuity between Claude instances. Full historical record.

---

## Project Identity

**Name:** Intel 8080 Emulator with Monitor ROM
**Creator:** Mike
**Co-Creator:** Claude (Anthropic)
**Started:** September 2025
**Repository:** https://github.com/mideliberto/intel8080_emu

**The Mantra:** "A fool admires complexity, genius admires simplicity."

**The Vision:** An 8080 that talks to Claude over the API. Internet-connected vintage computing. Big data for an 8-bit processor. Not a museum piece—a living system. Same ROM runs on Rust emulator today and real 8080 hardware with Pi coprocessor tomorrow.

**End State Goal:** An 8080 that:
1. Boots from ROM overlay (like real S-100 systems)
2. Stores data to 16MB files (maps to SD card)
3. Fetches data from the internet (via coprocessor)
4. Talks to Claude for assistance (maintaining project context)
5. Runs on real hardware (with Pi coprocessor)

---

## Mike's Profile

**Background:**
- Self-taught programmer since age 6-7 (MS BASIC)
- Languages: BASIC, Java, C/C++, Assembly
- CS minor from UIUC
- Read "Operating System Design and Implementation" growing up
- Grew up with 386/486 systems
- New to Rust (this project is the learning vehicle)

**Tendencies to Watch:**
- Over-engineers solutions (documented, self-aware about it)
- Gets excited about future features before finishing current work
- Perfectionist ("complete and well thought out" vs MVP)
- Late-night idea generation that needs morning review
- 2AM spec documents and feature creep

**What Works:**
- Direct feedback, no sugar-coating
- Being challenged on complexity
- Gilfoyle-style dry humor (from Silicon Valley)
- Reminders of the mantra when straying
- Prefers writing implementation code himself (learning project)
- Frequent `cargo test` cycles to validate progress
- "Write it down, sleep on it, pick ONE thing for tomorrow"

**Workflow Preferences:**
- Uses web interface for planning and architecture discussions
- Writes implementation code himself to learn Rust
- Runs `cargo test` frequently to validate changes
- Keeps debug statements out of final commits

---

## Key Decisions

Ordered newest to oldest. Never delete—only add.

### 2026-10-02: Hardware Alignment: Buildable, One Hard Fix
**Decision:** Accept the hardware-alignment pass. The changes that matter:
- The WAIT flip-flop is set from the 8224 STSTB, AND NOT RESET, AND D4/D6, AND the port window, not from SYNC.
- REQ = WAIT-FF Q AND the 8080 WAIT pin, so the Pi only sees REQ in T_W, after OUT data is valid.
- DIR = 8228 /I/OR, which deletes the status latch.
- After ACK, the Pi reads ACK back and blanks for at least 500 ns.
- 20 Pi GPIOs (A7 is redundant). Glue is an ATF22V10C GAL plus 74HCT74/14/08/125.
- Parts: AT28C64B ROM and 2x AS6C62256 RAM. Pololu 12 V boost, ICL7660 and a VBB clamp, with the Pi on its own supply. DS1813 reset supervisor. One 2-layer PCB.
- Pi side: Pi 4B busy-polling /dev/gpiomem, console over TCP with a 2 MiB output buffer. On RESET the Pi abandons the in-flight request at assertion and rebuilds devices at release. Device reset means rebuilding the IoBus.
- Port-trace line format: `IN pp vv` / `OUT pp vv` / `RESET`. No hardware single-step in v1.

**Problem:** SYNC has max-only delays, so it isn't a valid-status window. A memory write with D4/D6 set, to an address whose low byte is 00-6F, could phantom-set WAIT. REQ rose about 460 ns before OUT data, so the Pi would have latched status byte 10h.
**Evidence:** Four of five specialists found the SYNC hazard independently, and the new MCS-80 reference (docs/reference/8080_HARDWARE.md) found it separately. Datasheet budgets: RDYIN is low by STSTB+75..130 ns against the 8224's 167 ns deadline.
**Mantra:** Rejected: a status latch, a DIR flip-flop, an RP2040 front end, hardware single-step, bus buffers, and a 1 µs software delay. The fix removes parts.

### 2026-10-02: Emulator Shape Follows the Hardware
**Decision:**
- `reset()` models only the RESET pin: PC, INTE, halt, overlay and the pending interrupt. `new()` calls `reset()`. Test harnesses fill RAM with junk.
- Storage and StorageMount merge into one device serving 08-0F.
- Console becomes one device (input queue plus output buffer, no crossterm). Host polling, the key map and the hotkeys move to main.rs, and TestConsole is deleted.
- The 8080 exercisers are fetched by a pinned-hash script and run as an ignored test; the binaries are not committed (GPL and unknown licenses).

**Problem:** Zeroed RAM plus a friendly `reset()` let the monitor boot through a 61,440-NOP sled with the overlay broken, and it hid a missing `LXI SP` and missing workspace init. Every test still passed. The device layer the Pi is meant to reuse imported crossterm and wasn't Send.
**Evidence:** The code review's mutation testing (cargo-mutants 70.5%; ROM 10.6%) and the exercisers (8080EXM fails 11/24, all in the AC family).

### 2026-10-02: Debugger Pulled Forward, Before Phase 5
**Decision:** Build a host-side debugger before Phase 5. Core: break, step, continue, registers, memory, disassemble. It also has memory watchpoints, I/O port breaks and a port trace, an instruction trace ring buffer, and ROM symbols from the asl listing. Interface: Ctrl-E opens a `dbg>` line prompt, and the same parser takes a script, so tests can drive the debugger. Host-only; the 8080 never sees it. Pi-assisted hardware stepping is evaluated in the hardware-alignment pass.
**Mike's input:** "We should also implement a robust, useful, pragmatic debugger."
**Rationale:** Phase 5 is ROM assembly. The bug classes this review keeps finding (workspace and stack clobbers, port-protocol ordering, wandering PCs) are each one watchpoint, I/O break or trace away. The debugger is tooling for the current phase, not a feature.
**Mantra check:** Phase 10 shrinks to whatever is left. No TUI, no new dependencies.

### 2026-10-02: Spec Solidified Into Three Normative Docs
**Decision:** ARCHITECTURE.md covers memory, boot, overlay, the CPU contract, the hardware interface and host-side conveniences. DEVICE_SPECS.md covers every port protocol. The new MONITOR_SPEC.md covers commands, line input, messages and the HEX loader. QUICK_REFERENCE and README are cheat sheets that link to these. DESIGN_DECISIONS, VISION_UPDATED, CODE_TEMPLATES, CLAUDE_8080_SYSTEM_PROMPT and PROJECT_OVERVIEW moved to docs/archive/. MONITOR_IMPLEMENTATION_STATUS was deleted, replaced by MONITOR_SPEC.
**Rationale:** One fact, one home. Archived docs restated facts that had become wrong ("RST 7 as breakpoint", a polling CONOUT template, the old memory map).
**Method:** An ultracode workflow drafted the three docs, critiqued each through three lenses (consistency, hardware, testability and the mantra), revised, ran a completeness critic, then finalized and verified. 33 questions came back and Mike decided all of them.

### 2026-10-02: Timer Hardware Later, Interrupt Pin Now
**Decision:** v1 hardware has no timer. INT is wired through the 8228's single-level RST 7 and has no source. The emulator deletes the timer device but keeps a correct CPU interrupt input (`interrupt(rst)`: EI delay, HLT wake, 11 cycles), used only by tests. The tick source (Pi GPIO vs 8254) is decided when something needs it. With no source, HLT ends `run()`; the host prints `HLT at PC=xxxx` and exits.
**Mike's input:** "Will the hardware version have a timer?"
**Rationale:** Nothing in v1 needs a periodic interrupt. Wall time comes from the Pi, and delays are user busy-loops. But the INT pin is part of being an 8080A, so the emulator models the pin and not a fake device.

### 2026-10-02: The Pi Sees RESET; Reset Means Power-On State
**Decision:** RESET goes to a Pi GPIO input. The Pi drops any in-flight request without ACK and returns every device to power-on: storage unmounted, name buffer empty, mailbox idle, console FIFO flushed.
**Rationale:** The hardware critic found the Pi needs RESET anyway. Without it, a late ACK for a pre-reset request releases the first post-reset access with stale data. Full reset makes one rule, needs zero ROM bytes, and stops stale type-ahead from running as commands.

### 2026-10-02: Console Is a Pi FIFO Device
**Decision:** The Pi is the console at ports 0x00-0x02, behind READY. The Pi window becomes 0x00-0x6F. No UART chip and no ROM UART init. CONOUT is a bare `OUT 00h`. OUT 00 never waits: with no terminal or a full buffer, the byte is discarded. The terminal connects to the Pi (UART, USB gadget or TCP; this is Pi configuration).
**Supersedes:** 2026-10-02 "HEX Loader Paste Speed Is the Sender's Problem". The Pi buffers, so nothing paces.
**Rationale:** Fewest parts, no overrun, no flow control. The emulator's unbounded queue already models it exactly.

### 2026-10-02: Storage Errors Unmount; L/W Report Them
**Decision:** Any host I/O error on a data read (not past EOF), a write or a flush unmounts the file. Flush and unmount fsync. L and W read 0x0C after the transfer and print "Storage error" if bit 0 = 0. Every IN/OUT 0x0B advances the address, mounted or not.
**Rationale:** SD failure is real on the target. The error reuses an existing status bit, so the protocol gains no new flag.

### 2026-10-02: Mount Hygiene
**Decision:** Every OUT 0x0E (any value) clears the filename buffer. The device uppercases names before it validates and opens them. X sends `OUT 0Eh,03h` before the name. Status 02 prints "Invalid filename"; any other nonzero status prints "Mount failed".
**Rationale:** Closes the silent wrong-file create. Zero protocol additions, and the same file on case-insensitive macOS and on a case-sensitive Pi.

### 2026-10-02: Host-Side Quit and Debugger Entry
**Decision:** Quit is Ctrl-C, handled in the emulator's run loop, and the 8080 never sees it. The Phase 10 debugger is entered with a host hotkey (Ctrl-E) to an emulator prompt. Nothing intercepts the 8080's input stream. Q stays ask-Claude.
**Rationale:** On hardware, anything typed reaches the ROM. A prefix character would be stolen from the 8080 or would show up as an unknown command.

### 2026-10-02: Strict Arguments
**Decision:** end<start prints "Invalid range". More than 4 hex digits (6 for 24-bit) is an error. Byte arguments over FF are an error. G with a garbage argument is an error, while bare G defaults to 0100. A count of 0 or a garbage count is an error for L, W and M; an omitted count defaults to 0100. M with dst > src copies backward (memmove). Extra trailing tokens are ignored. CR and LF each end a line.
**Rationale:** Kills the class of bugs where one typo wipes 64K.


### 2026-10-02: One Service Mailbox for All Pi Services
**Decision:** HTTP, Claude, time, assembler and disassembler are text commands (`GET`, `ASK`, `TIME`, `ASM`, `DIS`) through one device at ports 0x10-0x13 (command char, control, status, response byte). The per-device register specs are superseded. Large results land in storage files. The device code is Rust behind `IoDevice`, written once: the emulator bus calls it, and on the Pi a GPIO front end calls the same code. Ports 0x08-0x6F are the Pi window.
**Mike's input:** "Do we need to do HTTP on the emulator itself? Why not do more of that on the Pi?"
**Rationale:** Five register maps with the same shape (send chars, command, poll, read bytes) is one protocol written five times. One mailbox means one ROM routine, one decode and one spec.
**Consequences:** Phase 6 shrinks to `TIME` + T. Phase 8 drops `N T` (the Pi has NTP). DNS needs no port.

### 2026-10-02: Phase 6 Shrinks; 8253 and Interrupts to Someday
**Decision:** Phase 6 = mailbox device + `TIME` + T command. The 8253, TIMER_ISR, TI/TS and the RST 7 vector wait until something needs a periodic interrupt.
**Rationale:** Three clocks were planned (time device, 8253 software clock, network time). Only one had a use case.

### 2026-10-02: Delete the Interim Timer
**Decision:** Remove `src/io/devices/timer.rs` and its hardwired hooks in `cpu.rs` (ports 0x30-0x32).
**Rationale:** No ROM user, no tests. It counts CPU cycles (a Pi can't), auto-acknowledges interrupts, drifts on every period, and shadows bus ports. Pending in TODO.md.

### 2026-10-02: G Pushes a WARM Return
**Decision:** G pushes a WARM entry (`LXI SP,STACK_TOP` then MAIN_LOOP) before `PCHL`. Programs exit with `RET`. No vector at 0000, so `JMP 0`/`RST 0` exits are not supported.
**Problem:** A program ending in RET popped ROM bytes `31 00` and jumped to 0031.
**Note:** Doesn't unblock R; that needs register capture (Phase 10). About 7 bytes. Pending in TODO.md.

### 2026-10-02: Pi Ports Use a READY Wait-State
**Decision:** Any I/O to a Pi port holds the 8080's READY low until the Pi releases it. Protocols stay instant-response; the ROM needs no busy-polling for byte transfers. Mount's 0xFF "busy" is reserved and never returned.
**Rationale:** Every protocol and ROM loop assumed a device answers within one IN/OUT cycle. One flip-flop plus a GPIO line keeps them all valid with zero ROM change. The alternative, busy bits plus polling, costs bytes in every loop.

### 2026-10-02: Claude Moves to Q; A Stays Assemble
**Decision:** Phase 7 keeps A/U (assemble/unassemble, DDT convention). Phase 9's ask-Claude command is Q.
**Follow-up:** The old future list had Q = Quit emulator. Logged as an Open Decision.

### 2026-10-02: No RST Vectors, No API Table
**Decision:** Drop the API jump table (0x0040-0x007F) and the RST 1-3 console vectors from the docs. RST 7 gets a single JMP when interrupts arrive. The register-preservation rules become descriptive: each routine's header is the contract.
**Rationale:** A public ABI with no callers is a promise with no customer.

### 2026-10-02: System Control Is a Bare Flip-Flop
**Decision:** Any OUT 0xFE disables the overlay. No 0x01 halt (the 8080 has HLT) and no 0xFF software cold reset (use the reset line). Overlay writes go through to RAM (ROM decoded on MEMR only). IN 0xFF: only bit 0 is defined.
**Rationale:** One 74LS74, no data comparator, no reset one-shot. Emulator change pending in TODO.md.

### 2026-10-02: Storage Edge Semantics: Code Mostly Wins
**Decision:**
- Mount creates missing files; there is no "not found". This keeps `W` to a new file working.
- Names over 12 chars return 0x02 instead of truncating.
- A failed mount unmounts the previous file.
- A past-EOF read returns 0xFF and still advances the address.
- 8.3 is a convention, not enforced.

**Rationale:** Each rule is simpler for a Pi to copy than the accident it replaces. Rust fixes pending in TODO.md.

### 2026-10-02: Rebuild the Stale ROM Binary
**Decision:** Rebuilt and committed `rom/monitor.bin`. The committed bin predated `X -` unmount, so all monitor tests had been running an old ROM.
**Lesson:** Rebuilding bakes in DATE/TIME, so "rebuild and diff" can't detect drift. A behavioral test (`X -` prints Unmounted) is the guard; it's on TODO.

### 2026-10-02: HEX Loader Paste Speed Is the Sender's Problem (for Now)
**Decision:** Phase 5 assumes the terminal paces lines (per-line delay). The console chip and flow control (6850 + RTS/CTS vs a Pi FIFO console) are a hardware-build decision.
**Problem:** About 7ms of ROM work per 16-byte record against 1ms per char at 9600 baud. A real UART overruns. The emulator's unbounded queue hides it.

### 2026-10-02: HEX Record Limits
**Decision:**
- Max 34 data bytes per record (80-char LINE_BUFFER); a longer line gets "Line too long".
- Types 00 and 01 only; 02-05 are errors.
- Each record is fully validated (length + checksum) before any byte is written.

**Rationale:** Standard tools emit 16 or 32 bytes per record. Smallest parser.

### 2026-10-02: HEX Loader Write Guard; Stack Gets Its Own Page
**Decision:** The loader rejects records touching 0x0000-0x00FF or 0xEF00-0xFFFF. User area is 0x0100-0xEEFF; 0xEF00-0xEFFF is the monitor stack page.
**Problem:** A record aimed at 0x0080 overwrote LINE_BUFFER mid-parse and loaded garbage silently. The stack (SP=F000) lived inside the documented user area.
**Note:** About 20 bytes. Other commands (F/M/E/L) still don't guard.

### 2026-10-02: HEX Loader Auto-Detects ':'
**Decision:** A line starting with `:` at the prompt is an Intel HEX record. There is no command letter and no loader mode; each line stands alone. `:` belongs to HEX; the Phase 10 emulator-command prefix is TBD.
**Rationale:** About 5 bytes of dispatch, straight paste works, no mode to get stuck in. Resolves the `H` and `:` collisions.

### 2026-10-02: Claude Writes the Code; Hardware Is the End State
**Decision:** Claude Code (including multi-agent ultracode runs) writes the Rust and ROM implementation. Mike owns architecture, decisions, and review. Supersedes the "Mike writes the code" rule from 2026-01-16.
**Rationale:** The true goal is a physical 8080 machine. Hand-writing the emulator was the bottleneck; acceleration matters more than the Rust exercise.
**Constraint added:** ROM code and device protocols must be buildable with real parts. Emulator-only conveniences stay on the Rust side.

### 2026-10-02: Repo Is the Single Source of Truth
**Decision:** The repo is the only source of truth for code AND docs. The Claude.ai Project is retired; all work moves to Claude Code.
**Problem:** PK copies had drifted: stale status (191 tests, 11 vs 14 commands), mojibake from repeated copy/paste, PK-only files the repo never saw.
**Rationale:** Two editable copies of the same doc is a distributed consensus problem. We are not solving Paxos for markdown.
**Outcome:** Session protocol in CLAUDE.md. Claude Code appends session notes here. Full PK export diffed against the repo before retiring it: nothing PK-only was lost.

### 2026-01-30: Dead Code Purge
**Decision:** Delete `disk.rs` (legacy 16-bit storage, superseded by `storage.rs`), strip debug `println!` from `update_flags()`
**Incident:** `null.rs` deleted by mistake instead of `disk.rs`; recreated.
**Lesson reinforced:** `git commit -A` to stage deletions; `cargo test` before commit.
**Outcome:** 204 tests passing (14 unit + 180 CPU + 10 monitor integration).

### 2026-01-16: Split Workflow — Web for Strategy, Claude Code for Tactics
**Decision:** Architecture and "should we even" debates happen in the Claude.ai Project. Implementation happens in Claude Code CLI. GitHub is the bridge.
**Rationale:** Web UI compaction during long coding sessions; Claude Code has real filesystem and `cargo test` access.
**Mike's input:** CLI over VS Code integration—deliberate friction keeps him writing the code himself.
**Claude's input:** "CLI keeps Claude as a consultant you deliberately summon, not a copilot who's always reaching for the wheel."
**Known risk:** Architectural decisions made in Claude Code without the mantra police present. Mitigation: design questions get logged as Open Decisions, not silently decided.

### 2025-12-20: Phase 4 Storage System Complete
**Decision:** 24-bit linear-addressed storage (16MB per file) with file mounting
**Implementation:** Storage device ports 0x08-0x0C, mount service 0x0D-0x0F
**Mike's input:** Asked about 24-bit helper routines, Claude advised against over-engineering
**Outcome:** Storage device handles auto-increment internally, ROM code stays simple

### 2025-12-19: Storage as Files, Not Disk Geometry
**Decision:** Linear-addressed storage, not IBM 3740 track/sector emulation
**Rationale:** 
- Maps directly to hardware (SD card, EEPROM with shift register)
- Mike's end state changed: building own 8080, not running CP/M
- Track/sector controller is "cosplay complexity" when not needed
**Mike's input:** "I am less worried about being able emulating CPM. I'd rather build my own 8080 device and run my monitor and period appropriate programs."
**Claude's input:** "If you're building actual hardware and running your own code, the track/sector controller is just cosplay complexity. You'd be implementing IBM 3740 semantics in silicon for... nostalgia?"
**Outcome:** Simple, elegant, hardware-ready. 16MB address space.

### 2025-12-19: Documentation Reorganization
**Decision:** Split 2000+ line spec into 7 focused documents
**Files created:**
- PROJECT_OVERVIEW.md
- ARCHITECTURE.md
- DEVICE_SPECS.md
- DESIGN_DECISIONS.md
- IMPLEMENTATION_ROADMAP.md
- QUICK_REFERENCE.md
- CODE_TEMPLATES.md
**Outcome:** Easier to navigate, clearer purpose per document

### 2025-12-16: ROM Overlay Boot Mechanism
**Decision:** ROM appears at 0x0000 AND 0xF000 on reset. OUT 0xFE disables overlay.
**Problem:** 8080 starts at 0x0000 but ROM lives at 0xF000
**Solution:** ROM overlay with hardware bank switching
**Rationale:** 
- Authentic S-100/Altair/IMSAI behavior
- Works on real hardware with 74LS74 flip-flop
- Single ROM file, pure address decoding (not two separate binaries)
**Mike's input:** Wanted CP/M compatibility path
**Outcome:** Clean boot sequence, matches real systems

### 2025-12-16: R Command Deferred
**Decision:** Skip register display command until actually needed
**Rationale:**
- No current use case for register display
- Requires return mechanism (G does PCHL and never returns)
- Monitor already exceeds DDT feature parity (11 vs 10 commands at the time)
**Mike's input:** "Because you suggested it LOL"
**Claude's input:** "So you're implementing a feature because a document told you to, not because you need it. That's the software engineering equivalent of buying a boat because the marina had a brochure."
**Outcome:** YAGNI applied correctly. Implementation notes preserved for future.
**Where:** Moved to Phase 9 (Debugger)

### 2025-12-16: Automated Testing Infrastructure
**Decision:** Created TestConsole device for scripted monitor testing
**Outcome:** 10 integration tests covering all commands

### 2025-12-09: Console Port Mapping Bug Fixed
**Decision:** Map ALL THREE ports (0x00, 0x01, 0x02) not just two
**Problem:** Monitor hung reading input status—port 0x02 wasn't mapped
**The bug:**
```rust
cpu.io_bus_mut().map_port(0x00, console.clone());
cpu.io_bus_mut().map_port(0x01, console);
// Port 0x02 missing!
```
**Mike's input:** "Good catch!"
**Claude's input:** "Three ports, three mappings. You changed the spec but didn't update main.rs to match."
**Outcome:** Console I/O fully working

### 2025-12-09: Align ROM to Spec
**Decision:** Update ROM port numbers to match spec, not vice versa
**Problem:** Spec said ports 0x00/0x01/0x02, ROM used 0x00/0x01 only
**Claude's input:** "Spec was designed, code was hacked. Align implementation to design."
**Outcome:** Clean separation: output on 0x00, input on 0x01, status on 0x02

### 2025-12-04: ASL Assembler Choice
**Decision:** Use AS Macro Assembler (Alfred Arnold) instead of custom assembler
**Rationale:** Battle-tested, cross-platform, proper documentation
**Mike was tempted to:** Write his own assembler from scratch
**Claude's input:** "Are you building an assembler or an emulator?"
**Outcome:** Saved months of distraction

### 2025-12-02: Delete Custom Assembler
**Decision:** Remove 683-line custom assembler (src/bin/asm8080.rs)
**Rationale:** AS assembler works fine, custom one was scope creep
**Mike's reaction:** "RIP custom assembler, you were beautiful but unnecessary"
**Outcome:** Focus back on emulator, not tooling

### 2025-12-02: Remove Layer 2.5 Abstraction
**Decision:** Delete redundant `*_by_code` methods
**What it was:**
- `get_reg_by_code()`
- `set_reg_by_code()`
- `get_pair_by_code()`
- `set_pair_by_code()`
- `get_push_pop_pair_by_code()`
- `set_push_pop_pair_by_code()`
- `test_condition_by_code()`
**Rationale:** Duplicated what `get_reg(Register::from_code(x))` already does
**Claude's input:** "Your 'layered' approach is over-engineered... You don't need both direct getters AND enum getters."
**Outcome:** Cleaner codebase, single approach

### 2025-12-02: Don't Need push_word Helper
**Decision:** Delete `push_word()` function, inline the code
**Context:** Mike wanted to create push_word using perform_push
**Claude's input:** "You already have `write_word`. You don't need a wrapper function that does exactly the same thing with a different name. That's not abstraction, that's just noise."
**Mike's reaction:** "Funny thing is in another thread you recommended this approach"
**Claude's response:** "Yeah, well, that version of me was wrong. Or more likely, I didn't have the full context of your codebase and your tendency to over-engineer."
**Outcome:** Code stayed simple

### 2025-12-02: Project Cleanup and GitHub Baseline
**Decision:** Major cleanup, establish clean repository
**Removed:**
- src/bin/asm8080.rs (683 lines)
- src/temo (scratch file)
- src/debug.rs (orphan)
- src/disassemble.rs (orphan)
- Layer 2.5 abstraction
- Various test debris
**Renamed:** intel8080cpu.rs → cpu.rs
**Added:** rom/Makefile, rom/monitor.asm, README.md
**Outcome:** Clean baseline, 181 passing tests

### 2025-09: Initial VM Configuration Exploration
**Decision:** Don't build elaborate VM/config system yet
**Context:** Mike asked about per-program configuration files for different hardware setups
**Claude's input:** (implicit) This was identified as over-engineering for the current phase
**Outcome:** Deferred to future, focus on core emulation first

---

## Lessons Learned

Patterns that emerged from actual work.

### Over-Engineering Incidents

**The Custom Assembler (Dec 2025):**
Mike started writing a custom 8080 assembler from scratch—683 lines before deletion. Claude redirected: "Are you building an assembler or an emulator?" The AS assembler works fine.
- **Saved:** Months of work
- **Lesson:** Use proven tools for solved problems

**Layer 2.5 (Dec 2025):**
Mike created redundant register access methods that duplicated existing enum-based access.
- **What it was:** `get_reg_by_code()` that just called `get_reg(Register::from_code(x))`
- **Lesson:** Don't wrap wrappers

**The push_word Function (Dec 2025):**
Mike wanted to create a push_word helper using perform_push.
- **Claude's response:** "You already have write_word. You manipulate sp directly everywhere else."
- **Lesson:** Functions must earn their existence

**R Command Feature Creep (Dec 2025):**
Mike wanted to implement R command "because you suggested it."
- **Claude's response:** "So you're implementing a feature because a document told you to..."
- **Lesson:** No use case = no implementation

**Late Night Spec Documents (Dec 2025):**
2AM ideas about 256 disks, Scrabble games, Claude integration, HTTP APIs.
- **Resolution:** Write it down, sleep on it, pick ONE thing for tomorrow
- **Lesson:** Morning review before committing to ideas

**Track/Sector Controller (Dec 2025):**
Original spec described IBM 3740 floppy controller emulation.
- **Claude's input:** "That's period-accurate for CP/M compatibility but... you're building your own 8080 hardware, not running CP/M"
- **Lesson:** Question the requirements before implementing

### What Worked Well

**Port-Based Abstraction:**
Every complex feature (HTTP, Claude API, TLS) lives behind simple ports. 8080 code stays simple. Coprocessor handles complexity.
- **Why it works:** Same interface works on emulator and real hardware

**Rule of Three:**
Wait for patterns to emerge before abstracting. Applied to device interfaces.
- **IoDevice trait:** Just 2 methods (read/write), not 10

**"Finish the Command":**
When Mike wants to add features, redirect to completing current work first.
- **Pattern:** "You have a working X. The Y isn't done. The Z is still deferred."

**Direct Gilfoyle Feedback:**
Mike explicitly requested direct, challenging feedback in the Gilfoyle style.
- **Example:** "So you're implementing a feature because a document told you to, not because you need it."

**Frequent Testing:**
`cargo test` after every change, not at the end.
- **Catches:** Bugs early, maintains confidence

**Historical Accuracy as Guide:**
Following CP/M conventions and real 8080 system patterns provides valuable guidance without constraining innovation.
- **Example:** ROM overlay boot matches real Altair/IMSAI behavior

### Debugging Patterns

**Strategic Debug Markers:**
Adding `eprintln!` statements, then removing them before commit.

**Systematic Edge Case Testing:**
D command infinite loop at FFFF, 16-bit address wraparound, high memory overflow.

**"Three Bugs, Three Fixes":**
Console I/O debugging session:
1. Double-echo → raw mode
2. Status port not working → wrong port numbers in ROM
3. Input still broken → port 0x02 not mapped

---

## Current State

**Last Updated:** October 3, 2026

### Completed

**CPU Core:** All 256 opcodes (5 undocumented aliases decoded), flags match ARCHITECTURE 5.1-5.3 including AC, 8080A interrupt input (EI delay, HLT wake), reset() = RESET pin. All four exercisers pass, 8080EXM included

**Monitor ROM v0.3:**
- 14 commands: D, E, F, M, S, C, H, G, I, O, L, W, X, ?
- ROM overlay boot mechanism
- 2511 of 4096 bytes used (1585 free)

**Devices:**
- Console (0x00-0x02), Storage + Mount as one device (0x08-0x0F, 24-bit / 16MB), System Control (0xFE-0xFF). One port map (`build_bus`) for main.rs and every harness. Device code has no terminal code; the host run loop (key map, input pump, Ctrl-C, halt) is in main.rs

**Testing (verified 2026-10-03):**
- 4 host + 129 CPU + 36 device + 16 monitor = 185, all passing (strict transcript harness, reference-model CPU tests, port-level device tests)
- 4 `#[ignore]` exercisers (TST8080, 8080PRE, CPUTEST, 8080EXM) all pass: `scripts/fetch_exercisers.sh`, then `cargo test --release --test exerciser -- --ignored`

### In Progress

- **Phase 5:** Intel HEX loader, parsed by the 8080 itself in ROM. All design decisions made 2026-10-02; build order in `TODO.md`. Not started.
- **Review findings:** 2026-10-02 review found CPU flag bugs (AC on subtract, DCR, ANA, DAA; PSW bits; EI delay; HLT), ROM range and parse bugs, and vacuous tests. All are listed in `TODO.md` with repros. The vacuous tests are replaced and the CPU bugs are fixed (2026-10-03); the ROM bugs are open.

### Open Decisions

Two host-only questions from 2026-10-03 (idle CPU at the prompt, exit at piped EOF); see `TODO.md` Open Decisions. Everything else closed 2026-10-02. The spec is the three normative docs: `docs/ARCHITECTURE.md`, `docs/DEVICE_SPECS.md`, `docs/MONITOR_SPEC.md`.

### Blocked/Deferred

- **R command:** Needs return mechanism, deferred to Phase 10 (Debugger)
- **Pi services:** one Service Mailbox at 0x10-0x13. Phase 6 `TIME`, 7 `ASM`/`DIS`, 8 `GET`, 9 `ASK`
- **8253 timer / interrupts:** Someday

### Future Vision (Documented, Not Started)

- HTTP client, system time device, Gutenberg e-reader, Claude API device
- Hardware prototype (Pi Zero + real 8080)

---

## Recent Sessions

### 2026-10-03: Devices and Host Run Loop (step C)
- Storage and Mount merged into one device on 08-0F (no `Rc<RefCell>` link): every IN/OUT 0B advances, host I/O errors unmount, `sync_all` on flush/unmount/remount/Drop, mount rules per DEVICE_SPECS 7 (uppercase first, 13th byte = too long, `.`/`..` invalid, >16MB = 01, failed mount unmounts), every OUT 0E clears the name, 0F reads 01 at power-on.
- Console is a FIFO plus a 2 MiB-capped output buffer with no terminal code; `null.rs`, `test_console.rs`, `storage_mount.rs` deleted. One `build_bus()` for main.rs and every harness; reset = build it again.
- main.rs owns the host side: `map_key` per ARCHITECTURE 7.1 (Ctrl+Alt now dropped like Alt), pump every 10,000 steps, `run_loop` returns Halted/Quit, raw mode only on a real TTY, piped stdin goes in unmapped.
- Tests: 36 port-level device tests, 4 run-loop tests, new transcript lines that fail on the old devices. cargo-mutants on the device and host files: every survivor equivalent, fsync-only or terminal-only.
- What bit us: the Ctrl-C test hung instead of failing when its mutant survived (the script fed empty batches forever); the script now panics after 100 idle pumps. The README piped example never exited (EOF just waits at the prompt), so it now uses a halting example. Idle CPU and EOF-exit went to Open Decisions.

### 2026-10-03: CPU Fixed, Exercisers Green (step B)
- AC fixed for SUB/SBB/CMP, DCR, ANA/ANI and DAA; POP PSW gives `(v&D5)|02`; aliases CB/D9/DD/ED/FD decoded, so no opcode panics. Reference-model tests lost their AC masks and now compare every flag. 8080EXM passes all 25 groups.
- Refactor: one `alu()` for register and immediate forms, push/pop helpers, dead code gone. `cpu.rs` 1122 -> 561 lines, `registers.rs` 312 -> 45, `memory.rs` and `timer.rs` deleted.
- Timer deleted; `interrupt(n)` input with 8080A acceptance (EI delay, HLT wake, 11 cycles). HLT fetches nothing, `run()` returns on halt, main.rs prints `HLT at PC=xxxx`.
- reset() = RESET pin only; `OUT FE` clears the overlay (no cold reset); writes under the overlay go to RAM; `map_port` panics on FE/FF.
- Exerciser shim is 8080 code (BDOS 2/9 via OUT 00), so the same bytes can run on hardware. cargo-mutants `cpu.rs`: every viable non-equivalent mutant killed.
- What bit us: nothing new. Logged that `run()` returns `()` where ARCHITECTURE 7.2 says a status; it lands with the run-loop quit work.

### 2026-10-03: Tests That Test (step A)
- Monitor tests are now a strict transcript harness: junk RAM (76 = HLT), junk registers and flags, SP = 0000; each step runs to the prompt and must match exactly; budget or HLT = failure. Transcripts are data in `tests/transcripts/`, meant to drive hardware too.
- CPU tests rebuilt around an exhaustive reference model, a 244-opcode cycle/length table, a branch table and RST/IN/OUT/wrap/reset/overlay tests; 72 vacuous, duplicate or now-redundant tests deleted. New `tests/device_tests.rs` tests storage and mount at port level; the private-access unit tests in `storage_mount.rs` went.
- Mutation: ROM hand mutants 59/65 killed (was 9/65), cargo-mutants `cpu.rs` 647/752 (was 544), storage + mount 74/80. Every survivor is equivalent, unobservable, or a tracked bug.
- What bit us: A5 junk is ANA L, a NOP slide into F000 that boots without the overlay; HLT fixed it. The adversarial review found five real ROM mutants surviving on transcript ordering (L without OUT 0A, W without OUT 09, W default 0101, addr24 high nibble, S end check); fixed by setting each address byte from scratch and adding six-digit addresses.
- Found a new ROM bug: S tests the candidate at end+1. Logged in TODO, test ships with the fix.

### 2026-10-02 (part 4): Hardware Alignment and a Real CPU Reference
- An 11-agent hardware-alignment workflow (CPU timing, Pi interface, memory and glue, power and build, software fit), with each specialist adversarially verified. Then 19 decisions, all accepted.
- Wrote docs/reference/8080_HARDWARE.md from the 1975 MCS-80 User's Manual. Every number was read from the page images, and the doc was independently verified. It caught the same SYNC hazard and OUT-data race the workflow found.
- What bit us: the spec's WAIT-set was wrong by datasheet. It's caught on paper, before any board exists.
- New docs/HARDWARE_BUILD.md: BOM (about 20 ICs) and a 9-step bring-up plan.
- Next: code. Start with the scratch prototypes (strict harness, exhaustive CPU tables, exerciser shim, CPU refactor diff).

### 2026-10-02 (part 3): Code Review, Exercisers, Debugger Pulled Forward
- A 16-agent review (CPU, then ROM and devices), with every finding adversarially verified. Ran `cargo-mutants` for real and the four 8080 exercisers under a CP/M shim.
- What bit us: the tests barely test. CPU tests alone score 26% on hand mutation, the ROM tests 10.6%. Zeroed RAM makes the emulator boot even with the overlay or reset broken. Several conditional-branch tests pass whichever way the branch goes, and `test_rp_not_taken` actually takes the return.
- The CPU is otherwise sound. TST8080 and 8080PRE pass with exact reference cycle totals, and exhaustive flag, cycle and wrap tables match the spec except for the tracked AC/PSW/alias/interrupt items. 8080EXM fails 11/24 groups, all AC.
- Decided: build the debugger before Phase 5; reset() models the pin only; merge the storage devices; one Console with host I/O in main.rs; exercisers fetched rather than committed.
- Findings and the order of work are in TODO.md under "Code Review (2026-10-02)". No code changed.

### 2026-10-02 (part 2): Decisions Closed, Spec Solidified
- Closed all 5 open decisions and the 33 questions the spec workflow raised. The big ones: the Pi FIFO console, the Pi seeing RESET with RESET meaning power-on state, the INT pin kept with the timer deferred, storage errors unmounting and L/W reporting them, strict arguments, and host-side quit and debugger.
- Two ultracode workflows produced the specs. The first (16 agents) drafted ARCHITECTURE, DEVICE_SPECS and the new MONITOR_SPEC, critiqued them through three lenses, revised them and ran a completeness critic. The second (11 agents) finalized the docs to the decisions, verified them and fixed what it found.
- Archived 6 non-normative docs and deleted MONITOR_IMPLEMENTATION_STATUS. QUICK_REFERENCE and README are now cheat sheets that link to the specs.
- What bit us: the spec authors' Q-RESET-PI recommendations disagreed between docs, which a single ID was supposed to prevent. The completeness critic caught it. The clock arithmetic was also off: an 18.432 MHz crystal gives 2.048 MHz, not 2.0, with a ~488 ns T-state.
- No code changes. Everything the specs require of the code is in TODO.md under "Decided, to implement" and "Review findings".

### 2026-10-02: Pre-Phase 5 Review
- Four parallel audits: docs vs. code, CPU core, devices and hardware buildability, ROM/tests/Phase 5. Every finding was reproduced in scratch tests or traced in the asm before being kept.
- What bit us: the committed `monitor.bin` was stale (no `X -` unmount), and the tests couldn't notice because none map storage. Two monitor tests are vacuous: they match echoed input and the banner date. Rebuilt the bin.
- CPU: AC is wrong on every subtract, DCR, ANA and DAA, and three tests assert the wrong values. The EI delay and HLT wake are missing, and the undocumented CALL/JMP/RET aliases panic.
- Decided 14 items. The big ones: `:` auto-detect HEX loader with a page-0/stack-page guard, a READY wait-state for Pi ports, one Service Mailbox for all Pi services (Phase 6 shrinks to TIME + T), and deleting the interim timer.
- Docs fixed to match code (ROM layout, boot steps, mount status codes, dispatch style, startup banner) and rewritten for the decisions. Code changes for the decisions are queued in TODO.md; no src/ or ROM source changed this session.

### 2026-10-02: Web/Claude Code Alignment
- Audited PK vs GitHub HEAD: drift was mostly encoding damage plus stale counts
- Diffed the full 21-file PK export against the repo: no PK-only content lost. Added `COLLABORATION_LOG_SPEC.md` and the ROM overlay session summary; dropped `COLLABORATION_LOG_UPDATE_PROMPT.md`
- Repaired mojibake and stale counts across docs; untracked `monitor.lst`/`monitor.p`
- Established repo-as-source-of-truth and session protocol (CLAUDE.md); web Project retired
- Decided: Claude writes the code from here (ultracode), Mike decides and reviews; physical hardware is the end state
- Found command collisions (`H`, `:`, `A`) and docs describing things the code doesn't do (0xFE halt, vector copy, timer ports). Logged as Open Decisions, not fixed

### 2026-01-30: Code Review and Cleanup
- Removed debug prints from `update_flags()`, deleted dead `disk.rs`
- Accidentally deleted `null.rs`, recreated it
- Set up `~/dev` as Obsidian vault, per-project CLAUDE.md, TODO.md

### 2026-01-16: Workflow Split
- Back after hiatus (new job)
- Adopted web-for-strategy / Claude-Code-for-tactics split
- Chose Claude Code CLI over VS Code integration
- Confirmed Phase 5 HEX parsing happens in 8080 code, not a Rust device

### 2025-12-20: Phase 4 Complete
- Storage device (24-bit, 16MB) fully implemented
- ROM commands: X (mount), L (load), W (write)
- READ_HEX_ADDR24 helper for 6-digit hex parsing
- Version bumped to v0.3
- Tests: 191 passing (181 CPU + 10 integration)

### 2025-12-19: Vision Documents Created
- VISION_UPDATED.md: Gutenberg reader, Claude integration concepts
- COLLABORATION_LOG_SPEC.md: This document's specification
- CLAUDE_8080_SYSTEM_PROMPT.md: API prompt template
- Estimated API cost: ~$0.014 per Claude request

### 2025-12-17: Disk Architecture Decision
- Rejected track/sector controller (IBM 3740 cosplay)
- Chose linear-addressed storage
- Updated specs to reflect new direction

### 2025-12-16: ROM Overlay and R Command
- Implemented S-100 style boot mechanism
- Created TestConsole for automated testing
- Deferred R command (no use case)
- Session summary: session_summary_rom_overlay.md

### 2025-12-09-10: Console I/O Bug Hunt
- Fixed port mapping bug (0x02 unmapped)
- Implemented raw mode for terminal
- Added Ctrl+C handling
- Created build timestamp in ROM

### 2025-12-04: Blog Posts and Documentation
- "The 683-Line Assembler I Didn't Need" (intro post)
- "8080 Monitor ROM: The D Command Lives"
- ASL date/time macros for build timestamps

### 2025-12-02-03: The Great Cleanup
- Deleted custom assembler (683 lines)
- Removed Layer 2.5 abstraction
- Established GitHub baseline
- 181 tests passing
- File loading architecture finalized

---

## Memorable Quotes

**On complexity:**
> "A fool admires complexity, genius admires simplicity."
> — Project mantra, invoked frequently

**On feature creep:**
> "So you're implementing a feature because a document told you to, not because you need it. That's the software engineering equivalent of buying a boat because the marina had a brochure."
> — Claude, on R command

**On over-engineering:**
> "Yeah, well, that version of me was wrong. Or more likely, I didn't have the full context of your codebase and your tendency to over-engineer."
> — Claude, admitting previous advice was wrong

**On disk architecture:**
> "If you're building actual hardware and running your own code, the track/sector controller is just cosplay complexity."
> — Claude, on IBM 3740 emulation

**On the custom assembler:**
> "RIP custom assembler, you were beautiful but unnecessary."
> — Mike, on deletion

**On the vision:**
> "I want it to talk to its co-creator."
> — Mike, on Claude API integration

**On the blog title:**
> "The 683-Line Assembler I Didn't Need"
> — Blog post title that captures the project's philosophy

---

## Review Metadata

**Last Review:** 2026-10-02
**Method:** Pre-Phase 5 spec/code/plan review in Claude Code (parallel audit agents, findings reproduced before reporting). Before that: audit of GitHub HEAD against Project Knowledge.
**Next Review:** Not needed while the session protocol holds. Claude Code appends a Recent Sessions entry at the end of every session.
