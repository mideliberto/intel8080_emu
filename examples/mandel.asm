; mandel.asm - prints the Mandelbrot set as ASCII art, then returns to the monitor.
; Load: paste mandel.hex. Run: G 0100. About 17M T-states: 8.5 s at 2 MHz.
; Region: real -2.0 to 0.44 across COLS columns, imaginary -1.18 to 1.18 down
; ROWS rows (the middle row is the real axis). Each character is the iteration
; count n at which z escaped, as RAMP[n]; a space never escaped in MAXIT
; iterations (the set).
;
; Numbers are 16-bit signed 4.12 fixed point (4096 = 1.0). For each point c:
; z = c, n = 0; then repeat: x2 = x*x, y2 = y*y as exact 32-bit products
; (8.24); if x2 + y2 > 4.0 (unsigned 32-bit compare) print RAMP[n]; else
; x = (x2 - y2) >> 12 + cx and y = (x*y) >> 11 + cy, both from the old x and
; y, with >> an arithmetic shift (floor) and the result cut to 16 bits;
; n = n + 1; at n = MAXIT print a space. When x2 + y2 <= 4.0, |x|, |y| <= 2,
; so the new |x|, |y| <= 6: no 16-bit overflow, and no 32-bit one in the next
; squares. tests/monitor_tests.rs has a Rust model of this exact integer
; algorithm (mandel_matches_the_model) and calls MUL alone against i32
; multiply (mandel_mul_is_exact).
;
; Stresses: carry and sign flags through long arithmetic chains. MUL is a
; signed 16x16->32 multiply: JP on the operands' signs, CMA and INX to make
; them positive, PUSH/POP PSW to keep the product's sign (XRA's S) across the
; work, and SUB/SBB from zero (MVI between, which sets no flags) to negate.
; The unsigned part is two 16x8 shift-and-add passes: DAD H shifts H:L out
; through CY, RAL takes CY into A and the next multiplier bit out of it, DAD D
; adds and ACI 0 carries into A; then ADD/ADC combine the halves. The 32-bit
; sum is ADD/ADC and the difference SUB/SBB through memory, with INX, LDAX and
; MOV between the bytes, which must leave CY alone; the shifts are DAD H then
; RAL; the escape test is CPI's CY and Z. About 5900 iterations, 17,100
; multiplies.

        CPU     8080
        ORG     0100H

COLS    EQU     40
ROWS    EQU     19
MAXIT   EQU     16
X0      EQU     -8192           ; -2.0
DX      EQU     256             ; 2.5 / 40
DY      EQU     539             ; 2.5 / 19
Y0      EQU     -9*DY           ; the middle row (9) is y = 0

START:  LXI     H,Y0
        SHLD    CY
        MVI     A,ROWS
        STA     ROWN
ROW:    LXI     H,X0
        SHLD    CX
        MVI     A,COLS
        STA     COLN
COL:    LHLD    CX              ; z = c
        SHLD    ZX
        LHLD    CY
        SHLD    ZY
        XRA     A
        STA     N
ITER:   LHLD    ZX              ; XX = x * x
        MOV     B,H
        MOV     C,L
        XCHG
        CALL    MUL
        SHLD    XX
        XCHG
        SHLD    XX+2
        LHLD    ZY              ; YY = y * y
        MOV     B,H
        MOV     C,L
        XCHG
        CALL    MUL
        SHLD    YY
        XCHG
        SHLD    YY+2
        LXI     H,YY            ; D:E:B:C = XX + YY
        LDA     XX
        ADD     M
        MOV     C,A
        INX     H
        LDA     XX+1
        ADC     M
        MOV     B,A
        INX     H
        LDA     XX+2
        ADC     M
        MOV     E,A
        INX     H
        LDA     XX+3
        ADC     M
        MOV     D,A
        CPI     4               ; escaped if above 04000000
        JC      STEP            ; D < 4
        JNZ     ESC             ; D > 4
        MOV     A,E
        ORA     B
        ORA     C
        JNZ     ESC             ; D = 4, the rest not all zero
