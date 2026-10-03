# TODO

## Open Decisions (Mike decides before anyone codes against them)
- None. All closed 2026-10-02; see COLLABORATION_LOG Key Decisions. The specs are docs/ARCHITECTURE.md, docs/DEVICE_SPECS.md, docs/MONITOR_SPEC.md.
- Host idle CPU (2026-10-03): the pump polls with `Duration::ZERO`, so waiting at the prompt burns a full host core. ARCHITECTURE 7.2 doesn't fix the poll duration. Option: when a pump brings nothing and the console FIFO is empty, block in poll(1 ms). Host-only; the 8080 can't see wall time.
- Piped stdin EOF (2026-10-03): a piped run never exits at end of input; it waits at the prompt until HLT or Ctrl-C. Exit at EOF (risks cutting output the CPU hasn't printed yet), or leave it?
- Spec wording, ROM (2026-10-03, step E). Docs only, the code is not in question: (a) MONITOR_SPEC 4.4 rule 4 says an argument error writes no memory outside the workspace, but every command pushes to the stack page (EF00-EFFF); add "and the stack page"? The step-E watchpoint test allows EFxx. (b) MONITOR_SPEC 6.1 says `C 0000 FFFF dest` compares 65536 bytes; it does, but C's own pushes change EFFC-EFFF while it runs, so a compare that covers the stack page reports them (as S reports its own pattern copy, 6.11). Note it in 6.1, or leave it?
- Exerciser shim exit (2026-10-03, step E review): this file said the shim's 0000 should JMP to WARM, but ARCHITECTURE 2 says WARM has no fixed address, so the shim jumps to F000 (cold start: banner, workspace reset). Publish a fixed WARM vector (e.g. `JMP WARM` at F003, a memory-map change) or keep cold-start re-entry?
- Debugger (2026-10-03, host-only unless noted): (a) HLT opens the prompt instead of exiting (changes 7.2)? (b) a bad `--script` line quits instead of going on? (c) workspace as labels (`ORG 80h` + `DS`) in `monitor.asm` so `w STOR_ADDR` works: a ROM source change; the Makefile already pads low addresses. (d) does the Pi daemon trace collapse repeats with ` ; xN` like the debugger (7.3 leaves event choice to each tool)?

## Current
- [x] Apply alignment package (archived: docs/archive/HANDOFF_2026-10.md)
- [x] Untrack build artifacts: `git rm --cached rom/monitor.lst rom/monitor.p`
- [x] Spec/state review (2026-10-02). Decisions in COLLABORATION_LOG Key Decisions.
- [x] Rebuild stale `rom/monitor.bin` (it predated `X -` unmount)
- [x] Close open decisions; solidify spec into 3 normative docs (2026-10-02)
- [x] Code review: emulator + ROM against the new specs (steps A-E, 2026-10-03)
- [x] Spec the debugger (ARCHITECTURE Host-Side Conveniences), then build it before Phase 5: core + watchpoints + I/O break/trace + trace ring + ROM symbols; line prompt + script mode. Done 2026-10-03: ARCHITECTURE 7.4, `src/debugger.rs`, `src/disasm.rs`, `rom/monitor.sym`, `tests/debugger_tests.rs`
- [x] Hardware-alignment pass (2026-10-02): design buildable with one hard fix (WAIT set via STSTB, not SYNC). Spec edits in ARCHITECTURE section 6 and DEVICE_SPECS; BOM and bring-up in docs/HARDWARE_BUILD.md
- [x] 8080A/8224/8228 hardware reference: docs/reference/8080_HARDWARE.md

## Next (Phase 5 - Intel HEX Loader), build order
Budget: ~250 bytes of the 1931 free (a working sketch came in at 176). The parser it needs exists: READ_HEX_WORD's absent/invalid contract and the shared error tails.
1. [x] Fix the vacuous tests (Review findings) and add a monitor test that maps storage and checks `X -` prints `Unmounted`. That test is the guard against shipping a stale bin. (2026-10-02: `tests/transcripts/storage.txt`)
2. [x] Edge tests for what the loader leans on: READ_LINE (full buffer, BS, CR/LF/CRLF) and TO_HEX_DIGIT (lowercase, invalid): `line_input.txt`, `hex_math.txt`. DEL shipped with the READ_LINE DEL fix (2026-10-03).
3. [ ] MAIN_LOOP: a line starting with `:` dispatches to the loader.
4. [ ] `GET_HEX_BYTE`: exactly 2 digits from (HL) into A, add to running sum in C, HL+=2, carry on error. Unit-test via monitor.
5. [ ] Pass 1: line length vs LL (max 34 data bytes, else "Line too long"), checksum (sum of all bytes == 0), type 00/01 only.
6. [ ] Pass 2: range guard (reject records touching 0000-00FF or EF00-FFFF), write type 00 data.
7. [ ] Type 01 (EOF) message; distinct error strings.
8. [ ] Integration tests: multi-record paste with CRLF, bad checksum mid-stream, 34-byte record OK, 35-byte rejected, page-00 and EFxx rejected, types 02-05 rejected.
9. [ ] Rebuild `monitor.bin`, record size, update docs.

## Decided, to implement (from the specs; small, not Phase 5)
Emulator:
- [x] Delete `src/io/devices/timer.rs` and its port hooks in `src/cpu.rs` (ports 0x30-0x32, tick at ~:947). Keep a CPU interrupt input `interrupt(rst)` with 8080A acceptance (EI delay, HLT wake, 11 cycles); tests only.
- [x] HLT: `execute_one` fetches nothing while halted; `run()` returns on halt; main.rs prints `HLT at PC=xxxx`, restores the terminal, exits; `perform_hlt` stops printing.
- [x] Host input pump + Ctrl-C quit in the emulator run loop; host key map per ARCHITECTURE; the run loop returns a quit/halted status (ARCHITECTURE 7.2). Done 2026-10-03: `run_loop`/`map_key` in `src/main.rs`; piped stdin feeds the console unmapped
- [x] Console output 8-bit transparent (was `value as char`, so bytes 80-FF went out as 2 UTF-8 bytes)
- [x] `IoBus::map_port` panics on 0xFE/0xFF (CPU-owned ports) instead of silently never being called
- [x] Power-on: `reset()` = RESET pin only (PC, INTE, halted, overlay, pending interrupt); `new()` calls `reset()`; no zeroed registers or preset SP; the harness fills RAM with junk (ARCHITECTURE 3.1). Harness side done: RAM and A-L = 76, flags = D7, SP = 0000 at boot
- [x] Merge Storage + StorageMount into one device serving 08-0F (drop the `Rc<RefCell<Storage>>`)
- [x] One Console (input VecDeque + output buffer, no crossterm); host polling, key map and Ctrl-C/E in main.rs; delete `test_console.rs` (and the unused `null.rs`)
- [x] `scripts/fetch_exercisers.sh` (pinned SHA-256) + `#[ignore]` `tests/exerciser.rs`. Write the CP/M shim in 8080 assembly, not Rust hooks, so the same bytes run on hardware: 0005 JMP to a print routine at <= EEFF; 0000 JMP to WARM (not HLT, which hangs hardware with no INT source). Goal: 8080EXM all PASS; the exercisers are also the chip-acceptance test. Done 2026-10-03: all four pass. The shim's 0000 jumps to F000 (cold start), not WARM: WARM is not a fixed address (ARCHITECTURE 2). This item and ARCHITECTURE disagree; logged in Open Decisions.
- [x] Console: 2 MiB output cap, discard on full; clear output on reset (reset rebuilds the Console)
- [x] Device reset = rebuild the IoBus and devices from config (no `reset()` on IoDevice, no Send). Storage `Drop` does `File::sync_all`; flush uses `sync_all` (`File::flush` was a no-op)
- [x] One port-mapping function shared by main.rs and a future Pi daemon binary (one crate, two binaries). Done 2026-10-03: `build_bus` in `src/io/mod.rs`, used by main.rs and all three harnesses
- [x] Strict monitor harness stores transcripts as data files, so the same files drive hardware conformance over the Pi TCP console (`tests/transcripts/*.txt`, format in the `tests/monitor_tests.rs` header)
- [x] Port 0xFE: any write disables overlay; drop 0xFF cold reset (`src/cpu.rs:758-769`). Overlay writes go through to RAM (`src/cpu.rs:212`)
- [x] Storage: every IN/OUT 0B advances the address (mounted or not); any host I/O error unmounts; flush/unmount fsync; host file > 16 MB fails mount with 01. Tests: power-on 0C=82, storage dir created at startup (now in `Storage::new`)
- [x] Mount: names > 12 chars -> 02 (was truncated); failed mount unmounts previous; every OUT 0E (any value) clears the name buffer; uppercase before validate/open; 0F reads 01 at power-on
ROM:
All done 2026-10-03 (step E), with transcripts or monitor tests for each:
- [x] G pushes WARM (`WARM: LXI SP,STACK_TOP` before MAIN_LOOP) before PCHL
- [x] CONOUT = `OUT 00h / RET`. The first Pi access after reset is now the banner's OUT 00 (`boot_io_is_out_fe_then_console_output`)
- [x] X: `OUT 0Eh,03h` before the name; 02 -> "Invalid filename", other nonzero -> "Mount failed"; "File not found" deleted. `io_break_and_port_trace_of_a_mount` has the `OUT 0E 03` line
- [x] L/W: read 0C after transfer (W after flush); bit 0 = 0 -> "Storage error"
- [x] Argument strictness per MONITOR_SPEC (Invalid range for end<start and count 0 incl. M; digit limits; byte args > FF; G garbage)
- [x] M copies backward when dst > src (memmove)
- [x] Delete unused `BUFFER_PTR` equate; `ROM_END` label, `make size` reports used bytes
- [x] `monitor.asm` header: memory map per ARCHITECTURE 1, routine-contract convention stated

## Review findings (2026-10-02)
Every item was reproduced by a scratch test or confirmed by tracing the asm.

### CPU (`src/cpu.rs`)
- [x] AC inverted on SUB/SBB/CMP/SUI/SBI/CPI (:349, :356, :374, :613, :643, :750). Repro: `MVI A,10h; MVI B,01h; SUB B` sets AC=1, expected 0. `tests/cpu_tests.rs:317` and `:2853` assert the wrong value.
- [x] DCR AC inverted (:422). Repro: `MVI B,10h; DCR B` sets AC=1, expected 0. `tests/cpu_tests.rs:421` asserts the wrong value.
- [x] ANA/ANI always clear AC (:362, :622 via `update_flags_logical` :272). Repro: `MVI A,08h; MVI B,00h; ANA B` gives AC=0, expected 1 (8080: AC = bit 3 of A|operand).
- [x] DAA always clears AC (:712). Repro: `MVI A,0Ah; DAA` gives A=10h (correct) and AC=0, expected 1.
- [x] POP PSW keeps flag bits 3 and 5 (`set_psw` :89). Repro: FFh FFh on the stack, `POP PSW; PUSH PSW` pushes flags FFh, expected D7h. `tests/cpu_tests.rs:2605` is vacuous: it writes 00h at the wrong SP.
- [x] Undocumented aliases panic (:944). Repro: `CB 05 00` panics "Unknown opcode", expected JMP. Same for D9 (RET) and DD/ED/FD (CALL).
- [x] EI has no one-instruction delay (:805, interrupt check at the top of `execute_one`). Repro: pending interrupt, `EI; MVI A,1` is taken before MVI runs. Breaks `EI; RET` ISR epilogues.
- [x] HLT is terminal (`run` :284, `handle_interrupt` :290). An interrupt never clears `halted`; `execute_one` keeps executing past HLT. Repro: `76 3E 01 76`, execute_one x2 gives A=01, expected 00.
- [x] Interrupt acknowledge adds 0 cycles (`handle_interrupt` :290). Expected 11 for RST.

### Devices
- [x] `..` and `.` pass validation, mount fails with 0x01 instead of 0x02. Repro: name `..`, mount: status 01. Fixed 2026-10-03.
- [x] Flush and write errors are swallowed (flush had no fsync). W prints "Written" regardless. Device side fixed 2026-10-03 (errors unmount); the W status check is the ROM item above.
- [x] `map_port` silently ineffective for 0x30-0x32, 0xFE and 0xFF: the CPU intercepts them first (`cpu.rs:758-782`). 0x30-0x32 goes away with the timer deletion.
- [x] Ctrl-C can't stop a program that doesn't poll the console. Repro: `G` into `JMP $`, then ^C: the process keeps running. Fixed 2026-10-03 (host pump).
- [x] Control keys mangled. Repro: Ctrl-A arrives as 0x61, Ctrl-S as 0x73; Esc and Tab are dropped. Fixed 2026-10-03 (`map_key`).

### Monitor ROM (`rom/monitor.asm`)
All fixed 2026-10-03 (step E); line numbers refer to the old ROM. Each repro is now a transcript line.
- [x] L/W count 0 moves 65536 bytes (:1351-1358, :1420-1427). Repro: `L 0 0200 0` wipes the workspace; the next `I 02` cold-boots.
- [x] L/W junk count silently means 256 (:1339, :1408). Repro: `L 0 0200 ZZ` loads 256 bytes.
- [x] D wrap heuristic stops early (:750-756). Repro: `D 0 F000` prints one line.
- [x] S same heuristic (:1123-1129). Repro: `F 0500 0502 77`, then `S 0000 FFFF 77 77 77` finds nothing.
- [x] F with end < start fills to FFFF (:847-861). Repro: `F 0300 0200 AA` fills 0300-EFFF, stack included.
- [x] C with end < start does 64K compares (:561-568). Repro: `C 0201 0200 0300` prints thousands of lines.
- [x] Too many hex digits are silently truncated (READ_HEX_WORD :360-415, READ_HEX_ADDR24 :460-530). Repro: `F 10200 1020F 1AA` fills 0200-020F with AA.
- [x] G with a bad argument runs 0100 (:879). Repro: `G ZZ`.
- [x] E: LF counts as "advance", and CR after a 2-digit value skips a byte (:1171, :1195-1198). Repro: `E 0200`, then `12` CR `34` CR `.` writes 0200=12, 0202=34.
- [x] READ_LINE stores DEL (0x7F) as a character (:299-302). Real terminals send DEL for backspace. Repro: `D 020<DEL>0 0200` prints "Invalid address".
- [x] M with an overlapping dest > src corrupts (:953). Repro: `M 0200 0201 0F` smears 0200. Documented as "forward copy" only.
- [x] `make size` always reports 4096 because p2bin pads (`rom/Makefile:34-36`).

### Test gaps
- [x] Vacuous: `test_search_finds_pattern` (`tests/monitor_tests.rs:85`) matches the echoed `F 0500` line; `test_io_read_status` (:107) matches "02" in the banner date; `test_compare_identical` (:95) only counts prompts.
- [x] No monitor test maps storage, so L/W/X in the ROM are untested. This is how the stale bin slipped through.
- [x] Untested: interrupts (RST entry, EI delay, HLT wake, DI blocking), with the interrupt rework. Done 2026-10-03: interrupts and overlay write-through. Done 2026-10-02: cycle counts (all 244 documented opcodes), the overlay at CPU level (reads, IN FF, OUT FE 00), the E command (one-digit entries), every error path that is correct today, range edges. Done 2026-10-03 with the ROM fixes: end < start, FFFF wrap, two-digit E entries.

## Code Review (2026-10-02)
Method: a 16-agent review workflow (CPU first, then ROM and devices) with every finding adversarially verified; real `cargo-mutants`; the four standard 8080 exercisers run under a CP/M shim. Only new findings are listed here; the items above still stand.

### Evidence
- **Exercisers:** TST8080 and 8080PRE pass, with cycle totals matching the reference exactly. CPUTEST fails on `DCR B` AC (flags 46h, expected 56h). 8080EXM fails 11 of 24 groups (aluop nn, aluop reg, daa/cma/stc/cmc, inr/dcr on all 8 targets); every failure is the tracked AC family.
- **cargo-mutants** (CPU + devices): 649 of 932 caught (70.5%). Survivors cluster in perform_alu and the immediate ALU ops (98), dead debug code (32), registers.rs metadata (50) and storage (14).
- **Hand mutation, CPU tests only:** 26% (35% with the monitor tests). **ROM:** 10.6% (7/66), and a strict-transcript harness kills 60/66. After step E (2026-10-03): the implementer's 61 hand mutants of the rewritten ROM all killed. The review's broader campaign (151 valid mutants, every command, parser, RANGE, tails, messages) killed 136; of the 15 survivors 5 are equivalent (DI, stub RET->RNZ, `'z'+1`->`'z'`, ORA->ADD in the E digit insert, bare-D 7F->70), 2 are unreachable with this device (X query and mount status masks; it only returns 00/01/02) and 8 were test gaps (lowercase x fold, E CR-with-no-digits, E `0` CR, E BS mask, M/L/W count low byte 00, C over 65536 bytes). New transcript lines and asserts kill all 8, plus boot `LXI SP` -> EF00 (`boot_assumes_nothing` now checks 0100-EEFF untouched). Before it, after the strict harness shipped: ROM 59/65 hand mutants killed; the 6 survivors are equivalent (DI, `'z'+1`, bare-D 7F->70, E BS mask), unobservable (W flush) or reachable only through end < start (F wrap guard). cargo-mutants `cpu.rs` 647/752; the misses are the tracked AC family, `|`/`^` equivalents, overlay write-through, OUT FE=FF, interrupts and dead debug code.
- **Exhaustive conformance** (all ALU ops x A x v x CY, INR/DCR, DAA, all 256 opcodes for cycles and length, address wrap): the only CPU violations are the ones already tracked. Cycle counts and wrap are correct.

### Tests that don't test (fix before any code change)
- [x] Zeroed RAM is a NOP slide into F000. Removing the overlay or emptying `reset()` passes every test (`cpu.rs:1109`, `memory.rs:12`). Fill RAM with junk (76 or A5) and SP with junk in the monitor harness boot. Add a CPU-level overlay test: read 0100 = rom[100]; write 55; OUT FE nonzero; read 0100 = 55; IN FF bit 0 is 1 then 0. Done 2026-10-02: junk is 76 (HLT), not A5: A5 is ANA L, which slides into F000 like a NOP; `boot_fails_without_overlay` guards it. The write-55 and nonzero-OUT-FE parts ship with the overlay fix.
- [x] Monitor harness: replace the fixed `run_cycles` budget with "run to the Nth prompt, exact transcript between prompts, budget exhausted = failure", plus memory and port effects at both range edges (`tests/monitor_tests.rs:10-30`).
- [x] RST tests are vacuous on the vector (`cpu_tests.rs:842-1035`). Mutant `n*8 -> n*7` survives. Assert PC, SP and the pushed address.
- [x] Conditional CALL/RET taken paths are untested (`cpu_tests.rs:797-830`). `test_rp_not_taken`/`test_rpo_not_taken` actually take the return, because MVI doesn't set flags (`:1549-1594`). Several Ccc tests target the wrong address (`:1414, :1429, :1443, :1458`). Replace with a table: 8 conditions x {Jcc, Ccc, Rcc} x {taken, not}, with flags set explicitly.
- [x] No-assert or off-topic tests: `test_in_instruction`/`test_out_instruction` (`:1769-1792`), `test_mov_all_to_m` (`:2235`), `daa_preserves_flags`, `all_arithmetic_preserve_bit1`. Duplicates: `:1167 = :1912`, among others.
- [x] Monitor tests: `test_dump_rom` (`:42-50`) passes with wrong line length and format, and its "31" check tests ROM layout. `test_move_command`/`test_hex_math` ignore boundaries and format. C, S and I can each be undispatched with the suite green.
- [x] Storage mount tests never check which file opened: ignoring `base_path` passes and writes NEW.BIN/TEST.BIN into the crate root (`storage_mount.rs:60` vs tests `:128-164`). Device protocols: 16 of 18 port-level mutants survive. Write port-level tests with no private-field access. Done 2026-10-02 (`tests/device_tests.rs`): cargo-mutants on storage + mount 74/80; the misses are flush (unobservable), `|` vs `^` on masked bytes (equivalent), and the 12-char truncation (tracked bug).
- [x] `tests/common/mod.rs` is dead, a byte-identical copy of `cpu_tests.rs:1-33`.
- [x] Wanted: a table-driven reference-model flag test, a 256-opcode cycle/length table test, and an exerciser test (see Open Decisions). The scratch prototypes exist and run in under 1 s. Done 2026-10-02: the reference model (exhaustive ALU, INR/DCR, DAA, DAD, rotates; AC masked for the tracked AC family until its fix) and the cycle/length table for the 244 documented opcodes. Done 2026-10-03: the aliases (all 256 opcodes in the table) and `tests/exerciser.rs` (all four pass).

### CPU (`src/cpu.rs`)
- [x] Untested and mutant-proven: DAD never clearing CY (`:455`), ADC/ACI AC carry-in (`:343, :740`), the CY boundaries on ADC/ADI/SBB/SUI/SBI/DAD, S after logical ops (`:276`), and ORI (`test_ori` uses F0|0F, where OR = XOR).
- [x] Interrupt acknowledge is a hand-written CALL 0038 fused with the vector's first instruction in one step (`:290-303, :826-830`). Spec: acknowledge executes `RST n` as its own 11-cycle step. Use `perform_rst`.
- [x] Pattern: the ALU is implemented twice. 8 immediate functions (`:601-647, :736-754`) duplicate `perform_alu` (`:328-385`), which is why the AC bug has 6 copies. Use one `alu(op, v)` for 10AAASSS and 11AAA110. Also: three SZP flag helpers that are the same code, dead DAA carry re-set, and the push/pop sequence copied 8 times. A scratch refactor came out at +71/-455 lines with tests green.
- [x] Dead code: `disassemble_at` (decodes 19 of 256 opcodes), `trace`, `debug_state` (`:958-1080`); about 85% of `registers.rs` (`FLAG_BIT_3`/`FLAG_BIT_5` are constants equal to 0); the `Memory` trait with one impl forcing `&mut self` reads (`memory.rs`). Stale edit-marker comments.

### ROM (`rom/monitor.asm`)
All done 2026-10-03 (step E): the WARM/error-tail/one-parser/RANGE refactor from the review prototypes, applied with the behavior fixes. 2511 -> 2165 bytes.
- [x] S: an invalid pattern token ends the pattern instead of erroring (`:1057-1061`). `S 0200 0210 AA ZZ` searches for AA. Strict parsing alone doesn't fix it.
- [x] S also tests the candidate end+1: the loop compares before it checks `current > end` (`:1081-1129`). Repro: `F 0500 0502 41`, then `S 0400 04FF 41` prints `0500`. Found 2026-10-02 by the strict harness. The RANGE helper rewrite should fix it; add `S 0400 04FF 41` to `search.txt` with the fix.
- [x] E ignores DEL (`:1173-1177`); spec 6.3 says DEL deletes a digit like BS. Fix with READ_LINE's DEL.
- [x] ROM size guard `IF $ > 0FFFFH` (`:1515-1517`) rejects an exactly-4096-byte ROM, and asl already errors past FFFF. Delete it.
- [x] Dead CONST routine (`:194-202`); single-use PRINT_BANNER wrapper.
- [x] Byte budget, all prototyped with golden transcripts identical:
  - 23 copies of a 9-byte error tail plus POP-cleanup chains. With WARM, use one tail per message and delete every cleanup POP.
  - 19 redundant `CALL SKIP_SPACES` (57 bytes).
  - READ_HEX_ADDR24 duplicates READ_HEX_WORD (89 bytes); use one C:D:E parser returning the digit count.
  - TO_HEX_DIGIT 42 -> 19 bytes.
  - The D default paths are duplicated (21 bytes), the 'AAAA:' prefix is copied 4 times (18), and the I/O stub init is 30 bytes of MVI/STA.
- [x] Contracts: READ_HEX_WORD returns the same flags for "absent" and "junk". Adopt Z=absent / C=invalid (7 peek sites collapse; fixes G, L/W count and S). One RANGE helper (4 uses: C, D, F, S) fixes all four tracked range bugs for -3 net bytes and deletes both `CPI 0F0H` heuristics.

### Devices
- [x] Console and TestConsole duplicate the protocol. The shipped Console has zero coverage and pulls crossterm into the device layer the Pi reuses. Fixed 2026-10-03: one Console, tested at port level.
- [x] StorageMount holds `Rc<RefCell<Storage>>`: it is !Send and splits one Pi device across two structs. Fixed 2026-10-03: one device.
- [x] The W flush (OUT 0C <- 02) can't be observed by any test. Done 2026-10-03: the monitor harness records every port access, and `load_and_write_port_sequences` pins `OUT 0C 02` before the status read. Durability itself is still unobservable (no fault injection).
- [x] IN 02 costs about 1.2 ms of wall time (a 1 ms poll), so console output is throttled to about 850 chars/s. Fixed 2026-10-03: `poll(Duration::ZERO)` every 10,000 steps in the host pump.

## Blocked
- [ ] R command - needs register capture on return (Phase 10)

## Someday
- [ ] Phase 6: Service Mailbox device (0x10-0x13), ROM mailbox client, `TIME` + T
- [ ] Phase 7: Assembler/disassembler (mailbox `ASM`/`DIS`)
- [ ] Phase 8: HTTP GET (mailbox `GET`)
- [ ] Phase 9: Claude (mailbox `ASK`, Q command)
- [ ] Phase 10: R command (the debugger shipped early, ARCHITECTURE 7.4)
- [ ] 8253 timer, TIMER_ISR, RST 7 vector: when something needs a periodic interrupt
- [ ] Hardware prototype: see docs/HARDWARE_BUILD.md (BOM, 9-step bring-up)
- [ ] Pi daemon binary (busy-poll /dev/gpiomem, TCP console, shared device code)
- [ ] Pi-assisted hardware single-step (needs A8-A15 on the Pi; not in v1)
- [ ] Interrupt tick source (Pi GPIO vs 8254) when something needs a periodic interrupt
