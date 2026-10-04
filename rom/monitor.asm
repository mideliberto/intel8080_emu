; monitor.asm - Intel 8080 Monitor ROM
; Build: make (asl + p2bin). Spec: docs/MONITOR_SPEC.md (commands, messages,
; argument grammar, routine contracts), docs/ARCHITECTURE.md (memory map, boot),
; docs/DEVICE_SPECS.md (ports).
;
; Memory map (ARCHITECTURE 1):
;   0000-007F  unused, not initialized; G writes the RST 6 vector at 0030
;   0080-00FF  monitor workspace (labels below)
;   0100-EEFF  user area
;   EF00-EFFF  monitor stack (SP starts at F000)
;   F000-FFFF  this ROM (also at 0000 through the overlay until OUT FE)
; The RAM test build (asl -D RAMBUILD, ARCHITECTURE 2.1) runs the same code at
; D000 and owns D000-EEFF too: the user area is 0100-CFFF.
;
; Ports: 00-02 console, 08-0C storage, 0D-0F mount, 10-13 mailbox, FE overlay off.
;
; Routine contracts: the header above each routine lists its inputs, outputs
; and the registers it trashes. A register that a header lists neither as an
; output nor as trashed is preserved. Commands are entered by JMP, end by
; JMP WARM (or an error tail, which prints and enters WARM), and need not
; balance the stack: WARM resets SP.

        CPU     8080

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

; Service Mailbox Ports (0x10-0x13, DEVICE_SPECS 8)
MAILBOX_DATA        EQU     10H         ; Write: append a command byte
MAILBOX_CTRL        EQU     11H         ; Write: 01 execute, 02 clear
MAILBOX_STATUS      EQU     12H         ; Read: 00 idle, 01 busy, 02 avail, 03 done, 80-FF error
MAILBOX_RESPONSE    EQU     13H         ; Read: pop a response byte

STACK_TOP       EQU     0F000H      ; Stack grows down from ROM
STACK_PAGE      EQU     0EF00H      ; Monitor stack page EF00-EFFF

; Build variants (ARCHITECTURE 2.1). `asl` alone builds the ROM. `asl -D RAMBUILD`
; builds the RAM test image: the same code at CODE_BASE, the HEX guard's top at
; the image, F/M/L refusing the image (IMG_GUARD), and " RAM" after the banner
; version. The ROM build assembles none of the RAMBUILD code.
        IFDEF   RAMBUILD
CODE_BASE       EQU     0D000H      ; RAM test image: G D000. Page-aligned.
USER_END        EQU     CODE_BASE   ; first address above the user area
        ELSE
CODE_BASE       EQU     0F000H      ; the ROM
USER_END        EQU     STACK_PAGE  ; first address above the user area
        ENDIF

CR              EQU     0DH
LF              EQU     0AH
BS              EQU     08H
SPACE           EQU     20H
DEL             EQU     7FH
ESC             EQU     1BH

BRK_VEC         EQU     0030H       ; RST 6 vector, written by G (MONITOR_SPEC 8.1)

; ============================================
; WORKSPACE (RAM at 0x0080-0x00FF, ARCHITECTURE 1.1)
; Labels, so the debugger symbols name them. DS reserves and emits no bytes.
; ============================================

        ORG     0080H
LINE_BUFFER:    DS      80          ; command line: 79 characters + NUL
                DS      2           ; free
LAST_DUMP_ADDR: DS      2           ; last dump address
LAST_EXAM_ADDR: DS      2           ; last examine address
IO_IN_STUB:     DS      3           ; IN pp / RET (self-modifying code)
IO_OUT_STUB:    DS      3           ; OUT pp / RET (self-modifying code)
SEARCH_PATTERN: DS      8           ; search pattern
SEARCH_LENGTH:  DS      1           ; search pattern length
SEARCH_END:     DS      2           ; search end address
STOR_ADDR:      DS      3           ; storage address (lo, mid, hi)
REGS:           DS      8           ; saved at a G return or RST 6 break: L H E D C B F A
                DS      14          ; free

LINE_LENGTH     EQU     80          ; LINE_BUFFER size

        ORG     CODE_BASE

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

