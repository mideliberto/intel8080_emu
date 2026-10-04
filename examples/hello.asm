; hello.asm - prints a line and returns to the monitor.
; Load: paste hello.hex at the monitor prompt. Run: G 0100.
; Shows the two rules every program here follows: console output is
; OUT 00H (no status poll, DEVICE_SPECS 4), and the exit is RET
; (G pushed the monitor's return address, MONITOR_SPEC 8).

        CPU     8080
        ORG     0100H

START:  LXI     H,MSG
LOOP:   MOV     A,M
        ORA     A
        RZ                      ; NUL: back to the monitor prompt
        OUT     00H
        INX     H
        JMP     LOOP

MSG:    DB      "Hello, 8080!",0DH,0AH,0

        END     START
