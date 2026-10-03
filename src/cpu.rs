// cpu.rs - Intel 8080 CPU emulator core
use crate::io::IoBus;
use std::io;
use std::path::Path;

use crate::registers::{Register, RegisterPair, PushPopPair, Condition};
use crate::registers::{FLAG_CARRY, FLAG_BIT_1, FLAG_PARITY, FLAG_AUX_CARRY, FLAG_ZERO, FLAG_SIGN};

/// A data transfer on the bus: address or port, then the byte. Opcode and operand
/// fetches are not transfers (ARCHITECTURE 7.4, Bus transfers).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Transfer {
    MemRead(u16, u8),
    MemWrite(u16, u8),
    In(u8, u8),
    Out(u8, u8),
}

pub struct Intel8080 {
    // Registers
    pub a: u8,
    pub b: u8,
    pub c: u8,
    pub d: u8,
    pub e: u8,
    pub h: u8,
    pub l: u8,
    pub flags: u8,
    pub sp: u16,
    pub pc: u16,

    // Memory and state
    ram: Vec<u8>,
    rom: Vec<u8>,                       // 4KB ROM at 0xF000
    pub rom_overlay_enabled: bool,      // When true, ROM visible at 0x0000 too
    io_bus: IoBus,

    pub halted: bool,
    pub interrupts_enabled: bool,     // INTE
    ei_delay: bool,                   // EI ran last step: no acceptance this step
    pending_interrupt: Option<u8>,    // RST n latched by interrupt(n)
    pub cycles: u64,
    transfers: Vec<Transfer>,         // data transfers of the last step
}

impl Intel8080 {
    /// Power-on. Registers, SP and RAM are undefined on a real 8080 (ARCHITECTURE 3.1);
    /// here they start at 0, and everything RESET defines comes from reset().
    pub fn new() -> Self {
        let mut cpu = Intel8080 {
            a: 0, b: 0, c: 0, d: 0, e: 0, h: 0, l: 0,
            flags: FLAG_BIT_1,
            sp: 0x0000,
            pc: 0x0000,
            ram: vec![0; 0x10000],
            rom: Vec::new(),
            rom_overlay_enabled: true,
            io_bus: IoBus::new(),
            halted: false,
            interrupts_enabled: false,
            ei_delay: false,
            pending_interrupt: None,
            cycles: 0,
            transfers: Vec::new(),
        };
        cpu.reset();
        cpu
    }

    pub fn io_bus_mut(&mut self) -> &mut IoBus {
        &mut self.io_bus
    }

    /// The data transfers of the last step, in bus order.
    pub fn transfers(&self) -> &[Transfer] {
        &self.transfers
    }

    // ============================================
    // REGISTERS
    // ============================================

    pub fn get_bc(&self) -> u16 { ((self.b as u16) << 8) | (self.c as u16) }
    pub fn set_bc(&mut self, val: u16) {
        self.b = (val >> 8) as u8;
        self.c = val as u8;
    }

    pub fn get_de(&self) -> u16 { ((self.d as u16) << 8) | (self.e as u16) }
    pub fn set_de(&mut self, val: u16) {
        self.d = (val >> 8) as u8;
        self.e = val as u8;
    }

    pub fn get_hl(&self) -> u16 { ((self.h as u16) << 8) | (self.l as u16) }
    pub fn set_hl(&mut self, val: u16) {
        self.h = (val >> 8) as u8;
        self.l = val as u8;
    }

    pub fn get_psw(&self) -> u16 { ((self.a as u16) << 8) | (self.flags as u16) }
    pub fn set_psw(&mut self, val: u16) {
        self.a = (val >> 8) as u8;
        self.flags = (val as u8 & 0xD5) | FLAG_BIT_1;  // bits 3 and 5 read as 0
    }

    fn get_reg(&mut self, reg: Register) -> u8 {
        match reg {
            Register::A => self.a,
            Register::B => self.b,
            Register::C => self.c,
            Register::D => self.d,
            Register::E => self.e,
            Register::H => self.h,
            Register::L => self.l,
            Register::M => self.load(self.get_hl()),
        }
    }

    fn set_reg(&mut self, reg: Register, value: u8) {
        match reg {
            Register::A => self.a = value,
            Register::B => self.b = value,
            Register::C => self.c = value,
            Register::D => self.d = value,
            Register::E => self.e = value,
            Register::H => self.h = value,
            Register::L => self.l = value,
            Register::M => self.write_byte(self.get_hl(), value),
        }
    }

