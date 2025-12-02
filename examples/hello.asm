; hello.asm - Simple test program
; Prints "Hello" to console and halts
;
; Assemble with AS:
;   asl -cpu 8080 hello.asm
;   p2bin hello.p hello.bin

        CPU     8080
        ORG     0100H

START:
        LXI     H,MESSAGE
LOOP:
        MOV     A,M
        ORA     A               ; Check for null terminator
        JZ      DONE
        OUT     00H             ; Output to console
        INX     H
        JMP     LOOP
        
DONE:
        HLT

MESSAGE:
        DB      "Hello, 8080!",0DH,0AH,0

        END     START
