# Parts Order

Non-normative. What to buy for one board, its bring-up, the step 4 jig and the Pi side. The board is `hw/board.net.txt`: every board line below is keyed by its reference designators, and `tests/netlist_tests.rs` checks that every netlist refdes is in exactly one Refs cell, that every Refs entry is in the netlist, and that each line orders at least its refdes count. The design intent is `HARDWARE_BUILD.md` 2 (BOM); **where this file disagrees with the specs, the specs win**; fix this file.

Checked 2026-10-04. Distributors: Mouser primary, Pololu direct for the boost module, DigiKey as the fallback for a line Mouser is out of, NOS sellers for the 8080A, 8224 and 8228 (decided 2026-10-04).

---

## 1. Conventions

- **Refs** are netlist designators. A ranged entry (U14-U16) covers every number in it. A line that serves no netlist part (sockets, shunts, supply, jig, Pi side) has `-` and says what it serves in Notes.
- **Need** is the refdes count; on a `-` line, the count for what it serves. **Order** includes spares: +1 per IC line (the EEPROM spare is burned with the current `monitor.bin` and labelled as the recovery image); passives rounded up to 10, which costs less than a second shipment. Order counts header pins on the TP line, parts on every other line.
- **Status** is the lifecycle, the source and the date it was read. "Distributor data" means a distributor or aggregator listing seen through a search, not the manufacturer's page. **Unverified** means not checked in this pass.
- **Mouser PN** marked (confirm) was not read from Mouser itself, which blocks automated reads; it follows Mouser's prefix for the maker. Check every line in the cart: the part, the package and the suffix. Stock and prices are not recorded; they go stale in days.

---

## 2. Mouser

### 2.1 ICs

| Refs | Part | MPN | Mouser PN | Need | Order | Status | Notes |
|------|------|-----|-----------|------|-------|--------|-------|
| U4 | 8 KB parallel EEPROM, PDIP-28 | AT28C64B-15PU | 556-AT28C64B15PU | 1 | 2 | Active, lifecycle Production (distributor data, 2026-10-04); Mouser lists it | Program with SDP off (`HARDWARE_BUILD.md` 4). Spare = recovery image. DigiKey has refused AT28C64B sales to non-manufacturers (forum.digikey.com/t/substitute-part/70309, 2026-07-06). |
| U5, U6 | 32K x 8 SRAM, PDIP-28 600 mil | AS6C62256-55PCN | 913-AS6C62256-55PCN (confirm) | 2 | 3 | Active (distributor data, 2026-10-04); Mouser lists it | |
| U7 | GAL, PDIP-24 300 mil | ATF22V10C-15PU | 556-AF22V10C15PU (confirm) | 1 | 2 | Active, lifecycle Production (distributor data, 2026-10-04) | Mouser spells it AF22V10C. Program as plain ATF22V10C, never CEX or PWD (`HARDWARE_BUILD.md` 2.1). |
| U8 | Dual D flip-flop, PDIP-14 | CD74HCT74E | 595-CD74HCT74E | 1 | 2 | Active (ti.com, 2026-10-04) | Replaces the NRND SN74HCT74N. Timing: `HARDWARE_BUILD.md` 4. |
| U9 | Hex Schmitt inverter, PDIP-14 | SN74HCT14N | 595-SN74HCT14N | 1 | 2 | Active (ti.com, 2026-10-04) | |
| U10, U11 | Quad AND, PDIP-14 | SN74HCT08N | 595-SN74HCT08N | 2 | 3 | Active (ti.com, 2026-10-04) | U08a, U08b. |
| U12 | 3-to-8 decoder, PDIP-16 | SN74HCT138N | 595-SN74HCT138N | 1 | 2 | Active (ti.com, 2026-10-04) | |
| U13 | Octal D flip-flop, PDIP-20 | SN74HCT374N | 595-SN74HCT374N | 1 | 2 | Active (ti.com, 2026-10-04) | IN latch. |
| U14-U16 | Octal transceiver, PDIP-20 | SN74LVC245AN | 595-SN74LVC245AN | 3 | 4 | Active (ti.com, 2026-10-04) | **Not LVCH**: check the cart line. |
| U17 | -5 V charge pump, PDIP-8 | MAX1044CPA+ | 700-MAX1044CPA+ (confirm) | 1 | 2 | Production (distributor data, 2026-10-04); stocked by ADI direct and Newark | ICL7660 class, same pinout (decision POWER). |
| U18 | Reset supervisor, TO-92 | DS1813-5+ | 700-DS1813-5+ | 1 | 2 | Production (distributor data, 2026-10-04; analog.com timed out, not read); Mouser lists 700-DS1813-5+ (aggregator data) | "+" is the RoHS ordering code; one broker lists the leaded DS1813-5 as obsolete. Confirm the status in the cart. |

