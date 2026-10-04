# System Architecture

Normative. Covers the memory map, ROM organization, reset and boot, the ROM overlay, the CPU behavioral contract, the hardware circuits behind the I/O ports, and the host-side rules for the emulator (including the host key map).

The other three normative specs:
- `DEVICE_SPECS.md`: every port protocol, register values, power-on state of each device, and the READY contract as software sees it.
- `MONITOR_SPEC.md`: line input, argument grammar, every monitor command and message, the Intel HEX loader, the `G` return contract, and ROM routine contracts.
- `PI_DAEMON.md`: the Pi software behind the window (`pi8080d`): how it meets 6.4 and 6.6, the console transport, build, deployment and its tests.

One fact, one home: this file links to those and does not restate them. "MUST" applies to both the emulator and the hardware unless a section says otherwise. Where the current code differs, the code is wrong; the fixes are tracked in `TODO.md` ("Decided, to implement" and "Review findings"). All decisions here were made by Mike (COLLABORATION_LOG Key Decisions).

The rule behind every section: **the ROM sees only what real parts provide.** If the 8080 can observe a behavior, a period chip or the Pi coprocessor must be able to produce it. Emulator conveniences stay on the host side (section 7).

---

## 1. Memory Map

```
0x0000-0x007F   Unused (128 bytes), except 0030-0032: RST 6 break vector (written by G)
0x0080-0x00FF   Monitor workspace (128 bytes)
0x0100-0xEEFF   User area (60,928 bytes)
0xEF00-0xEFFF   Monitor stack page (256 bytes, SP starts at 0xF000)
0xF000-0xFFFF   Monitor ROM (4,096 bytes)
```

| Range | Rule |
|-------|------|
| 0000-007F | Not initialized at boot. `G` writes `JMP BRK_ENTRY` at 0030-0032 before it starts a program, for breakpoints the user plants (`MONITOR_SPEC.md` 8.1); the rest is undefined and unused by the monitor. There is no other RST vector (RST 7 is reserved for an interrupt source, 6.7) and no API jump table. (What this means for programs started with `G`: `MONITOR_SPEC.md`, G Return Contract.) |
| 0080-00FF | Monitor workspace. Layout in 1.1. Initialized at cold boot only. A program that writes here can break monitor commands until the next reset. |
| 0100-EEFF | User programs and data. The monitor reads and writes this range only when a command tells it to. |
| EF00-EFFF | Monitor stack. Cold boot and WARM (3.2) both set SP to 0xF000, so the first push writes 0xEFFF and 0xEFFE. |
| F000-FFFF | ROM. Reads return ROM bytes. With JP-WE open (the default, 6.10), writes have no effect the 8080 can observe. |

**Monitor-owned ranges.** 0000-00FF and EF00-FFFF belong to the monitor. The HEX loader rejects any record that would write into either range (EOF and zero-length records are accepted at any address) (`MONITOR_SPEC.md`, Intel HEX Loader). The other monitor commands do not guard these ranges.

### 1.1 Workspace Layout

This table and the workspace labels in `rom/monitor.asm` (`ORG 0080H` and one `DS` per row, free rows unlabeled) MUST match. `tests/debugger_tests.rs` checks the table against `rom/monitor.sym`. If they disagree, that is a defect and goes in `TODO.md`.

| Address | Size | Name | Initialized at boot |
|---------|------|------|---------------------|
| 0080-00CF | 80 | LINE_BUFFER (at most 79 characters plus a NUL) | no |
| 00D0-00D1 | 2 | free | - |
| 00D2-00D3 | 2 | LAST_DUMP_ADDR | 0000 |
| 00D4-00D5 | 2 | LAST_EXAM_ADDR | 0000 |
| 00D6-00D8 | 3 | IO_IN_STUB (`IN pp` / `RET`) | DB 00 C9 |
| 00D9-00DB | 3 | IO_OUT_STUB (`OUT pp` / `RET`) | D3 00 C9 |
| 00DC-00E3 | 8 | SEARCH_PATTERN | no |
| 00E4 | 1 | SEARCH_LENGTH | no |
| 00E5-00E6 | 2 | SEARCH_END | no |
| 00E7-00E9 | 3 | STOR_ADDR (24-bit: lo, mid, hi) | no |
| 00EA-00F1 | 8 | REGS (A, F, BC, DE, HL at the last `G` return or RST 6 break: L H E D C B F A, `MONITOR_SPEC.md` 6.20, 8.1) | no |
| 00F2-00FF | 14 | free | - |

16 bytes are free in total. New workspace goes into 00F2-00FF first.

The I/O stubs run from RAM as self-modifying code. Real hardware runs it the same way, so it is allowed.

---

## 2. ROM Organization

- **Source:** `rom/monitor.asm`. **Image:** `rom/monitor.bin`, exactly 4096 bytes, assembled at `ORG 0F000H`, unused bytes 0xFF.
- **Fixed address:** only one. COLD_START is the first byte of the image (0xF000, which is also 0x0000 through the overlay). Every other routine address can move from build to build.
- **No WARM vector.** WARM (3.2) has no fixed address. A program reaches it only through the `G` return contract (`MONITOR_SPEC.md`), or by stopping at a planted `RST 6` breakpoint (`MONITOR_SPEC.md` 8.1, a debugging aid, not an exit); anything else that wants the monitor back jumps to F000, a cold start (banner, workspace reset). The CP/M exerciser shim (`tests/exerciser.rs`) does that at 0000. (Decided 2026-10-03.)
- **No public entry points.** User programs MUST NOT call ROM routines by address. Programs do their own I/O through the ports in `DEVICE_SPECS.md`. ROM routine contracts are in `MONITOR_SPEC.md` (ROM Routine Contracts); they describe the code, not an ABI.
- **Budget:** 4096 bytes. Used bytes = `ROM_END - 0F000H`, where `ROM_END` is a label after the last assembled byte. `make size` prints that number. The padded image size is not a measurement.
- **RAM test build:** the same source assembled at D000 as `rom/monitor_ram.hex`, so a ROM change can run on the board without burning an EEPROM (2.1).
- **Layout** (not normative): boot first (COLD_START), then WARM and MAIN_LOOP, the shared error exits, console I/O and print routines, input and parse routines, the commands, the storage commands, then the strings and ROM_END.

### 2.1 RAM Test Build

Decided 2026-10-03 (Mike, COLLABORATION_LOG Key Decisions). A ROM change can run on the board before an EEPROM is burned: the resident monitor's HEX loader (`MONITOR_SPEC.md` 7) loads the same source assembled for RAM, and `G` starts it. It is a development image, not a second ROM: the shipped ROM's memory map and command set do not change. Assembled without `RAMBUILD`, `monitor.bin` is byte-identical to the build before this section existed, apart from `DATE` and `TIME`, and `monitor.sym` is identical.

| Item | RAM test build |
|---|---|
| Source | `rom/monitor.asm` with `RAMBUILD` defined (`asl -D RAMBUILD`). Four things differ, each under `IFDEF RAMBUILD`: the base (`CODE_BASE`), the top of the HEX guard (`USER_END`), the F/M/L image guard (`IMG_GUARD`) and the banner marker. The shipped source changes only by naming the base and the guard top. |
| Image | `rom/monitor_ram.hex`, committed with `monitor.bin` and `monitor.sym`, so `cargo test` needs no assembler. Intel HEX type 00 records of 16 data bytes (43 characters; the last may be shorter), contiguous from D000, then one type 01 record, which the loader accepts at any address (`MONITOR_SPEC.md` 7.3). Only assembled bytes, no padding. Every record is inside the loader's limits (`MONITOR_SPEC.md` 7.2). |
| Base and entry | D000. The entry is COLD_START, the first byte: `G D000`. |
| Size | The ROM's used bytes plus the RAM-only code (47 bytes). The ROM is at most 4096 bytes, so the image always ends far below the stack page. |
| Memory | While the RAM monitor runs, D000-EEFF (the image range) is monitor-owned too, and the user area is 0100-CFFF. The workspace (0080-00FF) and the stack page (EF00-EFFF) are shared with the resident monitor and unchanged: one monitor runs at a time, and each cold start initializes them (1.1, 3.2). `LXI SP` discards the G_RETURN address that `G` pushed. |
| HEX guard | Records must lie inside 0100-CFFF: `MONITOR_SPEC.md` 7.2 step 6 with D000 in place of EF00. The same code, one constant. |
| F, M, L | A destination that touches D000-EEFF, wrapping past FFFF for M and L, prints `Address out of range` and writes nothing. L checks before it writes a port. RAM build only: the ROM's F, M and L do not guard (`MONITOR_SPEC.md` 6.4, 6.8, 6.9). E, A and programs still write anywhere; a write into the image has undefined results, as in the workspace and the stack page (1). |
| Overlay write at boot | Kept: the same boot code, `OUT 0FEH` included. The resident cold start has already cleared the overlay, so it changes nothing (4). 3.2 requirement 1 (the `OUT 0FEH` and what follows run from F000-FFFF) governs the reset path only; the RAM build is entered by `G`, with the overlay already clear. |
| Banner | `8080 Monitor v<version> RAM` (`MONITOR_SPEC.md` 1.1). |
| F000-FFFF | The resident ROM, untouched. `G F000` or RESET returns to it. |
| Debugger symbols | None: `monitor.sym` names the ROM build. |

**Build.** `cd rom && make` builds all three files (rules in `rom/Makefile`).

**Load and run,** on the board or on `pi8080d --sim` (`PI_DAEMON.md` 16), with the resident monitor at its prompt and the `PI_DAEMON.md` 11 tunnel up:

```
{ cat rom/monitor_ram.hex; echo 'G D000'; } | nc localhost 8080
```

Each record echoes and prompts, the EOF record prints `Loaded`, and the first banner line ends ` RAM`. Any other message means a record was rejected: send the file again. Then quit `nc` and connect the interactive client (a new client replaces the old, `PI_DAEMON.md` 7.1). A load is about 3.9M cycles (about 1.9 s at 2.048 MHz, before READY wait states).
- To load a new image, return to the resident monitor first (`G F000` or RESET): the RAM monitor's guard rejects its own range, so every record prints `Address out of range`.
- RAM keeps its contents across RESET (3.1), so `G D000` restarts the RAM monitor if nothing wrote D000-EEFF.

**What it does not test:** the reset path into the ROM (the fetch from the 0000 mirror, `OUT 0FEH` clearing the overlay) and anything that depends on the image sitting at F000. Those need the burned EEPROM (`HARDWARE_BUILD.md` 3).

**Test:** `ram_build_runs_the_transcripts` (`tests/monitor_tests.rs`) checks the file's shape and that its `Built:` date equals `monitor.bin`'s: a file left from an earlier day fails, one from the same day is not caught. Then, for every `tests/transcripts/*.txt` except `hex` and `search`, which write D000-EEFF by design, and for `tests/transcripts/ram/guard.txt` (the moved HEX guard and the F/M/L guard, at both edges of the image; its subdirectory keeps it out of every other harness): power on with the shipped ROM, paste the file and expect every record echoed and `Loaded`, `G D000` and check for the ` RAM` banner, play the transcript, check that the image is unchanged, `G F000` and check for a banner without ` RAM`.

