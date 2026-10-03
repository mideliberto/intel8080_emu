# Intel 8080A, 8224, 8228: Hardware Reference

Hardware companion to `Complete_Intel_8080_Instruction_Set_Reference.txt`. That file covers opcodes, flags and per-instruction state counts. This file covers pins, levels, bus timing, the processor cycle, and the two support chips, as needed to build a real 8080A machine.

Everything here is restated from one source, in our own words. Every number and pin assignment was read from the scanned page image, not from the OCR text. Numbers marked **derived** are arithmetic on the source's figures and are not printed in the manual.

---

## 1. Source and Conventions

**Source:** Intel, *8080 Microcomputer Systems User's Manual*, September 1975, document 98-153B (number on the front cover, PDF p.1). The scan has 262 PDF pages. Scan: http://bitsavers.org/components/intel/MCS80/98-153B_Intel_8080_Microcomputer_Systems_Users_Manual_197509.pdf (not vendored, because it is copyrighted). PDF page numbers below refer to that scan.

**Citations:** `(MCS-80 UM p.<printed page> / PDF p.<n>)`. Printed pages are numbered per chapter, e.g. `2-8` or `5-16`.

**Chapters used:**

| Topic | Printed pages | PDF pages |
|-------|---------------|-----------|
| Intro (device list) | - | 2 |
| Ch.2: the 8080 CPU, processor cycle, interrupt, hold, halt, start-up | 2-1 to 2-15 | 15-29 |
| Ch.2: Table 2-3, machine cycles per instruction | 2-16 to 2-20 | 30-34 |
| Ch.4: EI, DI, HLT descriptions | 4-14 | 58 |
| Ch.3: discrete CPU module design | 3-2 to 3-7 | 36-41 |
| 8224 data sheet | 5-1 to 5-6 | 63-68 |
| 8228 data sheet | 5-7 to 5-12 | 69-74 |
| 8080A data sheet | 5-13 to 5-17 | 75-79 |
| 8080A-1 data sheet (A.C.) | 5-22 to 5-23 | 84-85 |
| 8080A-2 data sheet (A.C.) | 5-26 to 5-27 | 88-89 |

**8080 vs 8080A.** The manual introduces the 8080A as "functionally and electrically compatible" with the 8080. Its listed gains are TTL drive capability and enhanced timing (PDF p.2). The data sheet front page repeats the compatibility statement and the TTL drive capability (MCS-80 UM p.5-13 / PDF p.75). Chapters 2 and 3 are written about the "8080" and use the original part in their examples. The data sheets in chapter 5 cover only the 8080A family; the manual has no data sheet for the original 8080. **For hardware, the 8080A data sheet (5-13 to 5-17) is normative.** Where chapters 2 and 3 disagree with it, see section 14.

**Speed grades** (MCS-80 UM p.5-16, 5-22, 5-26 / PDF p.78, 84, 88):

| Part | tCY min | tCY max | Notes |
|------|---------|---------|-------|
| 8080A | 480 ns | 2.0 us | Standard part. This project: 488.28 ns (2.048 MHz). |
| 8080A-2 | 380 ns | 2.0 us | |
| 8080A-1 | 320 ns | 2.0 us | The data sheet cautions that near full speed, timing compatibility with the 8224 and 8228 must be checked. |
| M8080A | - | - | Military range, +-10 % supplies, PDF p.91-97. Not covered here. |

The A.C. parameters that differ between grades are in section 3.4.

**Signal names.** `/X` means active low (an overbar in the manual): `/WR`, `/STSTB`, `/MEMR`, `/RESIN`, `/BUSEN`, `/WO`. "phi1↑" means the rising edge of phi1. "State" means one clock period, from one phi1 rising edge to the next.

---

## 2. 8080A Pinout

(MCS-80 UM p.5-14 / PDF p.76; the same pinout for the 8080 is on p.2-1 / PDF p.15; connections confirmed against p.5-12 / PDF p.74)

40-pin DIP. In = input to the CPU, Out = output, 3S = three-state.

| Pin | Name | Dir | Function |
|-----|------|-----|----------|
| 1 | A10 | Out 3S | Address bit 10 |
| 2 | GND (VSS) | Pwr | 0 V reference |
| 3 | D4 | I/O 3S | Data bit 4 |
| 4 | D5 | I/O 3S | Data bit 5 |
| 5 | D6 | I/O 3S | Data bit 6 |
| 6 | D7 | I/O 3S | Data bit 7 (MSB) |
| 7 | D3 | I/O 3S | Data bit 3 |
| 8 | D2 | I/O 3S | Data bit 2 |
| 9 | D1 | I/O 3S | Data bit 1 |
| 10 | D0 | I/O 3S | Data bit 0 (LSB) |
| 11 | -5V (VBB) | Pwr | Substrate bias, -5 V +-5 % |
| 12 | RESET | In | Active high. Clears PC, INTE and HLDA (section 10). |
| 13 | HOLD | In | DMA request. Floats the address and data buses (section 8). |
| 14 | INT | In | Interrupt request, active high (section 7) |
| 15 | phi2 | In | Clock phase 2, MOS level (not TTL) |
| 16 | INTE | Out | State of the internal interrupt-enable flip-flop |
| 17 | DBIN | Out | High while the data bus is in input mode. External logic uses it to enable drivers onto the bus. |
| 18 | /WR | Out | Low while write data on D7-D0 is stable (memory write or OUT) |
| 19 | SYNC | Out | High during the first state of each machine cycle. Status is on D7-D0 during SYNC. |
| 20 | +5V (VCC) | Pwr | +5 V +-5 % |
| 21 | HLDA | Out | Hold acknowledge. The buses are released (or are about to be). |
| 22 | phi1 | In | Clock phase 1, MOS level |
| 23 | READY | In | High: memory or I/O data is ready. Low in T2 or TW: insert wait states. |
| 24 | WAIT | Out | High while the CPU is in a wait state (also high in halt) |
| 25 | A0 | Out 3S | Address bit 0 (LSB) |
| 26 | A1 | Out 3S | |
| 27 | A2 | Out 3S | |
| 28 | +12V (VDD) | Pwr | +12 V +-5 % |
| 29 | A3 | Out 3S | |
| 30 | A4 | Out 3S | |
| 31 | A5 | Out 3S | |
| 32 | A6 | Out 3S | |
| 33 | A7 | Out 3S | |
| 34 | A8 | Out 3S | |
| 35 | A9 | Out 3S | |
| 36 | A15 | Out 3S | Address bit 15 (MSB) |
| 37 | A12 | Out 3S | |
| 38 | A13 | Out 3S | |
| 39 | A14 | Out 3S | |
| 40 | A11 | Out 3S | |

**Power rails** (MCS-80 UM p.5-14, 5-15 / PDF p.76, 77): VDD = +12 V +-5 % (pin 28), VCC = +5 V +-5 % (pin 20), VBB = -5 V +-5 % (pin 11), VSS = 0 V (pin 2). phi1 and phi2 are not TTL-compatible; they need MOS-level drive, which the 8224 provides.

**Address bus.** A15-A0 carry a memory address, or an I/O port number for IN and OUT. During an I/O machine cycle, the 8-bit port number appears on both A7-A0 and A15-A8 (Table 2-3 note 18, MCS-80 UM p.2-20 / PDF p.34). The 8080A can address 256 input and 256 output ports (p.5-14).

---

## 3. Electrical Characteristics (8080A)

### 3.1 Absolute Maximum Ratings

(MCS-80 UM p.5-15 / PDF p.77)

| Item | Rating |
|------|--------|
| Temperature under bias | 0 to +70 °C |
| Storage temperature | -65 to +150 °C |
| Any input or output, with respect to VBB | -0.3 V to +20 V |
| VCC, VDD and VSS, with respect to VBB | -0.3 V to +20 V |
| Power dissipation | 1.5 W |

All voltage ratings are referenced to VBB. The manual gives no power-up sequencing rule (section 14).

### 3.2 D.C. Characteristics

(MCS-80 UM p.5-15 / PDF p.77). TA = 0 to 70 °C, VDD = +12 V +-5 %, VCC = +5 V +-5 %, VBB = -5 V +-5 %, VSS = 0 V.

| Symbol | Parameter | Min | Typ | Max | Unit | Condition |
|--------|-----------|-----|-----|-----|------|-----------|
| VILC | Clock input low | VSS-1 | | VSS+0.8 | V | |
| VIHC | Clock input high | 9.0 | | VDD+1 | V | |
| VIL | Input low | VSS-1 | | VSS+0.8 | V | |
| VIH | Input high | 3.3 | | VCC+1 | V | |
| VOL | Output low | | | 0.45 | V | IOL = 1.9 mA, all outputs |
| VOH | Output high | 3.7 | | | V | IOH = -150 uA |
| IDD(AV) | Avg current, VDD | | 40 | 70 | mA | Operating, tCY = 0.48 us |
| ICC(AV) | Avg current, VCC | | 60 | 80 | mA | Operating, tCY = 0.48 us |
| IBB(AV) | Avg current, VBB | | 0.01 | 1 | mA | Operating, tCY = 0.48 us |
| IIL | Input leakage | | | +-10 | uA | VSS <= VIN <= VCC |
| ICL | Clock leakage | | | +-10 | uA | VSS <= VCLOCK <= VDD |
| IDL | Data bus leakage, input mode (note 2) | | | -100 | uA | VSS <= VIN <= VSS+0.8 V |
| | | | | -2.0 | mA | VSS+0.8 V <= VIN <= VCC |
| IFL | Address and data bus leakage during HOLD | | | +10 | uA | VADDR/DATA = VCC |
| | | | | -100 | uA | VADDR/DATA = VSS+0.45 V |

**Capacitance** (TA = 25 °C, VCC = VDD = VSS = 0 V, VBB = -5 V):

