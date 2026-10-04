# Hardware Build Plan

Non-normative. How to build the first real machine: parts, bring-up order, sourcing and the Pi platform decisions. The contract the board must meet is `ARCHITECTURE.md` section 6 (circuits) and `DEVICE_SPECS.md` (ports, READY contract); the Pi software is `PI_DAEMON.md`. **Where this file disagrees with them, the specs win**; fix this file.

Chip-level reference (pins, levels, cycle timing): `reference/8080_HARDWARE.md`. Source of this plan: the 2026-10-02 hardware-alignment pass.

---

## 1. Decisions

All accepted by Mike on 2026-10-02, except ROM-WE, STATUS-LEDS and LA-HEADER (2026-10-03), BUS-RESISTORS (revised 2026-10-03), GLUE, TEST-RESET, POWER and CONSTRUCTION (revised 2026-10-04) and NETLIST (2026-10-04). Record and rationale: `COLLABORATION_LOG.md`, Key Decisions ("Hardware Alignment: Buildable, One Hard Fix", "`pi8080d --sim`, the RAM Test Build, Board Debug Aids" and "Board Choices Signed Off; Bus Pull-Ups"). The IDs are the ones the specs cite.

| ID | Outcome |
|----|---------|
| WAIT-SET | WAIT flip-flop set asynchronously while STSTB low AND NOT RESET AND (D4 OR D6) AND port window. SYNC is optional, never the only qualifier. |
| REQ-QUALIFY | REQ = WAIT flip-flop Q AND the 8080A WAIT pin. The Pi sees REQ only in T_W. |
| DIR-SOURCE | DIR = 8228 /I/OR. Data 74LVC245A /OE = NOT /I/OR. No status latch. |
| LATCH | Separate LATCH GPIO clocks the 74HCT374 IN latch (not tied to ACK). |
| PI-GPIO | 20 GPIOs, A0-A6 (A7 is always 0 in the window). Pin map in `ARCHITECTURE.md` 6.4. Optional TEST_RESET on BCM 18. |
| GLUE | One ATF22V10C GAL plus 74HCT74/14/08, a second 74HCT08 (HALT term) and a 74HCT138 (ROM /WE gate). The GAL drives DB0 for IN FF itself; there is no 74HCT125 (`ARCHITECTURE.md` 6.5). Pinout and equations: `hw/glue.pld` (2.1). Discrete HCT is a valid fallback; it needs a tri-state buffer for the DB0 drive. |
| MEMORY-PARTS | AT28C64B-15PU ROM, 2x AS6C62256-55PCN RAM. |
| BUS-RESISTORS | 10 kohm pull-up SIPs on A0-A15 and system DB0-DB7; none on CPU-side D0-D7 (`ARCHITECTURE.md` 6.13). Jumpered 2.2 kohm DB pull-down for bring-up step 2 only, never fitted together with the socketed DB SIP. |
| POWER | 5 V input through a 2.1 mm centre-positive jack and a P-MOSFET reverse-polarity switch (`ARCHITECTURE.md` 6.9), Pololu U3V40F12 (+12 V), ICL7660-class charge pump (MAX1044) (-5 V), Schottky VBB clamp. Pi on its own supply. Current-limited bench +5 V for the first power-up. |
| RESET-SOURCE | DS1813-5 class open-drain supervisor, button on its RST node. |
| TEST-RESET | 2N3904 with a 4.7 kohm base resistor on /RESIN (`ARCHITECTURE.md` 6.6). Footprint fitted; populated only for the unattended test rig, whose pulses last at least 1 ms (DS1813 tPB). |
| CPU-SOURCING | One Intel/AMD 8080A reference plus two spares; 2x 8224, 2x 8228. |
| CONSTRUCTION | One 2-layer PCB, 160 x 120 mm, no backplane (2.2). The manual-ACK jig for step 4 is off-board and plugs into the Pi header (3.2). |
| NETLIST | `hw/board.net.txt` is the only home of pin numbers, checked pad by pad by `tests/netlist_tests.rs`; `ARCHITECTURE.md` 6 stays normative for circuits and rules (2.2). RDYIN is wired straight to the WAIT flip-flop /Q with a 10 kohm pull-up, no jumper. The 330 ohm Pi data array sits between the data 74LVC245A and the node of the Pi pins and the IN-latch inputs (`ARCHITECTURE.md` 6.4). |
| PI-PLATFORM | Pi 4B, busy-poll service (section 5; service model in `PI_DAEMON.md`). |
| CONSOLE-TRANSPORT | One TCP listener in the Pi daemon; a new client replaces the old. |
| CONSOLE-OUTPUT | Output buffer of at least 2 MiB; discard on full and on RESET. |
| RESET-TIMING | Abandon the in-flight request on RESET assertion; rebuild devices on release. |
| DEV-RESET | Device reset = rebuild the IoBus and devices from config. |
| TRACE-FORMAT | Line format only: `ARCHITECTURE.md` 7.3. |
| HW-STEP | No hardware single-step in v1 (Someday). The emulator is the debugger. |
| ROM-WE | AT28C64B /WE: 74HCT138 decode of MEMW to F000-FFFF, through jumper JP-WE, pulled up at the chip. Built open. Programmed with SDP off. In-circuit burns: `examples/burn` (`USER_GUIDE.md` 10). `ARCHITECTURE.md` 6.10. |
| STATUS-LEDS | WAIT, HALT, INTE and HLDA LEDs on 74HCT08 gates. `ARCHITECTURE.md` 6.11. |
| LA-HEADER | Three 2x10 analyzer headers (LA-A, LA-B, LA-C), 16 channels each, GND on pins 17-20; CPU-side D0-D7 through 1 kohm. `ARCHITECTURE.md` 6.12; channel order in 3.1 here, pins in the netlist. |

---

## 2. Bill of Materials

Design intent: one line per function. Order codes, quantities with spares, sockets, lifecycle status and the date each was checked: `PARTS_ORDER.md`, keyed to the netlist refdes.