### 2.2 Discretes and crystal

| Refs | Part | MPN | Mouser PN | Need | Order | Status | Notes |
|------|------|-----|-----------|------|-------|--------|-------|
| Q1 | NPN, TO-92 (E B C) | onsemi 2N3904BU | 512-2N3904BU | 1 | 2 | Active (distributor data, 2026-10-04) | TEST_RESET. Footprint fitted; populated for the test rig only. |
| Q2 | P-MOSFET, TO-220AB (G D S) | Vishay SUP53P06-20-E3 | 78-SUP53P06-20-GE3 (confirm) | 1 | 2 | No EOL notice; vishay.com lists SUP53P06-20-E3 as the orderable code (2026-10-04) | Reverse-polarity switch (`ARCHITECTURE.md` 6.9). Mouser's listing carries the GE3 suffix (aggregator data): confirm in the cart that it is the SUP53P06-20 in TO-220AB. |
| D1 | Schottky, DO-35 | Vishay BAT43-TAP | 78-BAT43-TAP (confirm) | 1 | 10 | Production (distributor data, 2026-10-04) | VBB clamp. |
| D2-D5 | Low-current 3 mm red LED | Broadcom HLMP-1700 | 630-HLMP-1700 (confirm) | 4 | 10 | Unverified | Characterized at 2 mA, Vf 1.7 V typ (distributor data). Confirm the datasheet maximum Vf at 2 mA against `ARCHITECTURE.md` 6.11. |
| Y1 | 18.432 MHz crystal, series, fundamental, HC-49/US | CTS ATS184-E | 774-ATS184-E | 1 | 1 | CTS DOC# 008-0309-0 rev N (read 2026-10-04); Mouser lists it (search of Mouser pages) | Datasheet: series load, ESR <= 40 ohm, drive level 100 uW typ and 1000 uW max, +/-30 ppm at 25 C, +/-50 ppm over -40..85 C. Step 1 checks start-up and drive level (`HARDWARE_BUILD.md` 3). |
| - | 18.432 MHz crystal, parallel 18 pF, HC-49/US | CTS ATS184B-E | 774-ATS184B-E | 1 | 1 | Same datasheet; Mouser lists it | Fallback for Y1 if the series part will not start (step 1). |

### 2.3 Resistors, networks, capacitors

