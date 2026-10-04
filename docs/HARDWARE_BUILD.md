# Hardware Build Plan

Non-normative. How to build the first real machine: parts, bring-up order, sourcing and the Pi platform decisions. The contract the board must meet is `ARCHITECTURE.md` section 6 (circuits) and `DEVICE_SPECS.md` (ports, READY contract); the Pi software is `PI_DAEMON.md`. **Where this file disagrees with them, the specs win**; fix this file.

Chip-level reference (pins, levels, cycle timing): `reference/8080_HARDWARE.md`. Source of this plan: the 2026-10-02 hardware-alignment pass.

---

## 1. Decisions

All accepted by Mike on 2026-10-02, except ROM-WE, STATUS-LEDS and LA-HEADER (2026-10-03) and BUS-RESISTORS (revised 2026-10-03). Record and rationale: `COLLABORATION_LOG.md`, Key Decisions ("Hardware Alignment: Buildable, One Hard Fix", "`pi8080d --sim`, the RAM Test Build, Board Debug Aids" and "Board Choices Signed Off; Bus Pull-Ups"). The IDs are the ones the specs cite.

| ID | Outcome |
|----|---------|
| WAIT-SET | WAIT flip-flop set asynchronously while STSTB low AND NOT RESET AND (D4 OR D6) AND port window. SYNC is optional, never the only qualifier. |
| REQ-QUALIFY | REQ = WAIT flip-flop Q AND the 8080A WAIT pin. The Pi sees REQ only in T_W. |
| DIR-SOURCE | DIR = 8228 /I/OR. Data 74LVC245A /OE = NOT /I/OR. No status latch. |
| LATCH | Separate LATCH GPIO clocks the 74HCT374 IN latch (not tied to ACK). |
| PI-GPIO | 20 GPIOs, A0-A6 (A7 is always 0 in the window). Pin map in `ARCHITECTURE.md` 6.4. Optional TEST_RESET on BCM 18. |
| GLUE | One ATF22V10C GAL plus 74HCT74/14/08/125, a second 74HCT08 (HALT term) and a 74HCT138 (ROM /WE gate). Discrete HCT is a valid fallback with no spec change. |
| MEMORY-PARTS | AT28C64B-15PU ROM, 2x AS6C62256-55PCN RAM. |
| BUS-RESISTORS | 10 kohm pull-up SIPs on A0-A15 and system DB0-DB7; none on CPU-side D0-D7 (`ARCHITECTURE.md` 6.13). Jumpered 2.2 kohm DB pull-down for bring-up step 2 only, never fitted together with the socketed DB SIP. |
| POWER | 5 V input, Pololu U3V40F12 (+12 V), ICL7660 (-5 V), Schottky VBB clamp. Pi on its own supply. Current-limited bench +5 V for the first power-up. |
| RESET-SOURCE | DS1813-5 class open-drain supervisor, button on its RST node. |
| TEST-RESET | Footprint fitted; populated only for the unattended test rig. |
| CPU-SOURCING | One Intel/AMD 8080A reference plus two spares; 2x 8224, 2x 8228. |
| CONSTRUCTION | One 2-layer PCB, no backplane. |
| PI-PLATFORM | Pi 4B, busy-poll service (section 5; service model in `PI_DAEMON.md`). |
| CONSOLE-TRANSPORT | One TCP listener in the Pi daemon; a new client replaces the old. |
| CONSOLE-OUTPUT | Output buffer of at least 2 MiB; discard on full and on RESET. |
| RESET-TIMING | Abandon the in-flight request on RESET assertion; rebuild devices on release. |
| DEV-RESET | Device reset = rebuild the IoBus and devices from config. |
| TRACE-FORMAT | Line format only: `ARCHITECTURE.md` 7.3. |
| HW-STEP | No hardware single-step in v1 (Someday). The emulator is the debugger. |
| ROM-WE | AT28C64B /WE: 74HCT138 decode of MEMW to F000-FFFF, through jumper JP-WE, pulled up at the chip. Built open. Programmed with SDP off. The burn routine is Someday. `ARCHITECTURE.md` 6.10. |
| STATUS-LEDS | WAIT, HALT, INTE and HLDA LEDs on 74HCT08 gates. `ARCHITECTURE.md` 6.11. |
| LA-HEADER | Three 2x10 analyzer headers (LA-A, LA-B, LA-C), 16 channels each, GND on pins 17-20; CPU-side D0-D7 through 1 kohm. `ARCHITECTURE.md` 6.12; pin order in 3.1 here. |

---

## 2. Bill of Materials

Availability was checked during the alignment pass and not re-verified since.