| Part | Qty | Role |
|------|-----|------|
| 8080A, Intel or AMD | 1 | CPU, reference chip |
| 8080A, any make except a NEC D8080A without the F suffix (KR580VM80A is fine) | 2 | CPU spares, accepted only after steps 2 and 8 |
| 8224 (or KR580GF24) | 2 | Clock generator and RESET/READY synchronizer (1 spare) |
| 8228 (or KR580VK28), never an 8238 | 2 | System controller and bus driver (1 spare) |
| 18.432 MHz crystal, series-resonant fundamental, HC-49/US (CTS ATS184-E; a parallel 18 pF part is the fallback, step 1) | 1 | 8224 crystal (2.048 MHz CPU) |
| 510 ohm resistor | 2 | XTAL1 and XTAL2 to GND (8224 note 1, 18 MHz) |
| AT28C64B-15PU | 1 | 4 KB ROM at F000 (A12, /CE to GND; /WE through JP-WE, `ARCHITECTURE.md` 6.10). Program with software data protection off |
| AS6C62256-55PCN | 2 | RAM 0000-7FFF and 8000-FFFF |
| ATF22V10C-15PU | 1 | Memory decode, window decode, WAIT set term, IN-latch enable, FE decode, IN FF onto DB0 (2.1). Program it as plain ATF22V10C, not CEX |
| CD74HCT74E (TI) | 1 | WAIT flip-flop + overlay flip-flop |
| 74HCT14 | 1 | NOT RESET, /I/OR invert for the data 245 /OE, NOT A15 for the RAM high /CE, 2-stage Schmitt on ACK (1 spare) |
| SN74HCT08N | 2 | U08a: REQ = Q AND 8080A WAIT, plus the WAIT, INTE and HLDA LED buffers. U08b: the HALT term (1 spare gate). `ARCHITECTURE.md` 6.11 |
| SN74HCT138N | 1 | ROM /WE gate (`ARCHITECTURE.md` 6.10) |
| SN74HCT374N | 1 | IN latch, Pi to system data bus |
| SN74LVC245AN (not LVCH) | 3 | 5 V to 3.3 V: A0-A6 + DIR; D0-D7; REQ + RESET. VCC from the Pi 3V3 |
| DS1813-5 (no substitute: the board has no external /RESIN pull-up and relies on the DS1813's internal pull-up and pushbutton stretch) | 1 | RESIN power-on, brownout and button reset |
| 6 mm tactile pushbutton | 1 | Reset (SW1). The jig's buttons: 3.2 |
| Pololu U3V40F12 | 1 | +12 V boost from +5 V (4%, <= 12.48 V) |
| MAX1044CPA+ (ICL7660 class) | 1 | -5 V charge pump for VBB (<= 1 mA). Its two 10 uF capacitors are in the electrolytic row |
| BAT43 or BAT85 Schottky | 1 | VBB clamp to GND at the CPU socket |
| 1 kohm resistor | 1 | 8228 INTA (pin 23) to +12 V, RST 7 strap |
| 10 kohm resistor | 4 | INT pull-down (HOLD and BUSEN tied to GND directly); ROM /WE pull-up; RDYIN pull-up (holds RDYIN high in step 2, before the 74HCT74 is fitted); P-MOSFET gate to GND |
| 4.7 kohm resistor | 2 | ACK and LATCH pull-downs, 5 V side |
| 330 ohm isolated 8-resistor array (16-pin DIP) | 1 | Pi D0-D7 series array, between the data 74LVC245A outputs and the node of the Pi pins and the IN-latch inputs (`ARCHITECTURE.md` 6.4): contention protection |
| 2.2 kohm 9-pin bused SIP + jumper | 1 | Bring-up NOP free-run pull-down on DB0-DB7; removed in normal operation |
| 10 kohm 9-pin bused SIP (8 resistors, common pin to +5 V), 2% | 3 | Bus pull-ups: A0-A7, A8-A15, system DB0-DB7 (`ARCHITECTURE.md` 6.13) |
| 9-pin 0.1-inch SIP socket (or socket strip) | 1 | DB pull-up SIP: out while the 2.2 kohm pull-down is in (step 2) |
| 2N3904 + 4.7 kohm resistor | 1 | Optional Pi TEST_RESET: open collector on /RESIN, base from BCM 18 (`ARCHITECTURE.md` 6.6). Footprint fitted; populated for the test rig only |
| Low-current 3 mm LED (`ARCHITECTURE.md` 6.11) | 4 | Status LEDs WAIT, HALT, INTE, HLDA |
| 1.0 kohm resistor | 4 | LED series resistors |
| 1 kohm isolated 8-resistor array (16-pin DIP) | 1 | Analyzer isolation on the CPU-side D0-D7 taps (`ARCHITECTURE.md` 6.12) |
| 2x10 0.1-inch header (a shrouded box header also takes single leads) | 3 | Logic-analyzer headers LA-A, LA-B, LA-C (3.1) |
| 2-pin 0.1-inch header + shunt | 1 | JP-WE ROM write enable. Built open, with the shunt parked on one pin |
| 0.1 uF ceramic | 21 | Decoupling, one per IC per rail (`ARCHITECTURE.md` 6.9; placement 2.3) |
| 10 uF electrolytic | 4 | Bulk on +5 V and +12 V; charge-pump flying capacitor; -5 V reservoir, which is also the -5 V bulk |
| Mean Well GST18A05-P1J (5 V 3 A, 5.5 x 2.1 mm centre-positive plug) | 1 | Board input. It must deliver at least about 4.9 V at the jack under load (`ARCHITECTURE.md` 6.9); checked loaded before step 1 (3) |
| 2.1 mm DC power jack, PCB mount | 1 | 5 V input (J5). Match its footprint at the first import (2.2) |
| Vishay SUP53P06-20 (logic-level P-MOSFET, TO-220, RDS(on) specified at VGS = -4.5 V) | 1 | 5 V reverse-polarity switch (Q2, `ARCHITECTURE.md` 6.9) |
| 2x20 0.1-inch shrouded box header | 1 | Pi header J1, the board end of the ribbon (keyed) |
| 1-pin 0.1-inch header | 7 | Test points TP1-TP7: GND x2, +5V (after the FET), +12V, -5V, +3V3_PI, /WSET (2.2). None on phi1 or phi2 (`ARCHITECTURE.md` 6.12 rule 1) |
| Raspberry Pi 4B + its own PSU + 2x20 short ribbon | 1 | Coprocessor (devices, console TCP, storage) |
| ZIF-40 socket + DIP sockets | 1 | CPU socket (chip tester), sockets for all DIPs |
| 2-layer PCB, 160 x 120 mm | 1 | Single board (2.2), test points and jumpers per bring-up stage |

18 ICs (U1-U18), plus two transistors (Q1, Q2) and the +12 V module. The netlist is the count (2.2).

### 2.1 GAL

- **Pinout and equations:** `hw/glue.pld`, the only home of the GAL pin list. All 22 signal pins are used: 16 inputs, 6 outputs.
- **Pin classes.** Nets driven by the 8080A or the 8224 (A8-A15, D4, D6, /STSTB) sit on dedicated input pins (1-11, 13) only, so no GAL image, a blank or wrong one included, can drive a CPU pin. The outputs sit on I/O pins (14-23). Any swap within a class is legal during routing: edit the pin list in `glue.pld` and the netlist, rebuild, commit. `tests/gal_tests.rs` enforces the classes.
- **Build:** `cd hw && make` (galette 0.3.0: `cargo install galette --version 0.3.0 --locked`). Commit `glue.pld` and `glue.jed` together. `cargo test` evaluates the committed `glue.jed` for every input combination against the emulator's decode and needs no galette.
- **Programming:** burn `hw/glue.jed` under the programmer's plain ATF22V10C (GAL mode, 5892 fuses) device, never a CEX or power-down entry (4). Verify on the programmer immediately before socketing. The `.jed` is stored byte-exact (`.gitattributes`): its file checksum covers the line ends.

### 2.2 Board

- **Netlist:** `hw/board.net.txt`, the only home of pin numbers (decision NETLIST); format in its header. `cargo test` checks it against `ARCHITECTURE.md` 6 pad by pad (`tests/netlist_tests.rs`): every pad exactly once, no single-pad nets, no open inputs (and the datasheet-open pins left open), no totem-pole output sharing a net with another driver, power pins on their rails, rails never joined, the circuits of 6.1-6.13, the part values they specify, the test-point nets, the Pi pin map from `src/pi`, no 5 V source reaching a Pi pin, the analyzer channel order of 3.1, and the GAL pins from `hw/glue.pld`.
- **Changing the board:** edit `hw/board.net.txt`, run `cargo test`, copy the file the stale-netlist failure names to `hw/board.kicad.net`, and commit both. A GAL pin move edits `hw/glue.pld` as well (2.1). An added, removed or renamed refdes edits its line in `PARTS_ORDER.md`; `cargo test` fails until every refdes is on exactly one order line.
- **KiCad:** `hw/board.kicad_pro` is minimal (KiCad fills it in on first open; commit the result). `hw/board.kicad_pcb` holds the 160 x 120 mm outline on Edge.Cuts, 2 layers, 1.6 mm FR4, 1 oz copper. `hw/board.kicad_dru` holds the fab minimums and the rail widths (`cargo test` checks that every rail has one: at least 0.8 mm on +5V, +5V_IN and GND, 0.4 mm on the others). Import `hw/board.kicad.net` into Pcbnew, then place (2.3) and route; Freerouting is optional. Once the board has footprints, `cargo test` also checks its refs, footprints and pad nets against the netlist.
- **Refdes:** U1 8080A, U2 8224, U3 8228, U4 ROM, U5 and U6 RAM low and high, U7 GAL, U8 74HCT74, U9 74HCT14, U10 U08a, U11 U08b, U12 74HCT138, U13 74HCT374 (IN latch), U14-U16 74LVC245A (A0-A6 and DIR; DB0-DB7; REQ and RESET), U17 charge pump, U18 DS1813, Q1 TEST_RESET, Q2 reverse-polarity FET, PS1 +12 V module, J1 Pi header, J2-J4 LA-A, LA-B, LA-C, J5 5 V jack, JP1 JP-WE, JP2 JP-PD, SW1 reset, RN1-RN6 resistor networks, TP1-TP7 test points, H1-H4 M3 holes.
- **Fab gate.** No board is ordered while an item below is open. Close each in the commit that verifies it, citing the source.
  - PS1 pad order. Pololu #4016 names the pins (VIN and GND doubled, VOUT, EN) but its text gives no order. Buy the module before fab and read it off the board.
  - Every footprint name and pad set, at the first Pcbnew import: zero warnings expected, result recorded in the log.
  - J5's footprint against the bought jack, including which pad is the centre pin.
  - `hw/board.kicad_pro` and `hw/board.kicad_pcb` open in KiCad 8 without complaint.
  - Every type table in `tests/netlist_tests.rs` checked against its datasheet. The tables are single-source: the test proves the netlist agrees with them, not that they agree with silicon.

### 2.3 Placement Constraints (est)

Guidance for placement and routing. Nothing here has a spec figure behind it unless it cites one; distances are estimates. The design rules that are checked are in `hw/board.kicad_dru` (2.2).

1. **Clock island.** U2 next to U1's phi1 and phi2 pins. Keep PHI1 and PHI2 as short as possible (about 50 mm), with no other trace between them and no vias if avoidable: the phi outputs are not short-circuit protected (`ARCHITECTURE.md` 6.1). Y1, R1 and R2 within about 10 mm of U2's XTAL pins. No test point on PHI1 or PHI2: step 1 probes U2's pins 10 and 11 directly (`ARCHITECTURE.md` 6.12 rule 1).
2. **CPU data.** U3 next to U1's data pins, D0-D7 short. D4 and D6 also carry the GAL tap: keep them within the 5-10 pF trace budget of `ARCHITECTURE.md` 6.12 rule 2 (about 50-100 mm at 1 pF/cm). RN5 at the U1/U3 data lines, J3 (LA-B) beside RN5.
3. **Address pull-ups.** RN1 and RN2 at U1's address pins (`ARCHITECTURE.md` 6.13).
4. **VBB clamp.** D1 at U1's VBB pin, at the CPU socket (`ARCHITECTURE.md` 6.9), with C3 beside it.
5. **Decoupling.** Each 0.1 uF at its IC's power pin, with the shortest loop to GND: C1, C2, C3 at U1 (+5 V, +12 V, -5 V); C4, C5 at U2 (+5 V, +12 V); C6-C21 at U3-U18 in order, one each (C17-C19 on +3V3_PI at U14-U16).
6. **WAIT path.** U8 between U2 and U7. Keep /STSTB to GAL pin 1, /WSET to U8's /PRE, and U8's /Q to U2's RDYIN short: the STSTB width at /PRE is a bench risk (6), and RDYIN has a 167 ns deadline (`ARCHITECTURE.md` 6.4). TP7 at U8's /PRE.
7. **GAL pin classes.** A routing swap of GAL pins stays within its class (2.1): CPU-driven nets on pins 1-11 and 13, outputs on 14-23. Edit `hw/glue.pld` and the netlist together and rebuild the `.jed`.
8. **Pi edge.** J1 on a board edge, pin 1 marked, keyed box header. U13-U16 and RN6 beside J1, C17-C19 on the 3V3 side. All eight J1 GND pins to the ground pour.
9. **ZIF clearance.** A keep-out for U1's lever and body: the ZIF body is wider than a DIP-40.
10. **Analyzer edge.** J2-J4 along one edge, pin 1 square, channel names on the silkscreen (3.1), at least two GND leads reachable per header.
11. **Operator edge.** SW1 and the labeled LEDs D2-D5; JP1 (silkscreen "JP-WE: OPEN = normal"); JP2 and the RN3 socket together (silkscreen "never both").
12. **Power entry.** J5 at a board edge, Q2 and C22 beside it, TP3 after Q2. PS1 near U1 and U2 (the +12 V loads), C23 at PS1's output. U17 with C24 and C25 near U1's VBB pin.
13. **Ground.** Solid GND pour on the bottom layer, stitched, no splits. The Pi ground joins only at J1.
14. **Silkscreen.** Refdes, values, pin 1 marks, rail names at the test points, BCM names at J1, "TP3: +5 V for the jig" (3.2).

---

## 3. Bring-Up Plan

One stage at a time. Do not start a stage until the previous one passes.

| Step | Pass criterion |
|------|----------------|
| 0. Bare PCB, no ICs. Continuity and rail-to-rail shorts. Power from a current-limited bench +5 V, Pololu and charge pump fitted. Then, before step 1, the board's own adapter (GST18A05-P1J) in J5 instead of the bench supply, loaded with 4.7 ohm 10 W from TP3 to TP1 (about 1 A: the +5 V estimate of `ARCHITECTURE.md` 6.9 plus the boost module's input, est) and 4.7 kohm from TP5 to TP2 (about 1 mA, the VBB budget). | +5 V 4.85-5.15, +12 V 11.4-12.6 (<= 12.48), -5 V -4.75..-5.25 at every socket. Single-shot scope of VBB at power-up and power-down stays <= +0.3 V relative to GND. With the adapter and the loads: +5 V at the jack (J5 centre pin to GND) at least 4.9 V, +5 V at TP3 4.85-5.15, the jack-to-TP3 drop at most about 40 mV (the FET, `ARCHITECTURE.md` 6.9, est), and VBB at TP5 -4.75..-5.25. If +5 V is low at the jack, the adapter fails (`ARCHITECTURE.md` 6.9): replace it before going on. If only the drop is high, check Q2 and the +5 V path before going on. |
| 1. 8224 + crystal + 510 ohm pair + DS1813. Scope phi1 and phi2 at U2 pins 11 and 10 (no test points), RESET and STSTB. 10x probes only: phi outputs are not short-circuit protected. | 2.048 MHz +/- crystal offset. phi1 >= 60 ns, phi2 >= 220 ns, phi high >= 9.0 V. RESET held >= 3 clocks (ms) after power-up and on every button press, and active high. The oscillator starts on every one of 20 power cycles. Crystal drive level within the crystal's rated maximum (ATS184-E: 1000 uW), measured with a current probe on one crystal lead as P = I^2 x ESR. If the series crystal does not start every time, fit the parallel fallback (4) and repeat. |
| 2. Add 8080A (ZIF) + 8228. No memory and no 74HCT74, so RDYIN is held high by its 10 kohm pull-up. 2.2k DB pull-down jumper in (floating reads = NOP) and the DB pull-up SIP out of its socket (never both, `ARCHITECTURE.md` 6.13). Address pull-up SIPs fitted. BUSEN, HOLD, INT tied. | A0 toggles at 256 kHz, A15 period 128 ms. Supply currents within datasheet max (+12 V <= 87 mA). Screen every CPU this way. |
| 3. Add ROM, RAM, the GAL (2.1: burn `hw/glue.jed`, verify immediately before socketing) and the 74HCT74, the 74HCT14 (U9: NOT RESET to the GAL, the overlay /PRE and the WAIT /CLR; NOT A15 to the RAM high /CE; the ACK Schmitt pair to the WAIT CLK, held low by the ACK pull-down; NOT /I/OR to the data 74LVC245A /OE), the 74HCT138 with JP-WE open, and U08a with the WAIT, INTE and HLDA LEDs (its REQ gate unused until step 4). The WAIT flip-flop's set path is live from here, with /Q on RDYIN. Pull-down out, DB pull-up SIP in. Burn the diagnostic image `rom/diag3.bin` (3.3) with the external programmer: read IN FF, OUT FE, read IN FF again, then the window check OUT 70, IN 70, OUT FD, IN FD (just outside the Pi window, 70-FD read the DB pull-ups), march-test 0000-EFFF (HLT at a fail address), then copy a short loop to 0100 and run it from RAM: for each byte of F000-FFFF, read it, write it back, wait at least 313 T (tBLC at the fastest legal clock, the page-close wait of `ARCHITECTURE.md` 6.10 rule 3; about 0.7 s for the whole range) and read it again, HLT at a fail address if the two reads differ, else HLT at the pass address after FFFF. If a wiring fault reaches ROM /WE despite JP-WE open, the first write starts a write cycle of the byte's own value. The wait closes the page load, so the read after it is a polling read under either reading of the datasheet (bench item K-1, 6): it returns data polling, I/O7 complemented (AT28C64B DS 4.2, 4.4), and the loop stops at F000 before writing anything wrong. The ROM /WE check in the pass column shows the fault too. Run the same image in the emulator first and dump the debugger instruction trace (3.3). The analyzer is attached throughout (3.1). | The analyzer shows status A2 at the pass address (0117, 3.3), then 8A at pass+1, then no further /STSTB. IN FF bit 0 reads 1 then 0. LA-C ch 6 (/Q) stays high throughout: a fall means the WAIT set fired outside the window, and the CPU stalls there in T_W. The fetch-address sequence of the first ~200 instructions matches the emulator trace exactly (3.1). LA-C shows a /Y7 low pulse for each of the 4096 writes to F000-FFFF and none during the march test, and ROM /WE (on a scope, or fly-wired to the LA-C spare pad) never goes low. At the HLT: WAIT lit, INTE and HLDA dark. Scope: DB high level during SRAM writes (the march test), where the 8228 drives DB at TTL levels into the SRAM inputs, >= 2.9 V. If it is lower, check that the DB pull-up SIP is fitted in its socket before going on. |
| 4. Put U08a gate 1 (REQ, fitted in step 3) to use, and add the IN latch and U08b with the HALT LED (the WAIT set path is live from step 3), with the off-board manual-ACK jig (3.2) plugged into J1 in place of the Pi ribbon. Burn the real monitor.bin. | The CPU freezes on the first Pi-window access, the banner's OUT 00. Each ACK press advances exactly one access. The scope shows RDYIN low <= STSTB+167 ns, the STSTB pulse at HCT74 PRE >= 20 ns, and REQ rising only after the 8080 WAIT pin. The LA shows zero REQs on memory cycles over 10^6 cycles and zero phantom REQs across 100 consecutive resets (LA-C alone: REQ never high while /MEMR or /MEMW is low). While frozen: WAIT lit, HALT dark; between ACK presses LA-B shows WAIT low only between accesses. On each jig-served IN, LA-B shows the DIP-switch byte on D0-D7 in T3. |
| 5. Connect the Pi 4B (own supply) and install `pi8080d`: cross-build it on the Mac with `cargo build --release --target aarch64-unknown-linux-musl --bin pi8080d` and install it as `/usr/local/bin/pi8080d` (`PI_DAEMON.md` 2), and install `scripts/pi8080d.service` (`PI_DAEMON.md` 11). Day one: with the board powered, scope REQ/ACK/LATCH for the per-access service time (`PI_DAEMON.md` 12.2). Then, for the boot port trace (`ARCHITECTURE.md` 7.3), add the flag to the unit with `sudo systemctl edit pi8080d` (`[Service]`, `ExecStart=`, `ExecStart=/usr/local/bin/pi8080d --storage /var/lib/pi8080d --trace /var/lib/pi8080d/trace.txt`), restart it, press RESET, and read the trace and the banner over the TCP console. Do not run a second `pi8080d` beside the unit: it is refused (`PI_DAEMON.md` 3.3). Remove the drop-in afterwards (tracing costs a write per line, `PI_DAEMON.md` 9). The rest of this step's bench checks: `PI_DAEMON.md` 14. | The boot port trace and the banner are byte-identical to the emulator's port trace and transcript for the same monitor.bin. Power both boards on in either order, and with the Pi off: the board waits at the first access (WAIT steady, HALT dark) and continues when the daemon starts. `F 0200 0200 76` then `G 0200`: WAIT and HALT steady, until RESET. |
| 6. The same `pi8080d` (shared IoBus). Replay the strict-harness transcript files over the TCP console (in `cargo test` they already pass through the daemon on the simulated board, `PI_DAEMON.md` 13.2). | Every transcript matches the emulator's output exactly. Ctrl-less paste of an Intel HEX file (Phase 5) loses no bytes. |
| 7. Storage conformance test (`DEVICE_SPECS.md` 3), with stress-ng on the Pi's other cores. Then press RESET during a long W/fsync. | I 0A/09/08 read 00 10 00 after both W and L, and C F000 FFFF 2000 prints nothing. After RESET mid-fsync the machine reboots to the banner, with storage unmounted and no hang. |
| 8. CPU acceptance: load the exercisers from storage with L, using the 8080-asm BDOS shim (0005 JMP to a print routine <= EEFF; 0000 JMP F000, a cold start, since WARM has no published address: ARCHITECTURE 2). Run TST8080, 8080PRE, CPUTEST (~2 min), 8080EXM (~3.2 h at 2.048 MHz, est from emulator cycle counts). | All PASS, with CRCs equal to the published values. Repeat for each spare CPU. Once the emulator AC fixes land, the emulator's output must match the silicon output line for line. |


