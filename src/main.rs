// main.rs - Updated with 16-bit operations and endianness

// main.rs - Updated with pattern matching for instruction families

struct Intel8080 {
    // Registers
    a: u8,
    b: u8, 
    c: u8,
    d: u8,
    e: u8,
    h: u8,
    l: u8,
    flags: u8,
    sp: u16,
    pc: u16,
    
    // State
    memory: [u8; 0x10000],
    halted: bool,
    interrupts_enabled: bool,
}

// Flag bit positions
const FLAG_CARRY: u8     = 0x01;
const FLAG_PARITY: u8    = 0x04;
const FLAG_AUX_CARRY: u8 = 0x10;
const FLAG_ZERO: u8      = 0x40;
const FLAG_SIGN: u8      = 0x80;

impl Intel8080 {
        fn disassemble_at(&self, addr: u16) -> (String, u8) {
        let opcode = self.memory[addr as usize];
        
        match opcode {
            0x00 => ("NOP".to_string(), 1),
            0x01 => {
                let word = self.read_word(addr.wrapping_add(1));
                (format!("LXI B,{:04X}h", word), 3)
            }
            0x06 => {
                let byte = self.memory[addr.wrapping_add(1) as usize];
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
                let byte = self.memory[addr.wrapping_add(1) as usize];
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

        fn debug_state(&self) {
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
        let (mnemonic, size) = self.disassemble_at(self.pc);
        println!("\nNext: [{:04X}] {}", self.pc, mnemonic);
        
        // Memory dump around PC
        println!("\nMemory at PC:");
        for offset in (0..16).step_by(8) {
            let addr = self.pc.wrapping_add(offset);
            print!("  {:04X}: ", addr);
            for i in 0..8 {
                print!("{:02X} ", self.memory[addr.wrapping_add(i) as usize]);
            }
            print!(" |");
            for i in 0..8 {
                let byte = self.memory[addr.wrapping_add(i) as usize];
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
    
    // Simpler one-line trace
    fn trace(&self) {
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

    fn new() -> Self {
        Intel8080 {
            a: 0, b: 0, c: 0, d: 0, e: 0, h: 0, l: 0,
            flags: 0x02,  // Bit 1 is always set
            sp: 0xF000,
            pc: 0,
            memory: [0; 0x10000],
            halted: false,
            interrupts_enabled: false,
        }
    }
    
    // ===== REGISTER HELPERS =====
    
    fn get_reg(&self, code: u8) -> u8 {
        match code & 0x07 {
            0 => self.b,
            1 => self.c,
            2 => self.d,
            3 => self.e,
            4 => self.h,
            5 => self.l,
            6 => self.memory[self.get_hl() as usize],  // M = [HL]
            7 => self.a,
            _ => unreachable!(),
        }
    }
    
    fn set_reg(&mut self, code: u8, value: u8) {
        match code & 0x07 {
            0 => self.b = value,
            1 => self.c = value,
            2 => self.d = value,
            3 => self.e = value,
            4 => self.h = value,
            5 => self.l = value,
            6 => self.memory[self.get_hl() as usize] = value,  // M = [HL]
            7 => self.a = value,
            _ => unreachable!(),
        }
    }
    
    fn get_pair(&self, code: u8) -> u16 {
        match code & 0x03 {
            0 => self.get_bc(),
            1 => self.get_de(),
            2 => self.get_hl(),
            3 => self.sp,
            _ => unreachable!(),
        }
    }
    
    fn set_pair(&mut self, code: u8, value: u16) {
        match code & 0x03 {
            0 => self.set_bc(value),
            1 => self.set_de(value),
            2 => self.set_hl(value),
            3 => self.sp = value,
            _ => unreachable!(),
        }
    }
    
    fn get_push_pop_pair(&self, code: u8) -> u16 {
        match code & 0x03 {
            0 => self.get_bc(),
            1 => self.get_de(),
            2 => self.get_hl(),
            3 => self.get_psw(),  // PSW for PUSH/POP, not SP!
            _ => unreachable!(),
        }
    }
    
    fn set_push_pop_pair(&mut self, code: u8, value: u16) {
        match code & 0x03 {
            0 => self.set_bc(value),
            1 => self.set_de(value),
            2 => self.set_hl(value),
            3 => self.set_psw(value),  // PSW for PUSH/POP
            _ => unreachable!(),
        }
    }
    
    // 16-bit register helpers
    fn get_bc(&self) -> u16 { ((self.b as u16) << 8) | (self.c as u16) }
    fn set_bc(&mut self, val: u16) { self.b = (val >> 8) as u8; self.c = val as u8; }
    
    fn get_de(&self) -> u16 { ((self.d as u16) << 8) | (self.e as u16) }
    fn set_de(&mut self, val: u16) { self.d = (val >> 8) as u8; self.e = val as u8; }
    
    fn get_hl(&self) -> u16 { ((self.h as u16) << 8) | (self.l as u16) }
    fn set_hl(&mut self, val: u16) { self.h = (val >> 8) as u8; self.l = val as u8; }
    
    fn get_psw(&self) -> u16 { ((self.a as u16) << 8) | (self.flags as u16) }
    fn set_psw(&mut self, val: u16) { 
        self.a = (val >> 8) as u8; 
        self.flags = (val as u8) | 0x02;  // Keep bit 1 set
    }
    
    // ===== MEMORY HELPERS =====
    
    fn fetch_byte(&mut self) -> u8 {
        let byte = self.memory[self.pc as usize];
        self.pc = self.pc.wrapping_add(1);
        byte
    }
    
    fn fetch_word(&mut self) -> u16 {
        let low = self.fetch_byte() as u16;
        let high = self.fetch_byte() as u16;
        (high << 8) | low
    }
    
    fn read_word(&self, address: u16) -> u16 {
        let low = self.memory[address as usize] as u16;
        let high = self.memory[address.wrapping_add(1) as usize] as u16;
        (high << 8) | low
    }
    
    fn write_word(&mut self, address: u16, value: u16) {
        self.memory[address as usize] = value as u8;
        self.memory[address.wrapping_add(1) as usize] = (value >> 8) as u8;
    }
    
    // ===== FLAG HELPERS =====
    
    fn update_flags(&mut self, result: u8, carry: bool) {
        self.flags = 0x02;  // Start with bit 1 set
        
        if result == 0 { self.flags |= FLAG_ZERO; }
        if result & 0x80 != 0 { self.flags |= FLAG_SIGN; }
        if result.count_ones() % 2 == 0 { self.flags |= FLAG_PARITY; }
        if carry { self.flags |= FLAG_CARRY; }
    }
    
    fn test_condition(&self, condition: u8) -> bool {
        match condition & 0x07 {
            0 => (self.flags & FLAG_ZERO) == 0,     // NZ
            1 => (self.flags & FLAG_ZERO) != 0,     // Z
            2 => (self.flags & FLAG_CARRY) == 0,    // NC
            3 => (self.flags & FLAG_CARRY) != 0,    // C
            4 => (self.flags & FLAG_PARITY) == 0,   // PO
            5 => (self.flags & FLAG_PARITY) != 0,   // PE
            6 => (self.flags & FLAG_SIGN) == 0,     // P
            7 => (self.flags & FLAG_SIGN) != 0,     // M
            _ => unreachable!(),
        }
    }
    
    // ===== MAIN EXECUTION LOOP WITH PATTERN MATCHING =====
    
    fn run(&mut self) {
        while !self.halted {
            self.execute_one();
            println!("{:?}", self.disassemble_at(self.pc));
            //            self.trace();
//            self.debug_state();
        }
    }
    
    fn execute_one(&mut self) {
        let opcode = self.fetch_byte();
        
        match opcode {
            // ===== SPECIAL CASES FIRST =====
            0x00 => {},  // NOP
            0x76 => self.halted = true,  // HLT
            
            // ===== MOV FAMILY: 01DDDSSS (0x40-0x7F) =====
            0x40..=0x7F => {
                if opcode != 0x76 {  // HLT is special
                    let dest = (opcode >> 3) & 0x07;
                    let src = opcode & 0x07;
                    let value = self.get_reg(src);
                    self.set_reg(dest, value);
                }
            }
            
            // ===== ARITHMETIC FAMILY: 10AAASSS (0x80-0xBF) =====
            0x80..=0xBF => {
                let operation = (opcode >> 3) & 0x07;
                let src = opcode & 0x07;
                let value = self.get_reg(src);
                
                match operation {
                    0 => {  // ADD
                        let result = self.a as u16 + value as u16;
                        self.a = result as u8;
                        self.update_flags(self.a, result > 0xFF);
                    }
                    1 => {  // ADC (add with carry)
                        let carry = if self.flags & FLAG_CARRY != 0 { 1 } else { 0 };
                        let result = self.a as u16 + value as u16 + carry;
                        self.a = result as u8;
                        self.update_flags(self.a, result > 0xFF);
                    }
                    2 => {  // SUB
                        let result = (self.a as i16) - (value as i16);
                        self.a = result as u8;
                        self.update_flags(self.a, result < 0);
                    }
                    3 => {  // SBB (subtract with borrow)
                        let carry = if self.flags & FLAG_CARRY != 0 { 1 } else { 0 };
                        let result = (self.a as i16) - (value as i16) - carry;
                        self.a = result as u8;
                        self.update_flags(self.a, result < 0);
                    }
                    4 => {  // ANA (AND)
                        self.a &= value;
                        self.update_flags(self.a, false);
                    }
                    5 => {  // XRA (XOR)
                        self.a ^= value;
                        self.update_flags(self.a, false);
                    }
                    6 => {  // ORA (OR)
                        self.a |= value;
                        self.update_flags(self.a, false);
                    }
                    7 => {  // CMP (compare)
                        let result = (self.a as i16) - (value as i16);
                        self.update_flags(result as u8, result < 0);
                        // CMP doesn't change A, only flags
                    }
                    _ => unreachable!(),
                }
            }
            
            // ===== MVI FAMILY: 00RRR110 =====
            b if (b & 0xC7) == 0x06 => {
                let reg = (opcode >> 3) & 0x07;
                let data = self.fetch_byte();
                self.set_reg(reg, data);
            }
            
            // ===== INR FAMILY: 00RRR100 =====
            b if (b & 0xC7) == 0x04 => {
                let reg = (opcode >> 3) & 0x07;
                let value = self.get_reg(reg);
                let result = value.wrapping_add(1);
                self.set_reg(reg, result);
                
                // INR doesn't affect carry
                let carry = self.flags & FLAG_CARRY;
                self.update_flags(result, false);
                self.flags = (self.flags & !FLAG_CARRY) | carry;
            }
            
            // ===== DCR FAMILY: 00RRR101 =====
            b if (b & 0xC7) == 0x05 => {
                let reg = (opcode >> 3) & 0x07;
                let value = self.get_reg(reg);
                let result = value.wrapping_sub(1);
                self.set_reg(reg, result);
                
                // DCR doesn't affect carry
                let carry = self.flags & FLAG_CARRY;
                self.update_flags(result, false);
                self.flags = (self.flags & !FLAG_CARRY) | carry;
            }
            
            // ===== LXI FAMILY: 00RP0001 =====
            b if (b & 0xCF) == 0x01 => {
                let pair = (opcode >> 4) & 0x03;
                let data = self.fetch_word();
                self.set_pair(pair, data);
            }
            
            // ===== DAD FAMILY: 00RP1001 =====
            b if (b & 0xCF) == 0x09 => {
                let pair = (opcode >> 4) & 0x03;
                let hl = self.get_hl() as u32;
                let value = self.get_pair(pair) as u32;
                let result = hl + value;
                self.set_hl(result as u16);
                
                // DAD only affects carry
                if result > 0xFFFF {
                    self.flags |= FLAG_CARRY;
                } else {
                    self.flags &= !FLAG_CARRY;
                }
            }
            
            // ===== INX FAMILY: 00RP0011 =====
            b if (b & 0xCF) == 0x03 => {
                let pair = (opcode >> 4) & 0x03;
                let value = self.get_pair(pair).wrapping_add(1);
                self.set_pair(pair, value);
                // INX doesn't affect flags
            }
            
            // ===== DCX FAMILY: 00RP1011 =====
            b if (b & 0xCF) == 0x0B => {
                let pair = (opcode >> 4) & 0x03;
                let value = self.get_pair(pair).wrapping_sub(1);
                self.set_pair(pair, value);
                // DCX doesn't affect flags
            }
            
            // ===== PUSH FAMILY: 11RP0101 =====
            b if (b & 0xCF) == 0xC5 => {
                let pair = (opcode >> 4) & 0x03;
                let value = self.get_push_pop_pair(pair);
                self.sp = self.sp.wrapping_sub(2);
                self.write_word(self.sp, value);
            }
            
            // ===== POP FAMILY: 11RP0001 =====
            b if (b & 0xCF) == 0xC1 => {
                let pair = (opcode >> 4) & 0x03;
                let value = self.read_word(self.sp);
                self.set_push_pop_pair(pair, value);
                self.sp = self.sp.wrapping_add(2);
            }
            
            // ===== CONDITIONAL JUMPS: 11CCC010 =====
            b if (b & 0xC7) == 0xC2 => {
                let condition = (opcode >> 3) & 0x07;
                let addr = self.fetch_word();
                if self.test_condition(condition) {
                    self.pc = addr;
                }
            }
            
            // ===== CONDITIONAL CALLS: 11CCC100 =====
            b if (b & 0xC7) == 0xC4 => {
                let condition = (opcode >> 3) & 0x07;
                let addr = self.fetch_word();
                if self.test_condition(condition) {
                    self.sp = self.sp.wrapping_sub(2);
                    self.write_word(self.sp, self.pc);
                    self.pc = addr;
                }
            }
            
            // ===== CONDITIONAL RETURNS: 11CCC000 =====
            b if (b & 0xC7) == 0xC0 => {
                let condition = (opcode >> 3) & 0x07;
                if self.test_condition(condition) {
                    self.pc = self.read_word(self.sp);
                    self.sp = self.sp.wrapping_add(2);
                }
            }
            
            // ===== RST FAMILY: 11NNN111 =====
            b if (b & 0xC7) == 0xC7 => {
                let vector = (opcode >> 3) & 0x07;
                self.sp = self.sp.wrapping_sub(2);
                self.write_word(self.sp, self.pc);
                self.pc = (vector * 8) as u16;
            }
            
            // ===== SINGLE INSTRUCTIONS =====
            0xC3 => self.pc = self.fetch_word(),  // JMP
            0xCD => {  // CALL
                let addr = self.fetch_word();
                self.sp = self.sp.wrapping_sub(2);
                self.write_word(self.sp, self.pc);
                self.pc = addr;
            }
            0xC9 => {  // RET
                self.pc = self.read_word(self.sp);
                self.sp = self.sp.wrapping_add(2);
            }
            
            // STAX/LDAX
            0x02 => self.memory[self.get_bc() as usize] = self.a,  // STAX B
            0x12 => self.memory[self.get_de() as usize] = self.a,  // STAX D
            0x0A => self.a = self.memory[self.get_bc() as usize],  // LDAX B
            0x1A => self.a = self.memory[self.get_de() as usize],  // LDAX D
            
            // Direct memory operations
            0x32 => {  // STA
                let addr = self.fetch_word();
                self.memory[addr as usize] = self.a;
            }
            0x3A => {  // LDA
                let addr = self.fetch_word();
                self.a = self.memory[addr as usize];
            }
            0x22 => {  // SHLD
                let addr = self.fetch_word();
                self.write_word(addr, self.get_hl());
            }
            0x2A => {  // LHLD
                let addr = self.fetch_word();
                self.set_hl(self.read_word(addr));
            }
            
            // Immediate arithmetic
            0xC6 => {  // ADI
                let data = self.fetch_byte();
                let result = self.a as u16 + data as u16;
                self.a = result as u8;
                self.update_flags(self.a, result > 0xFF);
            }
            0xD6 => {  // SUI
                let data = self.fetch_byte();
                let result = (self.a as i16) - (data as i16);
                self.a = result as u8;
                self.update_flags(self.a, result < 0);
            }
            0xE6 => {  // ANI
                let data = self.fetch_byte();
                self.a &= data;
                self.update_flags(self.a, false);
            }
            0xEE => {  // XRI
                let data = self.fetch_byte();
                self.a ^= data;
                self.update_flags(self.a, false);
            }
            0xF6 => {  // ORI
                let data = self.fetch_byte();
                self.a |= data;
                self.update_flags(self.a, false);
            }
            0xFE => {  // CPI
                let data = self.fetch_byte();
                let result = (self.a as i16) - (data as i16);
                self.update_flags(result as u8, result < 0);
            }
            // Add these cases to your execute_one() function:

// ===== ROTATE INSTRUCTIONS =====
            0x07 => {  // RLC
                let high_bit = self.a >> 7;
                self.a = (self.a << 1) | high_bit;
                self.flags = (self.flags & !FLAG_CARRY) | high_bit;
            }
            0x0F => {  // RRC
                let low_bit = self.a & 0x01;
                self.a = (self.a >> 1) | (low_bit << 7);
                self.flags = (self.flags & !FLAG_CARRY) | low_bit;
            }
            0x17 => {  // RAL
                let carry = self.flags & FLAG_CARRY;
                self.flags = (self.flags & !FLAG_CARRY) | (self.a >> 7);
                self.a = (self.a << 1) | carry;
            }
            0x1F => {  // RAR
                let carry = (self.flags & FLAG_CARRY) << 7;
                self.flags = (self.flags & !FLAG_CARRY) | (self.a & 0x01);
                self.a = (self.a >> 1) | carry;
            }

            // ===== ACCUMULATOR OPERATIONS =====
            0x27 => {  // DAA - Decimal Adjust Accumulator
                let mut adjust = 0;
                let mut carry = self.flags & FLAG_CARRY != 0;
                
                if (self.a & 0x0F) > 9 || (self.flags & FLAG_AUX_CARRY != 0) {
                    adjust = 0x06;
                }
                
                if (self.a > 0x99) || carry {
                    adjust |= 0x60;
                    carry = true;
                }
                
                self.a = self.a.wrapping_add(adjust);
                self.update_flags(self.a, carry);
            }
            0x2F => {  // CMA - Complement Accumulator
                self.a = !self.a;
            }
            0x37 => {  // STC - Set Carry
                self.flags |= FLAG_CARRY;
            }
            0x3F => {  // CMC - Complement Carry
                self.flags ^= FLAG_CARRY;
            }

            // ===== IMMEDIATE WITH CARRY =====
            0xCE => {  // ACI - Add Immediate with Carry
                let data = self.fetch_byte();
                let carry = if self.flags & FLAG_CARRY != 0 { 1 } else { 0 };
                let result = self.a as u16 + data as u16 + carry;
                self.a = result as u8;
                self.update_flags(self.a, result > 0xFF);
            }
            0xDE => {  // SBI - Subtract Immediate with Borrow
                let data = self.fetch_byte();
                let carry = if self.flags & FLAG_CARRY != 0 { 1 } else { 0 };
                let result = (self.a as i16) - (data as i16) - carry;
                self.a = result as u8;
                self.update_flags(self.a, result < 0);
            }

            // ===== I/O INSTRUCTIONS =====
            0xD3 => {  // OUT
                let port = self.fetch_byte();
                // You'll need an I/O handler here
                // self.io_write(port, self.a);
            }
            0xDB => {  // IN
                let port = self.fetch_byte();
                // You'll need an I/O handler here
                // self.a = self.io_read(port);
            }

            // ===== EXCHANGE INSTRUCTIONS =====
            0xE3 => {  // XTHL - Exchange HL with (SP)
                let temp = self.read_word(self.sp);
                self.write_word(self.sp, self.get_hl());
                self.set_hl(temp);
            }
            0xE9 => {  // PCHL - PC = HL
                self.pc = self.get_hl();
            }
            0xEB => {  // XCHG - Exchange DE with HL
                let temp = self.get_de();
                self.set_de(self.get_hl());
                self.set_hl(temp);
            }

            // ===== STACK/INTERRUPT =====
            0xF3 => {  // DI - Disable Interrupts
                self.interrupts_enabled = false;
            }
            0xF9 => {  // SPHL - SP = HL
                self.sp = self.get_hl();
            }
            0xFB => {  // EI - Enable Interrupts
                self.interrupts_enabled = true;
            }

            // ===== UNDOCUMENTED NOPs =====
            0x08 | 0x10 | 0x18 | 0x20 | 0x28 | 0x30 | 0x38 => {
                // These are undocumented NOPs
            }
            _ => panic!("Unknown opcode: 0x{:02X} at PC: 0x{:04X}", 
                       opcode, self.pc.wrapping_sub(1)),
        }
    }
    
    fn load_program(&mut self, program: &[u8], start_address: u16) {
        for (i, &byte) in program.iter().enumerate() {
            self.memory[start_address as usize + i] = byte;
        }
        self.pc = start_address;
    }
}

fn main() {
    let mut cpu = Intel8080::new();
    
    // Test program showing various instruction families
    let program = [
        // Test MOV family
        0x06, 0x05,     // MVI B, 5
        0x0E, 0x03,     // MVI C, 3
        0x78,           // MOV A,B  (pattern: 01DDDSSS)
        
        // Test arithmetic family
        0x81,           // ADD C    (pattern: 10AAASSS)
        
        // Test INR/DCR family
        0x04,           // INR B    (pattern: 00RRR100)
        0x0D,           // DCR C    (pattern: 00RRR101)
        
        // Test register pair operations
        0x21, 0x00, 0x10,  // LXI H, 0x1000  (pattern: 00RP0001)
        0x09,              // DAD B          (pattern: 00RP1001)
        0x23,              // INX H          (pattern: 00RP0011)
        
        // Test conditional jump
        0xC2, 0x12, 0x00,  // JNZ 0x0012 (jump to HLT)        
        // Test PUSH/POP family
        0xC5,           // PUSH B  (pattern: 11RP0101)
        0xE1,           // POP H   (pattern: 11RP0001)
        
        0x76,           // HLT
    ];
    
    cpu.load_program(&program, 0x0000);
    cpu.run();
    
    println!("Program finished!");
    println!("A={:02X} B={:02X} C={:02X} H={:02X} L={:02X}", 
             cpu.a, cpu.b, cpu.c, cpu.h, cpu.l);
    println!("HL={:04X} SP={:04X} Flags={:02X}", 
             cpu.get_hl(), cpu.sp, cpu.flags);
}