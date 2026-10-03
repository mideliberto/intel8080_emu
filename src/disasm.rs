// disasm.rs - The 8080 opcode table, read forwards (disassembler, the DIS line) and
// backwards (the single-line assembler). Device code: mailbox ASM and DIS
// (DEVICE_SPECS 8) and the debugger (ARCHITECTURE 7.4) share it.

/// All 256 opcodes in the notation of docs/reference/Complete_Intel_8080_Instruction_Set_Reference.txt:
/// d8 and p8 are a byte operand, d16 and a16 a word operand. A star marks an
/// undocumented alias (ARCHITECTURE 5.4).
const OPCODES: [&str; 256] = [
    "NOP", "LXI B,d16", "STAX B", "INX B", "INR B", "DCR B", "MVI B,d8", "RLC", "NOP*", "DAD B", "LDAX B", "DCX B", "INR C", "DCR C", "MVI C,d8", "RRC", // 00
    "NOP*", "LXI D,d16", "STAX D", "INX D", "INR D", "DCR D", "MVI D,d8", "RAL", "NOP*", "DAD D", "LDAX D", "DCX D", "INR E", "DCR E", "MVI E,d8", "RAR", // 10
    "NOP*", "LXI H,d16", "SHLD a16", "INX H", "INR H", "DCR H", "MVI H,d8", "DAA", "NOP*", "DAD H", "LHLD a16", "DCX H", "INR L", "DCR L", "MVI L,d8", "CMA", // 20
    "NOP*", "LXI SP,d16", "STA a16", "INX SP", "INR M", "DCR M", "MVI M,d8", "STC", "NOP*", "DAD SP", "LDA a16", "DCX SP", "INR A", "DCR A", "MVI A,d8", "CMC", // 30
    "MOV B,B", "MOV B,C", "MOV B,D", "MOV B,E", "MOV B,H", "MOV B,L", "MOV B,M", "MOV B,A", "MOV C,B", "MOV C,C", "MOV C,D", "MOV C,E", "MOV C,H", "MOV C,L", "MOV C,M", "MOV C,A", // 40
    "MOV D,B", "MOV D,C", "MOV D,D", "MOV D,E", "MOV D,H", "MOV D,L", "MOV D,M", "MOV D,A", "MOV E,B", "MOV E,C", "MOV E,D", "MOV E,E", "MOV E,H", "MOV E,L", "MOV E,M", "MOV E,A", // 50
    "MOV H,B", "MOV H,C", "MOV H,D", "MOV H,E", "MOV H,H", "MOV H,L", "MOV H,M", "MOV H,A", "MOV L,B", "MOV L,C", "MOV L,D", "MOV L,E", "MOV L,H", "MOV L,L", "MOV L,M", "MOV L,A", // 60
    "MOV M,B", "MOV M,C", "MOV M,D", "MOV M,E", "MOV M,H", "MOV M,L", "HLT", "MOV M,A", "MOV A,B", "MOV A,C", "MOV A,D", "MOV A,E", "MOV A,H", "MOV A,L", "MOV A,M", "MOV A,A", // 70
    "ADD B", "ADD C", "ADD D", "ADD E", "ADD H", "ADD L", "ADD M", "ADD A", "ADC B", "ADC C", "ADC D", "ADC E", "ADC H", "ADC L", "ADC M", "ADC A", // 80
    "SUB B", "SUB C", "SUB D", "SUB E", "SUB H", "SUB L", "SUB M", "SUB A", "SBB B", "SBB C", "SBB D", "SBB E", "SBB H", "SBB L", "SBB M", "SBB A", // 90
    "ANA B", "ANA C", "ANA D", "ANA E", "ANA H", "ANA L", "ANA M", "ANA A", "XRA B", "XRA C", "XRA D", "XRA E", "XRA H", "XRA L", "XRA M", "XRA A", // A0
    "ORA B", "ORA C", "ORA D", "ORA E", "ORA H", "ORA L", "ORA M", "ORA A", "CMP B", "CMP C", "CMP D", "CMP E", "CMP H", "CMP L", "CMP M", "CMP A", // B0
    "RNZ", "POP B", "JNZ a16", "JMP a16", "CNZ a16", "PUSH B", "ADI d8", "RST 0", "RZ", "RET", "JZ a16", "JMP* a16", "CZ a16", "CALL a16", "ACI d8", "RST 1", // C0
    "RNC", "POP D", "JNC a16", "OUT p8", "CNC a16", "PUSH D", "SUI d8", "RST 2", "RC", "RET*", "JC a16", "IN p8", "CC a16", "CALL* a16", "SBI d8", "RST 3", // D0
    "RPO", "POP H", "JPO a16", "XTHL", "CPO a16", "PUSH H", "ANI d8", "RST 4", "RPE", "PCHL", "JPE a16", "XCHG", "CPE a16", "CALL* a16", "XRI d8", "RST 5", // E0
    "RP", "POP PSW", "JP a16", "DI", "CP a16", "PUSH PSW", "ORI d8", "RST 6", "RM", "SPHL", "JM a16", "EI", "CM a16", "CALL* a16", "CPI d8", "RST 7", // F0
];

