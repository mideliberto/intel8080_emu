# I/O Device Specifications

## Philosophy

Same ports everywhere. 8080 code runs identically on:
- Rust emulator (development)
- Raspberry Pi (coprocessor to real 8080)
- Arduino/ESP32 (minimal coprocessor)

The 8080 doesn't know what's behind the ports. It doesn't care.

**Timing contract (decided 2026-10-02):** every device answers within its IN/OUT cycle. On hardware, any access to a Pi port (0x08-0x6F) holds the 8080's READY line low until the Pi releases it, using one flip-flop and a GPIO line. Protocols don't need busy-polling for single-byte operations. Long operations (a mailbox request, for example) report progress through a status port.

---

## Console (Ports 0x00-0x02)

**Status:** ✅ Implemented

### Registers

| Port | Read | Write |
|------|------|-------|
| 0x00 | - | Character out |
| 0x01 | Character in | - |
| 0x02 | Status | - |

### Status Byte (Port 0x02)

| Bit | Meaning |
|-----|---------|
| 0 | RX ready (char available) |
| 1 | TX ready (always 1) |

### Usage

```asm
; Output character in A
CONOUT:
        PUSH    PSW
CONOUT_WAIT:
        IN      02H             ; Status
        ANI     02H             ; TX ready?
        JZ      CONOUT_WAIT
        POP     PSW
        OUT     00H             ; Send char
        RET

; Input character to A
CONIN:
        IN      02H             ; Status
        ANI     01H             ; RX ready?
        JZ      CONIN
        IN      01H             ; Get char
        RET
```

---

## System Control (Ports 0xFE-0xFF)

**Status:** ✅ Implemented

### Registers

| Port | Read | Write |
|------|------|-------|
| 0xFE | - | Control command |
| 0xFF | Status | - |

### Control Commands (Port 0xFE Write)

| Value | Function |
|-------|----------|
| any | Disable ROM overlay (expose RAM at 0x0000) |

Decided 2026-10-02: any write disables the overlay. No halt command (the 8080 has `HLT`) and no software cold reset (use the reset line). The emulator still treats 0x00 as disable and 0xFF as cold reset; change pending in TODO.md. The ROM writes 0x00.

### Status Byte (Port 0xFF Read)

| Bit | Meaning |
|-----|---------|
| 0 | ROM overlay state (1=enabled, 0=disabled) |
| 1-7 | Undefined (mask them; emulator returns 0) |

---

## Storage Device (Ports 0x08-0x0C)

**Status:** ✅ Implemented

Linear-addressed storage with 24-bit addressing. 16MB address space. No sectors, no tracks, no banks. Just bytes.

### Registers

| Port | Read | Write |
|------|------|-------|
| 0x08 | Address low | Address low |
| 0x09 | Address mid | Address mid |
| 0x0A | Address high | Address high |
| 0x0B | Data (auto-inc) | Data (auto-inc) |
| 0x0C | Status | Control |

### Status Byte (Port 0x0C Read)

| Bit | Meaning |
|-----|---------|
| 0 | Mounted (1=yes) |
| 1 | Ready (always 1; hardware uses the READY wait-state) |
| 7 | EOF (address >= file size; also 1 when nothing is mounted) |

### Control Commands (Port 0x0C Write)

| Value | Function |
|-------|----------|
| 0x00 | Reset address to 0 |
| 0x01 | Decrement address |
| 0x02 | Flush write buffer |

### Data Port Edge Cases

- Read at or past EOF returns 0xFF. Decided 2026-10-02: the address still advances, like every other data access. The emulator currently does not advance on a past-EOF read; fix pending in TODO.md.
- Write past EOF extends the file.
- Read with nothing mounted returns 0xFF; write with nothing mounted is dropped.

### Read Sequence

```asm
; Read 256 bytes from storage:012345h to memory:2000h
        MVI     A,45H
        OUT     08H             ; Addr low
        MVI     A,23H
        OUT     09H             ; Addr mid
        MVI     A,01H
        OUT     0AH             ; Addr high
        LXI     H,2000H
        MVI     C,00H           ; 256 iterations
READ_LOOP:
        IN      0BH             ; Read + auto-increment (all 24 bits)
        MOV     M,A
        INX     H
        DCR     C
        JNZ     READ_LOOP
```

### Write Sequence

```asm
; Write 128 bytes from memory:3000h to storage:000000h
        XRA     A
        OUT     08H             ; Addr low = 0
        OUT     09H             ; Addr mid = 0
        OUT     0AH             ; Addr high = 0
        LXI     H,3000H
        MVI     C,80H           ; 128 bytes
WRITE_LOOP:
        MOV     A,M
        OUT     0BH             ; Write + auto-increment
        INX     H
        DCR     C
        JNZ     WRITE_LOOP
        MVI     A,02H
        OUT     0CH             ; Flush
```

