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

### 2026-10-03: Phase 7 Specified: ASM, DIS, A and U
**Decision:** Mike accepted every Phase 7 recommendation. Homes: DEVICE_SPECS 8 (the grammar, the DIS line, R1/R2, the vectors) and MONITOR_SPEC 6.16-6.17 (the commands, 6.17.1 vectors).
- **Q-ALIAS:** a starred mnemonic assembles to the lowest opcode of its group (`NOP*` 08, `JMP*` CB, `RET*` D9, `CALL*` DD). The byte round trip fails only for 10 18 20 28 30 38 ED FD, listed once in DEVICE_SPECS 8 R2.
- **Q-HSUFFIX:** ASM numbers are bare hex, 1-4 digits, a byte at most FF. No `H` suffix, no `0x`, `$` or sign.
- **Q-REGNUM:** in a number slot, a register letter is hex: `MVI A,D` = 3E 0D.
- **Q-ADDR-DEFAULT:** A and U require an address. No LAST_ASM/LAST_UNASM workspace.
- **Q-U-ARGS / Q-U-BADCOUNT:** `U addr [count]`, the count in instructions, default 8. A present but invalid count prints `Invalid hex value`, as L and W.
- **Q-A-LOOP:** only `.` ends A. An empty or all-space line prompts again with nothing sent. 82 prints `Invalid instruction`, any other failure `Service error`, and both prompt the same address again.
- **Q-DIS-SHAPE:** DIS takes `AAAA B0 B1 B2` and returns a binary length byte, the whole line and CR LF. The line is defined once, in DEVICE_SPECS 8 DIS; the debugger's instruction line is that line with symbols (ARCHITECTURE 7.4).
- **Q-LINE-FN:** the line format moves from the private `Debugger::insn` to `disasm::line`, device code that DIS and the debugger both call. A move, not a new abstraction.
- **Q-READY:** TIME, ASM and DIS complete within the execute access (bounded work under READY); every other mailbox command runs in the background. Reworded in DEVICE_SPECS 3 rule 3, HARDWARE_BUILD 5 and ARCHITECTURE 6.4, which all said "anything behind the mailbox".
- **Mailbox client:** extracted from T into one ROM routine set, MB_SEND/MB_PUT/MB_GET (MONITOR_SPEC 9). Three users: T, A, U.

**Rationale:** The ROM stays an editor and a byte pump: no opcode table in 4 KB, the Pi runs the same `OPCODES` the debugger uses. One number style everywhere (U prints what A reads). The A loop change came from a reproduced hazard: when an empty line or `Service error` ended A, pasted source fell through to the dispatcher, where `XCHG` ran as `X CHG` and mounted a file.
**Mantra check:** declined: `H` suffixes, distinct alias spellings, address defaults with new workspace, ROM-side line formatting (+30-40 bytes and a second copy of the format). Budget +328 bytes (2571 -> 2899), measured on a sketch.

### 2026-10-03: Pi Daemon Specified; PI_DAEMON.md Is the Fourth Normative Doc
**Decision:** Mike accepted every Pi daemon recommendation:
- **Fourth normative doc:** `docs/PI_DAEMON.md` joins ARCHITECTURE, DEVICE_SPECS and MONITOR_SPEC. This amends "Spec Solidified Into Three Normative Docs" (2026-10-02). HARDWARE_BUILD 5 shrinks to the platform decisions and a pointer.
- **build_bus takes the clock:** `build_bus(storage_dir, clock)`. main.rs and the harnesses pass `mailbox::local_time`, the daemon its NTP-gated clock. One port map.
- **Gpio trait, a deliberate exception to the rule of three:** `Gpio { read(off), write(off, v), reset_edge() }`, register level, with two implementations: `GpioMem` (Pi, `/dev/gpiomem` mmap) and `SimBoard` (tests). Without it the bus loop (lost, doubled or hung accesses, RESET races) could only be tested on the bench. Same kind of exception as the mailbox `Clock` seam.
- **RESET check cost:** the level read and the REQ-lost check run before every ACK; the edge-latch syscall runs only when more than 1 ms has passed since the last call. Premise: the DS1813 (RESET-SOURCE) holds RESET for at least 150 ms. ARCHITECTURE 6.6 and DEVICE_SPECS 3.3 now name it; a sub-ms reset source would need the latch before every ACK.
- **RESET edges:** raw GPIO v2 line-request ioctl plus GPIO_GET_CHIPINFO through `libc`, the chip found by its `pinctrl-bcm2711` label, struct sizes and ioctl numbers pinned by const asserts. No crate (not gpiocdev, not rppal).
- **Console:** listens on 127.0.0.1:8080 by default, reached with `ssh -L`; `--listen` with a wildcard opens it. A byte reaches the Pi when the service reads it from the transport, and RESET discards bytes still buffered there (DEVICE_SPECS 4).
- **Build:** static `aarch64-unknown-linux-musl`, cross-built on the Mac with `rust-lld`; native build on the Pi is the fallback. Verifying it is a task, not a decision.
- **No `--measure` mode:** day-one numbers come from a scope on REQ/ACK/LATCH and whole-command timing (PI_DAEMON 12.2).
- **Devices are not `Send`.** This clarifies the 2026-10-02 entry "Emulator Shape Follows the Hardware", which listed "wasn't Send" among the device layer's problems: the problem was crossterm in the device layer. Devices stay `Rc<RefCell<..>>`, and the daemon's one bus thread owns them.