| Symbol | Parameter | Typ | Max | Unit | Condition |
|--------|-----------|-----|-----|------|-----------|
| Cphi | Clock capacitance | 17 | 25 | pF | fc = 1 MHz |
| CIN | Input capacitance | 6 | 10 | pF | Unmeasured pins |
| COUT | Output capacitance | 10 | 20 | pF | returned to VSS |

**Notes to the D.C. table:**
1. RESET must be active for at least 3 clock cycles.
2. While DBIN is high and an input rises above VIH, the CPU switches an internal active pull-up onto that data line. This is why IDL reaches -2.0 mA above 0.8 V.
3. Supply current changes by -0.45 % per °C.

**What this means for the builder:**
- The data inputs need **VIH >= 3.3 V**, above a TTL VOH. The 8228's CPU-side outputs (VOH >= 3.6 V) and the 8224's READY and RESET outputs (VOH >= 3.6 V) are specified to meet it. Anything else that drives an 8080A input (INT, HOLD, or data without an 8228) must also reach 3.3 V.
- The outputs are weak: 1.9 mA sink and 150 uA source. Budgets for this build are in 13.4, item 7.

### 3.3 A.C. Characteristics (standard 8080A)

(MCS-80 UM p.5-16, 5-17 / PDF p.78, 79). TA = 0 to 70 °C, supplies as in 3.2. Timing reference levels: clock "1" = 8.0 V, "0" = 1.0 V; inputs "1" = 3.3 V, "0" = 0.8 V; outputs "1" = 2.0 V, "0" = 0.8 V.

| Symbol | Parameter | Min | Max | Unit | Load / note |
|--------|-----------|-----|-----|------|-------------|
| tCY | Clock period (note 3) | 0.48 | 2.0 | us | |
| tr, tf | Clock rise and fall time | 0 | 50 | ns | |
| tphi1 | phi1 pulse width | 60 | | ns | |
| tphi2 | phi2 pulse width | 220 | | ns | |
| tD1 | Delay phi1 to phi2 | 0 | | ns | |
| tD2 | Delay phi2 to phi1 | 70 | | ns | |
| tD3 | Delay phi1 to phi2 leading edges | 80 | | ns | |
| tDA | Address output delay from phi2 | | 200 | ns | CL = 100 pF |
| tDD | Data output delay from phi2 | | 220 | ns | CL = 100 pF |
| tDC | Signal output delay from phi1 or phi2 (SYNC, /WR, WAIT, HLDA) | | 120 | ns | CL = 50 pF |
| tDF | DBIN delay from phi2 | 25 | 140 | ns | CL = 50 pF |
| tDI | Delay for input bus to enter input mode (note 1) | | tDF | ns | |
| tDS1 | Data setup time during phi1 and DBIN | 30 | | ns | |
| tDS2 | Data setup time to phi2 during DBIN | 150 | | ns | |
| tDH | Data hold time from phi2 during DBIN (note 1) | see note 1 | | ns | |
| tIE | INTE output delay from phi2 | | 200 | ns | CL = 50 pF |
| tRS | READY setup time during phi2 | 120 | | ns | |
| tHS | HOLD setup time to phi2 | 140 | | ns | |
| tIS | INT setup time during phi2 (during phi1 in halt mode) | 120 | | ns | |
| tH | Hold time from phi2 (READY, INT, HOLD) | 0 | | ns | |
| tFD | Delay to float during hold (address and data bus) | | 120 | ns | |
| tAW | Address stable prior to /WR | note 5 | | ns | (a) |
| tDW | Output data stable prior to /WR | note 6 | | ns | (a) |
| tWD | Output data stable from /WR | note 7 | | ns | (a) |
| tWA | Address stable from /WR | note 7 | | ns | (a) |
| tHF | HLDA to float delay | note 8 | | ns | (a) |
| tWF | /WR to float delay | note 9 | | ns | (a) |
| tAH | Address hold time after DBIN during HLDA | -20 | | ns | (a) |

(a) CL = 100 pF for address and data; CL = 50 pF for /WR, HLDA and DBIN.

**Notes** (paraphrased from p.5-17):
1. Enable input data onto the bus using DBIN status; then there is no bus conflict and data hold is assured. tDH = 50 ns or tDF, whichever is less.
2. Load circuit: the output is loaded with CL plus a 150 uA current sink, and a diode network to +5 V through 2.1 kΩ (figure on p.5-17).
3. tCY = tD3 + trphi2 + tphi2 + tfphi2 + tD2 + trphi1 >= 480 ns.
4. When driving devices with VIH = 3.3 V: (a) maximum output rise time from 0.8 V to 3.3 V is 100 ns at CL = spec; (b) output delay measured to 3.0 V is the spec value + 60 ns at CL = spec; (c) if CL differs from spec, add 0.6 ns/pF above spec, or subtract 0.3 ns/pF below it, from the modified delay.
5. tAW = 2 tCY - tD3 - trphi2 - 140 ns.
6. tDW = tCY - tD3 - trphi2 - 170 ns.
7. Without HLDA: tWD = tWA = tD3 + trphi2 + 10 ns. With HLDA: tWD = tWA = tWF.
8. tHF = tD3 + trphi2 - 50 ns.
9. tWF = tD3 + trphi2 - 10 ns.
10. Input data must be stable for this window during DBIN in T3. Both tDS1 and tDS2 must be met.
11. READY must be stable for this window during T2 or TW. **READY must be externally synchronized.**
12. HOLD must be stable for this window during T2 or TW when entering hold, and during T3, T4, T5 and TWH while in hold mode. External synchronization is not required.
13. INT must be stable for this window in the last clock cycle of any instruction to be recognized on the following instruction. External synchronization is not required.
14. The waveform figure shows relationships only, not any particular machine cycle.

**Edge references in the waveform** (p.5-16, read from the image):
- tRS and tIS are drawn ending at the **falling** edge of phi2, with tH after that edge. Text on p.2-5 agrees for READY: it must precede the falling edge of phi2 by tRS.
- tHS is drawn ending at the **rising** edge of phi2. Text on p.2-13 agrees.
- tDA, tDD and tDF are measured from the rising edge of phi2.
- tDC for SYNC is measured from the rising edge of phi2. For /WR and WAIT it is measured from the rising edge of phi1 (see 4.3).

### 3.4 Speed-Grade Differences

(MCS-80 UM p.5-16/17, 5-22/23, 5-26/27 / PDF p.78/79, 84/85, 88/89). Values are in ns; "-" = same as the 8080A.

| Symbol | 8080A | 8080A-2 | 8080A-1 |
|--------|-------|---------|---------|
| tCY min | 480 | 380 | 320 |
| tr, tf max | 50 | - | 25 |
| tphi1 min | 60 | - | 50 |
| tphi2 min | 220 | 175 | 145 |
| tD2 min | 70 | - | 60 |
| tD3 min | 80 | 70 | 60 |
| tDA max | 200 | 175 | 150 (CL = 50 pF) |
| tDD max | 220 | 200 | 180 (CL = 50 pF) |
| tDC max | 120 | - | 110 |
| tDF min/max | 25/140 | - | 25/130 |
| tDS1 min | 30 | 20 | 10 |
| tDS2 min | 150 | 130 | 120 |
| tRS min | 120 | 90 | 90 |
| tHS min | 140 | 120 | 120 |
| tIS min | 120 | 100 | 100 |
| tAW formula constant | -140 | -130 | -110 |
| tDW formula constant | -170 | -170 | -150 |

### 3.5 At 2.048 MHz (this project)

With an 18.432 MHz crystal and the 8224, tCY = 9 / 18.432 MHz = **488.28 ns**. That is 8.28 ns (1.7 %) above the 8080A's 480 ns minimum. One oscillator period ("unit", 8224 terminology) is tCY/9 = **54.25 ns**. The manual uses exactly this case as its 8224 worked example (section 11.7).

Every 8080A clock requirement is met by the 8224 at 488.28 ns (8224 figures from MCS-80 UM p.5-6 / PDF p.68):

| 8080A requires | 8224 delivers at 488.28 ns |
|----------------|---------------------------|
| tCY >= 480 | 488.28 |
| tphi1 >= 60 | >= 89 |
| tphi2 >= 220 | >= 236 |
| tD1 >= 0 | >= 0 |
| tD2 >= 70 | >= 95 |
| tD3 >= 80 | 109 to 129 |
| tr, tf <= 50 | <= 20 |

**Derived** /WR-related minimums at 2.048 MHz. These use the formulas in 3.3 notes 5-9 with the 8224's tD3 = 108.5 to 128.5 ns, and trphi2 from 0 to 20 ns, taking the worst case each time:

| Symbol | Derived min |
|--------|-------------|
| tAW | 688 ns |
| tDW | 169 ns |
| tWD, tWA (no HLDA) | 118 ns |
| tHF | 58 ns |
| tWF | 98 ns |

---

## 4. The Processor Cycle

(MCS-80 UM p.2-3 to 2-10 / PDF p.17-24, Tables 2-1 and 2-2, Figures 2-3 to 2-7)

### 4.1 Terms

- **State:** one clock period, from one phi1↑ to the next. It is the smallest unit of activity. Every state, including TW, TWH and hold, is a whole number of clock periods.
- **Machine cycle:** one memory or I/O reference (DAD is the one exception, below). It lasts 3, 4 or 5 states, plus any wait states.
- **Instruction cycle:** 1 to 5 machine cycles, 4 to 18 states. The first machine cycle is always a FETCH (M1).
- The number of machine cycles equals the number of memory or I/O references the instruction makes. **Exception:** DAD has two extra machine cycles (M2, M3) that do an internal register-pair add. They make no memory reference, generate no SYNC, do not need READY, and accept HOLD (Table 2-3 note 8, p.2-20 / PDF p.34; Fig 2-3 footnote, p.2-3 / PDF p.17).

Per-opcode state counts are in `Complete_Intel_8080_Instruction_Set_Reference.txt`. The machine-cycle breakdown of each opcode (which M-cycles, which states) is in Table 2-3, MCS-80 UM p.2-16 to 2-20 / PDF p.30-34.

Hardware-relevant examples from Table 2-3:

