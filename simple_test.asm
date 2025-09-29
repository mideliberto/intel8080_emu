        ORG     0000H
        
; Test basic instructions
START:  MVI     A, 42H          ; Load 42h into A
        MVI     B, 10H          ; Load 10h into B
        ADD     B               ; Add B to A
        
; Test character literals  
        DB      'H', 'E', 'L', 'L', 'O'
        DB      "World"         ; String
        DB      0FFH            ; Hex byte
        
; Test forward reference
        JMP     END_LABEL
        NOP
END_LABEL:
        HLT