| Part | Qty | Role | Availability |
|------|-----|------|--------------|
| 8080A (Intel/AMD P8080A or C8080A) | 1 | CPU, reference chip | NOS/surplus only (eBay, UTSource), est $15-20; not re-verified |
| NEC uPD8080AF or KR580VM80A | 2 | CPU spares | Surplus. Never NEC D8080A without the F suffix (not Intel-compatible) |
| 8224 (or KR580GF24) | 2 | Clock generator and RESET/READY synchronizer (1 spare) | NOS/surplus |
| 8228 (or KR580VK28) | 2 | System controller and bus driver (1 spare) | NOS/surplus. 8238 also works (advanced I/OW/MEMW); recheck the IN-latch enable if used |
| 18.432 MHz crystal, series-resonant fundamental, HC-49 | 1 | 8224 crystal (2.048 MHz CPU) | Common. Most stock parts are parallel-resonant, which gives a small harmless offset |
| 510 ohm resistor | 2 | XTAL1 and XTAL2 to GND (8224 note 1, 18 MHz) | Common |
| AT28C64B-15PU | 1 | 4 KB ROM at F000 (A12, /CE to GND; /WE through JP-WE, `ARCHITECTURE.md` 6.10). Program with software data protection off | Microchip, Active, PDIP-28 |
| AS6C62256-55PCN | 2 | RAM 0000-7FFF and 8000-FFFF | Alliance, Active, PDIP-28 |
| ATF22V10C-15PU | 1 | Memory decode, window decode, WAIT set term, FE/FF decode | Microchip, Active, PDIP-24. Program it as plain ATF22V10C, not CEX |
| SN74HCT74 (TI) | 1 | WAIT flip-flop + overlay flip-flop | SN74HCT74N is NRND, SN74HCT74DR (SOIC) is Active. SN74AHCT74N is an Active DIP alternative (faster; timing not re-verified). Nexperia dropped the DIP |
| 74HCT14 | 1 | NOT RESET, /I/OR invert for the data 245 /OE, NOT A15 for the RAM high /CE, 2-stage Schmitt on ACK (1 spare) | Active, common |
| SN74HCT08N | 2 | U08a: REQ = Q AND 8080A WAIT, plus the WAIT, INTE and HLDA LED buffers. U08b: the HALT term (1 spare gate). `ARCHITECTURE.md` 6.11 | TI, Active, PDIP-14 (SCLS063G orderable table, checked 2026-10-03) |
| SN74HCT138N | 1 | ROM /WE gate (`ARCHITECTURE.md` 6.10) | TI, Active, PDIP-16 (SCLS171F orderable table, checked 2026-10-03) |
| 74HCT125 | 1 | IN FF: overlay Q onto DB0 (frees a GAL pin) | Active, common |
| SN74HCT374N | 1 | IN latch, Pi to system data bus | TI, Active |
| SN74LVC245AN (not LVCH) | 3 | 5 V to 3.3 V: A0-A6 + DIR; D0-D7; REQ + RESET. VCC from the Pi 3V3 | TI, Active |
| DS1813-5 (or equivalent open-drain 5 V supervisor, ~4.6 V trip) | 1 | RESIN power-on, brownout and button reset | Analog Devices/Maxim, TO-92; status not re-verified |
| Pushbutton | 2 | Reset; manual ACK (bring-up jig) | Common |
| Pololu U3V40F12 | 1 | +12 V boost from +5 V (4%, <= 12.48 V) | Pololu #4016, about $10 |
| ICL7660 (or ICL7660S/MAX1044) + 2x 10 uF | 1 | -5 V charge pump for VBB (<= 1 mA) | Common |
| BAT43 or BAT85 Schottky | 1 | VBB clamp to GND at the CPU socket | Common |
| 1 kohm resistor | 1 | 8228 INTA (pin 23) to +12 V, RST 7 strap | Common |
| 10 kohm resistor | 2 | INT pull-down (HOLD and BUSEN tied to GND directly); ROM /WE pull-up | Common |
| 4.7 kohm resistor | 2 | ACK and LATCH pull-downs, 5 V side | Common |
| 220-330 ohm resistor array (8) | 1 | Series resistors on Pi D0-D7 outputs (crash and contention protection) | Common |
| 2.2 kohm 9-pin bused SIP + jumper | 1 | Bring-up NOP free-run pull-down on DB0-DB7; removed in normal operation | Common |
| 10 kohm 9-pin bused SIP (8 resistors, common pin to +5 V), 2% | 3 | Bus pull-ups: A0-A7, A8-A15, system DB0-DB7 (`ARCHITECTURE.md` 6.13) | Common |
| 9-pin 0.1-inch SIP socket (or socket strip) | 1 | DB pull-up SIP: out while the 2.2 kohm pull-down is in (step 2) | Common |
| 8-position DIP switch | 1 | Manual IN-latch data for the bring-up jig (LATCH and ACK buttons via jumpers) | Common |
| 2N7000 | 1 | Optional Pi TEST_RESET (inverting open-drain on RESIN) | Common |
| Low-current 3 mm LED (`ARCHITECTURE.md` 6.11) | 4 | Status LEDs WAIT, HALT, INTE, HLDA | Common |
| 1.0 kohm resistor | 4 | LED series resistors | Common |
| 1 kohm isolated 8-resistor array (16-pin DIP) | 1 | Analyzer isolation on the CPU-side D0-D7 taps (`ARCHITECTURE.md` 6.12) | Common |
| 2x10 0.1-inch header (a shrouded box header also takes single leads) | 3 | Logic-analyzer headers LA-A, LA-B, LA-C (3.1) | Common |
| 2-pin 0.1-inch header + shunt | 1 | JP-WE ROM write enable. Built open, with the shunt parked on one pin | Common |
| 0.1 uF ceramic | 32 | Decoupling, one per IC per rail | Common |
| 10 uF electrolytic | 3 | Bulk per rail (+5, +12, -5) | Common |
| 5 V 2 A regulated supply | 1 | Board input | Common |
| Raspberry Pi 4B + its own PSU + 2x20 short ribbon | 1 | Coprocessor (devices, console TCP, storage) | Active |
| ZIF-40 socket + DIP sockets | 1 | CPU socket (chip tester), sockets for all DIPs | Common |
| 2-layer PCB | 1 | Single board, test points and jumpers per bring-up stage | Fab, about $10-30 |