; WARM - Reset the stack and prompt. Commands end here, and so does G_RETURN,
; where a program started by G returns with RET (MONITOR_SPEC 8).
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

        ; Command dispatch. ':' first: a pasted HEX file is many lines.
        CPI     ':'                 ; Not a command letter: an Intel HEX record
        JZ      HEX_RECORD
        CPI     'A'
        JZ      CMD_ASM
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
        CPI     'N'
        JZ      CMD_NET
        CPI     'O'
        JZ      CMD_OUTPUT
        CPI     'Q'
        JZ      CMD_ASK
        CPI     'R'
        JZ      CMD_REGS
        CPI     'S'
        JZ      CMD_SEARCH
        CPI     'T'
        JZ      CMD_TIME
        CPI     'U'
        JZ      CMD_UNASM
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
ERR_RECORD:
        LXI     H,MSG_BAD_RECORD
        JMP     PRINT_WARM
ERR_TOO_LONG:
        LXI     H,MSG_TOO_LONG
        JMP     PRINT_WARM
ERR_CHECKSUM:
        LXI     H,MSG_CHECKSUM
        JMP     PRINT_WARM
ERR_REC_TYPE:
        LXI     H,MSG_REC_TYPE
        JMP     PRINT_WARM
ERR_ADDR_RANGE:
        LXI     H,MSG_ADDR_RANGE
        JMP     PRINT_WARM
ERR_SERVICE:
        LXI     H,MSG_SERVICE
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
; test Z (absent) first, then CY (invalid), or CY first, then Z (CMD_GO):
; valid is CY=0 Z=0, and present-invalid is never absent.
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

; HEX_PAIR - Parse exactly two hex digits (an Intel HEX byte, MONITOR_SPEC 7.1)
; Input: HL = the first digit
; Output: CY=0: A = the byte, HL += 2
;         CY=1: either character is not a hex digit (the NUL included); HL is
;         somewhere in the pair
; Trashes: B, flags
HEX_PAIR:
        MOV     A,M
        CALL    TO_HEX_DIGIT
        RC
        ADD     A                   ; high nibble
        ADD     A
        ADD     A
        ADD     A
        MOV     B,A
        INX     H
        MOV     A,M
        CALL    TO_HEX_DIGIT
        RC
        ORA     B                   ; CY=0
        INX     H
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
; MAILBOX CLIENT (DEVICE_SPECS 8 reference client, MONITOR_SPEC 9)
; Used by T, A, U, N and Q. The caller sends with MB_SEND (and MB_PUT, MB_HEX),
; executes (OUT 11 <- 01), then calls MB_GET until it returns done or failed.
; T, N and Q wait out busy themselves (CT_GET) and enter at MB_GOT, so the
; Esc check (MB_KEY) runs only for them.
; ============================================

; MB_SEND - Clear the mailbox (the resync), then append a NUL-terminated string
; MB_PUT - Append a NUL-terminated string, without the clear
; Input: HL = string
; Output: HL = address of the NUL
; Trashes: A, flags
MB_SEND:
        MVI     A,02H
        OUT     MAILBOX_CTRL        ; clear (resync)
MB_PUT:
        MOV     A,M
        ORA     A
        RZ
        OUT     MAILBOX_DATA
        INX     H
        JMP     MB_PUT

; MB_HEX - Append A as two uppercase hex digits
; Input: A = byte
; Trashes: A, flags
MB_HEX:
        PUSH    PSW
        RRC
        RRC
        RRC
        RRC
        CALL    MB_NIBBLE
        POP     PSW
MB_NIBBLE:
        ANI     0FH
        CPI     0AH
        JC      MBN_DIGIT
        ADI     07H
MBN_DIGIT:
        ADI     '0'
        OUT     MAILBOX_DATA
        RET

