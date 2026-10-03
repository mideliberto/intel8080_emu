; monitor.asm - Intel 8080 Monitor ROM
; Build: make (asl + p2bin). Spec: docs/MONITOR_SPEC.md (commands, messages,
; argument grammar, routine contracts), docs/ARCHITECTURE.md (memory map, boot),
; docs/DEVICE_SPECS.md (ports).
;
; Memory map (ARCHITECTURE 1):
;   0000-007F  unused, not initialized
;   0080-00FF  monitor workspace (equates below)
;   0100-EEFF  user area
;   EF00-EFFF  monitor stack (SP starts at F000)
;   F000-FFFF  this ROM (also at 0000 through the overlay until OUT FE)
;
; Ports: 00-02 console, 08-0C storage, 0D-0F mount, FE overlay off.
;
; Routine contracts: the header above each routine lists its inputs, outputs
; and the registers it trashes. A register that a header lists neither as an
; output nor as trashed is preserved. Commands are entered by JMP, end by
; JMP WARM (or an error tail, which prints and enters WARM), and need not
; balance the stack: WARM resets SP.

        CPU     8080
        ORG     0F000H

; ============================================
; CONSTANTS
; ============================================

CONSOLE_DATA_OUT    EQU     00H
CONSOLE_DATA_IN     EQU     01H
CONSOLE_STATUS      EQU     02H         ; bit 0 = RX ready

SYSTEM_CONTROL      EQU     0FEH        ; any OUT: overlay off

; Storage Device Ports (0x08-0x0C)
STORAGE_ADDR_LO     EQU     08H
STORAGE_ADDR_MID    EQU     09H
STORAGE_ADDR_HI     EQU     0AH
STORAGE_DATA        EQU     0BH
STORAGE_CTRL        EQU     0CH         ; Write: control, Read: status (bit 0 = mounted)

; Storage Mount Ports (0x0D-0x0F)
MOUNT_FILENAME      EQU     0DH
MOUNT_CTRL          EQU     0EH
MOUNT_STATUS        EQU     0FH

STACK_TOP       EQU     0F000H      ; Stack grows down from ROM

CR              EQU     0DH
LF              EQU     0AH
BS              EQU     08H
SPACE           EQU     20H
DEL             EQU     7FH

; ============================================
; WORKSPACE (RAM at 0x0080-0x00FF, ARCHITECTURE 1.1)
; ============================================

LINE_BUFFER     EQU     0080H       ; 80 bytes for command line
LINE_LENGTH     EQU     80          ; 79 characters + NUL
LAST_DUMP_ADDR  EQU     00D2H       ; Last dump address (2 bytes)
LAST_EXAM_ADDR  EQU     00D4H       ; Last examine address (2 bytes)

; I/O stubs (self-modifying code)
IO_IN_STUB      EQU     00D6H       ; 3 bytes: IN xx / RET
IO_OUT_STUB     EQU     00D9H       ; 3 bytes: OUT xx / RET

; Search command workspace
SEARCH_PATTERN  EQU     00DCH       ; 8 bytes for pattern
SEARCH_LENGTH   EQU     00E4H       ; 1 byte for pattern length
SEARCH_END      EQU     00E5H       ; 2 bytes for end address

; Storage command workspace
STOR_ADDR       EQU     00E7H       ; 3 bytes: storage address (lo, mid, hi)

; ============================================
; COLD START (ARCHITECTURE 3.2)
; ============================================

COLD_START:
        ; Running from overlay - PC near 0x0000, reading ROM at 0xF000
        LXI     SP,STACK_TOP        ; Initialize stack pointer
        DI                          ; Disable interrupts
        JMP     BOOT_CONTINUE       ; Jump to F000+ address space

; Now executing from 0xF000+ range - safe to disable overlay
BOOT_CONTINUE:
        XRA     A                   ; A = 0x00
        OUT     SYSTEM_CONTROL      ; Disable overlay, expose RAM at 0x0000

        ; Initialize workspace (now writing to actual RAM)
        LXI     H,0000H
        SHLD    LAST_DUMP_ADDR      ; Default dump address = 0
        SHLD    LAST_EXAM_ADDR      ; Default exam address = 0

        ; I/O stubs: DB 00 C9 (IN 00 / RET) and D3 00 C9 (OUT 00 / RET)
        MVI     L,0DBH
        SHLD    IO_IN_STUB
        MVI     L,0D3H
        SHLD    IO_OUT_STUB
        MVI     A,0C9H
        STA     IO_IN_STUB+2
        STA     IO_OUT_STUB+2

        LXI     H,MSG_BANNER        ; Print the banner, then enter WARM

