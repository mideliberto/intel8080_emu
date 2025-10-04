#!/usr/bin/env rustc

// Standalone Intel 8080 Assembler
// Compile with: rustc asm8080.rs -o asm8080
// Or make executable: chmod +x asm8080.rs && ./asm8080.rs

use std::collections::HashMap;
use std::fs;
use std::path::Path;

// ============================================
// TYPES AND STRUCTURES
// ============================================

#[derive(Debug, Clone)]
pub struct Assembler {
    current_address: u16,
    symbols: HashMap<String, u16>,
    forward_refs: Vec<ForwardRef>,
    output: Vec<u8>,
    pass: u8,
    current_line: usize,
    current_line_text: String,
}

#[derive(Debug, Clone)]
struct ForwardRef {
    label: String,
    output_offset: usize,  // Offset in output buffer, not memory address
    size: u8,
}

#[derive(Debug)]
pub enum AsmError {
    InvalidMnemonic(String),
    InvalidOperand(String),
    InvalidRegister(String),
    InvalidNumber(String),
    UndefinedSymbol(String),
    DuplicateLabel(String),
    SyntaxError(String),
    IoError(String),
}

impl std::fmt::Display for AsmError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            AsmError::InvalidMnemonic(s) => write!(f, "Invalid mnemonic: {}", s),
            AsmError::InvalidOperand(s) => write!(f, "Invalid operand: {}", s),
            AsmError::InvalidRegister(s) => write!(f, "Invalid register: {}", s),
            AsmError::InvalidNumber(s) => write!(f, "Invalid number: '{}'", s),
            AsmError::UndefinedSymbol(s) => write!(f, "Undefined symbol: {}", s),
            AsmError::DuplicateLabel(s) => write!(f, "Duplicate label: {}", s),
            AsmError::SyntaxError(s) => write!(f, "Syntax error: {}", s),
            AsmError::IoError(s) => write!(f, "I/O error: {}", s),
        }
    }
}

type Result<T> = std::result::Result<T, AsmError>;

impl Assembler {
    pub fn new() -> Self {
        Assembler {
            current_address: 0,
            symbols: HashMap::new(),
            forward_refs: Vec::new(),
            output: Vec::new(),
            pass: 1,
            current_line: 0,
            current_line_text: String::new(),
        }
    }
    
    pub fn assemble_file(&mut self, input_path: &Path, output_path: &Path) -> Result<()> {
        let source = fs::read_to_string(input_path)
            .map_err(|e| AsmError::IoError(e.to_string()))?;
        
        let binary = self.assemble(&source)?;
        
        fs::write(output_path, binary)
            .map_err(|e| AsmError::IoError(e.to_string()))?;
        
        Ok(())
    }
    
pub fn assemble(&mut self, source: &str) -> Result<Vec<u8>> {
    // Pass 1: Build symbol table
    self.pass = 1;
    self.current_address = 0;
    self.current_line = 0;  // Initialize
    self.symbols.clear();
    self.output.clear();
    
    for line in source.lines() {
        self.current_line += 1;        // ← Add this
        self.current_line_text = line.to_string();  // ← Add this
        self.process_line(line)?;
    }
    
    // Pass 2: Generate code
    self.pass = 2;
    self.current_address = 0;
    self.current_line = 0;  // Reset
    self.output.clear();
    self.forward_refs.clear();
    
    for line in source.lines() {
        self.current_line += 1;        // ← Add this
        self.current_line_text = line.to_string();  // ← Add this
        self.process_line(line)?;
    }
    
    self.resolve_forward_refs()?;
    
    Ok(self.output.clone())
    }
    
