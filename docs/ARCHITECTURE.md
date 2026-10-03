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

This table and the equates in `rom/monitor.asm` (:50-66) MUST match. If they disagree, that is a defect and goes in `TODO.md`.

| Address | Size | Name | Initialized at boot |
|---------|------|------|---------------------|
| 0080-00CF | 80 | LINE_BUFFER (at most 79 characters plus a NUL) | no |
| 00D0-00D1 | 2 | free. The `BUFFER_PTR` equate (`monitor.asm:52`) is never referenced and is deleted (ROM change pending, TODO.md). | - |
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

- **Source:** `rom/monitor.asm`. **Image:** `rom/monitor.bin`, exactly 4096 bytes, assembled at `ORG 0F000H` (`monitor.asm:15`), unused bytes 0xFF.
- **Fixed address:** only one. COLD_START is the first byte of the image (0xF000, which is also 0x0000 through the overlay). Every other routine address can move from build to build.
- **No public entry points.** User programs MUST NOT call ROM routines by address. Programs do their own I/O through the ports in `DEVICE_SPECS.md`. ROM routine contracts are in `MONITOR_SPEC.md` (ROM Routine Contracts); they describe the code, not an ABI.
- **Budget:** 4096 bytes. Used bytes = `ROM_END - 0F000H`, where `ROM_END` is a label after the last assembled byte. `make size` prints that number. The padded image size is not a measurement. (Today `make size` prints the padded 4096, `rom/Makefile:34-35`; ROM change pending, TODO.md.)
- **Layout** (not normative): boot at F000, then MAIN_LOOP, console I/O and print routines, input and parse routines, the commands, the storage commands, then the strings.

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

**Emulator:** `reset()` (`cpu.rs:1101-1111`) produces this CPU state; it zeroes A-L, sets flags to 0x02 and SP to 0xF000, which are permitted choices. `Intel8080::new()` leaves the overlay clear (`cpu.rs:46`), so `reset()` MUST be called before running the ROM. Emulator RAM is 00 at power-on (`memory.rs:12`). Devices are created in their power-on state at process start, which is the emulator's only RESET. Any future host-side reset (Phase 10) MUST reset the devices as well as the CPU.

### 3.2 Boot Sequence

```
0x0000 (= F000 via overlay)
  COLD_START:    LXI  SP,0F000H
                 DI
                 JMP  BOOT_CONTINUE      ; absolute F0xx: leave the overlay mirror
  BOOT_CONTINUE: XRA  A
                 OUT  0FEH               ; clears the overlay flip-flop
                 ; init LAST_DUMP_ADDR, LAST_EXAM_ADDR, IO_IN_STUB, IO_OUT_STUB (table 1.1)
                 CALL PRINT_BANNER       ; banner text: MONITOR_SPEC.md
  WARM:          LXI  SP,0F000H
  MAIN_LOOP:     ...
```

Current code: `monitor.asm:72-109`. WARM does not exist yet, and CONOUT polls `IN 02H` before each `OUT 00H` (`monitor.asm:175-183`), so today the first Pi-window access is `IN 02H` (ROM changes pending, TODO.md).

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
- **Emulator:** a CPU with no ROM loaded treats all 64 KB as RAM, and the overlay has no effect. Current code drops writes to 0000-0FFF while the overlay is set (`cpu.rs:212`) and treats `OUT FE` with 00 as clear and FF as cold reset (`cpu.rs:760-766`); emulator change pending, TODO.md.

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

Reference vectors (each fails in the current code; TODO.md, Review findings):

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
- **Current code:** the interim timer (`src/io/devices/timer.rs`, port hooks at `cpu.rs:758-759`, `:775-776`, tick at `:947`) and `handle_interrupt` (`cpu.rs:290`, acceptance check at `:826`) are to be deleted and replaced by the above. Today EI has no delay, the acknowledge adds 0 cycles, and an interrupt does not end a halt. All are current bugs (emulator change pending, TODO.md).

### 5.8 HLT