**Rationale:** The daemon is the emulator's port map behind GPIO instead of behind the CPU model; everything else is transport. The trait buys "nothing commits red" for the code with the worst failure modes. The 1 ms gate takes a syscall off a 3 us budget without a second thread, on a premise the reset part guarantees.
**Mantra check:** declined: a watcher thread, a gpiocdev or rppal dependency, a measurement mode, Docker or zig for the cross build, `Send` devices.

### 2026-10-03: Phase 6 Readings Confirmed; Pi Clock Policy
**Decision:** Mike confirmed every Phase 6 literal reading as normative, plus the Pi clock policy. Each now has one home:
- MONITOR_SPEC 6.15: (1) an error after partial T output prints on the same line, no CR LF first (`2026-Service error`); (9) T transcripts match the shape, never a value, so they run on hardware; device-level tests may inject a clock (replaces "No injectable clock is needed").
- DEVICE_SPECS 8: (2) `ASM`/`DIS`/`GET`/`ASK` give 80 until their phase ships; (3) a 00 written to OUT 10 is appended like any byte (port 0D ignores 00); (4) OUT 10 in AVAIL/DONE/ERROR appends and changes no status or response, the bytes wait for the next execute; (5) precedence 81 > 80/82 > 83; (6) a year below 1000 is zero-padded, a year above 9999 gives 83; (7) the clock must return in-range fields, the device does not range-check them; (8) the device formats TIME from clock fields, so the emulator and the Pi daemon share the formatter.
- Harness header (`tests/monitor_tests.rs`): (10) keep the `libc` dependency (`localtime_r`) and the `\d` transcript escape; `\d` was already documented there.
- DEVICE_SPECS 8 (TIME clock) and HARDWARE_BUILD 5: (11) the Pi runs 64-bit Raspberry Pi OS (no 2038 wrap); "clock set" means the kernel reports NTP-synchronized (`adjtimex()` not `TIME_ERROR`), otherwise 83; local time follows the Pi's TZ, set at install. Lands with the Pi daemon (Someday).

**Rationale:** The implementation and both test sets already took these readings; writing them down turns "most literal" into "specified". 6 and 7 meet in the clock contract: the year bound is the clock's job (a year above 9999 reports "not set"), so the device stays a formatter with no range checks.
**Code gap:** `mailbox::local_time` does not yet report "not set" for a year above 9999 (unreachable before the year 10000). Logged in TODO, Decided, to implement; no behavior changed here.

### 2026-10-03: HEX Records: Control Characters Documented, Not Rejected
**Decision:** Mike closed the last Phase 5 wording question: a HEX record with an embedded Tab, Esc or other control character loads, because READ_LINE drops those bytes and applies BS/DEL before the loader sees the line, and the 7.1 grammar applies to the stored line. MONITOR_SPEC 7.1 now says so; the ROM is unchanged, and `hex.txt` proves it with a Tab/Esc/NUL record and a BS/DEL-corrected one.

### 2026-10-03: Phase 5 Wording Closed
**Decision:**
- MONITOR_SPEC 7.2: a failed record writes nothing outside the stack page. That's stricter than the workspace allowance, and the shipped ROM and tests already prove it.
- ARCHITECTURE 1: the loader rejects any record that *would write into* 0000-00FF or EF00-FFFF. EOF and zero-length records are accepted at any address.
- The message `Line too long` is renamed `Record too long`. It fires on LL above 22h, including short junk like `:23`, where "line" misled.
**Still open:** a record with an embedded Tab loads, because READ_LINE drops control characters first (TODO).

### 2026-10-03: Idle Wait Only While the 8080 Polls the Console
**Decision:** Mike closed the two host-only decisions left open by the 2026-10-03 implementation:
- **Idle wait trigger:** a pump waits up to 1 ms for host input only when it brought nothing, the console input FIFO is empty, AND the 8080 read `IN 02` during the last pump interval. This narrows the 2026-10-03 "Host idle CPU" decision below, which fired for any compute-bound program. Host-only (ARCHITECTURE 7.2).
- **Debugger NAME+n:** accepted as shipped. A location is named after the nearest symbol at or below it only within the same memory-map region (ARCHITECTURE 7.4 Location), so user RAM is never `STOR_ADDR+n`.