STEP:   LHLD    ZX              ; y = (x * y) >> 11 + cy
        XCHG
        LHLD    ZY
        MOV     B,H
        MOV     C,L
        CALL    MUL             ; D:E:H:L = x * y
        MOV     A,D             ; A:H:L = bits 31-8
        MOV     L,H
        MOV     H,E
        DAD     H               ; << 5: A:H = bits 26-11
        RAL
        DAD     H
        RAL
        DAD     H
        RAL
        DAD     H
        RAL
        DAD     H
        RAL
        MOV     L,H
        MOV     H,A
        XCHG
        LHLD    CY
        DAD     D
        SHLD    ZY
        LXI     D,XX            ; A:H:L = bits 31-8 of XX - YY
        LXI     H,YY
        LDAX    D
        SUB     M               ; byte 0: only its borrow is kept
        INX     D
        INX     H
        LDAX    D
        SBB     M
        MOV     C,A
        INX     D
        INX     H
        LDAX    D
        SBB     M
        MOV     B,A
        INX     D
        INX     H
        LDAX    D
        SBB     M
        MOV     H,B
        MOV     L,C
        DAD     H               ; << 4: A:H = bits 27-12
        RAL
        DAD     H
        RAL
        DAD     H
        RAL
        DAD     H
        RAL
        MOV     L,H
        MOV     H,A
        XCHG
        LHLD    CX              ; x = (XX - YY) >> 12 + cx
        DAD     D
        SHLD    ZX
        LDA     N
        INR     A
        STA     N
        CPI     MAXIT
        JC      ITER
        MVI     A,' '           ; never escaped
        JMP     PUT
ESC:    LDA     N
        MOV     E,A
        MVI     D,0
        LXI     H,RAMP
        DAD     D
        MOV     A,M
PUT:    OUT     00H
        LHLD    CX              ; next column
        LXI     D,DX
        DAD     D
        SHLD    CX
        LDA     COLN
        DCR     A
        STA     COLN
        JNZ     COL
        MVI     A,0DH
        OUT     00H
        MVI     A,0AH
        OUT     00H
        LHLD    CY              ; next row
        LXI     D,DY
        DAD     D
        SHLD    CY
        LDA     ROWN
        DCR     A
        STA     ROWN
        JNZ     ROW
        RET

; MUL: D:E:H:L = DE * BC, signed. Uses A, B, C.
MUL:    MOV     A,D             ; the product's sign, kept in PSW
        XRA     B
        PUSH    PSW
        MOV     A,D             ; DE = |DE|
        ORA     A
        JP      MUL1
        CMA
        MOV     D,A
        MOV     A,E
        CMA
        MOV     E,A
        INX     D
MUL1:   MOV     A,B             ; BC = |BC|
        ORA     A
        JP      MUL2
        CMA
        MOV     B,A
        MOV     A,C
        CMA
        MOV     C,A
        INX     B
MUL2:   LXI     H,0             ; A:H:L = DE * C: 8 times, shift A:H:L left;
        MOV     A,C             ; if a multiplier bit (from the top of A)
        REPT    8               ; came out, add DE, carrying into A. The
        DAD     H               ; product so far fits below the multiplier
        RAL                     ; bits left in A.
        JNC     $+6             ; past the ACI
        DAD     D
        ACI     0
        ENDM
        MOV     C,A             ; C:(stack) = DE * C
        PUSH    H
        LXI     H,0             ; A:H:L = DE * B, the same way
        MOV     A,B
        REPT    8
        DAD     H
        RAL
        JNC     $+6
        DAD     D
        ACI     0
        ENDM
        POP     D               ; D:E:H:L = A:H:L << 8 + C:D:E
        MOV     B,A
        MOV     A,L
        ADD     D
        MOV     L,A
        MOV     A,H
        ADC     C
        MOV     H,A
        MVI     A,0
        ADC     B
        MOV     D,A
        MOV     A,E
        MOV     E,H
        MOV     H,L
        MOV     L,A
        POP     PSW
        RP                      ; operands of the same sign
        XRA     A               ; D:E:H:L = 0 - D:E:H:L
        SUB     L
        MOV     L,A
        MVI     A,0
        SBB     H
        MOV     H,A
        MVI     A,0
        SBB     E
        MOV     E,A
        MVI     A,0
        SBB     D
        MOV     D,A
        RET

RAMP:   DB      ".,:;-~=+*xoO%&$@"      ; by n; MAXIT characters

CX:     DS      2
CY:     DS      2
ZX:     DS      2
ZY:     DS      2
XX:     DS      4
YY:     DS      4
N:      DS      1
COLN:   DS      1
ROWN:   DS      1

        END     START
