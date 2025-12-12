; monitor.asm - Intel 8080 Monitor ROM
; Assemble with: make
; 
; Memory Map:
;   0x0000-0x00FF: System workspace (RAM)
;   0x0100-0xEFFF: User program area (RAM)
;   0xF000-0xFFFF: Monitor ROM (this file)
;
; I/O Ports:
;   0x00: Console data out (write only)
;   0x01: Console data in (read only)
;   0x02: Console status (bit 0 = RX ready, bit 1 = TX ready)

        CPU     8080
        ORG     0F000H

; ============================================
; CONSTANTS
; ============================================

CONSOLE_DATA_OUT    EQU     00H
CONSOLE_DATA_IN     EQU     01H
CONSOLE_STATUS      EQU     02H

STACK_TOP       EQU     0F000H      ; Stack grows down from ROM

CR              EQU     0DH
LF              EQU     0AH
BS              EQU     08H
SPACE           EQU     20H

; ============================================
; WORKSPACE (RAM at 0x0080-0x00FF)
; ============================================

LINE_BUFFER     EQU     0080H       ; 80 bytes for command line
LINE_LENGTH     EQU     80          ; Max 80 chars
BUFFER_PTR      EQU     00D0H       ; Current position in buffer (2 bytes)
LAST_DUMP_ADDR  EQU     00D2H       ; Last dump address (2 bytes)
LAST_EXAM_ADDR  EQU     00D4H       ; Last examine address (2 bytes)

; I/O stubs (self-modifying code)
IO_IN_STUB      EQU     00D6H       ; 3 bytes: IN xx / RET
IO_OUT_STUB     EQU     00D9H       ; 3 bytes: OUT xx / RET

; ============================================
; COLD START
; ============================================

COLD_START:
        LXI     SP,STACK_TOP        ; Initialize stack pointer
        
        ; Initialize workspace
        LXI     H,0000H
        SHLD    LAST_DUMP_ADDR      ; Default dump address = 0
        SHLD    LAST_EXAM_ADDR      ; Default exam address = 0
        
        ; Initialize I/O stubs
        MVI     A,0DBH              ; IN opcode
        STA     IO_IN_STUB
        MVI     A,00H               ; Default port 0
        STA     IO_IN_STUB+1
        MVI     A,0C9H              ; RET opcode
        STA     IO_IN_STUB+2
        
        MVI     A,0D3H              ; OUT opcode
        STA     IO_OUT_STUB
        MVI     A,00H               ; Default port 0
        STA     IO_OUT_STUB+1
        MVI     A,0C9H              ; RET opcode
        STA     IO_OUT_STUB+2
        
        CALL    PRINT_BANNER        ; Show startup message

; ============================================
; MAIN LOOP
; ============================================

MAIN_LOOP:
        MVI     A,'>'               ; Print prompt
        CALL    CONOUT
        MVI     A,' '
        CALL    CONOUT
        
        CALL    READ_LINE           ; Read command into LINE_BUFFER
        
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
        CPI     'H'
        JZ      CMD_HEX_MATH
        CPI     'I'
        JZ      CMD_INPUT
        CPI     'O'
        JZ      CMD_OUTPUT
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
        ANI     02H
        JZ      CONOUT_WAIT
        POP     PSW
        OUT     CONSOLE_DATA_OUT
        RET

; CONIN - Input character to A
; Trashes: flags
CONIN:
        IN      CONSOLE_STATUS
        ANI     01H
        JZ      CONIN
        IN      CONSOLE_DATA_IN
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
; Trashes: A, HL, flags
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
; Trashes: A, B, C, HL, flags
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
        
        CPI     BS                  ; Backspace?
        JZ      RL_BACKSPACE
        
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
        
        ; Get E's high nibble (will go to D's low nibble)
        MOV     A,E
        ANI     0F0H                ; Isolate high nibble
        RRC
        RRC
        RRC
        RRC                         ; Move to low nibble position
        MOV     C,A                 ; Save it
        
        ; Shift D left 4 bits
        MOV     A,D
        ADD     A
        ADD     A
        ADD     A
        ADD     A                   ; D << 4
        ORA     C                   ; OR in E's high nibble
        MOV     D,A
        
        ; Shift E left 4 bits
        MOV     A,E
        ADD     A
        ADD     A
        ADD     A
        ADD     A                   ; E << 4
        MOV     E,A
        
        ; Add new digit
        POP     PSW
        ORA     E
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
        DAD     D                   ; HL = start + 127 = end
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
        
        ; Compare HL to DE: if HL > DE, we're done
        MOV     A,D
        CMP     H
        JC      CD_DONE             ; D < H, done
        JNZ     CD_LINE             ; D > H, continue
        MOV     A,E
        CMP     L
        JC      CD_DONE             ; E < L, done (D == H)
        JMP     CD_LINE             ; E >= L, continue
        