/// The instruction at the start of `bytes` (bytes past its length are ignored):
/// its text and its length. An address operand (a16) prints as `name(word)` when
/// that is Some, else as 4 hex digits; immediate data (d16) always as 4 hex digits;
/// a byte operand as 2.
pub fn disassemble(bytes: [u8; 3], name: impl Fn(u16) -> Option<String>) -> (String, u16) {
    let t = OPCODES[bytes[0] as usize];
    let word = u16::from_le_bytes([bytes[1], bytes[2]]);
    if let Some(m) = t.strip_suffix("a16") {
        (format!("{}{}", m, name(word).unwrap_or_else(|| format!("{:04X}", word))), 3)
    } else if let Some(m) = t.strip_suffix("d16") {
        (format!("{}{:04X}", m, word), 3)
    } else if let Some(m) = t.strip_suffix("d8").or(t.strip_suffix("p8")) {
        (format!("{}{:02X}", m, bytes[1]), 2)
    } else {
        (t.to_string(), 1)
    }
}

/// The DIS instruction line (DEVICE_SPECS 8, DIS), without a line ending, and the
/// instruction length: `AAAA  B0 B1 B2  TEXT`, the bytes field padded to 8. `name` as
/// for `disassemble`: DIS passes `|_| None`, the debugger its symbol lookup.
pub fn line(addr: u16, bytes: [u8; 3], name: impl Fn(u16) -> Option<String>) -> (String, u16) {
    let (text, len) = disassemble(bytes, name);
    let hex: Vec<String> = bytes[..len as usize].iter().map(|b| format!("{:02X}", b)).collect();
    (format!("{:04X}  {:<8}  {}", addr, hex.join(" "), text), len)
}

/// One instruction in the notation `disassemble` prints, as machine code (DEVICE_SPECS 8,
/// ASM), or None if it does not assemble. The table is searched from opcode 00 and the
/// first matching entry wins, so a starred alias gives the lowest opcode of its group.
/// A token must equal the entry's text or be a hex number, so a line with a byte outside
/// 20-7E, an empty operand or an operand with a space in it matches no entry.
pub fn assemble(line: &str) -> Option<Vec<u8>> {
    let line = line.to_ascii_uppercase();
    let line = line.trim_matches(' ');
    let (mnemonic, rest) = line.split_once(' ').unwrap_or((line, ""));
    let rest = rest.trim_matches(' ');
    let operands: Vec<&str> = if rest.is_empty() { Vec::new() } else { rest.split(',').map(|o| o.trim_matches(' ')).collect() };
    (0..=255u8).find_map(|op| {
        let (m, entry) = OPCODES[op as usize].split_once(' ').unwrap_or((OPCODES[op as usize], ""));
        let entry: Vec<&str> = if entry.is_empty() { Vec::new() } else { entry.split(',').collect() };
        if m != mnemonic || entry.len() != operands.len() {
            return None;
        }
        let mut code = vec![op];
        for (&e, &o) in entry.iter().zip(&operands) {
            match e {
                "d8" | "p8" => code.push(u8::try_from(number(o)?).ok()?),
                "d16" | "a16" => code.extend(number(o)?.to_le_bytes()),
                _ if e == o => {}
                _ => return None,
            }
        }
        Some(code)
    })
}

/// 1-4 hex digits and nothing else (no sign, prefix or suffix).
fn number(tok: &str) -> Option<u16> {
    if tok.len() > 4 || !tok.bytes().all(|b| b.is_ascii_hexdigit()) {
        return None;
    }
    u16::from_str_radix(tok, 16).ok() // None when empty
}
