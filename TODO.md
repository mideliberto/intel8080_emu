# TODO

## Current
- [x] Move project to ~/dev/intel8080_emu
- [x] Clean up tmp/ contents
- [x] Set up docs structure
- [ ] Run cargo test - verify 191 passing
- [ ] Commit clean state

## Next (Phase 5 - Intel HEX Loader)
- [ ] H command - parse Intel HEX records
- [ ] Type 00 (data) record handling
- [ ] Type 01 (EOF) record handling  
- [ ] Checksum validation
- [ ] Error messages for bad records

## Blocked
- [ ] R command - needs return mechanism from G (deferred to Phase 10)

## Someday
- [ ] Phase 6: Timer device, interrupts
- [ ] Phase 7: Assembler/disassembler devices
- [ ] Phase 8: HTTP client, network time
- [ ] Phase 9: Claude API integration
- [ ] Phase 10: Debugger (breakpoints, single-step)
- [ ] Hardware prototype (Pi Zero + real 8080)