; MB_KEY - Esc check for a running N or Q request (MONITOR_SPEC 6.18, 6.19).
; Reads every console byte waiting. Esc: reads the rest that waits, clears the
; mailbox (aborts the request), prints Aborted and enters WARM; does not return.
; Every other byte is discarded. Returns when no byte waits.
; Trashes: A, flags
MB_KEY:
        IN      CONSOLE_STATUS
        RRC                         ; bit 0 (a byte waits) -> CY
        RNC                         ; nothing waiting
        IN      CONSOLE_DATA_IN
        CPI     ESC
        JNZ     MB_KEY              ; any other byte: discarded, look again
MK_ESC:
        IN      CONSOLE_STATUS      ; Esc: discard the rest that waits
        RRC
        JNC     MK_ABORT
        IN      CONSOLE_DATA_IN
        JMP     MK_ESC
MK_ABORT:
        MVI     A,02H
        OUT     MAILBOX_CTRL        ; clear: aborts the request
        LXI     H,MSG_ABORTED
        JMP     PRINT_WARM

; MB_GET - Wait for the next result after execute. Polls status: 01 (busy)
; polls again. Three outcomes; callers test CY before Z:
;   byte:   CY=0, A = the next response byte (Z undefined). Status was 02.
;   done:   CY=1 Z=1. Status 03.
;   failed: CY=1 Z=0, A = the status: 00 (after execute: the Pi service
;           restarted) or 80-FF. 04-7F is never returned.
; MB_GOT - The same, entered with A = a status already read and not 01.
; Trashes: A, flags
MB_GET:
        IN      MAILBOX_STATUS
        CPI     01H
        JZ      MB_GET              ; 01 busy
MB_GOT:
        CPI     02H
        JNZ     MB_END
        IN      MAILBOX_RESPONSE    ; 02 avail (CY=0 from the CPI)
        RET
MB_END:
        CPI     03H                 ; Z: 03 done
        STC                         ; NZ: failed, A = status
        RET

; ============================================
; COMMANDS (MONITOR_SPEC 6)
; ============================================

; CMD_ASM - A addr (MONITOR_SPEC 6.16)
; Prompts AAAA: and reads a line. Empty: prompt again. '.': end. Otherwise
; mailbox ASM <text>, and each response byte is stored at the address, which
; advances (wrapping). Status 82 prints Invalid instruction, any other failure
; Service error; both prompt again at the address the line started at. Only
; '.' ends A, so pasted source never reaches the command dispatcher.
CMD_ASM:
        CALL    READ_HEX_WORD       ; DE = address
        JC      ERR_ADDR
CA_PROMPT:
        XCHG
        CALL    PRINT_ADDR          ; AAAA:
        XCHG
        CALL    PRINT_SPACE
        CALL    READ_LINE
        LXI     H,LINE_BUFFER
        CALL    SKIP_SPACES         ; HL = the text, A = its first character
        ORA     A
        JZ      CA_PROMPT           ; empty: same address, nothing sent
        CPI     '.'
        JZ      WARM
        PUSH    H
        LXI     H,STR_ASM
        CALL    MB_SEND             ; clear, "ASM "
        POP     H
        CALL    MB_PUT              ; the text, trailing spaces included
        MVI     A,01H
        OUT     MAILBOX_CTRL        ; execute
        PUSH    D                   ; the address this line started at
CA_GET:
        CALL    MB_GET
        JC      CA_END
        STAX    D                   ; a byte: store it, advance
        INX     D
        JMP     CA_GET
CA_END:
        POP     H                   ; HL = line start
        JZ      CA_PROMPT           ; done: DE = the next address
        XCHG                        ; failed: DE = line start
        CPI     82H
        LXI     H,MSG_BAD_INSN
        JZ      CA_FAIL
        LXI     H,MSG_SERVICE
CA_FAIL:
        CALL    PRINT_STRING
        JMP     CA_PROMPT


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
        IFDEF   RAMBUILD
        CALL    IMG_GUARD
        ENDIF
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

; CMD_GO - G [addr]. Bare G runs 0100. Writes JMP BRK_ENTRY at 0030 (MONITOR_SPEC 8.1)
; and pushes G_RETURN, so the program can return to the prompt with RET (MONITOR_SPEC 8).
CMD_GO:
        CALL    READ_HEX_WORD       ; DE = address
        JNC     CG_RUN
        JNZ     ERR_ADDR
        LXI     D,0100H             ; bare G