; ============================================
; MAIN LOOP
; ============================================

; PRINT_WARM - Print the string at HL, then enter WARM. Every message that
; ends a command (and every error) comes through here. Entered by JMP.
PRINT_WARM:
        CALL    PRINT_STRING

; WARM - Reset the stack and prompt. Commands end here, and G pushes this
; address so a program can return with RET (MONITOR_SPEC 8).
WARM:
        LXI     SP,STACK_TOP

MAIN_LOOP:
        MVI     A,'>'               ; Print prompt
        CALL    CONOUT
        CALL    PRINT_SPACE

        CALL    READ_LINE           ; Read command into LINE_BUFFER

        LXI     H,LINE_BUFFER       ; Point to start of buffer
        CALL    SKIP_SPACES         ; A = first non-space character
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
        CPI     'C'
        JZ      CMD_COMPARE
        CPI     'D'
        JZ      CMD_DUMP
        CPI     'E'
        JZ      CMD_EXAMINE
        CPI     'F'
        JZ      CMD_FILL
        CPI     'G'
        JZ      CMD_GO
        CPI     'H'
        JZ      CMD_HEX_MATH
        CPI     'I'
        JZ      CMD_INPUT
        CPI     'M'
        JZ      CMD_MOVE
        CPI     'O'
        JZ      CMD_OUTPUT
        CPI     'S'
        JZ      CMD_SEARCH
        CPI     'L'
        JZ      CMD_LOAD
        CPI     'W'
        JZ      CMD_WRITE
        CPI     'X'
        JZ      CMD_MOUNT
        CPI     '?'
        JZ      CMD_HELP

        LXI     H,MSG_UNKNOWN
        JMP     PRINT_WARM

; Error tails (MONITOR_SPEC 5). Entered by JMP from any depth: WARM resets SP.
ERR_HEX:
        LXI     H,MSG_BAD_HEX
        JMP     PRINT_WARM
ERR_ADDR:
        LXI     H,MSG_BAD_ADDR
        JMP     PRINT_WARM
ERR_PORT:
        LXI     H,MSG_BAD_PORT
        JMP     PRINT_WARM
ERR_RANGE:
        LXI     H,MSG_RANGE
        JMP     PRINT_WARM
ERR_NOSTOR:
        LXI     H,MSG_NO_STORAGE
        JMP     PRINT_WARM

; ============================================
; CONSOLE I/O ROUTINES
; ============================================

; CONOUT - Output character in A. No TX poll: the console never makes
; OUT 00 wait (MONITOR_SPEC 11).
; Input: A = character
; Trashes: nothing
CONOUT:
        OUT     CONSOLE_DATA_OUT
        RET

; CONIN - Wait for a console byte
; Output: A = byte
; Trashes: flags
CONIN:
        IN      CONSOLE_STATUS
        ANI     01H
        JZ      CONIN
        IN      CONSOLE_DATA_IN
        RET

; ============================================
; PRINT ROUTINES
; ============================================

; PRINT_STRING - Print null-terminated string at HL
; Input: HL = string
; Output: HL = address of the NUL
; Trashes: A, flags
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
        JMP     CONOUT

; PRINT_SPACE - Print a space
; Trashes: A, flags
PRINT_SPACE:
        MVI     A,SPACE
        JMP     CONOUT

; PRINT_ADDR - Print HL as four hex digits and ':'
; Input: HL = address
; Trashes: A, flags
PRINT_ADDR:
        CALL    PRINT_HEX_WORD
        MVI     A,':'
        JMP     CONOUT

; PRINT_HEX_WORD - Print HL as four hex digits
; Input: HL = word to print
; Trashes: A, flags
PRINT_HEX_WORD:
        MOV     A,H
        CALL    PRINT_HEX_BYTE
        MOV     A,L                 ; fall into PRINT_HEX_BYTE

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
        POP     PSW                 ; Restore original, fall into PRINT_HEX_NIBBLE

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
        JMP     CONOUT

; ============================================
; INPUT ROUTINES
; ============================================