| Instruction | Machine cycles |
|-------------|----------------|
| IN port | M1 fetch (4 states), M2 memory read of the port byte (3), M3 input (3). 10 states. The port byte is loaded into both W and Z, and WZ is output as the address in M3. That is why the port number appears on both A7-A0 and A15-A8. |
| OUT port | M1 fetch (4), M2 memory read (3), M3 output (3). 10 states. |
| RST n | M1 (5 states: T4-T5 decrement SP), M2 stack write of PCH, M3 stack write of PCL |
| CALL | M1 (5), M2 and M3 read the address bytes, M4 and M5 stack-write PCH and PCL |
| EI | M1 only. INTE is set in T4. |
| HLT | M1 (4 states), then an M2 that outputs PC and HLTA status, then the halt state (4.2, section 9) |

### 4.2 States

| State | What happens |
|-------|--------------|
| T1 | phi2↑ starts driving the address (memory address or I/O port) onto A15-A0, valid within tDA. Status is driven onto D7-D0, valid within tDD. SYNC goes high tDC after phi2↑. |
| T2 | phi2↑ removes status from D7-D0 and SYNC falls tDC later. In read-type cycles the bus switches to input and DBIN rises tDF after phi2↑. In write-type cycles output data replaces status within tDD. **READY and HOLD are sampled, and the CPU checks for a halt instruction.** |
| TW (optional) | Entered after T2, and repeated, while READY was low at the sample point. WAIT is high. Address, DBIN and /WR are held, and write data stays on the bus. |
| TWH | The halt state, entered after T2 of the machine cycle that follows HLT. WAIT is high and the buses float (Fig 2-11). |
| T3 | Data transfer. Fetch: the instruction byte is read in. Memory read and stack read: a data byte is read in. Interrupt: the forced instruction is read in. Memory write, stack write and output: the data byte is on the bus with /WR low. |
| T4, T5 (optional) | Internal operations only, used when the instruction needs them. The CPU skips one or both when it does not. A machine cycle can end after T3, T4 or T5, and the next state is the next machine cycle's T1. |
| Hold mode | Buses floated, HLDA high (section 8) |

The CPU state flow (Fig 2-4, p.2-7 / PDF p.21):

```
RESET -> T1 -> T2 --(READY and not HLTA)--> [HOLD? set internal hold FF] -> T3 -> (T4) -> (T5)
                 \--(not READY or HLTA)--> HLTA? yes -> TWH (halt)
                                                 no  -> TW (loops while not READY) -> back into the T2 exit path
end of machine cycle:
  internal hold FF set           -> hold mode until HOLD drops, then continue
  instruction finished and INT*INTE -> set internal INT FF -> next M1 is an interrupt cycle
  otherwise                      -> T1 of the next machine cycle
Figure footnotes: INTE FF is reset when the internal INT FF is set.
                  The internal INT FF is reset when the INTE FF is reset.
```

### 4.3 Signal Timing Within a Machine Cycle

(MCS-80 UM p.2-5, 2-8, 2-10 / PDF p.19, 22, 24; p.5-16 / PDF p.78)

| Signal | Asserted | Released |
|--------|----------|----------|
| A15-A0 | Valid tDA (<= 200 ns) after phi2↑ in T1 | Stable until the first phi2↑ after T3 (i.e. in T4 or the next T1) |
| D7-D0 status | Valid tDD (<= 220 ns) after phi2↑ in T1 | Removed at phi2↑ in T2 |
| SYNC | High tDC (<= 120 ns) after phi2↑ in T1 | Low tDC after phi2↑ in T2 |
| DBIN (read-type cycles) | High tDF (25-140 ns) after phi2↑ in T2 | Low tDF after phi2↑ in T3. Each TW extends it by one clock period. |
| D7-D0 write data | Valid tDD (<= 220 ns) after phi2↑ in T2 | Stable for the rest of the machine cycle, until replaced by status in the next T1 |
| /WR | Low tDC after phi1↑ of the first state after T2 (the first TW, or T3) | High tDC after phi1↑ of the state after T3. TW states lengthen it. |
| WAIT | High tDC after phi1↑ on entering TW | Low tDC after phi1↑ of T3 |

Read-type cycles (DBIN used): FETCH, MEMORY READ, STACK READ, INTERRUPT (p.2-8 / PDF p.22), and INPUT (INP status definition, p.2-6 / PDF p.20; Fig 2-6, p.2-9 / PDF p.23). Write-type cycles (/WR used): MEMORY WRITE, STACK WRITE, OUTPUT (p.2-10 / PDF p.24).

### 4.4 Machine-Cycle Types and the Status Word

The CPU outputs an 8-bit status word on D7-D0 during SYNC, in T1 of every machine cycle except DAD's internal M2 and M3, which have no SYNC (MCS-80 UM p.2-5, 2-6 / PDF p.19, 20; repeated on p.5-9 / PDF p.71).

| Bit | Name | Meaning when 1 (or 0 for /WO) |
|-----|------|-------------------------------|
| D0 | INTA | Interrupt acknowledge. Use it to gate the interrupt instruction onto the bus while DBIN is active. |
| D1 | /WO | 0 = this cycle writes (memory write or output). 1 = read (memory read or input). |
| D2 | STACK | The address bus holds the stack address from SP |
| D3 | HLTA | Acknowledge of a HLT instruction |
| D4 | OUT | The address bus holds an output port number. Output data is on the bus while /WR is low. |
| D5 | M1 | Fetch cycle for the first byte of an instruction |
| D6 | INP | The address bus holds an input port number. Put the input data on the bus while DBIN is high. |
| D7 | MEMR | The bus will carry memory read data |

The manual notes that INTA, INP and MEMR are the bits that control what drives data onto the 8080 bus.

Status words for the ten machine-cycle types (bit values from the chart; **hex values derived**):

| # | Machine cycle | D7 MEMR | D6 INP | D5 M1 | D4 OUT | D3 HLTA | D2 STACK | D1 /WO | D0 INTA | Hex | 8228 strobe |
|---|---------------|---|---|---|---|---|---|---|---|-----|-------------|
| 1 | Instruction fetch | 1 | 0 | 1 | 0 | 0 | 0 | 1 | 0 | A2 | /MEMR |
| 2 | Memory read | 1 | 0 | 0 | 0 | 0 | 0 | 1 | 0 | 82 | /MEMR |
| 3 | Memory write | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 00 | /MEMW |
| 4 | Stack read | 1 | 0 | 0 | 0 | 0 | 1 | 1 | 0 | 86 | /MEMR |
| 5 | Stack write | 0 | 0 | 0 | 0 | 0 | 1 | 0 | 0 | 04 | /MEMW |
| 6 | Input read | 0 | 1 | 0 | 0 | 0 | 0 | 1 | 0 | 42 | /I/OR |
| 7 | Output write | 0 | 0 | 0 | 1 | 0 | 0 | 0 | 0 | 10 | /I/OW |
| 8 | Interrupt acknowledge | 0 | 0 | 1 | 0 | 0 | 0 | 1 | 1 | 23 | /INTA |
| 9 | Halt acknowledge | 1 | 0 | 0 | 0 | 1 | 0 | 1 | 0 | 8A | none |
| 10 | Interrupt acknowledge while halted | 0 | 0 | 1 | 0 | 1 | 0 | 1 | 1 | 2B | /INTA |

Some cycles are identified by one bit (M1 marks a fetch). Others need a combination: a stack read is STACK plus MEMR.

### 4.5 Capturing Status

Status is valid on D7-D0 from tDD after phi2↑ in T1 until phi2↑ in T2. Three capture methods appear in the manual:

1. **8224 STSTB → 8228** (the standard method; MCS-80 UM p.5-3, 5-8 / PDF p.65, 70). The 8224 gates SYNC with an internal advanced phi1 (phi1A) to make /STSTB, a low pulse at the end of T1. The manual describes it as the earliest moment status is stable. The 8228 latches status on /STSTB. Timing is in 11.4 and 12.6.
2. **8212 latch strobed by SYNC and phi1** (the unnumbered "8080 Status Latch" drawing beside Table 2-1, p.2-6 / PDF p.20). The 8212's DS2 (pin 13) is SYNC, its /DS1 (pin 1) is the clock generator's inverted phi1 TTL output, and MD (pin 2) is tied to VCC. The latch is open while SYNC and phi1 coincide, which is phi1 of T2, and status appears on its outputs at T2 phi1↑.
3. **Discrete STSTB** (Fig 3-3 and 3-5, p.3-3, 3-4 / PDF p.37, 38). SYNC is NANDed with the advanced phi1A from a discrete clock generator, and the result strobes an 8212.

---

## 5. READY and WAIT

(MCS-80 UM p.2-5, 2-8, 2-10 / PDF p.19, 22, 24; p.5-14, 5-16, 5-17 / PDF p.76, 78, 79; Table 2-3 note 2, p.2-20 / PDF p.34)

**Sampling rule.** READY is sampled in T2 and in every TW. To take effect it must be stable from tRS (>= 120 ns, standard 8080A) before the falling edge of phi2 in that state until tH (>= 0 ns) after it.
- Low at the T2 sample: the next state is TW.
- Low at a TW sample: another TW follows.
- High at the sample: the next state is T3, starting at the next phi1↑.
- A wait is therefore always a whole number of clock periods.

**Synchronization.** The 8080A requires READY to be synchronized externally (note 11, p.5-17). The 8224 provides this (section 11.4). A raw asynchronous READY violates the spec.

**Indefinite wait.** The CPU stays in TW for as long as READY is low; there is no internal timeout (p.2-5). READY can also single-step the CPU (p.5-14). The CPU is dynamic, but the clocks keep running during TW, so a long wait is legal. The 2.0 us tCY maximum still applies.

**WAIT output.** WAIT goes high tDC after phi1↑ on entry to TW and low tDC after phi1↑ of T3. It acknowledges the wait state. WAIT is also high in the halt state (Fig 2-11). It is a status output; READY does not depend on it.