ROM changes can run on the board before a burn: the RAM test build (`ARCHITECTURE.md` 2.1), loaded through the resident monitor from step 5 on.

The first in-circuit burn (`examples/burn`, `USER_GUIDE.md` 10) comes after step 7: it needs the monitor, the console and storage. Run K-1 (section 6) just before it. Until then, and to recover from a failed burn, use the external programmer.

### 3.1 Logic Analyzer

The analyzer and the three headers it plugs into (`ARCHITECTURE.md` 6.12). Each 2x10 header carries 16 channels in order and GND on four pins; the silkscreen names the signals and marks pin 1 (square pad). Pin numbers live in `hw/board.net.txt` (J2-J4); `tests/netlist_tests.rs` checks the channel order below against the circuit. Use at least two GND leads per header.

**LA-A: address.** Channel n is An (n = 0-15), from the 8080A address pins.

**LA-B: CPU-side data and CPU timing**

| Ch | Signal | Source |
|----|--------|--------|
| 0-7 | D0-D7 | 8080A data pins, each through 1 kohm |
| 8 | SYNC | 8080A |
| 9 | DBIN | 8080A |
| 10 | /WR | 8080A |
| 11 | READY | 8224 READY (= 8080A READY) |
| 12 | WAIT | 8080A |
| 13 | /STSTB | 8224 |
| 14 | phi2 (TTL) | 8224 |
| 15 | RESET | 8224 |

