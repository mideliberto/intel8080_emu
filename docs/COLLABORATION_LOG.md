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

**Last Updated:** October 2, 2026

### Completed

**CPU Core:** All 256 opcodes, full flag system (S, Z, P, AC, C), stack, I/O port system, interrupts

**Monitor ROM v0.3:**
- 14 commands: D, E, F, M, S, C, H, G, I, O, L, W, X, ?
- ROM overlay boot mechanism
- ~2.4KB of 4KB ROM used (~1.6KB headroom)

**Devices:**
- Console (0x00-0x02), Storage (0x08-0x0C, 24-bit / 16MB), Storage Mount (0x0D-0x0F), System Control (0xFE-0xFF)

**Testing (verified 2026-10-02 against GitHub HEAD):**
- 14 unit + 180 CPU + 10 monitor integration = 204, all passing

### In Progress

- **Phase 5:** Intel HEX loader, parsed by the 8080 itself in ROM. Not started.

### Open Decisions

Live list with details in `TODO.md`. Blocking Phase 5: the `H` and `:` collisions. Also open: `A` collision (Phase 7 vs 9), existing timer at 0x30-0x32 vs planned 8253 at 0x70-0x73, port 0xFE halt (documented, not implemented), RST vector / API table copy (documented, not implemented).

### Blocked/Deferred

- **R command:** Needs return mechanism, deferred to Phase 10 (Debugger)
- **Network device:** Ports 0x10-0x1F reserved, Phase 8
- **Claude API integration:** Phase 9

### Future Vision (Documented, Not Started)

- HTTP client, system time device, Gutenberg e-reader, Claude API device
- Hardware prototype (Pi Zero + real 8080)

---

## Recent Sessions

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
**Method:** Audit of GitHub HEAD (2026-01-30 commit) against Project Knowledge, plus chats after 2026-01-16. Full PK export diffed in Claude Code.
**Next Review:** Not needed while the session protocol holds. Claude Code appends a Recent Sessions entry at the end of every session.