    fn get_pair(&self, pair: RegisterPair) -> u16 {
        match pair {
            RegisterPair::BC => self.get_bc(),
            RegisterPair::DE => self.get_de(),
            RegisterPair::HL => self.get_hl(),
            RegisterPair::SP => self.sp,
        }
    }

    fn set_pair(&mut self, pair: RegisterPair, val: u16) {
        match pair {
            RegisterPair::BC => self.set_bc(val),
            RegisterPair::DE => self.set_de(val),
            RegisterPair::HL => self.set_hl(val),
            RegisterPair::SP => self.sp = val,
        }
    }

    fn test_condition(&self, cond: Condition) -> bool {
        match cond {
            Condition::NZ => (self.flags & FLAG_ZERO) == 0,
            Condition::Z  => (self.flags & FLAG_ZERO) != 0,
            Condition::NC => (self.flags & FLAG_CARRY) == 0,
            Condition::C  => (self.flags & FLAG_CARRY) != 0,
            Condition::PO => (self.flags & FLAG_PARITY) == 0,
            Condition::PE => (self.flags & FLAG_PARITY) != 0,
            Condition::P  => (self.flags & FLAG_SIGN) == 0,
            Condition::M  => (self.flags & FLAG_SIGN) != 0,
        }
    }

    // ============================================
    // MEMORY
    // ============================================

    pub fn read_byte(&self, addr: u16) -> u8 {
        // Only apply ROM logic if ROM is loaded
        if !self.rom.is_empty() {
            if addr >= 0xF000 {
                return *self.rom.get((addr - 0xF000) as usize).unwrap_or(&0xFF);
            }
            if addr < 0x1000 && self.rom_overlay_enabled {
                return *self.rom.get(addr as usize).unwrap_or(&0xFF);
            }
        }
        self.ram[addr as usize]
    }

    /// A memory write cycle, logged as a transfer.
    pub fn write_byte(&mut self, addr: u16, value: u8) {
        self.transfers.push(Transfer::MemWrite(addr, value));
        // ROM is selected on reads only: writes under the overlay reach RAM (ARCHITECTURE 4).
        if !self.rom.is_empty() && addr >= 0xF000 {
            return;
        }
        self.ram[addr as usize] = value;
    }

    /// A data read, logged as a transfer. read_byte is the untraced peek.
    fn load(&mut self, addr: u16) -> u8 {
        let value = self.read_byte(addr);
        self.transfers.push(Transfer::MemRead(addr, value));
        value
    }

    fn load_word(&mut self, addr: u16) -> u16 {
        let low = self.load(addr) as u16;
        (self.load(addr.wrapping_add(1)) as u16) << 8 | low
    }

    fn fetch_byte(&mut self) -> u8 {
        let byte = self.read_byte(self.pc);
        self.pc = self.pc.wrapping_add(1);
        byte
    }

    fn fetch_word(&mut self) -> u16 {
        let low = self.fetch_byte() as u16;
        let high = self.fetch_byte() as u16;
        (high << 8) | low
    }

    pub fn read_word(&self, address: u16) -> u16 {
        let low = self.read_byte(address) as u16;
        let high = self.read_byte(address.wrapping_add(1)) as u16;
        (high << 8) | low
    }

    pub fn write_word(&mut self, address: u16, value: u16) {
        self.write_byte(address, value as u8);
        self.write_byte(address.wrapping_add(1), (value >> 8) as u8);
    }

    /// High byte first, to SP-1, then low to SP-2: the 8080's bus order.
    fn push(&mut self, value: u16) {
        self.sp = self.sp.wrapping_sub(1);
        self.write_byte(self.sp, (value >> 8) as u8);
        self.sp = self.sp.wrapping_sub(1);
        self.write_byte(self.sp, value as u8);
    }

    fn pop(&mut self) -> u16 {
        let value = self.load_word(self.sp);
        self.sp = self.sp.wrapping_add(2);
        value
    }

    // ============================================
    // FLAGS
    // ============================================

    /// Sets every flag: S, Z, P from `result`, CY and AC as given.
    fn set_flags(&mut self, result: u8, carry: bool, aux_carry: bool) {
        self.flags = FLAG_BIT_1;
        if result == 0 { self.flags |= FLAG_ZERO; }
        if result & 0x80 != 0 { self.flags |= FLAG_SIGN; }
        if result.count_ones() % 2 == 0 { self.flags |= FLAG_PARITY; }
        if carry { self.flags |= FLAG_CARRY; }
        if aux_carry { self.flags |= FLAG_AUX_CARRY; }
    }

    fn set_carry(&mut self, carry: bool) {
        if carry {
            self.flags |= FLAG_CARRY;
        } else {
            self.flags &= !FLAG_CARRY;
        }
    }

