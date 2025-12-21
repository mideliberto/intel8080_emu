# Quick Reference

## Monitor Commands

| Cmd | Syntax | Description |
|-----|--------|-------------|
| C | C start end dest | Compare memory regions |
| D | D [start] [end] | Dump memory (default 128 bytes) |
| E | E [addr] | Examine/modify memory |
| F | F start end val | Fill memory with value |
| G | G [addr] | Go (execute), default 0100 |
| H | H num1 num2 | Hex math: sum, difference |
| I | I port | Input from port |
| L | L stor mem [cnt] | Load from storage (24-bit addr) |
| M | M src dst cnt | Move memory (forward copy) |
| O | O port val | Output to port |
| S | S start end b1... | Search for bytes (1-8) |
| W | W mem stor [cnt] | Write to storage (24-bit addr) |
| X | X [file \| -] | Mount/unmount storage |
| ? | ? | Help |

### Future Commands

| Cmd | Syntax | Description |
|-----|--------|-------------|
| A | A prompt | Ask Claude |
| N G | N G url | HTTP GET |
| N T | N T | Get network time |
| T | T | Show time |
| TI | TI [freq] | Init timer |
| TS | TS | Timer status |
| R | R | Registers |
| V | V | Version |
| Z | Z | Cold restart |
| Q | Q | Quit emulator |

---

## Port Map

| Range | Device | Status |
|-------|--------|--------|
| 00-02 | Console | ✅ |
| 08-0C | Storage | ✅ |
| 0D-0F | Mount | ✅ |
| 20-27 | Disasm | Future |
| 28-2F | Asm | Future |
| 38-3F | Claude API | Future |
| 40-5F | Internet | Future |
| 60-6F | Time | Future |
| 70-73 | Timer | Future |
| FE | Sys Control | ✅ |
| FF | Sys Status | ✅ |

---

## Memory Map

| Range | Contents |
|-------|----------|
| 0000-003F | RST vectors |
| 0040-007F | BIOS table |
| 0080-00FF | Workspace |
| 0100-EFFF | User programs |
| F000-FFFF | ROM |

---

## Workspace Layout

| Address | Size | Purpose |
|---------|------|---------|
| 0080-00CF | 80 | LINE_BUFFER |
| 00D0-00D1 | 2 | BUFFER_PTR |
| 00D2-00D3 | 2 | LAST_DUMP_ADDR |
| 00D4-00D5 | 2 | LAST_EXAM_ADDR |
| 00D6-00D8 | 3 | IO_IN_STUB |
| 00D9-00DB | 3 | IO_OUT_STUB |
| 00DC-00E3 | 8 | SEARCH_PATTERN |
| 00E4 | 1 | SEARCH_LENGTH |
| 00E5-00E6 | 2 | SEARCH_END |
| 00E7-00E9 | 3 | STOR_ADDR (24-bit) |

---

## Flags Register

```
Bit 7: S (Sign)
Bit 6: Z (Zero)
Bit 5: 0
Bit 4: AC (Aux Carry)
Bit 3: 0
Bit 2: P (Parity)
Bit 1: 1
Bit 0: C (Carry)
```

---

## System Control (Port 0xFE)

| Value | Function |
|-------|----------|
| 00 | Disable overlay |
| 01 | Halt CPU |
| FF | Cold reset |

---

## Console Ports

| Port | R/W | Function |
|------|-----|----------|
| 00 | W | Data out |
| 01 | R | Data in |
| 02 | R | Status (bit0=RX, bit1=TX) |

---

## Storage Ports

| Port | R/W | Function |
|------|-----|----------|
| 08 | R/W | Address low |
| 09 | R/W | Address mid |
| 0A | R/W | Address high |
| 0B | R/W | Data (auto-inc 24-bit) |
| 0C | R | Status |
| 0C | W | Control |

**Status bits:** 0=mounted, 1=ready, 7=EOF

**Control:** 00=reset addr, 01=dec, 02=flush

**Address space:** 16MB (24-bit)

---

## Mount Ports

| Port | R/W | Function |
|------|-----|----------|
| 0D | W | Filename char |
| 0E | W | Command |
| 0F | R | Status |

**Commands:** 01=mount, 02=unmount, 03=query

**Status:** 00=OK, 01=not found, 02=invalid

---

## Claude Ports (Future)

| Port | R/W | Function |
|------|-----|----------|
| 38 | W | Prompt char |
| 39 | W | Command |
| 3A | R | Status |
| 3B | R | Response char |

**Commands:** 01=send, 02=clear

**Status:** 00=idle, 01=waiting, 02=ready, 03=done, 80+=error

---

## RST Vectors

| Vector | Address | Purpose |
|--------|---------|---------|
| RST 0 | 0000 | Cold start |
| RST 1 | 0008 | CONOUT |
| RST 2 | 0010 | CONIN |
| RST 3 | 0018 | CONST |
| RST 7 | 0038 | Timer ISR |

---

## Boot Sequence

```
1. Reset -> overlay on, PC=0000
2. Execute ROM at 0000 (via overlay)
3. JMP to F000+ range
4. OUT FE,00 -> overlay off
5. Copy vectors to RAM
6. Normal operation
```