; READ_LINE - Read a line into LINE_BUFFER (MONITOR_SPEC 2)
; CR or LF ends the line (echo CR LF). BS and DEL delete a character (echo
; BS SP BS). 20-7E and 80-FF are stored and echoed while fewer than 79 are
; stored, then discarded silently. Other control bytes are ignored.
; Output: LINE_BUFFER holds the line, NUL-terminated
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
        CPI     DEL                 ; DEL is a backspace too
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
        CALL    PRINT_SPACE
        MVI     A,BS
        CALL    CONOUT
        JMP     RL_LOOP

RL_DONE:
        MVI     M,0                 ; Null terminate
        JMP     PRINT_CRLF          ; Echo newline

; ============================================
; PARSING ROUTINES (MONITOR_SPEC 4)
; ============================================

; SKIP_SPACES - Skip spaces in buffer
; Input: HL = pointer into buffer
; Output: HL = first non-space, A = that character (NUL at end of line)
; Trashes: flags
SKIP_SPACES:
        MOV     A,M
        CPI     SPACE
        RNZ                         ; Return if not a space
        INX     H
        JMP     SKIP_SPACES

; READ_HEX_WORD - Parse a word argument (1-4 hex digits)
; READ_HEX_ADDR24 - Parse a storage address (1-6 hex digits) into STOR_ADDR
; Both skip leading spaces on entry. A token is a run of non-space characters.
; Input: HL = pointer into the line
; Output, three cases:
;   valid:   CY=0 Z=0. READ_HEX_WORD: DE = value. READ_HEX_ADDR24: STOR_ADDR =
;            value (lo, mid, hi). HL = the space or NUL after the token.
;   absent:  CY=1 Z=1. Only spaces were left; HL = the NUL.
;   invalid: CY=1 Z=0. The token has no digits, too many digits, or a
;            character other than a hex digit before the space or NUL that
;            must end it. HL is somewhere in the token.
; Callers of a required argument test only CY. Callers of an optional one
; test Z (absent) first, then CY (invalid): present-invalid is never absent.
; Trashes: A, B, C, flags; READ_HEX_ADDR24 also DE
READ_HEX_ADDR24:
        MVI     B,7                 ; 6 digits allowed
        CALL    RH_START
        RC
        XCHG
        SHLD    STOR_ADDR           ; lo, mid
        XCHG
        MOV     A,C
        STA     STOR_ADDR+2         ; hi
        JMP     RH_OK

READ_HEX_WORD:
        MVI     B,5                 ; 4 digits allowed
RH_START:
        CALL    SKIP_SPACES
        ORA     A
        STC
        RZ                          ; absent: CY=1 Z=1
        LXI     D,0                 ; C:D:E = value
        MVI     C,0
RH_LOOP:
        MOV     A,M
        CALL    TO_HEX_DIGIT
        JC      RH_END              ; not a digit: the token must end here
        DCR     B
        JZ      RH_BAD              ; one digit too many
        XCHG                        ; HL = value low 16, DE = line pointer
        PUSH    PSW                 ; save the digit
        DAD     H                   ; C:HL <<= 4
        MOV     A,C
        RAL
        MOV     C,A
        DAD     H
        MOV     A,C
        RAL
        MOV     C,A
        DAD     H
        MOV     A,C
        RAL
        MOV     C,A
        DAD     H
        MOV     A,C
        RAL
        MOV     C,A
        POP     PSW
        ORA     L                   ; add the digit
        MOV     L,A
        XCHG                        ; DE = value low 16, HL = line pointer
        INX     H
        JMP     RH_LOOP

RH_END:
        MOV     A,M                 ; a space or the NUL ends the token
        CPI     SPACE
        JZ      RH_OK
        ORA     A                   ; anything else (including a first
        JNZ     RH_BAD              ; character that is not a digit) is invalid
RH_OK:
        ORI     0FFH                ; valid: CY=0 Z=0
        RET
RH_BAD:
        ORI     0FFH                ; invalid: CY=1 Z=0
        STC
        RET

; READ_HEX_BYTE - Parse a byte argument: a word (READ_HEX_WORD) whose value
; is 00-FF. Skips leading spaces. Leading zeros are allowed (00AA); a value
; above FF is invalid, as are the READ_HEX_WORD error cases.
; Input: HL = pointer into the line
; Output: as READ_HEX_WORD (valid, absent, invalid), with E = the byte and
;         D = 0 when valid
; Trashes: A, B, C, flags
READ_HEX_BYTE:
        CALL    READ_HEX_WORD
        RC
        MOV     A,D
        ORA     A
        JZ      RH_OK
        JMP     RH_BAD

