# Session Summary: ROM Overlay Boot Mechanism

**Date:** December 16, 2025  
**Topic:** Implementing hardware-compatible boot mechanism for 8080 emulator

---

## Problem Statement

The 8080 CPU starts execution at address 0x0000 on reset, but our monitor ROM lives at 0xF000. We need RST vectors in low RAM (0x0000-0x003F) for interrupts to work, but RAM contents are undefined at power-on.

**Constraints:**
- Must be CP/M compatible (full RAM at 0x0000 after boot)
- Must work on future physical hardware build
- Must use single ROM file (not two separate binaries)

---

## Solution: ROM Overlay with Bank Switching

**Mechanism:**
1. On reset, ROM appears at BOTH 0x0000-0x0FFF AND 0xF000-0xFFFF (overlay enabled)
2. CPU starts at PC=0x0000, executes ROM code (mirrored from 0xF000)
3. ROM code jumps to 0xF000+ address space (escaping the overlay region)
4. ROM disables overlay via `OUT 0FEh, 00h`
5. Low memory (0x0000-0x0FFF) becomes RAM
6. ROM copies vectors from high ROM to low RAM
7. System continues normally

**Hardware Implementation (for future physical build):**
- 74LS74 flip-flop controls overlay state
- Set on reset (overlay enabled)
- Cleared by write to port 0xFE
- Address decode logic checks flip-flop for 0x0000-0x0FFF access
- Single EPROM chip - overlay is purely address decoding, not ROM content

**Port Assignments:**
```
0xFE: SYSTEM_CONTROL [Write]
      00h = Disable ROM overlay (expose RAM at 0x0000-0x0FFF)
      01h = Halt CPU
      FFh = Cold reset (re-enable overlay, restart at 0x0000)

0xFF: SYSTEM_STATUS [Read]
      Bit 0: ROM overlay state (1=enabled, 0=disabled)
```

---

## Specification Update

Updated project spec from v1.0 (1775 lines) to v1.1 (2069 lines).

**New sections added:**
- Design Decision #2: Boot Mechanism
- ROM Overlay Mechanism (in Memory Architecture)
- BOOT SEQUENCE section with power-on flow
- System Control Protocol (in Device Protocols)
- EMULATOR IMPLEMENTATION: ROM OVERLAY (Rust code sketches)
- Boot Sequence Quick Reference (in Appendix)

**File:** `Intel_8080_Emulator_-_Complete_Project_Specification_v1.1.md` (2069 lines)  
**Status:** Complete, replaces both v1.0 and truncated v1.1 in project knowledge

---

## Implementation Game Plan

### Phase 1: Rust - CPU Struct Changes

Add to `Intel8080` struct in `cpu.rs`:

```rust
pub struct Intel8080 {
    // ... existing fields ...
    
    // ROM overlay support
    rom: Vec<u8>,               // ROM data (4KB)
    rom_overlay_enabled: bool,  // True after reset, false after OUT 0xFE
}
```

Update `new()`:
```rust
pub fn new() -> Self {
    Intel8080 {
        // ... existing ...
        pc: 0x0000,  // Changed from 0 (was implicitly 0, but now meaningful)
        rom: Vec::new(),
        rom_overlay_enabled: true,
        // ...
    }
}
```

### Phase 2: Rust - Memory Access Changes

Replace `read_byte()`:
```rust
pub fn read_byte(&mut self, addr: u16) -> u8 {
    if addr >= 0xF000 {
        // Always ROM at F000-FFFF
        let rom_addr = (addr - 0xF000) as usize;
        if rom_addr < self.rom.len() {
            return self.rom[rom_addr];
        }
        return 0xFF;
    }
    if addr < 0x1000 && self.rom_overlay_enabled {
        // Overlay: 0000-0FFF mirrors ROM
        if (addr as usize) < self.rom.len() {
            return self.rom[addr as usize];
        }
        return 0xFF;
    }
    self.memory.read(addr)
}
```

Replace `write_byte()`:
```rust
pub fn write_byte(&mut self, addr: u16, value: u8) {
    if addr >= 0xF000 {
        return; // ROM - ignore
    }
    if addr < 0x1000 && self.rom_overlay_enabled {
        return; // Overlay active - ignore
    }
    self.memory.write(addr, value);
}
```

### Phase 3: Rust - Port 0xFE/0xFF Handling

Update `perform_out()`:
```rust
pub fn perform_out(&mut self) -> u8 {
    let port = self.fetch_byte();
    if port >= 0x30 && port <= 0x32 {
        self.timer.write(port, self.a);
    } else if port == 0xFE {
        // System control
        match self.a {
            0x00 => self.rom_overlay_enabled = false,
            0xFF => self.reset(),  // Cold reset
            _ => {}
        }
    } else {
        self.io_bus.write(port, self.a);
    }
    10
}
```

Update `perform_in()`:
```rust
pub fn perform_in(&mut self) -> u8 {
    let port = self.fetch_byte();
    self.a = if port >= 0x30 && port <= 0x32 {
        self.timer.read(port)
    } else if port == 0xFF {
        // System status - bit 0 = overlay state
        if self.rom_overlay_enabled { 0x01 } else { 0x00 }
    } else {
        self.io_bus.read(port)
    };
    10
}
```

### Phase 4: Rust - New Methods

