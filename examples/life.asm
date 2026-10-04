; life.asm - Conway's Game of Life on a 32x16 torus, then returns to the monitor.
; Load: paste life.hex. Run: G 0100. About 24.6M cycles: 12 s at 2 MHz.
; Seed: an R-pentomino (rows 6-8, columns 20-22) and a glider (rows 1-3,
; columns 2-4), from the table SEED. Runs 128 generations and prints
; generations 0, 32, 64, 96 and 128: "Gen ddd", then 16 rows of 32 cells,
; "#" alive, "." dead. The edges wrap: row 0 neighbours row 15, column 0
; column 31.
; Each generation: for each row, the three rows around it (wrapped) are summed
; column by column into S, then each cell's 3x3 sum (itself included) is
; S[c-1] + S[c] + S[c+1], columns wrapped. Alive next if the sum is 3, or 4
; and the cell is alive. The new grid goes to NXT, then is copied to CUR.
; The program and its data use 0100-081F only, so it runs under the RAM test
; build too.
; Stresses: bulk memory reads and writes (about 7.2K data accesses to CUR, NXT
; and S a generation, 0.92M over the run), 16-bit pointer arithmetic (DAD H as a
; shift, INX, LDAX/STAX, INR/DCR of a pointer's high byte), wrap by masking
; (ANI), ADD M sums, the flags CPI/DCR/SUB leave for the branches, and one
; long G (the step budget in the tests is 30M cycles). tests/monitor_tests.rs
; checks the output against a Rust model (life_matches_a_reference_model).

        CPU     8080
        ORG     0100H

CUR     EQU     0400H           ; the grid, 16 rows of 32 bytes, 0 or 1
NXT     EQU     0600H           ; the next grid: CUR + 0200H
S       EQU     0800H           ; 32 column sums
GENS    EQU     128             ; generations run
EVERY   EQU     32              ; print every EVERY-th (a power of 2)

START:  LXI     H,CUR           ; clear the grid
CLEAR:  MVI     M,0
        INX     H
        MOV     A,H
        CPI     NXT>>8
        JNZ     CLEAR
        LXI     D,SEED
SEEDLP: LDAX    D               ; row, or FF at the end
        CPI     0FFH
        JZ      SEEDED
        CALL    ROWADR
        INX     D
        LDAX    D               ; column
        ORA     L
        MOV     L,A
        MVI     M,1
        INX     D
        JMP     SEEDLP
SEEDED: XRA     A
        STA     GEN
GENLP:  LDA     GEN
        ANI     EVERY-1
        CZ      SHOW
        LDA     GEN
        CPI     GENS
        RZ                      ; back to the monitor
        CALL    STEP
        LDA     GEN
        INR     A
        STA     GEN
        JMP     GENLP

; STEP: one generation, CUR to NXT, then NXT copied to CUR.
STEP:   XRA     A
        STA     ROW
ROWLP:  LDA     ROW             ; S = the row above
        DCR     A
        CALL    ROWADR
        XCHG
        LXI     H,S
        MVI     B,32
COPYS:  LDAX    D
        MOV     M,A
        INX     D
        INX     H
        DCR     B
        JNZ     COPYS
        LDA     ROW             ; S += the row below
        INR     A
        CALL    ROWADR
        XCHG
        CALL    ADDROW
        LDA     ROW             ; S += this row
        CALL    ROWADR
        PUSH    H
        XCHG
        CALL    ADDROW
        POP     D               ; DE = this row's cell 0 in CUR
        MVI     H,S>>8
        MVI     C,0             ; column
COLLP:  MOV     A,C             ; A = S[c-1] + S[c] + S[c+1]
        DCR     A
        ANI     1FH
        MOV     L,A
        MOV     A,M
        MOV     L,C
        ADD     M
        MOV     B,A
        MOV     A,C
        INR     A
        ANI     1FH
        MOV     L,A
        MOV     A,B
        ADD     M
        CPI     3
        JZ      LIVE
        CPI     4
        JNZ     DEAD
        LDAX    D               ; 4: stays as it is
        JMP     PUT
LIVE:   MVI     A,1
        JMP     PUT
DEAD:   XRA     A
PUT:    INR     D               ; DE + 0200H: the cell in NXT
        INR     D
        STAX    D
        DCR     D
        DCR     D
        INX     D
        INR     C
        MOV     A,C
        CPI     32
        JNZ     COLLP
        LDA     ROW
        INR     A
        STA     ROW
        CPI     16
        JNZ     ROWLP
        LXI     H,NXT           ; CUR = NXT
        LXI     D,CUR
COPY:   MOV     A,M
        STAX    D
        INX     H
        INX     D
        MOV     A,H
        CPI     (NXT+0200H)>>8
        JNZ     COPY
        RET

; ADDROW: S[c] += the 32 bytes at DE.
ADDROW: LXI     H,S
        MVI     B,32
ADDLP:  LDAX    D
        ADD     M
        MOV     M,A
        INX     D
        INX     H
        DCR     B
        JNZ     ADDLP
        RET

; ROWADR: HL = CUR + (A AND 0FH) * 32, row A wrapped to 0-15.
ROWADR: ANI     0FH
        MOV     L,A
        MVI     H,0
        DAD     H
        DAD     H
        DAD     H
        DAD     H
        DAD     H
        MOV     A,H
        ADI     CUR>>8          ; CUR's low byte is 00
        MOV     H,A
        RET

; SHOW: print "Gen ddd" and the grid.
SHOW:   LXI     H,MSGGEN
        CALL    PUTS
        LDA     GEN
        MVI     C,100
        CALL    DIGIT
        MVI     C,10
        CALL    DIGIT
        ADI     '0'
        OUT     00H
        LXI     H,CRLF
        CALL    PUTS
        LXI     H,CUR
SHOWR:  MVI     B,32
SHOWC:  MOV     A,M
        ORA     A
        MVI     A,'.'
        JZ      SHOW1
        MVI     A,'#'
SHOW1:  OUT     00H
        INX     H
        DCR     B
        JNZ     SHOWC
        MVI     A,0DH
        OUT     00H
        MVI     A,0AH
        OUT     00H
        MOV     A,H
        CPI     NXT>>8
        JNZ     SHOWR
        RET

; DIGIT: print A / C as a digit; A = A mod C.
DIGIT:  MVI     B,'0'-1
DIGLP:  INR     B
        SUB     C
        JNC     DIGLP
        ADD     C
        MOV     D,A
        MOV     A,B
        OUT     00H
        MOV     A,D
        RET

; PUTS: print the NUL-terminated string at HL.
PUTS:   MOV     A,M
        ORA     A
        RZ
        OUT     00H
        INX     H
        JMP     PUTS

MSGGEN: DB      "Gen ",0
CRLF:   DB      0DH,0AH,0
; SEED: row, column pairs, FF at the end.
SEED:   DB      6,21, 6,22, 7,20, 7,21, 8,21     ; R-pentomino
        DB      1,3, 2,4, 3,2, 3,3, 3,4          ; glider, heading down-right
        DB      0FFH
GEN:    DB      0               ; the generation
ROW:    DB      0               ; STEP's row

        IF      $ > CUR
        ERROR   "program overlaps CUR"
        ENDIF

        END     START