CG_RUN:
        MVI     A,0C3H              ; JMP BRK_ENTRY at 0030 (MONITOR_SPEC 8.1)
        STA     BRK_VEC
        LXI     H,BRK_ENTRY
        SHLD    BRK_VEC+1
        LXI     H,G_RETURN
        PUSH    H                   ; SP = EFFE, (EFFE) = G_RETURN
        XCHG
        PCHL

; G_RETURN - A program started by G returns here with RET (MONITOR_SPEC 8).
; Saves A, F, BC, DE and HL in REGS for R, then enters WARM. SP is the store
; pointer: the four pushes fill REGS+7 down to REGS. SP points into REGS here:
; an interrupt source needs a DI first (MONITOR_SPEC 8).
G_RETURN:
        LXI     SP,REGS+8
        PUSH    PSW                 ; REGS+7 = A, REGS+6 = F
        PUSH    B                   ; REGS+5 = B, REGS+4 = C
        PUSH    D                   ; REGS+3 = D, REGS+2 = E
        PUSH    H                   ; REGS+1 = H, REGS+0 = L
        JMP     WARM

; BRK_ENTRY - RST 6 lands here through the JMP that G writes at 0030 (MONITOR_SPEC 8.1).
; Saves A, F, BC, DE, HL in REGS as G_RETURN does, prints BRK and the address of the
; RST 6, then enters WARM. Nothing before PUSH PSW touches the flags. SP points into
; REGS between the LXI SPs: an interrupt source needs a DI first (MONITOR_SPEC 8).
; The program's stack loses the two bytes RST pushed.
BRK_ENTRY:
        SHLD    REGS                ; REGS+0 = L, REGS+1 = H
        POP     H                   ; HL = break address + 1
        LXI     SP,REGS+8
        PUSH    PSW                 ; REGS+7 = A, REGS+6 = F
        PUSH    B                   ; REGS+5 = B, REGS+4 = C
        PUSH    D                   ; REGS+3 = D, REGS+2 = E
        LXI     SP,STACK_TOP        ; a CALL here would overwrite REGS+0/1
        XCHG
        DCX     D                   ; DE = address of the RST 6
        LXI     H,MSG_BRK
        CALL    PRINT_STRING        ; preserves DE
        XCHG
        CALL    PRINT_HEX_WORD
        CALL    PRINT_CRLF
        JMP     WARM

; CMD_REGS - R. Prints MSG_REGS, each '@' replaced by the next saved byte,
; from REGS+7 (A) down to REGS (L) (MONITOR_SPEC 6.20).
CMD_REGS:
        LXI     D,MSG_REGS
        LXI     H,REGS+7
CR_LOOP:
        LDAX    D
        INX     D
        ORA     A
        JZ      WARM
        CPI     '@'
        JZ      CR_BYTE
        CALL    CONOUT
        JMP     CR_LOOP
CR_BYTE:
        MOV     A,M
        DCX     H
        CALL    PRINT_HEX_BYTE
        JMP     CR_LOOP

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
        IFDEF   RAMBUILD
        XCHG
        CALL    IMG_GUARD           ; on dest
        XCHG
        ENDIF

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

; CMD_TIME - T (arguments ignored, MONITOR_SPEC 6.15). Mailbox TIME: each
; response byte is printed as it arrives, an LF as CR LF; done prints CR LF; a
; failure prints Service error, after any bytes already printed. N and Q share
; the loop from CT_EXEC. Esc (MB_KEY) is checked on each busy pass and at each
; LF, before it prints; TIME is never busy and has no LF, so T never checks.
CMD_TIME:
        LXI     H,STR_TIME
        CALL    MB_SEND             ; clear, "TIME"
CT_EXEC:
        MVI     A,01H
        OUT     MAILBOX_CTRL        ; execute
CT_GET:
        IN      MAILBOX_STATUS      ; the busy wait is CT_GET's own:
        CPI     01H                 ; A and U never reach MB_KEY
        JZ      CT_BUSY
        CALL    MB_GOT
        JC      CT_END
        CPI     LF                  ; LF prints as CR LF
        JNZ     CT_OUT
        CALL    MB_KEY              ; once per line: Esc stops a fast stream
        MVI     A,CR
        CALL    CONOUT
        MVI     A,LF
