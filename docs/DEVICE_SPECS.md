# I/O Device Specifications

## Philosophy

Same ports everywhere. 8080 code runs identically on:
- Rust emulator (development)
- Raspberry Pi (coprocessor to real 8080)
- Arduino/ESP32 (minimal coprocessor)

The 8080 doesn't know what's behind the ports. It doesn't care.

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
| 0x00 | Disable ROM overlay (expose RAM at 0x0000) |
| 0x01 | Halt CPU |
| 0xFF | Cold reset (re-enable overlay) |

### Status Byte (Port 0xFF Read)

| Bit | Meaning |
|-----|---------|
| 0 | ROM overlay state (1=enabled, 0=disabled) |
| 1-7 | Reserved / sense switches |

---

## Storage Device (Ports 0x08-0x0C)

**Status:** 🔲 Phase 4

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
| 1 | Ready (always 1) |
| 7 | EOF (address >= file size) |

### Control Commands (Port 0x0C Write)

| Value | Function |
|-------|----------|
| 0x00 | Reset address to 0 |
| 0x01 | Decrement address |
| 0x02 | Flush write buffer |

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

**Status:** 🔲 Phase 4

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
| 0x01 | File not found |
| 0x02 | Invalid filename |
| 0xFF | Busy |

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
        XRA     A
        OUT     0DH             ; Null terminator
        MVI     A,01H
        OUT     0EH             ; Mount command
WAIT_MOUNT:
        IN      0FH
        CPI     0FFH
        JZ      WAIT_MOUNT      ; Poll until not busy
        ORA     A
        JNZ     MOUNT_ERROR     ; Non-zero = error

FILENAME: DB 'CLAUDE.BIN',0
```

### Filename Rules

- Max 12 characters (8.3 format)
- Valid chars: a-z, A-Z, 0-9, ., -, _
- Null-terminated
- Relative to storage base path

---

## Disassembler (Ports 0x20-0x27)

**Status:** 🔲 Future

### Registers

| Port | Read | Write |
|------|------|-------|
| 0x20 | - | Opcode byte |
| 0x21 | - | Command |
| 0x22 | Status | - |
| 0x23 | Text char | - |

### Commands

| Value | Function |
|-------|----------|
| 0x01 | Disassemble |
| 0x02 | Reset |

### Status Byte

| Bits | Meaning |
|------|---------|
| 0-6 | Text length |
| 7 | Error flag |

---

## Assembler (Ports 0x28-0x2F)

**Status:** 🔲 Future

### Registers

| Port | Read | Write |
|------|------|-------|
| 0x28 | - | Text char |
| 0x29 | - | Command |
| 0x2A | Status | - |
| 0x2B | Opcode byte | - |
| 0x2C | Error position | - |

### Commands

| Value | Function |
|-------|----------|
| 0x01 | Assemble |
| 0x02 | Reset |

### Status Byte

| Bits | Meaning |
|------|---------|
| 0-3 | Bytes assembled |
| 4-7 | Error code (0=success) |

---

## Timer 8253 (Ports 0x70-0x73)

**Status:** 🔲 Future

### Registers

| Port | Read | Write |
|------|------|-------|
| 0x70 | Counter 0 | Counter 0 |
| 0x71 | Counter 1 | Counter 1 |
| 0x72 | Counter 2 | Counter 2 |
| 0x73 | - | Control |

### Control Register Format

```
Bits 7-6: Counter select (00=0, 01=1, 10=2, 11=read-back)
Bits 5-4: R/W mode (00=latch, 01=LSB, 10=MSB, 11=LSB then MSB)
Bits 3-1: Mode (010=rate generator - only mode implemented)
Bit 0:    BCD (0=binary - only mode supported)
```

### Initialize 100Hz Timer (2MHz CPU)

```asm
; Count = 2,000,000 / 100 = 20,000 = 0x4E20
        DI
        MVI     A,00110100b     ; Counter 0, LSB/MSB, Mode 2
        OUT     73H
        MVI     A,20H           ; LSB
        OUT     70H
        MVI     A,4EH           ; MSB
        OUT     70H
        EI
```

---

## Claude API (Ports 0x38-0x3F)

**Status:** 🔲 Phase 9

The 8080 talks to Claude. It sends bytes, gets bytes back. Doesn't know it's talking to an AI.

### Registers

| Port | Read | Write |
|------|------|-------|
| 0x38 | - | Prompt char |
| 0x39 | - | Command |
| 0x3A | Status | - |
| 0x3B | Response char | - |

### Commands (Port 0x39 Write)

| Value | Function |
|-------|----------|
| 0x01 | Send (submit prompt) |
| 0x02 | Clear (reset buffers) |

### Status Byte (Port 0x3A Read)

| Value | Meaning |
|-------|---------|
| 0x00 | Idle |
| 0x01 | Waiting (request in flight) |
| 0x02 | Ready (response available) |
| 0x03 | Done (no more response chars) |
| 0x80+ | Error |

### Usage

```asm
; Ask Claude something
        LXI     H,PROMPT
SEND_PROMPT:
        MOV     A,M
        ORA     A
        JZ      DO_SEND
        OUT     38H             ; Prompt char
        INX     H
        JMP     SEND_PROMPT
DO_SEND:
        MVI     A,01H
        OUT     39H             ; Send command
WAIT_RESPONSE:
        IN      3AH             ; Status
        CPI     02H             ; Ready?
        JNZ     WAIT_RESPONSE
READ_RESPONSE:
        IN      3AH
        CPI     03H             ; Done?
        JZ      FINISHED
        IN      3BH             ; Response char
        CALL    CONOUT
        JMP     READ_RESPONSE
        
PROMPT: DB 'What is the 8080?',0
```

### Implementation Notes

- API key stored in config file, not ROM
- System prompt includes project context
- Response streaming handles Claude's output
- Coprocessor (Rust/Pi) handles TLS, JSON, etc.

---

## HTTP Client (Ports 0x40-0x47)

**Status:** 🔲 Future

### Registers

| Port | Read | Write |
|------|------|-------|
| 0x40 | - | URL char |
| 0x41 | - | Command |
| 0x42 | Status code | - |
| 0x43 | Header char | - |
| 0x44 | Body char | - |
| 0x45 | - | POST body |

### Commands

| Value | Function |
|-------|----------|
| 0x01 | GET |
| 0x02 | POST |

---

## System Time (Ports 0x60-0x6F)

**Status:** 🔲 Future

Read-only time registers.

### Current Time (0x60-0x66)

| Port | Value |
|------|-------|
| 0x60 | Second (0-59) |
| 0x61 | Minute (0-59) |
| 0x62 | Hour (0-23) |
| 0x63 | Day (1-31) |
| 0x64 | Month (1-12) |
| 0x65 | Year (since 1900) |
| 0x66 | Day of week (0=Sun) |

### Uptime (0x68-0x6B)

| Port | Value |
|------|-------|
| 0x68 | Seconds |
| 0x69 | Minutes |
| 0x6A | Hours |
| 0x6B | Days |

---

## Hardware Implementation Notes

For future physical build:

| Device | Rust | Pi | Arduino |
|--------|------|-----|---------|
| Console | crossterm | UART | Serial |
| Storage | std::fs | SD card | SD.h |
| Network | reqwest | requests | WiFi.h |
| Timer | thread::sleep | hardware | Timer1 |
