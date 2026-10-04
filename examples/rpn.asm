; rpn.asm - an RPN calculator on signed 16-digit packed BCD numbers.
; Load: paste rpn.hex. Run: G 0100. It prints "RPN calculator" and prompts
; "> " for a line of tokens, read and echoed here (Backspace and Delete erase,
; CR ends the line, other control bytes are ignored, 79 characters at most).
;
; Tokens, spaces optional between them, case ignored:
;   digits  push a number (unsigned literal, leading zeros allowed)
;   + - * / Y op X: pop X and Y, push the result. / is integer division,
;           truncated toward zero: 0 17 - 5 / gives -3
;   p       print the top        s  print the stack, bottom first, one per line
;   c       clear the stack      q  return to the monitor
; The stack holds 4 numbers. Values are sign and magnitude, at most 16 digits:
; a result or literal above 9999999999999999 is an error. Errors print a
; message and drop the rest of the line; the stack is as before the token:
;   Overflow  Divide by zero  Stack overflow  Stack underflow  Bad input
;
; What it stresses in the emulator: DAA, and the AC and CY flags it reads.
; Every digit of every result goes through ADC + DAA, with carry-in 0 (add)
; and 1 (subtract), so both nibble corrections and the carry out of each byte
; are exercised, 9 bytes in a chain per add. DAA on the 8080 corrects only
; after an addition: there is no subtract flag (the Z80 has one), so DAA after
; SUB or SBB applies the addition correction and gives a wrong digit.
; Subtraction is therefore ten's complement: dst - src = dst + (99..9 - src)
; + 1, the nines complement made with MVI A,99H / SUB M (no borrow between
; nibbles for BCD digits), the + 1 as STC before the ADC chain; CY out set
; means no borrow. Also: RAL through CY across 18 bytes for each digit shift
; (INX and DCR must leave CY alone), DCR's Z inside loops, DAD SP / SPHL for
; the error exit, and console input polled on IN 02H / IN 01H (DEVICE_SPECS 4).
; Arithmetic: shift-and-add multiply and restoring division, one decimal digit
; per step. tests/monitor_tests.rs checks 200 operations against i128.

        CPU     8080
        ORG     0100H

N       EQU     9               ; magnitude bytes, low byte first: 18 digits
ESIZE   EQU     N+1             ; stack entry: sign (0 +, 1 -), magnitude
DEPTHMAX EQU    4               ; stack entries
LINEMAX EQU     79              ; characters in a line

START:  LXI     H,0
        DAD     SP
        SHLD    SAVSP           ; SP with the monitor's return address on top
        XRA     A
        STA     DEPTH
        LXI     H,MSGHI
        CALL    PUTS
PROMPT: LHLD    SAVSP           ; an error jumps here from any call depth
        SPHL
        MVI     A,'>'
        OUT     00H
        MVI     A,' '
        OUT     00H
        CALL    GETLN

; Each token handler returns to NEXT.
NEXT:   LHLD    LINEP
        MOV     A,M
        INX     H
        SHLD    LINEP
        ORA     A
        JZ      PROMPT          ; end of line
        LXI     H,NEXT
        PUSH    H
        CPI     '0'
        JC      NOTDIG
        CPI     '9'+1
        JC      NUMBER
NOTDIG: ORI     20H             ; lower case; space, + - * / unchanged
        CPI     ' '
        RZ
        CPI     '+'
        JZ      ADD
        CPI     '-'
        JZ      SUB
        CPI     '*'
        JZ      MUL
        CPI     '/'
        JZ      DIV
        CPI     'p'
        JZ      PRINT
        CPI     's'
        JZ      SHOW
        CPI     'c'
        JZ      CLEAR
        CPI     'q'
        JZ      QUIT
        LXI     H,MSGBAD
ERROR:  CALL    PUTS            ; HL = the message
        JMP     PROMPT

QUIT:   LHLD    SAVSP
        SPHL
        RET                     ; to the monitor (MONITOR_SPEC 8)

