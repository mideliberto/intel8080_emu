# Hardware Build Plan

Non-normative. How to build the first real machine: parts, bring-up order, sourcing and the Pi side. The contract the board must meet is `ARCHITECTURE.md` section 6 (circuits) and `DEVICE_SPECS.md` (ports, READY contract). **Where this file disagrees with them, the specs win**; fix this file.

Chip-level reference (pins, levels, cycle timing): `reference/8080_HARDWARE.md`. Source of this plan: the 2026-10-02 hardware-alignment pass.

---

## 1. Decisions

All accepted by Mike on 2026-10-02. Record and rationale: `COLLABORATION_LOG.md`, Key Decisions ("Hardware Alignment: Buildable, One Hard Fix"). The IDs are the ones the specs cite.

| ID | Outcome |
|----|---------|
| WAIT-SET | WAIT flip-flop set asynchronously while STSTB low AND NOT RESET AND (D4 OR D6) AND port window. SYNC is optional, never the only qualifier. |
| REQ-QUALIFY | REQ = WAIT flip-flop Q AND the 8080A WAIT pin. The Pi sees REQ only in T_W. |
| DIR-SOURCE | DIR = 8228 /I/OR. Data 74LVC245A /OE = NOT /I/OR. No status latch. |
| LATCH | Separate LATCH GPIO clocks the 74HCT374 IN latch (not tied to ACK). |
| PI-GPIO | 20 GPIOs, A0-A6 (A7 is always 0 in the window). Pin map in `ARCHITECTURE.md` 6.4. Optional TEST_RESET on BCM 18. |
| GLUE | One ATF22V10C GAL plus 74HCT74/14/08/125. Discrete HCT is a valid fallback with no spec change. |
| MEMORY-PARTS | AT28C64B-15PU ROM, 2x AS6C62256-55PCN RAM. |
| BUS-RESISTORS | None permanent. Jumpered 2.2 kohm DB pull-down for bring-up only; footprint for a 10 kohm pull-up SIP. |
| POWER | 5 V input, Pololu U3V40F12 (+12 V), ICL7660 (-5 V), Schottky VBB clamp. Pi on its own supply. Current-limited bench +5 V for the first power-up. |
| RESET-SOURCE | DS1813-5 class open-drain supervisor, button on its RST node. |
| TEST-RESET | Footprint fitted; populated only for the unattended test rig. |
| CPU-SOURCING | One Intel/AMD 8080A reference plus two spares; 2x 8224, 2x 8228. |
| CONSTRUCTION | One 2-layer PCB, no backplane. |
| PI-PLATFORM | Pi 4B, busy-poll service (section 5). |
| CONSOLE-TRANSPORT | One TCP listener in the Pi daemon; a new client replaces the old. |
| CONSOLE-OUTPUT | Output buffer of at least 2 MiB; discard on full and on RESET. |
| RESET-TIMING | Abandon the in-flight request on RESET assertion; rebuild devices on release. |
| DEV-RESET | Device reset = rebuild the IoBus and devices from config. |
| TRACE-FORMAT | Line format only: `ARCHITECTURE.md` 7.3. |
| HW-STEP | No hardware single-step in v1 (Someday). The emulator is the debugger. |

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
| AT28C64B-15PU | 1 | 4 KB ROM at F000 (A12, /CE to GND; /WE to VCC) | Microchip, Active, PDIP-28 |
| AS6C62256-55PCN | 2 | RAM 0000-7FFF and 8000-FFFF | Alliance, Active, PDIP-28 |
| ATF22V10C-15PU | 1 | Memory decode, window decode, WAIT set term, FE/FF decode | Microchip, Active, PDIP-24. Program it as plain ATF22V10C, not CEX |
| SN74HCT74 (TI) | 1 | WAIT flip-flop + overlay flip-flop | SN74HCT74N is NRND, SN74HCT74DR (SOIC) is Active. SN74AHCT74N is an Active DIP alternative (faster; timing not re-verified). Nexperia dropped the DIP |
| 74HCT14 | 1 | NOT RESET, /I/OR invert for the data 245 /OE, NOT A15 for the RAM high /CE, 2-stage Schmitt on ACK (1 spare) | Active, common |
| 74HCT08 | 1 | REQ = Q AND 8080A WAIT (3 spare gates) | Active, common |
| 74HCT125 | 1 | IN FF: overlay Q onto DB0 (frees a GAL pin) | Active, common |
| SN74HCT374N | 1 | IN latch, Pi to system data bus | TI, Active |
| SN74LVC245AN (not LVCH) | 3 | 5 V to 3.3 V: A0-A6 + DIR; D0-D7; REQ + RESET. VCC from the Pi 3V3 | TI, Active |
| DS1813-5 (or equivalent open-drain 5 V supervisor, ~4.6 V trip) | 1 | RESIN power-on, brownout and button reset | Analog Devices/Maxim, TO-92; status not re-verified |
| Pushbutton | 2 | Reset; manual ACK (bring-up jig) | Common |
| Pololu U3V40F12 | 1 | +12 V boost from +5 V (4%, <= 12.48 V) | Pololu #4016, about $10 |
| ICL7660 (or ICL7660S/MAX1044) + 2x 10 uF | 1 | -5 V charge pump for VBB (<= 1 mA) | Common |
| BAT43 or BAT85 Schottky | 1 | VBB clamp to GND at the CPU socket | Common |
| 1 kohm resistor | 1 | 8228 INTA (pin 23) to +12 V, RST 7 strap | Common |
| 10 kohm resistor | 1 | INT pull-down (HOLD and BUSEN tied to GND directly) | Common |
| 4.7 kohm resistor | 2 | ACK and LATCH pull-downs, 5 V side | Common |
| 220-330 ohm resistor array (8) | 1 | Series resistors on Pi D0-D7 outputs (crash and contention protection) | Common |
| 2.2 kohm 9-pin bused SIP + jumper | 1 | Bring-up NOP free-run pull-down on DB0-DB7; removed in normal operation | Common |
| 8-position DIP switch | 1 | Manual IN-latch data for the bring-up jig (LATCH and ACK buttons via jumpers) | Common |
| 2N7000 | 1 | Optional Pi TEST_RESET (inverting open-drain on RESIN) | Common |
| 0.1 uF ceramic | 30 | Decoupling, one per IC per rail | Common |
| 10 uF electrolytic | 3 | Bulk per rail (+5, +12, -5) | Common |
| 5 V 2 A regulated supply | 1 | Board input | Common |
| Raspberry Pi 4B + its own PSU + 2x20 short ribbon | 1 | Coprocessor (devices, console TCP, storage) | Active |
| ZIF-40 socket + DIP sockets | 1 | CPU socket (chip tester), sockets for all DIPs | Common |
| 2-layer PCB | 1 | Single board, test points and jumpers per bring-up stage | Fab, about $10-30 |