**During a wait:**
- The address stays valid until after T3.
- In read cycles DBIN stays high through all TW states.
- In write cycles /WR goes low at the start of the first TW and stays low. Output data stays stable.
- HOLD is still sampled. A HOLD that is recognized while READY is active, in T2 or TW, is acted on (section 8).

**Cycles that ignore READY.** DAD's internal M2 and M3 do not need READY (Table 2-3 note 8).

**Building a wait-state generator.** The manual shows no complete circuit; the following is **derived**. The manual's building blocks are:
- the 8224 RDYIN synchronizing flip-flop (5-3);
- the discrete equivalent, a 74S74 clocked from the clock generator (Fig 3-3, p.3-3 / PDF p.37);
- the remark that a slow memory needs "simple logic control of the READY input" to add one or more wait states (p.3-7 / PDF p.41).

The usual structure follows from the timing in 11.4: decode address and status, qualified by STSTB or SYNC, early in the cycle. Drive RDYIN low before the 8224 samples it in T2. Release RDYIN when the device (or a counter clocked by phi2 TTL or OSC) is done. Section 13 works the timing for this project.

---

## 6. DBIN, /WR and Data-Bus Direction

(MCS-80 UM p.2-8, 2-10 / PDF p.22, 24; p.5-14, 5-15, 5-17 / PDF p.76, 77, 79)

**Who drives D7-D0, state by state:**

| Period | Read-type cycle | Write-type cycle |
|--------|-----------------|------------------|
| T1 (from tDD after phi2↑) | CPU drives status | CPU drives status |
| T2 phi2↑ onward | CPU stops driving and enters input mode (tDI <= tDF). DBIN rises tDF after phi2↑. External drivers may drive while DBIN is high. | CPU replaces status with write data (tDD) and keeps driving |
| TW | DBIN high; external data may be driven | CPU drives write data; /WR low |
| T3 | Data latched by the CPU; it must meet tDS1 and tDS2. DBIN falls tDF after phi2↑. | /WR low; data stable |
| After T3 | External drivers must be off before the CPU drives status in the next T1 | /WR rises tDC after phi1↑ of the next state; data stays until the next T1 status |
| HOLD / HLDA | Bus floats | Bus floats |

**Input setup** (p.2-8): data must be stable tDS1 (>= 30 ns) before the falling edge of phi1 in T3, and tDS2 (>= 150 ns) before the rising edge of phi2 in T3. Both must be met. Hold is tDH = min(50 ns, tDF) after phi2↑ in T3.

**Rule** (note 1, p.5-17): gate external data onto the CPU bus with DBIN (and the status that names the cycle). Then the bus never has two drivers, and data hold is guaranteed. The 8228 does exactly this.

**Active pull-up.** While DBIN is high, each data input above VIH has an internal pull-up switched on (IDL up to 2 mA, 3.2). An external driver must sink that.

---

## 7. Interrupts

(MCS-80 UM p.2-11 / PDF p.25, Fig 2-8; p.2-7 / PDF p.21, Fig 2-4; p.5-14, 5-17 / PDF p.76, 79; p.4-14 / PDF p.58)

**INT input:**
- INT is active high and asynchronous; the CPU re-clocks it internally, so no external synchronizer is needed (note 13).
- To be recognized on the next instruction, INT must be stable from tIS (>= 120 ns) before the phi2 falling edge in the last clock cycle of the current instruction, until tH (0 ns) after it. In the halt state the setup is referenced to phi1 instead (tIS, "during phi1 in halt mode").
- A request that coincides with INTE high and phi2 sets an internal interrupt latch in the last state of the instruction cycle. The current instruction always finishes first.
- The CPU does not honor INT while it is in the HOLD state, or while INTE is reset (p.5-14).

**INTE flip-flop and pin:**
- EI sets INTE and DI resets it. Table 2-3 shows the flip-flop changing in T4 of the EI or DI M1. The INTE pin follows with tIE (<= 200 ns) from phi2.
- INTE resets automatically at T1 of the interrupt-acknowledge M1, so further interrupts are disabled. RESET also resets it.
- **EI delay:** interrupts become enabled only after the instruction that follows EI has executed (p.4-14 / PDF p.58). DI takes effect as soon as DI itself has executed.

**Interrupt-acknowledge machine cycle** (p.2-11):
1. It looks like an ordinary FETCH, with M1 set and SYNC as usual. Status also has INTA (D0) set: 23h normally, 2Bh when acknowledged from halt.
2. The PC is put on A15-A0 in T1 but **not incremented** (Fig 2-8: "inhibit store of PC+1"). The PC saved later is therefore the address of the instruction that was not yet fetched.
3. DBIN is active in T2-T3. External logic must place an instruction on the data bus in T3, meeting the T3 input setup in section 6. The memory must be kept off the bus during this cycle. With an 8228 this is automatic: it issues /INTA, not /MEMR.
4. The CPU executes whatever it reads.

**RST response:**
- RST n is the intended one-byte response: 11NNN111, a call to 8·n.
- Sequence: M1 interrupt-acknowledge cycle (5 states; T4-T5 decrement SP) → M2 stack write of PCH → M3 stack write of PCL (status 04h for both) → fetch from 8·n.
- Vectors: 00h, 08h, 10h, 18h, 20h, 28h, 30h, 38h.

**Multi-byte response:**
- Any instruction may be supplied. With an 8228, a CALL works: the 8228 produces an /INTA pulse for each of the three bytes (section 12.5).
- The manual does not say whether the PC increments during the CALL's operand reads in this case (section 14).

**Halt plus interrupt** (p.2-13, Fig 2-12, 2-14):
- INT with INTE set exits the halt state; T1 of the interrupt cycle starts at the next phi1↑. Status is 2Bh.
- INTE must be set before HLT is executed, or only RESET can leave halt.
- Fig 2-14 shows HOLD and INT inhibiting each other inside halt: a hold already in progress delays the interrupt, and an interrupt being taken delays a new hold.

---

## 8. HOLD and HLDA

(MCS-80 UM p.2-12, 2-13 / PDF p.26, 27, Fig 2-9, 2-10; p.5-14, 5-17 / PDF p.76, 79)

1. HOLD is synchronized internally; no external synchronizer is needed. It must be stable from tHS (>= 140 ns) before the phi2 rising edge, until tH after it.
2. HOLD is recognized in the halt state, or in T2 or TW when READY is active. HOLD, READY and phi2 together set the internal hold latch.
3. HLDA rises tDC after phi1↑:
   - of **T3**, for read and input cycles (Fig 2-9);
   - of **the state after T3**, for write and output cycles (Fig 2-10).
4. The address and data buses float tFD (<= 120 ns) after the next phi2↑. HLDA therefore slightly precedes the actual float.
5. If the current machine cycle needs T4 and T5, the CPU runs them internally after floating the buses. It stops at the end of the machine cycle.
6. HOLD is removed asynchronously. HLDA falls after the next phi1↑, and the CPU continues with the next machine cycle.
7. RESET clears the HLDA flip-flop.
8. The data sheet gives the HLDA-to-float, /WR-to-float and address-hold-after-DBIN figures (tHF, tWF, tAH) in 3.3.

This project ties HOLD inactive.

---

## 9. HALT

(MCS-80 UM p.2-13, 2-14, 2-15 / PDF p.27, 28, 29, Fig 2-11, 2-12, 2-14; Table 2-3 note 20, p.2-20 / PDF p.34)

**Entry.** HLT is fetched in M1 (4 states). The next machine cycle (M2) outputs the PC with status 8Ah (HLTA). After its T2 the CPU enters TWH, the halt state. In TWH:
- address and data float;
- WAIT is high;
- no further SYNC occurs.

**Exits:**

| Event | Result |
|-------|--------|
| RESET | Always exits. Goes to T1 with PC = 0. |
| INT with INTE = 1 | Exits to T1 at the next phi1↑. Takes the interrupt cycle (status 2Bh). INT setup in halt is referenced to phi1. |
| HOLD | Enters hold mode. When HOLD drops, the CPU **returns to halt** at the next phi1↑. HOLD is not an exit. |

If INTE = 0 at HLT, only RESET leaves the halt state.

---

## 10. RESET and Start-Up

(MCS-80 UM p.2-13 / PDF p.27; Fig 2-13, p.2-15 / PDF p.29; p.5-14, 5-15 / PDF p.76, 77)

| Item | Detail |
|------|--------|
| Polarity | Active high |
| Minimum width | **3 clock periods** (note 1, p.5-15; Fig 2-13 footnote). Clocks must be running. |
| Cleared | Program counter (to 0000h); INTE flip-flop (interrupts disabled); HLDA flip-flop |
| Not cleared | Flags, accumulator, stack pointer, and B, C, D, E, H, L. Undefined after power-up, and left unchanged by RESET. |
| Control outputs | Go inactive "immediately or some clock periods later" once RESET is active (Fig 2-13 footnote) |
| Buses during reset | Fig 2-13 shows A15-A0 and D7-D0 floating |
| Release | Per Fig 2-13, the internal reset follows RESET at clock boundaries. One state after the internal reset ends, T1 of M1 starts with PC = 0 on the address bus, then SYNC and the normal fetch. |
| Halt | RESET always exits the halt state to T1 (p.2-13) |

The manual adds that a system can put EI, HLT in locations 0 and 1 to wait for a start-up interrupt. Otherwise execution simply starts at 0.

---

## 11. 8224 Clock Generator and Driver

(MCS-80 UM p.5-1 to 5-6 / PDF p.63-68). Schottky bipolar, 16-pin DIP.

### 11.1 Pinout

(p.5-1 / PDF p.63; connections confirmed on p.5-3 and 5-12 / PDF p.65, 74)

