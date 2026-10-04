; diag3.asm - the bring-up step 3 diagnostic ROM (HARDWARE_BUILD.md 3, step 3; 3.3).
; Not a monitor program: a 4 KB EEPROM image burned in place of monitor.bin with
; the external programmer, for the board before the Pi exists. It runs from RESET
; through the overlay (F000, read at 0000), uses no stack and no console, and makes
; no access to ports 00-6F (the Pi window): with no Pi fitted one would stall the
; CPU in T_W until RESET (ARCHITECTURE 6.4 rule 4).
;
; Build: cd rom && make (rom/diag3.bin, 4096 bytes, unused bytes FF). Emulator:
; cargo run -- --rom rom/diag3.bin prints "HLT at PC=0118" (the address after the
; pass HLT); the fetch trace for the bench: HARDWARE_BUILD.md 3.3.
;
; Sequence (each check HLTs at its own address on a failure):
; 1. IN FF: bit 0 (the overlay flip-flop) must read 1 after RESET. OUT FE, then
;    IN FF: bit 0 must read 0 (DEVICE_SPECS 5; bits 1-7 are masked).
; 2. Window check: OUT 70, IN 70, OUT FD, IN FD, the ports just outside the Pi
;    window. Nothing is checked: the values are undefined (DEVICE_SPECS rule 2.4).
;    If the WAIT flip-flop sets on one of them the CPU stalls there in T_W (WAIT
;    lit, no HLT), which is the failure the bench looks for (LA-C ch 6).
; 3. March C- over 0000-EFFF, 0 = 55h and 1 = AAh: M0 up (w0), M1 up (r0,w1),
;    M2 up (r1,w0), M3 down (r0,w1), M4 down (r1,w0), M5 up (r0). It runs with
;    the overlay clear, so 0000-0FFF is RAM; an overlay that did not clear fails
;    M1 at 0000. Each read element has its own fail address.
; 4. Copy RLOOP to 0100 and jump to it. For each byte of F000-FFFF, from RAM:
;    read it, write it back, wait (below), read it again; HLT at the fail address
;    if the reads differ, else HLT at the pass address after FFFF. With JP-WE
;    open no write reaches the ROM. If one does, the second read is a polling
;    read, I/O7 complemented (AT28C64B DS 4.2, 4.4), and the loop stops at F000
;    having rewritten one byte with its own value.
;
; HLT addresses. The bench reads them off the analyzer: status A2 at the HLT's
; address, then 8A at the next address, then no further /STSTB (HARDWARE_BUILD.md
; 3.1). The emulator prints the next address ("HLT at PC=..."). On a march or
; write-back fail, HL is the failing address, and the last memory read before
; the HLT (status 82) is that address with the byte read.
;   F003  IN FF bit 0 = 0 after RESET (overlay not set)
;   F004  IN FF bit 0 = 1 after OUT FE (overlay not cleared)
;   F005  march M1: a cell did not read 55h
;   F006  march M2: a cell did not read AAh
;   F007  march M3: a cell did not read 55h
;   F008  march M4: a cell did not read AAh
;   F009  march M5: a cell did not read 55h
;   0118  ROM write-back: the second read of a ROM byte differed (HL = the byte)
;   0117  pass
;
; Timing, in T-states (2.048 MHz: 0.488 us each):
; - March: 34 T per byte for M0, 55 for M1-M4, 48 for M5: 302 T x 61440 bytes,
;   about 9.1 s.
; - Write-back: from the end of the write (MOV M,A) to the second read (MOV A,M,
;   after its 4 T fetch), 5 + 7 + 15 * WAITN + 4 = 331 T, 159 us at the fastest
;   legal clock (tCY 0.48 us), over tBLC = 150 us (313 T): the page load has
;   closed, so that read is a polling read under either reading of the datasheet
;   (ARCHITECTURE 6.10 rule 3, bench item K-1). 386 T per byte, about 0.77 s for
;   the 4096 bytes.
;
; Emulator behavior it stresses: the reset path through the overlay (the fetch at
; 0000 from ROM, the jump to F000-FFFF, OUT FE clearing the overlay so 0000-0FFF
; reads RAM), IN FF and OUT FE handled in the CPU, ports outside the Pi window
; reading FF with no device, writes to F000-FFFF reaching neither ROM nor any
; readable RAM, the fitted JP-WE model (one write, then status on the read after
; tBLC), INX/DCX wrap at FFFF/0000, and the cycle counts of the wait loop.

        CPU     8080
        ORG     0F000H

