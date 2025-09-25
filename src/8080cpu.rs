// Complete Intel 8080 with Enum Pattern for Type Safety

// ============================================
// ENUM DEFINITIONS (Layer 2 from earlier)
// ============================================

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Register {
    B = 0,
    C = 1,
    D = 2,
    E = 3,
    H = 4,
    L = 5,
    M = 6,  // Memory at HL
    A = 7,
}

impl Register {
    /// Convert from 3-bit opcode field to Register enum
    pub fn from_code(code: u8) -> Self {
        match code & 0x07 {
            0 => Register::B,
            1 => Register::C,
            2 => Register::D,
            3 => Register::E,
            4 => Register::H,
            5 => Register::L,
            6 => Register::M,
            7 => Register::A,
            _ => unreachable!(),
        }
    }
    
    /// Get the 3-bit encoding for this register
    pub fn to_code(self) -> u8 {
        self as u8
    }
    
    /// Get register name for disassembly
    pub fn name(self) -> &'static str {
        match self {
            Register::B => "B",
            Register::C => "C",
            Register::D => "D",
            Register::E => "E",
            Register::H => "H",
            Register::L => "L",
            Register::M => "M",
            Register::A => "A",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum RegisterPair {
    BC = 0,
    DE = 1,
    HL = 2,
    SP = 3,  // Note: Sometimes this is PSW instead
}

impl RegisterPair {
    /// Convert from 2-bit opcode field to RegisterPair
    pub fn from_code(code: u8) -> Self {
        match code & 0x03 {
            0 => RegisterPair::BC,
            1 => RegisterPair::DE,
            2 => RegisterPair::HL,
            3 => RegisterPair::SP,
            _ => unreachable!(),
        }
    }
    
    /// For PUSH/POP instructions, encoding is different
    pub fn from_push_pop_code(code: u8) -> PushPopPair {
        match code & 0x03 {
            0 => PushPopPair::BC,
            1 => PushPopPair::DE,
            2 => PushPopPair::HL,
            3 => PushPopPair::PSW,  // PSW instead of SP!
            _ => unreachable!(),
        }
    }
    
    pub fn to_code(self) -> u8 {
        self as u8
    }
    
    pub fn name(self) -> &'static str {
        match self {
            RegisterPair::BC => "BC",
            RegisterPair::DE => "DE",
            RegisterPair::HL => "HL",
            RegisterPair::SP => "SP",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PushPopPair {
    BC = 0,
    DE = 1,
    HL = 2,
    PSW = 3,  // AF for PUSH/POP
}

impl PushPopPair {
    pub fn name(self) -> &'static str {
        match self {
            PushPopPair::BC => "BC",
            PushPopPair::DE => "DE",
            PushPopPair::HL => "HL",
            PushPopPair::PSW => "PSW",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Condition {
    NZ = 0,  // Not Zero
    Z  = 1,  // Zero
    NC = 2,  // No Carry
    C  = 3,  // Carry
    PO = 4,  // Parity Odd
    PE = 5,  // Parity Even
    P  = 6,  // Plus (positive)
    M  = 7,  // Minus (negative)
}

impl Condition {
    pub fn from_code(code: u8) -> Self {
        match code & 0x07 {
            0 => Condition::NZ,
            1 => Condition::Z,
            2 => Condition::NC,
            3 => Condition::C,
            4 => Condition::PO,
            5 => Condition::PE,
            6 => Condition::P,
            7 => Condition::M,
            _ => unreachable!(),
        }
    }
    
    pub fn name(self) -> &'static str {
        match self {
            Condition::NZ => "NZ",
            Condition::Z => "Z",
            Condition::NC => "NC",
            Condition::C => "C",
            Condition::PO => "PO",
            Condition::PE => "PE",
            Condition::P => "P",
            Condition::M => "M",
        }
    }
}

// ============================================
// CPU STRUCTURE
// ============================================

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
    pub memory: Box<[u8; 0x10000]>,
    pub halted: bool,
    pub interrupts_enabled: bool,
    pub cycles: u64,
    
    // Combined approach
    instructions: [Option<Instruction>; 256],
}

#[derive(Clone)]
struct Instruction {
    mnemonic: &'static str,
    size: u8,
    base_cycles: u8,
    handler: OpHandler,
}

type OpHandler = fn(&mut Intel8080) -> u8;

// Flag constants
const FLAG_CARRY: u8     = 0b00000001;
const FLAG_BIT_1: u8     = 0b00000010;
const FLAG_PARITY: u8    = 0b00000100;
const FLAG_AUX_CARRY: u8 = 0b00010000;
const FLAG_ZERO: u8      = 0b01000000;
const FLAG_SIGN: u8      = 0b10000000;

impl Intel8080 {
    pub fn new() -> Self {
        let mut cpu = Intel8080 {
            a: 0, b: 0, c: 0, d: 0, e: 0, h: 0, l: 0,
            flags: FLAG_BIT_1,
            sp: 0, pc: 0,
            memory: Box::new([0; 0x10000]),
            halted: false,
            interrupts_enabled: false,
            cycles: 0,
            instructions: [None; 256],
        };
        
        cpu.init_instructions();
        cpu
    }
    
    // ============================================
    // LAYER 1: Direct register access
    // ============================================
    
    #[inline]
    pub fn get_a(&self) -> u8 { self.a }
    #[inline]
    pub fn get_b(&self) -> u8 { self.b }
    #[inline]
    pub fn get_c(&self) -> u8 { self.c }
    #[inline]
    pub fn get_d(&self) -> u8 { self.d }
    #[inline]
    pub fn get_e(&self) -> u8 { self.e }
    #[inline]
    pub fn get_h(&self) -> u8 { self.h }
    #[inline]
    pub fn get_l(&self) -> u8 { self.l }
    
    #[inline]
    pub fn set_a(&mut self, val: u8) { self.a = val; }
    #[inline]
    pub fn set_b(&mut self, val: u8) { self.b = val; }
    #[inline]
    pub fn set_c(&mut self, val: u8) { self.c = val; }
    #[inline]
    pub fn set_d(&mut self, val: u8) { self.d = val; }
    #[inline]
    pub fn set_e(&mut self, val: u8) { self.e = val; }
    #[inline]
    pub fn set_h(&mut self, val: u8) { self.h = val; }
    #[inline]
    pub fn set_l(&mut self, val: u8) { self.l = val; }
    
    #[inline]
    pub fn get_bc(&self) -> u16 {
        ((self.b as u16) << 8) | (self.c as u16)
    }
    
    #[inline]
    pub fn set_bc(&mut self, val: u16) {
        self.b = (val >> 8) as u8;
        self.c = val as u8;
    }
    
    #[inline]
    pub fn get_de(&self) -> u16 {
        ((self.d as u16) << 8) | (self.e as u16)
    }
    
    #[inline]
    pub fn set_de(&mut self, val: u16) {
        self.d = (val >> 8) as u8;
        self.e = val as u8;
    }
    
    #[inline]
    pub fn get_hl(&self) -> u16 {
        ((self.h as u16) << 8) | (self.l as u16)
    }
    
    #[inline]
    pub fn set_hl(&mut self, val: u16) {
        self.h = (val >> 8) as u8;
        self.l = val as u8;
    }
    
    #[inline]
    pub fn get_psw(&self) -> u16 {
        ((self.a as u16) << 8) | (self.flags as u16)
    }
    
    #[inline]
    pub fn set_psw(&mut self, val: u16) {
        self.a = (val >> 8) as u8;
        self.flags = (val as u8) | FLAG_BIT_1;
    }
    
    // ============================================
    // LAYER 2: Enum-based access (Type Safe!)
    // ============================================
    
    /// Get register value using enum (type safe)
    #[inline]
    pub fn get_reg(&self, reg: Register) -> u8 {
        match reg {
            Register::A => self.get_a(),
            Register::B => self.get_b(),
            Register::C => self.get_c(),
            Register::D => self.get_d(),
            Register::E => self.get_e(),
            Register::H => self.get_h(),
            Register::L => self.get_l(),
            Register::M => {
                let addr = self.get_hl();
                self.memory[addr as usize]
            }
        }
    }
    
    /// Set register value using enum (type safe)
    #[inline]
    pub fn set_reg(&mut self, reg: Register, val: u8) {
        match reg {
            Register::A => self.set_a(val),
            Register::B => self.set_b(val),
            Register::C => self.set_c(val),
            Register::D => self.set_d(val),
            Register::E => self.set_e(val),
            Register::H => self.set_h(val),
            Register::L => self.set_l(val),
            Register::M => {
                let addr = self.get_hl();
                self.memory[addr as usize] = val;
            }
        }
    }
    
    /// Get register pair value using enum
    #[inline]
    pub fn get_pair(&self, pair: RegisterPair) -> u16 {
        match pair {
            RegisterPair::BC => self.get_bc(),
            RegisterPair::DE => self.get_de(),
            RegisterPair::HL => self.get_hl(),
            RegisterPair::SP => self.sp,
        }
    }
    
    /// Set register pair value using enum
    #[inline]
    pub fn set_pair(&mut self, pair: RegisterPair, val: u16) {
        match pair {
            RegisterPair::BC => self.set_bc(val),
            RegisterPair::DE => self.set_de(val),
            RegisterPair::HL => self.set_hl(val),
            RegisterPair::SP => self.sp = val,
        }
    }
    
    /// Get register pair for PUSH/POP (different encoding!)
    #[inline]
    pub fn get_push_pop_pair(&self, pair: PushPopPair) -> u16 {
        match pair {
            PushPopPair::BC => self.get_bc(),
            PushPopPair::DE => self.get_de(),
            PushPopPair::HL => self.get_hl(),
            PushPopPair::PSW => self.get_psw(),
        }
    }
    
    /// Set register pair for PUSH/POP
    #[inline]
    pub fn set_push_pop_pair(&mut self, pair: PushPopPair, val: u16) {
        match pair {
            PushPopPair::BC => self.set_bc(val),
            PushPopPair::DE => self.set_de(val),
            PushPopPair::HL => self.set_hl(val),
            PushPopPair::PSW => self.set_psw(val),
        }
    }
    
    /// Test condition code
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
    
    // ============================================
    // LAYER 2.5: Backward compatibility with codes
    // ============================================
    
    /// Get register by code (for instruction decoding)
    #[inline]
    pub fn get_reg_by_code(&self, code: u8) -> u8 {
        self.get_reg(Register::from_code(code))
    }
    
    /// Set register by code (for instruction decoding)
    #[inline]
    pub fn set_reg_by_code(&mut self, code: u8, val: u8) {
        self.set_reg(Register::from_code(code), val)
    }
    
    /// Get pair by code
    #[inline]
    pub fn get_pair_by_code(&self, code: u8) -> u16 {
        self.get_pair(RegisterPair::from_code(code))
    }
    
    /// Set pair by code
    #[inline]
    pub fn set_pair_by_code(&mut self, code: u8, val: u16) {
        self.set_pair(RegisterPair::from_code(code), val)
    }
    
    // ============================================
    // INITIALIZE INSTRUCTIONS WITH ENUMS
    // ============================================
    
    fn init_instructions(&mut self) {
        // Initialize MOV instructions using enums
        for dest in 0..8u8 {
            for src in 0..8u8 {
                let opcode = 0x40 | (dest << 3) | src;
                
                if opcode == 0x76 {
                    self.instructions[0x76] = Some(Instruction {
                        mnemonic: "HLT",
                        size: 1,
                        base_cycles: 7,
                        handler: |cpu| { cpu.halted = true; 7 },
                    });
                } else {
                    let dest_reg = Register::from_code(dest);
                    let src_reg = Register::from_code(src);
                    let mnemonic = Box::leak(
                        format!("MOV {},{}", dest_reg.name(), src_reg.name()).into_boxed_str()
                    );
                    
                    self.instructions[opcode as usize] = Some(Instruction {
                        mnemonic,
                        size: 1,
                        base_cycles: if dest == 6 || src == 6 { 7 } else { 5 },
                        handler: Self::create_mov_handler(dest_reg, src_reg),
                    });
                }
            }
        }
        
        // LXI instructions with RegisterPair enum
        self.instructions[0x01] = Some(Instruction {
            mnemonic: "LXI B",
            size: 3,
            base_cycles: 10,
            handler: |cpu| cpu.lxi(RegisterPair::BC),
        });
        
        self.instructions[0x11] = Some(Instruction {
            mnemonic: "LXI D",
            size: 3,
            base_cycles: 10,
            handler: |cpu| cpu.lxi(RegisterPair::DE),
        });
        
        self.instructions[0x21] = Some(Instruction {
            mnemonic: "LXI H",
            size: 3,
            base_cycles: 10,
            handler: |cpu| cpu.lxi(RegisterPair::HL),
        });
        
        self.instructions[0x31] = Some(Instruction {
            mnemonic: "LXI SP",
            size: 3,
            base_cycles: 10,
            handler: |cpu| cpu.lxi(RegisterPair::SP),
        });
        
        // Conditional jumps with Condition enum
        self.instructions[0xC2] = Some(Instruction {
            mnemonic: "JNZ",
            size: 3,
            base_cycles: 10,
            handler: |cpu| cpu.jcc(Condition::NZ),
        });
        
        self.instructions[0xCA] = Some(Instruction {
            mnemonic: "JZ",
            size: 3,
            base_cycles: 10,
            handler: |cpu| cpu.jcc(Condition::Z),
        });
        
        // ... continue for other conditions
    }
    
    // ============================================
    // HANDLER CREATORS WITH ENUMS
    // ============================================
    
    fn create_mov_handler(dest: Register, src: Register) -> OpHandler {
        // Closure captures the enum values, not raw codes!
        move |cpu| {
            let val = cpu.get_reg(src);
            cpu.set_reg(dest, val);
            
            // Check for M register (memory access is slower)
            if dest == Register::M || src == Register::M { 
                7 
            } else { 
                5 
            }
        }
    }
    
    fn create_add_handler(src: Register) -> OpHandler {
        move |cpu| {
            let val = cpu.get_reg(src);
            let result = cpu.a as u16 + val as u16;
            let aux_carry = (cpu.a & 0x0F) + (val & 0x0F) > 0x0F;
            
            cpu.a = result as u8;
            cpu.update_all_flags(cpu.a, result > 0xFF, aux_carry);
            
            if src == Register::M { 7 } else { 4 }
        }
    }
    
    // ============================================
    // INSTRUCTION IMPLEMENTATIONS WITH ENUMS
    // ============================================
    
    /// MOV - Move register to register (type safe!)
    pub fn mov(&mut self, dest: Register, src: Register) -> u8 {
        let val = self.get_reg(src);
        self.set_reg(dest, val);
        
        if dest == Register::M || src == Register::M { 7 } else { 5 }
    }
    
    /// LXI - Load register pair immediate
    pub fn lxi(&mut self, pair: RegisterPair) -> u8 {
        let val = self.fetch_word();
        self.set_pair(pair, val);
        10
    }
    
    /// DAD - Double add to HL
    pub fn dad(&mut self, pair: RegisterPair) -> u8 {
        let hl = self.get_hl() as u32;
        let addend = self.get_pair(pair) as u32;
        let result = hl + addend;
        
        self.set_hl(result as u16);
        
        if result > 0xFFFF {
            self.flags |= FLAG_CARRY;
        } else {
            self.flags &= !FLAG_CARRY;
        }
        
        10
    }
    
    /// Conditional jump
    pub fn jcc(&mut self, cond: Condition) -> u8 {
        let addr = self.fetch_word();
        
        if self.test_condition(cond) {
            self.pc = addr;
        }
        
        10
    }
    
    /// PUSH - Push register pair
    pub fn push(&mut self, pair: PushPopPair) -> u8 {
        let val = self.get_push_pop_pair(pair);
        self.sp = self.sp.wrapping_sub(2);
        self.memory[self.sp as usize] = val as u8;
        self.memory[(self.sp + 1) as usize] = (val >> 8) as u8;
        11
    }
    
    /// POP - Pop register pair
    pub fn pop(&mut self, pair: PushPopPair) -> u8 {
        let low = self.memory[self.sp as usize] as u16;
        let high = self.memory[(self.sp + 1) as usize] as u16;
        let val = (high << 8) | low;
        self.set_push_pop_pair(pair, val);
        self.sp = self.sp.wrapping_add(2);
        10
    }
    
    // ============================================
    // HELPER FUNCTIONS
    // ============================================
    
    #[inline]
    fn fetch_byte(&mut self) -> u8 {
        let byte = self.memory[self.pc as usize];
        self.pc = self.pc.wrapping_add(1);
        byte
    }
    
    #[inline]
    fn fetch_word(&mut self) -> u16 {
        let low = self.fetch_byte() as u16;
        let high = self.fetch_byte() as u16;
        (high << 8) | low
    }
    
    fn update_all_flags(&mut self, result: u8, carry: bool, aux_carry: bool) {
        self.flags = FLAG_BIT_1;
        
        if result == 0 { self.flags |= FLAG_ZERO; }
        if result & 0x80 != 0 { self.flags |= FLAG_SIGN; }
        if result.count_ones() % 2 == 0 { self.flags |= FLAG_PARITY; }
        if carry { self.flags |= FLAG_CARRY; }
        if aux_carry { self.flags |= FLAG_AUX_CARRY; }
    }
}

// ============================================
// USAGE EXAMPLES
// ============================================

fn main() {
    let mut cpu = Intel8080::new();
    
    // BENEFIT 1: Type-safe register access
    cpu.set_reg(Register::B, 0x42);
    cpu.set_reg(Register::C, 0x10);
    let b_val = cpu.get_reg(Register::B);
    
    // BENEFIT 2: Clearer code
    cpu.mov(Register::A, Register::B);  // Much clearer than mov(7, 0)!
    
    // BENEFIT 3: Can't mix up register codes
    cpu.dad(RegisterPair::BC);  // Can't accidentally pass wrong value
    
    // BENEFIT 4: IDE autocomplete works!
    cpu.push(PushPopPair::PSW);  // IDE shows only valid options
    
    // BENEFIT 5: Pattern matching
    match Register::from_code(6) {
        Register::M => println!("It's a memory operation!"),
        Register::A => println!("Accumulator operation"),
        _ => println!("Regular register"),
    }
    
    // You can still use codes when decoding instructions:
    let opcode = 0x41;  // MOV B,C
    let dest_code = (opcode >> 3) & 0x07;
    let src_code = opcode & 0x07;
    
    // But convert to enums for type safety:
    let dest = Register::from_code(dest_code);
    let src = Register::from_code(src_code);
    cpu.mov(dest, src);
    
    println!("MOV {},{} executed!", dest.name(), src.name());
}

/*
BENEFITS OF THE ENUM PATTERN:

1. TYPE SAFETY
   - Can't pass invalid register codes
   - Compiler catches errors

2. READABILITY
   - mov(Register::A, Register::B) vs mov(7, 0)
   - Self-documenting code

3. IDE SUPPORT
   - Autocomplete shows valid options
   - Jump to definition works

4. PATTERN MATCHING
   - Can match on specific registers
   - Exhaustive checking

5. REFACTORING
   - Change register encoding in one place
   - Compiler ensures all uses are updated

IS IT NEEDED?
- Not strictly required, but highly recommended!
- Makes code much more maintainable
- Prevents subtle bugs
- Professional emulators use this pattern
*/