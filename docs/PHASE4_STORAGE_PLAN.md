# Phase 4: Storage System Implementation Plan

## Overview

Linear-addressed storage with 24-bit addressing and file mounting. 16MB address space. No sectors, no tracks, no banks—just bytes that map directly to SD card or EEPROM in hardware.

**Ports:**
- 0x08-0x0C: Storage Device (24-bit address, data, status/control)
- 0x0D-0x0F: Storage Mount Service (filename, control, status)

**Monitor Commands:**
- X [filename] - Mount storage file
- L storage_addr mem_addr [len] - Load from storage to memory  
- W mem_addr storage_addr [len] - Write memory to storage

Length defaults to 256 (0x100) if omitted. Storage addresses are 24-bit (up to 6 hex digits).

---

## Part 1: Rust Implementation

### Step 1.1: Add Port Constants

**File:** `src/io/devices/ports.rs` (new file)

```rust
// Storage Device (0x08-0x0C) - 24-bit addressing
pub const STORAGE_ADDR_LO: u8 = 0x08;
pub const STORAGE_ADDR_MID: u8 = 0x09;
pub const STORAGE_ADDR_HI: u8 = 0x0A;
pub const STORAGE_DATA: u8 = 0x0B;
pub const STORAGE_STATUS: u8 = 0x0C;  // Read: status, Write: control

// Storage Mount Service (0x0D-0x0F)
pub const MOUNT_FILENAME: u8 = 0x0D;
pub const MOUNT_CONTROL: u8 = 0x0E;
pub const MOUNT_STATUS: u8 = 0x0F;

// Control commands for Storage (write to 0x0C)
pub const STORAGE_CMD_RESET: u8 = 0x00;
pub const STORAGE_CMD_DEC: u8 = 0x01;
pub const STORAGE_CMD_FLUSH: u8 = 0x02;

// Control commands for Mount (write to 0x0E)
pub const MOUNT_CMD_MOUNT: u8 = 0x01;
pub const MOUNT_CMD_UNMOUNT: u8 = 0x02;
pub const MOUNT_CMD_QUERY: u8 = 0x03;

// Mount status codes (read from 0x0F)
pub const MOUNT_STATUS_OK: u8 = 0x00;
pub const MOUNT_STATUS_NOT_FOUND: u8 = 0x01;
pub const MOUNT_STATUS_INVALID: u8 = 0x02;
pub const MOUNT_STATUS_BUSY: u8 = 0xFF;
```

### Step 1.2: Storage Device Implementation

**File:** `src/io/devices/storage.rs`

