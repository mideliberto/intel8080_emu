; monitor.asm - Intel 8080 Monitor ROM
; Assemble with: make
; 
; Memory Map:
;   0x0000-0x00FF: System workspace (RAM)
;   0x0100-0xEFFF: User program area (RAM)
;   0xF000-0xFFFF: Monitor ROM (this file)
;
; I/O Ports:
;   0x00: Console data (read/write)
;   0x01: Console status (bit 0 = input ready, bit 1 = output ready)

        CPU     8080
        ORG     0F000H

; ============================================
; CONSTANTS
; ============================================

CONSOLE_DATA    EQU     00H
CONSOLE_STATUS  EQU     01H

STACK_TOP       EQU     0F000H      ; Stack grows down from ROM

CR              EQU     0DH
LF              EQU     0AH
BS              EQU     08H
SPACE           EQU     20H

; ============================================
; WORKSPACE (RAM at 0x0080-0x00FF)
; ============================================

LINE_BUFFER     EQU     0080H       ; 80 bytes for command line
LINE_LENGTH     EQU     50          ; Max 80 chars
BUFFER_PTR      EQU     00D0H       ; Current position in buffer (2 bytes)
LAST_DUMP_ADDR  EQU     00D2H       ; Last dump address (2 bytes)

; ============================================
; COLD START
; ============================================

COLD_START:
        LXI     SP,STACK_TOP        ; Initialize stack pointer
        
        ; Initialize workspace
        LXI     H,0000H
        SHLD    LAST_DUMP_ADDR      ; Default dump address = 0
        
        CALL    PRINT_BANNER        ; Show startup message
        
; ============================================
; MAIN LOOP
; ============================================

MAIN_LOOP:
        MVI     A,'>'               ; Print prompt
        CALL    CONOUT
        MVI     A,' '
        CALL    CONOUT
        
        MVI     A,'A'               ; DEBUG: before READ_LINE
        CALL    CONOUT
        
        CALL    READ_LINE           ; Read command into LINE_BUFFER
        
        MVI     A,'B'               ; DEBUG: after READ_LINE
        CALL    CONOUT
        HLT

RL_DEBUG:
        MVI     A,'X'
        CALL    CONOUT

                ; DEBUG: dump first 4 bytes of buffer
        LXI     H,LINE_BUFFER
        MOV     A,M
        CALL    PRINT_HEX_BYTE
        INX     H
        MOV     A,M
        CALL    PRINT_HEX_BYTE
        INX     H
        MOV     A,M
        CALL    PRINT_HEX_BYTE
        INX     H
        MOV     A,M
        CALL    PRINT_HEX_BYTE
        CALL    PRINT_CRLF
        
        LXI     H,LINE_BUFFER       ; Point to start of buffer
        CALL    SKIP_SPACES         ; Skip leading spaces
        
        MOV     A,M                 ; Get command character
        ORA     A                   ; Empty line?
        JZ      MAIN_LOOP           ; Yes, just prompt again
        
        ; Convert to uppercase if lowercase
        CPI     'a'
        JC      NOT_LOWER
        CPI     'z'+1
        JNC     NOT_LOWER
        SUI     20H                 ; Convert to uppercase
NOT_LOWER:
        
        INX     H                   ; Point past command char
        
        ; Command dispatch
        CPI     'D'
        JZ      CMD_DUMP
        CPI     'E'
        JZ      CMD_EXAMINE
        CPI     'G'
        JZ      CMD_GO
        CPI     '?'
        JZ      CMD_HELP
        
        ; Unknown command
        LXI     H,MSG_UNKNOWN
        CALL    PRINT_STRING
        JMP     MAIN_LOOP

; ============================================
; CONSOLE I/O ROUTINES
; ============================================

; CONOUT - Output character in A
; Preserves: all registers
CONOUT:
        PUSH    PSW
CONOUT_WAIT:
        IN      CONSOLE_STATUS
        ANI     02H                 ; Check output ready bit
        JZ      CONOUT_WAIT
        POP     PSW
        OUT     CONSOLE_DATA
        RET

; CONIN - Input character to A
; Trashes: flags
CONIN:
        IN      CONSOLE_STATUS
        ANI     01H                 ; Check input ready bit
        JZ      CONIN
        IN      CONSOLE_DATA
        RET

; CONST - Console status
; Returns: A = 0xFF if char available, 0x00 if not
; Trashes: flags
CONST:
        IN      CONSOLE_STATUS
        ANI     01H
        RZ                          ; Return 0 if no char
        MVI     A,0FFH              ; Return FF if char available
        RET