CD_DONE:
        POP     D                   ; Clean up stack
        SHLD    LAST_DUMP_ADDR      ; Save for next time
        JMP     MAIN_LOOP

; CMD_EXAMINE - Examine/modify memory
; Syntax: E [addr]
; Shows "ADDR: XX-" and waits for input
; Enter hex to modify, CR to advance, period to exit
CMD_EXAMINE:
        CALL    SKIP_SPACES
        MOV     A,M
        ORA     A                   ; Any address given?
        JZ      CE_USE_LAST
        
        CALL    READ_HEX_WORD       ; Parse address into DE
        JC      CE_ERROR            ; Invalid hex
        XCHG                        ; HL = address to examine
        JMP     CE_LOOP
        
CE_USE_LAST:
        LHLD    LAST_EXAM_ADDR
        
CE_LOOP:
        CALL    PRINT_HEX_WORD      ; Print address
        MVI     A,':'
        CALL    CONOUT
        MVI     A,' '
        CALL    CONOUT
        MOV     A,M                 ; Get current byte
        CALL    PRINT_HEX_BYTE
        MVI     A,'-'
        CALL    CONOUT
        
        CALL    READ_EXAM_BYTE      ; Get user input
        JC      CE_EXIT             ; Carry = exit requested
        MOV     A,B                 ; Check digit count
        ORA     A
        JZ      CE_NEXT             ; No digits = don't modify
        MOV     A,C                 ; Get the value from C
        MOV     M,A                 ; Store it
        
CE_NEXT:
        INX     H
        CALL    PRINT_CRLF
        JMP     CE_LOOP
        
CE_EXIT:
        SHLD    LAST_EXAM_ADDR
        CALL    PRINT_CRLF
        JMP     MAIN_LOOP
        
CE_ERROR:
        LXI     H,MSG_BAD_ADDR
        CALL    PRINT_STRING
        JMP     MAIN_LOOP

; CMD_GO - Execute at address
; Syntax: G [addr]
; If no address, defaults to 0100H (TPA)
CMD_GO:
        CALL    SKIP_SPACES
        CALL    READ_HEX_WORD       ; Parse address into DE
        JC      CG_DEFAULT          ; No address given
        XCHG                        ; HL = parsed address
        PCHL                        ; Jump and never return
        
CG_DEFAULT:
        LXI     H,0100H             ; Default to TPA
        PCHL

; CMD_HEX_MATH - Hex addition and subtraction
; Syntax: H num1 num2
; Output: sum difference
CMD_HEX_MATH:
        CALL    SKIP_SPACES
        CALL    READ_HEX_WORD       ; First number -> DE
        JC      CH_ERROR
        PUSH    D                   ; Save first number
        
        CALL    SKIP_SPACES
        CALL    READ_HEX_WORD       ; Second number -> DE
        JC      CH_POP_ERROR
        
        ; DE = second, stack = first
        POP     H                   ; HL = first
        PUSH    H                   ; Save first again
        PUSH    D                   ; Save second
        
        DAD     D                   ; HL = first + second
        CALL    PRINT_HEX_WORD
        CALL    PRINT_SPACE
        
        POP     D                   ; DE = second
        POP     H                   ; HL = first
        
        ; HL = first - second
        MOV     A,L
        SUB     E
        MOV     L,A
        MOV     A,H
        SBB     D
        MOV     H,A
        
        CALL    PRINT_HEX_WORD
        CALL    PRINT_CRLF
        JMP     MAIN_LOOP

CH_POP_ERROR:
        POP     D                   ; Clean stack
CH_ERROR:
        LXI     H,MSG_BAD_HEX
        CALL    PRINT_STRING
        JMP     MAIN_LOOP

; CMD_INPUT - Read from I/O port
; Syntax: I port
CMD_INPUT:
        CALL    SKIP_SPACES
        CALL    READ_HEX_WORD       ; Port -> DE (use E only)
        JC      CI_ERROR
        
        MOV     A,E                 ; Get port number (0-255)
        STA     IO_IN_STUB+1        ; Patch the IN instruction
        CALL    IO_IN_STUB          ; Execute: IN port / RET
        
        CALL    PRINT_HEX_BYTE      ; Print result
        CALL    PRINT_CRLF
        JMP     MAIN_LOOP

