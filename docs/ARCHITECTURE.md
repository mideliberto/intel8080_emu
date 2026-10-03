# System Architecture

Normative. Covers the memory map, ROM organization, reset and boot, the ROM overlay, the CPU behavioral contract, the hardware circuits behind the I/O ports, and the host-side rules for the emulator (including the host key map).

The other two normative specs:
- `DEVICE_SPECS.md`: every port protocol, register values, power-on state of each device, and the READY contract as software sees it.
- `MONITOR_SPEC.md`: line input, argument grammar, every monitor command and message, the Intel HEX loader, the `G` return contract, and ROM routine contracts.

One fact, one home: this file links to those and does not restate them. "MUST" applies to both the emulator and the hardware unless a section says otherwise. Where the current code differs, the code is wrong; the fixes are tracked in `TODO.md` ("Decided, to implement" and "Review findings"). All decisions here were made by Mike, 2026-10-02 or earlier.

The rule behind every section: **the ROM sees only what real parts provide.** If the 8080 can observe a behavior, a period chip or the Pi coprocessor must be able to produce it. Emulator conveniences stay on the host side (section 7).

---

## 1. Memory Map

```
0x0000-0x007F   Unused (128 bytes)
0x0080-0x00FF   Monitor workspace (128 bytes)
0x0100-0xEEFF   User area (60,928 bytes)
0xEF00-0xEFFF   Monitor stack page (256 bytes, SP starts at 0xF000)
0xF000-0xFFFF   Monitor ROM (4,096 bytes)
```

| Range | Rule |
|-------|------|
| 0000-007F | Not used by the monitor and not initialized at boot, so its contents are undefined. There are no RST vectors and no API jump table. (What this means for programs started with `G`: `MONITOR_SPEC.md`, G Return Contract.) |
| 0080-00FF | Monitor workspace. Layout in 1.1. Initialized at cold boot only. A program that writes here can break monitor commands until the next reset. |
| 0100-EEFF | User programs and data. The monitor reads and writes this range only when a command tells it to. |
| EF00-EFFF | Monitor stack. Cold boot and WARM (3.2) both set SP to 0xF000, so the first push writes 0xEFFF and 0xEFFE. |
| F000-FFFF | ROM. Reads return ROM bytes. Writes have no effect the 8080 can observe. |

**Monitor-owned ranges.** 0000-00FF and EF00-FFFF belong to the monitor. The HEX loader rejects any record that touches either range (`MONITOR_SPEC.md`, Intel HEX Loader). The other monitor commands do not guard these ranges.

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
| 00EA-00FF | 22 | free | - |

24 bytes are free in total. New workspace goes into 00EA-00FF first.

The I/O stubs run from RAM as self-modifying code. Real hardware runs it the same way, so it is allowed.

---

## 2. ROM Organization

- **Source:** `rom/monitor.asm`. **Image:** `rom/monitor.bin`, exactly 4096 bytes, assembled at `ORG 0F000H`, unused bytes 0xFF.
- **Fixed address:** only one. COLD_START is the first byte of the image (0xF000, which is also 0x0000 through the overlay). Every other routine address can move from build to build.
- **No WARM vector.** WARM (3.2) has no fixed address. A program reaches it only through the `G` return contract (`MONITOR_SPEC.md`); anything else that wants the monitor back jumps to F000, a cold start (banner, workspace reset). The CP/M exerciser shim (`tests/exerciser.rs`) does that at 0000. (Decided 2026-10-03.)
- **No public entry points.** User programs MUST NOT call ROM routines by address. Programs do their own I/O through the ports in `DEVICE_SPECS.md`. ROM routine contracts are in `MONITOR_SPEC.md` (ROM Routine Contracts); they describe the code, not an ABI.
- **Budget:** 4096 bytes. Used bytes = `ROM_END - 0F000H`, where `ROM_END` is a label after the last assembled byte. `make size` prints that number. The padded image size is not a measurement.
- **Layout** (not normative): boot at F000, then WARM and MAIN_LOOP, the shared error exits, console I/O and print routines, input and parse routines, the commands, the storage commands, then the strings and ROM_END.

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
- `new()` starts A-L, SP and RAM at 00 and flags at 02; the monitor harness overwrites them with junk before boot. Devices are created in their power-on state at process start by `build_bus` (`src/io/mod.rs`), which is the emulator's only RESET. Any future host-side reset (Phase 10) MUST reset the devices as well as the CPU.

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
5. **WARM** sits directly before MAIN_LOOP. It sets SP to 0xF000 and does nothing else. `G` pushes WARM's address before it jumps. The program-facing return contract is in `MONITOR_SPEC.md` (G Return Contract).
6. The ROM contains no timing-dependent code (no calibrated delay loops). The emulator runs unthrottled, and the hardware clock (6.1) is a design target, not a ROM dependency.

