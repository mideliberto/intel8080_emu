//! Register definitions for the Intel 8080 processor.
//! 
//! This module defines the various register types used in the 8080:
//! - 8-bit registers (A, B, C, D, E, H, L, M)
//! - Register pairs for 16-bit operations (BC, DE, HL, SP)
//! - Special pairs for PUSH/POP (includes PSW)
//! - Condition codes for branching

// ============================================
// FLAG BIT DEFINITIONS
// ============================================

/// Flag bit positions in the 8080 flags register
pub const FLAG_CARRY: u8     = 0b00000001;  // Bit 0: Carry flag
pub const FLAG_BIT_1: u8     = 0b00000010;  // Bit 1: Always set to 1
pub const FLAG_PARITY: u8    = 0b00000100;  // Bit 2: Parity flag (even parity)
pub const FLAG_BIT_3: u8     = 0b00000000;  // Bit 3: Unused (always 0)
pub const FLAG_AUX_CARRY: u8 = 0b00010000;  // Bit 4: Auxiliary carry (half-carry)
pub const FLAG_BIT_5: u8     = 0b00000000;  // Bit 5: Unused (always 0)
pub const FLAG_ZERO: u8      = 0b01000000;  // Bit 6: Zero flag
pub const FLAG_SIGN: u8      = 0b10000000;  // Bit 7: Sign flag (negative)

// ============================================
// CONDITION CODES
// ============================================

/// Condition codes for conditional jumps, calls, and returns
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum Condition {
    NZ = 0,  // Not Zero (Z flag = 0)
    Z  = 1,  // Zero (Z flag = 1)
    NC = 2,  // No Carry (C flag = 0)
    C  = 3,  // Carry (C flag = 1)
    PO = 4,  // Parity Odd (P flag = 0)
    PE = 5,  // Parity Even (P flag = 1)
    P  = 6,  // Plus/Positive (S flag = 0)
    M  = 7,  // Minus/Negative (S flag = 1)
}

impl Condition {
    /// Decode condition from 3-bit field in instruction
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
    
    /// Encode condition to 3-bit value
    pub fn to_code(self) -> u8 {
        self as u8
    }
    
    /// Get assembly mnemonic (e.g., "NZ", "C")
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
    
    /// Get the flag bit this condition tests
    pub fn flag_mask(self) -> u8 {
        match self {
            Condition::NZ | Condition::Z  => FLAG_ZERO,
            Condition::NC | Condition::C  => FLAG_CARRY,
            Condition::PO | Condition::PE => FLAG_PARITY,
            Condition::P  | Condition::M  => FLAG_SIGN,
        }
    }
    
    /// Check if condition requires flag to be set (true) or clear (false)
    pub fn flag_value(self) -> bool {
        match self {
            Condition::Z | Condition::C | Condition::PE | Condition::M => true,
            Condition::NZ | Condition::NC | Condition::PO | Condition::P => false,
        }
    }
}

// ============================================
// 8-BIT REGISTERS
// ============================================

/// 8-bit register encoding as found in opcodes
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum Register {
    B = 0,   // General purpose
    C = 1,   // General purpose
    D = 2,   // General purpose
    E = 3,   // General purpose
    H = 4,   // High byte of HL
    L = 5,   // Low byte of HL
    M = 6,   // Memory location pointed to by HL
    A = 7,   // Accumulator
}

impl Register {
    /// Decode register from 3-bit field in instruction
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
    
    /// Encode register to 3-bit value
    pub fn to_code(self) -> u8 {
        self as u8
    }
    
    /// Get assembly mnemonic (e.g., "A", "B", "M")
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
    
    /// Check if this register refers to memory
    pub fn is_memory(self) -> bool {
        matches!(self, Register::M)
    }
    
    /// Extra cycles needed for memory operations
    pub fn cycle_modifier(self) -> u8 {
        if self.is_memory() { 3 } else { 0 }
    }
    
    /// Get human-readable description
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

// ============================================
// REGISTER PAIRS
// ============================================

/// 16-bit register pairs for data operations
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum RegisterPair {
    BC = 0,  // B (high), C (low)
    DE = 1,  // D (high), E (low)
    HL = 2,  // H (high), L (low)
    SP = 3,  // Stack pointer (note: becomes PSW for PUSH/POP)
}

impl RegisterPair {
    // ===== Conversion methods =====
    
    /// Decode register pair from 2-bit field in instruction
    pub fn from_code(code: u8) -> Self {
        match code & 0x03 {
            0 => RegisterPair::BC,
            1 => RegisterPair::DE,
            2 => RegisterPair::HL,
            3 => RegisterPair::SP,
            _ => unreachable!(),
        }
    }
    
    /// Encode register pair to 2-bit value
    pub fn to_code(self) -> u8 {
        self as u8
    }
    
