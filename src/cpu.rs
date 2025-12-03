// cpu.rs - Intel 8080 CPU emulator core
use crate::memory::{Memory, FlatMemory};
use crate::io::IoBus;
use crate::io::devices::timer::Timer;
use crate::io::IoDevice;
use std::io;
use std::path::Path;


use crate::registers::{Register, RegisterPair, PushPopPair, Condition};
use crate::registers::{FLAG_CARRY, FLAG_BIT_1, FLAG_PARITY, FLAG_AUX_CARRY, FLAG_ZERO, FLAG_SIGN};

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
    memory: Box<dyn Memory>,
    io_bus: IoBus, 
    pub timer: Timer,

    pub halted: bool,
    pub interrupts_enabled: bool,
    pub cycles: u64,
}

impl Intel8080 {
    pub fn new() -> Self {
        Intel8080 {
            a: 0, b: 0, c: 0, d: 0, e: 0, h: 0, l: 0,
            flags: FLAG_BIT_1,
            sp: 0xF000,
            pc: 0,
            memory: Box::new(FlatMemory::new()),
            io_bus: IoBus::new(),
            timer: Timer::new(),
            halted: false,
            interrupts_enabled: false,
            cycles: 0,
        }
    }
    
    pub fn io_bus_mut(&mut self) -> &mut IoBus {
        &mut self.io_bus
    }
    // ============================================
    // LAYER 1: Direct register access
    // ============================================
    
    #[inline]
    pub fn get_bc(&self) -> u16 { ((self.b as u16) << 8) | (self.c as u16) }
    #[inline]
    pub fn set_bc(&mut self, val: u16) { 
        self.b = (val >> 8) as u8; 
        self.c = val as u8; 
    }
    
    #[inline]
    pub fn get_de(&self) -> u16 { ((self.d as u16) << 8) | (self.e as u16) }
    #[inline]
    pub fn set_de(&mut self, val: u16) { 
        self.d = (val >> 8) as u8; 
        self.e = val as u8; 
    }
    
    #[inline]
    pub fn get_hl(&self) -> u16 { ((self.h as u16) << 8) | (self.l as u16) }
    #[inline]
    pub fn set_hl(&mut self, val: u16) { 
        self.h = (val >> 8) as u8; 
        self.l = val as u8; 
    }
    
    #[inline]
    pub fn get_psw(&self) -> u16 { ((self.a as u16) << 8) | (self.flags as u16) }
    #[inline]
    pub fn set_psw(&mut self, val: u16) { 
        self.a = (val >> 8) as u8; 
        self.flags = (val as u8) | FLAG_BIT_1;
    }
    
    // ============================================
    // LAYER 2: Enum-based access
    // ============================================
    
    #[inline]
    pub fn get_reg(&mut self, reg: Register) -> u8 {
        match reg {
            Register::A => self.a,
            Register::B => self.b,
            Register::C => self.c,
            Register::D => self.d,
            Register::E => self.e,
            Register::H => self.h,
            Register::L => self.l,
            Register::M => self.read_byte(self.get_hl())    //memory[self.get_hl() as usize],
        }
    }
    
    #[inline]
    pub fn set_reg(&mut self, reg: Register, value: u8) {
        match reg {
            Register::A => self.a = value,
            Register::B => self.b = value,
            Register::C => self.c = value,
            Register::D => self.d = value,
            Register::E => self.e = value,
            Register::H => self.h = value,
            Register::L => self.l = value,
            Register::M => self.write_byte(self.get_hl() , value),//memory[self.get_hl() as usize] = val,
        }
    }
    
    #[inline]
    pub fn get_pair(&self, pair: RegisterPair) -> u16 {
        match pair {
            RegisterPair::BC => self.get_bc(),
            RegisterPair::DE => self.get_de(),
            RegisterPair::HL => self.get_hl(),
            RegisterPair::SP => self.sp,
        }
    }
    
    #[inline]
    pub fn set_pair(&mut self, pair: RegisterPair, val: u16) {
        match pair {
            RegisterPair::BC => self.set_bc(val),
            RegisterPair::DE => self.set_de(val),
            RegisterPair::HL => self.set_hl(val),
            RegisterPair::SP => self.sp = val,
        }
    }
    