---

## 4. ROM Overlay

| Condition | Read 0000-0FFF | Write 0000-0FFF | Read F000-FFFF | Write F000-FFFF |
|-----------|----------------|-----------------|----------------|-----------------|
| Overlay set (after reset) | ROM byte at the same offset | RAM (write-through) | ROM | no visible effect |
| Overlay clear | RAM | RAM | ROM | no visible effect |

- The ROM is selected on MEMR only (decode in 6.2). Writes always go to RAM, or have no visible effect at F000-FFFF.
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

The circuits the hardware build must contain. The software-visible behavior of every port is in `DEVICE_SPECS.md`. The hardware build itself is Someday; this section is the contract it must meet. Timing in this section was checked in the 2026-10 hardware-alignment pass against the MCS-80 User's Manual 98-153D (Oct 1977: 8080A p.6-3..6-5, 8224 p.6-21..6-25, 8228 p.6-32..6-36) and the TI/Nexperia 74HCT and 74LVC datasheets. Figures are datasheet worst case at tCY = 488.28 ns, with t = 0 at phi1 rising in T1, unless marked (est). Items marked **[bench]** can only be closed by measurement on the built board. Pins, levels and cycle timing of the 8080A, 8224 and 8228: `reference/8080_HARDWARE.md` (98-153B, Sep 1975 edition; its page numbers differ from 98-153D).

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
```

- OVL changes only during an I/O write (OUT FE) or RESET, never while MEMR is active.
- RAM covers all of 0000-FFFF. RAM_WE has no address term, so a write to F000-FFFF lands in the RAM under the ROM and is never read back: reads of F000-FFFF always select ROM (section 4).
- RAM is static. It MUST keep its contents with no CPU activity for unlimited time (READY waits, RESET held), so DRAM that needs CPU-driven refresh is excluded.
- **Pins.** Tie ROM /CE low and ROM /WE to VCC, so the 8080 can never write the ROM. ROM_OE drives only ROM /OE. An 8 KB ROM part has A12 tied low. RAM /CE comes from address bits only. RAM_OE drives RAM /OE and RAM_WE drives RAM /WE. No signal gated by MEMR may drive a /CE.
- **Read timing** (no memory wait states). Address is valid by 329 ns. MEMR arrives by 787 ns (DBIN 757 + 8228 tRR 30). Data must be on the system bus by 905 ns: tDS2 is 150 ns before phi2 of T3, less 8228 tRD 30. That gives 118 ns from MEMR to data and 576 ns from address to data. Memory /OE access plus the MEMR gate MUST fit in 118 ns. Timing a /CE access from MEMR misses the deadline (AT28C64B tCE 150).
- **Logic levels.** The 8228 drives the system data bus and MEMR/MEMW/I/OR/I/OW at TTL levels (VOH 2.4 V min at -1 mA, VOL 0.45 V). Every input on those nets MUST accept VIH <= 2.4 V: 74HCT/ACT, ATF22V10C, AT28C64B, 74LVC at 3.3 V, AS6C62256. Parts with CMOS thresholds MUST NOT be on those nets: 74HC, and AS6C1008/AS6C4008 (VIH 0.7 VCC). 8080A inputs need VIH 3.3 V. They are driven only by the 8224 (READY, RESET), the 8228 CPU-side D0-D7, and HCT outputs.
- **Bus loading.** Every load on the 8080A address pins and CPU-side data pins is CMOS, because 8080A IOL is 1.9 mA. There are no address or data buffers beyond the 8228. The status taps (D4, D6) are on the CPU side of the 8228.

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

1. **Set** asynchronously (PRE) while all of these hold: 8224 STSTB is low, RESET is inactive, the CPU-side status shows INP (D6) or OUT (D4), and the port decodes into 0x00-0x6F. Status and address are valid from 76 ns before STSTB falls (tDSS min 296 - tDD max 220) until phi2 of T2. SYNC alone MUST NOT qualify the set. SYNC has only maximum delays (tDC <= 120 ns, no minimum) against tDD <= 220 and tDA <= 200, and it falls up to 120 ns after phi2 of T2 while the bus changes to write data. A SYNC-gated set can therefore fire on a memory cycle. SYNC MAY be added as an extra term. The RESET term uses the same inverted RESET that drives the flip-flop's /CLR, for two reasons: the 8224 drives STSTB low during reset, and PRE and CLR low together give Q = /Q = 1. RDYIN MUST be low within 167 ns of STSTB falling (8224 tDRS = -167 ns) and stays low past STSTB + 217 ns (tDRH). STSTB is at least 40 ns wide, against a flip-flop PRE minimum of 20-24 ns. **[bench]** The STSTB width at PRE, after the gate path. The flip-flop MUST NOT be set from I/OR or I/OW. I/OW starts only in T_W.
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

The read-backs guarantee the 74HCT374 tsu 25 ns, th 10 ns and tw 20 ns. Back-to-back GPIO writes do not: on a Pi 4 they can land about 4 ns apart. The latch outputs drive the system data bus (DB0-DB7, never the 8080-side D0-D7) while I/OR is active and the port decodes into the window. The enable MUST NOT use Q or REQ, because the 8080 samples in T3, after ACK has cleared Q. No line is ever driven from both sides. The Pi drives D0-D7 only while REQ is high with DIR = IN, which lies inside I/OR active, when the data 74LVC245A is disabled. It releases them before ACK, while the 8080 is still frozen in T_W.

**Signals at the Pi (20 GPIO):**

| Signal | Count | Pi direction | Path | BCM GPIO |
|--------|-------|--------------|------|----------|
| A0-A6 | 7 | in | 74LVC245A (A7 is always 0 in the window) | 4-10 |
| D0-D7 | 8 | in on OUT cycles, out to the IN latch on IN cycles | in: 74LVC245A; out: 220-330 ohm series, then 74HCT374 inputs | 20-27 |
| DIR (/I/OR) | 1 | in | 74LVC245A | 11 |
| REQ (Q AND WAIT) | 1 | in | 74LVC245A | 12 |
| RESET | 1 | in | 74LVC245A | 13 |
| ACK | 1 | out | 74HCT14 Schmitt input | 16 |
| LATCH | 1 | out | 74HCT374 CLK (accepts 3.3 V) | 17 |

The Pi's GPIO runs at 3.3 V and is not 5 V tolerant. Every 5 V signal reaches it through a 74LVC245A powered from the Pi's 3V3 pin. Do not use a 74LVCH245A: its bus-hold fights the Pi on D0-D7. Every signal sits in GPLEV0, so one read is an atomic snapshot. D0-D7 sit in GPFSEL2, so turning the data lines around takes one register write.

**Power and boot independence.** The 8080 board and the Pi have separate supplies, grounds joined at the header, and may be powered in either order, provided that:
- every 74LVC245A runs from the Pi's 3V3, so an unpowered Pi leaves them in Ioff and is never back-powered;
- the Pi drives only ACK, LATCH and D0-D7 toward the board, and D0-D7 only between REQ and LATCH. At idle ACK and LATCH are low and D0-D7 are inputs with internal pull-downs, so the Pi never back-powers an unpowered 5 V board through 74HCT input clamps;
- ACK and LATCH have 4.7 kohm pull-downs on the 5 V side and use GPIOs whose boot-default pull is down (BCM 9-27, never 0-8 or 14/15). A 10 kohm pull-down against a Pi 4's 33 kohm minimum pull-up leaves only 33 mV of margin to HCT VIL;
- on startup, the service sets D0-D7 to input and drives ACK and LATCH low before anything else;
- the Pi service checks the REQ level when it starts and serves any request already pending. It MUST NOT depend on seeing a REQ edge.

**One codebase.** The emulator and the Pi daemon build the same IoBus with the same device mapping for 00-6F (one function). Each Pi-window access in the emulator, and each serviced REQ on the Pi, is exactly one IoBus read or write. Devices do only bounded local work in read and write (DEVICE_SPECS 3.3). Host I/O goes through Console push_input/take_output. RESET rebuilds the IoBus, so post-reset state equals power-on state by construction. Reference Pi service: one thread busy-polls the mmapped GPIO block (/dev/gpiomem) on an isolated core and calls the IoBus inline. RESET edges come from a gpio character-device edge request (6.6). Mailbox background work runs on other cores. The target Pi uses the BCM283x/2711 register model (Pi 4B for v1). A Pi 5 also works, but its GPIO reads cross PCIe to RP1 and are several times slower (est). Do not use rppal (archived 2025-07-01).

### 6.5 Overlay Glue (0xFE, 0xFF)

One 74HCT74 half; the other half is the WAIT flip-flop (6.4). /PRE = NOT RESET: the 8224 RESET is active high, and the same inverter drives the WAIT flip-flop's /CLR and the WAIT set term. /CLR = NOT (I/OW AND port = 0xFE), with a full 8-bit decode; the data bus is not decoded. A tri-state gate puts Q onto system DB0 while I/OR is active for port 0xFF. Q is OVL in the memory decode (6.2). Port semantics: `DEVICE_SPECS.md` (System Control).

### 6.6 Reset

- RESIN (8224 pin 2, active low, Schmitt input) is held low until every rail is in regulation and the -5 V and +12 V ramps are complete. The 8224 only synchronizes RESIN to phi2; it does not stretch it. The reset source therefore guarantees the 8080A's minimum of 3 clocks. Source and button: decision RESET-SOURCE (`HARDWARE_BUILD.md`, Decisions).
- The 8224 RESET output (pin 1, active high, VOH 3.6 V at -100 uA) has three loads, all CMOS inputs:
  - the 8080 RESET (pin 12), directly;
  - one 74HCT inverter, which drives the overlay flip-flop /PRE, the WAIT flip-flop /CLR and the WAIT set term (6.4);
  - the Pi's RESET GPIO, through a 74LVC245A.
- Clearing the WAIT flip-flop is required. Without it, a reset taken while the Pi is unresponsive leaves READY low, and the first opcode fetch at 0000 hangs.
- The 8224 also drives STSTB low during reset. The NOT RESET term in the WAIT set blocks it. **[bench]** REQ stays low across 100 consecutive resets.
- **The Pi on RESET.**
  - It requests RESET through the gpio character device with both-edge events. The kernel latches the edge, so a pulse of any length is seen, even during an fsync. It also reads the RESET level in every GPIO snapshot.
  - Immediately before every ACK, it checks for a latched or present RESET. If it finds one, it drops the request without raising ACK.
  - When RESET is next low, it returns every device to its power-on state (`DEVICE_SPECS.md` rule 2.8), and only then serves a REQ. The 8080's first Pi-window access after reset waits under READY until then. This costs zero ROM bytes.
  - A bouncing button can produce several RESET pulses. Each one is a full device reset.
- A late ACK for a cycle cut off by reset is never raised. One residual race is accepted: the bus thread is descheduled between its last RESET check and the ACK write for longer than the RESET pulse plus the boot path to the first Pi-window access (about 0.12 ms).
- Nothing else resets the machine, except the optional Pi TEST_RESET (decision TEST-RESET), which pulls the same RESIN node.

### 6.7 Other CPU Pins

- **INT** (pin 14): v1 has no interrupt source, so INT goes to GND through 10 kohm. The 8228 is wired for its single-level RST 7 feature: its INTA output (pin 23) is strapped to +12 V through 1 kohm (<= 5 mA, 8228 DC table). On acknowledge it then puts RST 7 (FF) on the 8080 bus during DBIN. The strap disables pin 23 as an INTA strobe, so a single-level source needs no new wiring and a vectored controller does. A future source (Pi GPIO tick or 8254, decided when a consumer appears, Someday) drives INT through a 74HCT gate. The 8080A VIH is 3.3 V, which neither a Pi GPIO (VOH 2.6-3.0 V) nor LS-TTL guarantees.
- **HOLD** (pin 13) to GND. 8080A HLDA (pin 21) goes to 8228 HLDA (pin 2). There is no DMA.
- **8228 BUSEN** (pin 22) to GND. A floating bipolar input reads high and tri-states the 8228.
- Any driver of an 8080A input MUST be a CMOS/HCT output. 74LS is not allowed there.

### 6.8 What Is Local and What Is the Pi

| Local (8080 board) | Pi coprocessor |
|--------------------|----------------|
| 8080A, 8224 clock and reset, 8228 system controller (6.1) | Console (a Pi FIFO device; the terminal connects to the Pi) |
| 4 KB ROM at F000, static RAM, memory decode (6.2) | Storage and Mount (SD card) |
| Overlay glue (6.5) | Service Mailbox |
| Window decode, WAIT flip-flop and REQ gate, IN latch, level translation (6.4) | Every other port in 0x00-0x6F |
| Reset circuit (6.6) | |

The console transport between the terminal and the Pi (UART with RTS/CTS, USB gadget serial, or TCP) is Pi configuration and is invisible to the 8080. Console behavior, including input flow control toward the terminal: `DEVICE_SPECS.md` (Console).

### 6.9 Power

- Datasheet maximum currents (8080A p.6-3, 8224 p.6-23, 8228 p.6-35):

  | Rail (all +/-5%) | Load |
  |------------------|------|
  | +5 V | 8080A 80 mA, 8224 115 mA, 8228 190 mA, plus memory and glue: about 0.8 A total (est) |
  | +12 V | 8080A 70 mA, 8224 12 mA, 8228 INTA strap 5 mA: 87 mA total |
  | -5 V | 8080A 1 mA |

- No 8080A pin or supply may go more than 0.3 V below VBB (8080A absolute maximum). VBB has a Schottky clamp to GND (anode VBB, cathode GND) at the CPU socket. **[bench]** At bring-up, scope VBB single-shot at power-up and power-down: it MUST stay at or below +0.3 V relative to GND.
- +12 V MUST never exceed 12.6 V, including at turn-on. The 8224 VDD absolute maximum is 13.5 V.
- If -5 V comes from a charge pump fed by +5 V, VBB tracks VCC. In that case +5 V MUST be held at 4.85-5.15 V at the board. How the rails are generated is decision POWER (`HARDWARE_BUILD.md`, Decisions).
- Decoupling: 0.1 uF per IC per rail, plus 10 uF bulk per rail.
- The Pi has its own supply (6.4, Power and boot independence).

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

Test: scripted Ctrl-A delivers 01 at `IN 01`; scripted Esc delivers 1B; scripted "é" delivers C3 A9; scripted Ctrl-E opens the prompt after the keys before it. Code: `map_key` in `src/main.rs`, tested there with the run loop.

On hardware the Pi passes every byte the terminal sends; the key map does not exist there. The ROM MUST NOT rely on receiving, or on not receiving, 03 or 05.

### 7.2 Run Loop

| Convenience | Contract |
|-------------|----------|
| **Host input pump** | The host run loop reads pending host keyboard input at least once every 10,000 executed steps and puts the mapped bytes (7.1) into the console input FIFO in arrival order. The key source is injectable, so tests can script it. Console output is drained to host stdout as often. When stdin is not a terminal, its bytes go to the FIFO unmapped and Ctrl-C is the shell's. Code: `run_loop` in `src/main.rs`. |
| **Idle wait** | A pump first takes what is pending without waiting. It then blocks for host input for up to 1 ms (a terminal poll, or a wait on the stdin reader) and takes what arrived only when all three hold: the first take brought nothing, the console input FIFO is empty, and the 8080 read `IN 02` since the previous pump (it is polling for input, as the monitor's CONIN does at the prompt). A compute-bound program does not read `IN 02`, so it never waits; a program that polls `IN 02` inside a compute loop does, about 1 ms per 10,000 steps. Measured 2026-10-03, release: idle at the monitor prompt (piped stdin at EOF) about 27% of a core (about 100% with no wait); a 26M-step `DCX B` loop run with `G` takes 0.20 s, the same as with no wait (the first trigger, without the `IN 02` condition, took 15x longer). The 8080 cannot see it: it adds no cycles. Code: `idle_waits` in `src/main.rs`, `Console::take_polled`. Tests: `idle_waits_only_when_nothing_came_the_fifo_is_empty_and_the_8080_polled`, `a_pump_waits_only_while_the_8080_polls_an_empty_fifo`, `a_compute_bound_program_never_waits` in `src/main.rs`; `console_host_side_sees_status_polls` in `tests/device_tests.rs`. |
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

Host-side only. The 8080 cannot observe it: it adds no cycles, no port, no memory and no byte to the console stream. Code: `src/debugger.rs` (commands, breaks, ring, trace), `src/disasm.rs` (disassembler), `src/main.rs` (entry and prompt). Tests: `tests/debugger_tests.rs`.

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
- **Instruction line:** `AAAA  B0 B1 B2  MNEMONIC` with the bytes field 8 wide. Intel mnemonics as in `docs/reference/Complete_Intel_8080_Instruction_Set_Reference.txt`, operands in hex (`MVI A,0D`, `LXI H,0080`, `IN 02`). An address operand (jumps, calls, `LDA`, `STA`, `LHLD`, `SHLD`) equal to a symbol prints as the name (`CALL SKIP_SPACES`); immediate data (`LXI`) always prints in hex. The undocumented aliases (5.4) print with a star: `NOP*`, `JMP*`, `RET*`, `CALL*`. Wherever an instruction line is listed (`u`, `s`, a stop report; not ring lines), an address that is a symbol is preceded by a `NAME:` line.
- **Ring line:** the instruction line padded with spaces to 34 characters, one space, then the registers before it ran: `A=44 F=56 BC=0B0D DE=0000 HL=0081 SP=F000`.
- **Stop report:** a reason line, then the last 8 ring lines (the last one is the instruction that caused a watchpoint or I/O stop), then the registers line, then the next instruction. The reason line always begins a line: when the console output before a stop does not end with LF, the host writes CR LF first. Reasons: `* break LOCATION`, `* watch read AAAA VV`, `* watch write AAAA VV`, `* io IN PP VV`, `* io OUT PP VV`, `* ctrl-e`, `* start`, `* halt`.

**Port trace.** One 7.3 line per `IN` or `OUT` transfer, ports FE and FF included. Repeats are collapsed by the 7.3 rule (`IN 02 02 ; x12`). Pending lines are written at every stop and at `t off` and quit, so a run split by a stop is written as two lines. The debugger writes no `RESET` line: it has no reset. Diffing against a Pi daemon trace (7.3): the Pi never sees ports 70-FF (`DEVICE_SPECS.md`), so drop the FE and FF lines, strip the ` ; xN` annotation, since poll counts depend on timing, then merge adjacent identical lines, since a stop splits a run (`grep -Ev '^(IN|OUT) F[EF] ' | sed 's/ ; x[0-9]*$//' | uniq`).

**Symbols.** `rom/monitor.sym` is built with `monitor.bin` by `cd rom && make` and committed with it: one `AAAA NAME` line per label in `monitor.asm`, from asl's NoICE output: the code labels and the workspace labels (1.1), so `w STOR_ADDR` works. EQU constants are left out: they are ports, characters and sizes. The emulator loads it from next to `monitor.bin` when present; without it a `NAME` argument is an error and output has no names.

**Not in v1:** reset, writing registers or memory, conditional breakpoints, expressions beyond `NAME+n`, a TUI.

---

## 8. Later Phases (Placeholders)

- **Phase 6:** Service Mailbox device (ports 10-13), mailbox `TIME`, and the `T` command. Protocol: `DEVICE_SPECS.md` (Service Mailbox). Command: `MONITOR_SPEC.md`. No memory-map or circuit change.
- **Phases 7-9:** more mailbox commands (`ASM`/`DIS`, `GET`, `ASK`). No architecture change.
- **Phase 10:** what is left after the debugger (7.4): the monitor's `R` command, which needs the `G` return contract to capture registers.
- **Someday:** a periodic interrupt source (tick from a Pi GPIO or an 8254, decided when a consumer appears) and its ISR placement; then the hardware build (section 6).