    fn process_line(&mut self, line: &str) -> Result<()> {
        // Remove comments
        let line = if let Some(pos) = line.find(';') {
            &line[..pos]
        } else {
            line
        };
        
        let line = line.trim();
        if line.is_empty() {
            return Ok(());
        }
        
        // Check for EQU directive (special case: LABEL EQU VALUE)
        if line.contains(" EQU ") {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() >= 3 && parts[1] == "EQU" {
                let label = parts[0];
                let value_str = parts[2..].join(" ");
                let value = self.parse_number(&value_str)
                    .map_err(|e| self.add_line_context(e))?;
                
                if self.pass == 1 {
                    if self.symbols.contains_key(label) {
                        return Err(self.add_line_context(AsmError::DuplicateLabel(label.to_string())));
                    }
                    self.symbols.insert(label.to_string(), value);
                }
                return Ok(());
            }
        }
        
        // Check for label
        let (label, rest) = if let Some(pos) = line.find(':') {
            let label = line[..pos].trim();
            let rest = line[pos + 1..].trim();
            (Some(label), rest)
        } else {
            (None, line)
        };
        
        if let Some(label) = label {
            if self.pass == 1 {
                if self.symbols.contains_key(label) {
                    return Err(self.add_line_context(AsmError::DuplicateLabel(label.to_string())));
                }
                self.symbols.insert(label.to_string(), self.current_address);
            }
        }
        
        if !rest.is_empty() {
            self.process_statement(rest)
                .map_err(|e| self.add_line_context(e))?;
        }
        
        Ok(())
    }
    
    fn add_line_context(&self, err: AsmError) -> AsmError {
        AsmError::SyntaxError(format!(
            "Line {}: {}\n    {}", 
            self.current_line, 
            err, 
            self.current_line_text.trim()
        ))
    }
    
    fn process_statement(&mut self, stmt: &str) -> Result<()> {
        let parts: Vec<&str> = stmt.split_whitespace().collect();
        if parts.is_empty() {
            return Ok(());
        }
        
        let mnemonic = parts[0].to_uppercase();
        let operands = if parts.len() > 1 {
            parts[1..].join(" ")
        } else {
            String::new()
        };
        
        if self.process_directive(&mnemonic, &operands)? {
            return Ok(());
        }
        
        self.process_instruction(&mnemonic, &operands)
    }
    
    fn process_directive(&mut self, directive: &str, operands: &str) -> Result<bool> {
        match directive {
            "ORG" => {
                let addr = self.parse_number(operands)?;
                self.current_address = addr;
                Ok(true)
            }
             "END" => {
            // END directive - marks end of source
            // Operand (if present) specifies entry point but we ignore it
            Ok(true)
            }
            "DB" | "DEFB" => {
                for operand in operands.split(',') {
                    let operand = operand.trim();
                    
                    // Handle double-quoted strings: "ABC"
                    if operand.starts_with('"') && operand.ends_with('"') {
                        let s = &operand[1..operand.len() - 1];
                        for ch in s.chars() {
                            self.emit_byte(ch as u8);
                        }
                    }
                    // Handle single-quoted characters: 'A'
                    else if operand.starts_with('\'') && operand.ends_with('\'') {
                        let s = &operand[1..operand.len() - 1];
                        if s.len() != 1 {
                            return Err(AsmError::SyntaxError(
                                format!("Single quotes must contain exactly one character: {}", operand)
                            ));
                        }
                        self.emit_byte(s.chars().next().unwrap() as u8);
                    }
                    // Handle numeric expressions
                    else {
                        let value = self.parse_expression(operand)?;
                        self.emit_byte(value as u8);
                    }
                }
                Ok(true)
            }
            "DW" | "DEFW" => {
                for operand in operands.split(',') {
                    let value = self.parse_expression(operand.trim())?;
                    self.emit_word(value);
                }
                Ok(true)
            }
            "DS" | "DEFS" => {
                let count = self.parse_number(operands)?;
                for _ in 0..count {
                    self.emit_byte(0);
                }
                Ok(true)
            }
            _ => Ok(false),
        }
    }
    
