; tictac.asm - tic-tac-toe: you are X, the computer is O and never loses.
; Load: paste tictac.hex. Run: G 0100.
; You move first. The board prints as three lines, a free square as its digit,
; then the prompt "> ". Type one key, 1-9 (no Enter needed; CR and LF are
; ignored). Any other key, or a taken square, prints "Bad move" and prompts
; again. The computer prints "I play n". The game ends with "You win" (never
; happens), "I win" or "Draw", then "Again? (Y/N)": Y starts a new game, N
; returns to the monitor, any other key asks again.
;
; The computer searches the whole game tree: negamax with alpha-beta pruning
; (NEGA). A win scores the number of squares still empty plus one, so it takes
; the fastest win and puts off a loss; ties go to the lowest square. Its first
; reply is the longest search, up to 7.4M T-states (3.6 s at 2.048 MHz), so
; "I play " prints before the search starts.
;
; Stresses: recursion. NEGA calls itself up to 8 deep, each level saving HL, BC
; and DE on the stack around the call, so the result depends on every PUSH, POP,
; CALL and RET restoring exactly what it saved, thousands of times a move.
; Also signed 8-bit compares by SUB and the sign flag (JM, JP), negation by
; CMA/INR, and LDAX B / MOV r,M table walks. tests/monitor_tests.rs plays every
; game a human can play against it and checks each reply against a minimax in
; Rust (tictac_never_loses_and_plays_optimal_moves).

        CPU     8080
        ORG     0100H

XMARK   EQU     1               ; a line sums to 3 only as X X X
OMARK   EQU     4               ; and to 12 only as O O O

START:  JMP     MAIN
; BOARD and FREE are the whole game state; the exhaustive test writes them
; (ttt_memory in tests/monitor_tests.rs), so they stay at 0103 and 010C.
BOARD:  DB      0,0,0,0,0,0,0,0,0 ; 0103-010B: one page, so H = 01 for all
FREE:   DB      0               ; empty squares
BESTP:  DW      0               ; NEGA's best root move (its address)

MAIN:   LXI     H,MSGHI
        CALL    PUTS
        LXI     H,BOARD
        MVI     B,9
CLEAR:  MVI     M,0
        INX     H
        DCR     B
        JNZ     CLEAR
        MVI     A,9
        STA     FREE
HUMAN:  CALL    SHOW
ASK:    LXI     H,PROMPT
        CALL    PUTS
        CALL    GETKEY
        SUI     '1'
        CPI     9
        JNC     BAD             ; not 1-9
        ADI     BOARD & 0FFH
        MOV     L,A
        MVI     H,BOARD >> 8
        MOV     A,M
        ORA     A
        JNZ     BAD             ; taken
        MVI     D,XMARK
        CALL    MOVE
        LXI     H,MSGYOU
        JC      OVER
        JZ      DRAW
        LXI     H,MSGME         ; the computer's turn: "I play " shows it
        CALL    PUTS            ; thinking
        LDA     FREE
        MOV     E,A
        LXI     B,0EC14H        ; alpha -20, beta 20: wider than any score
        MVI     D,OMARK
        CALL    NEGA
        LHLD    BESTP
        MOV     A,L
        SUI     (BOARD - '1') & 0FFH
        OUT     00H
        LXI     H,CRLF
        CALL    PUTS
        LHLD    BESTP
        MVI     D,OMARK
        CALL    MOVE
        LXI     H,MSGIW
        JC      OVER
        JNZ     HUMAN
DRAW:   LXI     H,MSGDRW
OVER:   PUSH    H
        CALL    SHOW
        POP     H
        CALL    PUTS
AGAIN:  LXI     H,MSGAGN
        CALL    PUTS
        CALL    GETKEY
        ANI     0DFH            ; lower case to upper
        CPI     'Y'
        JZ      MAIN
        CPI     'N'
        JNZ     AGAIN
        RET                     ; to the monitor

BAD:    LXI     H,MSGBAD
        CALL    PUTS
        JMP     ASK

; MOVE: put mark D on the square at HL. CY set if D now holds a line, else Z
; set if the board is full.
MOVE:   MOV     M,D
        LXI     H,FREE
        DCR     M
        CALL    WINS
        RC
        MOV     A,M
        ORA     A
        RET

