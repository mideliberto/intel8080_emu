# Example programs (USER_GUIDE.md 5). Each NAME.asm builds NAME.hex: Intel HEX,
# 16-byte records, for pasting at the monitor prompt. The .hex files are committed,
# so cargo test needs no assembler; tests/transcripts/example_NAME.txt pastes them.

HEX = $(patsubst %.asm,%.hex,$(wildcard *.asm))

all: $(HEX)

%.hex: %.asm
	asl -cpu 8080 -q $<
	p2hex -q $*.p $@ -F Intel -l 16
	rm -f $*.p

clean:
	rm -f *.p *.lst