CT_OUT:
        CALL    CONOUT
        JMP     CT_GET
CT_BUSY:
        CALL    MB_KEY              ; 01 busy: Esc aborts (not on the
        JMP     CT_GET              ; path from CT_EXEC: T never checks)
CT_END:
        JNZ     ERR_SERVICE
        CALL    PRINT_CRLF
        JMP     WARM

; CMD_NET - N text (MONITOR_SPEC 6.18). Mailbox GET with the rest of the line,
; verbatim (the device parses it); the response prints as T's does.
; Q (CMD_ASK) enters at CN_SEND with HL = its verb string and DE = the text.
CMD_NET:
        XCHG                        ; DE = the text after N (MB_SEND keeps DE)
        LXI     H,STR_GET
CN_SEND:
        CALL    MB_SEND             ; clear, the verb ("GET " or "ASK ")
        XCHG
        CALL    MB_PUT              ; the rest of the line, as stored
        JMP     CT_EXEC

; CMD_ASK - Q text (MONITOR_SPEC 6.19). Mailbox ASK with the rest of the line,
; verbatim (the device trims and checks it), through N's tail; the reply prints
; as T's does.
CMD_ASK:
        XCHG                        ; DE = the text after Q
        LXI     H,STR_ASK
        JMP     CN_SEND             ; clear, "ASK ", the text, execute, print

; CMD_UNASM - U addr [count] (MONITOR_SPEC 6.17). count instructions, default 8,
; 0 is Invalid range. For each: mailbox DIS AAAA B0 B1 B2 (the 3 bytes at the
; address, wrapping); the first response byte is the length, the rest (the line
; and its CR LF) is printed as it arrives; the address advances by the length.
; A failure, or done before the length byte, prints Service error and ends U.
CMD_UNASM:
        CALL    READ_HEX_WORD
        JC      ERR_ADDR
        PUSH    D                   ; address
        CALL    READ_HEX_WORD       ; DE = count
        LXI     B,0008H
        JZ      CU_COUNTED          ; Absent: 8
        JC      ERR_HEX
        MOV     B,D
        MOV     C,E
        MOV     A,B
        ORA     C
        JZ      ERR_RANGE           ; Count 0
CU_COUNTED:
        POP     H                   ; HL = address
CU_LINE:
        PUSH    B                   ; count
        PUSH    H
        LXI     H,STR_DIS
        CALL    MB_SEND             ; clear, "DIS "
        POP     H
        MOV     A,H
        CALL    MB_HEX
        MOV     A,L
        CALL    MB_HEX              ; AAAA
        PUSH    H
        MVI     B,3
CU_BYTE:
        MVI     A,SPACE
        OUT     MAILBOX_DATA
        MOV     A,M
        CALL    MB_HEX              ; B0, B1, B2
        INX     H
        DCR     B
        JNZ     CU_BYTE
        POP     H                   ; HL = address
        MVI     A,01H
        OUT     MAILBOX_CTRL        ; execute
        CALL    MB_GET              ; the length byte
        JC      ERR_SERVICE         ; done or failed before it
        MOV     C,A
        MVI     B,0
        DAD     B                   ; HL = the next address (wraps)
CU_TEXT:
        CALL    MB_GET
        JC      CU_END
        CALL    CONOUT
        JMP     CU_TEXT
CU_END:
        JNZ     ERR_SERVICE         ; failed: the message follows the bytes printed
        POP     B
        DCX     B
        MOV     A,B
        ORA     C
        JNZ     CU_LINE
        JMP     WARM

; CMD_HELP - ? (arguments ignored)
CMD_HELP:
        LXI     H,MSG_HELP
        JMP     PRINT_WARM

; ============================================
; INTEL HEX LOADER (MONITOR_SPEC 7)
; ============================================

