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

; ============================================
; COLD START
; ============================================

COLD_START:
        LXI     SP,STACK_TOP        ; Initialize stack pointer
        CALL    PRINT_BANNER        ; Show startup message
        
; ============================================
; MAIN LOOP
; ============================================

MAIN_LOOP:
        MVI     A,'>'               ; Print prompt
        CALL    CONOUT
        MVI     A,' '
        CALL    CONOUT
        
        CALL    CONIN               ; Wait for character
        CALL    CONOUT              ; Echo it
        
        ; For now, just echo and loop
        ; TODO: Command parsing
        
        CPI     CR                  ; If Enter pressed
        JNZ     MAIN_LOOP           ; Keep reading if not
        
        MVI     A,LF                ; Print newline
        CALL    CONOUT
        
        JMP     MAIN_LOOP

; ============================================
; CONSOLE I/O ROUTINES
; ============================================

; CONOUT - Output character in A
; Trashes: nothing
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

; ============================================
; STRINGS
; ============================================

MSG_BANNER:
        DB      CR,LF
        DB      "8080 Monitor v0.1",CR,LF
        DB      "Ready.",CR,LF
        DB      0

; ============================================
; PADDING TO FILL ROM
; ============================================

        ; Ensure we don't exceed 4KB
        IF      $ > 0FFFFH
        ERROR   "ROM exceeds 4KB!"
        ENDIF

        END     COLD_START
