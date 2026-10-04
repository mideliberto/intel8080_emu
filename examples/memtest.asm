; memtest.asm - RAM test over FIRST..LAST, then returns to the monitor.
; Load: paste memtest.hex. Run: G 0100.
; Default range 0200-EEFF: the user area above the program page (0100-01FF
; is not tested), never the workspace or the stack page. Change it with
; E 0103 (FIRST, low byte first) and E 0105 (LAST). FIRST must not be above
; LAST. Under the RAM test build, set LAST to CFFF first (E 0105, FF, CF, .):
; the default range overwrites the running RAM monitor at D000-EEFF.
; Two passes. Each writes the whole range with an address pattern (low XOR
; high, then its complement) before it reads any of it back, so a stuck
; bit and an address line that aliases two locations both fail.
; Prints "RAM OK", or "FAIL aaaa" for the first bad address.

        CPU     8080
        ORG     0100H

START:  JMP     MAIN
FIRST:  DW      0200H
LAST:   DW      0EEFFH

MAIN:   MVI     C,00H           ; pass 1: L XOR H
        CALL    PASS
        RC                      ; FAIL printed
        MVI     C,0FFH          ; pass 2: the complement
        CALL    PASS
        RC
        LXI     H,MSGOK
        JMP     PUTS            ; PUTS returns to the monitor

; PASS: write L XOR H XOR C over the range, then read it all back.
; CY clear on success; CY set after printing FAIL.
PASS:   CALL    RANGE
WLOOP:  MOV     A,L
        XRA     H
        XRA     C
        MOV     M,A
        INX     H
        DCX     D
        MOV     A,D
        ORA     E
        JNZ     WLOOP
        CALL    RANGE
RLOOP:  MOV     A,L
        XRA     H
        XRA     C
        CMP     M
        JNZ     BAD
        INX     H
        DCX     D
        MOV     A,D
        ORA     E               ; clears CY
        JNZ     RLOOP
        RET

; RANGE: HL = FIRST, DE = LAST - FIRST + 1 (0000 means 65536).
RANGE:  LHLD    FIRST
        XCHG
        LHLD    LAST
        MOV     A,L
        SUB     E
        MOV     L,A
        MOV     A,H
        SBB     D
        MOV     H,A
        INX     H
        XCHG
        RET

BAD:    PUSH    H
        LXI     H,MSGBAD
        CALL    PUTS
        POP     H
        MOV     A,H
        CALL    HEX2
        MOV     A,L
        CALL    HEX2
        LXI     H,CRLF
        CALL    PUTS
        STC
        RET

; HEX2: print A as two hex digits.
HEX2:   PUSH    PSW
        RRC
        RRC
        RRC
        RRC
        CALL    HEX1
        POP     PSW
HEX1:   ANI     0FH
        ADI     90H             ; 0-9 -> '0'-'9', A-F -> 'A'-'F'
        DAA
        ACI     40H
        DAA
        OUT     00H
        RET

; PUTS: print the NUL-terminated string at HL.
PUTS:   MOV     A,M
        ORA     A
        RZ
        OUT     00H
        INX     H
        JMP     PUTS

MSGOK:  DB      "RAM OK"
CRLF:   DB      0DH,0AH,0
MSGBAD: DB      "FAIL ",0

        END     START
