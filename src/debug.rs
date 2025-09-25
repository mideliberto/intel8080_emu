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
