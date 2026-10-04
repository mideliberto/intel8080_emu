; pi.asm - prints pi to 100 decimal places, then returns to the monitor.
; Load: paste pi.hex. Run: G 0100. About 22M T-states: 11 s at 2 MHz.
; Prints "3." and then the decimals, WIDTH to a line.
;
; The Rabinowitz-Wagon spigot (Amer. Math. Monthly 102, 1995, 195-203): pi is
; 2 + 1/3 (2 + 2/5 (2 + 3/7 (2 + ...))), held as the digits A(0)..A(LEN) of
; that mixed radix, all 2 to start. Each pass multiplies by 10 from the tail:
; X = 10 A(p) + Q (p+1), A(p) = X mod (2p+1), Q = X div (2p+1), for p = TOP
; down to 1, then X = 10 A(0) + Q gives the next digit X div 10 and A(0) =
; X mod 10. That digit can be 9 or 10, so it is held: 9s are counted, a 10
; adds one to the held predigit and turns the counted 9s into 0s. A pass needs
; fewer terms as fewer digits remain: TOP starts at LEN and drops by 3 a pass
; (10/3 terms a digit, LEN has 4 digits' margin).
;
; Stresses: 8x16 shift-add multiply (DAD H as a shift, ADD A for the next
; multiplier bit, the carry out of each), 16-bit divide by repeated SUB/SBB
; to a borrow, 16-bit add chains (DAD into 10 A), LHLD/SHLD and word access
; through a 694-byte array walked downward, and about 19,800 inner steps (the
; Q bound: Q < 30, so X < 65536 and every step fits 16 bits). The held-digit
; carry: the spigot makes a 10 at decimals 32 and 85 (they print 0, and 31
; and 84 print one more than the spigot made them); a 10 after held 9s first
; comes at decimal 361, past this run.
; DIGITS must be a multiple of WIDTH; both are assembly constants.

        CPU     8080
        ORG     0100H

DIGITS  EQU     100             ; decimals printed
WIDTH   EQU     50              ; decimals to a line
LEN     EQU     (10*(DIGITS+4))/3
DROP    EQU     3               ; terms dropped each pass

START:  LXI     H,ARRAY         ; A(0..LEN) = 2
        LXI     D,LEN+1
INIT:   MVI     M,2
        INX     H
        MVI     M,0
        INX     H
        DCX     D
        MOV     A,D
        ORA     E
        JNZ     INIT
        LXI     H,LEN
        SHLD    TOP
        MVI     A,0FFH          ; no predigit yet
        STA     PRE
        XRA     A
        STA     NINES
        STA     COL
        MVI     A,DIGITS+1      ; the 3 and the decimals
        STA     LEFT

PASS:   LHLD    TOP
        INX     H
        SHLD    P1              ; p+1
        DCX     H
        DAD     H
        LXI     D,ARRAY
        DAD     D
        SHLD    PTR             ; address of A(p)
        MVI     C,0             ; Q

; One term: HL = Q * (p+1), 8 bits by 16, high multiplier bit first.
TERM:   LHLD    P1
        XCHG
        LXI     H,0
        MOV     A,C
        MVI     B,8
MUL:    DAD     H
        ADD     A
        JNC     MUL1
        DAD     D
MUL1:   DCR     B
        JNZ     MUL
; X = 10 A(p) + Q (p+1)
        XCHG                    ; DE = Q (p+1)
        LHLD    PTR
        MOV     A,M
        INX     H
        MOV     H,M
        MOV     L,A             ; HL = A(p)
        DAD     H
        MOV     B,H
        MOV     C,L             ; BC = 2 A(p)
        DAD     H
        DAD     H
        DAD     B               ; 10 A(p)
        DAD     D               ; X
; Q = X div (2p+1), A(p) = X mod (2p+1)
        XCHG                    ; DE = X
        LHLD    P1
        DAD     H
        DCX     H               ; 2p+1
        XCHG                    ; HL = X, DE = 2p+1
        MVI     C,0FFH
DIV:    INR     C
        MOV     A,L
        SUB     E
        MOV     L,A
        MOV     A,H
        SBB     D
        MOV     H,A
        JNC     DIV
        DAD     D               ; the remainder
        XCHG
        LHLD    PTR
        MOV     M,E
        INX     H
        MOV     M,D
        DCX     H
        DCX     H
        DCX     H
        SHLD    PTR             ; A(p-1)
        LHLD    P1
        DCX     H
        SHLD    P1              ; the next term's p+1
        MOV     A,H
        ORA     A
        JNZ     TERM
        MOV     A,L
        CPI     1
        JNZ     TERM            ; until p = 1 is done

; A(0): the digit is (10 A(0) + Q) div 10; under 130, so 8 bits.
        LDA     ARRAY
        MOV     B,A
        ADD     A
        ADD     A
        ADD     B
        ADD     A               ; 10 A(0)
        ADD     C
        MVI     B,0FFH
TEN:    INR     B
        SUI     10
        JNC     TEN
        ADI     10
        STA     ARRAY           ; A(0) = X mod 10
        MOV     A,B             ; the digit
        CPI     9
        JZ      NINE
        CPI     10
        JZ      CARRY
        LDA     PRE             ; 0-8: print the predigit and the 9s
        CPI     0FFH
        CNZ     PUT
        MVI     C,9
        CALL    FLUSH
        MOV     A,B
        STA     PRE             ; it is the new predigit
        JMP     NEXT
NINE:   LXI     H,NINES         ; 9: hold it
        INR     M
        JMP     NEXT
CARRY:  LDA     PRE             ; 10: the predigit + 1, the 9s become 0s
        INR     A
        CALL    PUT
        XRA     A
        STA     PRE
        MVI     C,0
        CALL    FLUSH
NEXT:   LDA     LEFT
        ORA     A
        RZ                      ; all printed: back to the monitor
        LHLD    TOP
        LXI     D,-DROP
        DAD     D
        SHLD    TOP
        JMP     PASS

; FLUSH: print digit C NINES times; NINES = 0.
FLUSH:  LXI     H,NINES
FL1:    MOV     A,M
        ORA     A
        RZ
        DCR     M
        MOV     A,C
        CALL    PUT
        JMP     FL1

; PUT: print digit A, unless all are printed. "." and CR LF after the 3,
; CR LF after every WIDTH decimals. Keeps BC and HL.
PUT:    MOV     E,A
        LDA     LEFT
        ORA     A
        RZ
        DCR     A
        STA     LEFT
        MOV     A,E
        ADI     '0'
        OUT     00H
        LDA     LEFT
        CPI     DIGITS
        JNZ     PUT1
        MVI     A,'.'
        OUT     00H
        JMP     CRLF
PUT1:   LDA     COL
        INR     A
        STA     COL
        CPI     WIDTH
        RNZ
        XRA     A
        STA     COL
CRLF:   MVI     A,0DH
        OUT     00H
        MVI     A,0AH
        OUT     00H
        RET

TOP:    DS      2               ; the highest term this pass
P1:     DS      2               ; p+1 for the term being worked
PTR:    DS      2               ; address of A(p)
PRE:    DS      1               ; the held predigit, FF before the first
NINES:  DS      1               ; 9s held after it
COL:    DS      1               ; decimals on this line
LEFT:   DS      1               ; digits still to print
ARRAY:  DS      2*(LEN+1)       ; A(0..LEN), words

        END     START