| Refs | Part | MPN | Mouser PN | Need | Order | Status | Notes |
|------|------|-----|-----------|------|-------|--------|-------|
| R1, R2 | 510 ohm 1/4 W, axial | Yageo MFR-25FBF52-510R | 603-MFR-25FBF52-510R (confirm) | 2 | 10 | Unverified | 8224 XTAL pair. |
| R3, R8-R11 | 1 kohm 1/4 W, axial | Yageo MFR-25FBF52-1K | 603-MFR-25FBF52-1K (confirm) | 5 | 10 | Unverified | INTA strap, LED resistors. Also the jig's two 1 kohm (section 6). |
| R4, R5, R13, R14 | 10 kohm 1/4 W, axial | Yageo MFR-25FBF52-10K | 603-MFR-25FBF52-10K (confirm) | 4 | 10 | Unverified | |
| R6, R7, R12 | 4.7 kohm 1/4 W, axial | Yageo MFR-25FBF52-4K7 | 603-MFR-25FBF52-4K7 (confirm) | 3 | 10 | Unverified | Also the step 0 VBB load (`HARDWARE_BUILD.md` 3). |
| RN1-RN3 | 10 kohm x8, bused, SIP-9, 2% | Bourns 4609X-101-103LF | 652-4609X-1LF-10K (confirm) | 3 | 5 | Unverified | Bus pull-ups. One more for the jig (section 6). |
| RN4 | 2.2 kohm x8, bused, SIP-9 | Bourns 4609X-101-222LF | 652-4609X-1LF-2.2K (confirm) | 1 | 2 | Unverified | Step 2 DB pull-down. |
| RN5 | 1 kohm x8, isolated, DIP-16 | Bourns 4116R-1-102LF | 652-4116R-1LF-1K (confirm) | 1 | 2 | Unverified | Analyzer isolation. Soldered. |
| RN6 | 330 ohm x8, isolated, DIP-16 | Bourns 4116R-1-331LF | 652-4116R-1LF-330 (confirm) | 1 | 2 | Unverified | Pi D0-D7 series array. Soldered. |
| C1-C21 | 0.1 uF X7R 50 V, 5.08 mm leads | KEMET C322C104K5R5TA | 80-C322C104K5R (confirm) | 21 | 30 | Unverified | Fits the 5.00 mm disc footprint. |
| C22-C25 | 10 uF 25 V aluminium, 5 mm dia, 2.0 mm pitch | Panasonic EEU-FC1E100 | 667-EEU-FC1E100 (confirm) | 4 | 10 | Unverified | C25 is the -5 V reservoir: its + lead goes to GND (netlist). |

### 2.4 Electromechanical

| Refs | Part | MPN | Mouser PN | Need | Order | Status | Notes |
|------|------|-----|-----------|------|-------|--------|-------|
| SW1 | 6 mm tactile, THT | Omron B3F-1000 | 653-B3F-1000 (confirm) | 1 | 4 | Unverified | Reset. Two more for the jig (section 6). |
| J1 | 2x20 shrouded box header, 2.54 mm | Wurth 61204021621 | 710-61204021621 (confirm) | 1 | 1 | Unverified | Board end of the Pi ribbon (keyed). |
| J2-J4 | 2x10 shrouded box header, 2.54 mm | Wurth 61202021621 | 710-61202021621 (confirm) | 3 | 3 | Unverified | LA-A, LA-B, LA-C. |
| J5 | DC power jack, PCB, right angle | Same Sky PJ-002AH | 490-PJ-002AH (confirm) | 1 | 1 | Same Sky datasheet dated 09/12/2024 (read 2026-10-04) | 2.0 mm centre pin, 5.0 A. Check the fit of the adapter's 5.5 x 2.1 mm plug and the footprint before fab (`HARDWARE_BUILD.md` 2.2, fab gate). |
| JP1, JP2 | 2-pin 0.1-inch header | Wurth 61300211121 | 710-61300211121 (confirm) | 2 | 4 | Unverified | JP-WE, JP-PD. |
| - | Shunt, 0.1-inch | Wurth 60900213421 | 710-60900213421 (confirm) | 2 | 4 | Unverified | For JP1 (parked on one pin) and JP2. |
| TP1-TP9 | 0.1-inch single-row breakaway header, 40 pins | Wurth 61304011121 | 710-61304011121 (confirm) | 9 | 40 | Unverified | One pin per test point; Order is in pins (one strip). |
| H1-H4 | M3 standoff, nylon, about 10 mm, with screws | any | - | 4 | 4 | Unverified | Feet at the four mounting holes. |
| - | ZIF-40, 0.6 inch | Aries 40-6554-10 | 535-40-6554-10 (confirm) | 1 | 1 | Unverified | For U1 (the chip tester). Body and lever need clearance (`HARDWARE_BUILD.md` 2.3). |
| - | Machined DIP socket, 8-pin 0.3 | Mill-Max 110-43-308-41-001000 | 575-11043308 (confirm) | 1 | 1 | Unverified | For U17. |
| - | Machined DIP socket, 14-pin 0.3 | Mill-Max 110-43-314-41-001000 | 575-11043314 (confirm) | 4 | 4 | Unverified | For U8-U11. |
| - | Machined DIP socket, 16-pin 0.3 | Mill-Max 110-43-316-41-001000 | 575-11043316 (confirm) | 2 | 2 | Unverified | For U2, U12. |
| - | Machined DIP socket, 20-pin 0.3 | Mill-Max 110-43-320-41-001000 | 575-11043320 (confirm) | 4 | 4 | Unverified | For U13-U16. |
| - | Machined DIP socket, 24-pin 0.3 | Mill-Max 110-43-324-41-001000 | 575-11043324 (confirm) | 1 | 2 | Unverified | For U7. The spare is for the chip that comes out for programming. |
| - | Machined DIP socket, 28-pin 0.6 | Mill-Max 110-43-628-41-001000 | 575-11043628 (confirm) | 4 | 5 | Unverified | For U3-U6. The spare is for the ROM, which comes out for programming. |
| - | Machined SIP socket strip, 9 pins used | Mill-Max 310-43-109-41-001000 | 575-31043109 (confirm) | 1 | 1 | Unverified | For RN3, which is out for step 2. |