; HEX_RECORD - One Intel HEX record: ':' LL AAAA TT data CC, alone on the line.
; Entered by JMP from MAIN_LOOP with HL = the character after ':'.
; Two passes over LINE_BUFFER. Pass 1 runs MONITOR_SPEC 7.2 steps 1-4 and
; writes nothing. Pass 2 re-reads the header, runs steps 5-6, then acts
; (7.3). The first failing step prints its message (error tail -> WARM), so
; a rejected record writes nothing. No state survives the record: each line
; stands alone (7). Writes only inside 0100-(USER_END-1): 0100-EEFF in the ROM,
; 0100-CFFF in the RAM build. Never into LINE_BUFFER.
HEX_RECORD:
        PUSH    H                   ; pass 2 starts again at LL

        ; ---- Pass 1: syntax, length, checksum. No writes. ----

        ; Step 1: LL is two hex digits.
        CALL    HEX_PAIR
        JC      ERR_RECORD

        ; Step 2: LL <= 22h (34).
        CPI     23H
        JNC     ERR_TOO_LONG

        ; Step 3: AAAA, TT, the LL data bytes and CC (LL+4 pairs) are each two
        ; hex digits, and the line ends right after CC. The loop also adds up
        ; every byte for step 4; LL is the first.
        MOV     C,A                 ; C = sum
        ADI     4
        MOV     E,A                 ; E = pairs after LL
HR_PAIRS:
        CALL    HEX_PAIR
        JC      ERR_RECORD
        ADD     C
        MOV     C,A
        DCR     E
        JNZ     HR_PAIRS
        MOV     A,M
        ORA     A                   ; the NUL must follow CC
        JNZ     ERR_RECORD

        ; Step 4: the sum of LL through CC is 00 (mod 100h).
        MOV     A,C
        ORA     A
        JNZ     ERR_CHECKSUM

        ; ---- Pass 2: type, guard, then act. Pass 1 proved every pair, ----
        ; ---- so these HEX_PAIR calls do not test CY.                  ----

        POP     H                   ; HL = LL
        CALL    HEX_PAIR
        MOV     C,A                 ; C = LL
        CALL    HEX_PAIR
        MOV     D,A
        CALL    HEX_PAIR
        MOV     E,A                 ; DE = AAAA
        CALL    HEX_PAIR            ; A = TT, HL = first data pair

        ; Step 5: TT = 00 or 01. Type 01 (EOF) ignores LL, AAAA and data.
        CPI     01H
        JZ      HR_EOF
        ORA     A
        JNZ     ERR_REC_TYPE

        ; Type 00 with LL = 0 writes nothing, so the guard skips it.
        MOV     A,C
        ORA     A
        JZ      WARM

        ; Step 6: AAAA >= 0100h and AAAA + LL <= USER_END, the sum without wrap.
        MOV     A,D
        ORA     A                   ; AAAA < 0100: high byte 00
        JZ      ERR_ADDR_RANGE
        PUSH    H
        MVI     H,0
        MOV     L,C
        DAD     D                   ; HL = AAAA + LL, CY = carry out of bit 15
        JC      ERR_ADDR_RANGE
        MOV     A,L                 ; CY = HL < USER_END+1, i.e. HL <= USER_END
        SUI     (USER_END+1) & 0FFH
        MOV     A,H
        SBI     (USER_END+1) >> 8
        JNC     ERR_ADDR_RANGE
        POP     H

        ; Write (7.3): the LL data bytes to AAAA, AAAA+1, ...
HR_WRITE:
        CALL    HEX_PAIR
        STAX    D
        INX     D
        DCR     C
        JNZ     HR_WRITE
        JMP     WARM

HR_EOF:
        LXI     H,MSG_LOADED
        JMP     PRINT_WARM

        IFDEF   RAMBUILD