; ============================================
; PRINT ROUTINES
; ============================================

; PRINT_BANNER - Display startup message
PRINT_BANNER:
        LXI     H,MSG_BANNER
        CALL    PRINT_STRING
        RET

; PRINT_STRING - Print null-terminated string at HL
; Trashes: A, HL, flags
PRINT_STRING:
        MOV     A,M                 ; Get character
        ORA     A                   ; Check for null
        RZ                          ; Return if end of string
        CALL    CONOUT
        INX     H
        JMP     PRINT_STRING

; PRINT_CRLF - Print carriage return and line feed
; Trashes: A, flags
PRINT_CRLF:
        MVI     A,CR
        CALL    CONOUT
        MVI     A,LF
        CALL    CONOUT
        RET

; PRINT_SPACE - Print a space
; Trashes: A, flags
PRINT_SPACE:
        MVI     A,SPACE
        CALL    CONOUT
        RET

; PRINT_HEX_BYTE - Print A as two hex digits
; Input: A = byte to print
; Trashes: A, flags
PRINT_HEX_BYTE:
        PUSH    PSW                 ; Save original byte
        RRC                         ; Shift high nibble to low
        RRC
        RRC
        RRC
        CALL    PRINT_HEX_NIBBLE    ; Print high nibble
        POP     PSW                 ; Restore original
        CALL    PRINT_HEX_NIBBLE    ; Print low nibble
        RET

; PRINT_HEX_NIBBLE - Print low nibble of A as hex digit
; Input: A = value (low 4 bits used)
; Trashes: A, flags
PRINT_HEX_NIBBLE:
        ANI     0FH                 ; Mask to low nibble
        CPI     0AH                 ; Is it A-F?
        JC      PHN_DIGIT           ; No, it's 0-9
        ADI     07H                 ; Adjust for A-F
PHN_DIGIT:
        ADI     '0'                 ; Convert to ASCII
        CALL    CONOUT
        RET

; PRINT_HEX_WORD - Print HL as four hex digits
; Input: HL = word to print
; Trashes: A, flags
PRINT_HEX_WORD:
        MOV     A,H
        CALL    PRINT_HEX_BYTE
        MOV     A,L
        CALL    PRINT_HEX_BYTE
        RET

; ============================================
; INPUT ROUTINES
; ============================================

; READ_LINE - Read line into LINE_BUFFER
; Handles: BS (backspace), CR (end of line)
; Returns: LINE_BUFFER contains null-terminated string
; Trashes: A, B, HL, flags
READ_LINE:
        LXI     H,LINE_BUFFER       ; Point to buffer start
        MVI     B,0                 ; Character count
        
RL_LOOP:
        CALL    CONIN               ; Get character
        MOV     C,A                 ; Save it in C
        
        CPI     CR                  ; Enter pressed?
        JZ      RL_DONE
        CPI     LF
        JZ      RL_DONE
        
        CPI     SPACE               ; Ignore control chars
        JC      RL_LOOP
        
        ; Check buffer full
        MOV     A,B
        CPI     LINE_LENGTH-1       ; Room for char + null?
        JNC     RL_LOOP             ; Buffer full, ignore
        
        ; Store and echo character
        MOV     M,C                 ; Store in buffer
        INX     H                   ; Advance pointer
        INR     B                   ; Increment count
        MOV     A,C                 ; Echo character
        CALL    CONOUT
        JMP     RL_LOOP
        
RL_BACKSPACE:
        MOV     A,B                 ; Check if buffer empty
        ORA     A
        JZ      RL_LOOP             ; Nothing to delete
        
        DCX     H                   ; Back up pointer
        DCR     B                   ; Decrement count
        
        ; Erase character on screen: BS, space, BS
        MVI     A,BS
        CALL    CONOUT
        MVI     A,SPACE
        CALL    CONOUT
        MVI     A,BS
        CALL    CONOUT
        JMP     RL_LOOP
        
RL_DONE:
        MVI     M,0                 ; Null terminate
        CALL    PRINT_CRLF          ; Echo newline
        MVI     A,'Z'               ; DEBUG: made it to end
        CALL    CONOUT
        RET

; ============================================
; PARSING ROUTINES
; ============================================

