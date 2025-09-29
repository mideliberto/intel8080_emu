; ============================================
; Intel 8080 Assembler Test Suite
; Tests all opcodes, directives, and features
; ============================================

        ORG     0000H           ; Start at address 0

; ============================================
; TEST 1: NOP and HALT
; ============================================
START:  NOP                     ; 00
        NOP                     ; 00
        HLT                     ; 76

; ============================================
; TEST 2: DATA TRANSFER - MOV Instructions
; Test all 49 valid MOV combinations
; ============================================
        ORG     0010H

; MOV B,r
        MOV     B,B             ; 40
        MOV     B,C             ; 41
        MOV     B,D             ; 42
        MOV     B,E             ; 43
        MOV     B,H             ; 44
        MOV     B,L             ; 45
        MOV     B,M             ; 46
        MOV     B,A             ; 47

; MOV C,r
        MOV     C,B             ; 48
        MOV     C,C             ; 49
        MOV     C,D             ; 4A
        MOV     C,E             ; 4B
        MOV     C,H             ; 4C
        MOV     C,L             ; 4D
        MOV     C,M             ; 4E
        MOV     C,A             ; 4F

; MOV D,r
        MOV     D,B             ; 50
        MOV     D,C             ; 51
        MOV     D,D             ; 52
        MOV     D,E             ; 53
        MOV     D,H             ; 54
        MOV     D,L             ; 55
        MOV     D,M             ; 56
        MOV     D,A             ; 57

; MOV E,r
        MOV     E,B             ; 58
        MOV     E,C             ; 59
        MOV     E,D             ; 5A
        MOV     E,E             ; 5B
        MOV     E,H             ; 5C
        MOV     E,L             ; 5D
        MOV     E,M             ; 5E
        MOV     E,A             ; 5F

; MOV H,r
        MOV     H,B             ; 60
        MOV     H,C             ; 61
        MOV     H,D             ; 62
        MOV     H,E             ; 63
        MOV     H,H             ; 64
        MOV     H,L             ; 65
        MOV     H,M             ; 66
        MOV     H,A             ; 67

; MOV L,r
        MOV     L,B             ; 68
        MOV     L,C             ; 69
        MOV     L,D             ; 6A
        MOV     L,E             ; 6B
        MOV     L,H             ; 6C
        MOV     L,L             ; 6D
        MOV     L,M             ; 6E
        MOV     L,A             ; 6F

; MOV M,r
        MOV     M,B             ; 70
        MOV     M,C             ; 71
        MOV     M,D             ; 72
        MOV     M,E             ; 73
        MOV     M,H             ; 74
        MOV     M,L             ; 75
        ; Skip 76 (HLT)
        MOV     M,A             ; 77

; MOV A,r
        MOV     A,B             ; 78
        MOV     A,C             ; 79
        MOV     A,D             ; 7A
        MOV     A,E             ; 7B
        MOV     A,H             ; 7C
        MOV     A,L             ; 7D
        MOV     A,M             ; 7E
        MOV     A,A             ; 7F

; ============================================
; TEST 3: MVI - Move Immediate
; ============================================
        ORG     0080H

        MVI     B,00H           ; 06 00
        MVI     C,0FFH          ; 0E FF
        MVI     D,055H          ; 16 55
        MVI     E,0AAH          ; 1E AA
        MVI     H,012H          ; 26 12
        MVI     L,034H          ; 2E 34
        MVI     M,056H          ; 36 56
        MVI     A,078H          ; 3E 78

; ============================================
; TEST 4: LXI - Load Register Pair Immediate
; ============================================
        LXI     B,01234H        ; 01 34 12
        LXI     D,05678H        ; 11 78 56
        LXI     H,09ABCH        ; 21 BC 9A
        LXI     SP,0DEF0H       ; 31 F0 DE

; ============================================
; TEST 5: 16-bit Data Transfer
; ============================================
        LDA     01234H          ; 3A 34 12
        STA     05678H          ; 32 78 56
        LHLD    09ABCH          ; 2A BC 9A
        SHLD    0DEF0H          ; 22 F0 DE

; ============================================
; TEST 6: Register Indirect
; ============================================
        LDAX    B               ; 0A
        LDAX    D               ; 1A
        STAX    B               ; 02
        STAX    D               ; 12

; ============================================
; TEST 7: Exchange Operations
; ============================================
        XCHG                    ; EB
        XTHL                    ; E3
        SPHL                    ; F9
        PCHL                    ; E9

; ============================================
; TEST 8: ARITHMETIC - ADD Operations
; ============================================
        ORG     0100H

; ADD r
        ADD     B               ; 80
        ADD     C               ; 81
        ADD     D               ; 82
        ADD     E               ; 83
        ADD     H               ; 84
        ADD     L               ; 85
        ADD     M               ; 86
        ADD     A               ; 87