---

## 3. Reset and Boot

### 3.1 Reset State

**RESET = whole-machine power-on state, RAM excepted.** A hardware reset (power-on or the reset button) produces:

| Item | State after reset |
|------|-------------------|
| PC | 0x0000 |
| INTE (interrupt enable) | 0 |
| Halt state | cleared |
| Overlay flip-flop | set (ROM visible at 0x0000-0x0FFF for reads) |
| WAIT flip-flop (6.4) | clear (READY high, no Pi request pending) |
| A, B, C, D, E, H, L, flags, SP | undefined. The ROM MUST NOT rely on them. |
| RAM | undefined at power-on, preserved across reset. The ROM MUST NOT rely on either. |
| Pi devices | power-on state (`DEVICE_SPECS.md` rule 2.8). Circuit and GPIO detail: 6.6. |

There is no software reset.

**Emulator:**
- `reset()` models the RESET pin and nothing else. It sets PC=0, INTE=0, halted=false, overlay=1, clears any pending interrupt, and leaves A-L, flags, SP and RAM alone.
- `Intel8080::new()` builds the struct and calls `reset()`, so there is one home for power-on state.
- Test harnesses start registers, SP and RAM at values the ROM can't get lucky with. They fill RAM with a non-zero junk byte before boot, so a ROM that relies on zeroed RAM or a preset SP fails its tests. (Decided 2026-10-02.)
- `new()` starts A-L, SP and RAM at 00 and flags at 02; the monitor harness overwrites them with junk before boot. Devices are created in their power-on state at process start by `build_bus` (`src/io/mod.rs`), which is the emulator's only RESET. Any future host-side reset (Someday, TODO.md) MUST reset the devices as well as the CPU.

### 3.2 Boot Sequence

```
0x0000 (= F000 via overlay)
  COLD_START:    LXI  SP,0F000H
                 DI
                 JMP  BOOT_CONTINUE      ; absolute F0xx: leave the overlay mirror
  BOOT_CONTINUE: XRA  A
                 OUT  0FEH               ; clears the overlay flip-flop
                 ; init LAST_DUMP_ADDR, LAST_EXAM_ADDR, IO_IN_STUB, IO_OUT_STUB (table 1.1)
                 LXI  H,MSG_BANNER       ; banner text: MONITOR_SPEC.md
  PRINT_WARM:    CALL PRINT_STRING       ; also the tail of every message that ends a command
  WARM:          LXI  SP,0F000H
  MAIN_LOOP:     ...
```

Requirements:

1. Code fetched from the mirror (0x0000-0x0FFF) MUST jump to an absolute address at 0xF000 or above before `OUT 0FEH` executes. The `OUT 0FEH` and everything after it run from F000-FFFF.
2. Boot initializes no device. From reset to the first prompt the ROM executes `OUT 0FEH` and console output (`OUT 00H`) and no other I/O instruction. The first Pi-window access after reset is the banner's first `OUT 00H`.
3. Boot does not drain console input. RESET flushes the Pi's console FIFO (`DEVICE_SPECS.md` rule 2.8), so no stale bytes are waiting.
4. The ROM never executes `EI` or `HLT`.
5. **WARM** sits directly before MAIN_LOOP. It sets SP to 0xF000 and does nothing else. `G` pushes G_RETURN's address (`MONITOR_SPEC.md` 8), which ends at WARM. The program-facing return contract is in `MONITOR_SPEC.md` (G Return Contract).
6. The ROM contains no timing-dependent code (no calibrated delay loops). The emulator runs unthrottled, and the hardware clock (6.1) is a design target, not a ROM dependency.

---

## 4. ROM Overlay

| Condition | Read 0000-0FFF | Write 0000-0FFF | Read F000-FFFF | Write F000-FFFF |
|-----------|----------------|-----------------|----------------|-----------------|
| Overlay set (after reset) | ROM byte at the same offset | RAM (write-through) | ROM | no visible effect (JP-WE open, 6.10) |
| Overlay clear | RAM | RAM | ROM | no visible effect (JP-WE open, 6.10) |

- The ROM is selected on MEMR only (decode in 6.2). Writes always go to RAM, or have no visible effect at F000-FFFF. The exception is JP-WE fitted (6.10, never in normal use): a write to F000-FFFF then also programs the EEPROM. The overlay never routes a write to the EEPROM.
- Test: with the overlay set, write 0x55 to 0x0100. A read of 0x0100 returns ROM byte 0xF100. After `OUT 0FEH`, a read of 0x0100 returns 0x55.
- Only RESET sets the flip-flop. Any `OUT 0FEH`, whatever the value in A, clears it. There is no soft reset. The circuit is in 6.5; port semantics are in `DEVICE_SPECS.md` (System Control).
- **Emulator:** a CPU with no ROM loaded treats all 64 KB as RAM, and the overlay has no effect.

---

## 5. CPU Behavioral Contract

The emulator MUST behave like an Intel 8080A, not like a Z80 or an 8085. Every rule here can be tested against the CPU model alone.

### 5.1 Flags (PSW Low Byte)

```
bit  7  6  5  4   3  2  1  0
     S  Z  0  AC  0  P  1  CY
```

- Bits 5 and 3 are 0 and bit 1 is 1 after **every** instruction. That includes `POP PSW`, which loads `flags = (popped & 0xD5) | 0x02`.
- `PUSH PSW` writes A to (SP-1) and the flags to (SP-2). Test: with FF FF on the stack, `POP PSW` followed by `PUSH PSW` pushes flags 0xD7.
- S is bit 7 of the result. Z is 1 when the 8-bit result is 0. P is 1 when the result has an even number of 1 bits.

### 5.2 Flag Rules per Instruction

A and v are the operand values before the operation, and CY is the carry flag before the operation. "SZP" means the flag is set from the 8-bit result.

| Instructions | Result | CY | AC | SZP |
|--------------|--------|----|----|-----|
| ADD r/M, ADI | A+v | carry out of bit 7 | `(A&0F)+(v&0F) > 0F` | yes |
| ADC r/M, ACI | A+v+CY | carry out of bit 7 | `(A&0F)+(v&0F)+CY > 0F` | yes |
| SUB r/M, SUI | A-v, computed as A+~v+1 | 1 if A < v (borrow) | `(A&0F)+(~v&0F)+1 > 0F`, i.e. 1 when there is **no** borrow from bit 4 | yes |
| SBB r/M, SBI | A-v-CY, computed as A+~v+(1-CY) | 1 if A < v+CY | `(A&0F)+(~v&0F)+(1-CY) > 0F` | yes |
| CMP r/M, CPI | like SUB, but A is unchanged | as SUB | as SUB | from A-v |
| ANA r/M, ANI | A&v | 0 | bit 3 of `(A \| v)` | yes |
| XRA, XRI, ORA, ORI | A^v, A\|v | 0 | 0 | yes |
| INR r/M | x+1 | unchanged | `(x&0F) == 0F` | yes |
| DCR r/M | x-1 | unchanged | `(x&0F) != 0` | yes |
| DAA | see 5.3 | see 5.3 | see 5.3 | yes |
| DAD rp | HL+rp | carry out of bit 15 | unchanged | unchanged |
| RLC, RRC, RAL, RAR | rotate | bit shifted out | unchanged | unchanged |
| STC / CMC | - | 1 / complemented | unchanged | unchanged |
| POP PSW | - | from stack | from stack | from stack (masked as in 5.1) |
| everything else (including CMA, INX, DCX, MOV, MVI, LXI, loads, stores, branches, I/O, EI, DI) | - | unchanged | unchanged | unchanged |

Reference vectors (tested in `tests/cpu_tests.rs`):

| Sequence | Expected |
|----------|----------|
| `MVI A,10h; MVI B,01h; SUB B` | A=0F, AC=0, CY=0 |
| `MVI B,10h; DCR B` | B=0F, AC=0 |
| `MVI A,08h; MVI B,00h; ANA B` | A=00, AC=1, CY=0, Z=1 |
| `MVI A,0Ah; DAA` | A=10, AC=1, CY=0 |

### 5.3 DAA

1. Let lo = A & 0x0F and hi = A >> 4, both taken before the adjustment.
2. correction = 0. If lo > 9 or AC = 1, correction |= 0x06.
3. If hi > 9, or CY = 1, or (hi >= 9 and lo > 9), then correction |= 0x60 and the new CY = 1. Otherwise the new CY = 0.
4. AC = `(lo + (correction & 0x0F)) > 0x0F`. A = A + correction (8-bit). S, Z and P come from the new A.

### 5.4 Opcode Coverage

All 256 opcodes execute, and none panics. The 12 undocumented opcodes alias documented ones:

| Opcodes | Behaves as | Bytes | Cycles |
|---------|------------|-------|--------|
| 08 10 18 20 28 30 38 | NOP | 1 | 4 |
| CB | JMP a16 | 3 | 10 |
| D9 | RET | 1 | 10 |
| DD ED FD | CALL a16 | 3 | 17 |

### 5.5 Address Arithmetic

All 16-bit address arithmetic wraps modulo 0x10000: PC fetch past FFFF, SP in PUSH, POP, CALL, RET and interrupt acknowledge, the second byte of word accesses (LHLD, SHLD, XTHL, POP and the stack), and INX/DCX. Test: with 0xFFFF = 12 and 0x0000 = 34, `LHLD 0FFFFH` loads HL = 3412.

### 5.6 Cycle Counts

- Each instruction adds its T-state count from `docs/reference/Complete_Intel_8080_Instruction_Set_Reference.txt` to `cycles`, assuming zero wait states. Table 5.4 governs the CB, D9, DD, ED and FD aliases.
- Conditional jumps cost 10 whether or not they are taken. Conditional calls cost 17 when taken and 11 when not. Conditional returns cost 11 when taken and 5 when not.
- An interrupt acknowledge costs 11 (5.7). A step while halted costs 4 (5.8).
- The emulator does not model wait states. Nothing in the ROM depends on cycle counts.

### 5.7 Interrupts

The emulator CPU has one interrupt input, `interrupt(rst)`, callable from the host. In v1 only tests call it; no device raises an interrupt. On v1 hardware there is no interrupt source (6.7).