**Rationale:** The intent was "block when idle at the prompt". An empty FIFO alone doesn't mean idle; a program polling an empty FIFO does. Measured, release: a 26M-step loop went from 15x slower back to no-wait speed, and idle at the prompt is unchanged at about 27% of a core.
**Mantra check:** One bool on the Console, set by `IN 02`, read and cleared by the run loop. No timers, no heuristics on step counts.

### 2026-10-03: Host Idle, Halt Prompt, Fail-Fast Scripts, Workspace Symbols, Trace Repeats
**Decision:** Mike closed the 2026-10-03 open decisions:
- **Host idle CPU:** when a pump brings nothing and the console input FIFO is empty, block for host input up to 1 ms instead of polling with `Duration::ZERO`. Host-only (ARCHITECTURE 7.2).
- **Piped stdin EOF:** no change. A piped run ends on HLT or Ctrl-C, never at end of input (7.2).
- **WARM:** keep cold-start re-entry. No fixed WARM vector; the exerciser shim keeps `JMP F000` at 0000 (ARCHITECTURE 2).
- **Debugger:** (a) in an interactive run (terminal, no `--script`) HLT opens the `dbg>` prompt with `* halt`; piped and scripted runs still print `HLT at PC=xxxx` and exit. (b) A bad `--script` line exits with status 2; at the terminal a bad command still only prints `? message`. (c) The workspace is declared as labels (`ORG 0080H` + `DS`, matching ARCHITECTURE 1.1) so `monitor.sym` names it; ROM bytes unchanged. (d) Port-trace repeats collapse as `<line> ; xN` in the 7.3 format itself, for the debugger and the Pi daemon alike.
- **MONITOR_SPEC wording:** 4.4 rule 4 allows writes to the stack page (EF00-EFFF) as well as the workspace; 6.1 notes that C over the stack page reports the bytes its own stack use changed, as S does for its pattern copy (6.11).

**Rationale:** At the prompt the emulator spun a full host core. HLT ending an interactive session threw away the state you'd want to inspect. A script that carries on past a bad line runs with the wrong breakpoints and produces confidently wrong output. Workspace names make `w STOR_ADDR` work. One repeat rule for both tracers is what lets them diff.
**Mantra check:** No WARM vector, no EOF exit: both were "add a mechanism" options and both were declined. The workspace labels are a source-only change; the assembled ROM is byte-identical apart from the DATE/TIME stamp.

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
**Superseded** by 2026-10-02 "Console Is a Pi FIFO Device": the Pi buffers, nothing paces the sender (MONITOR_SPEC 2 and 7.4). Measured 2026-10-03 at 2.048 MHz: a 16-byte record takes 9.8-11.3 ms from its first character to WARM, 4.5-6.0 ms of it in the loader, depending on how many A-F digits it has.
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

**Monitor ROM v0.6:**
- 17 commands: A, C, D, E, F, G, H, I, L, M, O, S, T, U, W, X, ?, to MONITOR_SPEC sections 1-6, 8, 9, 11 (strict arguments, WARM, G return, memmove, Storage error, Mount failed)
- Intel HEX loader (Phase 5, MONITOR_SPEC 7): a `:` line is one record, validated in full (pass 1: steps 1-4) before the type, the guard and the write (pass 2)
- T (Phase 6, MONITOR_SPEC 6.15): mailbox `TIME`, `Service error` on 00 after execute or 80-FF
- A and U (Phase 7, MONITOR_SPEC 6.16-6.17): mailbox `ASM` and `DIS`; only `.` ends A. T, A and U share MB_SEND/MB_PUT/MB_GET, the DEVICE_SPECS 8 reference client
- ROM overlay boot mechanism; CONOUT is OUT 00 / RET, so the first Pi access after reset is the banner's OUT 00
- 2893 of 4096 bytes used (1203 free; `make size`)

**Debugger (host-side, ARCHITECTURE 7.4):** Ctrl-E / `--debug` / `--script`, break, step, registers, memory, disassembly with ROM symbols (`rom/monitor.sym`), watchpoints, I/O breaks, port trace, 256-step trace ring

**Devices:**
- Console (0x00-0x02), Storage + Mount as one device (0x08-0x0F, 24-bit / 16MB), Service Mailbox (0x10-0x13, `TIME`, `ASM`, `DIS`; the clock is a plain fn passed to `Mailbox::new`; `ASM`/`DIS` read the one opcode table in `src/disasm.rs`, which the debugger shares), System Control (0xFE-0xFF). One port map (`build_bus`) for main.rs and every harness. Device code has no terminal code; the host run loop (key map, input pump, Ctrl-C, halt) is in main.rs