```rust
use std::fs::{File, OpenOptions};
use std::io::{Read, Write, Seek, SeekFrom};
use std::path::PathBuf;
use crate::io::device::IoDevice;

pub struct Storage {
    file: Option<File>,
    address: u32,           // 24-bit address (stored in 32-bit for convenience)
    file_size: usize,
    dirty: bool,
}

impl Storage {
    pub fn new() -> Self {
        Storage {
            file: None,
            address: 0,
            file_size: 0,
            dirty: false,
        }
    }

    pub fn mount(&mut self, path: &PathBuf) -> Result<(), String> {
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .open(path)
            .map_err(|e| e.to_string())?;
        
        let metadata = file.metadata().map_err(|e| e.to_string())?;
        self.file_size = metadata.len() as usize;
        self.file = Some(file);
        self.address = 0;
        self.dirty = false;
        Ok(())
    }

    pub fn unmount(&mut self) {
        if let Some(ref mut f) = self.file {
            let _ = f.flush();
        }
        self.file = None;
        self.file_size = 0;
        self.address = 0;
        self.dirty = false;
    }

    pub fn is_mounted(&self) -> bool {
        self.file.is_some()
    }

    fn read_byte_at(&mut self, addr: u32) -> u8 {
        if let Some(ref mut file) = self.file {
            if file.seek(SeekFrom::Start(addr as u64)).is_ok() {
                let mut buf = [0u8; 1];
                if file.read_exact(&mut buf).is_ok() {
                    return buf[0];
                }
            }
        }
        0xFF  // Return 0xFF for unmounted or EOF
    }

    fn write_byte_at(&mut self, addr: u32, value: u8) {
        if let Some(ref mut file) = self.file {
            if file.seek(SeekFrom::Start(addr as u64)).is_ok() {
                let _ = file.write_all(&[value]);
                self.dirty = true;
                // Expand file size if needed
                if addr as usize >= self.file_size {
                    self.file_size = addr as usize + 1;
                }
            }
        }
    }

    fn flush(&mut self) {
        if let Some(ref mut file) = self.file {
            let _ = file.flush();
            self.dirty = false;
        }
    }

    fn increment_address(&mut self) {
        // 24-bit wrap
        self.address = (self.address.wrapping_add(1)) & 0x00FF_FFFF;
    }

    fn decrement_address(&mut self) {
        // 24-bit wrap
        self.address = (self.address.wrapping_sub(1)) & 0x00FF_FFFF;
    }
}

impl IoDevice for Storage {
    fn read(&mut self, port: u8) -> u8 {
        match port {
            0x08 => self.address as u8,                     // ADDR_LO
            0x09 => (self.address >> 8) as u8,              // ADDR_MID
            0x0A => (self.address >> 16) as u8,             // ADDR_HI
            0x0B => {                                        // DATA (auto-increment)
                let val = self.read_byte_at(self.address);
                self.increment_address();
                val
            }
            0x0C => {                                        // STATUS
                let mut status = 0u8;
                if self.is_mounted() { status |= 0x01; }    // Bit 0: mounted
                status |= 0x02;                              // Bit 1: ready (always)
                if self.address as usize >= self.file_size {
                    status |= 0x80;                          // Bit 7: EOF
                }
                status
            }
            _ => 0xFF
        }
    }

    fn write(&mut self, port: u8, value: u8) {
        match port {
            0x08 => {                                        // ADDR_LO
                self.address = (self.address & 0x00FFFF00) | (value as u32);
            }
            0x09 => {                                        // ADDR_MID
                self.address = (self.address & 0x00FF00FF) | ((value as u32) << 8);
            }
            0x0A => {                                        // ADDR_HI
                self.address = (self.address & 0x0000FFFF) | ((value as u32) << 16);
            }
            0x0B => {                                        // DATA (auto-increment)
                self.write_byte_at(self.address, value);
                self.increment_address();
            }
            0x0C => {                                        // CONTROL
                match value {
                    0x00 => self.address = 0,                // Reset address
                    0x01 => self.decrement_address(),        // Decrement
                    0x02 => self.flush(),                    // Flush buffer
                    _ => {}
                }
            }
            _ => {}
        }
    }
}
```

### Step 1.3: Storage Mount Service Implementation

**File:** `src/io/devices/storage_mount.rs`