- **INTE:** reset clears it, `DI` clears it, `EI` sets it, and accepting an interrupt clears it.
- **`interrupt(n)`**, n = 0-7, latches a pending `RST n`. The latch models a source that holds INT until INTA. A later call before acceptance replaces the pending n.
- **Acceptance** happens at an instruction boundary when an interrupt is pending and INTE = 1, except that `EI` enables acceptance only after the instruction that follows it completes. With INTE = 0 (after reset, `DI`, or during an ISR before `EI`) the interrupt stays pending and is not accepted.
- **Acknowledge:** one step executes `RST n` in place of the next fetch. It clears the pending latch and INTE, ends the halt state, pushes the address of the next unexecuted instruction (the address after the HLT when halted), sets PC = 8 × n, and adds 11 cycles. The instruction at the vector runs on the following step.
- **Tests:**
  - EI delay: pending `RST 7`, INTE = 0, PC at `EI; MVI A,01h; NOP`. Step 1 runs EI and step 2 runs MVI (A = 01). Step 3 is the acknowledge: PC = 0038, the pushed address is the NOP's, cycles += 11. Same shape with `EI; RET`: RET completes before the acknowledge.
  - HLT wake: `HLT` at 0100 with INTE = 1, then `interrupt(7)`. The next step leaves PC = 0038, halted = false, pushed address 0101, cycles += 11.
  - Blocked: `DI`, `interrupt(7)`, 10 steps of NOPs: no acknowledge. `HLT` with INTE = 0 stays halted with an interrupt pending.

### 5.8 HLT

- `HLT` costs 7 cycles, leaves PC at the next instruction, and sets the halt state.
- While halted, a step fetches nothing, changes no register or memory, and adds 4 cycles, unless an accepted interrupt (5.7) ends the halt.
- Test: `76 3E 01 76` followed by two steps leaves A unchanged and PC = 0001.
- `run()` returns when the CPU halts. The CPU core prints nothing. Host handling of the halt is in section 7.

---

## 6. Hardware Interface

The circuits the hardware build must contain. The software-visible behavior of every port is in `DEVICE_SPECS.md`. This section is the contract the board must meet. It stays normative for circuits and rules; pin numbers live only in the board netlist `hw/board.net.txt`, which `tests/netlist_tests.rs` checks against this section pad by pad (`HARDWARE_BUILD.md` 2.2). Timing in this section was checked in the 2026-10 hardware-alignment pass against the MCS-80 User's Manual 98-153D (Oct 1977: 8080A p.6-3..6-5, 8224 p.6-21..6-25, 8228 p.6-32..6-36) and the TI/Nexperia 74HCT and 74LVC datasheets. Figures are datasheet worst case at tCY = 488.28 ns, with t = 0 at phi1 rising in T1, unless marked (est). Items marked **[bench]** can only be closed by measurement on the built board. Pins, levels and cycle timing of the 8080A, 8224 and 8228: `reference/8080_HARDWARE.md` (98-153B, Sep 1975 edition; its page numbers differ from 98-153D), cited as "reference N". The ROM write path (6.10) is checked against the AT28C64B datasheet, Atmel 0270L-PEEPR-2/09 (https://ww1.microchip.com/downloads/en/DeviceDoc/doc0270.pdf), cited as "AT28C64B DS" with its section numbers. Gate drive and delays in 6.10-6.11 are from TI SCLS063G (SN74HCT08) and SCLS171F (SN74HCT138), worst case at VCC 4.5 V, -40 to 85 °C.

### 6.1 Clock and CPU Support

- CPU: 8080A at 2.048 MHz (18.432 MHz / 9 from the 8224). tCY = 488.28 ns, 1.7% above the 8080A minimum of 480 ns. Nominally "2 MHz". All timing analysis uses 488.28 ns, which is exactly where Intel characterizes the 8224 (p.6-25). At that tCY every phi1/phi2 width and delay meets the 8080A minimums with at least 16 ns of margin.
- Clock and reset: 8224 with an 18.432 MHz series-resonant fundamental-mode crystal (Intel 8801 equivalent). Fit 510 ohm from XTAL1 to GND and from XTAL2 to GND (8224 datasheet note 1, required at 18 MHz). TANK and OSC are unconnected. There is no trim capacitor (3.2 req. 6). phi1/phi2 are not short-circuit protected.
- The clock runs continuously. The 8080A is dynamic, so tCY must not exceed 2.0 us. Waits (READY) and RESET may last indefinitely only because the clock keeps running through them. Any hardware single-step uses READY, never the clock.
- System controller: 8228 (MEMR, MEMW, I/OR, I/OW strobes; INTA handling in 6.7).
- The ROM stays timing-independent (3.2, requirement 6).

### 6.2 Memory Decode

```
OVL     = overlay flip-flop Q (6.5)
ROM_SEL = A15..A12 = 1111  OR  ( OVL AND A15..A12 = 0000 )   ; address only
ROM_OE  = MEMR AND ROM_SEL
RAM_OE  = MEMR AND NOT ROM_SEL
RAM_WE  = MEMW
ROM_WE  = MEMW AND A15..A12 = 1111 AND JP_WE      ; JP_WE = jumper JP-WE fitted (6.10). No OVL term.
```

- Implementation: one ATF22V10C GAL, which also carries the 6.3-6.5 decode and drives DB0 for port 0xFF (6.5). Pinout and equations: `hw/glue.pld`; the burned fuse map `hw/glue.jed` is checked against the emulator's decode by `tests/gal_tests.rs`. All 22 signal pins are used.
- OVL changes only during an I/O write (OUT FE) or RESET, never while MEMR is active.
- RAM covers all of 0000-FFFF. RAM_WE has no address term, so a write to F000-FFFF lands in the RAM under the ROM and is never read back: reads of F000-FFFF always select ROM (section 4). With JP-WE open (the default) the write goes nowhere else. With JP-WE fitted it also programs the EEPROM (6.10).
- RAM is static. It MUST keep its contents with no CPU activity for unlimited time (READY waits, RESET held), so DRAM that needs CPU-driven refresh is excluded.
- **Pins.** Tie ROM /CE low. ROM /WE has a pull-up and reaches the write-enable gate only through JP-WE (6.10), so with JP-WE open the 8080 can never write the ROM. ROM_OE drives only ROM /OE. An 8 KB ROM part has A12 tied low. RAM /CE comes from address bits only. RAM_OE drives RAM /OE and RAM_WE drives RAM /WE. No signal gated by MEMR may drive a /CE.
- **Read timing** (no memory wait states). Address is valid by 329 ns. MEMR arrives by 787 ns (DBIN 757 + 8228 tRR 30). Data must be on the system bus by 905 ns: tDS2 is 150 ns before phi2 of T3, less 8228 tRD 30. That gives 118 ns from MEMR to data and 576 ns from address to data. Memory /OE access plus the MEMR gate MUST fit in 118 ns. Timing a /CE access from MEMR misses the deadline (AT28C64B tCE 150).
- **Logic levels.** The 8228 drives the system data bus and MEMR/MEMW/I/OR/I/OW at TTL levels (VOH 2.4 V min at -1 mA, VOL 0.45 V). Every input on those nets MUST accept VIH <= 2.4 V: 74HCT/ACT, ATF22V10C, AT28C64B, 74LVC at 3.3 V, AS6C62256. Parts with CMOS thresholds MUST NOT be on those nets: 74HC, and AS6C1008/AS6C4008 (VIH 0.7 VCC). 8080A inputs need VIH 3.3 V. They are driven only by the 8224 (READY, RESET), the 8228 CPU-side D0-D7, and HCT outputs.
- **Bus loading.** Every load on the 8080A address pins and CPU-side data pins is CMOS, because 8080A IOL is 1.9 mA. There are no address or data buffers beyond the 8228. The status taps (D4, D6) are on the CPU side of the 8228. The address pins also carry the 10 kohm pull-ups of 6.13; the CPU-side data pins carry none.

### 6.3 Port Address Decode

| Ports | Decode | Served by |
|-------|--------|-----------|
| 0x00-0x6F | `A7 = 0 AND NOT (A6 AND A5 AND A4)` | Pi, behind READY (6.4). Every port in this range, assigned or not. |
| 0x70-0xFD | rest | local chips (none fitted). Nothing drives the bus on `IN`. |
| 0xFE (write), 0xFF (read) | full 8-bit compare | overlay glue (6.5) |

Port assignments and the values returned by unassigned and unmapped ports are in `DEVICE_SPECS.md` (Port Map, Rules Common to All Ports).

During IN and OUT the 8080A drives the port number on both A0-A7 and A8-A15 (MCS-80 p.5-7). Glue MAY decode either copy. The Pi reads A0-A6. A7 is always 0 inside the window.

**Emulator:** the CPU model handles `OUT 0xFE` and `IN 0xFF` itself and never passes them to the IoBus. Every other access goes through the IoBus. `IoBus::map_port` MUST panic when given 0xFE or 0xFF. Test: mapping a device to 0xFE panics.

### 6.4 Pi Window and READY

Every Pi-window access holds READY low until the Pi releases it. The software contract that results (one instruction is one access, no byte-level busy polling, no timeout) is in `DEVICE_SPECS.md` (READY Contract).

**WAIT flip-flop.** One 74HCT74 half; the other half is the overlay flip-flop (6.5). Its /Q drives the 8224 RDYIN, so READY is low while Q = 1. Q ANDed with the 8080A WAIT output is the Pi's REQ.

1. **Set** asynchronously (PRE) while all of these hold: 8224 STSTB is low, RESET is inactive, the CPU-side status shows INP (D6) or OUT (D4), and the port decodes into 0x00-0x6F. Status and address are valid from 76 ns before STSTB falls (tDSS min 296 - tDD max 220) until phi2 of T2. SYNC alone MUST NOT qualify the set. SYNC has only maximum delays (tDC <= 120 ns, no minimum) against tDD <= 220 and tDA <= 200, and it falls up to 120 ns after phi2 of T2 while the bus changes to write data. A SYNC-gated set can therefore fire on a memory cycle. SYNC MAY be added as an extra term. The RESET term uses the same inverted RESET that drives the flip-flop's /CLR, for two reasons: the 8224 drives STSTB low during reset, and PRE and CLR low together give Q = /Q = 1. The IN-latch enable (IN cycle below) and the port 0xFF read (6.5) carry the same NOT RESET term. RDYIN MUST be low within 167 ns of STSTB falling (8224 tDRS = -167 ns) and stays low past STSTB + 217 ns (tDRH). STSTB is at least 40 ns wide, against a flip-flop PRE minimum of 20-24 ns. **[bench]** The STSTB width at PRE, after the gate path. The flip-flop MUST NOT be set from I/OR or I/OW. I/OW starts only in T_W.
2. **Cleared** by the rising edge of ACK (clocked, D tied low, not level-sensitive) or asynchronously by RESET. A held ACK level can never block the next set. ACK reaches CLK through two 74HCT14 Schmitt stages. REQ falls within about 100 ns of the ACK edge (est). After raising ACK, the Pi reads ACK back high, then waits at least 500 ns before it treats REQ as a new access, then lowers ACK. The Pi MUST NOT wait for REQ to go low: the next Pi-window REQ can follow about 5 us later, and a preempted Pi would miss the low and deadlock. A longer wait is always safe, because REQ stays high until ACK.
3. **Every Pi-window cycle inserts at least one T_W**, whatever level ACK is at. REQ is gated by the 8080A WAIT output, so the Pi cannot see or ACK a request before T_W. The 8080 leaves T_W only after an ACK edge or RESET. T3 starts 0.4-0.9 us after the ACK edge (8224 resynchronization). Every Pi-window IN or OUT costs 10 T-states plus N >= 1 T_W.
4. **No timeout.** A dead or absent Pi stalls the 8080 in T_W until RESET. An indefinite wait is allowed (MCS-80 p.2-5).