- `HLT` costs 7 cycles, leaves PC at the next instruction, and sets the halt state.
- While halted, a step fetches nothing, changes no register or memory, and adds 4 cycles, unless an accepted interrupt (5.7) ends the halt.
- Test: `76 3E 01 76` followed by two steps leaves A unchanged and PC = 0001.
- `run()` returns when the CPU halts. The CPU core prints nothing (`perform_hlt` prints today, `cpu.rs:311`; emulator change pending, TODO.md). Host handling of the halt is in section 7.

---

## 6. Hardware Interface

The circuits the hardware build must contain. The software-visible behavior of every port is in `DEVICE_SPECS.md`. The hardware build itself is Someday; this section is the contract it must meet. Timing marked **[verify]** is checked against the 8080A, 8224 and 8228 datasheets in the hardware-alignment pass (TODO.md).

### 6.1 Clock and CPU Support

- CPU: 8080A at 2.048 MHz (18.432 MHz / 9 from the 8224; one T-state ≈ 488 ns, within the 8080A's 480 ns minimum tCY). Nominally "2 MHz". All timing analysis uses 488 ns.
- Clock and reset: 8224 with an 18.432 MHz crystal.
- System controller: 8228 (MEMR, MEMW, I/OR, I/OW strobes; INTA handling in 6.7).
- The ROM stays timing-independent (3.2, requirement 6).

### 6.2 Memory Decode

```
OVL     = overlay flip-flop Q (6.5)
ROM_OE  = MEMR AND ( A15..A12 = 1111  OR  ( OVL AND A15..A12 = 0000 ) )
RAM_OE  = MEMR AND NOT ROM_OE
RAM_WE  = MEMW
```

- RAM_WE has no address term. A write to F000-FFFF goes to RAM if RAM is fitted there and to no device otherwise. Either way it has no visible effect, because reads of F000-FFFF always select ROM (section 4).
- RAM is static and covers at least 0000-EFFF. It MUST keep its contents with no CPU activity for unlimited time (READY waits, RESET held), so DRAM that needs CPU-driven refresh is excluded.

### 6.3 Port Address Decode

| Ports | Decode | Served by |
|-------|--------|-----------|
| 0x00-0x6F | `A7 = 0 AND NOT (A6 AND A5 AND A4)` | Pi, behind READY (6.4). Every port in this range, assigned or not. |
| 0x70-0xFD | rest | local chips (none fitted). Nothing drives the bus on `IN`. |
| 0xFE (write), 0xFF (read) | full 8-bit compare | overlay glue (6.5) |

Port assignments and the values returned by unassigned and unmapped ports are in `DEVICE_SPECS.md` (Port Map, Rules Common to All Ports).

**Emulator:** the CPU model handles `OUT 0xFE` and `IN 0xFF` itself and never passes them to the IoBus. Every other access goes through the IoBus. `IoBus::map_port` MUST panic when given 0xFE or 0xFF. Test: mapping a device to 0xFE panics. (Today `map_port` accepts them silently, `io/bus.rs:16-18`; emulator change pending, TODO.md.)

### 6.4 Pi Window and READY

Every Pi-window access holds READY low until the Pi releases it. The software contract that results (one instruction is one access, no byte-level busy polling, no timeout) is in `DEVICE_SPECS.md` (READY Contract).

**WAIT flip-flop.** Its Q output is the Pi's REQ line; its inverted output drives the 8224's RDYIN (READY low while Q = 1).

1. **Set** asynchronously during T1 when SYNC is high, the status on the CPU-side data bus shows INP (D6) or OUT (D4), and A0-A7 decode into 0x00-0x6F. RESET dominates the set condition. The set path (status valid during SYNC, decode, flip-flop, 8224 RDYIN) MUST meet the 8224's RDYIN setup time for the clock edge on which READY is sampled in T2. **[verify]** The flip-flop MUST NOT be set from the 8228's I/OR or I/OW strobes, which arrive too late.
2. **Cleared** by the rising edge of ACK (clocked, D tied low; not level-sensitive), or asynchronously by RESET. A held ACK level can never block the next set. Any REQ high the Pi reads after its ACK rising edge is a new access.
3. **Every Pi-window cycle inserts at least one T_W**, whatever level ACK is at. The 8080 leaves T_W only after an ACK edge or RESET.
4. **No timeout.** A dead or absent Pi stalls the 8080 in T_W until RESET.

**Direction.** A status latch clocked by STSTB holds the INP and OUT bits for the cycle. They give the Pi its DIR input and steer the data path.

**OUT cycle.** A0-A7 and D0-D7 stay valid through T_W. The OUT data byte reaches the Pi through a 74LVC245 down-translator that is enabled only during a Pi-window OUT cycle. The Pi samples address and data, applies the write, then raises ACK.

**IN cycle.** The Pi computes the byte, drives it on its D0-D7 GPIOs into a 74HCT374 IN latch, clocks the latch with a rising edge on LATCH, returns its D0-D7 GPIOs to input, then raises ACK. Glue enables the latch's outputs onto the system data bus only while I/OR is active on a Pi-window INP cycle. Nothing the Pi drives reaches the 8080 data bus at any other time, and no line is driven from both sides at once: the Pi drives its D0-D7 GPIOs only between REQ (with DIR = IN) and ACK, and the data 74LVC245 is enabled only on Pi-window OUT cycles.

**Signals at the Pi (21 GPIO):**

| Signal | Count | Pi direction | Path |
|--------|-------|--------------|------|
| A0-A7 | 8 | in | 74LVC245 (5 V to 3.3 V) |
| D0-D7 | 8 | in on OUT cycles, out to the IN latch on IN cycles | in: 74LVC245; out: 74HCT374 inputs |
| DIR | 1 | in | 74LVC245 |
| REQ | 1 | in | 74LVC245 |
| RESET | 1 | in | 74LVC245 |
| LATCH | 1 | out | 74HCT input (accepts 3.3 V levels) |
| ACK | 1 | out | 74HCT input (accepts 3.3 V levels) |

The Pi's GPIO runs at 3.3 V and is not 5 V tolerant; every 5 V signal reaches it through a 74LVC245.

**Bring-up.** No power-on sequencing between the 8080 and the Pi is needed, provided that:
- ACK and LATCH have pull-downs on the 5 V side, so they are inactive while the Pi boots;
- no ACK edge reaches the WAIT flip-flop before the Pi's device service is running (ACK uses a GPIO whose boot-default pull is down);
- the Pi service checks the REQ level when it starts and serves any request already pending. It MUST NOT depend on seeing a REQ edge.

**One codebase.** One serviced REQ maps to exactly one `IoDevice::read` or `IoDevice::write` call. The emulator's IoBus and the Pi's GPIO front end call the same Rust device code.

### 6.5 Overlay Glue (0xFE, 0xFF)

One 74LS74 half. RESET presets it. I/OW ANDed with a full 8-bit decode of port 0xFE clears it; the data bus is not decoded. A tri-state gate puts Q onto D0 while I/OR is active for port 0xFF. Q is OVL in the memory decode (6.2). Port semantics: `DEVICE_SPECS.md` (System Control).

### 6.6 Reset

- Power-on reset and the reset button drive the 8224's RESIN. The 8224's RESET output drives the 8080's RESET, presets the overlay flip-flop, clears the WAIT flip-flop, and goes to the Pi's RESET GPIO.
- Clearing the WAIT flip-flop is required. Without it, a reset taken while the Pi is unresponsive leaves READY low, and the first opcode fetch at 0000 hangs.
- **The Pi on RESET:** it detects the RESET assertion as a GPIO edge event (so a reset pulse of any length is seen), drops any request it is serving without raising ACK, and returns every device to its power-on state (`DEVICE_SPECS.md` rule 2.8). It finishes that before it serves the next REQ; the 8080's first Pi-window access after reset waits under READY until then. This costs zero ROM bytes.
- A late ACK for a cycle cut off by reset is never raised, so it cannot release the first post-reset access with stale data.
- Nothing else resets the machine.

### 6.7 Other CPU Pins

- **INT:** v1 has no interrupt source, so INT is held inactive. The 8228 is wired for its single-level RST 7 feature (INTA input to +12 V through 1 kΩ), so the 8228 supplies `RST 7` on acknowledge and a future source needs no new wiring. The tick source (Pi GPIO or 8254) is decided when a consumer appears (Someday).
- **HOLD** is tied inactive. There is no DMA.

### 6.8 What Is Local and What Is the Pi

| Local (8080 board) | Pi coprocessor |
|--------------------|----------------|
| 8080A, 8224 clock and reset, 8228 system controller (6.1) | Console (a Pi FIFO device; the terminal connects to the Pi) |
| 4 KB ROM at F000, static RAM, memory decode (6.2) | Storage and Mount (SD card) |
| Overlay glue (6.5) | Service Mailbox |
| Status latch (INP, OUT at STSTB), window decode, WAIT flip-flop, IN latch, level translation (6.4) | Every other port in 0x00-0x6F |
| Reset circuit (6.6) | |

The console transport between the terminal and the Pi (UART with RTS/CTS, USB gadget serial, or TCP) is Pi configuration and is invisible to the 8080. Console behavior, including input flow control toward the terminal: `DEVICE_SPECS.md` (Console).

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
| Ctrl-E | none: reserved for the Phase 10 debugger hotkey; dropped until then |
| Printable ASCII (20-7E) | its byte |
| Non-ASCII character | its UTF-8 bytes, in order |
| Any other key (arrows, function keys, and so on) | none: dropped |

Test: scripted Ctrl-A delivers 01 at `IN 01`; scripted Esc delivers 1B; scripted "é" delivers C3 A9. Current code maps Ctrl-A to 61 and drops Tab and Esc (`console.rs:81-84`); emulator change pending, TODO.md.

On hardware the Pi passes every byte the terminal sends; the key map does not exist there. The ROM MUST NOT rely on receiving, or on not receiving, 03 or 05.

### 7.2 Run Loop

| Convenience | Contract |
|-------------|----------|
| **Host input pump** | The host run loop reads pending host keyboard input at least once every 10,000 executed steps and puts the mapped bytes (7.1) into the console input FIFO in arrival order. The key source is injectable, so tests can script it. (Today input is read only inside `IN 02`, `console.rs:43-53`; emulator change pending, TODO.md.) |
| **Ctrl-C quits** | When the pump reads Ctrl-C, nothing goes into the FIFO. The run loop returns a quit status, and `main.rs` restores the terminal mode and exits. This works whatever the 8080 is doing, including `JMP $`. Test: script Ctrl-C while the CPU runs `JMP $`; the run loop returns quit within 10,000 steps, and `IN 01` never returns 03. |
| **Halt** | `run()` returns a halted status when the CPU halts (5.8); v1 has no interrupt source that could wake it. `main.rs` prints `HLT at PC=xxxx` to host stdout, where xxxx is PC (the address after the HLT), restores the terminal, and exits. |
| **Debugger hotkey** (Phase 10) | Ctrl-E opens an emulator-side prompt. Designed in Phase 10. Any reset it offers resets the devices too (3.1). |
| **Host banner and exit lines** | "8080 Emulator", the build timestamp, and any exit message go to host stdout from `main.rs`, never from the CPU core or from a device the 8080 can see. |
| **Test harness** | Allowed only on the host side: `TestConsole` (scripted input and captured output); test `IoDevice`s mapped with `map_port`, for example one that records every port access; `load_program` (writes that bypass the ROM and the overlay, `cpu.rs:1086`); a CPU with no ROM loaded; `interrupt(rst)` (5.7); direct access to the registers, memory, `cycles`, `halted` and `interrupts_enabled`; and `trace`, `debug_state` and `disassemble_at`. |
| **No throttle** | The emulator runs at host speed. `cycles` counts T-states only. |

---

## 8. Later Phases (Placeholders)

- **Phase 6:** Service Mailbox device (ports 10-13), mailbox `TIME`, and the `T` command. Protocol: `DEVICE_SPECS.md` (Service Mailbox). Command: `MONITOR_SPEC.md`. No memory-map or circuit change.
- **Phases 7-9:** more mailbox commands (`ASM`/`DIS`, `GET`, `ASK`). No architecture change.
- **Phase 10:** the debugger, host-side (section 7).
- **Someday:** a periodic interrupt source (tick from a Pi GPIO or an 8254, decided when a consumer appears) and its ISR placement; then the hardware build (section 6).