    #[inline]
    pub fn test_condition(&self, cond: Condition) -> bool {
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
    
    #[inline]
    pub fn get_push_pop_pair(&self, pair: PushPopPair) -> u16 {
        match pair {
            PushPopPair::BC => self.get_bc(),
            PushPopPair::DE => self.get_de(),
            PushPopPair::HL => self.get_hl(),
            PushPopPair::PSW => self.get_psw(),
        }
    }
    
    #[inline]
    pub fn set_push_pop_pair(&mut self, pair: PushPopPair, val: u16) {
        match pair {
            PushPopPair::BC => self.set_bc(val),
            PushPopPair::DE => self.set_de(val),
            PushPopPair::HL => self.set_hl(val),
            PushPopPair::PSW => self.set_psw(val),
        }
    }
    // ============================================
    // MEMORY HELPERS
    // ============================================
    
    #[inline]
    pub fn read_byte(&mut self, addr: u16) -> u8 {
        self.memory.read(addr)
    }
    
    #[inline]
    pub fn write_byte(&mut self, addr: u16, value: u8) {
        self.memory.write(addr, value)
    }
    #[inline]
    pub fn fetch_byte(&mut self) -> u8 {
        let byte = self.read_byte(self.pc);
        self.pc = self.pc.wrapping_add(1);
        byte
    }
    
    #[inline]
    pub fn fetch_word(&mut self) -> u16 {
        let low = self.fetch_byte() as u16;
        let high = self.fetch_byte() as u16;
        (high << 8) | low
    }
    
    #[inline]
    pub fn read_word(&mut self, address: u16) -> u16 {
        let low = self.read_byte(address) as u16;
        let high = self.read_byte(address.wrapping_add(1)) as u16;
        (high << 8) | low
    }
    
    #[inline]
    pub fn write_word(&mut self, address: u16, value: u16) {
        self.write_byte(address, value as u8);
        self.write_byte(address.wrapping_add(1), (value >> 8) as u8);
    }
    
    // ============================================
    // FLAG HELPERS
    // ============================================
    
        pub fn update_flags(&mut self, result: u8, carry: bool) {
            println!("update_flags: result={}, carry={}", result, carry);
            self.flags = FLAG_BIT_1;
            
            if result == 0 { 
                println!("  Setting FLAG_ZERO (0x{:02X})", FLAG_ZERO);
                self.flags |= FLAG_ZERO; 
            }
            if result & 0x80 != 0 { self.flags |= FLAG_SIGN; }
            if result.count_ones() % 2 == 0 { self.flags |= FLAG_PARITY; }
            if carry { self.flags |= FLAG_CARRY; }
            println!("  Final flags: {:08b}", self.flags);
        }
            fn update_flags_arithmetic(&mut self, result: u8, carry: bool, aux_carry: bool) {
            self.flags = FLAG_BIT_1;
            
            if result == 0 { self.flags |= FLAG_ZERO; }
            if result & 0x80 != 0 { self.flags |= FLAG_SIGN; }
            if result.count_ones() % 2 == 0 { self.flags |= FLAG_PARITY; }
            if carry { self.flags |= FLAG_CARRY; }
            if aux_carry { self.flags |= FLAG_AUX_CARRY; }
        }

    fn update_flags_logical(&mut self, result: u8) {
        self.flags = FLAG_BIT_1;
        
        if result == 0 { self.flags |= FLAG_ZERO; }
        if result & 0x80 != 0 { self.flags |= FLAG_SIGN; }
        if result.count_ones() % 2 == 0 { self.flags |= FLAG_PARITY; }
        // Carry and aux carry are cleared
    }
    // ============================================
    // MAIN EXECUTION
    // ============================================
    
    pub fn run(&mut self) {
        while !self.halted {
            self.execute_one();
        }
    }
//
    fn handle_interrupt(&mut self) {
        // Disable interrupts (8080 does this automatically)
        self.interrupts_enabled = false;
        
        // Push PC onto stack
        self.sp = self.sp.wrapping_sub(2);
        self.write_word(self.sp, self.pc);
        
        // Jump to interrupt vector (typically RST 7 = 0x0038)
        self.pc = 0x0038;
        
        // Clear the interrupt
        self.timer.interrupt_pending = false;
    }

    pub fn perform_nop(&mut self) -> u8{
        // Do nothing
        4
    }

    pub fn perform_hlt(&mut self) -> u8{
        self.halted = true;
        7
    }

    pub fn perform_mov(&mut self, opcode: u8) -> u8 {
        let dest = Register::from_code((opcode >> 3) & 0x07);
        let src = Register::from_code(opcode & 0x07);
        let value = self.get_reg(src);
        self.set_reg(dest, value);
        if dest == Register::M || src == Register::M {
            7
        } else {
            5
        }
    }

    pub fn perform_alu(&mut self, opcode: u8) -> u8{
        let operation = (opcode >> 3) & 0x07;
        let src = Register::from_code(opcode & 0x07);
        let value = self.get_reg(src);
        
        match operation {
            0 => {  // ADD
                let result = self.a as u16 + value as u16;
                let aux_carry = (self.a & 0x0F) + (value & 0x0F) > 0x0F;
                self.a = result as u8;
                self.update_flags_arithmetic(self.a, result > 0xFF, aux_carry);
            }
            1 => {  // ADC (add with carry)
                let carry_in = if self.flags & FLAG_CARRY != 0 { 1 } else { 0 };
                let result = self.a as u16 + value as u16 + carry_in;
                let aux_carry = (self.a & 0x0F) + (value & 0x0F) + carry_in as u8 > 0x0F;
                self.a = result as u8;
                self.update_flags_arithmetic(self.a, result > 0xFF, aux_carry);
            }
            2 => {  // SUB
                let result = (self.a as i16) - (value as i16);
                let aux_borrow = (self.a & 0x0F) < (value & 0x0F);
                self.a = result as u8;
                self.update_flags_arithmetic(self.a, result < 0,aux_borrow);
            }
            3 => {  // SBB (subtract with borrow)
                let carry = if self.flags & FLAG_CARRY != 0 { 1 } else { 0 };
                let result = (self.a as i16) - (value as i16) - carry;
                let aux_borrow = (self.a as i16 & 0x0F) - (value as i16 & 0x0F) - carry < 0;
                self.a = result as u8;
                self.update_flags_arithmetic(self.a, result < 0, aux_borrow);  
            }
            4 => {  // ANA (AND)
                self.a &= value;
                self.update_flags_logical(self.a);
            }
            5 => {  // XRA
                self.a ^= value;
                self.update_flags_logical(self.a);
            }
            6 => {  // ORA (OR)
                self.a |= value;
                self.update_flags_logical(self.a);
            }
            7 => {  // CMP (compare)
                let result = (self.a as i16) - (value as i16);
                let aux_borrow = (self.a & 0x0F) < (value & 0x0F);
                self.update_flags_arithmetic(result as u8, result < 0, aux_borrow); 
                // CMP doesn't change A, only flags
            }
            _ => unreachable!(),
        }
        if src == Register::M {
            7
        } else {
            4
        }
    }

    pub fn perform_mvi(&mut self, opcode: u8) -> u8{
        let reg = Register::from_code((opcode >> 3) & 0x07);
        let value = self.fetch_byte();
        self.set_reg(reg, value);
        if reg == Register::M {
            10
        } else {
            7
        }
    }

    pub fn perform_inr(&mut self, opcode: u8) -> u8{
        let reg = Register::from_code((opcode >> 3) & 0x07);
        let value = self.get_reg(reg);
        let result = value.wrapping_add(1);
        let aux_carry = (value & 0x0F) == 0x0F;  // Overflow from bit 3
        
        self.set_reg(reg, result);
        
        // Preserve carry, set everything else
        let carry = self.flags & FLAG_CARRY;
        self.update_flags_arithmetic(result, false, aux_carry);
        self.flags = (self.flags & !FLAG_CARRY) | carry;
        
        if reg == Register::M {
            10
        } else {
            5
        }
    }

    pub fn perform_dcr(&mut self, opcode: u8) -> u8{
        let reg = Register::from_code((opcode >> 3) & 0x07);
        let value = self.get_reg(reg);
        let result = value.wrapping_sub(1);
        let aux_borrow = (value & 0x0F) == 0x00;  // Borrow from bit 4
        
        self.set_reg(reg, result);
        
        // Preserve carry, set everything else
        let carry = self.flags & FLAG_CARRY;
        self.update_flags_arithmetic(result, false, aux_borrow);
        self.flags = (self.flags & !FLAG_CARRY) | carry;
        
        if reg == Register::M {
            10
        } else {
            5
        }
    }

    pub fn perform_lxi(&mut self, opcode: u8) -> u8{
        let pair = RegisterPair::from_code((opcode >> 4) & 0x03);
        let value = self.fetch_word();
        self.set_pair(pair, value);
        10
    }
    
    pub fn perform_dad(&mut self, opcode: u8) -> u8{
        let pair = RegisterPair::from_code((opcode >> 4) & 0x03);
        let value = self.get_pair(pair);
        let hl = self.get_hl();
        let result = hl as u32 + value as u32;
        self.set_hl(result as u16);
        // Set carry flag if overflow from 16 bits
        if result > 0xFFFF {
            self.flags |= FLAG_CARRY;
        } else {
            self.flags &= !FLAG_CARRY;
        }
        10
    }

    pub fn perform_inx(&mut self, opcode: u8) -> u8{
        let pair = RegisterPair::from_code((opcode >> 4) & 0x03);
        let value = self.get_pair(pair).wrapping_add(1);
        self.set_pair(pair, value);
        // INX doesn't affect flags
        5
    }

    pub fn perform_dcx(&mut self, opcode: u8) -> u8{
        let pair = RegisterPair::from_code((opcode >> 4) & 0x03);
        let value = self.get_pair(pair).wrapping_sub(1);
        self.set_pair(pair, value);
        // DCX doesn't affect flags
        5
    }

    pub fn perform_push(&mut self, opcode: u8) -> u8{
        let pair = PushPopPair::from_code((opcode >> 4) & 0x03);
        let value = self.get_push_pop_pair(pair);
        self.sp = self.sp.wrapping_sub(2);
        self.write_word(self.sp, value);
        11
    }

    pub fn perform_pop(&mut self, opcode: u8) -> u8{
        let pair = PushPopPair::from_code((opcode >> 4) & 0x03);
        let value = self.read_word(self.sp);
        self.set_push_pop_pair(pair, value);
        self.sp = self.sp.wrapping_add(2);
        10
    }

    pub fn perform_conditional_jump(&mut self, opcode: u8) -> u8{
        let condition = Condition::from_code((opcode >> 3) & 0x07);
        let addr = self.fetch_word();
        if self.test_condition(condition) {
            self.pc = addr;
            10
        } else {
            10
        }
    }
    
    pub fn perform_conditional_call(&mut self, opcode: u8) -> u8{
        let condition = Condition::from_code((opcode >> 3) & 0x07);
        let addr = self.fetch_word();
        if self.test_condition(condition) {
            self.sp = self.sp.wrapping_sub(2);
            self.write_word(self.sp, self.pc);
            self.pc = addr;
            17
        } else {
            11
        }
    }

    pub fn perform_conditional_return(&mut self, opcode: u8) -> u8{
        let condition = Condition::from_code((opcode >> 3) & 0x07);
        if self.test_condition(condition) {
            self.pc = self.read_word(self.sp);
            self.sp = self.sp.wrapping_add(2);
            11
        } else {
            5
        }
    }

    pub fn perform_rst(&mut self, opcode: u8) -> u8{
        let n = (opcode >> 3) & 0x07;
        let addr = n as u16 * 8;
        self.sp = self.sp.wrapping_sub(2);
        self.write_word(self.sp, self.pc);
        self.pc = addr;
        11
    }

    pub fn perform_jmp(&mut self) -> u8{
        let addr = self.fetch_word();
        self.pc = addr;
        10
    }

    pub fn perform_call(&mut self) -> u8{
        let addr = self.fetch_word();
        self.sp = self.sp.wrapping_sub(2);
        self.write_word(self.sp, self.pc);
        self.pc = addr;
        17
    }

    pub fn perform_ret(&mut self) -> u8{
        self.pc = self.read_word(self.sp);
        self.sp = self.sp.wrapping_add(2);
        10
    }

    pub fn perform_stax_b(&mut self) -> u8{
        self.write_byte(self.get_bc(), self.a);
        7
    }

    pub fn perform_stax_d(&mut self) -> u8{
        self.write_byte(self.get_de(), self.a);
        7
    }

    pub fn perform_ldax_b(&mut self) -> u8{
        self.a = self.read_byte(self.get_bc());
        7
    }

    pub fn perform_ldax_d(&mut self) -> u8{
        self.a = self.read_byte(self.get_de());
        7
    }

    pub fn perform_sta(&mut self) -> u8{
        let addr = self.fetch_word();
        self.write_byte(addr, self.a);
        13
    }

    pub fn perform_lda(&mut self) -> u8{
        let addr = self.fetch_word();
        self.a = self.read_byte(addr);
        13
    }
    
    pub fn perform_shld(&mut self) -> u8{
        let addr = self.fetch_word();
        self.write_word(addr, self.get_hl());
        16
    }

    pub fn perform_lhld(&mut self) -> u8{
        let addr = self.fetch_word();
        let value = self.read_word(addr);
        self.set_hl(value);
        16
    }

    pub fn perform_adi(&mut self) -> u8{
        let data = self.fetch_byte();
        let result = self.a as u16 + data as u16;
        let aux_carry = (self.a & 0x0F) + (data & 0x0F) > 0x0F;
        self.a = result as u8;
        self.update_flags_arithmetic(self.a, result > 0xFF, aux_carry);
        7
    }

    pub fn perform_sui(&mut self) -> u8{
        let data = self.fetch_byte();
        let result = (self.a as i16) - (data as i16);
        let aux_borrow = (self.a & 0x0F) < (data & 0x0F);  // ← ADD THIS
        self.a = result as u8;
        self.update_flags_arithmetic(self.a, result < 0,aux_borrow);
        7
    }

    pub fn perform_ani(&mut self) -> u8{
        let data = self.fetch_byte();
        self.a &= data;
        self.update_flags_logical(self.a);  // ← Uses logical version
        7
    }

    pub fn perform_xri(&mut self) -> u8{
        let data = self.fetch_byte();
        self.a ^= data;
        self.update_flags_logical(self.a);  // ← Uses logical version
        7
    }

    pub fn perform_ori(&mut self) -> u8{
        let data = self.fetch_byte();
        self.a |= data;
        self.update_flags_logical(self.a);  // ← Uses logical version
        7
    }

    pub fn perform_cpi(&mut self) -> u8{
        let data = self.fetch_byte();
        let result = (self.a as i16) - (data as i16);
        let aux_borrow = (self.a & 0x0F) < (data & 0x0F);  // ← ADD
        self.update_flags_arithmetic(result as u8, result < 0, aux_borrow); 
        // CPI doesn't change A, only flags
        7
    }

    pub fn perform_rlc(&mut self) -> u8{
        let carry = (self.a & 0x80) != 0;
        self.a = (self.a << 1) | if carry { 1 } else { 0 };
        if carry {
            self.flags |= FLAG_CARRY;
        } else {
            self.flags &= !FLAG_CARRY;
        }
        4
    }

    pub fn perform_rrc(&mut self) -> u8{
        let carry = (self.a & 0x01) != 0;
        self.a = (self.a >> 1) | if carry { 0x80 } else { 0 };
        if carry {
            self.flags |= FLAG_CARRY;
        } else {
            self.flags &= !FLAG_CARRY;
        }
        4
    }

    pub fn perform_ral(&mut self) -> u8{
        let carry_in = if self.flags & FLAG_CARRY != 0 { 1 } else { 0 };
        let carry_out = (self.a & 0x80) != 0;
        self.a = (self.a << 1) | carry_in;
        if carry_out {
            self.flags |= FLAG_CARRY;
        } else {
            self.flags &= !FLAG_CARRY;
        }
        4
    }

    pub fn perform_rar(&mut self) -> u8{
        let carry_in = if self.flags & FLAG_CARRY != 0 { 0x80 } else { 0 };
        let carry_out = (self.a & 0x01) != 0;
        self.a = (self.a >> 1) | carry_in;
        if carry_out {
            self.flags |= FLAG_CARRY;
        } else {
            self.flags &= !FLAG_CARRY;
        }
        4
    }

    pub fn perform_daa(&mut self) -> u8{
        let mut correction = 0;
        let mut carry = false;

        // Check lower nibble
        if (self.a & 0x0F) > 9 || (self.flags & FLAG_AUX_CARRY) != 0 {
            correction |= 0x06;
        }

        // Check upper nibble
        if (self.a >> 4) > 9 || (self.flags & FLAG_CARRY) != 0 || ((self.a & 0x0F) > 9 && (self.a >> 4) >= 9) {
            correction |= 0x60;
            carry = true;
        }

        let result = self.a.wrapping_add(correction);
        self.a = result;
        self.update_flags(result, carry);
        if carry {
            self.flags |= FLAG_CARRY;
        } else {
            self.flags &= !FLAG_CARRY;
        }
        4
    }

    pub fn perform_cma(&mut self) -> u8{
        self.a = !self.a;
        4
    }

    pub fn perform_stc(&mut self) -> u8{
        self.flags |= FLAG_CARRY;
        4
    }

    pub fn perform_cmc(&mut self) -> u8{
        self.flags ^= FLAG_CARRY;
        4
    }

    pub fn perform_aci(&mut self) -> u8{
        let data = self.fetch_byte();
        let carry_in = if self.flags & FLAG_CARRY != 0 { 1 } else { 0 };
        let result = self.a as u16 + data as u16 + carry_in;
        let aux_carry = (self.a & 0x0F) + (data & 0x0F) + carry_in as u8 > 0x0F;
        self.a = result as u8;
        self.update_flags_arithmetic(self.a, result > 0xFF, aux_carry);
        7
    }

    pub fn perform_sbi(&mut self) -> u8{
        let data = self.fetch_byte();
        let carry = if self.flags & FLAG_CARRY != 0 { 1 } else { 0 };
        let result = (self.a as i16) - (data as i16) - carry;
        let aux_borrow = (self.a as i16 & 0x0F) - (data as i16 & 0x0F) - carry < 0;  // ← ADD THIS
        self.a = result as u8;
        self.update_flags_arithmetic(self.a, result < 0, aux_borrow);  
        7
    }

    pub fn perform_out(&mut self) -> u8{
        let port = self.fetch_byte();
        if port >= 0x30 && port <= 0x32 {
            self.timer.write(port, self.a);
        } else {
            self.io_bus.write(port, self.a);
        }
        10
    }

    pub fn perform_in(&mut self) -> u8{
        let port = self.fetch_byte();
        self.a = if port >= 0x30 && port <= 0x32 {
            self.timer.read(port)
        } else {
            self.io_bus.read(port)
        };
        10
    }

    pub fn perform_xthl(&mut self) -> u8{
        let temp = self.read_word(self.sp);
        self.write_word(self.sp, self.get_hl());
        self.set_hl(temp);
        18
    }

    pub fn perform_pchl(&mut self) -> u8{
        self.pc = self.get_hl();
        5
    }

    pub fn perform_xchg(&mut self) -> u8{
        let temp = self.get_de();
        self.set_de(self.get_hl());
        self.set_hl(temp);
        4
    }

    pub fn perform_ei(&mut self) -> u8{
        self.interrupts_enabled = true;
        4
    }

    pub fn perform_sphl(&mut self) -> u8{
        self.sp = self.get_hl();
        5
    }

    pub fn perform_di(&mut self) -> u8{
        self.interrupts_enabled = false;
        4
    }

    pub fn perform_nop_undoc(&mut self) -> u8{
        // Do nothing
        4
    }
    
    pub fn execute_one(&mut self) -> u8 {
        if self.interrupts_enabled && self.timer.interrupt_pending {
            self.handle_interrupt();
        }
        
        let opcode = self.fetch_byte();
        let cycles = match opcode {
            // ===== SPECIAL CASES FIRST =====
            0x00 => self.perform_nop(),  // NOP
            0x76 => self.perform_hlt(),  // HLT
            
            // ===== MOV FAMILY: 01DDDSSS (0x40-0x7F) =====
            0x40..=0x7F => self.perform_mov(opcode),
            
            // ===== ARITHMETIC FAMILY: 10AAASSS (0x80-0xBF) =====
            0x80..=0xBF => self.perform_alu(opcode),
            
            // ===== MVI FAMILY: 00RRR110 =====
            b if (b & 0xC7) == 0x06 => self.perform_mvi(opcode),
            
            // ===== INR FAMILY: 00RRR100 =====
            b if (b & 0xC7) == 0x04 => self.perform_inr(opcode),
            
            // ===== DCR FAMILY: 00RRR101 =====
            b if (b & 0xC7) == 0x05 => self.perform_dcr(opcode),
            
            // ===== LXI FAMILY: 00RP0001 =====
            b if (b & 0xCF) == 0x01 => self.perform_lxi(opcode),
            
            // ===== DAD FAMILY: 00RP1001 =====
            b if (b & 0xCF) == 0x09 => self.perform_dad(opcode),
            
            // ===== INX FAMILY: 00RP0011 =====
            b if (b & 0xCF) == 0x03 => self.perform_inx(opcode),
            
            // ===== DCX FAMILY: 00RP1011 =====
            b if (b & 0xCF) == 0x0B => self.perform_dcx(opcode),
            
            // ===== PUSH FAMILY: 11RP0101 =====
            b if (b & 0xCF) == 0xC5 => self.perform_push(opcode),
            
            // ===== POP FAMILY: 11RP0001 =====
            b if (b & 0xCF) == 0xC1 => self.perform_pop(opcode),
            
            // ===== CONDITIONAL JUMPS: 11CCC010 =====
            b if (b & 0xC7) == 0xC2 => self.perform_conditional_jump(opcode),
            
            // ===== CONDITIONAL CALLS: 11CCC100 =====
            b if (b & 0xC7) == 0xC4 => self.perform_conditional_call(opcode),

            // ===== CONDITIONAL RETURNS: 11CCC000 =====
            b if (b & 0xC7) == 0xC0 => self.perform_conditional_return(opcode),
            
            // ===== RST FAMILY: 11NNN111 =====
            b if (b & 0xC7) == 0xC7 => self.perform_rst(opcode),
            
            // ===== SINGLE INSTRUCTIONS =====
            0xC3 => self.perform_jmp(),  // JMP
            0xCD => self.perform_call(), // CALL
            0xC9 => self.perform_ret(),  // RET

    
            // STAX/LDAX
            0x02 => self.perform_stax_b(),   //memory[self.get_bc() as usize] = self.a,  // STAX B
            0x12 => self.perform_stax_d(),   //memory[self.get_de() as usize] = self.a,  // STAX D
            0x0A => self.perform_ldax_b(),    //self.memory[self.get_bc() as usize],  // LDAX B
            0x1A => self.perform_ldax_d(),     //self.memory[self.get_de() as usize],  // LDAX D
            
            // Direct memory operations
            0x32 => self.perform_sta(),  // STA //self.memory[addr as usize] = self.a;
            
            0x3A => self.perform_lda(), // LDA //self.a = self.memory[addr as usize];
            0x22 => self.perform_shld(),  // SHLD
            0x2A => self.perform_lhld(),  // LHLD
            
            // Immediate arithmetic
            0xC6 => self.perform_adi(),  // ADI
            0xD6 => self.perform_sui(), // SUI
            0xE6 => self.perform_ani(),  // ANI
            0xEE => self.perform_xri(),  // XRI
            0xF6 => self.perform_ori(),  // ORI
            0xFE => self.perform_cpi(),  // CPI

            // ===== ROTATE INSTRUCTIONS =====
            0x07 => self.perform_rlc(),  // RLC
            0x0F => self.perform_rrc(),  // RRC     
            0x17 => self.perform_ral(),  
            0x1F => self.perform_rar(),  // RAR

            // ===== ACCUMULATOR OPERATIONS =====
            0x27 => self.perform_daa(),  // DAA - Decimal Adjust Accumulator

            0x2F => self.perform_cma(),  // CMA - Complement Accumulator

            0x37 => self.perform_stc(),  // STC - Set Carry
            0x3F => self.perform_cmc(),  // CMC - Complement Carry

            // ===== IMMEDIATE WITH CARRY =====
            0xCE => self.perform_aci(),  // ACI - Add Immediate with Carry
            0xDE => self.perform_sbi(),  // SBI - Subtract Immediate with Borrow


            // ===== I/O INSTRUCTIONS =====
            0xD3 => self.perform_out(),  // OUT
            0xDB => self.perform_in(),   // IN

            // ===== EXCHANGE INSTRUCTIONS =====
            0xE3 => self.perform_xthl(),  // XTHL - Exchange Top of Stack with HL
            0xE9 => self.perform_pchl(),  // PCHL - Load PC from HL
            0xEB => self.perform_xchg(),  // XCHG - Exchange DE and HL  

            // ===== STACK/INTERRUPT =====
            0xF3 => self.perform_di(),  // DI - Disable Interrupts
            0xF9 => self.perform_sphl(),  // SPHL - Load SP from HL 
            0xFB => self.perform_ei(),  // EI - Enable Interrupts

            // ===== UNDOCUMENTED NOPs =====
            0x08 | 0x10 | 0x18 | 0x20 | 0x28 | 0x30 | 0x38 => self.perform_nop_undoc(),

            _ => panic!("Unknown opcode: 0x{:02X} at PC: 0x{:04X}", 
                       opcode, self.pc.wrapping_sub(1)),
        };
        self.timer.tick(cycles as u64);

        self.cycles += cycles as u64;  // <-- ADD THIS

        cycles
    }
    
    // ============================================
    // DEBUG UTILITIES
    // ============================================
    
    pub fn disassemble_at(&mut self, addr: u16) -> (String, u8) {
        let opcode = self.read_byte(addr);  //self.memory[addr as usize];
        
        match opcode {
            0x00 => ("NOP".to_string(), 1),
            0x01 => {
                let word = self.read_word(addr.wrapping_add(1));
                (format!("LXI B,{:04X}h", word), 3)
            }
            0x06 => {
                let byte = self.read_byte(addr.wrapping_add(1));
                //let byte = self.memory[addr.wrapping_add(1) as usize];
                (format!("MVI B,{:02X}h", byte), 2)
            }
            0x21 => {
                let word = self.read_word(addr.wrapping_add(1));
                (format!("LXI H,{:04X}h", word), 3)
            }
            0x22 => {
                let word = self.read_word(addr.wrapping_add(1));
                (format!("SHLD {:04X}h", word), 3)
            }
            0x2A => {
                let word = self.read_word(addr.wrapping_add(1));
                (format!("LHLD {:04X}h", word), 3)
            }
            0x32 => {
                let word = self.read_word(addr.wrapping_add(1));
                (format!("STA {:04X}h", word), 3)
            }
            0x3A => {
                let word = self.read_word(addr.wrapping_add(1));
                (format!("LDA {:04X}h", word), 3)
            }
            0x3E => {
                let byte = self.read_byte(addr.wrapping_add(1));
                //let byte = self.memory[addr.wrapping_add(1) as usize];
                (format!("MVI A,{:02X}h", byte), 2)
            }
            0x76 => ("HLT".to_string(), 1),
            0x77 => ("MOV M,A".to_string(), 1),
            0x78 => ("MOV A,B".to_string(), 1),
            0x7E => ("MOV A,M".to_string(), 1),
            0x80 => ("ADD B".to_string(), 1),
            0xC1 => ("POP B".to_string(), 1),
            0xC3 => {
                let word = self.read_word(addr.wrapping_add(1));
                (format!("JMP {:04X}h", word), 3)
            }
            0xC5 => ("PUSH B".to_string(), 1),
            _ => (format!("DB {:02X}h", opcode), 1),
        }
    }
    
    pub fn trace(&mut self) {
        let (mnemonic, _) = self.disassemble_at(self.pc);
        println!("{:04X}: {:<12} | A={:02X} BC={:04X} DE={:04X} HL={:04X} SP={:04X} [{}{}{}{}{}]",
                 self.pc, mnemonic, self.a, 
                 self.get_bc(), self.get_de(), self.get_hl(), self.sp,
                 if self.flags & 0x80 != 0 { "S" } else { "-" },
                 if self.flags & 0x40 != 0 { "Z" } else { "-" },
                 if self.flags & 0x10 != 0 { "A" } else { "-" },
                 if self.flags & 0x04 != 0 { "P" } else { "-" },
                 if self.flags & 0x01 != 0 { "C" } else { "-" });    
        }
    
    pub fn debug_state(&mut self) {
        println!("\n========== CPU STATE ==========");
        
        // Main registers
        println!("A:{:02X}  B:{:02X}  C:{:02X}  D:{:02X}  E:{:02X}  H:{:02X}  L:{:02X}",
                 self.a, self.b, self.c, self.d, self.e, self.h, self.l);
        
        // Register pairs and pointers
        println!("BC:{:04X}  DE:{:04X}  HL:{:04X}  SP:{:04X}  PC:{:04X}",
                 self.get_bc(), self.get_de(), self.get_hl(), self.sp, self.pc);
        
        // Flags
        println!("FLAGS:{:02X} [{}{}{}{}{}]",
                 self.flags,
                 if self.flags & 0x80 != 0 { "S" } else { "-" },  // Sign
                 if self.flags & 0x40 != 0 { "Z" } else { "-" },  // Zero
                 if self.flags & 0x10 != 0 { "A" } else { "-" },  // Aux carry
                 if self.flags & 0x04 != 0 { "P" } else { "-" },  // Parity
                 if self.flags & 0x01 != 0 { "C" } else { "-" }); // Carry
        
        // Next instruction
        let (mnemonic, _size) = self.disassemble_at(self.pc);
        println!("\nNext: [{:04X}] {}", self.pc, mnemonic);
        
        // Memory dump around PC
        println!("\nMemory at PC:");
        for offset in (0..16).step_by(8) {
            let addr = self.pc.wrapping_add(offset);
            print!("  {:04X}: ", addr);
            for i in 0..8 {
                print!("{:02X} ", self.read_byte(addr.wrapping_add(i)));
                //print!("{:02X} ", self.memory[addr.wrapping_add(i) as usize]);
            }
            print!(" |");
            for i in 0..8 {
                let byte = self.read_byte(addr.wrapping_add(i));
                //let byte = self.memory[addr.wrapping_add(i) as usize];
                let ch = if byte >= 0x20 && byte <= 0x7E { byte as char } else { '.' };
                print!("{}", ch);
            }
            println!("|");
        }
        
        // Stack preview
        if self.sp < 0xFFFC && self.sp > 0 {
            println!("\nStack (top 3 words):");
            for i in 0..3 {
                let addr = self.sp.wrapping_add(i * 2);
                if addr < 0xFFFE {
                    let word = self.read_word(addr);
                    println!("  [{:04X}] = {:04X}", addr, word);
                }
            }
        }
        
        println!("==============================");
    }
    
    // ============================================
    // PUBLIC UTILITIES
    // ============================================
    
pub fn load_program(&mut self, program: &[u8], start_address: u16) {
    //println!("Loading {} bytes at 0x{:04X}: {:02X?}", 
    //         program.len(), start_address, program);
    for (i, &byte) in program.iter().enumerate() {
        self.write_byte(start_address +i as u16, byte);   
        //self.memory[start_address as usize + i] = byte;
    }
    self.pc = start_address;
    
    // Verify what actually got loaded
    //print!("Verify: ");
    for i in 0..program.len() {
        //print!("{:02X} ",self.read_byte(start_address + i as u16) );
        //print!("{:02X} ", self.memory[start_address as usize + i]);
    }
    //println!();
}
pub fn load_program_from_file(&mut self, path: &Path, start_address: u16) -> io::Result<usize> {
    let program = std::fs::read(path)?;
    self.load_program(&program, start_address);
    Ok(program.len())
}
}