CLEAR:  XRA     A
        STA     DEPTH
        RET

; A literal: shift each digit into NUM, then push it.
NUMBER: XRA     A
        STA     NUM             ; sign +
        LXI     H,NUM+1
        CALL    CLR
        LHLD    LINEP
        DCX     H               ; the first digit
DIGIT:  MOV     A,M
        SUI     '0'
        CPI     10
        JNC     NUMEND
        INX     H
        PUSH    H
        PUSH    PSW
        LXI     H,NUM+1
        MVI     C,N
        CALL    SHL4            ; NUM * 10
        POP     PSW
        LXI     H,NUM+1
        ORA     M
        MOV     M,A             ; + the digit
        LDA     NUM+N
        CALL    OVCHK
        POP     H
        JMP     DIGIT
NUMEND: SHLD    LINEP
        LDA     DEPTH
        CPI     DEPTHMAX
        LXI     H,MSGSOV
        JNC     ERROR
        INR     A
        STA     DEPTH
        DCR     A
        CALL    ENTRY
        XCHG                    ; DE = the new top
        LXI     H,NUM
        MVI     C,ESIZE
        JMP     COPY

PRINT:  LDA     DEPTH
        ORA     A
        LXI     H,MSGSUN
        JZ      ERROR
        DCR     A
        CALL    ENTRY
        JMP     PRNUM

SHOW:   XRA     A
SHOW1:  LXI     H,DEPTH
        CMP     M
        RNC                     ; all printed
        PUSH    PSW
        CALL    ENTRY
        CALL    PRNUM
        POP     PSW
        INR     A
        JMP     SHOW1

; + and -: OPA = Y, OPB = X. Same signs add the magnitudes; different signs
; subtract the smaller from the larger and take the larger one's sign.
SUB:    CALL    BINOP
        LDA     OPB
        XRI     1               ; Y - X = Y + (-X)
        STA     OPB
        JMP     ADDOPS
ADD:    CALL    BINOP
ADDOPS: LDA     OPA
        LXI     H,OPB
        XRA     M
        JNZ     DIFF
        LXI     H,OPA+1
        LXI     D,OPB+1
        CALL    BCDADD
        LDA     OPA+N
        CALL    OVCHK
        LXI     H,OPA
        JMP     DONE
DIFF:   LXI     H,OPA+1
        LXI     D,OPB+1
        CALL    BCDSUB
        LXI     H,OPA
        JC      DONE            ; |Y| >= |X|: Y's sign
        LXI     H,OPA+1
        LXI     D,OPB+1
        CALL    BCDADD          ; undo
        LXI     H,OPB+1
        LXI     D,OPA+1
        CALL    BCDSUB          ; |X| - |Y|
        LXI     H,OPB           ; X's sign
        JMP     DONE

; *: for each digit of Y, high first (shifted out of OPA into EXT):
; RES = RES * 10 + digit * |X|. RES only grows, so the check after each
; digit catches every overflow before RES outgrows its 18 digits.
MUL:    CALL    BINOP
        LXI     H,RES+1
        CALL    CLR
        LXI     H,EXT
        CALL    CLR
        MVI     B,2*N
MUL1:   PUSH    B
        LXI     H,OPA+1
        MVI     C,2*N
        CALL    SHL4            ; the next digit of Y into EXT
        LXI     H,RES+1
        MVI     C,N
        CALL    SHL4            ; RES * 10
        LDA     EXT
        ANI     0FH
MUL2:   JZ      MUL3
        PUSH    PSW
        LXI     H,RES+1
        LXI     D,OPB+1
        CALL    BCDADD
        POP     PSW
        DCR     A
        JMP     MUL2
MUL3:   LDA     RES+N
        CALL    OVCHK
        POP     B
        DCR     B
        JNZ     MUL1
        LDA     OPA
        LXI     H,OPB
        XRA     M
        STA     RES
        LXI     H,RES
        JMP     DONE