; TO_HEX_DIGIT - Convert ASCII to hex value
; Input: A = ASCII character
; Output: A = hex value (0-15) and CY=0, or CY=1 if not 0-9 A-F a-f
; Trashes: flags
TO_HEX_DIGIT:
        SUI     '0'
        RC                          ; below '0'
        CPI     10
        CMC
        RNC                         ; '0'-'9': 0-9, carry clear
        ANI     0DFH                ; fold 'a'-'f' onto 'A'-'F'
        SUI     'A'-'0'
        RC                          ; between '9' and 'A'
        CPI     6
        CMC
        RC                          ; above 'F'
        ADI     10                  ; 10-15, carry clear
        RET

; RANGE - Byte count of an inclusive range (MONITOR_SPEC 4.3)
; Input: HL = start, DE = end
; Output: BC = end - start + 1, where 0 means 65536
; Exits to ERR_RANGE (no return) if end < start.
; Trashes: A, flags
RANGE:
        MOV     A,E
        SUB     L
        MOV     C,A
        MOV     A,D
        SBB     H
        MOV     B,A
        JC      ERR_RANGE
        INX     B
        RET

; ============================================
; COMMANDS (MONITOR_SPEC 6)
; ============================================

; CMD_COMPARE - C start end dest
; Prints AAAA:XX BBBB:YY for each mismatch. dest wraps past FFFF.
CMD_COMPARE:
        CALL    READ_HEX_WORD
        JC      ERR_HEX
        PUSH    D                   ; Stack: start

        CALL    READ_HEX_WORD
        JC      ERR_HEX
        PUSH    D                   ; Stack: end, start

        CALL    READ_HEX_WORD
        JC      ERR_HEX             ; DE = dest

        POP     B                   ; BC = end
        POP     H                   ; HL = start
        PUSH    D                   ; Save dest
        MOV     D,B
        MOV     E,C                 ; DE = end
        CALL    RANGE               ; BC = count
        POP     D                   ; DE = dest

CC_LOOP:
        MOV     A,M                 ; A = first byte
        PUSH    H                   ; Save first pointer
        PUSH    B                   ; Save count
        XCHG                        ; HL = dest
        CMP     M                   ; Compare with second byte
        XCHG                        ; HL = first, DE = dest
        JZ      CC_NEXT

        ; Mismatch. Relies on B surviving the print routines.
        PUSH    D                   ; Save second
        MOV     B,M                 ; B = first byte
        CALL    PRINT_ADDR
        MOV     A,B
        CALL    PRINT_HEX_BYTE
        CALL    PRINT_SPACE

        POP     H                   ; HL = second pointer
        MOV     B,M                 ; B = second byte
        PUSH    H                   ; Save second again
        CALL    PRINT_ADDR
        MOV     A,B
        CALL    PRINT_HEX_BYTE
        CALL    PRINT_CRLF

        POP     D                   ; DE = second ptr
CC_NEXT:
        POP     B                   ; BC = count
        POP     H                   ; HL = first ptr
        INX     H                   ; first++
        INX     D                   ; second++
        DCX     B                   ; count--
        MOV     A,B
        ORA     C
        JNZ     CC_LOOP
        JMP     WARM

; CMD_DUMP - D [start [end]]
; No args: LAST_DUMP_ADDR for 80h bytes; one arg: start for 80h bytes (both
; capped at FFFF); two args: start to end. Whole 16-byte lines; LAST_DUMP_ADDR
; = the next line start (wrapped) when done.
CMD_DUMP:
        CALL    READ_HEX_WORD
        JZ      CD_NO_ARGS
        JC      ERR_ADDR
        PUSH    D                   ; Save start address

        CALL    READ_HEX_WORD
        JZ      CD_ONE_ARG
        JC      ERR_ADDR            ; DE = end

        POP     H                   ; HL = start
        JMP     CD_DUMP_RANGE

CD_NO_ARGS:
        LHLD    LAST_DUMP_ADDR      ; Continue from last address
        PUSH    H
CD_ONE_ARG:
        POP     D                   ; DE = start
        LXI     H,007FH
        DAD     D                   ; HL = start + 7F = end
        JNC     CD_NO_CAP
        LXI     H,0FFFFH            ; Cap at FFFF
CD_NO_CAP:
        XCHG                        ; HL = start, DE = end

CD_DUMP_RANGE:
        CALL    RANGE               ; BC = end - start + 1
        DCX     B                   ; BC = end - start