```rust
pub fn reset(&mut self) {
    self.a = 0; self.b = 0; self.c = 0; self.d = 0;
    self.e = 0; self.h = 0; self.l = 0;
    self.flags = FLAG_BIT_1;
    self.sp = 0xF000;
    self.pc = 0x0000;  // Start at 0, overlay provides ROM
    self.halted = false;
    self.interrupts_enabled = false;
    self.rom_overlay_enabled = true;
}

pub fn load_rom(&mut self, rom_data: &[u8]) {
    self.rom = rom_data.to_vec();
}

pub fn load_rom_from_file(&mut self, path: &Path) -> io::Result<usize> {
    self.rom = std::fs::read(path)?;
    Ok(self.rom.len())
}
```

### Phase 5: Rust - Main.rs Update

```rust
fn main() {
    // ... setup ...
    
    let mut cpu = Intel8080::new();
    
    // Load ROM (doesn't set PC - reset() handles that)
    cpu.load_rom_from_file(Path::new("rom/monitor.bin"))
        .expect("Failed to load ROM");
    
    // Set up console device
    let console = Rc::new(RefCell::new(Console::new()));
    cpu.io_bus_mut().map_port(0x00, console.clone());
    cpu.io_bus_mut().map_port(0x01, console.clone());
    cpu.io_bus_mut().map_port(0x02, console);
    
    cpu.run();
    // ...
}
```

### Phase 6: ROM - Assembly Changes

Update `monitor.asm` constants section:
```asm
SYSTEM_CONTROL  EQU     0FEH
SYSTEM_STATUS   EQU     0FFH
```

Update `COLD_START`:
```asm
COLD_START:
        ; Running from overlay (PC near 0x0000, reading ROM)
        LXI     SP,STACK_TOP
        DI
        JMP     BOOT_PHASE2         ; Jump to real ROM address (0xF000+)

; Now PC is in 0xF000 range - safe to disable overlay
BOOT_PHASE2:
        XRA     A                   ; A = 0x00
        OUT     SYSTEM_CONTROL      ; Disable overlay
        
        ; Now 0x0000-0x0FFF is RAM
        ; TODO: Copy RST vectors from ROM to RAM (future enhancement)
        
        ; Continue with existing initialization...
        LXI     H,0000H
        SHLD    LAST_DUMP_ADDR
        SHLD    LAST_EXAM_ADDR
        ; ... rest of init ...
```

**Critical insight:** The `JMP BOOT_PHASE2` escapes the overlay region. After the jump, PC is in the 0xF000+ range, so when overlay is disabled, we're still executing valid ROM code. Without this jump, disabling overlay would cause PC (still near 0x0006) to suddenly be reading garbage RAM.

---

## Current Source Code Architecture

Based on uploaded source zip:

```
intel8080_emu/
├── Cargo.toml
├── build.rs              # Generates BUILD_TIMESTAMP
├── src/
│   ├── main.rs           # Entry point, sets up console, loads ROM
│   ├── lib.rs            # Exports Intel8080
│   ├── cpu.rs            # CPU emulation (1069 lines, all opcodes)
│   ├── memory.rs         # Memory trait + FlatMemory (simple)
│   ├── registers.rs      # Register enums, flags
│   └── io/
│       ├── mod.rs
│       ├── bus.rs        # IoBus with Rc<RefCell<dyn IoDevice>>
│       ├── device.rs     # IoDevice trait (read/write only)
│       └── devices/
│           ├── console.rs
│           ├── timer.rs  # Hardcoded ports 0x30-0x32 in cpu.rs
│           ├── disk.rs
│           └── null.rs
├── rom/
│   ├── monitor.asm       # Monitor ROM source
│   ├── Makefile          # ASL build
│   └── monitor.bin       # Compiled ROM
└── tests/
    └── cpu_tests.rs
```

**Key observations:**
- Memory is a trait (`Memory`) with `FlatMemory` implementation
- ROM will be stored separately in CPU struct (not in Memory)
- Timer is special-cased in `perform_in()`/`perform_out()` (ports 0x30-0x32)
- System control (0xFE/0xFF) will follow same pattern as timer
- Console mapped via IoBus on ports 0x00-0x02

---

## Testing Strategy

1. **Test overlay read**: After reset, `read_byte(0x0000)` should return `rom[0]`
2. **Test overlay write ignored**: After reset, `write_byte(0x0000, 0xFF)` should not change RAM
3. **Test overlay disable**: After `OUT 0xFE` with A=0, `read_byte(0x0000)` returns RAM
4. **Test ROM always accessible**: `read_byte(0xF000)` always returns ROM regardless of overlay
5. **Integration**: Full boot sequence works

---

## Utility: Source Zip Script

Created `scripts/zip_source.sh` to package source for upload:
- Outputs to `/tmp/` (avoids git tracking)
- Excludes: `target/`, `.git/`, `*.bin`, `*.o`, `*.lst`, IDE files, `.DS_Store`
- Run from project root: `./scripts/zip_source.sh`

---

## Files Updated/Created This Session

1. **Spec v1.1** - `Intel_8080_Emulator_-_Complete_Project_Specification_v1.1.md` (2069 lines)
   - Complete replacement for project knowledge
   
2. **Zip script** - `scripts/zip_source.sh`
   - Already in your repo

---

## Next Steps (Priority Order)

1. **Rust changes** - Implement overlay in `cpu.rs` (Phases 1-5 above)
2. **Test overlay** - Write unit tests for overlay behavior
3. **ROM changes** - Update `COLD_START` in `monitor.asm` (Phase 6)
4. **Integration test** - Boot and verify monitor still works
5. **Future** - Add vector copy routine (copy RST vectors from ROM to RAM)

---

## Key Reminders

- **One ROM file** - Overlay is address decoding, not separate binaries
- **Jump before disable** - Must escape overlay region before `OUT 0FEh`
- **Mantra** - "A fool admires complexity, genius admires simplicity"
- **GitHub** - Network disabled from Claude's container; use zip script for uploads