; ADC r (Add with Carry)
        ADC     B               ; 88
        ADC     C               ; 89
        ADC     D               ; 8A
        ADC     E               ; 8B
        ADC     H               ; 8C
        ADC     L               ; 8D
        ADC     M               ; 8E
        ADC     A               ; 8F

; Immediate arithmetic
        ADI     042H            ; C6 42
        ACI     055H            ; CE 55

; ============================================
; TEST 9: ARITHMETIC - SUB Operations
; ============================================
; SUB r
        SUB     B               ; 90
        SUB     C               ; 91
        SUB     D               ; 92
        SUB     E               ; 93
        SUB     H               ; 94
        SUB     L               ; 95
        SUB     M               ; 96
        SUB     A               ; 97

; SBB r (Subtract with Borrow)
        SBB     B               ; 98
        SBB     C               ; 99
        SBB     D               ; 9A
        SBB     E               ; 9B
        SBB     H               ; 9C
        SBB     L               ; 9D
        SBB     M               ; 9E
        SBB     A               ; 9F

; Immediate
        SUI     011H            ; D6 11
        SBI     022H            ; DE 22

; ============================================
; TEST 10: INCREMENT/DECREMENT
; ============================================
; INR r
        INR     B               ; 04
        INR     C               ; 0C
        INR     D               ; 14
        INR     E               ; 1C
        INR     H               ; 24
        INR     L               ; 2C
        INR     M               ; 34
        INR     A               ; 3C

; DCR r
        DCR     B               ; 05
        DCR     C               ; 0D
        DCR     D               ; 15
        DCR     E               ; 1D
        DCR     H               ; 25
        DCR     L               ; 2D
        DCR     M               ; 35
        DCR     A               ; 3D

; INX rp
        INX     B               ; 03
        INX     D               ; 13
        INX     H               ; 23
        INX     SP              ; 33

; DCX rp
        DCX     B               ; 0B
        DCX     D               ; 1B
        DCX     H               ; 2B
        DCX     SP              ; 3B

; ============================================
; TEST 11: 16-bit Arithmetic
; ============================================
        DAD     B               ; 09
        DAD     D               ; 19
        DAD     H               ; 29
        DAD     SP              ; 39

; ============================================
; TEST 12: Decimal Adjust
; ============================================
        DAA                     ; 27

; ============================================
; TEST 13: LOGICAL Operations
; ============================================
        ORG     0200H

; ANA r (AND)
        ANA     B               ; A0
        ANA     C               ; A1
        ANA     D               ; A2
        ANA     E               ; A3
        ANA     H               ; A4
        ANA     L               ; A5
        ANA     M               ; A6
        ANA     A               ; A7
        ANI     0F0H            ; E6 F0

; XRA r (XOR)
        XRA     B               ; A8
        XRA     C               ; A9
        XRA     D               ; AA
        XRA     E               ; AB
        XRA     H               ; AC
        XRA     L               ; AD
        XRA     M               ; AE
        XRA     A               ; AF
        XRI     0FFH            ; EE FF

; ORA r (OR)
        ORA     B               ; B0
        ORA     C               ; B1
        ORA     D               ; B2
        ORA     E               ; B3
        ORA     H               ; B4
        ORA     L               ; B5
        ORA     M               ; B6
        ORA     A               ; B7
        ORI     00FH            ; F6 0F

; CMP r (Compare)
        CMP     B               ; B8
        CMP     C               ; B9
        CMP     D               ; BA
        CMP     E               ; BB
        CMP     H               ; BC
        CMP     L               ; BD
        CMP     M               ; BE
        CMP     A               ; BF
        CPI     080H            ; FE 80

; ============================================
; TEST 14: ROTATE Operations
; ============================================
        RLC                     ; 07
        RRC                     ; 0F
        RAL                     ; 17
        RAR                     ; 1F

; ============================================
; TEST 15: Complement and Carry
; ============================================
        CMA                     ; 2F
        STC                     ; 37
        CMC                     ; 3F

; ============================================
; TEST 16: UNCONDITIONAL JUMPS
; ============================================
        ORG     0300H

        JMP     LABEL1          ; C3 xx xx
LABEL1: CALL    SUBRTN          ; CD xx xx
        JMP     FORWARD         ; C3 xx xx

; ============================================
; TEST 17: CONDITIONAL JUMPS
; ============================================
        JNZ     SKIP1           ; C2 xx xx
SKIP1:  JZ      SKIP2           ; CA xx xx
SKIP2:  JNC     SKIP3           ; D2 xx xx
SKIP3:  JC      SKIP4           ; DA xx xx
SKIP4:  JPO     SKIP5           ; E2 xx xx
SKIP5:  JPE     SKIP6           ; EA xx xx
SKIP6:  JP      SKIP7           ; F2 xx xx
SKIP7:  JM      SKIP8           ; FA xx xx