About 22 ICs in total.

---

## 3. Bring-Up Plan

One stage at a time. Do not start a stage until the previous one passes.

| Step | Pass criterion |
|------|----------------|
| 0. Bare PCB, no ICs. Continuity and rail-to-rail shorts. Power from a current-limited bench +5 V, Pololu and ICL7660 fitted. | +5 V 4.85-5.15, +12 V 11.4-12.6 (<= 12.48), -5 V -4.75..-5.25 at every socket. Single-shot scope of VBB at power-up and power-down stays <= +0.3 V relative to GND. |
| 1. 8224 + crystal + 510 ohm pair + DS1813. Scope phi1, phi2, RESET and STSTB. 10x probes only: phi outputs are not short-circuit protected. | 2.048 MHz +/- crystal offset. phi1 >= 60 ns, phi2 >= 220 ns, phi high >= 9.0 V. RESET held >= 3 clocks (ms) after power-up and on every button press, and active high. |
| 2. Add 8080A (ZIF) + 8228. No memory, RDYIN jumpered high, 2.2k DB pull-down jumper in (floating reads = NOP) and the DB pull-up SIP out of its socket (never both, `ARCHITECTURE.md` 6.13). Address pull-up SIPs fitted. BUSEN, HOLD, INT tied. | A0 toggles at 256 kHz, A15 period 128 ms. Supply currents within datasheet max (+12 V <= 87 mA). Screen every CPU this way. |
| 3. Add ROM, RAM, GAL decode and the 74HCT74 overlay half, the 74HCT138 with JP-WE open, and U08a with the WAIT, INTE and HLDA LEDs (its REQ gate unused until step 4). RDYIN still jumpered high, pull-down out, DB pull-up SIP in. Burn a small diagnostic image (asl): read IN FF, OUT FE, read IN FF again, march-test 0000-EFFF (HLT at a fail address), then copy a short loop to 0100 and run it from RAM: for each byte of F000-FFFF, read it, write it back and read it again, HLT at a fail address if the two reads differ, else HLT at the pass address after FFFF. If a wiring fault reaches ROM /WE despite JP-WE open, the first write starts a write cycle of the byte's own value, and for up to tWC = 10 ms reads return data polling, I/O7 complemented (AT28C64B DS 4.2, 4.4), so the loop stops at F000 before writing anything wrong. Run the same image in the emulator first and dump the debugger instruction trace. The analyzer is attached throughout (3.1). | The analyzer shows status A2 at the pass address, then 8A at pass+1, then no further /STSTB. IN FF bit 0 reads 1 then 0. The fetch-address sequence of the first ~200 instructions matches the emulator trace exactly (3.1). LA-C shows a /Y7 low pulse for each of the 4096 writes to F000-FFFF and none during the march test, and ROM /WE (pin 27, on a scope or the spare channel) never goes low. At the HLT: WAIT lit, INTE and HLDA dark. Scope: DB high level during SRAM writes (the march test), where the 8228 drives DB at TTL levels into the SRAM inputs, >= 2.9 V. If it is lower, check that the DB pull-up SIP is fitted in its socket before going on. |
| 4. Add the WAIT-FF set path, the REQ AND gate, the IN latch, and U08b with the HALT LED, with the manual-ACK jig (button -> HCT14, DIP switches + LATCH button on the 374). Burn the real monitor.bin. | The CPU freezes on the first Pi-window access, the banner's OUT 00. Each ACK press advances exactly one access. The scope shows RDYIN low <= STSTB+167 ns, the STSTB pulse at HCT74 PRE >= 20 ns, and REQ rising only after the 8080 WAIT pin. The LA shows zero REQs on memory cycles over 10^6 cycles and zero phantom REQs across 100 consecutive resets (LA-C alone: REQ never high while /MEMR or /MEMW is low). While frozen: WAIT lit, HALT dark; between ACK presses LA-B shows WAIT low only between accesses. |
| 5. Connect the Pi 4B (own supply) and install `pi8080d`: cross-build it on the Mac with `cargo build --release --target aarch64-unknown-linux-musl --bin pi8080d` (`PI_DAEMON.md` 2), copy it to `/usr/local/bin/`, and install `scripts/pi8080d.service` (`PI_DAEMON.md` 11). Day one: with the board powered, scope REQ/ACK/LATCH for the per-access service time (`PI_DAEMON.md` 12.2). Then run `pi8080d --trace` for the boot port trace (`ARCHITECTURE.md` 7.3) and the banner over the TCP console. The rest of this step's bench checks: `PI_DAEMON.md` 14. | The boot port trace and the banner are byte-identical to the emulator's port trace and transcript for the same monitor.bin. Power both boards on in either order, and with the Pi off: the board waits at the first access (WAIT steady, HALT dark) and continues when the daemon starts. `F 0200 0200 76` then `G 0200`: WAIT and HALT steady, until RESET. |
| 6. The same `pi8080d` (shared IoBus). Replay the strict-harness transcript files over the TCP console (in `cargo test` they already pass through the daemon on the simulated board, `PI_DAEMON.md` 13.2). | Every transcript matches the emulator's output exactly. Ctrl-less paste of an Intel HEX file (Phase 5) loses no bytes. |
| 7. Storage conformance test (`DEVICE_SPECS.md` 3), with stress-ng on the Pi's other cores. Then press RESET during a long W/fsync. | I 0A/09/08 read 00 10 00 after both W and L, and C F000 FFFF 2000 prints nothing. After RESET mid-fsync the machine reboots to the banner, with storage unmounted and no hang. |
| 8. CPU acceptance: load the exercisers from storage with L, using the 8080-asm BDOS shim (0005 JMP to a print routine <= EEFF; 0000 JMP F000, a cold start, since WARM has no published address: ARCHITECTURE 2). Run TST8080, 8080PRE, CPUTEST (~2 min), 8080EXM (~3.2 h at 2.048 MHz, est from emulator cycle counts). | All PASS, with CRCs equal to the published values. Repeat for each spare CPU. Once the emulator AC fixes land, the emulator's output must match the silicon output line for line. |


