# TODO

## Open Decisions (Mike decides before anyone codes against them)
- [ ] HEX loader command letter. Roadmap says `H`; `H` is Hex Math. Pick a letter, or make the monitor treat a line starting with `:` as a HEX record.
- [ ] `:` ownership. Phase 10 plans `:` as the emulator-command prefix; it is also the Intel HEX record start char. Phase 10 is unbuilt, so it is the cheap one to move.
- [ ] `A` ownership. Phase 7 plans `A` = assemble line; Phase 9 plans `A` = ask Claude. One of them moves.
- [ ] Timer. `src/io/devices/timer.rs` exists and is hardwired into `cpu.rs` at ports 0x30-0x32. Docs plan an 8253 at 0x70-0x73 (Phase 6) and reserve 0x30-0x37 for the debugger. Keep, move, or delete the existing timer?
- [ ] Port 0xFE value 0x01 (Halt CPU). In DEVICE_SPECS and QUICK_REFERENCE; `cpu.rs` ignores it. Implement or drop from the spec.
- [ ] RST vectors / API table. ARCHITECTURE and QUICK_REFERENCE say boot copies vectors to 0x0000-0x003F and an API table to 0x0040-0x007F. `monitor.asm` does neither. Still the plan? Phase 6 interrupts need RST 7.

## Current
- [x] Apply alignment package (archived: docs/archive/HANDOFF_2026-10.md)
- [x] Untrack build artifacts: `git rm --cached rom/monitor.lst rom/monitor.p`
- [ ] Spec/state review (ultracode) to resolve Open Decisions above

## Next (Phase 5 - Intel HEX Loader)
- [ ] Record parser in ROM: `:LLAAAATT<data>CC`
- [ ] Type 00 (data) records
- [ ] Type 01 (EOF) record
- [ ] Checksum validation (sum of all bytes incl. checksum == 0)
- [ ] Error reporting for bad records
- [ ] Monitor integration test(s) via TestConsole

## Blocked
- [ ] R command - needs return mechanism from G (Phase 10)

## Someday
- [ ] Phase 6: Timer device, interrupts
- [ ] Phase 7: Assembler/disassembler devices
- [ ] Phase 8: HTTP client, network time
- [ ] Phase 9: Claude API integration
- [ ] Phase 10: Debugger (breakpoints, single-step, R)
- [ ] Hardware prototype (Pi Zero + real 8080)