; ============================================
; TEST 18: CONDITIONAL CALLS
; ============================================
SKIP8:  CNZ     SUBRTN          ; C4 xx xx
        CZ      SUBRTN          ; CC xx xx
        CNC     SUBRTN          ; D4 xx xx
        CC      SUBRTN          ; DC xx xx
        CPO     SUBRTN          ; E4 xx xx
        CPE     SUBRTN          ; EC xx xx
        CP      SUBRTN          ; F4 xx xx
        CM      SUBRTN          ; FC xx xx

; ============================================
; TEST 19: RETURNS
; ============================================
SUBRTN: RET                     ; C9
        RNZ                     ; C0
        RZ                      ; C8
        RNC                     ; D0
        RC                      ; D8
        RPO                     ; E0
        RPE                     ; E8
        RP                      ; F0
        RM                      ; F8

; ============================================
; TEST 20: RESTART Instructions
; ============================================
FORWARD:
        RST     0               ; C7
        RST     1               ; CF
        RST     2               ; D7
        RST     3               ; DF
        RST     4               ; E7
        RST     5               ; EF
        RST     6               ; F7
        RST     7               ; FF

; ============================================
; TEST 21: STACK Operations
; ============================================
        ORG     0400H

        PUSH    B               ; C5
        PUSH    D               ; D5
        PUSH    H               ; E5
        PUSH    PSW             ; F5

        POP     B               ; C1
        POP     D               ; D1
        POP     H               ; E1
        POP     PSW             ; F1

; ============================================
; TEST 22: INPUT/OUTPUT
; ============================================
        IN      010H            ; DB 10
        IN      0FFH            ; DB FF
        OUT     020H            ; D3 20
        OUT     000H            ; D3 00

; ============================================
; TEST 23: INTERRUPT Control
; ============================================
        EI                      ; FB
        DI                      ; F3

; ============================================
; TEST 24: Forward References
; ============================================
        JMP     FWD_LABEL       ; Forward reference
        DB      00H
        DB      00H
FWD_LABEL:
        NOP

; ============================================
; TEST 25: Backward References
; ============================================
BACK_LABEL:
        NOP
        JMP     BACK_LABEL      ; Backward reference

; ============================================
; TEST 26: Data Definition
; ============================================
        ORG     0500H

; Single bytes
        DB      00H
        DB      0FFH
        DB      055H, 0AAH      ; Multiple bytes
        DB      'H', 'E', 'L', 'L', 'O'  ; ASCII string

; Words (16-bit, little-endian)
        DW      01234H
        DW      05678H
        DW      LABEL1          ; Label reference
        DW      START           ; Label reference

; ============================================
; TEST 27: Complex Expressions (if supported)
; ============================================
        ORG     0600H

CONST1  EQU     042H
CONST2  EQU     0100H

        MVI     A,CONST1        ; Use defined constant
        LXI     H,CONST2        ; Use 16-bit constant
        JMP     CONST2          ; Jump to constant address

; ============================================
; TEST 28: All Addressing Modes Example
; ============================================
        ORG     0700H

; Immediate
        MVI     A,055H

; Register
        MOV     B,A

; Register Indirect
        MOV     M,A             ; [HL] <- A

; Direct
        STA     01000H          ; [1000H] <- A
        LDA     01000H          ; A <- [1000H]

; ============================================
; TEST 29: Boundary Tests
; ============================================
        ORG     0FFFEH          ; Near end of memory

        NOP                     ; At FFFE
        HLT                     ; At FFFF

; ============================================
; TEST 30: Label Stress Test
; ============================================
        ORG     0800H

L1:     JMP     L2
L2:     JMP     L3
L3:     JMP     L4
L4:     JMP     L5
L5:     JMP     L6
L6:     JMP     L7
L7:     JMP     L8
L8:     JMP     L9
L9:     JMP     L10
L10:    HLT

; ============================================
; TEST 31: Numeric Formats (if supported)
; ============================================
        ORG     0900H

        DB      10              ; Decimal
        DB      10H             ; Hexadecimal  
        DB      0AH             ; Hex with leading zero
        DB      00001010B       ; Binary (if supported)
        DB      012Q            ; Octal (if supported)
        DB      012O            ; Octal alternative

; ============================================
; TEST 32: Special Cases
; ============================================

; Empty instruction (just label)
EMPTY_LABEL:

; Multiple labels on same line
MULTI1:
MULTI2: NOP

; Maximum length identifiers
VERY_LONG_LABEL_NAME_THAT_TESTS_PARSER:
        NOP

; ============================================
; TEST 33: Comment Styles
; ============================================

        NOP     ; End-of-line comment
        ; Full line comment
;       Comment with leading spaces
        NOP     ; Comment with special chars !@#$%^&*()

; ============================================
; FINAL: End of test
; ============================================
        ORG     0A00H
        
PROGRAM_END:
        HLT

        END     START           ; END directive with entry point