    /// Decode PUSH/POP pair from instruction (SP becomes PSW)
    pub fn from_push_pop_code(code: u8) -> PushPopPair {
        match code & 0x03 {
            0 => PushPopPair::BC,
            1 => PushPopPair::DE,
            2 => PushPopPair::HL,
            3 => PushPopPair::PSW,  // Note: PSW not SP
            _ => unreachable!(),
        }
    }
    
    /// Convert to PUSH/POP encoding
    pub fn to_push_pop(self) -> PushPopPair {
        match self {
            RegisterPair::BC => PushPopPair::BC,
            RegisterPair::DE => PushPopPair::DE,
            RegisterPair::HL => PushPopPair::HL,
            RegisterPair::SP => PushPopPair::PSW,  // SP becomes PSW
        }
    }

    // ===== Display methods =====
    
    /// Get assembly mnemonic (e.g., "BC", "HL")
    pub fn name(self) -> &'static str {
        match self {
            RegisterPair::BC => "BC",
            RegisterPair::DE => "DE",
            RegisterPair::HL => "HL",
            RegisterPair::SP => "SP",
        }
    }
    
    /// Get human-readable description
    pub fn description(self) -> &'static str {
        match self {
            RegisterPair::BC => "BC register pair",
            RegisterPair::DE => "DE register pair",
            RegisterPair::HL => "HL register pair",
            RegisterPair::SP => "Stack Pointer",
        }
    }

    // ===== Component access =====
    
    /// Get the low byte register of this pair
    pub fn low_register(self) -> Option<Register> {
        match self {
            RegisterPair::BC => Some(Register::C),
            RegisterPair::DE => Some(Register::E),
            RegisterPair::HL => Some(Register::L),
            RegisterPair::SP => None,  // SP has no 8-bit components
        }
    }
    
    /// Get the high byte register of this pair
    pub fn high_register(self) -> Option<Register> {
        match self {
            RegisterPair::BC => Some(Register::B),
            RegisterPair::DE => Some(Register::D),
            RegisterPair::HL => Some(Register::H),
            RegisterPair::SP => None,  // SP has no 8-bit components
        }
    }

    // ===== Capability queries =====
    
    /// Check if this pair matches a PUSH/POP pair
    pub fn matches_push_pop(&self, pp: PushPopPair) -> bool {
        match (*self, pp) {
            (RegisterPair::BC, PushPopPair::BC) => true,
            (RegisterPair::DE, PushPopPair::DE) => true,
            (RegisterPair::HL, PushPopPair::HL) => true,
            _ => false,  // SP and PSW don't match
        }
    }
    
    /// Check if this pair supports indirect addressing (LDAX/STAX)
    pub fn supports_indirect(self) -> bool {
        matches!(self, RegisterPair::BC | RegisterPair::DE)
    }
}

// ============================================
// PUSH/POP REGISTER PAIRS
// ============================================

/// Register pairs for PUSH/POP operations (includes PSW)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum PushPopPair {
    BC = 0,   // BC pair
    DE = 1,   // DE pair
    HL = 2,   // HL pair
    PSW = 3,  // Program Status Word (A + Flags)
}

impl PushPopPair {
    /// Decode PUSH/POP pair from 2-bit field in instruction
    pub fn from_code(code: u8) -> Self {
        match code & 0x03 {
            0 => PushPopPair::BC,
            1 => PushPopPair::DE,
            2 => PushPopPair::HL,
            3 => PushPopPair::PSW,
            _ => unreachable!(),
        }
    }
    
    /// Encode PUSH/POP pair to 2-bit value
    pub fn to_code(self) -> u8 {
        self as u8
    }
    
    /// Get assembly mnemonic (e.g., "BC", "PSW")
    pub fn name(self) -> &'static str {
        match self {
            PushPopPair::BC => "BC",
            PushPopPair::DE => "DE",
            PushPopPair::HL => "HL",
            PushPopPair::PSW => "PSW",
        }
    }
    
    /// Get human-readable description
    pub fn description(self) -> &'static str {
        match self {
            PushPopPair::BC => "BC register pair",
            PushPopPair::DE => "DE register pair",
            PushPopPair::HL => "HL register pair",
            PushPopPair::PSW => "Program Status Word (A + Flags)",
        }
    }
    
    /// Convert to regular register pair (if not PSW)
    pub fn to_register_pair(self) -> Option<RegisterPair> {
        match self {
            PushPopPair::BC => Some(RegisterPair::BC),
            PushPopPair::DE => Some(RegisterPair::DE),
            PushPopPair::HL => Some(RegisterPair::HL),
            PushPopPair::PSW => None,  // PSW has no register pair equivalent
        }
    }
    
    /// Check if this is the PSW special case
    pub fn is_psw(self) -> bool {
        matches!(self, PushPopPair::PSW)
    }
}