```rust
use std::path::PathBuf;
use std::cell::RefCell;
use std::rc::Rc;
use crate::io::device::IoDevice;
use super::storage::Storage;

pub struct StorageMount {
    storage: Rc<RefCell<Storage>>,
    base_path: PathBuf,
    filename_buffer: Vec<u8>,
    status: u8,
}

// Status codes
const STATUS_OK: u8 = 0x00;
const STATUS_NOT_FOUND: u8 = 0x01;
const STATUS_INVALID: u8 = 0x02;
const STATUS_BUSY: u8 = 0xFF;

impl StorageMount {
    pub fn new(storage: Rc<RefCell<Storage>>, base_path: PathBuf) -> Self {
        StorageMount {
            storage,
            base_path,
            filename_buffer: Vec::with_capacity(13),  // 8.3 + null
            status: STATUS_OK,
        }
    }

    fn mount_file(&mut self) {
        // Build filename from buffer
        let filename = String::from_utf8_lossy(&self.filename_buffer);
        let filename = filename.trim_matches(char::from(0));
        
        if filename.is_empty() || filename.len() > 12 {
            self.status = STATUS_INVALID;
            return;
        }

        // Check for invalid characters
        if !filename.chars().all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '-' || c == '_') {
            self.status = STATUS_INVALID;
            return;
        }

        let path = self.base_path.join(filename);
        
        let mut storage = self.storage.borrow_mut();
        match storage.mount(&path) {
            Ok(()) => self.status = STATUS_OK,
            Err(_) => self.status = STATUS_NOT_FOUND,
        }
    }

    fn unmount(&mut self) {
        let mut storage = self.storage.borrow_mut();
        storage.unmount();
        self.status = STATUS_OK;
    }

    fn query(&mut self) {
        let storage = self.storage.borrow();
        self.status = if storage.is_mounted() { STATUS_OK } else { STATUS_NOT_FOUND };
    }
}

impl IoDevice for StorageMount {
    fn read(&mut self, port: u8) -> u8 {
        match port {
            0x0F => self.status,
            _ => 0xFF
        }
    }

    fn write(&mut self, port: u8, value: u8) {
        match port {
            0x0D => {                                        // MOUNT_FILENAME
                if value == 0 {
                    // Null terminator - filename complete
                } else if self.filename_buffer.len() < 12 {
                    self.filename_buffer.push(value);
                }
            }
            0x0E => {                                        // MOUNT_CONTROL
                match value {
                    0x01 => {                                // Mount
                        self.mount_file();
                        self.filename_buffer.clear();
                    }
                    0x02 => self.unmount(),                 // Unmount
                    0x03 => self.query(),                   // Query
                    _ => {}
                }
            }
            _ => {}
        }
    }
}
```

### Step 1.4: Wire Into Main

**File:** `src/main.rs` (add to device setup)

```rust
use std::rc::Rc;
use std::cell::RefCell;
use std::path::PathBuf;

// In device setup section:
let storage = Rc::new(RefCell::new(Storage::new()));
let storage_mount = StorageMount::new(
    Rc::clone(&storage),
    PathBuf::from("./storage/")
);

// Map storage device ports
cpu.io_bus_mut().map_port(0x08, storage.clone());
cpu.io_bus_mut().map_port(0x09, storage.clone());
cpu.io_bus_mut().map_port(0x0A, storage.clone());
cpu.io_bus_mut().map_port(0x0B, storage.clone());
cpu.io_bus_mut().map_port(0x0C, storage.clone());

// Map mount service ports
let mount = Rc::new(RefCell::new(storage_mount));
cpu.io_bus_mut().map_port(0x0D, mount.clone());
cpu.io_bus_mut().map_port(0x0E, mount.clone());
cpu.io_bus_mut().map_port(0x0F, mount.clone());
```

### Step 1.5: Tests

