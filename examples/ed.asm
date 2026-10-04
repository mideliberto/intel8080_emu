; ed.asm - a line editor in the spirit of ed, with files on the storage device.
; Load: paste ed.hex. Run: G 0100. It prompts with "*". Commands:
;   a           append lines after the last one, until a line that is only "."
;   i n         insert lines before line n (n = last line + 1 appends), until "."
;   d n         delete line n
;   p           print every line, each as its decimal number, a space, the text
;   c           clear the buffer
;   w NAME      write the buffer to file NAME; prints the byte count
;   r NAME      read file NAME after the last line; prints the bytes it added
;   q           return to the monitor
; Command letters fold to lowercase. An empty line prompts again. Anything
; wrong prints "?" (ed's one message): an unknown command, a missing, extra or
; out-of-range argument, a bad file name, a storage error, a full buffer.
;
; Lines are typed as at the monitor (MONITOR_SPEC 2): echoed here, BS or DEL
; erases, 79 characters at most, other control bytes ignored. CR or LF ends a
; line, but an LF right after a CR is skipped, so a CR LF terminal adds no blank
; lines (the monitor's own CR before G counts: the LF of its CR LF is skipped).
;
; The buffer is 0500-CFFF (BUF to LIMIT), clear of the RAM test build's image at
; D000, as lines of text that each end in LF (0AH). A typed line that does not
; fit prints "?" and is dropped; input mode goes on until ".".
;
; Files: w writes the buffer from storage address 0, then one 1AH end marker
; (CP/M's ^Z), flushes, checks that the file is still mounted, and unmounts.
; The storage device cannot shorten a file (DEVICE_SPECS 6), so a shorter text
; written over a longer file leaves the old tail behind the marker; r stops at
; the marker or at EOF, whichever comes first. A file whose last line has no LF
; gets one. A file that does not fit adds the lines that do, then prints "?";
; anything w wrote fits back into an empty buffer. A storage error during r
; also keeps the whole lines read before it, drops the line it cut, and
; prints "?" (DEVICE_SPECS 6: the failed read returns FF, then the file is
; unmounted).
; A mount creates a missing file empty (DEVICE_SPECS 7: there is no "not
; found"), so r of a missing name reads nothing, prints "?" and leaves an empty
; file of that name. After w and r nothing is mounted.
;
; Stresses: the storage and mount protocol (DEVICE_SPECS 6, 7) driven by a
; program instead of the ROM: the resync query, the name bytes, mount status,
; auto-increment writes past EOF, flush, status bits 0 and 7, unmount, and
; files written over longer files; console input polled through IN 02 / IN 01
; with a FIFO full of type-ahead; 16-bit pointer arithmetic, block moves up
; and down (insert, delete), DAD carry for the decimal parser's overflow check,
; decimal output by subtraction, and an error exit that resets SP.

        CPU     8080
        ORG     0100H

BUF     EQU     0500H           ; text buffer, BUF to LIMIT-1
LIMIT   EQU     0D000H          ; a page boundary: the checks compare high bytes
EOFCH   EQU     1AH             ; end-of-text marker in a file
LF      EQU     0AH

START:  LXI     H,0
        DAD     SP
        SHLD    SAVSP           ; ERR resets SP to this
        LXI     H,BUF
        SHLD    ENDP
        MVI     A,0DH
        STA     PREV            ; the monitor's line ended in CR

CMD:    MVI     A,'*'
        OUT     00H
        CALL    GETLN
        CALL    SKIPSP
        MOV     A,M
        ORA     A
        JZ      CMD             ; empty line: prompt again
        ORI     20H             ; fold A-Z to a-z
        INX     H
        CPI     'a'
        JZ      APPEND
        CPI     'i'
        JZ      INSERT
        CPI     'd'
        JZ      DELETE
        CPI     'p'
        JZ      PRINT
        CPI     'c'
        JZ      CLEAR
        CPI     'w'
        JZ      WRITE
        CPI     'r'
        JZ      READ
        CPI     'q'
        JNZ     ERR
        CALL    ENDARG
        RET                     ; SP is SAVSP: back to the monitor

; ERR: print "?" and take the next command, from any call depth.
ERR:    LHLD    SAVSP
        SPHL
        CALL    QMARK
        JMP     CMD

QMARK:  MVI     A,'?'
        OUT     00H
CRLF:   MVI     A,0DH
        OUT     00H
        MVI     A,LF
        OUT     00H
        RET

CLEAR:  CALL    ENDARG
        LXI     H,BUF
        SHLD    ENDP
        JMP     CMD

APPEND: CALL    ENDARG
        LHLD    ENDP
        JMP     INS

INSERT: CALL    NUM
        CALL    FIND

; INS: input mode; each line goes in at HL, and the text from HL up moves up.
INS:    SHLD    INSP
INSLP:  CALL    GETLN           ; HL = LINE
        MOV     A,M
        CPI     '.'
        JNZ     INSLEN
        INX     H
        MOV     A,M
        DCX     H
        ORA     A
        JZ      CMD             ; "." alone ends input mode
INSLEN: MVI     C,0FFH
LENLP:  INR     C               ; C = the line's length, 0-79
        MOV     A,M
        INX     H
        ORA     A
        JNZ     LENLP
        LHLD    ENDP
        MVI     B,0
        DAD     B               ; the new end - 1
        MOV     A,H
        CPI     LIMIT >> 8
        JC      ROOM
        CALL    QMARK           ; no room: drop the line
        JMP     INSLP
ROOM:   INX     H               ; HL = the new end
        XCHG                    ; DE = destination + 1
        LHLD    ENDP            ; HL = source + 1
        PUSH    D
MOVUP:  LDA     INSP            ; down to the insertion point
        CMP     L
        JNZ     MOVUP1
        LDA     INSP+1
        CMP     H
        JZ      MOVED
MOVUP1: DCX     H
        DCX     D
        MOV     A,M
        STAX    D
        JMP     MOVUP
MOVED:  XTHL
        SHLD    ENDP
        POP     H               ; HL = the insertion point
        LXI     D,LINE
CPYLN:  LDAX    D
        ORA     A
        JZ      CPYEND
        MOV     M,A
        INX     H
        INX     D
        JMP     CPYLN
CPYEND: MVI     M,LF
        INX     H
        JMP     INS             ; the next line goes after this one

DELETE: CALL    NUM
        CALL    FIND
        CALL    ATEND
        JZ      ERR             ; n = last line + 1
        MOV     D,H
        MOV     E,L             ; DE = the line
DELSK:  MOV     A,M
        INX     H
        CPI     LF
        JNZ     DELSK           ; HL = the next line
DELMV:  CALL    ATEND
        JZ      DELEND
        MOV     A,M
        STAX    D
        INX     H
        INX     D
        JMP     DELMV
DELEND: XCHG
        SHLD    ENDP
        JMP     CMD

PRINT:  CALL    ENDARG
        LXI     H,BUF
        LXI     D,1             ; line number
PRLP:   CALL    ATEND
        JZ      CMD
        PUSH    D
        PUSH    H
        XCHG
        CALL    PRDEC
        MVI     A,' '
        OUT     00H
        POP     H
PRCH:   MOV     A,M
        INX     H
        CPI     LF
        JZ      PREOL
        OUT     00H
        JMP     PRCH
PREOL:  CALL    CRLF
        POP     D
        INX     D
        JMP     PRLP

WRITE:  CALL    MOUNT
        LXI     H,BUF
WRLP:   CALL    ATEND
        JZ      WRDONE
        MOV     A,M
        OUT     0BH
        INX     H
        JMP     WRLP
WRDONE: MVI     A,EOFCH
        OUT     0BH
        MVI     A,02H
        OUT     0CH             ; flush
        CALL    UNMNT
        LHLD    ENDP
        LXI     D,-BUF
        DAD     D               ; the byte count, without the marker
        JMP     COUNT

READ:   CALL    MOUNT
        LHLD    ENDP
        PUSH    H               ; the old end
RDLP:   IN      0CH
        RLC                     ; CY = bit 7, EOF
        JC      RDEND
        IN      0BH
        CPI     EOFCH
        JZ      RDEND
        MOV     B,A
        MOV     A,H
        CPI     LIMIT >> 8      ; room for this byte
        JNC     RDFULL
        MOV     M,B
        INX     H
        MOV     A,B
        CPI     LF
        JNZ     RDLP
        SHLD    ENDP            ; a whole line: keep it
        JMP     RDLP
RDFULL: CALL    UNMNT           ; the lines read so far stay
        JMP     ERR
RDEND:  CALL    ATEND
        JZ      RDDONE
        IN      0CH             ; a line with no LF yet: was it cut by an error?
        RRC                     ; CY = bit 0, mounted
        JNC     RDFULL          ; unmounted: drop it (UNMNT prints "?")
        MOV     A,H
        CPI     LIMIT >> 8      ; room for its LF
        JNC     RDFULL
        MVI     M,LF            ; the last line had no LF
        INX     H
        SHLD    ENDP
RDDONE: CALL    UNMNT
        LHLD    ENDP
        POP     D
        MOV     A,L
        SUB     E
        MOV     L,A
        MOV     A,H
        SBB     D
        MOV     H,A             ; the bytes added
        ORA     L
        JZ      ERR             ; nothing: an empty or a missing file
COUNT:  CALL    PRDEC
        CALL    CRLF
        JMP     CMD

; MOUNT: mount the name at HL (after spaces, to the end of the line), as the
; DEVICE_SPECS 7 reference client does. ERR unless the status is 00.
MOUNT:  CALL    SKIPSP
        MVI     A,03H
        OUT     0EH             ; query: clears a stale name (resync)
MNTLP:  MOV     A,M
        ORA     A
        JZ      MNTGO
        OUT     0DH
        INX     H
        JMP     MNTLP
MNTGO:  MVI     A,01H
        OUT     0EH
        IN      0FH             ; 00 mounted, 01 open failed, 02 bad name
        ORA     A
        JNZ     ERR
        RET

; UNMNT: unmount; ERR if the file was no longer mounted (a storage error
; unmounts it, DEVICE_SPECS 6).
UNMNT:  IN      0CH
        MOV     B,A
        MVI     A,02H
        OUT     0EH
        MOV     A,B
        RRC                     ; CY = bit 0, mounted
        JNC     ERR
        RET

; NUM: DE = the decimal number at HL, which must end the line. ERR on no
; digit, on anything after it, and above 65535.
NUM:    CALL    SKIPSP
        LXI     D,0
        MOV     A,M
        CALL    DIGIT
        JC      ERR
NUMLP:  MOV     A,M
        CALL    DIGIT
        JC      ENDARG
        PUSH    H
        MOV     H,D
        MOV     L,E
        DAD     H               ; 2n
        JC      ERR
        MOV     D,H
        MOV     E,L
        DAD     H               ; 4n
        JC      ERR
        DAD     H               ; 8n
        JC      ERR
        DAD     D               ; 10n
        JC      ERR
        MOV     E,A
        MVI     D,0
        DAD     D               ; 10n + digit
        JC      ERR
        XCHG
        POP     H
        INX     H
        JMP     NUMLP

; DIGIT: A = the value of the digit in A; CY set if it is not 0-9.
DIGIT:  SUI     '0'
        CPI     10
        CMC
        RET

; ENDARG: ERR unless only spaces are left at HL.
ENDARG: CALL    SKIPSP
        MOV     A,M
        ORA     A
        RZ
        JMP     ERR

SKIPSP: MOV     A,M
        CPI     ' '
        RNZ
        INX     H
        JMP     SKIPSP

; FIND: HL = the start of line DE (the end of the text for the last line + 1).
; ERR for 0 and above that.
FIND:   MOV     A,D
        ORA     E
        JZ      ERR
        LXI     H,BUF
FINDLP: DCX     D
        MOV     A,D
        ORA     E
        RZ
        CALL    ATEND
        JZ      ERR
SKIPLN: MOV     A,M
        INX     H
        CPI     LF
        JNZ     SKIPLN
        JMP     FINDLP

; ATEND: Z if HL = ENDP. Changes A only.
ATEND:  LDA     ENDP
        CMP     L
        RNZ
        LDA     ENDP+1
        CMP     H
        RET

; PRDEC: print HL in decimal, no leading zeros.
PRDEC:  MVI     B,0             ; 1 once a digit is printed
        LXI     D,-10000
        CALL    PRDIG
        LXI     D,-1000
        CALL    PRDIG
        LXI     D,-100
        CALL    PRDIG
        LXI     D,-10
        CALL    PRDIG
        MOV     A,L
        ADI     '0'
        OUT     00H
        RET
PRDIG:  MVI     C,'0'-1
PRDLP:  INR     C
        DAD     D               ; CY while HL was at least the power of ten
        JC      PRDLP
        MOV     A,L             ; undo the last subtraction
        SUB     E
        MOV     L,A
        MOV     A,H
        SBB     D
        MOV     H,A
        MOV     A,B
        ORA     A
        MOV     A,C
        JNZ     PRDOUT
        CPI     '0'
        RZ                      ; a leading zero
PRDOUT: OUT     00H
        MVI     B,1
        RET

; GETLN: read a line into LINE, NUL-terminated (MONITOR_SPEC 2, but an LF
; right after a CR is skipped). Returns HL = LINE.
GETLN:  LXI     H,LINE
        MVI     C,0             ; characters stored
GETC:   IN      02H
        RRC
        JNC     GETC
        IN      01H
        MOV     B,A
        LDA     PREV
        MOV     D,A
        MOV     A,B
        STA     PREV
        CPI     0DH
        JZ      GOTLN
        CPI     LF
        JNZ     GETC1
        MOV     A,D
        CPI     0DH
        JZ      GETC            ; the LF of a CR LF
        JMP     GOTLN
GETC1:  CPI     08H
        JZ      BACK
        CPI     7FH
        JZ      BACK
        CPI     ' '
        JC      GETC            ; other control bytes
        MOV     A,C
        CPI     79
        JNC     GETC            ; full: discarded
        MOV     M,B
        INX     H
        INR     C
        MOV     A,B
        OUT     00H
        JMP     GETC
BACK:   MOV     A,C
        ORA     A
        JZ      GETC
        DCX     H
        DCR     C
        MVI     A,08H
        OUT     00H
        MVI     A,' '
        OUT     00H
        MVI     A,08H
        OUT     00H
        JMP     GETC
GOTLN:  MVI     M,0
        CALL    CRLF
        LXI     H,LINE
        RET

SAVSP:  DS      2               ; SP at entry
ENDP:   DS      2               ; the end of the text
INSP:   DS      2               ; input mode: where the next line goes
PREV:   DS      1               ; the last byte read
LINE:   DS      80

        IF      $ > BUF
        ERROR   "the program runs into BUF"
        ENDIF

        END     START