; SKIP_SPACES - Skip spaces in buffer
; Input: HL = pointer into buffer
; Output: HL = pointer to first non-space (or null)
; Trashes: A, flags
SKIP_SPACES:
        MOV     A,M
        CPI     SPACE
        RNZ                         ; Return if not a space
        INX     H
        JMP     SKIP_SPACES

; READ_HEX_WORD - Parse hex number from buffer
; Input: HL = pointer into buffer
; Output: DE = parsed value, HL = advanced past number
;         Carry set if no valid hex digits found
; Trashes: A, BC, flags
READ_HEX_WORD:
        CALL    SKIP_SPACES         ; Skip leading spaces
        LXI     D,0                 ; Initialize result
        MVI     B,0                 ; Digit count
        
RHW_LOOP:
        MOV     A,M                 ; Get character
        CALL    TO_HEX_DIGIT        ; Convert to 0-15
        JC      RHW_DONE            ; Not a hex digit, done
        
        ; Shift DE left 4 bits and add new digit
        ; DE = DE * 16 + A
        PUSH    PSW                 ; Save digit
        
        ; Shift DE left 4 bits
        MOV     A,D
        RLC
        RLC
        RLC
        RLC
        ANI     0F0H
        MOV     D,A
        
        MOV     A,E
        RLC
        RLC
        RLC
        RLC
        MOV     C,A                 ; Save shifted E
        ANI     0F0H
        ORA     D                   ; Combine with D bits
        MOV     D,A
        
        MOV     A,C
        ANI     0FH                 ; Low nibble of shifted E
        MOV     E,A
        
        POP     PSW                 ; Restore digit
        ORA     E                   ; Add digit to low nibble
        MOV     E,A
        
        INX     H                   ; Advance buffer pointer
        INR     B                   ; Count digit
        JMP     RHW_LOOP
        
RHW_DONE:
        MOV     A,B                 ; Check digit count
        ORA     A
        STC                         ; Set carry (no digits)
        RZ                          ; Return with carry if no digits
        ORA     A                   ; Clear carry (success)
        RET

; TO_HEX_DIGIT - Convert ASCII to hex value
; Input: A = ASCII character
; Output: A = hex value (0-15), Carry clear
;         Carry set if not a hex digit
; Trashes: flags
TO_HEX_DIGIT:
        CPI     '0'
        JC      THD_FAIL            ; Below '0'
        CPI     '9'+1
        JC      THD_DIGIT           ; '0'-'9'
        
        CPI     'A'
        JC      THD_FAIL            ; Between '9' and 'A'
        CPI     'F'+1
        JC      THD_ALPHA           ; 'A'-'F'
        
        CPI     'a'
        JC      THD_FAIL            ; Between 'F' and 'a'
        CPI     'f'+1
        JNC     THD_FAIL            ; Above 'f'
        
        ; 'a'-'f': convert to uppercase first
        SUI     20H
        
THD_ALPHA:
        SUI     'A'-10              ; Convert 'A'-'F' to 10-15
        ORA     A                   ; Clear carry
        RET
        
THD_DIGIT:
        SUI     '0'                 ; Convert '0'-'9' to 0-9
        ORA     A                   ; Clear carry
        RET
        
THD_FAIL:
        STC                         ; Set carry = not hex
        RET

; ============================================
; COMMANDS
; ============================================

; CMD_DUMP - Dump memory
; Syntax: D [start] [end]
; If no args, continues from last address
; If one arg, dumps 128 bytes from start
; If two args, dumps from start to end
CMD_DUMP:

        MVI     A,'!'               ; DEBUG: Did we get here?
        CALL    CONOUT
        CALL    PRINT_CRLF

        CALL    SKIP_SPACES
        MOV     A,M
        ORA     A                   ; End of line?
        JZ      CD_NO_ARGS
        
        ; Parse start address
        CALL    READ_HEX_WORD
        JC      CD_ERROR            ; No valid address
        PUSH    D                   ; Save start address
        
        CALL    SKIP_SPACES
        MOV     A,M
        ORA     A                   ; End of line?
        JZ      CD_ONE_ARG
        
        ; Parse end address
        CALL    READ_HEX_WORD
        JC      CD_POP_ERROR        ; Invalid end address
        
        ; Two args: start in stack, end in DE
        POP     H                   ; HL = start
        JMP     CD_DUMP_RANGE
        
CD_NO_ARGS:
        ; Continue from last address
        LHLD    LAST_DUMP_ADDR
        LXI     D,007FH             ; 128 bytes
        DAD     D                   ; HL = start, calculate end
        XCHG                        ; DE = end
        LHLD    LAST_DUMP_ADDR      ; HL = start
        JMP     CD_DUMP_RANGE
        