    fn carry(&self) -> bool {
        self.flags & FLAG_CARRY != 0
    }

    // ============================================
    // EXECUTION
    // ============================================

    /// Runs until the CPU halts.
    pub fn run(&mut self) {
        while !self.halted {
            self.execute_one();
        }
    }

    /// The INT input (ARCHITECTURE 5.7): latches RST n until the CPU accepts it.
    /// A later call before acceptance replaces n.
    pub fn interrupt(&mut self, n: u8) {
        assert!(n < 8, "RST {} does not exist", n);
        self.pending_interrupt = Some(n);
    }

    /// One step: an interrupt acknowledge, a halted step, or one instruction.
    /// Returns the cycles it took.
    pub fn execute_one(&mut self) -> u8 {
        self.transfers.clear();
        let ei_delay = std::mem::replace(&mut self.ei_delay, false);
        let cycles = match self.pending_interrupt {
            Some(n) if self.interrupts_enabled && !ei_delay => {
                // The acknowledge executes RST n in place of the next fetch.
                self.pending_interrupt = None;
                self.interrupts_enabled = false;
                self.halted = false;
                self.perform_rst(n << 3)
            }
            _ if self.halted => 4,
            _ => {
                let opcode = self.fetch_byte();
                self.execute(opcode)
            }
        };
        self.cycles += cycles as u64;
        cycles
    }

    fn perform_mov(&mut self, opcode: u8) -> u8 {
        let dest = Register::from_code((opcode >> 3) & 0x07);
        let src = Register::from_code(opcode & 0x07);
        let value = self.get_reg(src);
        self.set_reg(dest, value);
        if dest == Register::M || src == Register::M { 7 } else { 5 }
    }

    /// ADD ADC SUB SBB ANA XRA ORA CMP (op 0-7), for 10AAASSS and 11AAA110.
    fn alu(&mut self, op: u8, value: u8) {
        let cy = self.carry() as u8;
        match op {
            0 | 1 => {  // ADD, ADC
                let ci = if op == 1 { cy } else { 0 };
                let result = self.a as u16 + value as u16 + ci as u16;
                let aux_carry = (self.a & 0x0F) + (value & 0x0F) + ci > 0x0F;
                self.a = result as u8;
                self.set_flags(self.a, result > 0xFF, aux_carry);
            }
            2 | 3 | 7 => {  // SUB, SBB, CMP: A + ~v + (1 - borrow in); AC is the carry out of bit 3
                let bi = if op == 3 { cy } else { 0 };
                let result = (self.a as i16) - (value as i16) - bi as i16;
                let aux_carry = (self.a & 0x0F) + (!value & 0x0F) + (1 - bi) > 0x0F;
                self.set_flags(result as u8, result < 0, aux_carry);
                if op != 7 {
                    self.a = result as u8;
                }
            }
            4 => {
                let aux_carry = (self.a | value) & 0x08 != 0;
                self.a &= value;
                self.set_flags(self.a, false, aux_carry);
            }
            5 => { self.a ^= value; self.set_flags(self.a, false, false); }
            _ => { self.a |= value; self.set_flags(self.a, false, false); }
        }
    }

    fn perform_alu(&mut self, opcode: u8) -> u8 {
        let src = Register::from_code(opcode & 0x07);
        let value = self.get_reg(src);
        self.alu((opcode >> 3) & 0x07, value);
        if src == Register::M { 7 } else { 4 }
    }

    fn perform_mvi(&mut self, opcode: u8) -> u8 {
        let reg = Register::from_code((opcode >> 3) & 0x07);
        let value = self.fetch_byte();
        self.set_reg(reg, value);
        if reg == Register::M { 10 } else { 7 }
    }

    fn perform_inr(&mut self, opcode: u8) -> u8 {
        let reg = Register::from_code((opcode >> 3) & 0x07);
        let value = self.get_reg(reg);
        let result = value.wrapping_add(1);
        self.set_reg(reg, result);
        self.set_flags(result, self.carry(), (value & 0x0F) == 0x0F);
        if reg == Register::M { 10 } else { 5 }
    }

    fn perform_dcr(&mut self, opcode: u8) -> u8 {
        let reg = Register::from_code((opcode >> 3) & 0x07);
        let value = self.get_reg(reg);
        let result = value.wrapping_sub(1);
        self.set_reg(reg, result);
        self.set_flags(result, self.carry(), (value & 0x0F) != 0x00);
        if reg == Register::M { 10 } else { 5 }
    }

