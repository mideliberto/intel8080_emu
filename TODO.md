# TODO

## Open Decisions (Mike decides before anyone codes against them)
- [ ] `Q` collision. Q = ask Claude (decided 2026-10-02), but QUICK_REFERENCE's old future list had Q = Quit emulator. Quitting is emulator-only; it probably belongs with the Phase 10 emulator-command prefix, not the ROM.
- [ ] Mount filename buffer resync. The buffer is cleared only by Mount (`src/io/devices/storage_mount.rs:103`); NUL is ignored (`:93`). An aborted name send leaks into the next mount: `JUNK`, unmount, `A.BIN`, mount mounts `JUNKA.BIN`. Option: NUL clears the buffer and the ROM sends NUL first (protocol change).
- [ ] Filename case. The ROM passes names verbatim, so `test.bin` and `TEST.BIN` are the same file on macOS and different files on a Pi's ext4. Option: the device uppercases before open.
- [ ] Phase 10 emulator-command prefix (`:` now belongs to Intel HEX). Not needed until Phase 10.
- [ ] Hardware build: console chip and flow control (6850 + RTS/CTS vs Pi-side FIFO console). Phase 5 assumes sender pacing. Note: the ROM does no UART init at boot (`rom/monitor.asm:79-102`); a 6850 or 8251 needs one.

## Current
- [x] Apply alignment package (archived: docs/archive/HANDOFF_2026-10.md)
- [x] Untrack build artifacts: `git rm --cached rom/monitor.lst rom/monitor.p`
- [x] Spec/state review (2026-10-02). Decisions in COLLABORATION_LOG Key Decisions.
- [x] Rebuild stale `rom/monitor.bin` (it predated `X -` unmount)

## Next (Phase 5 - Intel HEX Loader), build order
Budget: ~250 bytes of the 1585 free (a working sketch came in at 176).
1. [ ] Fix the vacuous tests (Review findings) and add a monitor test that maps storage and checks `X -` prints `Unmounted`. That test is the guard against shipping a stale bin.
2. [ ] Edge tests for what the loader leans on: READ_LINE (full buffer, BS, CR/LF/CRLF, DEL) and TO_HEX_DIGIT (lowercase, invalid).
3. [ ] MAIN_LOOP: a line starting with `:` dispatches to the loader.
4. [ ] `GET_HEX_BYTE`: exactly 2 digits from (HL) into A, add to running sum in C, HL+=2, carry on error. Unit-test via monitor.
5. [ ] Pass 1: line length vs LL (max 34 data bytes, else "Line too long"), checksum (sum of all bytes == 0), type 00/01 only.
6. [ ] Pass 2: range guard (reject records touching 0000-00FF or EF00-FFFF), write type 00 data.
7. [ ] Type 01 (EOF) message; distinct error strings.
8. [ ] Integration tests: multi-record paste with CRLF, bad checksum mid-stream, 34-byte record OK, 35-byte rejected, page-00 and EFxx rejected, types 02-05 rejected.
9. [ ] Rebuild `monitor.bin`, record size, update docs.

## Decided, to implement (small; not Phase 5)
- [ ] G pushes a WARM entry (`WARM: LXI SP,STACK_TOP`, falls into MAIN_LOOP) before PCHL, so programs exit with RET (`rom/monitor.asm:876-885`)
- [ ] Delete `src/io/devices/timer.rs` and its hooks in `src/cpu.rs` (ports 0x30-0x32, `handle_interrupt`, tick at ~:947)
- [ ] Port 0xFE: any write disables overlay; drop 0xFF cold reset (`src/cpu.rs:758-769`). Overlay writes go through to RAM (`src/cpu.rs:212`)
- [ ] Storage: names >12 chars return 0x02 (`storage_mount.rs:95` truncates, so the `:49` check never fires); a failed mount unmounts the previous file; a past-EOF read still advances the address (`storage.rs:72-82`)
- [ ] ROM: drop the dead "File not found" path (`rom/monitor.asm:1274-1277`, MSG_NOT_FOUND)

## Review findings (2026-10-02)
Every item was reproduced by a scratch test or confirmed by tracing the asm.

### CPU (`src/cpu.rs`)
- [ ] AC inverted on SUB/SBB/CMP/SUI/SBI/CPI (:349, :356, :374, :613, :643, :650). Repro: `MVI A,10h; MVI B,01h; SUB B` sets AC=1, expected 0. `tests/cpu_tests.rs:317` and `:2853` assert the wrong value.
- [ ] DCR AC inverted (:422). Repro: `MVI B,10h; DCR B` sets AC=1, expected 0. `tests/cpu_tests.rs:421` asserts the wrong value.
- [ ] ANA/ANI always clear AC (:362, :622 via `update_flags_logical` :272). Repro: `MVI A,08h; MVI B,00h; ANA B` gives AC=0, expected 1 (8080: AC = bit 3 of A|operand).
- [ ] DAA always clears AC (:712). Repro: `MVI A,0Ah; DAA` gives A=10h (correct) and AC=0, expected 1.
- [ ] POP PSW keeps flag bits 3 and 5 (`set_psw` :89). Repro: FFh FFh on the stack, `POP PSW; PUSH PSW` pushes flags FFh, expected D7h. `tests/cpu_tests.rs:2605` is vacuous: it writes 00h at the wrong SP.
- [ ] Undocumented aliases panic (:944). Repro: `CB 05 00` panics "Unknown opcode", expected JMP. Same for D9 (RET) and DD/ED/FD (CALL).
- [ ] EI has no one-instruction delay (:805, interrupt check at the top of `execute_one`). Repro: pending interrupt, `EI; MVI A,1` is taken before MVI runs. Breaks `EI; RET` ISR epilogues.
- [ ] HLT is terminal (`run` :284, `handle_interrupt` :290). An interrupt never clears `halted`; `execute_one` keeps executing past HLT. Repro: `76 3E 01 76`, execute_one x2 gives A=01, expected 00.
- [ ] Interrupt acknowledge adds 0 cycles (`handle_interrupt` :290). Expected 11 for RST.

