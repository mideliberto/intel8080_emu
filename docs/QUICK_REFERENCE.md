# Quick Reference

## Monitor Commands

| Cmd | Syntax | Description |
|-----|--------|-------------|
| C | C start end dest | Compare memory regions |
| D | D [start] [end] | Dump memory (default 128 bytes) |
| E | E [addr] | Examine/modify memory |
| F | F start end val | Fill memory with value |
| G | G [addr] | Go (execute), default 0100. Exit with RET (pending, see TODO) |
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
| : | :LLAAAATT...CC | Intel HEX record, auto-detected at the prompt (Phase 5) |
| T | T | Show time (Phase 6, mailbox `TIME`) |
| A | A addr | Assemble (Phase 7, mailbox `ASM`) |
| U | U addr | Unassemble (Phase 7, mailbox `DIS`) |
| N | N url | HTTP GET (Phase 8, mailbox `GET`) |
| Q | Q prompt | Ask Claude (Phase 9, mailbox `ASK`) |
| R | R | Registers |
| V | V | Version |
| Z | Z | Cold restart |

The old plan's "Q = Quit emulator" collides with Q = ask Claude. That's an open decision in TODO.md.

---

## Port Map

| Range | Device | Status |
|-------|--------|--------|
| 00-02 | Console | ✅ |
| 08-0C | Storage | ✅ |
| 0D-0F | Mount | ✅ |
| 10-13 | Service Mailbox (TIME/ASM/DIS/GET/ASK) | Future |
| 08-6F | Pi window (READY wait-state) | - |
| 30-32 | Interim timer in cpu.rs | To be deleted (TODO) |
| FE | Sys Control | ✅ |
| FF | Sys Status | ✅ |

---

## Memory Map

| Range | Contents |
|-------|----------|
| 0000-007F | Unused (no vectors, no API table) |
| 0080-00FF | Workspace |
| 0100-EEFF | User programs |
| EF00-EFFF | Monitor stack page |
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
| any | Disable overlay (emulator: 00 only, FF = cold reset, until the TODO fix lands) |

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

**Status:** 00=OK/mounted (missing file is created), 01=open error (mount) or not mounted (query), 02=invalid name

---

## Service Mailbox Ports (Future)

| Port | R/W | Function |
|------|-----|----------|
| 10 | W | Command char |
| 11 | W | Control (01=execute, 02=clear) |
| 12 | R | Status |
| 13 | R | Response byte |

**Status:** 00=idle, 01=busy, 02=byte available, 03=done, 80+=error

---

## RST Vectors

None installed. Interrupts stay disabled.

---

## Boot Sequence

```
1. Reset -> overlay on, PC=0000
2. Execute ROM at 0000 (via overlay)
3. JMP to F000+ range
4. OUT FE,00 -> overlay off
5. Init workspace + I/O stubs, banner
6. MAIN_LOOP
```
