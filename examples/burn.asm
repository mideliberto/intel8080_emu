; burn.asm - programs the ROM (F000-FFFF) from a 4096-byte image in RAM, through
; jumper JP-WE (ARCHITECTURE 6.10), then cold-starts the new monitor.
; Load: the image to 1000-1FFF (X MONITOR.BIN, L 0 1000 1000), then paste
; burn.hex. Fit JP-WE. Run: G 0100. Procedure: USER_GUIDE.md 10.
; The image address is SRC: change it with E 0103 (low byte first). The image
; must lie in RAM, clear of this program (0100-01FF), the workspace and the
; stack page; F000-FFFF itself is not an image.
;
; Before any write it checks that the image is a ROM build: byte 0 is 31
; (COLD_START's LXI SP) and byte 6 is F0 (the high byte of its JMP; the RAM
; test build has D0 there, ARCHITECTURE 3.2). Otherwise it prints
; "Not a ROM image" and returns to the monitor with nothing written.
;
; Then for each 64-byte page, F000 first: write the 64 bytes, wait for the page
; load to close, toggle-poll I/O6 until the write cycle ends, and verify the
; page. After the last page it verifies all 4096 bytes and jumps to F000: the
; new monitor's banner is the success message. Remove JP-WE at its first prompt.
; A byte that does not verify prints "Burn failed aaaa" (its address) and the
; program spins until RESET. Once the first byte is written the ROM may be half
; new, so nothing here calls or reads the ROM except as data, and the console
; output is OUT 00H (DEVICE_SPECS 4). With JP-WE open nothing toggles, the poll
; ends at once, and verify fails at the first byte that differs: no hang.
;
; Timing (ARCHITECTURE 6.10 rules 3 and 4), in T-states, 8080A tCY 0.48-2.0 us:
; - Page load: LOAD is 46 T from one write to the next, 92 us at the slowest
;   clock (tCY 2.0 us), inside tBLC = 150 us. It touches only RAM and the
;   EEPROM: no Pi-window access, no READY wait, interrupts never enabled.
; - Page close: from the end of the page's last write to the first poll read,
;   at least 44 + 15 * WAITN = 344 T, 165 us at the fastest clock (tCY
;   0.48 us), over tBLC (313 T). Longer is always safe. This is the one delay
;   ARCHITECTURE 6.10 rule 3 allows: the end of the write is still polled.
; - Poll: two reads of the page's last byte. Between them comes the XRA M
;   fetch from RAM, so ROM /OE goes high between the reads (I/O6 toggles on
;   /OE, AT28C64B DS 4.5). Done when bit 6 of the two reads agrees.

        CPU     8080
        ORG     0100H

WAITN   EQU     20              ; page-close wait loop count, 15 T each

START:  JMP     MAIN
SRC:    DW      1000H           ; image address

MAIN:   LHLD    SRC
        MOV     A,M
        CPI     31H             ; LXI SP
        JNZ     NOTROM
        LXI     D,6
        DAD     D
        MOV     A,M
        CPI     0F0H            ; JMP BOOT_CONTINUE, high byte, ROM build
        JNZ     NOTROM
        LHLD    SRC
        XCHG                    ; DE = source
        LXI     H,0F000H        ; HL = destination

PAGE:   PUSH    H               ; page start, destination
        PUSH    D               ; page start, source
LOAD:   LDAX    D               ;  7
        MOV     M,A             ;  7  the write
        INX     D               ;  5
        INX     H               ;  5
        MOV     A,L             ;  5
        ANI     3FH             ;  7
        JNZ     LOAD            ; 10  46 T per byte
        DCX     H               ;  5  the page's last byte
        MVI     B,WAITN         ;  7
CLOSE:  DCR     B               ;  5
        JNZ     CLOSE           ; 10
POLL:   MOV     A,M             ; I/O6 toggles on each read while the
        XRA     M               ; chip programs
        ANI     40H
        JNZ     POLL
        POP     D
        POP     H
VERIFY: LDAX    D
        CMP     M
        JNZ     FAIL
        INX     D
        INX     H
        MOV     A,L
        ANI     3FH
        JNZ     VERIFY
        MOV     A,H             ; HL wraps to 0000 after FFFF
        ORA     L
        JNZ     PAGE

        LHLD    SRC             ; all 4096 bytes again
        XCHG
        LXI     H,0F000H
CHECK:  LDAX    D
        CMP     M
        JNZ     FAIL
        INX     D
        INX     H
        MOV     A,H
        ORA     L
        JNZ     CHECK
        JMP     0F000H          ; cold start the new monitor

; FAIL: HL = the byte that did not verify. Never returns.
FAIL:   PUSH    H
        LXI     H,MSGBURN
        CALL    PUTS
        POP     H
        MOV     A,H
        CALL    HEX2
        MOV     A,L
        CALL    HEX2
        LXI     H,CRLF
        CALL    PUTS
SPIN:   JMP     SPIN            ; until RESET

NOTROM: LXI     H,MSGNOT
        JMP     PUTS            ; PUTS returns to the monitor

; HEX2: print A as two hex digits.
HEX2:   PUSH    PSW
        RRC
        RRC
        RRC
        RRC
        CALL    HEX1
        POP     PSW
HEX1:   ANI     0FH
        ADI     90H             ; 0-9 -> '0'-'9', A-F -> 'A'-'F'
        DAA
        ACI     40H
        DAA
        OUT     00H
        RET

; PUTS: print the NUL-terminated string at HL.
PUTS:   MOV     A,M
        ORA     A
        RZ
        OUT     00H
        INX     H
        JMP     PUTS

MSGNOT: DB      "Not a ROM image"
CRLF:   DB      0DH,0AH,0
MSGBURN: DB     "Burn failed ",0

        END     START