    fn perform_daa(&mut self) -> u8 {
        let (lo, hi) = (self.a & 0x0F, self.a >> 4);
        let mut correction = 0;
        if lo > 9 || (self.flags & FLAG_AUX_CARRY) != 0 {
            correction |= 0x06;
        }
        let carry = hi > 9 || self.carry() || (lo > 9 && hi >= 9);
        if carry {
            correction |= 0x60;
        }
        self.a = self.a.wrapping_add(correction);
        self.set_flags(self.a, carry, lo + (correction & 0x0F) > 0x0F);
        4
    }

    fn perform_rst(&mut self, opcode: u8) -> u8 {
        self.push(self.pc);
        self.pc = (opcode & 0x38) as u16;
        11
    }

    fn perform_call(&mut self) -> u8 {
        let addr = self.fetch_word();
        self.push(self.pc);
        self.pc = addr;
        17
    }

    fn perform_out(&mut self) -> u8 {
        let port = self.fetch_byte();
        self.transfers.push(Transfer::Out(port, self.a));
        if port == 0xFE {
            self.rom_overlay_enabled = false;  // any value clears the overlay flip-flop
        } else {
            self.io_bus.write(port, self.a);
        }
        10
    }

    fn perform_in(&mut self) -> u8 {
        let port = self.fetch_byte();
        self.a = if port == 0xFF {
            self.rom_overlay_enabled as u8  // bit 0 = overlay
        } else {
            self.io_bus.read(port)
        };
        self.transfers.push(Transfer::In(port, self.a));
        10
    }