ROM changes can run on the board before a burn: the RAM test build (`ARCHITECTURE.md` 2.1), loaded through the resident monitor from step 5 on.

### 3.1 Logic Analyzer

The analyzer and the three headers it plugs into (`ARCHITECTURE.md` 6.12). On each 2x10 header, pin n carries channel n-1 for n = 1-16, and pins 17-20 are GND. Pin 1 has the square pad, and the signal names are on the silkscreen. Use at least two GND leads per header.

**LA-A: address**

| Pin | Ch | Signal | Source |
|-----|----|--------|--------|
| 1 | 0 | A0 | 8080A pin 25 |
| 2 | 1 | A1 | 8080A pin 26 |
| 3 | 2 | A2 | 8080A pin 27 |
| 4 | 3 | A3 | 8080A pin 29 |
| 5 | 4 | A4 | 8080A pin 30 |
| 6 | 5 | A5 | 8080A pin 31 |
| 7 | 6 | A6 | 8080A pin 32 |
| 8 | 7 | A7 | 8080A pin 33 |
| 9 | 8 | A8 | 8080A pin 34 |
| 10 | 9 | A9 | 8080A pin 35 |
| 11 | 10 | A10 | 8080A pin 1 |
| 12 | 11 | A11 | 8080A pin 40 |
| 13 | 12 | A12 | 8080A pin 37 |
| 14 | 13 | A13 | 8080A pin 38 |
| 15 | 14 | A14 | 8080A pin 39 |
| 16 | 15 | A15 | 8080A pin 36 |
| 17-20 | - | GND | |

