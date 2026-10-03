// Flag bits of the 8080 PSW low byte. Bits 3 and 5 are always 0.
pub const FLAG_CARRY: u8     = 0x01;
pub const FLAG_BIT_1: u8     = 0x02;  // always 1
pub const FLAG_PARITY: u8    = 0x04;
pub const FLAG_AUX_CARRY: u8 = 0x10;
pub const FLAG_ZERO: u8      = 0x40;
pub const FLAG_SIGN: u8      = 0x80;

// Opcode fields. from_code ignores the bits above the field.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Condition { NZ, Z, NC, C, PO, PE, P, M }

impl Condition {
    pub fn from_code(code: u8) -> Self {
        [Self::NZ, Self::Z, Self::NC, Self::C, Self::PO, Self::PE, Self::P, Self::M][(code & 7) as usize]
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Register { B, C, D, E, H, L, M, A }

impl Register {
    pub fn from_code(code: u8) -> Self {
        [Self::B, Self::C, Self::D, Self::E, Self::H, Self::L, Self::M, Self::A][(code & 7) as usize]
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RegisterPair { BC, DE, HL, SP }

impl RegisterPair {
    pub fn from_code(code: u8) -> Self {
        [Self::BC, Self::DE, Self::HL, Self::SP][(code & 3) as usize]
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PushPopPair { BC, DE, HL, PSW }

impl PushPopPair {
    pub fn from_code(code: u8) -> Self {
        [Self::BC, Self::DE, Self::HL, Self::PSW][(code & 3) as usize]
    }
}
