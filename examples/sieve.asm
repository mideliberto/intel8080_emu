; sieve.asm - Sieve of Eratosthenes over 0..8191, then returns to the monitor.
; Load: paste sieve.hex. Run: G 0100. About 7.4M cycles (3.6 s at 2.048 MHz).
; Prints the number of primes below 8192, the last ten of them, and their sum:
;   1028 primes below 8192
;   Last ten: 8101 8111 8117 8123 8147 8161 8167 8171 8179 8191
;   Sum: 3908641
; One bit per number in a 1 KB array after the code (1 = composite), cleared
; at start, so G runs it again. tests/monitor_tests.rs computes all three
; lines itself (sieve_matches_a_model).
; Stresses: 16-bit loops on DAD/INX/DCX with CPI on the high byte; bit
; addressing (n/8 by a 3-step 16-bit right shift through CY with RAR, the
; mask by RLC); the tally walks the array with a mask whose RLC carry out of
; bit 7 advances the byte pointer; the sum is an ADD/ADC/ACI chain across
; INX and MOV, which must leave CY alone; decimal output divides 32 bits by
; 10 with an ADD/RAL shift chain through memory; the last ten go out through
; the stack in reverse, behind a sentinel.

        CPU     8080
        ORG     0100H

N       EQU     8192            ; the sieve covers 0..N-1; N is a multiple of 256

START:  LXI     H,BITS          ; clear the array: nothing marked yet
        LXI     D,N/8
CLR:    MVI     M,0
        INX     H
        DCX     D
        MOV     A,D
        ORA     E
        JNZ     CLR
        MVI     A,03H           ; 0 and 1 are not prime
        STA     BITS
        LXI     H,0
        SHLD    COUNT
        SHLD    SUM
        SHLD    SUM+2

; Sieve: for each prime p with p*p < N, mark p*p, p*p+p, ... up to N-1.
        LXI     D,2             ; DE = p
NEXTP:  LXI     H,0             ; HL = p*p (p < 256)
        MOV     B,E
SQ:     DAD     D
        DCR     B
        JNZ     SQ
        MOV     A,H
        CPI     N/256
        JNC     TALLY           ; p*p >= N: every composite is marked
        PUSH    H
        MOV     H,D
        MOV     L,E
        CALL    BITADR
        ANA     M
        POP     H
        JNZ     SKIP            ; p is composite
CROSS:  PUSH    H
        CALL    BITADR
        ORA     M
        MOV     M,A
        POP     H
        DAD     D               ; m += p (stays below N + 256: no carry)
        MOV     A,H
        CPI     N/256
        JC      CROSS
SKIP:   INX     D
        JMP     NEXTP

; Tally: n = 0..N-1 in DE, its byte at HL, its bit in mask C.
TALLY:  LXI     H,BITS
        LXI     D,0
        MVI     C,01H
TLOOP:  MOV     A,M
        ANA     C
        CZ      FOUND           ; a prime
        MOV     A,C
        RLC                     ; mask 80 -> 01 sets CY: next byte
        MOV     C,A
        JNC     TNEXT
        INX     H
TNEXT:  INX     D
        MOV     A,D
        CPI     N/256
        JC      TLOOP

        LHLD    COUNT
        CALL    PDEC16
        LXI     H,MSGN
        CALL    PUTS

; Last ten: walk down from N-1, push each prime; 0000 below them ends the list.
        LXI     H,0
        PUSH    H
        LXI     D,N-1
        MVI     B,10
LAST:   MOV     H,D
        MOV     L,E
        CALL    BITADR
        ANA     M
        JNZ     LNEXT
        PUSH    D
        DCR     B
        JZ      LPRINT
LNEXT:  DCX     D
        JMP     LAST
LPRINT: LXI     H,MSGL
        CALL    PUTS
LPOP:   POP     H               ; smallest first
        MOV     A,H
        ORA     L
        JZ      LEND
        MVI     A,' '
        OUT     00H
        CALL    PDEC16
        JMP     LPOP