### 2.5 Board supply

| Refs | Part | MPN | Mouser PN | Need | Order | Status | Notes |
|------|------|-----|-----------|------|-------|--------|-------|
| - | 5 V 3 A adapter, 5.5 x 2.1 mm plug, centre positive | Mean Well GST18A05-P1J | 709-GST18A05-P1J (confirm) | 1 | 1 | Listed by Jameco, Distrelec, element14 (2026-10-04) | Tolerance and load regulation +/-5% (distributor data), so it is checked loaded before step 1 (`HARDWARE_BUILD.md` 3). IEC C14 inlet: needs a C13 mains cord. |

---

## 3. Pololu Direct

| Refs | Part | MPN | Pololu PN | Need | Order | Status | Notes |
|------|------|-----|-----------|------|-------|--------|-------|
| PS1 | +12 V boost module | Pololu U3V40F12 | #4016 | 1 | 2 | Active and Preferred (pololu.com, 2026-10-04) | 12 V, 4%. VIN and GND doubled, VOUT, EN; the pin order is labelled on the back, not in the page text. Buy before the PCB goes to fab and read the order off the module (`HARDWARE_BUILD.md` 2.2, fab gate). 6x1 header strip included. The spare: the board has no other +12 V source. Not stocked by Mouser. |

---

## 4. NOS and Surplus: 8080A, 8224, 8228

Order these when the PCB goes to fab, so the return windows run while the board can test them. Every chip is a suspect until it passes `HARDWARE_BUILD.md` 3 step 2 and step 8; only then is it accepted. Typical sources: eBay vintage sellers, UTSource, RC Freelance, Jameco (occasionally).

| Refs | Part | Acceptable | Need | Order | Notes |
|------|------|------------|------|-------|-------|
| U1 | 8080A, reference | Intel or AMD (P, D or C package; -1 and -2 grades too) | 1 | 1 | Decision CPU-SOURCING. Faster grades run at tCY 488 ns. |
| - | 8080A, spares | Any make, except a NEC D8080A without the F suffix. KR580VM80A is fine. | 2 | 2 | NEC D8080A / uPD8080A without F has a SUB flag in bit 5 and a different DAA. The trap is the NEC logo: an Intel D8080A is a ceramic Intel part and is fine; NEC uPD8080AF is fine. |
| U2 | 8224 clock generator | Intel or AMD 8224, KR580GF24 | 1 | 2 | One spare. |
| U3 | 8228 system controller | Intel or AMD 8228, KR580VK28 | 1 | 2 | One spare. **Never an 8238 or KR580VK38**: its advanced /MEMW and /I/OW fall outside the 8228 timing that `ARCHITECTURE.md` 6.10 and 6.11 assume. |