**When the Pi may sample.** REQ = WAIT flip-flop Q AND the 8080A WAIT output (pin 24). REQ rises only in T_W (>= 976 ns). By then A0-A6, DIR and OUT data have been valid at the Pi for at least about 90 ns. OUT data reaches the Pi by 883 ns: tDD 220, plus 8228 tWD 40, plus LVC245 6.3. Any GPIO read with REQ high therefore has valid port, direction and data. Q alone rises in T1, up to about 460 ns before OUT data is valid, while the 8228 system bus still carries the status byte (10h). Q MUST NOT reach the Pi ungated.

**Direction.** DIR is the 8228 /I/OR, passed to the Pi through a 74LVC245A; low means IN. On an IN cycle /I/OR is low from 787 ns at the latest through T_W and T3. On an OUT cycle it is high throughout. DIR is therefore valid whenever REQ is high. There is no status latch.

**OUT cycle.** The data 74LVC245A carries system DB0-DB7 to the Pi. Its /OE = NOT /I/OR (one inverter), so it is enabled except during I/OR. The Pi samples address and data from a read in which REQ is high, applies the write, then raises ACK.

**IN cycle.** The Pi:
1. computes the byte and drives it on its D0-D7 GPIOs into a 74HCT374 IN latch;
2. reads D0-D7 back until they match, then reads once more;
3. raises LATCH and reads it back high, then lowers LATCH;
4. returns D0-D7 to input;
5. raises ACK.

The read-backs guarantee the 74HCT374 tsu 25 ns, th 10 ns and tw 20 ns. Back-to-back GPIO writes do not: on a Pi 4 they can land about 4 ns apart. The latch outputs drive the system data bus (DB0-DB7, never the 8080-side D0-D7) while I/OR is active, RESET is inactive and the port decodes into the window. The NOT RESET term keeps the latch off DB if the 8228 asserts /I/OR during RESET while the floated address ramps through the window (6.13). The enable MUST NOT use Q or REQ, because the 8080 samples in T3, after ACK has cleared Q. No line is ever driven from both sides. The Pi drives D0-D7 only while REQ is high with DIR = IN, which lies inside I/OR active, when the data 74LVC245A is disabled. It releases them before ACK, while the 8080 is still frozen in T_W.

**Signals at the Pi (20 GPIO):**

| Signal | Count | Pi direction | Path | BCM GPIO |
|--------|-------|--------------|------|----------|
| A0-A6 | 7 | in | 74LVC245A (A7 is always 0 in the window) | 4-10 |
| D0-D7 | 8 | in on OUT cycles, out to the IN latch on IN cycles | in: 74LVC245A B outputs through a 330 ohm series array; out: the 74HCT374 inputs, which share the node of the Pi pins (the array sits between that node and the 245) | 20-27 |
| DIR (/I/OR) | 1 | in | 74LVC245A | 11 |
| REQ (Q AND WAIT) | 1 | in | 74LVC245A | 12 |
| RESET | 1 | in | 74LVC245A | 13 |
| ACK | 1 | out | 74HCT14 Schmitt input | 16 |
| LATCH | 1 | out | 74HCT374 CLK (accepts 3.3 V) | 17 |

The Pi's GPIO runs at 3.3 V and is not 5 V tolerant. Every 5 V signal reaches it through a 74LVC245A powered from the Pi's 3V3 pin. Do not use a 74LVCH245A: its bus-hold fights the Pi on D0-D7. Every signal sits in GPLEV0, so one read is an atomic snapshot. D0-D7 sit in GPFSEL2, so turning the data lines around takes one register write. The 330 ohm array limits the current if the Pi drives D0-D7 while the data 74LVC245A is enabled (a daemon fault, or a jig on the header), and the Pi's read-back still sees the latch inputs, which sit on its own node. On OUT cycles it adds a time constant of about 10 ns (est: 330 ohm into the Pi pin, the 74HCT374 input and the ribbon, about 30 pF), well inside the >= 90 ns by which OUT data leads REQ (above).

**Power and boot independence.** The 8080 board and the Pi have separate supplies, grounds joined at the header, and may be powered in either order, provided that:
- every 74LVC245A runs from the Pi's 3V3, so an unpowered Pi leaves them in Ioff and is never back-powered;
- the Pi drives only ACK, LATCH and D0-D7 toward the board, and D0-D7 only between REQ and LATCH. At idle ACK and LATCH are low and D0-D7 are inputs with internal pull-downs, so the Pi never back-powers an unpowered 5 V board through 74HCT input clamps;
- ACK and LATCH have 4.7 kohm pull-downs on the 5 V side and use GPIOs whose boot-default pull is down (BCM 9-27, never 0-8 or 14/15). A 10 kohm pull-down against a Pi 4's 33 kohm minimum pull-up leaves only 33 mV of margin to HCT VIL;
- on startup, the service sets D0-D7 to input and drives ACK and LATCH low before anything else;
- the Pi service checks the REQ level when it starts and serves any request already pending. It MUST NOT depend on seeing a REQ edge.

**One codebase.** The emulator and the Pi daemon build the same IoBus with the same device mapping for 00-6F (one function). Each Pi-window access in the emulator, and each serviced REQ on the Pi, is exactly one IoBus read or write. Devices do only bounded local work in read and write (DEVICE_SPECS 3.3). Host I/O goes through Console push_input/take_output. RESET rebuilds the IoBus, so post-reset state equals power-on state by construction. The Pi service is specified in `PI_DAEMON.md`: one thread busy-polls the mmapped GPIO block on an isolated core and calls the IoBus inline. Mailbox commands that do not complete within the execute access run in a worker process on other cores (DEVICE_SPECS 8, Background commands; PI_DAEMON 1). The target Pi uses the BCM2711 register model (Pi 4B for v1, decision PI-PLATFORM).

### 6.5 Overlay Glue (0xFE, 0xFF)

One 74HCT74 half; the other half is the WAIT flip-flop (6.4). /PRE = NOT RESET: the 8224 RESET is active high, and the same inverter drives the WAIT flip-flop's /CLR, the WAIT set term and the IN-latch enable (6.4). /CLR = NOT (I/OW AND port = 0xFE), with a full 8-bit decode; the data bus is not decoded. Q is OVL in the memory decode (6.2). Port semantics: `DEVICE_SPECS.md` (System Control).

**Port 0xFF read.** A GAL I/O pin drives system DB0 directly: output = Q, output enable = I/OR AND port = 0xFF (full 8-bit decode) AND NOT RESET, high impedance otherwise. DB1-DB7 are not driven. /I/OR to DB0 valid is at most 15 ns (tEA, ATF22V10C -15 grade, 0735U 4.3), inside the 118 ns read budget (6.2). The NOT RESET term keeps the pin off DB if the 8228 asserts /I/OR during RESET, while the floated address reads port 0xFF and the ROM may drive DB (6.13).

### 6.6 Reset

- RESIN (8224 pin 2, active low, Schmitt input) is held low until every rail is in regulation and the -5 V and +12 V ramps are complete. The 8224 only synchronizes RESIN to phi2; it does not stretch it. The reset source therefore guarantees the 8080A's minimum of 3 clocks. Source and button: decision RESET-SOURCE (`HARDWARE_BUILD.md`, Decisions).
- The 8224 RESET output (pin 1, active high, VOH 3.6 V at -100 uA) has three loads, all CMOS inputs:
  - the 8080 RESET (pin 12), directly;
  - one 74HCT inverter, which drives the overlay flip-flop /PRE, the WAIT flip-flop /CLR and one GAL input: the WAIT set term, the IN-latch enable and the port 0xFF read (6.4, 6.5);
  - the Pi's RESET GPIO, through a 74LVC245A.
- Clearing the WAIT flip-flop is required. Without it, a reset taken while the Pi is unresponsive leaves READY low, and the first opcode fetch at 0000 hangs.
- The 8224 also drives STSTB low during reset. The NOT RESET term in the WAIT set blocks it. **[bench]** REQ stays low across 100 consecutive resets.
- **The Pi on RESET.**
  - It requests RESET through the gpio character device with both-edge events. The kernel latches the edge, so a pulse of any length is seen, even during an fsync. It also reads the RESET level in every GPIO snapshot.
  - Before every ACK it checks that RESET is not present and REQ is still high, in a GPIO read made after the device operation. It also asks the kernel's edge latch, but only when more than 1 ms has passed since it last asked. That is enough because of the reset source: a DS1813 (decision RESET-SOURCE) holds RESET for at least 100 ms after any assertion (tRST and tPBRST 100 ms minimum, 150 typical, DS1813 AC table), the button and TEST_RESET included, so no pulse can start and end unseen within 1 ms of the previous check. A pulse that came and went shows as REQ low, since only ACK or RESET clears the WAIT flip-flop. If any check finds a RESET, the Pi drops the request without raising ACK. A reset source that can make sub-millisecond pulses would require the latch to be asked before every ACK. Detail: `PI_DAEMON.md` 4-5.
  - When RESET is next low, it returns every device to its power-on state (`DEVICE_SPECS.md` rule 2.8), and only then serves a REQ. The 8080's first Pi-window access after reset waits under READY until then. This costs zero ROM bytes.
  - A bouncing button can produce several RESET pulses. Each one is a full device reset.
- A late ACK for a cycle cut off by reset is never raised. One residual race is accepted: the bus thread is descheduled between its last RESET check and the ACK write for longer than the RESET pulse plus the boot path to the first Pi-window access (about 0.12 ms).
- Nothing else resets the machine, except the optional Pi TEST_RESET (decision TEST-RESET), which pulls the same RESIN node through a 2N3904: collector on RESIN, emitter to GND, base from BCM 18 through 4.7 kohm. At the Pi's minimum VOH of 2.6 V (6.7, INT) and VBE(sat) <= 0.85 V (onsemi 2N3904) that is at least 0.37 mA of base current, against at most 1.75 mA of collector current (the DS1813 pull-up, 5.25 V into 3.5 kohm minimum, plus the 8224 IF of 0.25 mA, reference 11.6), so the transistor saturates. With no Pi the base floats behind 4.7 kohm and the transistor stays off. A test rig that drives TEST_RESET MUST hold each pulse for at least the DS1813 pushbutton detect time tPB, or the full reset time after the pulse, which the 1 ms gate above relies on, is not guaranteed. tPB is 1 us minimum in DS1813 rev 022306; an older Dallas edition says 1 ms in its text, so the rig holds at least 1 ms (`PI_DAEMON.md` 15).

### 6.7 Other CPU Pins

