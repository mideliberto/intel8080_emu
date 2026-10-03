# Makefile for 8080 Monitor ROM
# Requires AS (Macro Assembler AS) - http://john.ccac.rwth-aachen.de:8000/as/

ASL = asl
P2BIN = p2bin
P2HEX = p2hex

# Source files
ROM_SRC = monitor.asm

# Output files (monitor.bin, monitor.sym and monitor_ram.hex are committed)
ROM_BIN = monitor.bin
ROM_SYM = monitor.sym
ROM_P = monitor.p
RAM_HEX = monitor_ram.hex
RAM_P = monitor_ram.p

.PHONY: all clean size disasm

all: $(ROM_BIN) $(ROM_SYM) $(RAM_HEX)

# Assemble to .p file (and .lst, and .noi with the labels)
$(ROM_P): $(ROM_SRC)
	$(ASL) -cpu 8080 -L -g NOICE $(ROM_SRC)

# Debugger symbols (ARCHITECTURE 7.4): one 'AAAA NAME' line per label (code and workspace).
# asl writes 0x80, not 0x0080: pad with zeros, then keep the last 4 digits.
$(ROM_SYM): $(ROM_P)
	sed -n 's/^DEFINE \([A-Z0-9_]*\) 0x\([0-9a-f]*\)$$/000\2 \1/p' monitor.noi | sed 's/^0*\([0-9a-f]\{4\} \)/\1/' | tr a-f A-F | sort > $(ROM_SYM)

# Convert .p to binary
$(ROM_BIN): $(ROM_P)
	$(P2BIN) $(ROM_P) $(ROM_BIN) -r '$$F000-$$FFFF'

# RAM test build (ARCHITECTURE 2.1): the same source with RAMBUILD defined, at D000,
# as Intel HEX with 16-byte records for the resident monitor's HEX loader.
$(RAM_P): $(ROM_SRC)
	$(ASL) -cpu 8080 -D RAMBUILD -o $(RAM_P) $(ROM_SRC)

$(RAM_HEX): $(RAM_P)
	$(P2HEX) $(RAM_P) $(RAM_HEX) -F Intel -l 16

clean:
	rm -f *.p *.bin *.lst *.noi *.sym $(RAM_HEX)

# Show disassembly (requires objdump or similar)
disasm: $(ROM_BIN)
	hexdump -C $(ROM_BIN) | head -64

# Bytes used: ROM_END - F000 (ARCHITECTURE 2). The bin is padded to 4096, so
# its file size measures nothing; asl itself errors past FFFF.
size: $(ROM_SYM)
	@end=$$(sed -n 's/^\([0-9A-F]*\) ROM_END$$/\1/p' $(ROM_SYM)); \
	echo "ROM used: $$((0x$$end - 0xF000)) of 4096 bytes"