| Pin | Name | Dir | Function |
|-----|------|-----|----------|
| 1 | RESET | Out | Synchronized reset to 8080A pin 12, active high |
| 2 | /RESIN | In | Reset input, active low, Schmitt trigger. Connect the RC power-on network and/or a switch to GND here. |
| 3 | RDYIN | In | Asynchronous ready input |
| 4 | READY | Out | Synchronized READY to 8080A pin 23 |
| 5 | SYNC | In | From 8080A pin 19 |
| 6 | phi2 (TTL) | Out | TTL-level copy of phi2, for external timing (e.g. DMA) |
| 7 | /STSTB | Out | Status strobe, active low, to 8228 pin 1 |
| 8 | GND | Pwr | 0 V |
| 9 | VDD | Pwr | +12 V |
| 10 | phi2 | Out | MOS-level phi2 to 8080A pin 15 |
| 11 | phi1 | Out | MOS-level phi1 to 8080A pin 22 |
| 12 | OSC | Out | Buffered oscillator output (crystal frequency) |
| 13 | TANK | In | LC tank, needed only for overtone crystals |
| 14 | XTAL2 | In | Crystal |
| 15 | XTAL1 | In | Crystal |
| 16 | VCC | Pwr | +5 V |

### 11.2 Oscillator and Clock Generation

(p.5-2, 5-3, 5-4 / PDF p.64, 65, 66)

- **Frequency.** The crystal is series-resonant, fundamental mode. The oscillator runs at 9 times the CPU clock: f_xtal = 9 / tCY.
  - Manual's examples: 500 ns → 18 MHz; 800 ns → 11.25 MHz.
  - This project: **18.432 MHz → tCY = 488.28 ns (2.048 MHz)**.
- **Crystals above 10 MHz** may need a small trim capacitor (3-10 pF) in series with the crystal.
- **Overtone crystals** use the TANK input, with an LC network (F = 1 / (2π·√(LC))) AC-coupled to ground. With TANK, run the crystal in 3rd-overtone mode.
- **Clock generator.** A synchronous divide-by-9 counter with decode gating. Both phases come from a 2-5-2 pattern of oscillator periods ("units"; 1 unit = 1/f_xtal):

```
unit:    |0 1|2 3 4 5 6|7 8|0 1|2 ...
phi1:    |###|_________|___|###|__
phi2:    |___|#########|___|___|##
         <2u>  <-5u->   <2u>
phi1 high 2 units, phi2 high 5 units, phi2-fall to next phi1-rise 2 units.
phi1-fall to phi2-rise is nominally 0 (tD1 min 0).
```

Manual's example at tCY = 500 ns (OSC = 18 MHz, 55 ns units): phi1 = 110 ns, phi2 = 275 ns, phi2-to-phi1 = 110 ns.

- **Drivers.** The phi1 and phi2 outputs are high-level drivers that connect directly to the 8080A. **They have no short-circuit protection** (D.C. table note 1, p.5-4 / PDF p.66). phi2 (TTL) is a TTL-level copy of phi2.

**Crystal requirements** (p.5-4 / PDF p.66):

| Item | Requirement |
|------|-------------|
| Tolerance | 0.005 % over 0-70 °C |
| Resonance | Series, fundamental (3rd overtone when used with the tank circuit) |
| Load capacitance | 20-35 pF |
| Equivalent resistance | "75-20 ohms" (as printed) |
| Power dissipation (min) | 4 mW |

### 11.3 STSTB

(p.5-3 / PDF p.65)

/STSTB is SYNC gated with the internal advanced phi1 (phi1A). It is a low pulse of about one unit (tPW) at the end of T1, starting tDSS after phi2↑ in T1:
- at most 6 units after T1 phi2↑ (tDSS = 6tCY/9 - 30 ns to 6tCY/9);
- one unit before T2 phi1↑.

It connects directly to the 8228.

**Reset also forces /STSTB low.** The text says power-on reset generates /STSTB "for a longer period" (p.5-3). The block diagram (p.5-1 / PDF p.63) ORs the RESET output into the /STSTB gate, so /STSTB is low for as long as RESET is high, whether from power-on or a /RESIN switch. That resets the 8228 without a dedicated pin.

### 11.4 Ready Synchronizer (RDYIN → READY)

(p.5-3, 5-5, 5-6 / PDF p.65, 67, 68)

- RDYIN is a "wait request" input. It goes to the D input of a D flip-flop clocked by the internal phi2D, and the Q output is READY. READY is therefore synchronous with the CPU clock and at the right level for the 8080A.
- The manual puts the flip-flop in bipolar logic outside the CPU because MOS delays inside the CPU would cost the designer about 200 ns of decision time.
- **Sampling, as specified.** tDRS and tDRH are referenced to the /STSTB falling edge in T1; tDR is referenced to the phi2 falling edge:
  - RDYIN setup tDRS has a minimum of 50 ns - 4·tCY/9. This is **negative** at normal speeds.
  - RDYIN hold tDRH is 4·tCY/9.
  - READY out changes at least tDR (4·tCY/9 - 25 ns) before the phi2 falling edge, where the 8080A samples it.