CD_LINE:
        PUSH    B
        CALL    PRINT_ADDR
        CALL    PRINT_SPACE
        PUSH    H                   ; Save line start for ASCII
        MVI     E,16                ; Byte counter
CD_HEX_BYTE:
        MOV     A,M
        CALL    PRINT_HEX_BYTE
        CALL    PRINT_SPACE
        MOV     A,E
        CPI     9
        CZ      PRINT_SPACE         ; Extra space after 8th byte
        INX     H
        DCR     E
        JNZ     CD_HEX_BYTE
        CALL    PRINT_SPACE
        POP     H
        MVI     E,16
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
        DCR     E
        JNZ     CD_ASCII
        CALL    PRINT_CRLF
        POP     B
        MOV     A,C                 ; BC -= 16; done on borrow
        SUI     16
        MOV     C,A
        MOV     A,B
        SBI     0
        MOV     B,A
        JNC     CD_LINE
        SHLD    LAST_DUMP_ADDR      ; HL = next line start
        JMP     WARM

; CMD_EXAMINE - E [addr]
; Prints AAAA: XX- and reads keys: up to 2 hex digits, BS/DEL delete one, CR
; stores (if any digits) and advances, '.' exits. Everything else is ignored.
CMD_EXAMINE:
        CALL    READ_HEX_WORD
        JZ      CE_USE_LAST
        JC      ERR_ADDR
        XCHG                        ; HL = address to examine
        JMP     CE_LOOP

CE_USE_LAST:
        LHLD    LAST_EXAM_ADDR

CE_LOOP:
        CALL    PRINT_ADDR
        CALL    PRINT_SPACE
        MOV     A,M                 ; Get current byte
        CALL    PRINT_HEX_BYTE
        MVI     A,'-'
        CALL    CONOUT
        LXI     B,0                 ; B = digit count, C = value

CE_KEY:
        CALL    CONIN
        CPI     '.'
        JZ      CE_EXIT
        CPI     CR
        JZ      CE_ENTER
        CPI     BS
        JZ      CE_BS
        CPI     DEL
        JZ      CE_BS
        MOV     D,A                 ; Save the key for the echo
        CALL    TO_HEX_DIGIT
        JC      CE_KEY              ; Not hex: ignore
        MOV     E,A
        MOV     A,B
        CPI     2
        JNC     CE_KEY              ; Two digits already: ignore
        INR     B
        MOV     A,C                 ; C = C * 16 + digit
        ADD     A
        ADD     A
        ADD     A
        ADD     A
        ORA     E
        MOV     C,A
        MOV     A,D                 ; Echo the key as typed
        CALL    CONOUT
        JMP     CE_KEY

CE_BS:
        MOV     A,B
        ORA     A
        JZ      CE_KEY              ; Nothing to delete
        DCR     B
        MOV     A,C                 ; C = C / 16
        RRC
        RRC
        RRC
        RRC
        ANI     0FH
        MOV     C,A
        MVI     A,BS                ; Erase on screen
        CALL    CONOUT
        CALL    PRINT_SPACE
        MVI     A,BS
        CALL    CONOUT
        JMP     CE_KEY

CE_ENTER:
        MOV     A,B
        ORA     A
        JZ      CE_NEXT             ; No digits: don't modify
        MOV     M,C
CE_NEXT:
        INX     H
        CALL    PRINT_CRLF
        JMP     CE_LOOP

CE_EXIT:
        SHLD    LAST_EXAM_ADDR
        CALL    PRINT_CRLF
        JMP     WARM

; CMD_FILL - F start end byte. Never wraps.
CMD_FILL:
        CALL    READ_HEX_WORD
        JC      ERR_HEX
        PUSH    D                   ; Save start

        CALL    READ_HEX_WORD
        JC      ERR_HEX
        PUSH    D                   ; Save end

        CALL    READ_HEX_BYTE
        JC      ERR_HEX

        MOV     A,E                 ; A = byte
        POP     D                   ; DE = end
        POP     H                   ; HL = start
        PUSH    PSW
        CALL    RANGE               ; BC = count
        POP     PSW
        MOV     E,A
CF_LOOP:
        MOV     M,E
        INX     H
        DCX     B
        MOV     A,B
        ORA     C
        JNZ     CF_LOOP
        JMP     WARM