**LA-B: CPU-side data and CPU timing**

| Pin | Ch | Signal | Source |
|-----|----|--------|--------|
| 1 | 0 | D0 | 8080A pin 10, through 1 kohm |
| 2 | 1 | D1 | 8080A pin 9, through 1 kohm |
| 3 | 2 | D2 | 8080A pin 8, through 1 kohm |
| 4 | 3 | D3 | 8080A pin 7, through 1 kohm |
| 5 | 4 | D4 | 8080A pin 3, through 1 kohm |
| 6 | 5 | D5 | 8080A pin 4, through 1 kohm |
| 7 | 6 | D6 | 8080A pin 5, through 1 kohm |
| 8 | 7 | D7 | 8080A pin 6, through 1 kohm |
| 9 | 8 | SYNC | 8080A pin 19 |
| 10 | 9 | DBIN | 8080A pin 17 |
| 11 | 10 | /WR | 8080A pin 18 |
| 12 | 11 | READY | 8224 pin 4 (= 8080A pin 23) |
| 13 | 12 | WAIT | 8080A pin 24 |
| 14 | 13 | /STSTB | 8224 pin 7 |
| 15 | 14 | phi2 (TTL) | 8224 pin 6 |
| 16 | 15 | RESET | 8224 pin 1 |
| 17-20 | - | GND | |

**LA-C: system strobes, flip-flops, Pi handshake**

| Pin | Ch | Signal | Source |
|-----|----|--------|--------|
| 1 | 0 | /MEMR | 8228 pin 24 |
| 2 | 1 | /MEMW | 8228 pin 26 |
| 3 | 2 | /I/OR | 8228 pin 25 |
| 4 | 3 | /I/OW | 8228 pin 27 |
| 5 | 4 | INTE | 8080A pin 16 |
| 6 | 5 | HLDA | 8080A pin 21 |
| 7 | 6 | /Q | WAIT flip-flop /Q at the 74HCT74 (RDYIN once step 4 removes the jumper) |
| 8 | 7 | REQ | U08a gate 1 output (5 V side) |
| 9 | 8 | ACK | 5 V board side, at the 74HCT14 input (3.3 V level) |
| 10 | 9 | LATCH | 74HCT374 CLK (3.3 V level) |
| 11 | 10 | OVL | overlay flip-flop Q |
| 12 | 11 | ROM /OE | ROM_OE net |
| 13 | 12 | ROM_WE decode | 74HCT138 /Y7 (JP-WE pin 1) |
| 14 | 13 | HALT | U08b gate 3 output |
| 15 | 14 | /RESIN | 8224 pin 2 (supervisor and button node) |
| 16 | 15 | spare | pad, unconnected (ROM /WE pin 27 for the step 3 check) |
| 17-20 | - | GND | |

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

---

## 4. Sourcing

- **Never buy a NEC D8080A without the F suffix.** It has a SUB flag in bit 5 and a different DAA, so it is not Intel-compatible. uPD8080AF is fine.
- The 8080A, 8224 and 8228 are NOS/surplus only. Quality varies. Buy spares: one reference CPU plus two, and two each of 8224 and 8228.
- Accept a chip only after it passes the bring-up step 2 current screen and all four exercisers (step 8). Until then it is a suspect, not a spare.
- 74HCT74: TI SN74HCT74N is NRND and Nexperia no longer makes the DIP. Buy TI stock while it lasts, or use SN74AHCT74N (timing not re-verified).
- 74LVC245A only, never 74LVCH245A: bus-hold fights the Pi on D0-D7.
- ATF22V10C: program it as plain ATF22V10C, not CEX. The ROM programmer is already needed for the AT28C64B.
- AT28C64B: program it with software data protection off. Parts ship that way (AT28C64B DS 4.6.2, https://ww1.microchip.com/downloads/en/DeviceDoc/doc0270.pdf). If the programmer offers an SDP option, leave it off: with A12 tied low the 8080 cannot clear SDP in circuit (`ARCHITECTURE.md` 6.10 rule 5).
- SN74HCT08N and SN74HCT138N: TI, Active in PDIP (orderable tables in SCLS063G and SCLS171F, checked 2026-10-03).
- Crystal: series-resonant if available. A parallel-resonant 18.432 MHz part gives a small frequency offset, which is harmless because the ROM is timing-independent.

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