CD_ONE_ARG:
        POP     H                   ; HL = start
        PUSH    H                   ; Save start again
        LXI     D,007FH
        DAD     D                   ; HL = start + 127 = end
        XCHG                        ; DE = end
        POP     H                   ; HL = start
        JMP     CD_DUMP_RANGE
        
CD_POP_ERROR:
        POP     D                   ; Clean up stack
CD_ERROR:
        LXI     H,MSG_BAD_ADDR
        CALL    PRINT_STRING
        JMP     MAIN_LOOP

; CD_DUMP_RANGE - Dump memory from HL to DE
; Input: HL = start address, DE = end address
CD_DUMP_RANGE:
        PUSH    D                   ; Save end address

CD_LINE:
        ; Print address
        CALL    PRINT_HEX_WORD
        MVI     A,':'
        CALL    CONOUT
        CALL    PRINT_SPACE
        
        ; Print 16 hex bytes
        PUSH    H                   ; Save line start for ASCII
        MVI     C,16                ; Byte counter
        
CD_HEX_BYTE:
        MOV     A,M                 ; Get byte
        CALL    PRINT_HEX_BYTE
        CALL    PRINT_SPACE
        
        ; Extra space after 8th byte
        MOV     A,C
        CPI     9
        JNZ     CD_NO_GAP
        CALL    PRINT_SPACE
CD_NO_GAP:
        
        INX     H
        DCR     C
        JNZ     CD_HEX_BYTE
        
        ; Print ASCII representation
        CALL    PRINT_SPACE
        POP     H                   ; Restore line start
        MVI     C,16
        
CD_ASCII:
        MOV     A,M
        CPI     SPACE               ; Printable? (>= 0x20)
        JC      CD_DOT
        CPI     07FH                ; Printable? (< 0x7F)
        JC      CD_PRINT_CHAR
CD_DOT:
        MVI     A,'.'
CD_PRINT_CHAR:
        CALL    CONOUT
        INX     H
        DCR     C
        JNZ     CD_ASCII
        
        CALL    PRINT_CRLF
        
        ; Check if done (HL > end address)
        POP     D                   ; DE = end address
        PUSH    D                   ; Keep it on stack
        
        ; Compare HL to DE
        MOV     A,H
        CMP     D
        JC      CD_LINE             ; H < D, continue
        JNZ     CD_DONE             ; H > D, done
        MOV     A,L
        CMP     E
        JC      CD_LINE             ; L < E, continue
        JZ      CD_LINE             ; L == E, do one more line... 
                                    ; Actually should stop. Let me think.
        ; If HL > DE, we're done
        ; If HL <= DE, continue
        
CD_DONE:
        POP     D                   ; Clean up stack
        SHLD    LAST_DUMP_ADDR      ; Save for next time
        JMP     MAIN_LOOP

; CMD_EXAMINE - Placeholder
CMD_EXAMINE:
        LXI     H,MSG_NOT_IMPL
        CALL    PRINT_STRING
        JMP     MAIN_LOOP

; CMD_GO - Placeholder
CMD_GO:
        LXI     H,MSG_NOT_IMPL
        CALL    PRINT_STRING
        JMP     MAIN_LOOP

; CMD_HELP - Show help
CMD_HELP:
        LXI     H,MSG_HELP
        CALL    PRINT_STRING
        JMP     MAIN_LOOP

; ============================================
; STRINGS
; ============================================

MSG_BANNER:
        DB      CR,LF
        DB      "8080 Monitor v0.1",CR,LF
        DB      "Ready.",CR,LF
        DB      0

MSG_UNKNOWN:
        DB      "Unknown command. Type ? for help.",CR,LF,0

MSG_BAD_ADDR:
        DB      "Invalid address",CR,LF,0

MSG_NOT_IMPL:
        DB      "Not implemented",CR,LF,0

MSG_HELP:
        DB      "Commands:",CR,LF
        DB      "  D [start] [end]  - Dump memory",CR,LF
        DB      "  E [addr]         - Examine/modify",CR,LF
        DB      "  G [addr]         - Go (execute)",CR,LF
        DB      "  ?                - Help",CR,LF
        DB      0

; ============================================
; PADDING
; ============================================

        IF      $ > 0FFFFH
        ERROR   "ROM exceeds 4KB!"
        ENDIF

        END     COLD_START