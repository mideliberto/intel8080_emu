// Flag bit positions for the 8080
pub const FLAG_CARRY: u8     = 0b00000001;  // Bit 0
pub const FLAG_BIT_1: u8     = 0b00000010;  // Bit 1 (always 1)
pub const FLAG_PARITY: u8    = 0b00000100;  // Bit 2
pub const FLAG_BIT_3: u8     = 0b00000000;  // Bit 3 (always 0)
pub const FLAG_AUX_CARRY: u8 = 0b00010000;  // Bit 4
pub const FLAG_BIT_5: u8     = 0b00000000;  // Bit 5 (always 0)
pub const FLAG_ZERO: u8      = 0b01000000;  // Bit 6
pub const FLAG_SIGN: u8      = 0b10000000;  // Bit 7

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
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
    /// Convert from 3-bit condition field in opcode
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
    
    /// Get the 3-bit encoding
    pub fn to_code(self) -> u8 {
        self as u8
    }
    
    /// Get mnemonic for disassembly
    pub fn name(self) -> &'static str {
        match self {
            Condition::NZ => "NZ",
            Condition::Z  => "Z",
            Condition::NC => "NC",
            Condition::C  => "C",
            Condition::PO => "PO",
            Condition::PE => "PE",
            Condition::P  => "P",
            Condition::M  => "M",
        }
    }
    
    /// Get human-readable description
    pub fn description(self) -> &'static str {
        match self {
            Condition::NZ => "not zero",
            Condition::Z  => "zero",
            Condition::NC => "no carry",
            Condition::C  => "carry",
            Condition::PO => "parity odd",
            Condition::PE => "parity even",
            Condition::P  => "plus",
            Condition::M  => "minus",
        }
    }
    
    /// Which flag bit this condition tests
    pub fn flag_mask(self) -> u8 {
        match self {
            Condition::NZ | Condition::Z  => FLAG_ZERO,
            Condition::NC | Condition::C  => FLAG_CARRY,
            Condition::PO | Condition::PE => FLAG_PARITY,
            Condition::P  | Condition::M  => FLAG_SIGN,
        }
    }
    
    /// Should the flag be set (true) or clear (false) for this condition
    pub fn flag_value(self) -> bool {
        match self {
            Condition::Z | Condition::C | Condition::PE | Condition::M => true,
            Condition::NZ | Condition::NC | Condition::PO | Condition::P => false,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
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
    /// Check if this is a memory reference
    pub fn is_memory(self) -> bool {
        matches!(self, Register::M)
    }
    
    /// Get cycle count for operations with this register
    /// (Memory operations take longer)
    pub fn cycle_modifier(self) -> u8 {
        if self.is_memory() { 3 } else { 0 }  // Add 3 cycles for M
    }
    
    /// For debugging/verbose output
    pub fn description(self) -> &'static str {
        match self {
            Register::B => "B register",
            Register::C => "C register", 
            Register::D => "D register",
            Register::E => "E register",
            Register::H => "H register (high)",
            Register::L => "L register (low)",
            Register::M => "Memory [HL]",
            Register::A => "Accumulator",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum RegisterPair {
    BC = 0,
    DE = 1,
    HL = 2,
    SP = 3,  // Note: Sometimes this is PSW instead
}

impl RegisterPair {
    // Conversion methods

    pub fn from_code(code: u8) -> Self {
        match code & 0x03 {
            0 => RegisterPair::BC,
            1 => RegisterPair::DE,
            2 => RegisterPair::HL,
            3 => RegisterPair::SP,
            _ => unreachable!(),
        }
    }
    pub fn to_code(self) -> u8 {
        self as u8
    }
    pub fn from_push_pop_code(code: u8) -> PushPopPair {
        match code & 0x03 {
            0 => PushPopPair::BC,
            1 => PushPopPair::DE,
            2 => PushPopPair::HL,
            3 => PushPopPair::PSW,  // PSW instead of SP!
            _ => unreachable!(),
        }
    }
    pub fn to_push_pop(self) -> PushPopPair {
        match self {
            RegisterPair::BC => PushPopPair::BC,
            RegisterPair::DE => PushPopPair::DE,
            RegisterPair::HL => PushPopPair::HL,
            RegisterPair::SP => PushPopPair::PSW,  // This is the quirk!
        }
    }

    //Display Methods
    pub fn name(self) -> &'static str {
        match self {
            RegisterPair::BC => "BC",
            RegisterPair::DE => "DE",
            RegisterPair::HL => "HL",
            RegisterPair::SP => "SP",
        }
    }
    pub fn description(self) -> &'static str {
        match self {
            RegisterPair::BC => "BC register pair",
            RegisterPair::DE => "DE register pair",
            RegisterPair::HL => "HL register pair",
            RegisterPair::SP => "Stack Pointer",
        }
    }

    //Component Methods
    pub fn low_register(self) -> Option<Register> {
        match self {
            RegisterPair::BC => Some(Register::C),
            RegisterPair::DE => Some(Register::E),
            RegisterPair::HL => Some(Register::L),
            RegisterPair::SP => None,
        }
    }
    pub fn high_register(self) -> Option<Register> {
        match self {
            RegisterPair::BC => Some(Register::B),
            RegisterPair::DE => Some(Register::D),
            RegisterPair::HL => Some(Register::H),
            RegisterPair::SP => None,  // SP doesn't map to 8-bit registers
        }
    }

    // Capability queries
    pub fn matches_push_pop(&self, pp: PushPopPair) -> bool {
        match (*self, pp) {
            (RegisterPair::BC, PushPopPair::BC) => true,
            (RegisterPair::DE, PushPopPair::DE) => true,
            (RegisterPair::HL, PushPopPair::HL) => true,
            _ => false,
        }
    }
    pub fn supports_indirect(self) -> bool {
        matches!(self, RegisterPair::BC | RegisterPair::DE)
    }
}


#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
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
    pub fn from_code(code: u8) -> Self {
        match code & 0x03 {
            0 => PushPopPair::BC,
            1 => PushPopPair::DE,
            2 => PushPopPair::HL,
            3 => PushPopPair::PSW,
            _ => unreachable!(),
        }
    }
    
    /// Get the 2-bit encoding
    pub fn to_code(self) -> u8 {
        self as u8
    }
    
    pub fn description(self) -> &'static str {
        match self {
            PushPopPair::BC => "BC register pair",
            PushPopPair::DE => "DE register pair",
            PushPopPair::HL => "HL register pair",
            PushPopPair::PSW => "Program Status Word (A + Flags)",
        }
    }
    /// Convert to RegisterPair (if not PSW)
    pub fn to_register_pair(self) -> Option<RegisterPair> {
        match self {
            PushPopPair::BC => Some(RegisterPair::BC),
            PushPopPair::DE => Some(RegisterPair::DE),
            PushPopPair::HL => Some(RegisterPair::HL),
            PushPopPair::PSW => None,  // PSW doesn't map to a register pair
        }
    }
    
    /// Is this the PSW (special case)?
    pub fn is_psw(self) -> bool {
        matches!(self, PushPopPair::PSW)
    }
}