**File:** `src/io/devices/storage_test.rs`

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::tempdir;

    #[test]
    fn test_storage_mount_read() {
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("test.bin");
        
        // Create test file with known content
        let mut file = File::create(&file_path).unwrap();
        file.write_all(&[0x41, 0x42, 0x43, 0x44]).unwrap();
        
        let mut storage = Storage::new();
        storage.mount(&file_path).unwrap();
        
        // Set address to 0
        storage.write(0x08, 0x00);  // ADDR_LO
        storage.write(0x09, 0x00);  // ADDR_MID
        storage.write(0x0A, 0x00);  // ADDR_HI
        
        // Read with auto-increment
        assert_eq!(storage.read(0x0B), 0x41);
        assert_eq!(storage.read(0x0B), 0x42);
        assert_eq!(storage.read(0x0B), 0x43);
        assert_eq!(storage.read(0x0B), 0x44);
    }

    #[test]
    fn test_24bit_addressing() {
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("big.bin");
        
        let mut storage = Storage::new();
        storage.mount(&file_path).unwrap();
        
        // Set address to 0x012345
        storage.write(0x08, 0x45);  // ADDR_LO
        storage.write(0x09, 0x23);  // ADDR_MID
        storage.write(0x0A, 0x01);  // ADDR_HI
        
        // Verify address read back
        assert_eq!(storage.read(0x08), 0x45);
        assert_eq!(storage.read(0x09), 0x23);
        assert_eq!(storage.read(0x0A), 0x01);
    }

    #[test]
    fn test_storage_write() {
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("write_test.bin");
        
        let mut storage = Storage::new();
        storage.mount(&file_path).unwrap();
        
        // Write bytes at address 0
        storage.write(0x08, 0x00);
        storage.write(0x09, 0x00);
        storage.write(0x0A, 0x00);
        storage.write(0x0B, 0xAA);
        storage.write(0x0B, 0xBB);
        storage.write(0x0C, 0x02);  // Flush
        
        // Reset address and read back
        storage.write(0x0C, 0x00);  // Reset address
        assert_eq!(storage.read(0x0B), 0xAA);
        assert_eq!(storage.read(0x0B), 0xBB);
    }

    #[test]
    fn test_status_bits() {
        let mut storage = Storage::new();
        
        // Not mounted
        let status = storage.read(0x0C);
        assert_eq!(status & 0x01, 0);  // Not mounted
        assert_eq!(status & 0x02, 0x02);  // Always ready
    }
}
```

---

## Part 2: Assembly ROM Commands

### Step 2.1: Add Port Equates

**File:** `rom/monitor.asm` (add to constants section)

```asm
; Storage Device (0x08-0x0C) - 24-bit addressing
STORAGE_ADDR_LO     EQU 08H
STORAGE_ADDR_MID    EQU 09H
STORAGE_ADDR_HI     EQU 0AH
STORAGE_DATA        EQU 0BH
STORAGE_STATUS      EQU 0CH

; Storage control commands
STORAGE_RESET_ADDR  EQU 00H
STORAGE_DECREMENT   EQU 01H
STORAGE_FLUSH       EQU 02H

; Storage Mount Service (0x0D-0x0F)
MOUNT_FILENAME      EQU 0DH
MOUNT_CONTROL       EQU 0EH
MOUNT_STATUS        EQU 0FH

; Mount control commands
MOUNT_CMD_MOUNT     EQU 01H
MOUNT_CMD_UNMOUNT   EQU 02H
MOUNT_CMD_QUERY     EQU 03H

; Mount status codes
MOUNT_OK            EQU 00H
MOUNT_NOT_FOUND     EQU 01H
MOUNT_INVALID       EQU 02H
MOUNT_BUSY          EQU 0FFH
```

### Step 2.2: X Command - Mount Storage File

```asm
;============================================================
; CMD_MOUNT_STORAGE (X command)
; Mount a storage file
; Usage: X [filename]
;   X CLAUDE.BIN  - Mount file CLAUDE.BIN
;   X             - Show current mount status
;============================================================
CMD_MOUNT_STORAGE:
        CALL    SKIP_SPACES
        MOV     A,M
        ORA     A
        JZ      MOUNT_SHOW_STATUS   ; No args - show status
        
        ; Send filename to mount service
MOUNT_SEND_NAME:
        MOV     A,M
        ORA     A
        JZ      MOUNT_SEND_NULL
        CPI     ' '                 ; Stop at space
        JZ      MOUNT_SEND_NULL
        OUT     MOUNT_FILENAME
        INX     H
        JMP     MOUNT_SEND_NAME
        
MOUNT_SEND_NULL:
        XRA     A
        OUT     MOUNT_FILENAME      ; Send null terminator
        
        ; Issue mount command
        MVI     A,MOUNT_CMD_MOUNT
        OUT     MOUNT_CONTROL
        
        ; Check result
        IN      MOUNT_STATUS
        ORA     A
        JZ      MOUNT_OK_MSG
        CPI     MOUNT_NOT_FOUND
        JZ      MOUNT_NOT_FOUND_MSG
        JMP     MOUNT_INVALID_MSG
        