---

## Storage Mount Service (Ports 0x0D-0x0F)

**Status:** ✅ Implemented

### Registers

| Port | Read | Write |
|------|------|-------|
| 0x0D | - | Filename char |
| 0x0E | - | Command |
| 0x0F | Status | - |

### Commands (Port 0x0E Write)

| Value | Function |
|-------|----------|
| 0x01 | Mount (open file) |
| 0x02 | Unmount |
| 0x03 | Query status |

### Status Codes (Port 0x0F Read)

| Value | Meaning |
|-------|---------|
| 0x00 | OK / Mounted |
| 0x01 | Mount: error opening file. Query: not mounted |
| 0x02 | Invalid filename |
| 0xFF | Reserved (never returned; hardware uses READY) |

Mount creates a missing file (empty) and returns 0x00. There is no "file not found"; decided 2026-10-02, so that `W` to a new file works.

### Mount Sequence

```asm
; Mount "CLAUDE.BIN"
        LXI     H,FILENAME
SEND_NAME:
        MOV     A,M
        ORA     A
        JZ      DO_MOUNT
        OUT     0DH             ; Send char
        INX     H
        JMP     SEND_NAME
DO_MOUNT:
        MVI     A,01H
        OUT     0EH             ; Mount command (ends the name)
        IN      0FH
        ORA     A
        JNZ     MOUNT_ERROR     ; Non-zero = error

FILENAME: DB 'CLAUDE.BIN',0
```

### Filename Rules

- Max 12 characters. 8.3 is a convention, not enforced
- Valid chars: a-z, A-Z, 0-9, ., -, _
- No terminator needed: Mount (0x01) ends the name and clears the buffer. 0x00 on port 0x0D is ignored
- Relative to storage base path
- Decided 2026-10-02:
  - Names over 12 chars return 0x02. The emulator currently truncates to 12 and mounts that; fix pending.
  - A failed mount unmounts the previous file. The emulator currently leaves it mounted; fix pending.

---

## Service Mailbox (Ports 0x10-0x13)

**Status:** 🔲 Future (first use: Phase 6 `TIME`)

Decided 2026-10-02. Every Pi-side service goes through this one device: time, HTTP, Claude, assembler, disassembler. The 8080 sends a text command and reads back a byte stream. The Pi does the hard parts: TLS, DNS, JSON, NTP, the API key. The ROM needs one send/poll/stream routine, which the `T`, `N`, `Q`, `A` and `U` commands share.

### Registers

| Port | Read | Write |
|------|------|-------|
| 0x10 | - | Command char |
| 0x11 | - | Control |
| 0x12 | Status | - |
| 0x13 | Response byte | - |

### Control (Port 0x11 Write)

| Value | Function |
|-------|----------|
| 0x01 | Execute the command buffered via 0x10 |
| 0x02 | Clear (discard command and response) |

### Status (Port 0x12 Read)

| Value | Meaning |
|-------|---------|
| 0x00 | Idle |
| 0x01 | Busy (request in flight) |
| 0x02 | Response byte available |
| 0x03 | Done (response exhausted) |
| 0x80+ | Error |

### Commands

Text, defined by the phase that needs them. Planned:

| Command | Phase | Response |
|---------|-------|----------|
| `TIME` | 6 | Current date/time as text (the Pi keeps time via NTP) |
| `ASM <line>` | 7 | Opcode bytes |
| `DIS <bytes>` | 7 | Disassembly text |
| `GET <url> [> FILE]` | 8 | Body as text, or written to a storage file |
| `ASK <prompt>` | 9 | Claude's reply as text |

Large results (web pages, books) go into a storage file. The 8080 then reads them through the storage ports it already has.

### Implementation

The device logic lives in Rust behind the `IoDevice` trait. In the emulator the CPU bus calls it; on the Pi a GPIO front end calls the same code. It is written once.

---

## Superseded Device Specs

Replaced on 2026-10-02 by the Service Mailbox. The full text is in git history (commit 41f04ce and earlier).

- Disassembler (0x20-0x27) and Assembler (0x28-0x2F): now `DIS` / `ASM`
- Claude API (0x38-0x3F): now `ASK`
- HTTP Client (0x40-0x47): now `GET`
- System Time (0x60-0x6F): now `TIME`
- Timer 8253 (0x70-0x73): deferred to Someday. Nothing needs a periodic interrupt yet. If one comes back, it is a local chip in 0x70+, not a Pi device.

---

## Hardware Implementation Notes

For future physical build:

| Device | Rust | Pi | Arduino |
|--------|------|-----|---------|
| Console | crossterm | UART | Serial |
| Storage | std::fs | SD card | SD.h |
| Service Mailbox | Rust device | same Rust code behind GPIO | - |