; /: restoring division. Each step shifts the next digit of Y from OPA into
; the remainder EXT (OPA's magnitude and EXT are one 18-byte register) and
; subtracts |X| until it borrows; the count is the quotient digit, which goes
; into the low nibble the shift emptied. EXT < 10 * |X| fits in 18 digits.
DIV:    CALL    BINOP
        LXI     H,OPB+1
        CALL    ISZERO
        LXI     H,MSGDIV
        JZ      ERROR
        LXI     H,EXT
        CALL    CLR
        MVI     B,2*N
DIV1:   PUSH    B
        LXI     H,OPA+1
        MVI     C,2*N
        CALL    SHL4
        MVI     C,0             ; the quotient digit
DIV2:   LXI     H,EXT
        LXI     D,OPB+1
        CALL    BCDSUB
        JNC     DIV3            ; borrowed
        INR     C
        JMP     DIV2
DIV3:   LXI     H,EXT
        LXI     D,OPB+1
        CALL    BCDADD          ; restore
        LDA     OPA+1
        ORA     C
        STA     OPA+1
        POP     B
        DCR     B
        JNZ     DIV1
        LDA     OPA
        LXI     H,OPB
        XRA     M
        STA     OPA
        LXI     H,OPA
        JMP     DONE

; BINOP: Y and X to OPA and OPB, or Stack underflow.
BINOP:  LDA     DEPTH
        CPI     2
        LXI     H,MSGSUN
        JC      ERROR
        SUI     2
        CALL    ENTRY           ; HL = Y; X follows it
        LXI     D,OPA
        MVI     C,ESIZE
        CALL    COPY
        LXI     D,OPB
        MVI     C,ESIZE
        JMP     COPY

; DONE: the result at HL replaces Y and X. Zero is always +.
DONE:   PUSH    H
        INX     H
        CALL    ISZERO
        POP     H
        JNZ     DONE1
        MVI     M,0
DONE1:  LDA     DEPTH
        DCR     A
        STA     DEPTH
        DCR     A
        PUSH    H
        CALL    ENTRY
        XCHG                    ; DE = the new top
        POP     H
        MVI     C,ESIZE
        JMP     COPY

; OVCHK: A = a magnitude's top byte; nonzero means over 16 digits.
OVCHK:  ORA     A
        RZ
        LXI     H,MSGOVF
        JMP     ERROR

; BCDADD: (HL) += (DE), N bytes. ADDC adds CY too. Returns CY = carry out.
; Uses A, B, DE, HL; keeps C.
BCDADD: ORA     A
ADDC:   MVI     B,N
ADD1:   LDAX    D
        ADC     M
        DAA
        MOV     M,A
        INX     H
        INX     D
        DCR     B               ; leaves CY
        JNZ     ADD1
        RET

; BCDSUB: (HL) -= (DE), N bytes, as (HL) + nines complement of (DE) + 1.
; Returns CY set when there was no borrow, (HL) >= (DE). Keeps C.
BCDSUB: PUSH    H
        XCHG
        LXI     D,TMP
        MVI     B,N
SUB1:   MVI     A,99H
        SUB     M
        STAX    D
        INX     H
        INX     D
        DCR     B
        JNZ     SUB1
        POP     H
        LXI     D,TMP
        STC
        JMP     ADDC

; SHL4: shift C bytes at HL (low byte first) left one digit. Uses A, B.
SHL4:   MVI     B,4
SHL1:   PUSH    H
        PUSH    B
        ORA     A               ; CY = 0 into the low bit
SHL2:   MOV     A,M
        RAL
        MOV     M,A
        INX     H
        DCR     C
        JNZ     SHL2
        POP     B
        POP     H
        DCR     B
        JNZ     SHL1
        RET

; CLR: zero N bytes at HL. Uses B.
CLR:    MVI     B,N
CLR1:   MVI     M,0
        INX     H
        DCR     B
        JNZ     CLR1
        RET

; ISZERO: Z set when the N bytes at HL are all zero. Uses A, C.
ISZERO: MVI     C,N
        XRA     A
ISZ1:   ORA     M
        INX     H
        DCR     C
        JNZ     ISZ1
        ORA     A
        RET

; ENTRY: HL = stack entry A (0 = bottom). Uses A, DE.
ENTRY:  LXI     H,STK
        LXI     D,ESIZE
        INR     A
ENT1:   DCR     A
        RZ
        DAD     D
        JMP     ENT1

; COPY: C bytes from HL to DE.
COPY:   MOV     A,M
        STAX    D
        INX     H
        INX     D
        DCR     C
        JNZ     COPY
        RET

; PRNUM: print the entry at HL: sign, digits without leading zeros, CR LF.
PRNUM:  MOV     A,M
        ORA     A
        JZ      PRN1
        MVI     A,'-'
        OUT     00H
PRN1:   LXI     D,N
        DAD     D               ; the top byte
        MVI     B,N
        MVI     C,0             ; nonzero once a digit is printed
PRN2:   MOV     A,M
        RRC
        RRC
        RRC
        RRC
        CALL    PRDIG
        MOV     A,M
        CALL    PRDIG
        DCX     H
        DCR     B
        JNZ     PRN2
        MOV     A,C
        ORA     A
        JNZ     CRLF
        MVI     A,'0'           ; the value 0
        OUT     00H
        JMP     CRLF

PRDIG:  ANI     0FH
        MOV     E,A
        ORA     C
        RZ                      ; a leading zero
        MOV     C,A
        MOV     A,E
        ORI     '0'
        OUT     00H
        RET

; GETLN: read a line into LINE with echo, NUL-terminate it, LINEP = LINE.
GETLN:  LXI     H,LINE
        MVI     B,0             ; characters stored
GL1:    IN      02H
        ANI     01H
        JZ      GL1
        IN      01H
        CPI     0DH
        JZ      GLEND
        CPI     08H
        JZ      GLBS
        CPI     7FH
        JNC     GLDEL           ; DEL, or 80-FF
        CPI     ' '
        JC      GL1             ; other control bytes
        MOV     C,A
        MOV     A,B
        CPI     LINEMAX
        JNC     GL1             ; full: discard
        MOV     A,C
        MOV     M,A
        INX     H
        INR     B
        OUT     00H
        JMP     GL1
GLDEL:  JNZ     GL1             ; 80-FF ignored
GLBS:   MOV     A,B
        ORA     A
        JZ      GL1             ; nothing to erase
        DCX     H
        DCR     B
        MVI     A,08H
        OUT     00H
        MVI     A,' '
        OUT     00H
        MVI     A,08H
        OUT     00H
        JMP     GL1
GLEND:  MVI     M,0
        LXI     H,LINE
        SHLD    LINEP
CRLF:   MVI     A,0DH
        OUT     00H
        MVI     A,0AH
        OUT     00H
        RET

; PUTS: print the NUL-terminated string at HL.
PUTS:   MOV     A,M
        ORA     A
        RZ
        OUT     00H
        INX     H
        JMP     PUTS

MSGHI:  DB      "RPN calculator",0DH,0AH,0
MSGOVF: DB      "Overflow",0DH,0AH,0
MSGDIV: DB      "Divide by zero",0DH,0AH,0
MSGSOV: DB      "Stack overflow",0DH,0AH,0
MSGSUN: DB      "Stack underflow",0DH,0AH,0
MSGBAD: DB      "Bad input",0DH,0AH,0

; Variables: not in the .hex; START sets DEPTH, everything else is written
; before it is read.
SAVSP:  DS      2
DEPTH:  DS      1
LINEP:  DS      2
STK:    DS      DEPTHMAX*ESIZE
OPB:    DS      ESIZE           ; X
OPA:    DS      ESIZE           ; Y
EXT:    DS      N               ; follows OPA's magnitude (MUL, DIV)
RES:    DS      ESIZE
TMP:    DS      N
NUM:    DS      ESIZE
LINE:   DS      LINEMAX+1

        END     START