**Testing (verified 2026-10-03):**
- 13 host + 130 CPU + 37 device + 34 mailbox + 41 monitor + 16 debugger + 8 terminal = 279, all passing (real-binary pty tests on Unix, strict transcript harness with a `\d` digit escape, reference-model CPU tests, port-level device tests)
- 4 `#[ignore]` exercisers (TST8080, 8080PRE, CPUTEST, 8080EXM) all pass: `scripts/fetch_exercisers.sh`, then `cargo test --release --test exerciser -- --ignored`

### In Progress

- **Phase 6:** done 2026-10-03 (Service Mailbox, `TIME`, T, v0.5).
- **Phase 7:** done 2026-10-03 (mailbox `ASM`/`DIS`, A, U, v0.6).
- **Pi daemon:** specified 2026-10-03 (`docs/PI_DAEMON.md`); code not started (`TODO.md`, Current).
- **Review findings:** 2026-10-02 review found CPU flag bugs, ROM range and parse bugs, and vacuous tests. All fixed by 2026-10-03 (steps A-E); `TODO.md` keeps the repros.

### Open Decisions

One open: the MONITOR_SPEC 6.17 U cost figure (about 9,500 cycles a line) vs about 4,450-5,600 measured on v0.6 (`TODO.md`, Open Decisions). The Phase 6 items (the 6.15 test bullet vs the injected clock, Pi "clock not set" detection and TZ, and the six literal spec readings) closed 2026-10-03; see Key Decisions. The four Phase 5 spec-wording items (the HEX guard wording, what 7.2's "nothing is written" covers, 7.1 vs the control characters READ_LINE drops, `Line too long` on short lines) closed 2026-10-03, as did the 2026-10-03 set and the two host-only follow-ups; see Key Decisions. The Pi daemon and Phase 7 sets closed 2026-10-03 too. The spec is the four normative docs: `docs/ARCHITECTURE.md`, `docs/DEVICE_SPECS.md`, `docs/MONITOR_SPEC.md`, `docs/PI_DAEMON.md`.

### Blocked/Deferred

- **R command:** Needs return mechanism (Phase 10). The debugger itself shipped 2026-10-03
- **Pi services:** one Service Mailbox at 0x10-0x13. Phase 6 `TIME` and 7 `ASM`/`DIS` done; 8 `GET` (brings the background worker and BUSY), 9 `ASK`
- **8253 timer / interrupts:** Someday

### Future Vision (Documented, Not Started)

- HTTP client, system time device, Gutenberg e-reader, Claude API device
- Hardware prototype (Pi 4B + real 8080; `docs/HARDWARE_BUILD.md`, `docs/PI_DAEMON.md`)

---

## Recent Sessions

### 2026-10-03: Phase 7 Review Fixes
- Review found no Rust or ROM bug; four small test/doc gaps, all applied. The fifth item (U's `MVI B,0` is an equivalent mutant) is left as is: 2 bytes for not leaning on the CU_BYTE loop's exit state.
- `a_runs_the_mailbox_client` now sends `  MVI A,0D  ` and asserts the wire bytes are `ASM MVI A,0D  ` (6.16 step 5: leading spaces dropped, trailing kept). The reviewer's surviving mutant (send the whole line buffer) now fails it; checked with `make -B`.
- `u_text_typed_into_a_gives_the_bytes_back` takes the instruction length from U's bytes field, not from `disasm::disassemble` (the code under test).
- MONITOR_SPEC 6.17.1 names `scripted`, not the removed `t_against`. The U cost in TODO, roadmap and log corrected to about 4,450-5,600 cycles a line (LXI SP,EFFE is the longest); the 6.17 figure stays an Open Decision.
- What bit us: ASM trims both ends on the Pi, so the stored bytes could not tell what the ROM sent; only the port sequence does. 279 tests, exercisers 4/4, clippy clean.

### 2026-10-03: Phase 7, Assemble and Unassemble (v0.6)
- Rust: `disasm::assemble` reads the OPCODES table backwards (first match from 00, so starred aliases give 08 CB D9 DD); `disasm::line` is the DIS line, moved out of the debugger, which now calls it with its symbol lookup (output and tests unchanged). Mailbox `ASM` and `DIS`: argument string after the first 20h, 82 on a bad one, never 83, no worker.
- ROM: MB_SEND/MB_PUT/MB_GET (the DEVICE_SPECS 8 reference client) and MB_HEX; T moved onto them with its port sequence unchanged; A (only `.` ends it; empty lines, `Invalid instruction` and `Service error` prompt again) and U (count in instructions, default 8). Banner v0.6. 2571 -> 2893 bytes (+322 against a +328 sketch). Measured: an A line of `MVI A,0D` 3,760 cycles, a U line about 4,450-5,600.
- Tests 266 -> 279: every DEVICE_SPECS 8 ASM/DIS vector and R1/R2 over 768 cases at port level; every MONITOR_SPEC 6.17.1 row (transcripts `assemble.txt`, `unassemble.txt`; ports, scripted, Identity and Round trip as Rust tests; rule 4 rows). The paste regression runs with LF and CR LF ends and a mid-paste failure, then `X` reports nothing mounted. Release exercisers 4/4, clippy clean.
- Mutants: Rust 62/62 after deleting three redundant checks in `assemble`/`number` that only produced equivalent mutants (bad bytes and bad operands already fail the table match). ROM 201 generated: 193 killed, 4 unviable, 2 more killed by new tests (U count high byte, a failure on U's last line), 2 equivalent.
- What bit us: U's spec figure (about 9,500 cycles a line) is about twice the measurement; logged in TODO Open Decisions rather than reworded. A mutant that drops U's `JNZ ERR_SERVICE` survived every scripted row, because the next line's execute failed the same way; only a failure on the last line shows it.

### 2026-10-03: Pi Daemon and Phase 7 Specs Integrated
- Mike accepted every recommendation in both spec sets (Key Decisions, same date). Installed `docs/PI_DAEMON.md` as the fourth normative doc, with the open-decision markers replaced by the decisions.
- Phase 7 spec text applied: DEVICE_SPECS 8 rewritten (ASM, DIS, the shared table, R1/R2, vectors, the MB_SEND/MB_PUT/MB_GET reference client); MONITOR_SPEC 3, 4.3, 4.4, 5, 6.14, 6.15, 6.16, 6.17, 9, 10, 11. The banner and Scope v0.6 edits wait for the ROM.
- Cross-doc: ARCHITECTURE 6.4 (Pi service and background work), 6.6 (the RESET check names the DS1813 premise), 7.4 (DIS line, the trace diff recipe drops `RESET`), 8; DEVICE_SPECS 3.3, 4 (arrival and RESET discard), 10, scope and conventions; HARDWARE_BUILD 5 cut to decisions and pointers, steps 5-6 use `pi8080d`; QUICK_REFERENCE, README, roadmap (Phase 7 in progress, daemon track), CLAUDE.md Status and Key Files.
- Code that does not exist yet is marked pending in the specs and listed in TODO (daemon, cross-build check, Phase 7 items 1-7). Docs only; `cargo test` green.
- Left to Mike: the musl `cargo check`/`clippy` gate belongs in CLAUDE.md's rules; it lives in PI_DAEMON 2 until he adds it. Test comments that quote the old mailbox text are listed in TODO Phase 7 item 6 rather than edited now, since the code they test has not changed.

### 2026-10-03: Loose Ends: Clippy Clean, Real-Terminal Tests
- `cargo clippy --all-targets` clean: Default for Debugger and Console (derived), Intel8080 and IoBus (`impl Default` calling `new()`: RESET state, and a 256-port array too long to derive); a `MountCase` alias in `device_tests.rs`.
- `tests/terminal_tests.rs` (8, Unix): the real binary under a pty via `rexpect` (Unix-only dev-dependency), wrapped in `sh` so each test checks the exit status and `stty -g` before = after. Raw-mode boot, echo and run, Ctrl-C mid `JMP $`, Ctrl-E / bad line / `c`, interactive HLT / `q`, Backspace 7F -> 08 at `IN 01` and in a line edit, a piped `--script` run leaving the terminal alone. No pty: skips and passes.
- Tests 258 -> 266. cargo-mutants `src/main.rs`: 8 missed -> 0 (65 caught, 5 timeouts, 6 unviable). Review: ship; 60 parallel runs without a flake, the no-pty skip checked under `sandbox-exec`, 9 hand mutations of main.rs all caught. Release exercisers 4/4.
- What bit us: rexpect's `nix` pulled the runtime `libc` 0.2.178 -> 0.2.190 in Cargo.lock. Still untested: Ctrl-D at the interactive `dbg>` (the `Flow::Quit` arm); cargo-mutants doesn't generate it.

### 2026-10-03: Phase 6 Readings Made Normative
- Mike confirmed the six Phase 6 literal readings, the 6.15 test wording and the Pi clock policy (Key Decisions, same date). Written into MONITOR_SPEC 6.15, DEVICE_SPECS 8 (a new TIME clock subsection, the precedence line, the OUT 10 rules, the implementation map) and HARDWARE_BUILD 5. Docs and test comments only; no code or ROM change.
- Verified against the code: every item matches `mailbox.rs`, `CMD_TIME` and the tests except the year bound. `local_time` still returns a year above 9999 (a 5-digit year, 20 bytes); unreachable before the year 10000, logged in TODO, Decided, to implement.
- Test comments and transcript headers that quoted the old spec text (`no NTP sync and no RTC`, `NTP or RTC`, `the line before it`, `the Pi's local time`, `The emulator uses the host clock`) now quote the new text.
- What bit us: tests that quote the spec go stale when the spec is reworded; grep the quotes after any spec edit.

### 2026-10-03: Phase 6, Time (v0.5)
- `src/io/devices/mailbox.rs` per DEVICE_SPECS 8, mapped at 10-13 by `build_bus`: 128-byte buffer (exactly 128 accepted, overflow -> 81 on execute), execute from any state, clear = the power-on state, IN 13 = 00 with no side effect outside AVAIL, command word before the first 20h, `TIME` exact or 82, empty/lowercase/unknown 80. TIME completes within the execute access, so no worker and no BUSY.
- Clock: `Mailbox::new(clock: fn() -> Option<(u16, u8, u8, u8, u8, u8)>)` (date and time fields; the device formats the 19 bytes), None = not set (83). `build_bus` passes `mailbox::local_time` (`localtime_r` via `libc`, already in the lock through crossterm; the only new direct dependency); tests pass a fixed or a failing closure. Simplest seam that makes the value and the 83 path deterministic.
- ROM: `CMD_TIME` is the reference client inline (T is its only user, no abstraction until Phase 7 brings more), `Service error` tail, dispatch, help line, banner v0.5. 2456 -> 2571 bytes (+115).
- Tests 222 -> 258: `time.txt` with a new `\d` transcript escape (any decimal digit, expected output only) so the transcript still runs on hardware; the exact reference-client port sequence on a fixed clock; 83 -> `Service error`; a scripted device for BUSY, an empty response, 00 after execute and an error mid-response. ROM mutants of T 20/20 killed; device hand mutants 13/16, the 3 survivors equivalent today. Release exercisers 4/4.
- Black-box spec tests: an independent set written from DEVICE_SPECS 8 and MONITOR_SPEC 6.15 alone, each test quoting its sentence (`tests/mailbox_tests.rs`, `mailbox.txt`, 5 T tests). They found no place where the implementation contradicts the spec. The one fix was on our side: the clock handed the device 19 preformatted bytes, so the "zero-padded" test only exercised its own formatter; formatting moved into the device, where the Pi daemon shares it. Where the two sets read ambiguous spec text, both took the literal reading; six such readings are logged in TODO Open Decisions (same-line `Service error`, placeholders = 80, NUL on 10, OUT 10 outside IDLE, 81 > 80/82 > 83, the TIME field range). Review (ship, all low): dropped three redundant T tests (a 49-mutant ROM campaign kills the same 48 without them), dropped a partial year > 9999 guard, `local_time` no longer wraps on a 32-bit time_t, all mailbox tests in one file.
- What bit us: MONITOR_SPEC 6.15 says T tests need no injectable clock and never match a value; the port tests do both. Logged in TODO Open Decisions rather than reworded. Also: "clock not set" has no meaning on the emulator host, so 83 is reachable only through an injected clock; Pi detection is open.

### 2026-10-03: Phase 5, the Intel HEX Loader (v0.4)
- 3-way implement and judge: three candidate loaders were built independently and judged against the union of their tests, 2,400 fuzzed records checked against an independent model of 7.2/7.3, and mutants. The two-pass one shipped: one labelled block per MONITOR_SPEC 7.2 step, the step 6 formula as written, and nothing written outside the stack page on any failure. The smallest candidate (+247) decoded into LINE_BUFFER during validation, which fails that reading of 7.2.
- `HEX_RECORD` in `rom/monitor.asm`: pass 1 (steps 1-4: LL, LL <= 22h, every pair and the end of line, checksum) writes nothing; pass 2 re-reads the header, runs steps 5-6 (type, guard 0100-EEFF via `DAD` with the carry as the wrap check), then writes. New `HEX_PAIR` (exactly two digits, trashes B). Grafted from another candidate: `:` is dispatched first, and the pass-1 count lives in E, so HEX_PAIR needs no PUSH B. Help line, five messages, banner v0.4. 2165 -> 2454 bytes (+289: 163 code, 87 messages, 39 help line). Measured from the line to WARM at 2.048 MHz, before READY wait states: a 16-byte record 20.0k-23.1k cycles (9.8-11.3 ms; READ_LINE, echo and dispatch 10,810, the loader 9.2k-12.3k), a 34-byte record 36.6k-43.1k (17.9-21.1 ms), the spread being the A-F digit count (TO_HEX_DIGIT 34 vs 80 cycles). About 4 KB/s of text: a full 0100-EEFF image in 16-byte records takes roughly 40 s.
- Tests 219 -> 222: `hex.txt` (every 7.5 vector, CRLF, LF and CR pastes, truncation inside a paste, step-order cases, guard edges, LINE_BUFFER targets, LAST_DUMP_ADDR and LAST_EXAM_ADDR untouched); `hex_records_are_validated_before_any_write` (from HEX_RECORD the debugger stops on any write outside the stack page and any OUT but port 00; 27 non-writing records reach WARM with their message, data records first write from HR_WRITE); `hex_guard_sweep` (5,120 records against the step 6 formula). 18/18 loader mutants killed. Release exercisers 4/4.
- Review: ship, no ROM defects. 76 valid mutants, 74 killed; the 2 survivors are equivalent (`ORA B` -> `XRA B` in HEX_PAIR; `MVI H,0` in step 6, redundant while LINE_BUFFER is in page 0, kept for clarity). Fixed: single cycle figures became data-dependent ranges, and `hex.txt` comments describe what each section covers instead of which candidate it came from.
- What bit us: no test in any candidate had a full record with both a wrong CC and trailing junk, so checking the checksum before the end of the line (step 4 before step 3) survived every test set. `:0401000001020304F0X` now kills it. Four spec-wording questions went to TODO Open Decisions, none changing the ROM.

### 2026-10-03: Idle Wait Narrowed to a Polling 8080
- The idle wait now also requires that the 8080 read `IN 02` since the previous pump: `Console::take_polled` (a flag `IN 02` sets) and the pure `idle_waits` in `src/main.rs`. Compute-bound programs never wait.
- Measured, release: the 26M-step `DCX B` loop run with `G` takes 0.20 s, the same as a build with no wait (the old trigger took 15x longer); idle at the prompt (piped stdin at EOF) about 27% of a core, unchanged.
- Tests 216 -> 219: the decision function over all 8 combinations, the run-loop wait sequence for a polling program, a compute-bound loop that never sees a wait, and the Console poll flag at port level. Each new check fails with the `IN 02` condition removed or the flag never set. Release exercisers 4/4.
- Decided: both open host-only decisions closed (Key Decisions: Idle Wait Only While the 8080 Polls the Console). ARCHITECTURE 7.2 Idle wait row says the exact trigger and the measurements.
- What bit us: a program that polls `IN 02` inside a compute loop (a break-key check) still waits 1 ms per 10,000 steps. Nothing in the ROM does that today; written into 7.2.

### 2026-10-03: The 2026-10-03 Decisions Implemented
- Host idle wait (1 ms when a pump brings nothing and the FIFO is empty); interactive HLT opens `dbg>` (piped and scripted runs still print `HLT at PC=xxxx` and exit); a bad `--script` line exits 2; piped EOF unchanged and now written down; no WARM vector, the shim keeps F000.
- Workspace declared as `ORG 0080H` + `DS` per ARCHITECTURE 1.1: `monitor.sym` gains 9 names, ROM bytes identical to the previous bin except DATE/TIME (diffed). The 7.3 repeat rule (`<line> ; xN`, decimal, no ` ; x1`) moved into 7.3; the debugger already wrote it. MONITOR_SPEC 4.4 rule 4 and 6.1 reworded.
- Tests 208 -> 216 (pump wait choice, halt-prompt decision and path, bad script exits 2, workspace symbols vs table 1.1, trace repeat format). Release exercisers 4/4.
- Review fixes: Current State open decisions rewritten; 7.2 idle wait says what was measured; 7.4 diff recipe merges stop-split runs (`uniq`); README piped vs `--script` endings; stale comments in `monitor_tests.rs` and `rom/Makefile`.
- What bit us: the decided idle trigger also fires for compute-bound programs (empty FIFO): a 26M-step loop runs about 15x slower in release. And workspace labels made "nearest symbol" name user RAM (`0100 STOR_ADDR+19`), so NAME+n now stays within one memory-map region. Both shipped and logged in TODO Open Decisions for Mike.

### 2026-10-03: The ROM to MONITOR_SPEC (step E)
- `rom/monitor.asm` rewritten on the review's prototypes: WARM (`LXI SP` before MAIN_LOOP) that every command and error tail jumps to, one tail per message, no POP cleanup chains. One parser (C:D:E, digit limit in B) for READ_HEX_WORD and READ_HEX_ADDR24 with the absent (CY Z) / invalid (CY, NZ) contract and the terminator check; READ_HEX_BYTE on top for byte forms. One RANGE helper for C, D, F, S; both `CPI 0F0H` heuristics gone.
- Behavior to spec: Invalid range (end < start, count 0 incl. M), 4/6 digit limits, bytes > FF rejected, present-invalid never absent, G pushes WARM and rejects junk, L/W parse everything before any port write and check status after (W after the flush), X resyncs with OUT 0E,03 and prints Mount failed, M copies backward when dst > src, E per 6.3 (CR stores and advances, LF ignored, DEL deletes), READ_LINE DEL = BS, CONOUT = OUT 00 / RET. Dead CONST, PRINT_BANNER, BUFFER_PTR, File not found and the size guard deleted; ROM_END + `make size`. 2511 -> 2165 bytes.
- Tests: new transcript lines for the spec behaviors above (new `go.txt`), plus monitor tests for D line counts to FFFF, the G entry contract, Mount failed (a directory), Storage error (host unmounts mid-transfer), L/W port sequences, boot I/O = OUT FE then OUT 00 only, and a debugger-driven test that 43 argument errors reach WARM with no port written and no write outside the workspace and stack. 18 of the 23 monitor tests fail on the old ROM. Hand mutants of the new ROM: 61/61 killed.
- Review: no ROM defects; 151 hand mutants, 136 killed, 5 equivalent, 2 unreachable with this device, 8 real test gaps. Closed with transcript lines (E: CR with no digits, `12 BS` CR, `1 BS` CR, `0` CR on nonzero bytes; M/L/W count 100; lowercase `x`) and asserts (`C 0000 FFFF 0001` ends with the FFFF/0000 pair; boot leaves 0100-EEFF untouched); each of the 8 plus boot `LXI SP,EF00` re-checked as killed. The exerciser shim's F000-vs-WARM conflict moved from a silent [x] to TODO Open Decisions.
- What bit us: `make` decides by timestamp, so a mutant written in the same second as the previous build ran the old binary; mutation runs need `make -B`. A mutation script crash left the scratch ROM mutated, so the next run measured mutants on a broken base; restored and rerun. `C 0000 FFFF 0000` reports EFFC-EFFF because C's own pushes change the stack page mid-compare: true to the code, but the spec doesn't say it (logged, with 4.4 rule 4's missing stack page, in TODO Open Decisions).

### 2026-10-03: The Debugger (step D)
- Spec first: ARCHITECTURE 7.4 (Ctrl-E, `--debug`, `--script FILE`; 15 short commands; exact output formats; stop report = reason, last 8 ring steps, registers, next instruction). Host-only, no new dependencies. `src/disasm.rs` (256-entry table checked against the reference), `src/debugger.rs` (breaks, watchpoints, I/O breaks, port trace collapsing repeats to ` ; xN`, 256-step ring, symbols), main.rs (Ctrl-E, line-mode prompt, script then terminal).
- Watchpoint mechanism: the CPU lists each step's data transfers (`Intel8080::transfers()`: memory reads and writes, IN, OUT; opcode and operand fetches left out). One hook serves watchpoints, I/O breaks and the trace. Push and XTHL now write in 8080 bus order (high byte to SP-1 first); no state or cycle changes.
- `rom/monitor.sym` from asl's NoICE output (code labels only; EQUs mix addresses with ports and characters), committed with `monitor.bin`. Host choices inside the mandate: a halt is still not a debugger stop (7.2 unchanged); a bad script line prints `? ...` and the script goes on; no debugger reset in v1.
- Tests: 12 debugger tests (exact stop report, the `L 0 0200 0` wrap caught writing 0080, an I/O break on 0E, the mount trace, break at `CMD_DUMP`, the binary run with `--script`/`--debug`/bad arguments), a transfers table in the CPU tests, 3 run-loop tests. cargo-mutants: debugger + disasm all caught; main.rs survivors only on tty-only paths (raw mode, terminal read_line).
- Review fixes: only address operands (a16) print as symbols, so `LXI SP,F000` no longer reads `LXI SP,COLD_START`; `monitor.sym` lines are zero-padded (asl writes `0x80`, which would have panicked the loader once a label sits below 1000); a stop report starts on its own line (CR LF when console output is mid-line); 7.4 corrected on the 8228 (operand fetches are plain MEMR, leaving them out is our choice) and on diffing against a Pi trace (drop FE/FF, ignore ` ; xN`).
- What bit us: ROM tests that pin whole traces churn with every pending ROM fix (CONOUT still polls IN 02 before each OUT 00), so ROM-side debugger tests assert the stable parts and RAM programs pin the exact formats. Pushing a byte order the real chip doesn't use would have made watchpoints report the wrong byte first.

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