; CMD_GO - G [addr]. Bare G runs 0100. Pushes WARM, so the program can
; return to the prompt with RET (MONITOR_SPEC 8).
CMD_GO:
        CALL    READ_HEX_WORD       ; DE = address
        JZ      CG_DEFAULT
        JC      ERR_ADDR
        XCHG                        ; HL = address
        JMP     CG_RUN
CG_DEFAULT:
        LXI     H,0100H
CG_RUN:
        LXI     D,WARM
        PUSH    D                   ; SP = EFFE, (EFFE) = WARM
        PCHL

; CMD_HEX_MATH - H a b. Prints (a+b) (a-b), mod 10000h.
CMD_HEX_MATH:
        CALL    READ_HEX_WORD       ; First number -> DE
        JC      ERR_HEX
        PUSH    D                   ; Save first number

        CALL    READ_HEX_WORD       ; Second number -> DE
        JC      ERR_HEX

        POP     H                   ; HL = first
        PUSH    H
        DAD     D                   ; HL = first + second
        CALL    PRINT_HEX_WORD
        CALL    PRINT_SPACE
        POP     H                   ; HL = first

        MOV     A,L                 ; HL = first - second
        SUB     E
        MOV     L,A
        MOV     A,H
        SBB     D
        MOV     H,A

        CALL    PRINT_HEX_WORD
        CALL    PRINT_CRLF
        JMP     WARM

; CMD_INPUT - I port. Prints the byte read.
CMD_INPUT:
        CALL    READ_HEX_BYTE       ; E = port
        JC      ERR_PORT

        MOV     A,E
        STA     IO_IN_STUB+1        ; Patch the IN instruction
        CALL    IO_IN_STUB          ; Execute: IN port / RET

        CALL    PRINT_HEX_BYTE      ; Print result
        CALL    PRINT_CRLF
        JMP     WARM

; CMD_MOVE - M src dst count. Copies backward when dst > src, so an
; overlapping move keeps the source contents (memmove). Addresses wrap.
CMD_MOVE:
        CALL    READ_HEX_WORD
        JC      ERR_HEX
        PUSH    D                   ; Save source

        CALL    READ_HEX_WORD
        JC      ERR_HEX
        PUSH    D                   ; Save dest

        CALL    READ_HEX_WORD
        JC      ERR_HEX

        MOV     B,D
        MOV     C,E                 ; BC = count
        MOV     A,B
        ORA     C
        JZ      ERR_RANGE           ; Count 0
        POP     D                   ; DE = dest
        POP     H                   ; HL = source

        MOV     A,L                 ; CY = source < dest
        SUB     E
        MOV     A,H
        SBB     D
        JNC     CM_FORWARD

        DAD     B                   ; Backward: from the last byte down
        DCX     H                   ; HL = source + count - 1
        XCHG
        DAD     B
        DCX     H                   ; HL = dest + count - 1
        XCHG
CM_BACKWARD:
        MOV     A,M
        STAX    D
        DCX     H
        DCX     D
        DCX     B
        MOV     A,B
        ORA     C
        JNZ     CM_BACKWARD
        JMP     WARM

CM_FORWARD:
        MOV     A,M
        STAX    D
        INX     H
        INX     D
        DCX     B
        MOV     A,B
        ORA     C
        JNZ     CM_FORWARD
        JMP     WARM

; CMD_OUTPUT - O port byte. No port is written unless both parse.
CMD_OUTPUT:
        CALL    READ_HEX_BYTE       ; E = port
        JC      ERR_PORT
        MOV     A,E
        STA     IO_OUT_STUB+1       ; Patch the port (workspace only)

        CALL    READ_HEX_BYTE       ; E = value
        JC      ERR_PORT

        MOV     A,E
        CALL    IO_OUT_STUB         ; Execute: OUT port / RET
        JMP     WARM

; CMD_SEARCH - S start end b1 [b2 ... b8]
; Prints each candidate address in start..end where the pattern matches.
; Candidates never wrap; pattern bytes past FFFF do.
CMD_SEARCH:
        CALL    READ_HEX_WORD
        JC      ERR_HEX
        PUSH    D                   ; Save start

        CALL    READ_HEX_WORD
        JC      ERR_HEX
        XCHG
        SHLD    SEARCH_END
        XCHG

        LXI     D,SEARCH_PATTERN    ; DE = pattern pointer
        MVI     B,0                 ; B = pattern length