- **INT** (pin 14): v1 has no interrupt source, so INT goes to GND through 10 kohm. The 8228 is wired for its single-level RST 7 feature: its INTA output (pin 23) is strapped to +12 V through 1 kohm (<= 5 mA, 8228 DC table). On acknowledge it then puts RST 7 (FF) on the 8080 bus during DBIN. The strap disables pin 23 as an INTA strobe, so a single-level source needs no new wiring and a vectored controller does. A future source (Pi GPIO tick or 8254, decided when a consumer appears, Someday) drives INT through a 74HCT gate. The 8080A VIH is 3.3 V, which neither a Pi GPIO (VOH 2.6-3.0 V) nor LS-TTL guarantees.
- **HOLD** (pin 13) to GND. 8080A HLDA (pin 21) goes to 8228 HLDA (pin 2). There is no DMA.
- **8228 BUSEN** (pin 22) to GND. A floating bipolar input reads high and tri-states the 8228.
- Any driver of an 8080A input MUST be a CMOS/HCT output. 74LS is not allowed there.
- An 8080A output MAY drive only CMOS inputs, the 8224/8228, an analyzer probe (6.12) and, on A0-A15, the 10 kohm pull-ups (6.13). It MUST NOT drive an LED or a TTL input: it sources 150 uA at VOH 3.7 V and sinks 1.9 mA at VOL 0.45 V (reference 3.2). The status LEDs (6.11) therefore hang on 74HCT08 buffers.

### 6.8 What Is Local and What Is the Pi

| Local (8080 board) | Pi coprocessor |
|--------------------|----------------|
| 8080A, 8224 clock and reset, 8228 system controller (6.1) | Console (a Pi FIFO device; the terminal connects to the Pi) |
| 4 KB ROM at F000, static RAM, memory decode (6.2) | Storage and Mount (SD card) |
| Overlay glue (6.5) | Service Mailbox |
| Window decode, WAIT flip-flop and REQ gate, IN latch, level translation (6.4) | Every other port in 0x00-0x6F |
| Reset circuit (6.6) | |
| ROM write-enable gate and jumper JP-WE (6.10) | |
| Status LEDs (6.11) | |
| Logic-analyzer headers (6.12) | |
| Bus pull-ups (6.13) | |

The console transport between the terminal and the Pi (UART with RTS/CTS, USB gadget serial, or TCP) is Pi configuration and is invisible to the 8080. Console behavior, including input flow control toward the terminal: `DEVICE_SPECS.md` (Console).

### 6.9 Power

- Datasheet maximum currents (8080A p.6-3, 8224 p.6-23, 8228 p.6-35):

  | Rail (all +/-5%) | Load |
  |------------------|------|
  | +5 V | 8080A 80 mA, 8224 115 mA, 8228 190 mA, plus memory, glue, the status LEDs (<= 15 mA, 6.11) and the bus pull-ups (<= 13 mA, 6.13): about 0.8 A total (est) |
  | +12 V | 8080A 70 mA, 8224 12 mA, 8228 INTA strap 5 mA: 87 mA total |
  | -5 V | 8080A 1 mA |

- No 8080A pin or supply may go more than 0.3 V below VBB (8080A absolute maximum). VBB has a Schottky clamp to GND (anode VBB, cathode GND) at the CPU socket. **[bench]** At bring-up, scope VBB single-shot at power-up and power-down: it MUST stay at or below +0.3 V relative to GND.
- +12 V MUST never exceed 12.6 V, including at turn-on. The 8224 VDD absolute maximum is 13.5 V.
- If -5 V comes from a charge pump fed by +5 V, VBB tracks VCC. In that case +5 V MUST be held at 4.85-5.15 V at the board. How the rails are generated is decision POWER (`HARDWARE_BUILD.md`, Decisions).
- **5 V input.** A 2.1 mm centre-positive DC jack feeds the board through a P-channel MOSFET reverse-polarity switch: drain on the jack's centre pin, source on the board +5 V, gate to GND through 10 kohm. With the right polarity the body diode conducts until the channel enhances at VGS = -VIN. With the plug reversed VGS is 0 and the body diode is reverse-biased, so no current flows and no rail goes negative. The part MUST be a logic-level P-MOSFET with RDS(on) specified at VGS = -4.5 V or less and a VGS rating above 5.25 V, so that no gate zener is needed. Fitted: Vishay SUP53P06-20, RDS(on) <= 25 mohm at VGS = -4.5 V, ID = -20 A, 25 °C, VGS +/-20 V (Vishay document 68633 rev C). At about 1 A (the +5 V load above plus about 0.25 A into the +12 V boost, est) it drops at most 25 mV, about 40 mV hot (est: RDS(on) rises with temperature). The 4.85-5.15 V window is measured at the board, after the FET (TP3), so the adapter MUST deliver at least about 4.9 V at the jack under load. **[bench]** +5 V at TP3 and at the jack, board loaded.
- Decoupling: 0.1 uF per IC per rail, plus 10 uF bulk per rail.
- The Pi has its own supply (6.4, Power and boot independence).

### 6.10 ROM Write Enable (JP-WE)

Decided 2026-10-03 (Mike). The circuit that lets the 8080 reprogram its own ROM. The routine that would do it (a monitor `burn` command) is Someday (`TODO.md`). This section fixes the circuit, its default and the rules that routine must follow.

**Circuit.** One 74HCT138 decodes the ROM range during MEMW. Jumper JP-WE connects its output to the EEPROM:

| 74HCT138 input or output | Connection |
|--------------------------|------------|
| A, B, C | A12, A13, A14 |
| G1 | A15 |
| /G2A | 8228 /MEMW |
| /G2B | GND |
| /Y7 | JP-WE, and LA-C (6.12) |
| /Y6-/Y0 | unconnected |

The other side of JP-WE is the AT28C64B /WE, which has a 10 kohm pull-up to +5 V. Pin numbers: `hw/board.net.txt` (U12, JP1), checked against this list by `tests/netlist_tests.rs`.

- /Y7 is low only while A15-A12 = 1111 and /MEMW is low (SCLS171F Table 7-1). With JP-WE fitted, ROM /WE follows /Y7: that is ROM_WE in 6.2. /Y7 then also drives the pull-up, 0.5 mA, inside the 4 mA at which HCT output levels are specified.
- The MEMW edges reach /WE through one enable path, at most 42 ns (SCLS171F 5.5). Address changes cannot glitch /WE: the address is stable from 688 ns before /WR falls until at least 118 ns after it rises (tAW, tWA, reference 3.5), and /MEMW is inactive outside that window.

**Default: open.** With JP-WE open, the pull-up holds ROM /WE high whatever the logic does, with the 138 missing, unpowered or faulty included. That is the state the old tie to VCC gave, and WE high inhibits every write (AT28C64B DS 4.6.1 (c)). The board is built, and runs in normal use, with JP-WE open: the shunt is parked on one pin. /Y7 still pulses on every write to F000-FFFF, so the decode can be checked with no risk to the ROM (`HARDWARE_BUILD.md` 3, step 3).

**Range.** F000-FFFF only. There is no OVL term, so with the overlay set a write to 0000-0FFF still goes only to RAM (section 4). A12 is tied low at the chip, so a write to F000+n programs EEPROM cell n (0000-0FFF), and the upper 4 KB of the 8 KB part is never written. RAM_WE has no address term, so the same write also lands in the RAM under the ROM (6.2), which nothing reads.

**Write timing.** All figures are worst case at tCY 488.28 ns. Limits are from AT28C64B DS 14 and 16; 8080A, 8224 and 8228 figures are from the reference sections named. JP-WE adds no delay.

| AT28C64B requirement | This board |
|----------------------|------------|
| tWP: /WE low >= 100 ns | /WR is low from tDC after phi1 rising in T3 to tDC after phi1 rising in the next state. Memory writes insert no T_W. tDC is 0-120 ns, so /WR is low for at least 368 ns (reference 3.3, 4.3). 8228 tWR is 5-45 ns on each edge (reference 12.4), so /MEMW is low for >= 328 ns. The 138 enable path is <= 42 ns with no stated minimum, so /WE is low for >= 286 ns. |
| tAS 0 ns, tAH 50 ns (address to /WE falling) | address stable >= 688 ns before /WR falls (tAW) and >= 118 ns after /WR rises (tWA) (reference 3.5) |
| tDS: data >= 50 ns before /WE rises | data on D0-D7 >= 169 ns before /WR falls (tDW, reference 3.5). It reaches DB within 8228 tWD 40, so it is on DB >= 129 ns before /WR falls, and >= 415 ns before /WE rises. |
| tDH: data >= 0 ns after /WE rises | DB holds >= 123 ns after /WR rises (tWD 118, reference 3.5 and 13.3, plus 8228 tWD min 5, reference 12.6; derived: the 8228 keeps its direction from the latched status until the next /STSTB). /WE rises <= 87 ns after /WR (8228 tWR 45 + 138 42), so the hold margin is >= 36 ns. |
| tOES, tOEH: /OE high 0 ns before and after | the 8228 issues no MEMR in a write cycle (reference 12.3), so ROM_OE is inactive and ROM /OE is high |
| tWPH: /WE high >= 50 ns between writes | two writes are at least one machine cycle (>= 3 states) apart |

The 15 ns /WE noise filter (typ, AT28C64B DS 4.6.1 (d)) is not relied on.

**Rules for any code that writes the ROM.** Normative now, so that the Someday routine is built to them:
1. **JP-WE is open in normal use.** Fit it only for a burn and remove it afterwards. While it is fitted, any write to F000-FFFF reprograms the monitor: a user program, the E, A, F, L and M commands (they do not guard F000-FFFF, `MONITOR_SPEC.md` 6), and a stack that wraps from SP = 0000 into FFFF.
2. **Run from RAM with the overlay clear.** After each byte or page write, every read of the EEPROM is a polling read, not data, for up to tWC = 10 ms (AT28C64B DS 4.2, 4.4, 4.5, 16). Code fetched from F000-FFFF, or from the 0000 mirror while the overlay is set, would be garbage during that time.
3. **Detect the end of a write by polling, never by a delay.** Read the last address written until bit 7 returns the true data (DATA polling, AT28C64B DS 4.4), or until bit 6 stops toggling (AT28C64B DS 4.5). The ROM has no timing-dependent code (3.2, requirement 6).
4. **Page writes.** A page is 1-64 bytes in one 64-byte page: chip A6-A12 must be the same for every byte, which with A12 tied low means one 64-byte-aligned block of F000-FFFF. Each byte must follow the previous one within tBLC = 150 us, or the chip closes the page and ignores the rest (AT28C64B DS 4.3, 16). The routine therefore buffers the data in RAM first and makes no Pi-window access inside a page load: a Pi-window access can wait under READY without bound (6.4, rule 4).
5. **Software data protection stays disabled.** The part ships with SDP disabled (AT28C64B DS 4.6.2), and the chip MUST be programmed with SDP left off. The SDP enable and disable sequences write to chip address 1555 (AT28C64B DS 19, 20), which needs A12 = 1. A12 is tied low, so the 8080 can neither set nor clear SDP. A chip with SDP set rejects every in-circuit write, and each rejected write still starts a tWC polling period (AT28C64B DS 4.6.2).
6. **Power transitions.** The chip blocks writes below VCC 3.8 V (typ) and for 5 ms (typ) after VCC reaches it (AT28C64B DS 4.6.1 (a), (b)). The reset supervisor holds RESET below about 4.6 V (6.6). Neither replaces rule 1.