    fn process_instruction(&mut self, mnemonic: &str, operands: &str) -> Result<()> {
        match mnemonic {
            // No operand instructions
            "NOP" => self.emit_byte(0x00),
            "HLT" => self.emit_byte(0x76),
            "RLC" => self.emit_byte(0x07),
            "RRC" => self.emit_byte(0x0F),
            "RAL" => self.emit_byte(0x17),
            "RAR" => self.emit_byte(0x1F),
            "DAA" => self.emit_byte(0x27),
            "CMA" => self.emit_byte(0x2F),
            "STC" => self.emit_byte(0x37),
            "CMC" => self.emit_byte(0x3F),
            "RET" => self.emit_byte(0xC9),
            "PCHL" => self.emit_byte(0xE9),
            "XCHG" => self.emit_byte(0xEB),
            "XTHL" => self.emit_byte(0xE3),
            "SPHL" => self.emit_byte(0xF9),
            "DI" => self.emit_byte(0xF3),
            "EI" => self.emit_byte(0xFB),
            
            // MOV instructions
            "MOV" => {
                let (dest, src) = self.parse_two_operands(operands)?;
                let dest_code = self.register_code(&dest)?;
                let src_code = self.register_code(&src)?;
                self.emit_byte(0x40 | (dest_code << 3) | src_code);
            }
            
            // MVI instructions
            "MVI" => {
                let parts: Vec<&str> = operands.split(',').map(|s| s.trim()).collect();
                if parts.len() != 2 {
                    return Err(AsmError::SyntaxError(format!("MVI expects 2 operands")));
                }
                let reg_code = self.register_code(parts[0])?;
                let imm = self.parse_expression(parts[1])?;
                self.emit_byte(0x06 | (reg_code << 3));
                self.emit_byte(imm as u8);
            }
            
            // LXI instructions
            "LXI" => {
                let parts: Vec<&str> = operands.split(',').map(|s| s.trim()).collect();
                if parts.len() != 2 {
                    return Err(AsmError::SyntaxError(format!("LXI expects 2 operands")));
                }
                let pair_code = self.register_pair_code(parts[0])?;
                let imm = self.parse_expression(parts[1])?;
                self.emit_byte(0x01 | (pair_code << 4));
                self.emit_word(imm);
            }
            
            // Arithmetic group
            "ADD" => self.emit_arithmetic(0x80, operands)?,
            "ADC" => self.emit_arithmetic(0x88, operands)?,
            "SUB" => self.emit_arithmetic(0x90, operands)?,
            "SBB" => self.emit_arithmetic(0x98, operands)?,
            "ANA" => self.emit_arithmetic(0xA0, operands)?,
            "XRA" => self.emit_arithmetic(0xA8, operands)?,
            "ORA" => self.emit_arithmetic(0xB0, operands)?,
            "CMP" => self.emit_arithmetic(0xB8, operands)?,
            
            // Immediate arithmetic
            "ADI" => self.emit_immediate(0xC6, operands)?,
            "ACI" => self.emit_immediate(0xCE, operands)?,
            "SUI" => self.emit_immediate(0xD6, operands)?,
            "SBI" => self.emit_immediate(0xDE, operands)?,
            "ANI" => self.emit_immediate(0xE6, operands)?,
            "XRI" => self.emit_immediate(0xEE, operands)?,
            "ORI" => self.emit_immediate(0xF6, operands)?,
            "CPI" => self.emit_immediate(0xFE, operands)?,
            
            // INR/DCR
            "INR" => {
                let reg_code = self.register_code(operands)?;
                self.emit_byte(0x04 | (reg_code << 3));
            }
            "DCR" => {
                let reg_code = self.register_code(operands)?;
                self.emit_byte(0x05 | (reg_code << 3));
            }
            
            // INX/DCX
            "INX" => {
                let pair_code = self.register_pair_code(operands)?;
                self.emit_byte(0x03 | (pair_code << 4));
            }
            "DCX" => {
                let pair_code = self.register_pair_code(operands)?;
                self.emit_byte(0x0B | (pair_code << 4));
            }
            
            // DAD
            "DAD" => {
                let pair_code = self.register_pair_code(operands)?;
                self.emit_byte(0x09 | (pair_code << 4));
            }
            
            // LDAX/STAX
            "LDAX" => {
                match operands.to_uppercase().as_str() {
                    "B" | "BC" => self.emit_byte(0x0A),
                    "D" | "DE" => self.emit_byte(0x1A),
                    _ => return Err(AsmError::InvalidOperand(operands.to_string())),
                }
            }
            "STAX" => {
                match operands.to_uppercase().as_str() {
                    "B" | "BC" => self.emit_byte(0x02),
                    "D" | "DE" => self.emit_byte(0x12),
                    _ => return Err(AsmError::InvalidOperand(operands.to_string())),
                }
            }
            
            // LDA/STA
            "LDA" => {
                self.emit_byte(0x3A);
                let addr = self.parse_expression(operands)?;
                self.emit_word(addr);
            }
            "STA" => {
                self.emit_byte(0x32);
                let addr = self.parse_expression(operands)?;
                self.emit_word(addr);
            }
            
            // LHLD/SHLD
            "LHLD" => {
                self.emit_byte(0x2A);
                let addr = self.parse_expression(operands)?;
                self.emit_word(addr);
            }
            "SHLD" => {
                self.emit_byte(0x22);
                let addr = self.parse_expression(operands)?;
                self.emit_word(addr);
            }
            
            // Jump instructions
            "JMP" => {
                self.emit_byte(0xC3);
                let addr = self.parse_expression(operands)?;
                self.emit_word(addr);
            }
            "JNZ" => self.emit_conditional_jump(0xC2, operands)?,
            "JZ" => self.emit_conditional_jump(0xCA, operands)?,
            "JNC" => self.emit_conditional_jump(0xD2, operands)?,
            "JC" => self.emit_conditional_jump(0xDA, operands)?,
            "JPO" => self.emit_conditional_jump(0xE2, operands)?,
            "JPE" => self.emit_conditional_jump(0xEA, operands)?,
            "JP" => self.emit_conditional_jump(0xF2, operands)?,
            "JM" => self.emit_conditional_jump(0xFA, operands)?,
            
            // Call instructions
            "CALL" => {
                self.emit_byte(0xCD);
                let addr = self.parse_expression(operands)?;
                self.emit_word(addr);
            }
            "CNZ" => self.emit_conditional_jump(0xC4, operands)?,
            "CZ" => self.emit_conditional_jump(0xCC, operands)?,
            "CNC" => self.emit_conditional_jump(0xD4, operands)?,
            "CC" => self.emit_conditional_jump(0xDC, operands)?,
            "CPO" => self.emit_conditional_jump(0xE4, operands)?,
            "CPE" => self.emit_conditional_jump(0xEC, operands)?,
            "CP" => self.emit_conditional_jump(0xF4, operands)?,
            "CM" => self.emit_conditional_jump(0xFC, operands)?,
            
            // Return instructions
            "RNZ" => self.emit_byte(0xC0),
            "RZ" => self.emit_byte(0xC8),
            "RNC" => self.emit_byte(0xD0),
            "RC" => self.emit_byte(0xD8),
            "RPO" => self.emit_byte(0xE0),
            "RPE" => self.emit_byte(0xE8),
            "RP" => self.emit_byte(0xF0),
            "RM" => self.emit_byte(0xF8),
            
            // RST instructions
            "RST" => {
                let n = self.parse_number(operands)?;
                if n > 7 {
                    return Err(AsmError::InvalidOperand(format!("RST {}", n)));
                }
                self.emit_byte(0xC7 | ((n as u8) << 3));
            }
            
            // PUSH/POP
            "PUSH" => {
                let code = self.push_pop_code(operands)?;
                self.emit_byte(0xC5 | (code << 4));
            }
            "POP" => {
                let code = self.push_pop_code(operands)?;
                self.emit_byte(0xC1 | (code << 4));
            }
            
            // I/O instructions
            "IN" => {
                self.emit_byte(0xDB);
                let port = self.parse_expression(operands)?;
                self.emit_byte(port as u8);
            }
            "OUT" => {
                self.emit_byte(0xD3);
                let port = self.parse_expression(operands)?;
                self.emit_byte(port as u8);
            }
            
            _ => return Err(AsmError::InvalidMnemonic(mnemonic.to_string())),
        }
        
        Ok(())
    }
    