What to check in a listing:

- A photo of the actual top marking, not a stock picture. Reject NEC-logo D8080A or uPD8080A without F; reject an 8238 offered as "8228 compatible".
- Date code: a 2010s code on an "Intel 8080A" is a fake.
- Straight, uncorroded pins; no sanded or re-inked tops.
- KR580 parts: the seller ships to the US, and the marking is КР580ВМ80А, КР580ГФ24 or КР580ВК28.
- A return window, and the seller's acceptance of a return after testing.

---

## 5. Pi Side

| Refs | Part | MPN | Source | Need | Order | Status | Notes |
|------|------|-----|--------|------|-------|--------|-------|
| - | Raspberry Pi 4 Model B, 2 GB or more | Raspberry Pi 4B | any authorized reseller | 1 | 1 | Unverified | Skip if one is on hand (`HARDWARE_BUILD.md` 5). |
| - | Pi 4 USB-C supply, 5.1 V 3 A | Raspberry Pi official 15.3 W | any | 1 | 1 | Unverified | Its own supply (`HARDWARE_BUILD.md` 5). |
| - | Passive heatsink or heatsink case | any | any | 1 | 1 | Unverified | One core runs at 100% (`PI_DAEMON.md` 11; USER_GUIDE). |
| - | 2x20 IDC ribbon, female-female, short | Adafruit 1988 | Adafruit, or Mouser 485-1988 (confirm) | 1 | 1 | Unverified | The Pi header is unshrouded: mark pin 1 on the Pi-end connector. |
| - | microSD, 16-32 GB, A1 | name brand | any | 1 | 1 | Unverified | OS and the storage directory. |

---

## 6. Manual-ACK Jig (Off-Board)

`HARDWARE_BUILD.md` 3.2. Parts not listed here come from the board lines: two 1 kohm (R3 line), the 10 kohm bused SIP (RN1-RN3 line), two pushbuttons (SW1 line).

| Refs | Part | MPN | Mouser PN | Need | Order | Status | Notes |
|------|------|-----|-----------|------|-------|--------|-------|
| - | 2x20 IDC socket on a short ribbon stub | any 2x20 2.54 mm IDC socket and 40-way ribbon | - | 1 | 1 | Unverified | Connects only GND, BCM 16, 17 and 20-27 (`HARDWARE_BUILD.md` 3.2). |
| - | 1 uF X7R 50 V, 5.08 mm leads | KEMET C322C105K5R5TA | 80-C322C105K5R (confirm) | 1 | 1 | Unverified | ACK debounce. |
| - | 8-position DIP switch | CTS 206-8ST | 774-2068ST (confirm) | 1 | 1 | Unverified | D0-D7. |

---

## 7. Tools (Not Ordered Here; Mark What Is on Hand)

| Item | Needed for | Note |
|------|------------|------|
| EEPROM/GAL programmer | ROM, GAL, burn recovery | XGecu T48 or TL866II Plus through minipro. Before buying: it must accept galette's `hw/glue.jed` (`Device: GAL22V10`, `*QF5892`) for an ATF22V10C without conversion, or the GAL proof covers a file other than the one burned. Not tested here. |
| Oscilloscope, 2 channels, single-shot, about 100 MHz, 10x probes | steps 0-5 | Step 1 requires 10x probes. |
| Current probe, or a way to measure crystal current | step 1 | Crystal drive level (`HARDWARE_BUILD.md` 3). |
| DMM | steps 0, 5 | Continuity, rails, supply polarity. |
| Bench supply, current-limited +5 V | step 0 | |
| 4.7 ohm 10 W resistor | before step 1 | Load for the adapter check (`HARDWARE_BUILD.md` 3). |
| C13 mains cord | board supply | The adapter has a C14 inlet. |
| Logic analyzer | steps 3-5 | `HARDWARE_BUILD.md` 3.1. |