    fn execute(&mut self, opcode: u8) -> u8 {
        match opcode {
            0x00 | 0x08 | 0x10 | 0x18 | 0x20 | 0x28 | 0x30 | 0x38 => 4,  // NOP and undocumented NOPs
            0x76 => { self.halted = true; 7 }  // HLT

            // MOV 01DDDSSS, ALU 10AAASSS
            0x40..=0x7F => self.perform_mov(opcode),
            0x80..=0xBF => self.perform_alu(opcode),

            // MVI 00RRR110, INR 00RRR100, DCR 00RRR101
            b if (b & 0xC7) == 0x06 => self.perform_mvi(opcode),
            b if (b & 0xC7) == 0x04 => self.perform_inr(opcode),
            b if (b & 0xC7) == 0x05 => self.perform_dcr(opcode),

            // LXI 00RP0001, DAD 00RP1001, INX 00RP0011, DCX 00RP1011
            b if (b & 0xCF) == 0x01 => {
                let value = self.fetch_word();
                self.set_pair(RegisterPair::from_code(b >> 4), value);
                10
            }
            b if (b & 0xCF) == 0x09 => {
                let result = self.get_hl() as u32 + self.get_pair(RegisterPair::from_code(b >> 4)) as u32;
                self.set_hl(result as u16);
                self.set_carry(result > 0xFFFF);
                10
            }
            b if (b & 0xCF) == 0x03 => {
                let pair = RegisterPair::from_code(b >> 4);
                self.set_pair(pair, self.get_pair(pair).wrapping_add(1));
                5
            }
            b if (b & 0xCF) == 0x0B => {
                let pair = RegisterPair::from_code(b >> 4);
                self.set_pair(pair, self.get_pair(pair).wrapping_sub(1));
                5
            }

            // PUSH 11RP0101, POP 11RP0001
            b if (b & 0xCF) == 0xC5 => {
                let value = match PushPopPair::from_code(b >> 4) {
                    PushPopPair::BC => self.get_bc(),
                    PushPopPair::DE => self.get_de(),
                    PushPopPair::HL => self.get_hl(),
                    PushPopPair::PSW => self.get_psw(),
                };
                self.push(value);
                11
            }
            b if (b & 0xCF) == 0xC1 => {
                let value = self.pop();
                match PushPopPair::from_code(b >> 4) {
                    PushPopPair::BC => self.set_bc(value),
                    PushPopPair::DE => self.set_de(value),
                    PushPopPair::HL => self.set_hl(value),
                    PushPopPair::PSW => self.set_psw(value),
                }
                10
            }

            // Jcc 11CCC010, Ccc 11CCC100, Rcc 11CCC000
            b if (b & 0xC7) == 0xC2 => {
                let addr = self.fetch_word();
                if self.test_condition(Condition::from_code(b >> 3)) {
                    self.pc = addr;
                }
                10
            }
            b if (b & 0xC7) == 0xC4 => {
                if self.test_condition(Condition::from_code(b >> 3)) {
                    self.perform_call()
                } else {
                    self.pc = self.pc.wrapping_add(2);
                    11
                }
            }
            b if (b & 0xC7) == 0xC0 => {
                if self.test_condition(Condition::from_code(b >> 3)) {
                    self.pc = self.pop();
                    11
                } else {
                    5
                }
            }

            // ALU immediate 11AAA110, RST 11NNN111
            b if (b & 0xC7) == 0xC6 => {
                let value = self.fetch_byte();
                self.alu((b >> 3) & 0x07, value);
                7
            }
            b if (b & 0xC7) == 0xC7 => self.perform_rst(opcode),

            // JMP, CALL, RET and their undocumented aliases (ARCHITECTURE 5.4)
            0xC3 | 0xCB => { self.pc = self.fetch_word(); 10 }
            0xCD | 0xDD | 0xED | 0xFD => self.perform_call(),
            0xC9 | 0xD9 => { self.pc = self.pop(); 10 }

            0x02 => { self.write_byte(self.get_bc(), self.a); 7 }  // STAX B
            0x12 => { self.write_byte(self.get_de(), self.a); 7 }  // STAX D
            0x0A => { self.a = self.load(self.get_bc()); 7 }  // LDAX B
            0x1A => { self.a = self.load(self.get_de()); 7 }  // LDAX D
            0x32 => { let addr = self.fetch_word(); self.write_byte(addr, self.a); 13 }   // STA
            0x3A => { let addr = self.fetch_word(); self.a = self.load(addr); 13 }   // LDA
            0x22 => { let addr = self.fetch_word(); self.write_word(addr, self.get_hl()); 16 }  // SHLD
            0x2A => { let addr = self.fetch_word(); let v = self.load_word(addr); self.set_hl(v); 16 }  // LHLD

            0x07 => { let cy = self.a & 0x80 != 0; self.a = self.a.rotate_left(1); self.set_carry(cy); 4 }  // RLC
            0x0F => { let cy = self.a & 0x01 != 0; self.a = self.a.rotate_right(1); self.set_carry(cy); 4 } // RRC
            0x17 => { let cy = self.a & 0x80 != 0; self.a = (self.a << 1) | self.carry() as u8; self.set_carry(cy); 4 }        // RAL
            0x1F => { let cy = self.a & 0x01 != 0; self.a = (self.a >> 1) | (self.carry() as u8) << 7; self.set_carry(cy); 4 } // RAR

            0x27 => self.perform_daa(),
            0x2F => { self.a = !self.a; 4 }             // CMA
            0x37 => { self.flags |= FLAG_CARRY; 4 }     // STC
            0x3F => { self.flags ^= FLAG_CARRY; 4 }     // CMC

            0xD3 => self.perform_out(),
            0xDB => self.perform_in(),

            0xE3 => {  // XTHL
                let top = self.load_word(self.sp);
                self.write_byte(self.sp.wrapping_add(1), self.h);  // 8080 bus order: H, then L
                self.write_byte(self.sp, self.l);
                self.set_hl(top);
                18
            }
            0xE9 => { self.pc = self.get_hl(); 5 }      // PCHL
            0xEB => {                                   // XCHG
                let de = self.get_de();
                self.set_de(self.get_hl());
                self.set_hl(de);
                4
            }
            0xF3 => { self.interrupts_enabled = false; 4 }  // DI
            0xF9 => { self.sp = self.get_hl(); 5 }          // SPHL
            0xFB => { self.interrupts_enabled = true; self.ei_delay = true; 4 }  // EI

            _ => unreachable!("all 256 opcodes are decoded above"),
        }
    }

    // ============================================
    // LOADING AND RESET
    // ============================================

    /// Writes `program` at `start_address`, bypassing the ROM and the overlay, and sets PC there.
    pub fn load_program(&mut self, program: &[u8], start_address: u16) {
        for (i, &byte) in program.iter().enumerate() {
            self.ram[start_address.wrapping_add(i as u16) as usize] = byte;
        }
        self.pc = start_address;
    }

    /// The RESET pin (ARCHITECTURE 3.1). Registers, flags, SP and RAM are left alone.
    pub fn reset(&mut self) {
        self.pc = 0x0000;
        self.interrupts_enabled = false;
        self.ei_delay = false;
        self.pending_interrupt = None;
        self.halted = false;
        self.rom_overlay_enabled = true;
    }

    /// Load ROM data (mapped at 0xF000, and at 0x0000 while the overlay is set)
    pub fn load_rom(&mut self, rom_data: &[u8]) {
        self.rom = rom_data.to_vec();
    }

    pub fn load_rom_from_file(&mut self, path: &Path) -> io::Result<usize> {
        self.rom = std::fs::read(path)?;
        Ok(self.rom.len())
    }
}
