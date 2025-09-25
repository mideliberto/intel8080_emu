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