CS_PARSE:
        PUSH    B
        PUSH    D
        CALL    READ_HEX_BYTE
        MOV     A,E                 ; A = the byte
        POP     D
        POP     B
        JZ      CS_PARSED           ; End of line
        JC      ERR_HEX             ; Invalid token
        STAX    D
        INX     D
        INR     B
        MOV     A,B
        CPI     8                   ; Max 8 bytes; later tokens are ignored
        JC      CS_PARSE

CS_PARSED:
        MOV     A,B
        ORA     A
        JZ      ERR_HEX             ; No pattern
        STA     SEARCH_LENGTH
        LHLD    SEARCH_END
        XCHG                        ; DE = end
        POP     H                   ; HL = start
        CALL    RANGE               ; BC = candidate count

CS_LOOP:
        PUSH    B                   ; Save count
        PUSH    H                   ; Save candidate
        LXI     D,SEARCH_PATTERN
        LDA     SEARCH_LENGTH
        MOV     B,A
CS_COMPARE:
        LDAX    D                   ; A = pattern byte
        CMP     M
        JNZ     CS_NEXT
        INX     H
        INX     D
        DCR     B
        JNZ     CS_COMPARE

        POP     H                   ; Match: print the candidate
        PUSH    H
        CALL    PRINT_HEX_WORD
        CALL    PRINT_CRLF

CS_NEXT:
        POP     H
        POP     B
        INX     H
        DCX     B
        MOV     A,B
        ORA     C
        JNZ     CS_LOOP
        JMP     WARM

; CMD_HELP - ? (arguments ignored)
CMD_HELP:
        LXI     H,MSG_HELP
        JMP     PRINT_WARM

; ============================================
; STORAGE COMMANDS
; ============================================

; CMD_MOUNT - X [name | -]
;   X       query: OUT 0E<-03, IN 0F
;   X -     unmount: OUT 0E<-02 (the rest of the line is ignored)
;   X name  OUT 0E<-03 (resync), name to OUT 0D, OUT 0E<-01, IN 0F
CMD_MOUNT:
        CALL    SKIP_SPACES         ; A = first character
        CPI     '-'
        JZ      CX_UNMOUNT
        MOV     B,A                 ; B = 0 for a query
        MVI     A,03H               ; Query; clears a stale name in the device
        OUT     MOUNT_CTRL
        MOV     A,B
        ORA     A
        JZ      CX_QUERY

CX_SEND:
        MOV     A,M
        ORA     A                   ; End of line?
        JZ      CX_MOUNT
        CPI     SPACE               ; Space ends the name
        JZ      CX_MOUNT
        OUT     MOUNT_FILENAME
        INX     H
        JMP     CX_SEND

CX_MOUNT:
        MVI     A,01H               ; Mount
        OUT     MOUNT_CTRL
        IN      MOUNT_STATUS
        ORA     A
        JZ      CX_MOUNTED
        LXI     H,MSG_INVALID_FILE
        CPI     02H
        JZ      PRINT_WARM
        LXI     H,MSG_MOUNT_FAILED
        JMP     PRINT_WARM

CX_QUERY:
        IN      MOUNT_STATUS
        ORA     A
        JNZ     ERR_NOSTOR
CX_MOUNTED:
        LXI     H,MSG_MOUNTED
        JMP     PRINT_WARM

CX_UNMOUNT:
        MVI     A,02H               ; Unmount
        OUT     MOUNT_CTRL
        LXI     H,MSG_UNMOUNTED
        JMP     PRINT_WARM

; CMD_LOAD - L stor mem [count]
; Checks "mounted", parses all three arguments (count default 0100, 0 is
; Invalid range), then sets the storage address, reads count bytes into
; mem.., and prints Loaded, or Storage error if the file is gone.
CMD_LOAD:
        IN      STORAGE_CTRL
        ANI     01H
        JZ      ERR_NOSTOR

        CALL    READ_HEX_ADDR24     ; STOR_ADDR = storage address
        JC      ERR_HEX
        CALL    READ_HEX_WORD       ; DE = memory address
        JC      ERR_HEX
        PUSH    D
        CALL    READ_HEX_WORD       ; DE = count
        LXI     B,0100H
        JZ      CL_COUNTED          ; Absent: 0100
        JC      ERR_HEX
        MOV     B,D
        MOV     C,E
        MOV     A,B
        ORA     C
        JZ      ERR_RANGE           ; Count 0