; NEGA: negamax with alpha-beta. D = the mark to move, E = empty squares (at
; least 1, and the other side has no line), B = alpha, C = beta (signed, within
; -20..20, alpha below beta). Returns A = the score for D, clamped to alpha..beta.
; A move that wins scores E (the squares empty after it, plus one), one that
; fills the board scores 0, any other the negated score of the reply. At the
; root (E = FREE) the address of the first best move goes to BESTP.
NEGA:   LXI     H,BOARD
NLOOP:  MOV     A,M
        ORA     A
        JNZ     NNEXT           ; taken
        MOV     M,D             ; try it
        CALL    WINS
        MOV     A,E
        JC      NUNDO           ; a win scores E
        DCR     A
        JZ      NUNDO           ; the board is full: a draw scores 0
        PUSH    H
        PUSH    B
        PUSH    D
        MOV     E,A
        MOV     A,B
        CMA
        INR     A
        MOV     H,A             ; -alpha
        MOV     A,C
        CMA
        INR     A
        MOV     B,A             ; the reply's alpha = -beta
        MOV     C,H             ; and its beta = -alpha
        MOV     A,D
        XRI     XMARK + OMARK   ; the other mark (the bits do not overlap)
        MOV     D,A
        CALL    NEGA
        CMA
        INR     A               ; the reply's score, negated
        POP     D
        POP     B
        POP     H
NUNDO:  MVI     M,0             ; take it back
        SUB     B               ; score - alpha: no overflow within -40..40
        JM      NNEXT
        JZ      NNEXT           ; not above alpha
        ADD     B
        MOV     B,A             ; alpha = score
        LDA     FREE
        CMP     E
        JNZ     NCUT
        SHLD    BESTP           ; at the root: the best move so far
NCUT:   MOV     A,B
        SUB     C
        JP      NRET            ; alpha at or above beta: cut off
NNEXT:  INX     H
        MOV     A,L
        CPI     (BOARD + 9) & 0FFH
        JNZ     NLOOP
NRET:   MOV     A,B
        RET

; WINS: CY set if mark D holds a line: the line's three squares sum to 3 * D.
; Keeps BC, DE and HL.
WINS:   PUSH    H
        PUSH    B
        PUSH    D
        MOV     A,D
        ADD     A
        ADD     D
        MOV     E,A             ; 3 * D
        LXI     B,LINES
        MVI     H,BOARD >> 8
WLINE:  LDAX    B
        ORA     A
        JZ      WDONE           ; the end of the table, CY clear
        MOV     L,A
        MOV     D,M
        INX     B
        LDAX    B
        MOV     L,A
        MOV     A,M
        ADD     D
        MOV     D,A
        INX     B
        LDAX    B
        MOV     L,A
        MOV     A,M
        ADD     D
        INX     B
        CMP     E
        JNZ     WLINE
        STC
WDONE:  POP     D
        POP     B
        POP     H
        RET

; SHOW: print the board, three squares a line: X, O, or the square's digit.
SHOW:   LXI     H,BOARD
        MVI     C,'1'
SROW:   MVI     B,3
SSQ:    MOV     A,M
        ORA     A
        MOV     A,C
        JZ      SPUT            ; free: its digit
        MOV     A,M
        CPI     XMARK
        MVI     A,'X'
        JZ      SPUT
        MVI     A,'O'
SPUT:   OUT     00H
        INX     H
        INR     C
        DCR     B
        JZ      SEOL
        MVI     A,' '
        OUT     00H
        JMP     SSQ
SEOL:   PUSH    H
        LXI     H,CRLF
        CALL    PUTS
        POP     H
        MOV     A,C
        CPI     '1' + 9
        JNZ     SROW
        RET

; GETKEY: wait for a key other than CR or LF, echo it (printable ones only) and
; CR LF; return it in A.
GETKEY: IN      02H
        ANI     01H
        JZ      GETKEY
        IN      01H
        CPI     0DH
        JZ      GETKEY
        CPI     0AH
        JZ      GETKEY
        CPI     ' '
        JC      GKNL
        CPI     7FH
        JNC     GKNL
        OUT     00H
GKNL:   PUSH    PSW
        LXI     H,CRLF
        CALL    PUTS
        POP     PSW
        RET

; PUTS: print the NUL-terminated string at HL.
PUTS:   MOV     A,M
        ORA     A
        RZ
        OUT     00H
        INX     H
        JMP     PUTS

SQ      EQU     BOARD & 0FFH    ; a line is three squares' low address bytes
LINES:  DB      SQ+0,SQ+1,SQ+2, SQ+3,SQ+4,SQ+5, SQ+6,SQ+7,SQ+8 ; rows
        DB      SQ+0,SQ+3,SQ+6, SQ+1,SQ+4,SQ+7, SQ+2,SQ+5,SQ+8 ; columns
        DB      SQ+0,SQ+4,SQ+8, SQ+2,SQ+4,SQ+6, 0             ; diagonals

MSGHI:  DB      "You are X. Type 1-9.",0DH,0AH,0
PROMPT: DB      "> ",0
MSGBAD: DB      "Bad move",0DH,0AH,0
MSGME:  DB      "I play ",0
MSGYOU: DB      "You win",0DH,0AH,0
MSGIW:  DB      "I win",0DH,0AH,0
MSGDRW: DB      "Draw",0DH,0AH,0
MSGAGN: DB      "Again? (Y/N)",0DH,0AH,"> ",0
CRLF:   DB      0DH,0AH,0

        END     START