MOUNT_OK_MSG:
        LXI     H,MSG_MOUNTED
        CALL    PRINT_STRING
        JMP     MAIN_LOOP
        
MOUNT_NOT_FOUND_MSG:
        LXI     H,MSG_FILE_NOT_FOUND
        CALL    PRINT_STRING
        JMP     MAIN_LOOP
        
MOUNT_INVALID_MSG:
        LXI     H,MSG_INVALID_FILENAME
        CALL    PRINT_STRING
        JMP     MAIN_LOOP

MOUNT_SHOW_STATUS:
        MVI     A,MOUNT_CMD_QUERY
        OUT     MOUNT_CONTROL
        IN      MOUNT_STATUS
        ORA     A
        JNZ     MOUNT_NONE
        LXI     H,MSG_STORAGE_MOUNTED
        CALL    PRINT_STRING
        JMP     MAIN_LOOP
        
MOUNT_NONE:
        LXI     H,MSG_NO_STORAGE
        CALL    PRINT_STRING
        JMP     MAIN_LOOP

; Strings
MSG_MOUNTED:          DB 'Mounted',CR,LF,0
MSG_FILE_NOT_FOUND:   DB 'File not found',CR,LF,0
MSG_INVALID_FILENAME: DB 'Invalid filename',CR,LF,0
MSG_STORAGE_MOUNTED:  DB 'Storage mounted',CR,LF,0
MSG_NO_STORAGE:       DB 'No storage mounted',CR,LF,0
```

### Step 2.3: L Command - Load from Storage to Memory

```asm
;============================================================
; CMD_LOAD_STORAGE (L command)
; Load data from storage to memory
; Usage: L storage_addr mem_addr [len]
;   L 0 1000           - Load 256 bytes from storage:000000 to mem:1000
;   L 0 1000 80        - Load 128 bytes
;   L 10000 1000 100   - Load 256 bytes from storage:010000 to mem:1000
;
; Storage addresses are 24-bit (up to 6 hex digits)
; Memory addresses are 16-bit (up to 4 hex digits)
;============================================================
CMD_LOAD_STORAGE:
        ; Check if storage mounted
        IN      STORAGE_STATUS
        ANI     01H                 ; Bit 0 = mounted
        JZ      ERR_NOT_MOUNTED
        
        ; Parse storage address (24-bit, up to 6 digits)
        CALL    SKIP_SPACES
        CALL    READ_HEX_WORD       ; Gets 16 bits into DE
        JC      ERR_SYNTAX
        ; For now, use DE as low 16 bits, high byte = 0
        ; TODO: Parse full 24-bit if needed
        PUSH    D                   ; Save storage addr low
        MVI     A,0
        PUSH    PSW                 ; Save storage addr high (0 for now)
        
        ; Parse memory address (required)
        CALL    SKIP_SPACES
        CALL    READ_HEX_WORD
        JC      ERR_SYNTAX_POP2
        PUSH    D                   ; Save mem address
        
        ; Parse length (optional, default 256)
        CALL    SKIP_SPACES
        MOV     A,M
        ORA     A
        JZ      LOAD_DEFAULT_LEN
        CALL    READ_HEX_WORD
        JC      LOAD_DEFAULT_LEN
        MOV     B,D                 ; BC = length
        MOV     C,E
        JMP     LOAD_DO
        
LOAD_DEFAULT_LEN:
        LXI     B,0100H             ; Default 256 bytes
        
LOAD_DO:
        POP     D                   ; DE = mem address (dest)
        POP     PSW                 ; A = storage addr high
        POP     H                   ; HL = storage addr low
        
        ; Set storage address (24-bit)
        PUSH    PSW                 ; Save high byte
        MOV     A,L
        OUT     STORAGE_ADDR_LO
        MOV     A,H
        OUT     STORAGE_ADDR_MID
        POP     PSW
        OUT     STORAGE_ADDR_HI
        
        ; Copy loop: storage -> memory