    fn parse_two_operands(&self, operands: &str) -> Result<(String, String)> {
        let parts: Vec<&str> = operands.split(',').map(|s| s.trim()).collect();
        if parts.len() != 2 {
            return Err(AsmError::SyntaxError(format!("Expected 2 operands, got: {}", operands)));
        }
        Ok((parts[0].to_string(), parts[1].to_string()))
    }
    
    fn parse_expression(&mut self, expr: &str) -> Result<u16> {
        let expr = expr.trim();
        
        if expr == "$" {
            return Ok(self.current_address);
        }
        
        if expr.chars().next().map(|c| c.is_alphabetic() || c == '_').unwrap_or(false) {
            if let Some(&addr) = self.symbols.get(expr) {
                return Ok(addr);
            } else if self.pass == 2 {
                // Forward reference - record where in output buffer this needs to be fixed
                self.forward_refs.push(ForwardRef {
                    label: expr.to_string(),
                    output_offset: self.output.len(),  // Current position in output
                    size: 2,  // Assume 16-bit for now
                });
                return Ok(0);  // Placeholder
            } else {
                return Ok(0);  // Pass 1 - just return 0
            }
        }
        
        self.parse_number(expr)
    }
    
    fn parse_number(&self, s: &str) -> Result<u16> {
        let s = s.trim();
        
        // Hexadecimal with H suffix
        if s.ends_with('H') || s.ends_with('h') {
            let hex_str = &s[..s.len() - 1];
            // Handle case where hex starts with letter (e.g., FFH needs leading 0)
            u16::from_str_radix(hex_str, 16)
                .map_err(|_| AsmError::InvalidNumber(s.to_string()))
        } 
        // Binary with B suffix
        else if s.ends_with('B') || s.ends_with('b') {
            let bin_str = &s[..s.len() - 1];
            u16::from_str_radix(bin_str, 2)
                .map_err(|_| AsmError::InvalidNumber(s.to_string()))
        }
        // Octal with O or Q suffix
        else if s.ends_with('O') || s.ends_with('o') || s.ends_with('Q') || s.ends_with('q') {
            let oct_str = &s[..s.len() - 1];
            u16::from_str_radix(oct_str, 8)
                .map_err(|_| AsmError::InvalidNumber(s.to_string()))
        }
        // Hexadecimal with 0x prefix
        else if s.starts_with("0X") || s.starts_with("0x") {
            u16::from_str_radix(&s[2..], 16)
                .map_err(|_| AsmError::InvalidNumber(s.to_string()))
        } 
        // Decimal (default)
        else {
            s.parse::<u16>()
                .map_err(|_| AsmError::InvalidNumber(s.to_string()))
        }
    }
    