LEND:   LXI     H,MSGS
        CALL    PUTS
        LHLD    SUM
        SHLD    NUM
        LHLD    SUM+2
        SHLD    NUM+2
        CALL    PDEC
        LXI     H,CRLF
        JMP     PUTS            ; PUTS returns to the monitor

; FOUND: COUNT += 1, SUM += DE (32 bits). Keeps BC, DE, HL.
FOUND:  PUSH    H
        LHLD    COUNT
        INX     H
        SHLD    COUNT
        LXI     H,SUM
        MOV     A,M
        ADD     E
        MOV     M,A
        INX     H
        MOV     A,M
        ADC     D
        MOV     M,A
        INX     H
        MOV     A,M
        ACI     0
        MOV     M,A
        INX     H
        MOV     A,M
        ACI     0
        MOV     M,A
        POP     H
        RET

; BITADR: HL = n -> HL = BITS + n/8, A = 1 shl (n mod 8). Keeps BC, DE.
BITADR: PUSH    B
        MOV     A,L
        ANI     07H
        MOV     C,A             ; bit number
        MVI     B,3
SHR:    MOV     A,H             ; HL >>= 1
        ORA     A               ; CY = 0
        RAR
        MOV     H,A
        MOV     A,L
        RAR
        MOV     L,A
        DCR     B
        JNZ     SHR
        PUSH    D
        LXI     D,BITS
        DAD     D
        POP     D
        MVI     A,01H
        INR     C
MASK:   DCR     C
        JZ      MDONE
        RLC
        JMP     MASK
MDONE:  POP     B
        RET

; PDEC16: print HL in decimal. PDEC: print the 32 bits at NUM in decimal,
; no leading zeros; NUM ends 0. Both destroy A, BC, E, HL.
PDEC16: SHLD    NUM
        LXI     H,0
        SHLD    NUM+2
PDEC:   MVI     E,0             ; digits on the stack
PDIV:   CALL    DIV10
        ADI     '0'
        PUSH    PSW
        INR     E
        LXI     H,NUM
        MOV     A,M
        INX     H
        ORA     M
        INX     H
        ORA     M
        INX     H
        ORA     M
        JNZ     PDIV
PPUT:   POP     PSW             ; most significant first
        OUT     00H
        DCR     E
        JNZ     PPUT
        RET

; DIV10: NUM /= 10 (32 bits, shift and subtract); A = the remainder.
DIV10:  MVI     B,32
        MVI     C,0             ; remainder
DLOOP:  LXI     H,NUM           ; NUM <<= 1; its top bit into CY
        MOV     A,M
        ADD     A
        MOV     M,A
        INX     H
        MOV     A,M
        RAL
        MOV     M,A
        INX     H
        MOV     A,M
        RAL
        MOV     M,A
        INX     H
        MOV     A,M
        RAL
        MOV     M,A
        MOV     A,C             ; remainder = 2 * remainder + CY (at most 19)
        RAL
        MOV     C,A
        SUI     10
        JC      DNEXT
        MOV     C,A
        LXI     H,NUM
        INR     M               ; quotient bit (bit 0 is 0 after the shift)
DNEXT:  DCR     B
        JNZ     DLOOP
        MOV     A,C
        RET

; PUTS: print the NUL-terminated string at HL.
PUTS:   MOV     A,M
        ORA     A
        RZ
        OUT     00H
        INX     H
        JMP     PUTS

MSGN:   DB      " primes below 8192",0DH,0AH,0
MSGL:   DB      "Last ten:",0
MSGS:   DB      0DH,0AH,"Sum: ",0
CRLF:   DB      0DH,0AH,0

COUNT:  DS      2               ; set at run time: no records in the .hex
SUM:    DS      4               ; little-endian
NUM:    DS      4               ; PDEC's dividend
BITS:   DS      N/8

        END     START