- **At tCY = 488.28 ns** (the manual's own example column):
  - tDRS = -167 ns and tDRH = 217 ns, so **RDYIN must be stable from 167 ns to 217 ns after /STSTB falls**;
  - READY then reaches the CPU at least 192 ns before the T2 phi2↓. The 8080A needs only tRS = 120 ns.
- **Derived:**
  - The capture point sits about 4 units after /STSTB↓, which is about 1 unit after phi2↑ of the state being sampled.
  - The flip-flop is clocked every clock period, so in each TW RDYIN is sampled at the same phase of the state. There is no STSTB in TW to reference it to.
  - From phi1↑ of T2 (or of any TW), the worst-case envelope combines the earliest setup deadline with the latest hold. **RDYIN must not change from about 83 ns to about 183 ns after phi1↑.** These figures come from tD3 = 108.5-128.5, tDSS = 296-326, tDRS and tDRH.

### 11.5 Reset Synchronizer (/RESIN → RESET)

(p.5-3 / PDF p.65)

- An external RC network on /RESIN converts the slow power-supply rise into a slow input edge. The internal Schmitt trigger turns it into a clean edge at its threshold.
- The Schmitt output feeds a D flip-flop clocked by phi2D. Its active-high output, RESET, meets the 8080A's input spec, and its timing (tDR) is the same as READY's.
- For a manual reset, a switch to GND on /RESIN is added alongside the power-on RC.
- The manual gives **no RC component values**, so the 3-clock minimum (section 10) is the only constraint it states.

### 11.6 D.C. Characteristics

(p.5-4 / PDF p.66). TA = 0 to 70 °C, VCC = +5.0 V +-5 %, VDD = +12 V +-5 %.

| Symbol | Parameter | Min | Max | Unit | Condition |
|--------|-----------|-----|-----|------|-----------|
| IF | Input current loading | | -0.25 | mA | VF = 0.45 V |
| IR | Input leakage | | 10 | uA | VR = 5.25 V |
| VC | Input forward clamp voltage | | 1.0 | V | IC = -5 mA |
| VIL | Input low | | 0.8 | V | VCC = 5.0 V |
| VIH | Input high, reset input | 2.6 | | V | |
| VIH | Input high, all other inputs | 2.0 | | V | |
| VIH-VIL | /RESIN input hysteresis (printed ".25 mV", label "REDIN") | 0.25 | | (see 14) | VCC = 5.0 V |
| VOL | Output low: phi1, phi2, READY, RESET, /STSTB | | 0.45 | V | IOL = 2.5 mA |
| VOL | Output low: all other outputs | | 0.45 | V | IOL = 15 mA |
| VOH | Output high: phi1, phi2 | 9.4 | | V | IOH = -100 uA |
| VOH | Output high: READY, RESET | 3.6 | | V | IOH = -100 uA |
| VOH | Output high: all other outputs | 2.4 | | V | IOH = -1 mA |
| ISC | Output short-circuit current (low-voltage outputs only) | -10 | -60 | mA | VO = 0 V, VCC = 5.0 V |
| ICC | Supply current, VCC | | 115 | mA | |
| IDD | Supply current, VDD | | 12 | mA | |

### 11.7 A.C. Characteristics

(p.5-5, 5-6 / PDF p.67, 68). VCC = +5.0 V +-5 %, VDD = +12.0 V +-5 %, TA = 0 to 70 °C. The general formulas are printed on p.5-5. The 488.28 ns column is the manual's own example table on p.5-6, rounded as printed.

| Symbol | Parameter | Min (formula) | Max (formula) | Min @ 488.28 | Max @ 488.28 | Condition |
|--------|-----------|---------------|---------------|--------------|--------------|-----------|
| tphi1 | phi1 pulse width | 2tCY/9 - 20 | | 89 | | CL = 20-50 pF |
| tphi2 | phi2 pulse width | 5tCY/9 - 35 | | 236 | | CL = 20-50 pF |
| tD1 | phi1 to phi2 delay | 0 | | 0 | | CL = 20-50 pF |
| tD2 | phi2 to phi1 delay | 2tCY/9 - 14 | | 95 | | CL = 20-50 pF |
| tD3 | phi1 to phi2 leading-edge delay | 2tCY/9 | 2tCY/9 + 20 | 109 | 129 | CL = 20-50 pF |
| tR | phi1, phi2 rise time | | 20 | | 20 | CL = 20-50 pF |
| tF | phi1, phi2 fall time | | 20 | | 20 | CL = 20-50 pF |
| tDphi2 | phi2 to phi2 (TTL) delay | -5 | +15 | -5 | +15 | phi2 TTL: CL = 30 pF, R1 = 300 Ω, R2 = 600 Ω |
| tDSS | phi2 to /STSTB delay | 6tCY/9 - 30 | 6tCY/9 | 296 | 326 | /STSTB: CL = 15 pF, R1 = 2 kΩ, R2 = 4 kΩ |
| tPW | /STSTB pulse width | tCY/9 - 15 | | 40 | | as above |
| tDRS | RDYIN setup time to /STSTB | 50 ns - 4tCY/9 | | -167 | | as above |
| tDRH | RDYIN hold time after /STSTB | 4tCY/9 | | 217 | | as above |
| tDR | RDYIN or /RESIN to phi2 delay (p.5-6 labels it READY or RESET to phi2) | 4tCY/9 - 25 | | 192 | | READY and RESET: CL = 10 pF, R1 = 2 kΩ, R2 = 4 kΩ |
| tCLK | CLK period | (typ) tCY/9 | | (typ) 54.25, derived | | |
| fmax | Maximum oscillating frequency | 27 MHz | | | 18.432 MHz (example) | |
| Cin | Input capacitance | | 8 pF | | | VCC = 5 V, VDD = 12 V, VBIAS = 2.5 V, f = 1 MHz |

Measurement points: phi1 and phi2 at "0" = 1.0 V and "1" = 8.0 V; all other signals at 1.5 V. The test circuit is R1 to VCC and R2 to GND on the output node, with CL to GND.

Edge references, read from the p.5-6 waveform:
- tDSS runs from phi2↑ to the /STSTB falling edge.
- tDRS and tDRH run from the RDYIN (or /RESIN) transition to the /STSTB falling edge.
- tDR runs from the READY (or RESET) output transition to the phi2 falling edge.

---

## 12. 8228 System Controller and Bus Driver

(MCS-80 UM p.5-7 to 5-12 / PDF p.69-74). Schottky bipolar, 28-pin DIP.

### 12.1 Pinout

(p.5-7 / PDF p.69, read from a 220 dpi crop; connections confirmed on p.5-9 and 5-12 / PDF p.71, 74)

| Pin | Name | Dir | Function |
|-----|------|-----|----------|
| 1 | /STSTB | In | Status strobe from 8224 pin 7 |
| 2 | HLDA | In | From 8080A pin 21 |
| 3 | /WR | In | From 8080A pin 18 |
| 4 | DBIN | In | From 8080A pin 17 |
| 5 | DB4 | I/O | System data bus bit 4 |
| 6 | D4 | I/O | CPU data bus bit 4 (8080A pin 3) |
| 7 | DB7 | I/O | System bit 7 |
| 8 | D7 | I/O | CPU bit 7 (8080A pin 6) |
| 9 | DB3 | I/O | System bit 3 |
| 10 | D3 | I/O | CPU bit 3 (8080A pin 7) |
| 11 | DB2 | I/O | System bit 2 |
| 12 | D2 | I/O | CPU bit 2 (8080A pin 8) |
| 13 | DB0 | I/O | System bit 0 |
| 14 | GND | Pwr | 0 V |
| 15 | D0 | I/O | CPU bit 0 (8080A pin 10) |
| 16 | DB1 | I/O | System bit 1 |
| 17 | D1 | I/O | CPU bit 1 (8080A pin 9) |
| 18 | DB5 | I/O | System bit 5 |
| 19 | D5 | I/O | CPU bit 5 (8080A pin 4) |
| 20 | DB6 | I/O | System bit 6 |
| 21 | D6 | I/O | CPU bit 6 (8080A pin 5) |
| 22 | /BUSEN | In | Bus enable, active low, asynchronous |
| 23 | /INTA | Out | Interrupt acknowledge strobe. Also the RST 7 option pin (12.5). |
| 24 | /MEMR | Out | Memory read |
| 25 | /I/OR | Out | I/O read |
| 26 | /MEMW | Out | Memory write |
| 27 | /I/OW | Out | I/O write |
| 28 | VCC | Pwr | +5 V |

### 12.2 Functional Blocks

(p.5-8 / PDF p.70)

- **Bidirectional bus driver.** 8 bits, between the CPU-side D7-D0 and the system-side DB7-DB0.
  - The CPU side meets the 8080A's 3.3 V VIH; the D0-D7 outputs have VOH >= 3.6 V. It also stays within the CPU's 1.9 mA drive.
  - The system side gives roughly 10 mA (typical) of drive.
  - The gating array sets the direction. /BUSEN can force the driver to high impedance for DMA.
- **Status latch.** Loads the status byte from D7-D0 while /STSTB is low, at the start of every machine cycle. Its outputs feed the gating array.
- **Gating array.** Combines the latched status with DBIN, /WR and HLDA to make the five active-low control strobes. The text describes them this way:
  - the read strobes (/MEMR, /I/OR, /INTA) come from status bits combined with DBIN;
  - the write strobes (/MEMW, /I/OW) come from status bits combined with /WR.
  - The waveform timing is in 12.4; it does not fully agree with this description for the read strobes (section 14).

### 12.3 Control Outputs per Machine Cycle

(status chart p.5-9 / PDF p.71)

| Cycle (status) | Strobe |
|----------------|--------|
| Fetch (A2), memory read (82), stack read (86) | /MEMR |
| Memory write (00), stack write (04) | /MEMW |
| Input read (42) | /I/OR |
| Output write (10) | /I/OW |
| Interrupt acknowledge (23), interrupt acknowledge while halted (2B) | /INTA |
| Halt acknowledge (8A) | none |

### 12.4 Strobe Timing

(waveform and A.C. table p.5-10 / PDF p.72, read from a 250 dpi crop)

- **/MEMR, /I/OR, /INTA:**
  - As drawn, they go low **tDC (20-60 ns) after the falling edge of /STSTB**. That is the T1/T2 boundary, before DBIN rises.
  - They go high tRR (<= 30 ns) after DBIN falls in T3.
  - During a hold acknowledge, the strobe instead goes high tHD (<= 25 ns) after HLDA rises.
- **/MEMW, /I/OW:** low tWR (5-45 ns) after /WR falls, and high tWR after /WR rises.
- **CPU-side bus during reads:** the 8228 starts driving D7-D0 tRE (<= 45 ns) after DBIN rises and stops tRE after DBIN falls. System-bus data reaches the CPU side within tRD (<= 30 ns).
- **System-side bus during writes:** the 8228 starts driving DB7-DB0 tWE (<= 30 ns) after /STSTB (the waveform measures it from the /STSTB rising edge). CPU data appears on DB within tWD (5-40 ns).
- **/BUSEN:** DB7-DB0 enter or leave high impedance within tE (<= 30 ns).

### 12.5 RST 7 Feature and Multi-Byte Interrupt Instructions

(p.5-7, 5-8, 5-11 / PDF p.69, 70, 73)

**Single-level RST 7:**
- **Wiring:** connect the /INTA output (pin 23) to the **+12 V supply through a series 1 kΩ resistor**. The test circuit on p.5-11 shows 1 kΩ +-10 %. IINT is at most 5 mA.
- **Operation:** the 8228 senses the voltage on pin 23 internally. In an interrupt-acknowledge cycle it then gates a **RST 7** instruction (opcode FFh, a call to 0038h) onto the bus while DBIN is active.
- No interrupt instruction port is needed. The manual does not say whether pin 23, pulled up to +12 V through the resistor, still works as an /INTA strobe in this mode; treat it as unusable (derived).
- The text says "onto the bus". The CPU-side D7-D0 must carry it, since that is what the 8080A reads.

**Multi-byte CALL:**
- When CALL is the interrupt instruction, the 8228 generates **one /INTA pulse for each of the three bytes**. An external interrupt port can therefore supply opcode, low address and high address, each on its own /INTA.
- The manual says this allows an unlimited number of interrupt levels.
- It does not say how the 8228 recognizes the CALL.

**/BUSEN:**
- Asynchronous. 1 = the data-bus output buffers **and the control-signal outputs** go to high impedance. 0 = normal operation.

### 12.6 A.C. Characteristics

(p.5-10 / PDF p.72). TA = 0 to 70 °C, VCC = 5 V +-5 %.

| Symbol | Parameter | Min | Max | Unit | Condition |
|--------|-----------|-----|-----|------|-----------|
| tPW | Width of status strobe | 22 | | ns | |
| tSS | Setup time, status inputs D0-D7 (to /STSTB falling edge, as drawn) | 8 | | ns | |
| tSH | Hold time, status inputs D0-D7 (after /STSTB rising edge, as drawn) | 5 | | ns | |
| tDC | Delay from /STSTB to any control signal | 20 | 60 | ns | CL = 100 pF |
| tRR | Delay from DBIN to control outputs | | 30 | ns | CL = 100 pF |
| tRE | Delay from DBIN to enable/disable 8080 bus | | 45 | ns | CL = 25 pF |
| tRD | Delay from system bus to 8080 bus during read | | 30 | ns | CL = 25 pF |
| tWR | Delay from /WR to control outputs | 5 | 45 | ns | CL = 100 pF |
| tWE | Delay to enable system bus DB0-DB7 after /STSTB | | 30 | ns | CL = 100 pF |
| tWD | Delay from 8080 bus D0-D7 to system bus DB0-DB7 during write | 5 | 40 | ns | CL = 100 pF |
| tE | Delay from system bus enable to system bus DB0-DB7 | | 30 | ns | CL = 100 pF |
| tHD | HLDA to read status outputs | | 25 | ns | |
| tDS | Setup time, system bus inputs to HLDA | 10 | | ns | |
| tDH | Hold time, system bus inputs to HLDA | 20 | | ns | CL = 100 pF |

Measurement points (p.5-10): D0-D7, when they are outputs, at "0" = 0.8 V and "1" = 3.0 V; all others at 1.5 V. Test loads (note 2 and test circuit, p.5-11 / PDF p.73): R1 to VCC, R2 to GND, CL to GND; D0-D7 use R1 = 4 kΩ, R2 = ∞, CL = 25 pF; other outputs use R1 = 500 Ω, R2 = 1 kΩ, CL = 100 pF.

### 12.7 D.C. Characteristics

(p.5-11 / PDF p.73). TA = 0 to 70 °C, VCC = 5 V +-5 %. Typicals are at 25 °C and nominal supply.

| Symbol | Parameter | Min | Typ | Max | Unit | Condition |
|--------|-----------|-----|-----|-----|------|-----------|
| VC | Input clamp voltage, all inputs | | 0.75 | -1.0 | V | VCC = 4.75 V, IC = -5 mA |
| IF | Input load current, /STSTB | | | 500 | uA | VCC = 5.25 V, VF = 0.45 V |
| IF | Input load current, D2 and D6 | | | 750 | uA | as above |
| IF | Input load current, D0, D1, D4, D5, D7 | | | 250 | uA | as above |
| IF | Input load current, all other inputs | | | 250 | uA | as above |
| IR | Input leakage, /STSTB | | | 100 | uA | VCC = 5.25 V, VR = 5.25 V |
| IR | Input leakage, DB0-DB7 | | | 20 | uA | as above |
| IR | Input leakage, all other inputs | | | 100 | uA | as above |
| VTH | Input threshold voltage, all inputs | 0.8 | | 2.0 | V | VCC = 5 V |
| ICC | Power supply current | | 140 | 190 | mA | VCC = 5.25 V |
| VOL | Output low, D0-D7 | | | 0.45 | V | VCC = 4.75 V, IOL = 2 mA |
| VOL | Output low, all other outputs | | | 0.45 | V | IOL = 10 mA |
| VOH | Output high, D0-D7 | 3.6 | 3.8 | | V | VCC = 4.75 V, IOH = -10 uA |
| VOH | Output high, all other outputs | 2.4 | | | V | IOH = -1 mA |
| IOS | Short-circuit current, all outputs | 15 | | 90 | mA | VCC = 5 V |
| IO(off) | Off-state output current, all control outputs | | | 100 | uA | VCC = 5.25 V, VO = 5.25 V |
| | | | | -100 | uA | VO = 0.45 V |
| IINT | /INTA current (RST 7 circuit) | | | 5 | mA | 1 kΩ to +12 V |

**Capacitance** (sampled, not 100 % tested; VBIAS = 2.5 V, VCC = 5.0 V, TA = 25 °C, f = 1 MHz):

| Symbol | Parameter | Typ | Max | Unit |
|--------|-----------|-----|-----|------|
| CIN | Input capacitance | 8 | 12 | pF |
| COUT | Output capacitance, control signals | 7 | 15 | pF |
| I/O | I/O capacitance (D or DB) | 8 | 15 | pF |

### 12.8 Standard Interconnect

(p.5-12 / PDF p.74)

| From | To |
|------|-----|
| 8224 phi1 (11) | 8080A phi1 (22) |
| 8224 phi2 (10) | 8080A phi2 (15) |
| 8224 READY (4) | 8080A READY (23) |
| 8224 RESET (1) | 8080A RESET (12) |
| 8080A SYNC (19) | 8224 SYNC (5) |
| 8224 /STSTB (7) | 8228 /STSTB (1) |
| 8080A DBIN (17) | 8228 DBIN (4) |
| 8080A /WR (18) | 8228 /WR (3) |
| 8080A HLDA (21) | 8228 HLDA (2) |
| 8080A D0-D7 | 8228 D0-D7, pins as in 12.1 |

- The 8080A drives A15-A0 directly; the address bus is unbuffered in this figure.
- Inputs: RDYIN, /RESIN, HOLD (DMA request), INT and /BUSEN.
- Outputs: INTE, WAIT (8080A pin 24, drawn with no destination), OSC and phi2 (TTL).
- The crystal goes on 8224 pins 14 and 15.

---

## 13. Worked I/O Cycles at 2.048 MHz

All numbers in this section are **derived** from the tables above (8080A p.5-16/17, 8224 p.5-5/6, 8228 p.5-10). Times are in ns from phi1↑ of M3 T1 (t = 0) of an IN or OUT instruction (the M3 I/O cycle, 4.1).

- tCY = 488.28; one unit u = 54.25.
- phi2↑ in any state falls 108.5-128.5 after that state's phi1↑ (tD3).
- Where a range is given it is the spec envelope; "nom" is the nominal 2-5-2 pattern.

### 13.1 Clock-Level Sketch (one TW)

Each character is one oscillator unit (54.25 ns); each state is 9 units. `#` = high, `_` = low, `x` = may change within this window, `=` = valid/driven, `.` = floating or don't-care.

```
state      T1        T2        TW        T3        next T1
unit       012345678 012345678 012345678 012345678 012345678
phi1       ##_______ ##_______ ##_______ ##_______ ##_______
phi2       __#####__ __#####__ __#####__ __#####__ __#####__
SYNC       __xx##### ##xx_____ _________ _________ __xx#####
/STSTB     ########_ ######### ######### ######### ########_
A15-A0     xxxxxx=== ========= ========= ========= ==xxxxxx.   port number on A7-0 and A15-8
D7-D0 (IN) xxxxxx=== ==xx..... ......=== ===xx....             status 42h; input valid by ~72 ns before T3
D7-D0 (OUT)xxxxxx=== ==xxxx=== ========= ========= ==xxxx===   status 10h, then A; held to next T1
DBIN (IN)  _________ ___xx#### ######### ###xx____ _________
/WR (OUT)  ######### ######### xx_______ _________ xx#######
WAIT       _________ _________ xx####### xx_______ _________
RDYIN      ......... ..^^..... ..^^..... ..^^.....             ^ = must be stable (sample window)
READY@CPU  ......... .....vv.. .....vv.. .........             v = 8080A sample (tRS before phi2 falling)
```

The sketch does not reproduce a manual figure. Edges are placed by nominal timing and drawn to the nearest unit.

### 13.2 IN (M3: Input, Status 42h), Pi Window Port, One or More TW

| t (ns) | State | Event | Source |
|--------|-------|-------|--------|
| 0 | T1 | phi1↑ | |
| 108.5-128.5 | T1 | phi2↑ | tD3 |
| <= 248.5 | T1 | SYNC↑ | tDC <= 120 after phi2↑ |
| <= 328.5 | T1 | A7-A0 = A15-A8 = port number, valid | tDA <= 200 |
| <= 348.5 | T1 | D7-D0 = 42h (INP, /WO = 1), valid | tDD <= 220 |
| 404-454 (nom 434) | T1 | /STSTB↓. Status latched (8228 tSS 8). The project's STSTB status latch can capture INP here. | tDSS 296-326 after phi2↑ |
| 424-514 | T1/T2 | 8228 /I/OR↓, per the 8228 waveform (but see section 14) | 8228 tDC 20-60 after /STSTB↓ |
| ~444-508 | T1/T2 | /STSTB↑ (width >= 40) | tPW |
| 488.3 | T2 | phi1↑ | |
| 597-617 | T2 | phi2↑. CPU removes status and enters input mode. | |
| **<= 571 worst / ~601 nom** | **T2** | **RDYIN must be valid (low, to wait) by /STSTB↓ + 167 and held to /STSTB↓ + 217** | **tDRS, tDRH** |
| 622-757 | T2 | DBIN↑. 8228 drives the CPU bus within 45 ns. | tDF; 8228 tRE |
| <= 737 | T2 | SYNC↓ | tDC |
| ~868 nom | T2 | phi2↓: 8080A samples READY. The 8224 put READY there >= 192 ns earlier. | tRS 120 |
| 976.6 | TW | phi1↑, first TW | |
| <= 1096.6 | TW | WAIT↑ | tDC |
| each TW | TW | RDYIN sampled at the same phase. To release, RDYIN must be high and stable by ~83 ns (worst) / ~113 ns (nom) after this TW's phi1↑. | 11.4 |
| T3 start - 72 | last TW | **Input byte must be on the system bus (DB)**: CPU tDS2 = 150 before T3 phi2↑ (earliest 108.5 into T3), plus 8228 tRD 30. tDS1 (30 before T3 phi1↓) is less strict. | tDS1, tDS2, tRD |
| T3 + 0-120 | T3 | WAIT↓ | tDC |
| T3 + 133-269 | T3 | DBIN↓ (tDF after phi2↑). /I/OR↑ within 30 ns. The 8228 releases the CPU bus within 45 ns. Data hold min(50, tDF) after phi2↑. | tDF, tRR, tRE, tDH |
| T3 + 488 | next M1 T1 | IN has no T4/T5 in M3 | Table 2-3 |

**Latency from RDYIN release to T3.**
- RDYIN high before the ~83 ns point of a TW: T3 starts at the end of that TW, 405 ns or more later.
- RDYIN high just after that point: one more TW runs. T3 starts up to ~893 ns later.
- Add the WAIT flip-flop's clear-path delay to both.

### 13.3 OUT (M3: Output, Status 10h), Pi Window Port

| t (ns) | State | Event | Source |
|--------|-------|-------|--------|
| 0-454 | T1 | As IN, with status 10h (OUT, /WO = 0) | |
| ~444-538 | T1/T2 | 8228 enables DB7-DB0 toward the system bus within 30 ns after /STSTB↑ (the bus carries whatever the CPU side holds) | tWE (from /STSTB↑ at ~444-508) |
| **<= 571 worst** | **T2** | **RDYIN deadline, same as IN** | |
| 597-617 | T2 | phi2↑. CPU replaces status with A. | |
| **<= 837** | **T2** | **OUT data (A) valid on CPU D7-D0** | **tDD <= 220** |
| **<= 877** | **T2** | **OUT data valid on system DB7-DB0** | **8228 tWD <= 40** |
| ~868 nom | T2 | phi2↓: READY sampled | |
| 976.6 | TW | phi1↑, first TW | |
| <= 1096.6 | TW | /WR↓; WAIT↑ | tDC |
| <= 1141.6 | TW | **8228 /I/OW↓** | tWR 5-45 |
| ... | TW | Address and data stay stable through every TW | p.2-10 |
| T3 | T3 | WAIT↓; /WR stays low | |
| next T1 + 0-120 | next M1 T1 | /WR↑; /I/OW↑ within 45 ns. Data and address stay valid at least tWD/tWA (>= ~118 derived) after /WR↑, then the next status replaces them. | tDC, tWR, tWD |

### 13.4 Consequences for ARCHITECTURE.md 6.4

These are derived observations for Mike, not decisions. They cover only what this source shows.

1. **Set path budget, the "[verify]" item.** The deadline is RDYIN valid by /STSTB↓ + 167 ns.
   - Status and address are valid at most 220 ns and 200 ns after T1 phi2↑. /STSTB↓ is at least 296 ns after that same edge.
   - From status valid to the RDYIN deadline: 296 + 167 - 220 = **243 ns** minimum. From address valid: 263 ns.
   - Measured from /STSTB↓ itself: **167 ns**.
   - So decode, flip-flop and the 8224 RDYIN input must settle within 243 ns of status valid. If the set is qualified by /STSTB, they must settle within 167 ns of /STSTB↓.
2. **Qualifying the set with SYNC level is not hazard-free by the data sheet.**
   - SYNC rises up to 120 ns after T1 phi2↑, but status and address may take up to 220 ns and 200 ns. For up to ~100 ns, SYNC can be high while D7-D0 still hold the previous cycle's contents (e.g. a written byte with D4 = 1) and the address is stale or settling.
   - In T2, status leaves the bus tDD after phi2↑ while SYNC falls tDC after phi2↑. Neither has a specified minimum, so the bus can change while SYNC is still high.
   - An asynchronous set gated by SYNC level can therefore fire spuriously, for example on a memory write whose data has D4 = 1 and whose address low byte is in 00-6F.
   - Qualifying the set with /STSTB low avoids this. That is the window the 8224/8228 use, with status valid at least 76 ns before /STSTB↓, and it still leaves 167 ns.
3. **/I/OR vs "arrives too late".** ARCHITECTURE says the 8228 strobes are too late to set the flip-flop.
   - For **/I/OW** this holds. It follows /WR, which falls in the first TW (~1097-1142 ns), long after the RDYIN deadline (~571 ns).
   - For **/I/OR** the 8228 waveform shows assertion 20-60 ns after /STSTB↓, which would be early enough. But the 8228 text says the read strobes are derived from DBIN, and DBIN rises after the deadline (~622-757 ns).
   - The manual's own discrete equivalent (Fig 3-5, p.3-4 / PDF p.38) gates the read strobes with DBIN, which sides with the text.
   - Section 14 lists the conflict. Not depending on /I/OR for the set, as ARCHITECTURE already does, is safe under either reading.
4. **OUT data is not valid when REQ rises.**
   - REQ can rise in T1 (~400-450 ns with STSTB; earlier with SYNC). The accumulator is not on the CPU bus until <= 837 ns, or on DB until <= 877 ns, plus the 74LVC245 delay.
   - Until then D7-D0 carry status (10h) or are in transition.
   - The Pi must not sample OUT data within ~0.5 us of REQ, or DIR/REQ for OUT must be qualified later (e.g. by /WR or /I/OW).
   - This is not in ARCHITECTURE 6.4 today.
5. **IN data has ample margin.** The Pi loads the 74HCT374 before raising ACK, and the latch output is enabled under /I/OR, so DB is valid before RDYIN rises. The 72 ns-before-T3 requirement is met if latch OE plus propagation delay settles at least 72 ns before the release TW ends. /I/OR is active by T2 under either reading of the 8228 (at most ~757 ns plus gate delay), and T3 is at least ~1465 ns in, so the margin is large.
6. **/STSTB is held low throughout RESET** (11.3). A status latch clocked by /STSTB sees a rising edge when reset ends. At that moment the bus may be floating or undefined, so INP and OUT in that latch may be junk until the first M1's /STSTB, a few clock periods later (section 10: internal reset release, one idle state, then T1). ARCHITECTURE gates its data paths with the window decode and REQ; the latch alone should not enable a driver.
7. **Loading on the CPU-side bus and address bus (6.4 taps both):**
   - The 8080A sinks 1.9 mA and sources 150 uA.
   - The 8228 already takes up to 750 uA (IF) on D2 and D6, and up to 100 uA (IR) high-side on its D inputs.
   - What remains on D4 and D6: ~1.15-1.65 mA low, ~50 uA high. That allows about two LS loads, or many HCT/CMOS loads.
   - A15-A0 are unbuffered in the standard interface. Every decoder, memory and translator input hangs directly on the CPU, so CMOS/HCT loads (or an address buffer) are needed for the planned fan-out. The 3.3 V VIH applies only to CPU inputs, not to these.

---

## 14. Errata, Discrepancies and Unresolved Points

**Errors printed in the source:**
1. **8224 hysteresis line** (p.5-4 / PDF p.66). It is printed as "REDIN Input Hysteresis, min .25, unit mV". The label presumably means /RESIN, and 0.25 mV is implausible for a Schmitt input. 0.25 V is likely, but the scan says mV. **Unresolved; do not rely on it.**
2. **8224 example A.C. table header** (p.5-6 / PDF p.68) reads "VDD = +5V +-5%; VDD = +12V +-5%". The first should be VCC. Typo.
3. **8228 interface figures** (p.5-9 / PDF p.71 and p.5-12 / PDF p.74) label the 8080A pin 21 signal "HDLA". It is HLDA.
4. **8228 VC row** (p.5-11 / PDF p.73) prints typ 0.75 and max -1.0 V, with inconsistent signs. Transcribed as printed.
5. **8224 crystal "Equivalent Resistance: 75-20 ohms"** (p.5-4) is transcribed as printed; the scan does not say what the range means. Unresolved.
6. **M1 bit position in the text** (p.2-5 / PDF p.19) says M1 is D6. The status charts (p.2-6 / PDF p.20, p.5-9 / PDF p.71) put M1 on D5, as does the 8228. The charts are right.

**Places where the manual contradicts itself:**

7. **8228 read-strobe derivation.** The text (p.5-8) says /MEMR, /I/OR and /INTA come from status combined with DBIN. The waveform (p.5-10) draws them going low tDC (20-60 ns) after the /STSTB falling edge, before DBIN rises, and ending on DBIN falling. The A.C. table on the same page defines tDC as the delay from /STSTB to any control signal, which agrees with the waveform. Against that, the discrete equivalent in Fig 3-5 (p.3-4 / PDF p.38) gates each read-status bit with DBIN, as the p.5-8 text describes. The source cannot settle which the 8228 silicon does. This matters only if a design relies on /I/OR arriving early.
8. **Clock levels and drive, chapter 3 vs data sheet.**
   - Ch.3 (p.3-3 / PDF p.37) says the clocks swing from 0.6 V (VILC) to 11 V (VIHC) into 20 pF maximum.
   - The 8080A data sheet (p.5-15) gives VILC max VSS+0.8 V, VIHC min 9.0 V, and Cphi 17 pF typical, 25 pF maximum.
   - Ch.3 describes the original 8080 and a discrete design. Use the data sheet.
9. **Data bus IOL.** Ch.3 p.3-4 (PDF p.38) gives the 8080 data bus drive as 1.7 mA maximum. P.3-5 (PDF p.39) gives the 8080A output drive as 1.9 mA, matching the 8080A data sheet. The wording suggests 1.7 mA is the original 8080 and 1.9 mA the 8080A, but the manual never says so outright.
10. **HLT length.**
   - Ch.4 (p.4-14 / PDF p.58) lists HLT as 1 machine cycle, 7 states.
   - Table 2-3 (p.2-18) and Fig 2-11 (p.2-14) show M1 (4 states) followed by an M2 (T1, T2 with HLTA status) and then TWH.
   - Neither count matches cleanly. M1 (4 states) plus M2's T1 and T2 is 6 states; Ch.4's 7 is reached only by counting the first TWH, and its "1 cycle" ignores M2. The hardware view is the Table 2-3 / Fig 2-11 one: a second machine cycle with SYNC and status 8Ah does occur.
11. **"Three ways to exit halt"** (p.2-13) lists HOLD, but HOLD returns the CPU to halt afterwards (p.2-13, Table 2-3 note 20). HOLD is not a real exit.
12. **EI timing.** The INTE flip-flop, and so the INTE pin, is set in T4 of EI (Table 2-3), but interrupts are accepted only after the instruction that follows EI (p.4-14). The INTE pin is therefore not a reliable "interrupts are now accepted" indicator for one instruction.

**Points the manual does not cover:**

13. **8080 vs 8080A differences.** The manual says only that the 8080A is compatible and adds TTL drive capability and enhanced timing (PDF p.2). It contains no 8080 data sheet and no list of differences. Nothing more can be concluded from this source.
14. **Power-supply sequencing.** No rule is given. The absolute maximum ratings are referenced to VBB (p.5-15). Unresolved here; take it from another source.
15. **8224 power-on RC values.** None are given. The only requirement stated is RESET >= 3 clocks.
16. **8224 TANK pin with a fundamental crystal.** The manual says TANK is "used only for overtone crystals". The standard-interface figure (p.5-12 / PDF p.74) brings pin 13 out as a labeled TANK input line, drawn like RDYIN and /RESIN, with nothing attached. Neither says outright to leave it open with a fundamental crystal.
17. **Interrupt with a CALL.** The manual does not say whether the PC is incremented during the CALL's M2 and M3 operand reads in an interrupt cycle; it states the inhibit only for M1 (Fig 2-8). It also does not say how the 8228 knows to issue three /INTA pulses. Unresolved.
18. **RST 7 bus side.** The manual says the 8228 gates RST 7 onto "the bus". That the CPU side D7-D0 carries it is inferred, not stated.
19. **Wait-state generator circuit.** The manual has no complete circuit (5). Section 5's description and section 13 are derived.

**Reading method:**

20. **OCR.** The text layer has many errors, e.g. "SOSO" for 8080, "OAA" for DAA, "OCR" for DCR, "bi·SYNC", "Ag" for A9. Every table value and pin number in this document was read from page images rendered at 110-250 dpi, never from the OCR. No numeric value depended on OCR text.
21. **Waveform edge assignments.** tRS and tIS end at the phi2 falling edge, tHS at the phi2 rising edge, and the 8228 tSS/tSH/tDC/tWE references are as stated in the text. These were read from enlarged crops of p.5-16 and p.5-10 and agree with the chapter 2 text where it speaks. They are drawing readings, not printed statements.