**Emulator.** It models JP-WE open: a write to F000-FFFF never changes the ROM image (section 4). A fitted jumper (write cycle, polling reads) is modeled together with the burn routine (Someday).

### 6.11 Status LEDs

Decided 2026-10-03 (Mike). Four LEDs, each driven by a 74HCT08 output, never by an 8080A pin (6.7).

| LED | Lit while | Driver |
|-----|-----------|--------|
| WAIT | 8080A WAIT (pin 24) is high: a wait state (T_W) or the halt state (TWH) (reference 5, 9) | U08a, buffer |
| HALT | the CPU is halted: HALT = WAIT AND /Q AND /I/OR AND /I/OW | U08b, three gates |
| INTE | 8080A INTE (pin 16) is high: interrupts enabled (reference 7) | U08a, buffer |
| HLDA | 8080A HLDA (pin 21) is high (reference 8) | U08a, buffer |

- U08a is the existing 74HCT08. Gate 1 is REQ (6.4). Its three spare gates become buffers, with the second input tied to +5 V, so each LED adds one HCT input to its 8080A pin (Ci <= 10 pF, SCLS063G 4.4).
- U08b is a second 74HCT08: H1 = WAIT_B AND /Q, where WAIT_B is the WAIT buffer output and /Q is the WAIT flip-flop output that drives RDYIN (6.4); H2 = /I/OR AND /I/OW; HALT = H1 AND H2. Gate 4 has both inputs tied to GND.
- HLDA is always dark in v1, because HOLD is tied low (6.7). If it lights, HOLD is miswired.

**Why HALT is this term.** The 8080A has no HALT pin, and the board has no status latch (6.4, Direction). WAIT is high both in the halt state and in every wait state (reference 5, 9). On this board only Pi-window accesses wait: memory, ports FE/FF and ports 70-FD insert none (6.2, 6.3, 6.5). A Pi-window wait is always covered by one of two terms:
- From T1 until the ACK, the WAIT flip-flop is set, so /Q is low.
- From the ACK until WAIT falls in T3 (0.4-0.9 us, 6.4 rule 3), /I/OR (on IN) or /I/OW (on OUT) is active.

In the halt state neither term applies. The flip-flop is clear, because its set needs INP or OUT status and the halt state has no SYNC (6.4 rule 1, reference 9, 11.3). The 8228 issues no strobe for the halt-acknowledge status 8A (reference 12.3). WAIT AND /Q alone would also be true for the 0.4-0.9 us after every ACK, which glows visibly under console traffic.

HALT is therefore high in TWH, and otherwise for at most a few tens of ns at the end of some IN cycles: WAIT_B can fall as late as 150 ns into T3 (tDC 120 + HCT08 tpd 30, SCLS063G 4.5), and /I/OR can rise as early as 133 ns into T3 (tD3 108.5 + tDF 25, reference 3.3, 3.5, 4.3). Pi-window accesses are microseconds apart, so the LED cannot show this pulse. An analyzer on LA-C can (6.12), and it is not a fault.

**Reading them.**

| WAIT | HALT | INTE | Meaning |
|------|------|------|---------|
| off | off | any | running, not in a wait |
| dim or flickering | off | any | running with Pi-window traffic (the monitor prompt polls `IN 02`) |
| steady | off | any | stalled in a Pi-window access with no Pi service: daemon stopped or Pi off (6.4 rule 4) |
| steady | steady | off | halted with interrupts disabled: only RESET exits (reference 9) |
| steady | steady | on | halted, waiting for an interrupt (no source in v1, 6.7) |
| off | on | any | impossible (HALT includes WAIT): wiring fault |

The monitor never executes HLT or EI (3.2, requirement 4), so HALT and INTE light only for user programs.

**Drive and resistor.** Each LED is wired from a 74HCT08 output through 1.0 kohm to the anode, with the cathode to GND, so it is lit when the output is high.
- 74HCT08 VOH is >= 3.84 V at -4 mA (SCLS063G 4.4). VOH cannot exceed VCC, at most 5.25 V (6.9).
- Use a low-current LED with Vf 1.6-2.2 V at 2 mA: red, orange, yellow or GaP green. InGaN blue, white and true-green parts (Vf about 3 V) are too dim at this drive.
- LED current I = (VOH - Vf) / 1.0 kohm ranges from 1.6 mA (3.84 V, Vf 2.2 V) to 3.7 mA (5.25 V, Vf 1.6 V). That stays within the 4 mA at which VOH is specified, and far below the 25 mA per-output and 50 mA VCC/GND absolute maximums (SCLS063G 4.1). The three LEDs on U08a total <= 11 mA.

### 6.12 Logic-Analyzer Headers

Decided 2026-10-03 (Mike). Three 2x10 0.1-inch headers carry the bus to an external logic analyzer:
- LA-A: address.
- LA-B: CPU-side data and the CPU timing signals.
- LA-C: system strobes, the WAIT and overlay flip-flops, the Pi handshake, the ROM write decode and HALT.

Each header has 16 signals in channel order and GND on four pins. Channel order, the analyzer and sigrok decoding: `HARDWARE_BUILD.md` 3.1; pin numbers: `hw/board.net.txt`.

1. **Logic levels only.** phi1 and phi2 (8224 pins 11, 10) swing to >= 9.4 V (reference 11.6) and MUST NOT be on a header. The clock reference is the 8224 phi2 (TTL) output (pin 6). 8228 /INTA (pin 23) sits on the +12 V RST 7 strap (6.7) and MUST NOT be on a header. ACK and LATCH are 3.3 V levels (6.4). Every other signal is a 5 V level.
2. **Direct taps, except CPU-side D0-D7,** which go through 1 kohm series resistors at the tap (one isolated 8-resistor array). On reads the 8228 drives the CPU side. Its VOH (>= 3.6 V) is characterized only at -10 uA, and its tRD of 30 ns, which is part of the 118 ns memory read budget (6.2), only at 25 pF (reference 12.6, 12.7). D4 and D6 already carry more than that before any probe: the 8080A pin (COUT <= 20 pF on a bidirectional pin, reference 3.2), the GAL tap (CIN <= 8 pF, ATF22V10C) and 5-10 pF of trace, 33-38 pF (est). That is a pre-existing risk (`HARDWARE_BUILD.md` 6); the resistor reduces what the probe adds to it, and bring-up step 3 must pass with the analyzer attached. The cost is about 33 ns of extra edge delay at the analyzer (2.2 x 1 kohm x 15 pF, est), which the sampling rules in `HARDWARE_BUILD.md` 3.1 allow for.
3. **Analyzer load.** The board is designed for an analyzer that adds, per channel, at most 15 pF including its lead and at most 10 uA at any level from 0 to 5.25 V, powered or not. An analyzer outside that MUST NOT be attached: one that clamps its inputs to its own rail, or is attached unpowered, pulls the direct taps down and, through the 1 kohm, D0-D7 below the 8080A's VIH of 3.3 V. Within it:
   - **DC load.** <= 10 uA, inside the 8080A's 150 uA source (reference 3.2).
   - **Capacitance.** The 8080A address and data pins are specified into 100 pF, and SYNC, DBIN, /WR, WAIT and HLDA into 50 pF. Above that, add 0.6 ns per pF (reference 3.3, note 4c). With the analyzer attached the load is (est, <= 10 pF per CMOS input from SCLS063G 4.4, SCLS171F 5.4, AT28C64B DS 13 and ATF22V10C, plus 5-10 pF of trace): address pins <= about 75 pF (A15 is the heaviest, with the GAL, RAM /CE, 74HCT14, 74HCT138 and the probe); control pins <= about 45 pF (WAIT is the heaviest, with two 74HCT08 inputs and the probe).
4. **The board MUST meet all of section 6 with the analyzer attached.** Bring-up runs with it attached (`HARDWARE_BUILD.md` 3). A board that passes only without the analyzer fails.

### 6.13 Bus Pull-Ups

Decided 2026-10-03 (Mike). 10 kohm pull-up SIPs hold the address bus and the system data bus when nothing drives them. The 8080A floats A0-A15 and D0-D7 in the halt state (reference 9), during RESET (reference 10) and in hold (reference 8; HOLD is tied low in v1, 6.7). Without pull-ups every CMOS input on those nets then floats: RAM, ROM, the 74HCT138 and 74HCT14, and the 74LVC245As to the Pi. Floating inputs cost supply current and let decode outputs toggle. In halt and hold no strobe is active. Whether the 8228 read strobes stay inactive through RESET is unresolved: reference 14 item 7 says DBIN-gated in its text, while its waveform follows STSTB, which the 8224 holds low in reset. If /MEMR or /I/OR is active then, the NOT RESET terms on the IN-latch enable (6.4) and the port 0xFF read (6.5) keep both off DB, so only the memory the floated address selects can drive it.

| Net | Pull-up | Part |
|-----|---------|------|
| A0-A15, at the 8080A pins | 10 kohm to +5 V | two 9-pin bused SIPs (A0-A7, A8-A15) |
| System DB0-DB7 (8228 side) | 10 kohm to +5 V | one 9-pin bused SIP, in a socket (Bring-up below) |
| CPU-side D0-D7 | none: it MUST NOT have a pull-up | |

**Sources.** 8080A and 8228 from reference 3.2, 3.3, 12.6 and 12.7 (MCS-80 UM p.5-15, 5-17 and 5-11 / PDF p.77, 79 and 73). AS6C62256: Alliance datasheet v1.0 (Feb 2007), DC and test-load tables, p.3. AT28C64B DS 8 (p.5) and 12 (p.7). ATF22V10C: Atmel 0735U-PLD-7/10, 4.1 (p.4), 4.3 (p.5) and 8 (p.8). TI SCLS005E (SN74HCT374) 5.4 p.5, SCAS218X (SN74LVC245A) 7.3 p.5 and 7.5 p.6. A 10 kohm 2% pull-up passes at most (5.25 - 0.45 V) / 9.8 kohm = 0.49 mA into a low output. Worst case unless marked (est).

**Address bus (A0-A15).**
- The 8080A sinks 1.9 mA at VOL 0.45 V and sources 150 uA at VOH 3.7 V (reference 3.2).
- Low: the pull-up's 0.49 mA plus at most about 45 uA of input leakage on the heaviest line (GAL 10, ROM 10, 74LVC245A 10, analyzer 10 (6.12 rule 3), the RAMs and HCT inputs 1 each) is 0.54 mA. That leaves 1.36 mA of the 1.9 mA.
- High: the pull-up supplies current toward VCC, so it adds to the 150 uA source instead of using it.