LOAD_LOOP:
        IN      STORAGE_DATA        ; Read byte (auto-increment)
        STAX    D                   ; Store to memory
        INX     D
        DCX     B
        MOV     A,B
        ORA     C
        JNZ     LOAD_LOOP
        
        LXI     H,MSG_LOADED
        CALL    PRINT_STRING
        JMP     MAIN_LOOP

ERR_NOT_MOUNTED:
        LXI     H,MSG_NO_STORAGE
        CALL    PRINT_STRING
        JMP     MAIN_LOOP

ERR_SYNTAX_POP2:
        POP     PSW
        POP     D
ERR_SYNTAX:
        LXI     H,MSG_BAD_HEX
        CALL    PRINT_STRING
        JMP     MAIN_LOOP

MSG_LOADED:         DB 'Loaded',CR,LF,0
```

### Step 2.4: W Command - Write Memory to Storage

```asm
;============================================================
; CMD_WRITE_STORAGE (W command)
; Write memory to storage
; Usage: W mem_addr storage_addr [len]
;   W 1000 0           - Write 256 bytes from mem:1000 to storage:000000
;   W 1000 0 80        - Write 128 bytes
;   W 1000 10000 100   - Write 256 bytes from mem:1000 to storage:010000
;============================================================
CMD_WRITE_STORAGE:
        ; Check if storage mounted
        IN      STORAGE_STATUS
        ANI     01H
        JZ      ERR_NOT_MOUNTED
        
        ; Parse memory address (required, source)
        CALL    SKIP_SPACES
        CALL    READ_HEX_WORD
        JC      ERR_SYNTAX
        PUSH    D                   ; Save mem address
        
        ; Parse storage address (24-bit)
        CALL    SKIP_SPACES
        CALL    READ_HEX_WORD
        JC      ERR_SYNTAX_POP1
        PUSH    D                   ; Save storage addr low
        MVI     A,0
        PUSH    PSW                 ; Save storage addr high (0 for now)
        
        ; Parse length (optional, default 256)
        CALL    SKIP_SPACES
        MOV     A,M
        ORA     A
        JZ      WRITE_DEFAULT_LEN
        CALL    READ_HEX_WORD
        JC      WRITE_DEFAULT_LEN
        MOV     B,D
        MOV     C,E
        JMP     WRITE_DO
        
WRITE_DEFAULT_LEN:
        LXI     B,0100H
        
WRITE_DO:
        POP     PSW                 ; A = storage addr high
        POP     D                   ; DE = storage addr low
        POP     H                   ; HL = mem address (source)
        
        ; Set storage address (24-bit)
        PUSH    PSW
        MOV     A,E
        OUT     STORAGE_ADDR_LO
        MOV     A,D
        OUT     STORAGE_ADDR_MID
        POP     PSW
        OUT     STORAGE_ADDR_HI
        
        ; Copy loop: memory -> storage
WRITE_LOOP:
        MOV     A,M                 ; Read from memory
        OUT     STORAGE_DATA        ; Write to storage (auto-increment)
        INX     H
        DCX     B
        MOV     A,B
        ORA     C
        JNZ     WRITE_LOOP
        
        ; Flush
        MVI     A,STORAGE_FLUSH
        OUT     STORAGE_STATUS
        
        LXI     H,MSG_WRITTEN
        CALL    PRINT_STRING
        JMP     MAIN_LOOP

ERR_SYNTAX_POP1:
        POP     D
        JMP     ERR_SYNTAX

MSG_WRITTEN:        DB 'Written',CR,LF,0
```

### Step 2.5: Add to Command Dispatcher

```asm
; In command dispatch (NOT_LOWER section):
        CPI     'L'
        JZ      CMD_LOAD_STORAGE
        CPI     'W'
        JZ      CMD_WRITE_STORAGE
        CPI     'X'
        JZ      CMD_MOUNT_STORAGE