WAITN   EQU     21              ; write-back wait loop count, 15 T each

        JMP     START           ; F000, fetched at 0000: leave the mirror

; Fail table: one HLT each, at the fixed addresses of the header.
F_OVL1: HLT                     ; F003
F_OVL0: HLT                     ; F004
F_M1:   HLT                     ; F005
F_M2:   HLT                     ; F006
F_M3:   HLT                     ; F007
F_M4:   HLT                     ; F008
F_M5:   HLT                     ; F009

START:  IN      0FFH
        ANI     01H
        JZ      F_OVL1          ; overlay not set by RESET
        OUT     0FEH
        IN      0FFH
        ANI     01H
        JNZ     F_OVL0          ; OUT FE did not clear it
        OUT     70H             ; window check
        IN      70H
        OUT     0FDH
        IN      0FDH

        MVI     B,55H           ; march 0
        MVI     C,0AAH          ; march 1
        LXI     H,0000H
M0:     MOV     M,B             ; up (w0)
        INX     H
        MOV     A,H
        CPI     0F0H
        JNZ     M0
        LXI     H,0000H
M1:     MOV     A,M             ; up (r0,w1)
        CMP     B
        JNZ     F_M1
        MOV     M,C
        INX     H
        MOV     A,H
        CPI     0F0H
        JNZ     M1
        LXI     H,0000H
M2:     MOV     A,M             ; up (r1,w0)
        CMP     C
        JNZ     F_M2
        MOV     M,B
        INX     H
        MOV     A,H
        CPI     0F0H
        JNZ     M2
        LXI     H,0EFFFH
M3:     MOV     A,M             ; down (r0,w1)
        CMP     B
        JNZ     F_M3
        MOV     M,C
        DCX     H
        MOV     A,H
        CPI     0FFH            ; HL wrapped from 0000
        JNZ     M3
        LXI     H,0EFFFH
M4:     MOV     A,M             ; down (r1,w0)
        CMP     C
        JNZ     F_M4
        MOV     M,B
        DCX     H
        MOV     A,H
        CPI     0FFH
        JNZ     M4
        LXI     H,0000H
M5:     MOV     A,M             ; up (r0)
        CMP     B
        JNZ     F_M5
        INX     H
        MOV     A,H
        CPI     0F0H
        JNZ     M5

        LXI     H,RIMG          ; RAM passed: copy RLOOP to 0100
        LXI     D,RLOOP
        MVI     B,RLEN
COPY:   MOV     A,M
        STAX    D
        INX     H
        INX     D
        DCR     B
        JNZ     COPY
        JMP     RLOOP

; The write-back loop, assembled for 0100, stored here. Runs from RAM: while a
; write cycle runs, ROM reads are status, not code (ARCHITECTURE 6.10 rule 2).
RIMG:
        PHASE   0100H
RLOOP:  LXI     H,0F000H
NEXT:   MOV     A,M             ;  7  first read
        MOV     M,A             ;  7  write it back
        MOV     B,A             ;  5
        MVI     C,WAITN         ;  7
WAIT:   DCR     C               ;  5
        JNZ     WAIT            ; 10
        MOV     A,M             ;  7  second read, after its 4 T fetch
        CMP     B               ;  4
        JNZ     RFAIL           ; 10
        INX     H               ;  5
        MOV     A,H             ;  5
        ORA     L               ;  4  HL wraps to 0000 after FFFF
        JNZ     NEXT            ; 10  386 T per byte
PASS:   HLT                     ; 0117
RFAIL:  HLT                     ; 0118
REND:
        DEPHASE

RLEN    EQU     REND-RLOOP
DIAG_END:                       ; bytes used: DIAG_END - F000
        END