**System data bus (DB0-DB7).**
- Drivers and their guaranteed sink: the 8228 system side 10 mA at 0.45 V (writes); AS6C62256 2 mA at 0.4 V; AT28C64B 2.1 mA at 0.40 V; 74HCT374 (IN latch) 6 mA at 0.33 V (-40 to 85 °C); ATF22V10C (DB0, 6.5) 16 mA at 0.5 V (0735U 4.1).
- Low: the pull-up's 0.49 mA, the 8228 DB input's 0.25 mA (IF, "all other inputs", reference 12.7) and the off-state leakage of the idle parts, at most 37 uA (ROM 10, 74LVC245A 10, GAL 10 on DB0, 74HCT374 5, each RAM 1), total 0.78 mA. The weakest driver, the SRAM, keeps 1.22 mA of its 2 mA. DB0 also carries the GAL pin-keeper, which a driver overdrives with 40 uA (typ, no max, 0735U 8), inside that margin.
- High: the pull-up only helps. It lifts the 8228's TTL high (VOH 2.4 V at -1 mA) toward VCC, which adds margin against the AS6C62256 VIH of 2.4 V (`HARDWARE_BUILD.md` 6).

**Why not CPU-side D0-D7.**
- **No pull-up value fits the budget.** On a read the 8228 drives the CPU side against the 8080A's active pull-up: IDL up to 2.0 mA while DBIN is high and the line is above 0.8 V (reference 3.2 note 2, p.5-15). The 8228 guarantees VOL 0.45 V only at IOL 2 mA (reference 12.7). A 10 kohm pull-up adds 0.45 mA at 0.8 V, so a bit going from 1 to 0 under DBIN would need 2.45 mA from a part rated for 2. Any larger resistor still adds current to a budget with zero margin, so the net is excluded instead of given a weaker pull-up.
- **Nothing there needs it.** Its only CMOS inputs are the GAL's D4 and D6, and the ATF22V10C holds every input and I/O pin with a pin-keeper (0735U 8). The 8228's D inputs are bipolar. The analyzer sits behind 1 kohm (6.12).

**Timing.**
- **Driven edges.** On a falling edge the driver also sinks the pull-up's current (<= 0.49 mA). That is less than the DC load each part's own A.C. figures are measured with: 8080A about 1.8 mA (2.1 kohm and a diode to +5 V, p.5-17 note 2), 8228 system side 500 ohm to VCC (p.5-11 note 2), AT28C64B 1.8 kohm to 5 V (DS 12), AS6C62256 one TTL load at 2 mA (p.3). The 74HCT374 sinks 0.78 mA of its 6 mA (est: TI specifies its delays into 50 pF only), and the GAL 0.78 mA of its 16 mA on DB0. Rising edges gain the pull-up's current. tDA, tDD, the 118 ns read budget (6.2) and the 6.10 write timing all stand.
- **Undriven lines** rise with tau = 10 kohm x C: about 0.75 us on the address bus (<= 75 pF, 6.12 rule 3) and 0.8 us on DB (est, about 80 pF: 8228 15, ROM 12, two RAMs 16, the 374, GAL and 245 about 25, trace 10). From 0.45 V a line crosses 2.0 V after about 0.4 tau and reaches 4.5 V after about 2.2 tau (est). A floated bus therefore reads FFFF or FF within about 2 us.
- **Slow ramps.** Through the 74LVC245A input threshold that ramp is about 200 ns/V (est), outside the part's 10 ns/V input transition rate (SCAS218X 7.3). That is not new: DB is undriven between bus cycles in normal operation, whenever no memory, write cycle, IN latch or overlay read drives it, and without a pull-up it drifts slower still and can stop at the threshold. The Pi never uses a 245 output then: it reads only while REQ is high, when the address and, on OUT, DB are driven (6.4). On A12-A15 the same ramp (halt, RESET, hold) is also slower than the 74HCT138's 500 ns maximum input transition time (est, 10-90 % is 2.2 tau, about 1.65 us; SCLS171F 5.2). That is harmless: /G2A is /MEMW, inactive then, so every 138 output stays high (6.10), and without the pull-ups those inputs would float and could stop at the threshold, which is worse. The 74HCT14 on A15 is a Schmitt input and has no such limit.

