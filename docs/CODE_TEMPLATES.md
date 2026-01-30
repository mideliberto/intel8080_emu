# Code Templates

Reference code for project structure. The actual code is the source of truth - this is for reference when setting up new components.

---

## Project Structure

```
intel8080_emu/
├── Cargo.toml
├── build.rs                 # Generates BUILD_TIMESTAMP
├── src/
│   ├── main.rs              # Entry point
│   ├── lib.rs               # Exports Intel8080
│   ├── cpu.rs               # CPU emulation
│   ├── memory.rs            # Memory trait
│   ├── registers.rs         # Register enums, flags
│   └── io/
│       ├── mod.rs
│       ├── bus.rs           # IoBus
│       ├── device.rs        # IoDevice trait
│       └── devices/
│           ├── console.rs
│           ├── test_console.rs
│           ├── timer.rs
│           ├── disk.rs
│           └── null.rs
├── rom/
│   ├── Makefile
│   ├── monitor.asm
│   └── monitor.bin
└── tests/
    ├── cpu_tests.rs
    ├── monitor_tests.rs
    └── common/
        └── mod.rs
```

---

## Cargo.toml Reference

```toml
[package]
name = "intel8080_emu"
version = "0.1.0"
edition = "2021"
build = "build.rs"

[dependencies]
crossterm = "0.27"

[dev-dependencies]
# Add as needed
```

---

## Configuration (Future)

```json
{
  "system": {
    "name": "8080 Development System",
    "rom_path": "./rom/monitor.bin"
  },
  
  "cpu": {
    "speed_mhz": 2.0,
    "cycle_accurate": true
  },
  
  "storage": {
    "base_path": "./storage/",
    "max_size_kb": 64
  },
  
  "console": {
    "echo": true,
    "control_c_abort": true
  },
  
  "claude": {
    "api_key_env": "ANTHROPIC_API_KEY",
    "model": "claude-sonnet-4-20250514",
    "system_prompt_path": "./claude_system.txt"
  },
  
  "debug": {
    "trace_instructions": false,
    "log_port_io": false
  }
}
```

---

## IoDevice Trait

```rust
pub trait IoDevice {
    fn read(&mut self, port: u8) -> u8;
    fn write(&mut self, port: u8, value: u8);
}
```

---

## Device Implementation Pattern

```rust
use std::cell::RefCell;
use std::rc::Rc;

pub struct MyDevice {
    // Device state
}

impl MyDevice {
    pub fn new() -> Self {
        MyDevice { /* init */ }
    }
}

impl IoDevice for MyDevice {
    fn read(&mut self, port: u8) -> u8 {
        match port {
            0x08 => { /* handle port 8 */ }
            _ => 0xFF
        }
    }

    fn write(&mut self, port: u8, value: u8) {
        match port {
            0x08 => { /* handle port 8 */ }
            _ => {}
        }
    }
}

// Usage in main.rs:
let device = Rc::new(RefCell::new(MyDevice::new()));
cpu.io_bus_mut().map_port(0x08, device.clone());
cpu.io_bus_mut().map_port(0x09, device.clone());
```

---

## Port Constants (Future)

When implementing more devices, consider a single source of truth:

```rust
// src/ports.rs
pub const CONSOLE_DATA_OUT: u8 = 0x00;
pub const CONSOLE_DATA_IN: u8 = 0x01;
pub const CONSOLE_STATUS: u8 = 0x02;

pub const STORAGE_ADDR_LO: u8 = 0x08;
pub const STORAGE_ADDR_MID: u8 = 0x09;
pub const STORAGE_ADDR_HI: u8 = 0x0A;
pub const STORAGE_DATA: u8 = 0x0B;
pub const STORAGE_STATUS: u8 = 0x0C;

pub const MOUNT_FILENAME: u8 = 0x0D;
pub const MOUNT_CONTROL: u8 = 0x0E;
pub const MOUNT_STATUS: u8 = 0x0F;

// ... etc
```

Then use a build.rs to generate assembly equates if needed.

---

## Test Pattern

```rust
#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_cpu() -> Intel8080 {
        let mut cpu = Intel8080::new();
        // Setup...
        cpu
    }

    #[test]
    fn test_something() {
        let mut cpu = create_test_cpu();
        // Test...
        assert_eq!(cpu.a, expected);
    }
}
```

---

## Integration Test Pattern

```rust
// tests/monitor_tests.rs

mod common;
use common::TestConsole;

#[test]
fn test_dump_command() {
    let mut cpu = create_cpu_with_rom();
    let console = Rc::new(RefCell::new(TestConsole::new()));
    
    console.borrow_mut().queue_input("D 0 F\r");
    
    // Run until prompt appears...
    
    let output = console.borrow().get_output();
    assert!(output.contains("0000:"));
}
```

---

## ROM Build (Makefile)

```makefile
ASL = asl
P2BIN = p2bin

all: monitor.bin

monitor.bin: monitor.p
	$(P2BIN) monitor.p monitor.bin -r '$$F000-$$FFFF'

monitor.p: monitor.asm
	$(ASL) -L monitor.asm

clean:
	rm -f *.p *.bin *.lst

.PHONY: all clean
```

---

## Assembly Patterns

### Console Output

```asm
CONOUT:
        PUSH    PSW
CONOUT_WAIT:
        IN      CONSOLE_STATUS
        ANI     02H
        JZ      CONOUT_WAIT
        POP     PSW
        OUT     CONSOLE_DATA_OUT
        RET
```

### Parse Hex Word

```asm
; Input: HL = buffer pointer
; Output: DE = value, HL = advanced, Carry = error
READ_HEX_WORD:
        CALL    SKIP_SPACES
        LXI     D,0
        MVI     B,0             ; Digit count
RHW_LOOP:
        MOV     A,M
        CALL    TO_HEX_DIGIT
        JC      RHW_DONE
        ; Shift DE left 4, add digit
        ; ...
        INX     H
        INR     B
        JMP     RHW_LOOP
RHW_DONE:
        MOV     A,B
        ORA     A
        STC
        RZ                      ; No digits = error
        ORA     A               ; Clear carry = success
        RET
```

### Self-Modifying I/O

```asm
; For I/O commands that need variable port numbers
IO_IN_STUB      EQU     00D6H   ; 3 bytes: IN xx / RET
IO_OUT_STUB     EQU     00D9H   ; 3 bytes: OUT xx / RET

; Initialize at cold start:
        MVI     A,0DBH          ; IN opcode
        STA     IO_IN_STUB
        MVI     A,00H           ; Default port
        STA     IO_IN_STUB+1
        MVI     A,0C9H          ; RET opcode
        STA     IO_IN_STUB+2
        
; Usage:
        MOV     A,E             ; Port number
        STA     IO_IN_STUB+1    ; Patch
        CALL    IO_IN_STUB      ; Execute IN port / RET
```