CI_ERROR:
        LXI     H,MSG_BAD_PORT
        CALL    PRINT_STRING
        JMP     MAIN_LOOP

; CMD_OUTPUT - Write to I/O port
; Syntax: O port value
CMD_OUTPUT:
        CALL    SKIP_SPACES
        CALL    READ_HEX_WORD       ; Port -> DE
        JC      CO_ERROR
        MOV     A,E
        STA     IO_OUT_STUB+1       ; Patch port
        
        CALL    SKIP_SPACES
        CALL    READ_HEX_WORD       ; Value -> DE
        JC      CO_ERROR
        
        MOV     A,E                 ; Value to output
        CALL    IO_OUT_STUB         ; Execute: OUT port / RET
        JMP     MAIN_LOOP

CO_ERROR:
        LXI     H,MSG_BAD_PORT
        CALL    PRINT_STRING
        JMP     MAIN_LOOP

; CMD_HELP - Show help
; Syntax: ?
CMD_HELP:
        LXI     H,MSG_HELP
        CALL    PRINT_STRING
        JMP     MAIN_LOOP

; ============================================
; HELPER ROUTINES
; ============================================

; READ_EXAM_BYTE - Read byte value for examine command
; Reads up to 2 hex digits from console
; Output: Carry set = exit (period pressed)
;         Carry clear: B = digit count, C = value
;         (caller checks B: 0 = no modification, >0 = store C)
; Trashes: A, B, C, D, flags
READ_EXAM_BYTE:
        MVI     B,0                 ; Digit count
        MVI     C,0                 ; Accumulated value
        
REB_LOOP:
        CALL    CONIN
        MOV     D,A                 ; Save original for echo
        
        CPI     '.'                 ; Exit?
        JZ      REB_EXIT
        CPI     CR                  ; Enter?
        JZ      REB_DONE
        CPI     BS                  ; Backspace?
        JZ      REB_BS
        
        CALL    TO_HEX_DIGIT        ; Convert to 0-15
        JC      REB_LOOP            ; Not hex, ignore
        
        ; Valid hex digit in A
        PUSH    PSW
        MOV     A,C
        ADD     A                   ; Shift left 4
        ADD     A
        ADD     A
        ADD     A
        MOV     C,A
        POP     PSW
        ORA     C                   ; Add new digit
        MOV     C,A
        
        INR     B                   ; Count digit
        MOV     A,D                 ; Echo original char
        CALL    CONOUT
        
        MOV     A,B
        CPI     2                   ; Two digits entered?
        JC      REB_LOOP            ; No, keep reading
        ; Fall through with 2 digits
        
REB_DONE:
        ; Carry clear, B = digit count, C = value
        ORA     A                   ; Clear carry
        RET
        
REB_EXIT:
        STC                         ; Set carry = exit
        RET
        
REB_BS:
        MOV     A,B
        ORA     A
        JZ      REB_LOOP            ; Nothing to delete
        
        DCR     B
        MOV     A,C                 ; Undo the shift
        RRC
        RRC
        RRC
        RRC
        ANI     0FH
        MOV     C,A
        
        MVI     A,BS                ; Erase on screen
        CALL    CONOUT
        MVI     A,' '
        CALL    CONOUT
        MVI     A,BS
        CALL    CONOUT
        JMP     REB_LOOP

; ============================================
; STRINGS
; ============================================

MSG_BANNER:
        DB      CR,LF
        DB      "8080 Monitor v0.1",CR,LF
        DB      'Built: ', DATE, ' ', TIME, CR, LF
        DB      "Ready.",CR,LF
        DB      0

MSG_HELP:
        DB      "Commands:",CR,LF
        DB      "  D [start] [end]  - Dump memory",CR,LF
        DB      "  E [addr]         - Examine/modify",CR,LF
        DB      "  G [addr]         - Go (execute)",CR,LF
        DB      "  H num1 num2      - Hex math (+/-)",CR,LF
        DB      "  I port           - Input from port",CR,LF
        DB      "  O port value     - Output to port",CR,LF
        DB      "  ?                - Help",CR,LF
        DB      0

MSG_UNKNOWN:
        DB      "Unknown command. Type ? for help.",CR,LF,0

MSG_BAD_ADDR:
        DB      "Invalid address",CR,LF,0

MSG_BAD_HEX:
        DB      "Invalid hex value",CR,LF,0

MSG_BAD_PORT:
        DB      "Invalid port/value",CR,LF,0

; ============================================
; PADDING
; ============================================

        IF      $ > 0FFFFH
        ERROR   "ROM exceeds 4KB!"
        ENDIF

        END     COLD_START