About 20 ICs in total.

---

## 3. Bring-Up Plan

One stage at a time. Do not start a stage until the previous one passes.

| Step | Pass criterion |
|------|----------------|
| 0. Bare PCB, no ICs. Continuity and rail-to-rail shorts. Power from a current-limited bench +5 V, Pololu and ICL7660 fitted. | +5 V 4.85-5.15, +12 V 11.4-12.6 (<= 12.48), -5 V -4.75..-5.25 at every socket. Single-shot scope of VBB at power-up and power-down stays <= +0.3 V relative to GND. |
| 1. 8224 + crystal + 510 ohm pair + DS1813. Scope phi1, phi2, RESET and STSTB. 10x probes only: phi outputs are not short-circuit protected. | 2.048 MHz +/- crystal offset. phi1 >= 60 ns, phi2 >= 220 ns, phi high >= 9.0 V. RESET held >= 3 clocks (ms) after power-up and on every button press, and active high. |
| 2. Add 8080A (ZIF) + 8228. No memory, RDYIN jumpered high, 2.2k DB pull-down jumper in (floating reads = NOP). BUSEN, HOLD, INT tied. | A0 toggles at 256 kHz, A15 period 128 ms. Supply currents within datasheet max (+12 V <= 87 mA). Screen every CPU this way. |
| 3. Add ROM, RAM, GAL decode and the 74HCT74 overlay half. RDYIN still jumpered high, pull-down out. Burn a small diagnostic image (asl): read IN FF, OUT FE, read IN FF again, march-test 0000-EFFF, HLT at a pass or fail address. Run the same image in the emulator first and dump the debugger instruction trace. | The logic analyzer shows HLT at the pass address. IN FF bit 0 reads 1 then 0. The fetch-address sequence of the first ~200 instructions matches the emulator trace exactly. Scope: DB high level on SRAM reads >= 2.9 V (else fit the pull-up SIP). |
| 4. Add the WAIT-FF set path, the REQ AND gate and the IN latch, with the manual-ACK jig (button -> HCT14, DIP switches + LATCH button on the 374). Burn the real monitor.bin. | The CPU freezes on the first Pi-window access. Today's ROM freezes on IN 02; after the CONOUT fix it is OUT 00. Each ACK press advances exactly one access. The scope shows RDYIN low <= STSTB+167 ns, the STSTB pulse at HCT74 PRE >= 20 ns, and REQ rising only after the 8080 WAIT pin. The LA shows zero REQs on memory cycles over 10^6 cycles and zero phantom REQs across 100 consecutive resets. |
| 5. Connect the Pi 4B (own supply). Day one: measure GPLEV read latency and GPIO write-to-pin timing. Then run a tracer daemon that ACKs every access, serves the console over TCP, and logs the port-trace line format (`ARCHITECTURE.md` 7.3). | The boot port trace and the banner are byte-identical to the emulator's port trace and transcript for the same monitor.bin. Power both boards on in either order, and with the Pi off: the board waits at the first access and continues when the daemon starts. |
| 6. Full device daemon (shared IoBus). Replay the strict-harness transcript files over the TCP console. | Every transcript matches the emulator's output exactly. Ctrl-less paste of an Intel HEX file (Phase 5) loses no bytes. |
| 7. Storage conformance test (`DEVICE_SPECS.md` 3), with stress-ng on the Pi's other cores. Then press RESET during a long W/fsync. | I 0A/09/08 read 00 10 00 after both W and L, and C F000 FFFF 2000 prints nothing. After RESET mid-fsync the machine reboots to the banner, with storage unmounted and no hang. |
| 8. CPU acceptance: load the exercisers from storage with L, using the 8080-asm BDOS shim (0005 JMP to a print routine <= EEFF; 0000 JMP to the monitor warm entry). Run TST8080, 8080PRE, CPUTEST (~2 min), 8080EXM (~3.2 h at 2.048 MHz, est from emulator cycle counts). | All PASS, with CRCs equal to the published values. Repeat for each spare CPU. Once the emulator AC fixes land, the emulator's output must match the silicon output line for line. |