**LA-C: system strobes, flip-flops, Pi handshake**

| Ch | Signal | Source |
|----|--------|--------|
| 0 | /MEMR | 8228 |
| 1 | /MEMW | 8228 |
| 2 | /I/OR | 8228 |
| 3 | /I/OW | 8228 |
| 4 | INTE | 8080A |
| 5 | HLDA | 8080A |
| 6 | /Q | WAIT flip-flop /Q at the 74HCT74, which is RDYIN (10 kohm pull-up: high in step 2, before the 74HCT74 is fitted) |
| 7 | REQ | U08a gate 1 output (5 V side) |
| 8 | ACK | 5 V board side, at the 74HCT14 input (3.3 V level) |
| 9 | LATCH | 74HCT374 CLK (3.3 V level) |
| 10 | OVL | overlay flip-flop Q |
| 11 | ROM /OE | ROM_OE net |
| 12 | ROM_WE decode | 74HCT138 /Y7 (the JP-WE side) |
| 13 | HALT | U08b gate 3 output |
| 14 | /RESIN | 8224 /RESIN (supervisor and button node) |
| 15 | spare | pad, unconnected (fly-wire to ROM /WE for the step 3 check) |

Not on any header: phi1 and phi2 (MOS level, >= 9.4 V) and 8228 /INTA pin 23 (+12 V strap) (`ARCHITECTURE.md` 6.12 rule 1).

