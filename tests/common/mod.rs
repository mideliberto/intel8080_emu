use intel8080_emu::intel8080cpu::Intel8080;
use intel8080_emu::registers::*;

fn setup_cpu(program: &[u8]) -> Intel8080 {
    let mut cpu = Intel8080::new();
    cpu.load_program(program, 0);
    cpu
}

fn run_until_halt(cpu: &mut Intel8080) {
    let mut count = 0;
    const MAX_INSTRUCTIONS: usize = 1000;
    
    while !cpu.halted && count < MAX_INSTRUCTIONS {
        cpu.execute_one();
        count += 1;
    }
    
    // FIXED: Fail explicitly with debug info instead of just asserting
    if !cpu.halted {
        panic!(
            "Program didn't halt within {} instructions.\n\
             PC=0x{:04X}, A=0x{:02X}, B=0x{:02X}, C=0x{:02X}\n\
             Flags=0b{:08b} [{}{}{}{}{}]",
            MAX_INSTRUCTIONS,
            cpu.pc, cpu.a, cpu.b, cpu.c, cpu.flags,
            if cpu.flags & FLAG_SIGN != 0 { "S" } else { "-" },
            if cpu.flags & FLAG_ZERO != 0 { "Z" } else { "-" },
            if cpu.flags & FLAG_AUX_CARRY != 0 { "A" } else { "-" },
            if cpu.flags & FLAG_PARITY != 0 { "P" } else { "-" },
            if cpu.flags & FLAG_CARRY != 0 { "C" } else { "-" }
        );
    }
}