    fn register_code(&self, reg: &str) -> Result<u8> {
        match reg.to_uppercase().as_str() {
            "B" => Ok(0),
            "C" => Ok(1),
            "D" => Ok(2),
            "E" => Ok(3),
            "H" => Ok(4),
            "L" => Ok(5),
            "M" => Ok(6),
            "A" => Ok(7),
            _ => Err(AsmError::InvalidRegister(reg.to_string())),
        }
    }
    
    fn register_pair_code(&self, pair: &str) -> Result<u8> {
        match pair.to_uppercase().as_str() {
            "B" | "BC" => Ok(0),
            "D" | "DE" => Ok(1),
            "H" | "HL" => Ok(2),
            "SP" => Ok(3),
            _ => Err(AsmError::InvalidRegister(pair.to_string())),
        }
    }
    
    fn push_pop_code(&self, pair: &str) -> Result<u8> {
        match pair.to_uppercase().as_str() {
            "B" | "BC" => Ok(0),
            "D" | "DE" => Ok(1),
            "H" | "HL" => Ok(2),
            "PSW" | "AF" => Ok(3),
            _ => Err(AsmError::InvalidRegister(pair.to_string())),
        }
    }
    
    fn emit_byte(&mut self, byte: u8) {
        if self.pass == 2 {
            self.output.push(byte);
        }
        self.current_address += 1;
    }
    