; IMG_GUARD - RAM test build only (ARCHITECTURE 2.1): F, M and L refuse a
; destination that touches the running image, CODE_BASE-(STACK_PAGE-1).
; Input: HL = first address, BC = count (0 means 65536); the range wraps past FFFF
; Exits to ERR_ADDR_RANGE (no return) if any byte of it lies in the image.
; Trashes: A, flags
IMG_GUARD:
        MOV     A,H                 ; HL inside the image: (H - base page) < pages
        SUI     CODE_BASE >> 8
        CPI     (STACK_PAGE - CODE_BASE) >> 8
        JC      ERR_ADDR_RANGE
        MOV     A,B                 ; 65536 bytes reach everything
        ORA     C
        JZ      ERR_ADDR_RANGE
        PUSH    D
        XRA     A                   ; DE = CODE_BASE - HL, the distance to the image
        SUB     L
        MOV     E,A
        MVI     A,CODE_BASE >> 8
        SBB     H
        MOV     D,A
        MOV     A,E                 ; CY = DE < BC: the range reaches CODE_BASE
        SUB     C
        MOV     A,D
        SBB     B
        POP     D
        JC      ERR_ADDR_RANGE
        RET
        ENDIF

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
        IFDEF   RAMBUILD
        POP     H                   ; HL = memory address
        PUSH    H
        CALL    IMG_GUARD           ; before any port is written
        ENDIF
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
; STRINGS (MONITOR_SPEC 1.1, 5, 6.14; mailbox commands)
; ============================================

MSG_BANNER:
        DB      CR,LF
        DB      "8080 Monitor v0.9"
        IFDEF   RAMBUILD
        DB      " RAM"
        ENDIF
        DB      CR,LF
        DB      'Built: ', DATE, ' ', TIME, CR, LF
        DB      "Ready.",CR,LF
        DB      0

MSG_HELP:
        DB      "Commands:",CR,LF
        DB      "  A addr           - Assemble",CR,LF
        DB      "  C start end dest - Compare memory",CR,LF
        DB      "  D [start] [end]  - Dump memory",CR,LF
        DB      "  E [addr]         - Examine/modify",CR,LF
        DB      "  F start end val  - Fill memory",CR,LF
        DB      "  G [addr]         - Go (execute)",CR,LF
        DB      "  H num1 num2      - Hex math (+/-)",CR,LF
        DB      "  I port           - Input from port",CR,LF
        DB      "  L stor mem [cnt] - Load from storage",CR,LF
        DB      "  M src dst cnt    - Move memory",CR,LF
        DB      "  N url [> file]   - HTTP GET",CR,LF
        DB      "  O port value     - Output to port",CR,LF
        DB      "  Q text           - Ask Claude",CR,LF
        DB      "  R                - Registers",CR,LF
        DB      "  S start end pat  - Search memory",CR,LF
        DB      "  T                - Show time",CR,LF
        DB      "  U addr [cnt]     - Unassemble",CR,LF
        DB      "  W mem stor [cnt] - Write to storage",CR,LF
        DB      "  X [file | -]     - Mount/unmount storage",CR,LF
        DB      "  :LLAAAATT..CC    - Intel HEX record",CR,LF
        DB      "  ?                - Help",CR,LF
        DB      0

MSG_REGS:
        DB      "A=@ F=@ BC=@@ DE=@@ HL=@@",CR,LF,0
MSG_BRK:
        DB      "BRK ",0
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
MSG_TOO_LONG:
        DB      "Record too long",CR,LF,0
MSG_BAD_RECORD:
        DB      "Bad record",CR,LF,0
MSG_CHECKSUM:
        DB      "Checksum error",CR,LF,0
MSG_REC_TYPE:
        DB      "Bad record type",CR,LF,0
MSG_ADDR_RANGE:
        DB      "Address out of range",CR,LF,0
MSG_SERVICE:
        DB      "Service error",CR,LF,0
MSG_BAD_INSN:
        DB      "Invalid instruction",CR,LF,0
MSG_ABORTED:
        DB      "Aborted",CR,LF,0

; Mailbox command words (MB_SEND)
STR_TIME:
        DB      "TIME",0
STR_ASM:
        DB      "ASM ",0
STR_DIS:
        DB      "DIS ",0
STR_GET:
        DB      "GET ",0
STR_ASK:
        DB      "ASK ",0

; ROM_END - first byte after the ROM contents. make size: ROM_END - F000.
; The RAM build ends far below the stack page: D000 + 4096 + its few extra bytes.
ROM_END:

        END     COLD_START