**Analyzer.**
- Within the load the board allows (`ARCHITECTURE.md` 6.12 rule 3): never attach it unpowered, and treat "5 V tolerant" as unproven until its input current is checked from 0 to 5.25 V (a series resistor plus a clamp to 3.3 V fails the rule).
- Supported by sigrok, with a threshold anywhere from 1.2 to 1.8 V (TTL VOL 0.45 V and VOH 2.4 V, the 8080A and HCT outputs, and the 3.3 V Pi lines all read correctly).
- At least 24 MS/s on every channel in use, about 11 samples per T-state at tCY 488 ns. Streaming USB-2 analyzers often cannot reach that rate with 16 or more channels enabled, so check the rate at the channel count used.
- **32 channels** (LA-A + LA-B) are needed for machine-cycle decoding and the step 3 fetch-sequence check. **16 channels** (one header) cover the other checks: LA-C alone covers the step 3 write decode, step 4 and step 5.
- **Planned unit** (Mike, 2026-10-03): 32 channels, sigrok-supported, >= 24 MS/s, 5 V tolerant. It takes LA-A + LA-B at once, or any two headers. Check its input current against the first bullet before it is first attached.

**sigrok decoding.**
- **Decoder.** libsigrokdecode has no 8080 decoder (decoder list checked 2026-10-03). Its `z80` decoder requires /M1, which the 8080A does not have as a pin (M1 is status bit D5, `reference/8080_HARDWARE.md` 4.4), and it prints Zilog mnemonics. Use the generic `parallel` decoder: channels `clk`, `d0`-`d15` and `rst`; options `clock_edge` (rising, falling or either), `wordsize`, `endianness` and `reset_polarity`; at most 16 data lines per instance, so stack two instances for address plus data.
- **Capture.** Timing mode, >= 24 MS/s. Trigger on RESET falling (LA-B ch 15) to start at the first fetch after reset.
- **Machine cycles.** Run two instances, both with `clk` = /STSTB and `clock_edge=rising`: instance 1 with `d0`-`d15` = A0-A15 (the address), instance 2 with `d0`-`d7` = D0-D7 (the status byte). Status is valid on D0-D7 from <= 349 ns into T1 until phi2 rises in T2, >= 597 ns. /STSTB rises at about 444-508 ns. The address is valid through T3 (`reference/8080_HARDWARE.md` 4.3, 13.2). So both are stable for >= 89 ns after the edge, longer than one 41.7 ns sample even with the 1 kohm isolation delay (about 33 ns, est, which only shifts edges already >= 95 ns before /STSTB rises). Status bytes: A2 fetch, 82 memory read, 00 memory write, 86 stack read, 04 stack write, 42 IN, 10 OUT, 8A halt (`reference/8080_HARDWARE.md` 4.4).
- **Fetch sequence (step 3).** Keep the rows with status A2; their addresses are the opcode addresses in execution order. Compare them line by line with the PCs in the emulator's debugger instruction trace for the same image.
- **Write data.** `clk` = /WR, `clock_edge=rising`, `d0`-`d7` = D0-D7. Data stays valid >= 118 ns after /WR rises (`reference/8080_HARDWARE.md` 3.5).
- **Command line.** Name the channels at capture time after the tables above; PulseView does the same interactively. The address instance, for example: `sigrok-cli -i boot.sr -P parallel:clk=STSTB:d0=A0:d1=A1:d2=A2:d3=A3:d4=A4:d5=A5:d6=A6:d7=A7:d8=A8:d9=A9:d10=A10:d11=A11:d12=A12:d13=A13:d14=A14:d15=A15:clock_edge=rising -A parallel=items` (one annotation per /STSTB edge; the decoder's `words` row stays empty unless `wordsize` is set).

### 3.2 Manual-ACK Jig (Off-Board)

Step 4 serves Pi-window accesses by hand. The jig plugs into J1 in place of the Pi ribbon; it is not on the board and not in the netlist.

- **Connector.** A 2x20 IDC socket that connects only GND, BCM 16 (ACK), BCM 17 (LATCH) and BCM 20-27 (D0-D7); their J1 pins are in `hw/board.net.txt`. It MUST NOT connect the 3V3 pins (the board's +3V3_PI, the 74LVC245A supply) or the 5V pins.
- **Supply.** +5 V from a clip lead to TP3. With no Pi, +3V3_PI is unpowered and every 74LVC245A is in Ioff, so 5 V on the Pi-side pins is safe (SCAS218X). `ARCHITECTURE.md` 6.4 relies on the same state with the Pi off.
- **ACK.** A pushbutton to +5 V through 1 kohm, with 1 uF to GND. The board's 4.7 kohm pull-down discharges it, and the two Schmitt stages (`ARCHITECTURE.md` 6.4 rule 2) square it.
- **LATCH.** A pushbutton to +5 V through 1 kohm. The board's 4.7 kohm pull-down holds it low; bounce only re-clocks the same data.
- **D0-D7.** An 8-position DIP switch to +5 V, with 10 kohm pull-downs (one bused SIP). The switches drive the IN-latch inputs directly. Through the 330 ohm array they reach only the outputs of the unpowered data 74LVC245A.

Parts: a 2x20 IDC socket on a short ribbon stub, two pushbuttons, two 1 kohm resistors, a 1 uF capacitor, an 8-position DIP switch, a 10 kohm 9-pin bused SIP and a clip lead.

### 3.3 Step 3 Diagnostic Image

`rom/diag3.asm`, built by `cd rom && make` into `rom/diag3.bin` (4096 bytes, unused bytes FF, committed like `monitor.bin`; 172 bytes used). It is not a monitor program: it is burned with the external programmer in place of `monitor.bin`, runs from RESET through the overlay (F000, fetched at 0000), uses no stack and no console, and makes no access to ports 00-6F, so it runs with no Pi and no jig. Its first byte is not 31, so `examples/burn` refuses it (`ARCHITECTURE.md` 3.2 requirement 7). The source header gives the sequence and the T-state counts.

**Sequence.** IN FF (bit 0 must be 1), OUT FE, IN FF (bit 0 must be 0); OUT 70, IN 70, OUT FD, IN FD (values not checked: undefined, `DEVICE_SPECS.md` rule 2.4); March C- over 0000-EFFF with 55h and AAh (up w0; up r0,w1; up r1,w0; down r0,w1; down r1,w0; up r0); then copy the write-back loop to 0100 and run it: each byte of F000-FFFF read, written back, 331 T later (>= 313 T, `ARCHITECTURE.md` 6.10 rule 3) read again. About 9.8 s at 2.048 MHz: 9.1 s march, 0.77 s write-back.

**HLT addresses.** Read off the analyzer: status A2 at the address below, then 8A at the next address, then no further /STSTB. The emulator prints the next address (`HLT at PC=...`). On a march or write-back fail, the last memory read before the HLT (status 82) shows the failing address and the byte read; HL holds the address too.

| HLT at | Meaning |
|--------|---------|
| 0117 | Pass |
| 0118 | ROM write-back: the second read of a ROM byte differed. A write reached ROM /WE (data polling, I/O7 complemented); expect F000 |
| F003 | IN FF bit 0 = 0 after RESET: overlay flip-flop not set |
| F004 | IN FF bit 0 = 1 after OUT FE: overlay not cleared |
| F005 | March M1: a cell did not read 55h (an overlay that reads ROM at 0000-0FFF fails here at 0000) |
| F006 | March M2: a cell did not read AAh |
| F007 | March M3: a cell did not read 55h |
| F008 | March M4: a cell did not read AAh |
| F009 | March M5: a cell did not read 55h |

No HLT, with WAIT lit: the WAIT flip-flop set on the window check (or any access), and the CPU waits in T_W at that IN or OUT.

**Emulator trace.** From the repo root:

```
printf 's 100\nring\n' > /tmp/diag3.dbg
cargo run -- --rom rom/diag3.bin --script /tmp/diag3.dbg > /tmp/diag3.trace
sed -n '/^dbg> ring$/,$p' /tmp/diag3.trace | grep -E '^[0-9A-F]{4}  ' | cut -c1-4
```

The last line prints the opcode addresses of the first 256 instructions in execution order (`ARCHITECTURE.md` 7.4: `s 100` steps 256, hex; `ring` lists them). Compare them with the A2 rows of the capture (3.1, Fetch sequence). Ignore the register columns of the trace: on the board the registers are undefined after RESET, and the fetch sequence does not depend on them. `cargo run -- --rom rom/diag3.bin < /dev/null` runs it to the end and prints `HLT at PC=0118`. Tests: `tests/diag_tests.rs` (the bus transfers against a model of this sequence, every HLT address above by fault injection, JP-WE fitted under both readings, and this trace).

---

## 4. Sourcing

- **Order list:** `PARTS_ORDER.md`: one line per order line, keyed to the netlist refdes, with spares and lifecycle status (`tests/netlist_tests.rs` checks that it covers every refdes). Mouser primary, Pololu direct for the U3V40F12 (Mouser does not stock it), DigiKey as the fallback for a line Mouser is out of. DigiKey has refused AT28C64B sales to non-manufacturers.
- **8080A spares: any make except a NEC D8080A without the F suffix.** That part has a SUB flag in bit 5 and a different DAA, so it is not Intel-compatible. uPD8080AF and KR580VM80A are fine. The reference CPU is Intel or AMD (CPU-SOURCING).
- The 8080A, 8224 and 8228 are NOS/surplus only. Quality varies. Buy spares: one reference CPU plus two, and two each of 8224 (or KR580GF24) and 8228 (or KR580VK28). Order them when the PCB goes to fab, so the return windows run while the board can test them. What to check in a listing: `PARTS_ORDER.md` 4.
- Accept a chip only after it passes the bring-up step 2 current screen and all four exercisers (step 8). Until then it is a suspect, not a spare.
- **Never an 8238 (or KR580VK38).** Its advanced /MEMW and /I/OW fall outside the 8228 timing that the ROM write (`ARCHITECTURE.md` 6.10) and the HALT term (6.11) were derived from.
- 74HCT74: TI CD74HCT74E (Active). SN74HCT74N is NRND and Nexperia no longer makes the DIP. Against the SN74HCT74 at 4.5 V, -40 to 85 C (TI SCHS409, SCLS169G): PRE or CLR to Q 50 ns (44), CLK to Q 44 ns (35), PRE pulse width 20 ns minimum (`ARCHITECTURE.md` 6.4 assumes 20-24). GAL 15 ns plus 50 ns puts RDYIN low 65 ns after STSTB falls, inside the 167 ns of 6.4. CLK to Q adds 9 ns to the "about 100 ns (est)" REQ fall, against the Pi's 500 ns wait after the ACK read-back (6.4 rule 2).
- Charge pump: MAX1044CPA+, ICL7660 class with the same pinout (decision POWER).
- DS1813: order DS1813-5+, the RoHS ordering code of the DS1813-5; the netlist value names the part.
- 74LVC245A only, never 74LVCH245A: bus-hold fights the Pi on D0-D7.
- ATF22V10C: program it as plain ATF22V10C, not CEX. CEX and power-down (PWD) entries write the 5893-fuse JEDEC, which makes pin 4 (A10 in the current pin list) the power-down pin (ATF22V10C 0735U 9, 10). The ROM programmer is already needed for the AT28C64B; before buying one, check that it takes `hw/glue.jed` unconverted (`PARTS_ORDER.md` 7).
- AT28C64B: program it with software data protection off. Parts ship that way (AT28C64B DS 4.6.2, https://ww1.microchip.com/downloads/en/DeviceDoc/doc0270.pdf). If the programmer offers an SDP option, leave it off: with A12 tied low the 8080 cannot clear SDP in circuit (`ARCHITECTURE.md` 6.10 rule 5).
- SN74HCT08N and SN74HCT138N: TI, Active in PDIP (orderable tables in SCLS063G and SCLS171F, checked 2026-10-03; ti.com again 2026-10-04).
- Crystal: CTS ATS184-E, series-resonant fundamental, ESR <= 40 ohm, drive level 1000 uW maximum (CTS DOC# 008-0309-0 rev N). The parallel ATS184B-E (18 pF) is the fallback if the series part fails the step 1 start-up check. A frequency offset is harmless because the ROM is timing-independent; start-up and drive level are what step 1 checks.
- Board supply: Mean Well GST18A05-P1J. Its +/-5% load regulation (distributor data) is wider than the 4.85-5.15 V window of `ARCHITECTURE.md` 6.9, so it is checked loaded before step 1.

---

## 5. Pi Platform

Decisions only. The service model (bus loop, RESET handling, console transport, TIME clock source, build, deployment, tests and the bench checks) is normative in `PI_DAEMON.md`.

- **Platform:** Raspberry Pi 4B for v1, on its own supply, grounds joined at the header. A Pi 5 also works but its GPIO reads cross PCIe to RP1 and are several times slower (est); not in v1.
- **OS:** 64-bit Raspberry Pi OS, so `time_t` is 64-bit and the clock does not wrap in 2038.
- **Clock (mailbox `TIME`):** set only when the kernel reports NTP-synchronized. Normative: `DEVICE_SPECS.md` 8, TIME clock.
- **Bus service:** one thread busy-polls the mmapped GPIO block (`/dev/gpiomem`) on an isolated core and calls the IoBus inline. Cost: one core at 100%. `PI_DAEMON.md` 1, 4, 11; service-time estimates in 12.
- **RESET:** kernel-latched edges from the GPIO character device (raw v2 ioctl, no crate). No rppal: it was archived 2025-07-01. `PI_DAEMON.md` 5.
- **Console:** one TCP listener in the daemon; a new client replaces the old (CONSOLE-TRANSPORT). TCP backpressure is the out-of-band input flow control `DEVICE_SPECS.md` 4 requires. Address and access: `PI_DAEMON.md` 7, 10, 11. Use socat to bridge a serial port or USB gadget if wanted.
- **Background work:** network work runs in a worker process on the other cores; the device starts, checks and finishes it under READY as bounded work (`DEVICE_SPECS.md` 3.3; `PI_DAEMON.md` 1).
- **Code:** one crate, two binaries. The emulator and the daemon call the same port-mapping function to build the same IoBus. Build: `PI_DAEMON.md` 2.

---

## 6. Residual Risks (Bench Only)

These cannot be closed on paper. Each has a bring-up check.

- **STSTB pulse width at the HCT74 PRE** after the gate path (datasheet >= 40 ns in, 20-24 ns needed). Step 4.
- **Reset-release runt** from the 8224 STSTB/RESET skew could set WAIT as reset ends. Step 4: zero phantom REQs across 100 resets.
- **SRAM VIH at zero datasheet margin** against the 8228 VOH (2.4 V both). The risk is on writes, when the 8228 drives DB into the SRAM; on reads the SRAM drives DB itself. Real margin is large because the load is CMOS, and the DB pull-up SIP can only raise the high level (`ARCHITECTURE.md` 6.13). Step 3: DB high >= 2.9 V on SRAM writes; if lower, check the DB SIP is fitted.
- **Pi per-access service time** is unmeasured (`PI_DAEMON.md` 12.2, 14). Step 5.
- **8228 tRD on CPU-side D4/D6.** tRD (30 ns) is characterized at 25 pF, and D4/D6 carry about 33-38 pF before any probe (est, `ARCHITECTURE.md` 6.12 rule 2). Pre-existing: the analyzer only adds to it, through the 1 kohm isolation. The CPU-side data pins have no datasheet margin in sink current either: on a read the 8228 is rated for IOL 2 mA, exactly the 8080A's active pull-up (IDL 2.0 mA), and the analyzer (<= 10 uA, all eight pins) and on D4/D6 the GAL pin-keeper (40 uA typ, no max, ATF22V10C 0735U 8) sit on top of it. That is why CPU-side D0-D7 get no pull-up (`ARCHITECTURE.md` 6.13). Step 3 runs with the analyzer attached and must pass.
- **NOS chip quality.** Buy spares; never a NEC D8080A without F. Steps 2 and 8.
- **K-1: does I/O6 toggle inside the page-load window?** The AT28C64B datasheet leaves open whether a read inside tBLC, before programming starts, is a polling read (`ARCHITECTURE.md` 6.10 rule 3). Informational: `examples/burn` waits out tBLC before it polls, so it is correct either way. After step 7, with the monitor at its prompt: paste the record below, fit JP-WE, `G 0300`, remove JP-WE, `D 0380 0380`. The program writes FFFF's own byte back (so the ROM does not change), reads it twice at once, stores bit 6 of the XOR of the two reads at 0380 (bits 5-0 of a status read are undefined), then waits out the page load and toggle-polls: `LXI H,FFFF / MOV A,M / MOV M,A / MOV A,M / XRA M / ANI 40 / STA 0380 / MVI B,14 / DCR B / JNZ 030E / MOV A,M / XRA M / ANI 40 / JNZ 0312 / RET`. 40 at 0380: I/O6 toggled inside tBLC (the emulator's default reading). 00: it did not (the literal reading, `set_load_window_cells`), the chip skipped the identical byte, or JP-WE was open. Record the result in `docs/COLLABORATION_LOG.md`. Test: `jp_we_k1_probe`.

  ```
  :1A03000021FFFF7E777EAEE640328003061405C20E037EAEE640C21203C9E4
  ```