---

## 4. Sourcing

- **Never buy a NEC D8080A without the F suffix.** It has a SUB flag in bit 5 and a different DAA, so it is not Intel-compatible. uPD8080AF is fine.
- The 8080A, 8224 and 8228 are NOS/surplus only. Quality varies. Buy spares: one reference CPU plus two, and two each of 8224 and 8228.
- Accept a chip only after it passes the bring-up step 2 current screen and all four exercisers (step 8). Until then it is a suspect, not a spare.
- 74HCT74: TI SN74HCT74N is NRND and Nexperia no longer makes the DIP. Buy TI stock while it lasts, or use SN74AHCT74N (timing not re-verified).
- 74LVC245A only, never 74LVCH245A: bus-hold fights the Pi on D0-D7.
- ATF22V10C: program it as plain ATF22V10C, not CEX. The ROM programmer is already needed for the AT28C64B.
- Crystal: series-resonant if available. A parallel-resonant 18.432 MHz part gives a small frequency offset, which is harmless because the ROM is timing-independent.

---

## 5. Pi Platform and Service Model

- **Platform:** Raspberry Pi 4B for v1, on its own supply, grounds joined at the header. A Pi 5 also works but its GPIO reads cross PCIe to RP1 and are several times slower (est).
- **OS:** 64-bit Raspberry Pi OS, so `time_t` is 64-bit and the clock does not wrap in 2038.
- **Clock (mailbox `TIME`):** the clock is set only when the kernel reports NTP-synchronized (`adjtimex()` does not return `TIME_ERROR`); until then `TIME` gives 83. The Pi 4B has no RTC, and an added RTC alone would not count. Local time follows the Pi's configured time zone (TZ), set at install. Implemented with the Pi daemon. Normative text: `DEVICE_SPECS.md` 8, TIME clock.
- **Bus service:** one thread busy-polls the mmapped GPIO block through `/dev/gpiomem` on a core isolated with `isolcpus`, and calls the IoBus inline. One GPLEV0 read is an atomic snapshot of every signal. Cost: one core at 100%. Expected sub-us to about 3 us per access (est); measure GPLEV latency on day one (bring-up step 5).
- **RESET:** a gpio character-device (gpio-cdev) both-edge request. The kernel latches the edge, so no pulse is missed, even during an fsync. The bus thread checks it before every ACK (`ARCHITECTURE.md` 6.6).
- **No rppal.** It was archived 2025-07-01.
- **Console:** one TCP listener in the daemon; a new client replaces the old. TCP backpressure is the out-of-band input flow control `DEVICE_SPECS.md` (Console) requires. Use socat to bridge a serial port or USB gadget if wanted.
- **Output buffer:** at least 2 MiB, discarded on full and on RESET (`DEVICE_SPECS.md`, Console).
- **Background work:** mailbox requests run on the other cores, never under READY.
- **Code:** one crate, two binaries. The emulator and the daemon call the same port-mapping function to build the same IoBus.

---

## 6. Residual Risks (Bench Only)

These cannot be closed on paper. Each has a bring-up check.

- **STSTB pulse width at the HCT74 PRE** after the gate path (datasheet >= 40 ns in, 20-24 ns needed). Step 4.
- **Reset-release runt** from the 8224 STSTB/RESET skew could set WAIT as reset ends. Step 4: zero phantom REQs across 100 resets.
- **SRAM VIH at zero datasheet margin** against the 8228 VOH (2.4 V both). Real margin is large because the load is CMOS. Step 3: DB high >= 2.9 V, else fit the pull-up SIP.
- **Pi GPLEV latency** is unmeasured. Step 5.
- **NOS chip quality.** Buy spares; never a NEC D8080A without F. Steps 2 and 8.