### Devices
- [ ] `..` and `.` pass validation, mount fails with 0x01 instead of 0x02 (`storage_mount.rs:55`). Repro: name `..`, mount: status 01.
- [ ] 8.3 not enforced (`storage_mount.rs:49-58`). Repro: `A.B.C` mounts. Docs now say "convention"; listed in case you want it enforced.
- [ ] Flush and write errors are swallowed (`storage.rs:88`, flush has no fsync). W prints "Written" regardless.
- [ ] `map_port` silently ineffective for 0x30-0x32, 0xFE and 0xFF: the CPU intercepts them first (`cpu.rs:758-782`). 0x30-0x32 goes away with the timer deletion.
- [ ] Ctrl-C can't stop a program that doesn't poll the console (`console.rs:43-53, 76-80`). Repro: `G` into `JMP $`, then ^C: the process keeps running.
- [ ] Control keys mangled (`console.rs:81-84`). Repro: Ctrl-A arrives as 0x61, Ctrl-S as 0x73; Esc and Tab are dropped.

### Monitor ROM (`rom/monitor.asm`)
- [ ] L/W count 0 moves 65536 bytes (:1351-1358, :1420-1427). Repro: `L 0 0200 0` wipes the workspace; the next `I 02` cold-boots.
- [ ] L/W junk count silently means 256 (:1339, :1408). Repro: `L 0 0200 ZZ` loads 256 bytes.
- [ ] D wrap heuristic stops early (:750-756). Repro: `D 0 F000` prints one line.
- [ ] S same heuristic (:1123-1129). Repro: `F 0500 0502 77`, then `S 0000 FFFF 77 77 77` finds nothing.
- [ ] F with end < start fills to FFFF (:847-861). Repro: `F 0300 0200 AA` fills 0300-EFFF, stack included.
- [ ] C with end < start does 64K compares (:561-568). Repro: `C 0201 0200 0300` prints thousands of lines.
- [ ] Too many hex digits are silently truncated (READ_HEX_WORD :360-415, READ_HEX_ADDR24 :460-530). Repro: `F 10200 1020F 1AA` fills 0200-020F with AA.
- [ ] G with a bad argument runs 0100 (:879). Repro: `G ZZ`.
- [ ] E: LF counts as "advance", and CR after a 2-digit value skips a byte (:1171, :1195-1198). Repro: `E 0200`, then `12` CR `34` CR `.` writes 0200=12, 0202=34.
- [ ] READ_LINE stores DEL (0x7F) as a character (:299-302). Real terminals send DEL for backspace. Repro: `D 020<DEL>0 0200` prints "Invalid address".
- [ ] M with an overlapping dest > src corrupts (:953). Repro: `M 0200 0201 0F` smears 0200. Documented as "forward copy" only.
- [ ] `make size` always reports 4096 because p2bin pads (`rom/Makefile:37-39`).

### Test gaps
- [ ] Vacuous: `test_search_finds_pattern` (`tests/monitor_tests.rs:85`) matches the echoed `F 0500` line; `test_io_read_status` (:107) matches "02" in the banner date; `test_compare_identical` (:95) only counts prompts.
- [ ] No monitor test maps storage, so L/W/X in the ROM are untested. This is how the stale bin slipped through.
- [ ] Untested: interrupts (RST entry, EI delay, HLT wake, DI blocking); cycle counts beyond LXI/MOV; the overlay at CPU level; the E command; error paths; range edges (end < start, FFFF wrap).

## Blocked
- [ ] R command - needs register capture on return (Phase 10)

## Someday
- [ ] Phase 6: Time (mailbox `TIME` + T)
- [ ] Phase 7: Assembler/disassembler (mailbox `ASM`/`DIS`)
- [ ] Phase 8: HTTP GET (mailbox `GET`)
- [ ] Phase 9: Claude (mailbox `ASK`, Q command)
- [ ] Phase 10: Debugger (breakpoints, single-step, R)
- [ ] 8253 timer, TIMER_ISR, RST 7 vector: when something needs a periodic interrupt
- [ ] Hardware prototype (Pi Zero + real 8080, READY wait-state on the Pi window 0x08-0x6F)