    fn emit_word(&mut self, word: u16) {
        self.emit_byte((word & 0xFF) as u8);
        self.emit_byte((word >> 8) as u8);
    }
    
    fn emit_arithmetic(&mut self, base_opcode: u8, operand: &str) -> Result<()> {
        let reg_code = self.register_code(operand)?;
        self.emit_byte(base_opcode | reg_code);
        Ok(())
    }
    
    fn emit_immediate(&mut self, opcode: u8, operand: &str) -> Result<()> {
        self.emit_byte(opcode);
        let value = self.parse_expression(operand)?;
        self.emit_byte(value as u8);
        Ok(())
    }
    
    fn emit_conditional_jump(&mut self, opcode: u8, operand: &str) -> Result<()> {
        self.emit_byte(opcode);
        let addr = self.parse_expression(operand)?;
        self.emit_word(addr);
        Ok(())
    }
    
    fn resolve_forward_refs(&mut self) -> Result<()> {
        for fref in &self.forward_refs {
            if let Some(&target) = self.symbols.get(&fref.label) {
                let offset = fref.output_offset;
                if offset + (fref.size as usize) <= self.output.len() {
                    if fref.size == 2 {
                        // 16-bit address (little-endian)
                        self.output[offset] = (target & 0xFF) as u8;
                        self.output[offset + 1] = (target >> 8) as u8;
                    } else {
                        // 8-bit address
                        self.output[offset] = target as u8;
                    }
                }
            } else {
                return Err(AsmError::UndefinedSymbol(fref.label.clone()));
            }
        }
        Ok(())
    }
}

// ============================================
// MAIN FUNCTION
// ============================================

fn main() {
    let args: Vec<String> = std::env::args().collect();
    
    if args.len() < 2 {
        eprintln!("Intel 8080 Assembler");
        eprintln!("\nUsage: {} <input.asm> [output.bin]", args[0]);
        eprintln!("\nExamples:");
        eprintln!("  {} program.asm           # Creates program.bin", args[0]);
        eprintln!("  {} program.asm rom.bin   # Creates rom.bin", args[0]);
        std::process::exit(1);
    }
    
    let input_path = Path::new(&args[1]);
    let output_path_buf;
    let output_path = if args.len() > 2 {
        Path::new(&args[2])
    } else {
        output_path_buf = input_path.with_extension("bin");
        output_path_buf.as_path()
    };
    
    let mut assembler = Assembler::new();
    
    match assembler.assemble_file(input_path, output_path) {
        Ok(()) => {
            println!("✓ Assembly successful!");
            println!("  Output: {}", output_path.display());
            
            if !assembler.symbols.is_empty() {
                println!("\nSymbol table:");
                let mut symbols: Vec<_> = assembler.symbols.iter().collect();
                symbols.sort_by_key(|&(_, addr)| addr);
                for (label, addr) in symbols {
                    println!("  {:20} = ${:04X}", label, addr);
                }
            }
        }
        Err(e) => {
            eprintln!("✗ Assembly failed:");
            eprintln!("{}", e);
            std::process::exit(1);
        }
    }
}