**Consequences.**
- In halt, RESET and hold the address bus reads FFFF and DB reads FF, unless the ROM drives DB during RESET (above). Nothing samples them then: the WAIT set needs /STSTB low and NOT RESET (6.4 rule 1), port FF is outside the Pi window, and in halt and hold no strobe is active.
- An undriven DB drifts to FF. An `IN` from a port nothing drives (70-FD, FE) needs its data on DB about 1 us after the memory released DB in the instruction's M2 (est), so it reads FF in practice. This is informative only. `DEVICE_SPECS.md` 2.4 still says the value is undefined, and software MUST NOT depend on FF.
- **Bring-up.** The DB SIP and the 2.2 kohm bring-up pull-down (`HARDWARE_BUILD.md` 3, step 2) MUST NOT be fitted together. The pull-down alone holds an undriven DB line at <= 0.55 V (the 8228's 0.25 mA IF into 2.2 kohm). With the 10 kohm pull-up added it sits at about 1.4 V (est), above the 8228's minimum threshold of 0.8 V, so the free-run no longer reads NOPs. The DB SIP is therefore socketed: out for step 2 and for any CPU screened that way, in from step 3 on. The address SIPs stay fitted throughout.
- **Power.** All 24 lines low draw at most 13 mA from +5 V: 24 x 5.25 V / 9.8 kohm, with the low at 0 V (6.9).

---

## 7. Host-Side Conveniences (Emulator Only)

**Rule:** none of these is visible to the 8080 as a port, a memory location, a byte in its input stream, or a timing difference. The ROM contains no code path that exists only for the emulator. This section is the only home for host-reserved keys and the key map; `DEVICE_SPECS.md` and `MONITOR_SPEC.md` link here.

### 7.1 Host Key Map

The emulator turns host key presses into console input bytes:

| Key | Byte(s) delivered to the console FIFO |
|-----|---------------------------------------|
| Enter | 0D |
| Backspace | 08 |
| Tab | 09 |
| Esc | 1B |
| Ctrl-A .. Ctrl-Z (either letter case) | 01 .. 1A, except Ctrl-C and Ctrl-E |
| Ctrl-C | none: host quit (7.2) |
| Ctrl-E | none: stops the CPU and opens the debugger prompt (7.4) |
| Printable ASCII (20-7E) | its byte |
| Non-ASCII character | its UTF-8 bytes, in order |
| Any other key (arrows, function keys, and so on) | none: dropped |

Test: scripted Ctrl-A delivers 01 at `IN 01`; scripted Esc delivers 1B; scripted "é" delivers C3 A9; scripted Ctrl-E opens the prompt after the keys before it. Code: `map_key` in `src/main.rs`, tested there with the run loop. The real binary under a pseudo-terminal (crossterm raw mode and key decoding, Backspace 7F -> 08, Ctrl-C, Ctrl-E and `c`, an interactive HLT and `q`, the terminal mode restored on exit, a piped run never touching it): `tests/terminal_tests.rs`, Unix only.

On hardware the Pi passes every byte the terminal sends; the key map does not exist there. The ROM MUST NOT rely on receiving, or on not receiving, 03 or 05.

### 7.2 Run Loop

| Convenience | Contract |
|-------------|----------|
| **Host input pump** | The host run loop reads pending host keyboard input at least once every 10,000 executed steps and puts the mapped bytes (7.1) into the console input FIFO in arrival order. The key source is injectable, so tests can script it. Console output is drained to host stdout as often. When stdin is not a terminal, its bytes go to the FIFO unmapped and Ctrl-C is the shell's. Code: `run_loop` in `src/main.rs`. |
| **Idle wait** | A pump first takes what is pending without waiting. It then blocks for host input for up to 1 ms (a terminal poll, or a wait on the stdin reader) and takes what arrived only when all three hold: the first take brought nothing, the console input FIFO is empty, and the 8080 read `IN 02` since the previous pump (it is polling for input, as the monitor's CONIN does at the prompt). A compute-bound program does not read `IN 02`, so it never waits; a program that polls `IN 02` inside a compute loop does, about 1 ms per 10,000 steps. Measured 2026-10-03, release: idle at the monitor prompt (piped stdin at EOF) about 27% of a core (about 100% with no wait); a 26M-step `DCX B` loop run with `G` takes 0.20 s, the same as with no wait (the first trigger, without the `IN 02` condition, took 15x longer). The 8080 cannot see it: it adds no cycles. `N` and `Q` trigger it too: their BUSY wait and each LF's Esc check read `IN 02` with the FIFO empty (`MONITOR_SPEC.md` 6.18), so a long wait no longer burns a host core, and a fast stream in `cargo run` is paced by up to 1 ms per 10,000 steps. Code: `idle_waits` in `src/main.rs`, `Console::take_polled`. Tests: `idle_waits_only_when_nothing_came_the_fifo_is_empty_and_the_8080_polled`, `a_pump_waits_only_while_the_8080_polls_an_empty_fifo`, `a_compute_bound_program_never_waits` in `src/main.rs`; `console_host_side_sees_status_polls` in `tests/device_tests.rs`. |
| **End of piped input** | When stdin is not a terminal, end of input changes nothing: the FIFO just stays empty. The run ends on a halt or Ctrl-C (the shell's), never at EOF, so output the 8080 has not printed yet is never cut off. |
| **Ctrl-C quits** | When the pump reads Ctrl-C, nothing goes into the FIFO. The run loop returns a quit status, and `main.rs` restores the terminal mode and exits. This works whatever the 8080 is doing, including `JMP $`. Test: script Ctrl-C while the CPU runs `JMP $`; the run loop returns quit within 10,000 steps, and `IN 01` never returns 03. |
| **Halt** | v1 has no interrupt source that could wake a halted CPU (5.8). In an interactive run (stdin is a terminal and there is no `--script`), a halt opens the debugger prompt with the reason `halt` (7.4): the user can inspect, and `q` quits. Otherwise (piped stdin, or `--script`) the run loop returns a halted status, and `main.rs` prints `HLT at PC=xxxx` to host stdout, where xxxx is PC (the address after the HLT), restores the terminal, and exits. Code: `halt_prompts` in `src/main.rs`. |
| **Debugger** | Every step goes through the debugger (7.4), which records the trace ring and checks breakpoints. When the pump reads Ctrl-E, the bytes before it go into the FIFO and the debugger prompt opens before the next step. |
| **Host banner and exit lines** | "8080 Emulator", the build timestamp, and any exit message go to host stdout from `main.rs`, never from the CPU core or from a device the 8080 can see. |
| **Test harness** | Allowed only on the host side: the real port map from `build_bus` with the `Console`'s host side (scripted input and captured output); test `IoDevice`s mapped with `map_port`, for example one that records every port access; `load_program` (writes that bypass the ROM and the overlay); a CPU with no ROM loaded; `interrupt(rst)` (5.7); direct access to the registers, memory, `cycles`, `halted` and `interrupts_enabled`; the debugger's command parser (7.4). |
| **No throttle** | The emulator runs at host speed, apart from the idle wait above. `cycles` counts T-states only. |

### 7.3 Port Trace Format

This is the only home for the port-trace line format. One line per event:

```
IN pp vv
OUT pp vv
RESET
```

- `pp` is the port and `vv` the byte transferred, each two uppercase hex digits (`MONITOR_SPEC.md` numeric output convention).
- Any line MAY end with ` ; annotation`, free text after the semicolon.
- **Repeats:** a run of N > 1 identical consecutive lines MAY be collapsed into one line, `<line> ; xN`, with N in decimal. A single line carries no ` ; x1`. The emulator debugger and the Pi daemon both collapse repeats this way, so a polling loop is one line in either trace and the traces diff cleanly. (Decided 2026-10-03.)
- Both the emulator debugger and the Pi daemon emit this format. Hardware traces are diffed against emulator traces for the same ROM and input.
- Only the line format and the repeat rule are fixed. Which events a given tool records is decided with that tool.

### 7.4 Debugger

Host-side only. The 8080 cannot observe it: it adds no cycles, no port, no memory and no byte to the console stream. Code: `src/debugger.rs` (commands, breaks, ring, trace), `src/disasm.rs` (the opcode table, the assembler and the instruction line `disasm::line`: device code for mailbox `ASM` and `DIS`, `DEVICE_SPECS.md` 8, which the debugger reuses), `src/main.rs` (entry and prompt). Tests: `tests/debugger_tests.rs`.

**Entry.**

| How | Effect |
|-----|--------|
| Ctrl-E (7.1) | Stops at the next step boundary, at most one pump interval (7.2) later. |
| `--debug` | Starts stopped, before the first instruction. |
| `--script FILE` | Starts stopped and reads commands from FILE, one per line, before the terminal. Each line is echoed as `dbg> line`. Blank lines and lines starting with `#` are skipped. |
| A breakpoint, watchpoint or I/O break | Stops (below). |
| A halt, in an interactive run (7.2) | Stops with the reason `halt`. The CPU stays halted: `s` prints the registers line, and `c` stops again at once. |

At the prompt the terminal is in line mode: the prompt is `dbg> `, Ctrl-C is the shell's, and end of input (Ctrl-D) is `q`. `c` and `s` return to the 8080. Commands come from the script until it runs out, then from the terminal; when stdin is not a terminal, a stop after the script has run out quits.

**Arguments.** Separated by spaces. Numbers are hex, any case, no prefix or suffix: 1-4 digits for addresses, counts and lengths, 1-2 for ports. An address is a number, `NAME`, or `NAME+n` with n a number; `NAME` is a ROM symbol, any case. A token that is valid hex is a number, never a symbol. An address wraps modulo 0x10000.

**Commands.**

| Command | Effect |
|---------|--------|
| `c` | Continue. A breakpoint at the current PC does not stop the first step. |
| `s [n]` | Step n instructions (default 1). Stops early on a breakpoint, watchpoint or I/O break, with a stop report; otherwise prints the registers line and the next instruction. On a halted CPU it does nothing but print the registers line. |
| `r` | Registers line. |
| `m ADDR [LEN]` | Memory, in the monitor's `D` line format (`MONITOR_SPEC.md`, D), 16 bytes per line from ADDR, enough lines to cover LEN bytes (default 40). Reads as the CPU would (ROM, overlay); a debugger read is not a bus transfer and triggers nothing. |
| `u [ADDR] [N]` | Disassemble N instructions (default 8) from ADDR (default PC). |
| `b ADDR` | Breakpoint: stop before the instruction at ADDR executes. |
| `w ADDR[-END] [r\|w]` | Watchpoint on ADDR..END inclusive (END >= ADDR): `r` data reads, `w` writes, default both. Stops after the instruction that made the transfer. |
| `io PORT [in\|out]` | I/O break: stop after an `IN` or `OUT` on PORT (default both). |
| `bl` | List the breakpoints, watchpoints and I/O breaks, one per line, each as the command that sets it, in the order set. |
| `bc [ADDR]` | Clear the breakpoint at ADDR; with no argument clear every breakpoint, watchpoint and I/O break. |
| `t FILE` / `t off` | Port trace to FILE (created or truncated) / stop it. |
| `ring [N]` | The last N steps (default all) from the trace ring, oldest first. The ring holds the last 256 steps. |
| `sym ADDR` | The address and its location (below). |
| `?` | One-line command summary. |
| `q` | Quit the emulator, like Ctrl-C (7.2). |

A bad command or argument prints one line `? message` and changes nothing. At the terminal the prompt goes on. In a script it fails fast: after the `? message` line the emulator exits with status 2, so a broken script never runs on with the wrong breakpoints. (Decided 2026-10-03.)

**Bus transfers.** Watchpoints, I/O breaks and the port trace see the data transfers of each step: memory reads and writes made by the instruction (stack accesses and the interrupt-acknowledge push included) and every `IN` and `OUT`, ports FE and FF included. Opcode and operand fetches are not data transfers (use `b`). A write to F000-FFFF is a transfer even though the ROM ignores it. Transfers come in the 8080's bus order: a push (`PUSH`, `CALL`, `RST`, the acknowledge) writes the high byte to SP-1 first, then the low byte to SP-2; `XTHL` reads SP and SP+1, then writes H to SP+1 and L to SP. Mechanism: `Intel8080::transfers()` lists the last step's transfers. On hardware, operand fetches are ordinary MEMR cycles too (only the opcode fetch sets M1), so leaving them out is the debugger's choice, not something the 8228 status byte separates; `b` covers them. Recording transfers changes no CPU state and no cycle count.

**Output.** Hex is uppercase and zero-padded (`MONITOR_SPEC.md` numeric output convention).

- **Location:** `AAAA`, or `AAAA NAME` / `AAAA NAME+n` with the nearest symbol at or below AAAA in the same memory-map region (section 1), so a user-area address is never named after the workspace.
- **Registers line:** `PC=F28B SP=F000 A=44 F=56 -ZAP- BC=0B0D DE=0000 HL=0081 INTE=0 OVL=0`, then ` HLT` when halted. The flag field is S Z A P C, a letter when set and `-` when clear. OVL is the overlay flip-flop (section 4).
- **Instruction line:** the mailbox `DIS` line (`DEVICE_SPECS.md` 8, DIS: `AAAA  B0 B1 B2  MNEMONIC`, the bytes field 8 wide, Intel mnemonics, operands in hex, the undocumented aliases starred), with symbols: an address operand (jumps, calls, `LDA`, `STA`, `LHLD`, `SHLD`) equal to a symbol prints as the name (`CALL SKIP_SPACES`); immediate data (`LXI`) always prints in hex. Wherever an instruction line is listed (`u`, `s`, a stop report; not ring lines), an address that is a symbol is preceded by a `NAME:` line.
- **Ring line:** the instruction line padded with spaces to 34 characters, one space, then the registers before it ran: `A=44 F=56 BC=0B0D DE=0000 HL=0081 SP=F000`.
- **Stop report:** a reason line, then the last 8 ring lines (the last one is the instruction that caused a watchpoint or I/O stop), then the registers line, then the next instruction. The reason line always begins a line: when the console output before a stop does not end with LF, the host writes CR LF first. Reasons: `* break LOCATION`, `* watch read AAAA VV`, `* watch write AAAA VV`, `* io IN PP VV`, `* io OUT PP VV`, `* ctrl-e`, `* start`, `* halt`.

**Port trace.** One 7.3 line per `IN` or `OUT` transfer, ports FE and FF included. Repeats are collapsed by the 7.3 rule (`IN 02 02 ; x12`). Pending lines are written at every stop and at `t off` and quit, so a run split by a stop is written as two lines. The debugger writes no `RESET` line: it has no reset. Diffing against a Pi daemon trace (7.3): the Pi never sees ports 70-FF (`DEVICE_SPECS.md`), so drop the FE and FF lines; drop `RESET` lines, which only the daemon writes; drop the `IN 02 02` lines (empty-FIFO console polls carry no data; `N` and `Q` interleave one with every BUSY status read, `MONITOR_SPEC.md` 6.18, so without this their `IN 12 01` lines never merge); strip the ` ; xN` annotation, since poll counts depend on timing; then merge adjacent identical lines, since a stop splits a run (`grep -Ev '^(IN|OUT) F[EF] |^RESET$|^IN 02 02' | sed 's/ ; x[0-9]*$//' | uniq`).

**Symbols.** `rom/monitor.sym` is built with `monitor.bin` by `cd rom && make` and committed with it: one `AAAA NAME` line per label in `monitor.asm`, from asl's NoICE output: the code labels and the workspace labels (1.1), so `w STOR_ADDR` works. EQU constants are left out: they are ports, characters and sizes. The emulator loads it from next to `monitor.bin` when present; without it a `NAME` argument is an error and output has no names.

**Not in v1:** reset, writing registers or memory, conditional breakpoints, expressions beyond `NAME+n`, a TUI.

---

## 8. Later Phases (Placeholders)

- **Phase 6 (done 2026-10-03):** Service Mailbox device (ports 10-13), mailbox `TIME`, and the `T` command. Protocol: `DEVICE_SPECS.md` (Service Mailbox). Command: `MONITOR_SPEC.md`. No memory-map or circuit change.
- **Phase 7 (done 2026-10-03):** mailbox `ASM` and `DIS`, and the `A` and `U` commands (`DEVICE_SPECS.md` 8, `MONITOR_SPEC.md` 6.16-6.17). No memory-map or circuit change.
- **Phase 8 (done 2026-10-03):** mailbox `GET` and the `N` command (`DEVICE_SPECS.md` 8, `MONITOR_SPEC.md` 6.18). The first background command, its worker a `curl` process. No memory-map or circuit change.
- **Phase 9 (done 2026-10-03):** mailbox `ASK` and the `Q` command (`DEVICE_SPECS.md` 8, `MONITOR_SPEC.md` 6.19). A second background command on the same `curl` worker; the API key lives on the Pi, never in ROM. No memory-map or circuit change.
- **Phase 10 (done 2026-10-03):** the monitor's `R` command and the G_RETURN capture (`MONITOR_SPEC.md` 6.20, 8) and the REGS workspace row (1.1). No circuit change.
- **Phase 11 (done 2026-10-03):** example programs, the user guide and a consistency pass. No memory-map, circuit or
  ROM change.
- **Phase 12 (Track B):** RST 6 breakpoints (`MONITOR_SPEC.md` 8.1): a 0030-0032 vector written by G. No circuit change.
- **Someday:** a periodic interrupt source (tick from a Pi GPIO or an 8254, decided when a consumer appears) and its ISR placement; then the hardware build (section 6); a monitor routine that reprograms the ROM through JP-WE (6.10 rules), with its emulator model.
