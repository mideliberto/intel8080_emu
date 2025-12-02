# Makefile for 8080 Monitor ROM
# Requires AS (Macro Assembler AS) - http://john.ccac.rwth-aachen.de:8000/as/

ASL = asl
P2BIN = p2bin

# Source files
ROM_SRC = monitor.asm

# Output files
ROM_BIN = monitor.bin
ROM_P = monitor.p

.PHONY: all clean

all: $(ROM_BIN)

# Assemble to .p file
$(ROM_P): $(ROM_SRC)
	$(ASL) -cpu 8080 -L $(ROM_SRC)

# Convert .p to binary
$(ROM_BIN): $(ROM_P)
	$(P2BIN) $(ROM_P) $(ROM_BIN) -r '$$F000-$$FFFF'

clean:
	rm -f *.p *.bin *.lst

# Show disassembly (requires objdump or similar)
disasm: $(ROM_BIN)
	hexdump -C $(ROM_BIN) | head -64

# Check ROM size
size: $(ROM_BIN)
	@echo "ROM size: $$(wc -c < $(ROM_BIN)) bytes (max 4096)"
	@test $$(wc -c < $(ROM_BIN)) -le 4096 || echo "WARNING: ROM exceeds 4KB!"
