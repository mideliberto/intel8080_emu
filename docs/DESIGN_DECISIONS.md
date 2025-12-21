# Design Decisions

The 12 key architectural decisions for this project.

---

## 1. Memory Layout

**Decision:** High ROM (0xF000-0xFFFF) with vectors at bottom

```
0x0000-0x007F   Vectors + API table (RAM, copied from ROM)
0x0080-0x00FF   System workspace
0x0100-0xEFFF   User programs
0xF000-0xFFFF   Monitor ROM
```

**Rationale:** Clean separation. ROM at top stays out of the way. Vectors at bottom where the 8080 expects them. 0x0100 is a nice round address to start user code. Simple.

---

## 2. Boot Mechanism

**Decision:** ROM overlay with bank switching

**Problem:** 8080 starts at 0x0000 but ROM lives at 0xF000.

**Solution:** On reset, ROM appears at BOTH 0x0000 and 0xF000. ROM code jumps to 0xF000+ range, then disables overlay via OUT to port 0xFE. Low memory becomes RAM.

**Rationale:** Proven pattern from real S-100 systems. Works in emulation today, works on real hardware tomorrow. One ROM binary, no tricks.

---

## 3. CPU Speed

**Decision:** Configurable, default 2.0 MHz

**Rationale:** Authentic 8080 speed. Configurable for faster testing or slower debugging. When we hit real hardware, this becomes actual clock speed.

---

## 4. Storage Architecture

**Decision:** Linear-addressed (not track/sector)

```
Interface: 16-bit address, 8-bit data, auto-increment
Protocol: OUT addr_lo, OUT addr_hi, IN/OUT data
```

**Rationale:**
- Linear addressing maps directly to SD card / EEPROM
- No fake disk geometry to emulate
- Hardware implementation is trivial: 74LS193 counter + 74LS374 latch
- Same code works on emulator and real hardware

**Trade-off:** Can't run CP/M disk images. We're not building a CP/M machine.

---

## 5. API Entry Points

**Decision:** Hybrid RST + CALL approach

**RST Vectors (1-byte calls):**
- RST 1: CONOUT
- RST 2: CONIN
- RST 3: CONST
- RST 7: Timer interrupt

**API Table (0x0040+):**
- PRINT_STRING, PRINT_HEX_*, READ_HEX_*, etc.

**Rationale:** RST is compact (1 byte) for hot-path calls. API table at fixed addresses for everything else. User code can call either.

---

## 6. Register Preservation

**Decision:** Hybrid - document what each function trashes

**Rules:**
- Flags: Always trashed
- A: Preserved if input-only, trashed if return value
- HL: Preserved unless it's a return value
- BC, DE: Preserved unless documented

**Every function documents its register usage.**

**Rationale:** Flexible. Honest. No surprises. Better than pretending everything is preserved when it isn't.

---

## 7. Monitor Commands

**Decision:** Classic command set, familiar syntax

**Implemented:** D, E, F, M, S, C, H, G, I, O, ?

**Future:** X, L, W (storage), N (network), A (ask Claude)

**Deferred:** R (registers) - needs return mechanism

**Rationale:** Anyone who's used a monitor ROM will feel at home. The commands are tools, not the destination.

---

## 8. Command Parser

**Decision:** Jump table dispatch with shared parsing helpers

**Helpers:**
- SKIP_SPACES
- READ_HEX_WORD
- TO_HEX_DIGIT
- PRINT_HEX_BYTE/WORD

**Rationale:** Simple, extensible, minimal code duplication. Adding a command means adding one table entry and one handler.

---

## 9. User Interface

**Decision:** Minimal, clean

- Prompt: `> ` (simple)
- Errors: Descriptive but terse ("Invalid address", not "?")
- Startup: Banner + version
- Line editing: BS, basic only

**Rationale:** Clear feedback. No mystery meat errors. The user isn't psychic.

---

## 10. File Formats

**Decision:**
- Config: JSON (modern, readable)
- Intel HEX: Standard format, validate checksums
- Disassembly: Period-appropriate (uppercase, H suffix)

**Rationale:** JSON is fine - this isn't a purity contest. Intel HEX is universal for 8-bit code.

---

## 11. Emulator vs ROM Commands

**Decision:** Clear separation with `:` prefix

**ROM commands:** D, E, F, G, etc. (the 8080 sees these)

**Emulator commands:** :bp, :step, :trace, :load, :save

**Rationale:** No confusion about what's running on the 8080 vs the host. The 8080 code doesn't know the emulator exists.

---

## 12. Device Architecture

**Decision:** Port-based I/O device model

All complex operations (HTTP, Claude API, file I/O) are exposed as I/O devices. ROM sends/receives bytes through ports. The coprocessor (Rust emulator or Pi) handles the complexity.

**Rationale:**
- 8080 code stays dead simple
- Same ROM works on emulator or real hardware
- Clean abstraction boundary
- Testable in isolation
- Future-proof: new capabilities = new port range

**This is the key decision.** Everything modern happens behind the ports. The 8080 just moves bytes.
