; hanoi.asm - recursive Towers of Hanoi, then returns to the monitor.
; Load: paste hanoi.hex. Run: G 0100.
; Prints every move for 4 disks, A to C ("1: A->B" = disk 1 from peg A to
; peg B), then for n = 1 to 16 disks the number of moves, 2^n - 1, counted in
; HL by performing every move of the recursion. Then "Stack OK" if SP is back
; where G left it, "Stack BAD" if not. About 13.9M T-states: 7 s at 2 MHz
; (n = 16 alone is half of it).
;
; What it stresses: CALL, RET, the conditional RZ, PUSH and POP of BC and DE,
; and stack discipline at depth. HANOI(n) for n > 1 is a 6-byte frame (return
; address, BC, DE), so n = 16 nests 16 frames below its caller (SP reaches
; EFA2 from G's EFFE). The second recursive call is a tail call (JMP after the
; frame is popped), so a wrong PUSH/POP pairing or SP arithmetic shows up as a
; wrong peg, disk or count, or a lost return address. Also DCR B into Z, INX H
; carrying through 16 bits (n = 16 counts to FFFF), and the decimal print's
; 16-bit SUB/SBB borrow chain and DAD. Rust checks it against its own model
; (hanoi_matches_a_rust_model in tests/monitor_tests.rs).

        CPU     8080
        ORG     0100H

START:  LXI     H,0
        DAD     SP
        SHLD    SP0             ; SP as G left it
        LXI     H,MSGTOP
        CALL    PUTS
        MVI     A,1
        STA     SHOW
        MVI     B,4
        CALL    SOLVE
        XRA     A
        STA     SHOW
        LXI     H,MSGCNT
        CALL    PUTS
        MVI     A,1
NLOOP:  STA     N
        MOV     B,A
        CALL    SOLVE           ; HL = the move count
        PUSH    H
        LXI     H,MSGN
        CALL    PUTS
        LDA     N
        MOV     L,A
        MVI     H,0
        CALL    PDEC
        LXI     H,MSGSEP
        CALL    PUTS
        POP     H
        CALL    PDEC
        LXI     H,CRLF
        CALL    PUTS
        LDA     N
        INR     A
        CPI     17
        JNZ     NLOOP
        LHLD    SP0             ; SP now must equal SP at entry
        XCHG
        LXI     H,0
        DAD     SP
        MOV     A,L
        CMP     E
        JNZ     SPBAD
        MOV     A,H
        CMP     D
        JNZ     SPBAD
        LXI     H,MSGOK
        JMP     PUTS            ; PUTS returns to the monitor
SPBAD:  LXI     H,MSGBAD
        JMP     PUTS

; SOLVE: move B disks (B > 0) from peg A to peg C. HL = the moves made.
SOLVE:  LXI     H,0
        MVI     C,'A'
        MVI     D,'C'
        MVI     E,'B'
                                ; fall into HANOI
; HANOI: move B disks (B > 0) from peg C to peg D through peg E, counting
; each move in HL. Changes A, B, C, D, E.
HANOI:  DCR     B               ; B = n - 1
        JZ      MOVE            ; n = 1: one move, MOVE returns
        PUSH    B
        PUSH    D
        MOV     A,D             ; n - 1 disks from C to E through D
        MOV     D,E
        MOV     E,A
        CALL    HANOI
        POP     D
        POP     B
        CALL    MOVE            ; disk n from C to D
        MOV     A,C             ; n - 1 disks from E to D through C:
        MOV     C,E             ; a tail call, B is already n - 1
        MOV     E,A
        JMP     HANOI

; MOVE: move disk B + 1 from peg C to peg D: count it in HL, and print it
; when SHOW is not zero. Changes A only.
MOVE:   INX     H
        LDA     SHOW
        ORA     A
        RZ
        MOV     A,B
        ADI     '1'
        OUT     00H
        MVI     A,':'
        OUT     00H
        MVI     A,' '
        OUT     00H
        MOV     A,C
        OUT     00H
        MVI     A,'-'
        OUT     00H
        MVI     A,'>'
        OUT     00H
        MOV     A,D
        OUT     00H
        MVI     A,0DH
        OUT     00H
        MVI     A,0AH
        OUT     00H
        RET

; PDEC: print HL in decimal without leading zeros. Changes all registers.
PDEC:   MVI     B,'0'           ; the digit to suppress: '0' until one prints
        LXI     D,10000
        CALL    DIGIT
        LXI     D,1000
        CALL    DIGIT
        LXI     D,100
        CALL    DIGIT
        LXI     D,10
        CALL    DIGIT
        MOV     A,L             ; the units digit always prints
        ADI     '0'
        OUT     00H
        RET

; DIGIT: print HL / DE (0-9) unless it is B; HL = HL mod DE. B = 0 once a
; digit has printed (no digit is 0).
DIGIT:  MVI     C,'0'
DLOOP:  MOV     A,L             ; HL = HL - DE
        SUB     E
        MOV     L,A
        MOV     A,H
        SBB     D
        MOV     H,A
        JC      DDONE           ; borrow: went below zero
        INR     C
        JMP     DLOOP
DDONE:  DAD     D               ; undo the last subtraction
        MOV     A,C
        CMP     B
        RZ                      ; a leading zero
        MVI     B,0
        OUT     00H
        RET

; PUTS: print the NUL-terminated string at HL.
PUTS:   MOV     A,M
        ORA     A
        RZ
        OUT     00H
        INX     H
        JMP     PUTS

MSGTOP: DB      "Hanoi, 4 disks, A to C:",0DH,0AH,0
MSGCNT: DB      "Moves for n disks:",0DH,0AH,0
MSGN:   DB      "n=",0
MSGSEP: DB      ": ",0
MSGOK:  DB      "Stack OK"
CRLF:   DB      0DH,0AH,0
MSGBAD: DB      "Stack BAD",0DH,0AH,0
SHOW:   DB      0               ; not zero: MOVE prints
N:      DB      0
SP0:    DW      0

        END     START