CL_COUNTED:
        LDA     STOR_ADDR
        OUT     STORAGE_ADDR_LO
        LDA     STOR_ADDR+1
        OUT     STORAGE_ADDR_MID
        LDA     STOR_ADDR+2
        OUT     STORAGE_ADDR_HI
        POP     D                   ; DE = memory address

CL_LOOP:
        IN      STORAGE_DATA        ; Read + auto-increment
        STAX    D
        INX     D
        DCX     B
        MOV     A,B
        ORA     C
        JNZ     CL_LOOP

        LXI     H,MSG_LOADED

; STOR_CHECK - L and W end here with HL = the success message. Prints it if
; the file is still mounted (status bit 0), else Storage error.
STOR_CHECK:
        IN      STORAGE_CTRL
        ANI     01H
        JNZ     PRINT_WARM
        LXI     H,MSG_STOR_ERROR
        JMP     PRINT_WARM

; CMD_WRITE - W mem stor [count]
; As L, in the other direction, then flush (OUT 0C<-02) before the check.
CMD_WRITE:
        IN      STORAGE_CTRL
        ANI     01H
        JZ      ERR_NOSTOR

        CALL    READ_HEX_WORD       ; DE = memory address
        JC      ERR_HEX
        PUSH    D
        CALL    READ_HEX_ADDR24     ; STOR_ADDR = storage address
        JC      ERR_HEX
        CALL    READ_HEX_WORD       ; DE = count
        LXI     B,0100H
        JZ      CW_COUNTED          ; Absent: 0100
        JC      ERR_HEX
        MOV     B,D
        MOV     C,E
        MOV     A,B
        ORA     C
        JZ      ERR_RANGE           ; Count 0
CW_COUNTED:
        LDA     STOR_ADDR
        OUT     STORAGE_ADDR_LO
        LDA     STOR_ADDR+1
        OUT     STORAGE_ADDR_MID
        LDA     STOR_ADDR+2
        OUT     STORAGE_ADDR_HI
        POP     H                   ; HL = memory address

CW_LOOP:
        MOV     A,M
        OUT     STORAGE_DATA        ; Write + auto-increment
        INX     H
        DCX     B
        MOV     A,B
        ORA     C
        JNZ     CW_LOOP

        MVI     A,02H               ; Flush
        OUT     STORAGE_CTRL
        LXI     H,MSG_WRITTEN
        JMP     STOR_CHECK

; ============================================
; STRINGS (MONITOR_SPEC 1.1, 5, 6.14)
; ============================================

MSG_BANNER:
        DB      CR,LF
        DB      "8080 Monitor v0.3",CR,LF
        DB      'Built: ', DATE, ' ', TIME, CR, LF
        DB      "Ready.",CR,LF
        DB      0

MSG_HELP:
        DB      "Commands:",CR,LF
        DB      "  C start end dest - Compare memory",CR,LF
        DB      "  D [start] [end]  - Dump memory",CR,LF
        DB      "  E [addr]         - Examine/modify",CR,LF
        DB      "  F start end val  - Fill memory",CR,LF
        DB      "  G [addr]         - Go (execute)",CR,LF
        DB      "  H num1 num2      - Hex math (+/-)",CR,LF
        DB      "  I port           - Input from port",CR,LF
        DB      "  L stor mem [cnt] - Load from storage",CR,LF
        DB      "  M src dst cnt    - Move memory",CR,LF
        DB      "  O port value     - Output to port",CR,LF
        DB      "  S start end pat  - Search memory",CR,LF
        DB      "  W mem stor [cnt] - Write to storage",CR,LF
        DB      "  X [file | -]     - Mount/unmount storage",CR,LF
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
MSG_RANGE:
        DB      "Invalid range",CR,LF,0
MSG_NO_STORAGE:
        DB      "No storage mounted",CR,LF,0
MSG_STOR_ERROR:
        DB      "Storage error",CR,LF,0
MSG_MOUNTED:
        DB      "Mounted",CR,LF,0
MSG_UNMOUNTED:
        DB      "Unmounted",CR,LF,0
MSG_INVALID_FILE:
        DB      "Invalid filename",CR,LF,0
MSG_MOUNT_FAILED:
        DB      "Mount failed",CR,LF,0
MSG_LOADED:
        DB      "Loaded",CR,LF,0
MSG_WRITTEN:
        DB      "Written",CR,LF,0

; ROM_END - first byte after the ROM contents. make size: ROM_END - F000.
ROM_END:

        END     COLD_START