```

---

## Part 3: Implementation Order

### Day 1: Rust Foundation
1. Create `src/io/devices/ports.rs` with constants
2. Create `storage.rs` with basic struct
3. Implement 24-bit address handling
4. Implement mount/unmount
5. Test: Can set/read 24-bit address

### Day 2: Rust Read/Write
1. Implement data port read with 24-bit auto-increment
2. Implement data port write with 24-bit auto-increment
3. Implement status port
4. Implement control commands (reset, flush, decrement)
5. Test: Can read/write through ports

### Day 3: Rust Mount Service
1. Create `storage_mount.rs`
2. Implement filename buffer
3. Implement mount command
4. Implement status query
5. Wire both devices into main.rs
6. Test: Can mount file via ports

### Day 4: ROM X Command
1. Add port equates to monitor.asm
2. Implement X command parser
3. Implement filename sending
4. Implement status checking
5. Test: Can mount file from monitor

### Day 5: ROM L Command
1. Implement L command parser
2. Implement 24-bit storage address setup
3. Implement load loop
4. Test: L 0 1000 loads default 256 bytes
5. Test: L 0 1000 80 loads explicit length
6. Verify with D command

### Day 6: ROM W Command
1. Implement W command parser
2. Implement 24-bit storage address setup
3. Implement write loop with flush
4. Test: W 1000 0 writes default 256 bytes
5. Test: Round-trip (W then L to different mem addr)

### Day 7: Integration & Polish
1. Error handling review
2. Edge cases (empty file, unmounted, EOF)
3. Update HELP command
4. Document in status file

---

## Part 4: Testing Checklist

### Rust Unit Tests
- [ ] Storage: mount file succeeds
- [ ] Storage: mount missing file creates it
- [ ] Storage: 24-bit address register set/get
- [ ] Storage: read data with 24-bit auto-increment
- [ ] Storage: write data with 24-bit auto-increment
- [ ] Storage: address wraps at 24-bit boundary
- [ ] Storage: status bits correct
- [ ] Storage: control commands work (reset, flush, decrement)
- [ ] Mount: filename buffer accumulates
- [ ] Mount: mount command triggers storage mount
- [ ] Mount: status reflects mount state

### Integration Tests
- [ ] Monitor boots with storage devices present
- [ ] X with no args shows "No storage mounted"
- [ ] X DATA.BIN mounts file
- [ ] X with mounted file shows status
- [ ] L 0 1000 loads 256 bytes (default len)
- [ ] L 0 1000 80 loads 128 bytes (explicit len)
- [ ] D 1000 shows loaded data
- [ ] E 1000 AA modifies memory
- [ ] W 1000 0 writes 256 bytes to storage
- [ ] L 0 2000 reads modified data back
- [ ] Data persists across emulator restart

### Edge Cases
- [ ] L without mount shows error
- [ ] W without mount shows error
- [ ] X with invalid filename shows error
- [ ] Reading past EOF returns 0xFF
- [ ] Writing expands file

---

## Part 5: Success Criteria

Phase 4 is **COMPLETE** when:

1. **Rust side:**
   - Storage device responds on ports 0x08-0x0C (24-bit addressing)
   - StorageMount device responds on ports 0x0D-0x0F
   - Files can be mounted/unmounted
   - Data can be read/written with 24-bit auto-increment

2. **Monitor side:**
   - X command mounts files
   - L storage_addr mem_addr [len] loads to memory
   - W mem_addr storage_addr [len] writes from memory
   - Error messages for invalid operations

3. **Workflow verified:**
   ```
   > X TEST.BIN          ; Mount file
   Mounted
   > L 0 1000            ; Load 256 bytes from storage:000000 to mem:1000
   Loaded
   > D 1000              ; Verify data
   1000: 00 01 02 03...
   > E 1000 FF           ; Modify first byte
   > W 1000 0            ; Write 256 bytes back
   Written
   > X                   ; Check status
   Storage mounted
   ```

4. **Future use case verified:**
   ```
   > X CLAUDE.BIN        ; Mount Claude app
   Mounted
   > L 0 100             ; Load to TPA
   Loaded
   > G 100               ; Run it
   ```
