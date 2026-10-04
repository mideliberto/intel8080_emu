// gal_tests.rs - The GAL fuse map against the board's decode (ARCHITECTURE 6.2-6.5).
// hw/glue.jed is the file the programmer burns. It is evaluated for all 2^16 input
// vectors under the pin list in hw/glue.pld and compared with a reference model:
// the emulator's own memory and FE/FF decode, plus the ARCHITECTURE 6.3-6.5 terms
// the emulator does not model (the 00-6F window, STSTB, D4/D6, RESET). The .pld
// equations are never read; galette is not needed here (cd hw && make rebuilds the
// .jed).
//
// Fuse map: the GAL22V10 JEDEC layout (5892 fuses), which the ATF22V10C uses.
// 132 rows x 44 columns, fuse = row x 44 + column. Row 0 is AR, rows 1-130 are the
// ten OLMC blocks (OE row first, then the product rows), row 131 is SP. Then the
// (S0, S1) pair of each OLMC from pin 23 down, then 64 signature bits. Each pin owns
// a column pair: c is the true literal, c + 1 the complement, and fuse 0 (intact)
// puts the literal in the row. Not proven here: that the silicon reads the map the
// same way. That is bring-up step 3 and 4 (HARDWARE_BUILD 3).

use intel8080_emu::Intel8080;

const JED: &str = include_str!("../hw/glue.jed");
const PLD: &str = include_str!("../hw/glue.pld");

const FUSES: usize = 5892;
const COLS: usize = 44;
// Column of each pin's true literal (index = DIP pin 1-24); None for GND and VCC.
const COL: [Option<usize>; 25] = [
    None, Some(0), Some(4), Some(8), Some(12), Some(16), Some(20), Some(24), Some(28),
    Some(32), Some(36), Some(40), None, Some(42), Some(38), Some(34), Some(30), Some(26),
    Some(22), Some(18), Some(14), Some(10), Some(6), Some(2), None,
];
// OLMC of I/O pin 14 + i: its OE row, and its row count including the OE row.
const OE_ROW: [usize; 10] = [122, 111, 98, 83, 66, 49, 34, 21, 10, 1];
const ROWS: [usize; 10] = [9, 11, 13, 15, 17, 17, 15, 13, 11, 9];

#[derive(Clone, Copy, PartialEq, Debug)]
enum Class {
    Cpu,    // driven by the 8080A or the 8224: dedicated input pins (1-11, 13) only
    Input,  // any other input: any input pin
    Output, // I/O pins (14-23) only
}
use Class::*;

// The 22 signal names, polarity included (/X = active-low net).
const NAMES: [(&str, Class); 22] = [
    ("A8", Cpu), ("A9", Cpu), ("A10", Cpu), ("A11", Cpu),
    ("A12", Cpu), ("A13", Cpu), ("A14", Cpu), ("A15", Cpu),
    ("D4", Cpu), ("D6", Cpu), ("/STSTB", Cpu),
    ("OVL", Input), ("/MEMR", Input), ("/IOR", Input), ("/IOW", Input), ("/RESET", Input),
    ("/ROMOE", Output), ("/RAMOE", Output), ("DB0", Output),
    ("/OVLCLR", Output), ("/WSET", Output), ("/LATOE", Output),
];

/// The pin list: the first 24 tokens after the two header lines, comments stripped.
fn pins() -> [&'static str; 25] {
    let tokens: Vec<&str> = PLD.lines().skip(2)
        .map(|l| l.split(';').next().unwrap())
        .flat_map(str::split_whitespace)
        .take(24)
        .collect();
    let mut p = [""; 25];
    p[1..].copy_from_slice(&tokens);
    p
}

fn class(name: &str) -> Class {
    NAMES.iter().find(|n| n.0 == name).unwrap_or_else(|| panic!("unknown pin name {name}")).1
}

/// The fuse array from the JEDEC text between STX and ETX. true = blown.
fn fuses() -> Vec<bool> {
    let body = &JED[JED.find('\x02').expect("STX") + 1..JED.find('\x03').expect("ETX")];
    let mut default = false;
    let mut f = Vec::new();
    for field in body.split('*').skip(1).map(str::trim) {
        if field == format!("QF{FUSES}") {
            f = vec![default; FUSES];
        } else if let Some(d) = field.strip_prefix('F') {
            default = d == "1";
        } else if let Some(rest) = field.strip_prefix('L') {
            let mut it = rest.split_whitespace();
            let at: usize = it.next().unwrap().parse().unwrap();
            for (i, b) in it.collect::<String>().chars().enumerate() {
                f[at + i] = b == '1';
            }
        }
    }
    assert_eq!(f.len(), FUSES, "QF{FUSES} missing: a 5893-fuse file makes pin 4 (A10) the power-down pin");
    f
}

fn row_true(f: &[bool], row: usize, level: &[bool; 25]) -> bool {
    (1..=24).all(|p| match COL[p] {
        None => true,
        Some(c) => (f[row * COLS + c] || level[p]) && (f[row * COLS + c + 1] || !level[p]),
    })
}

fn s0(f: &[bool], pin: usize) -> bool { f[5808 + 2 * (23 - pin)] }
fn s1(f: &[bool], pin: usize) -> bool { f[5809 + 2 * (23 - pin)] }

/// The decode the emulator implements, by probing it. rom_sel[ovl][hi]: a memory read
/// at hi:xx selects the ROM. fe_write[p]: OUT p clears the overlay. ff_read[p]: IN p
/// returns the overlay in bit 0. Nothing is mapped on the IoBus, so every other port
/// reads FF and never touches the overlay.
struct Decode {
    rom_sel: [[bool; 256]; 2],
    fe_write: [bool; 256],
    ff_read: [bool; 256],
}

fn emulator_decode() -> Decode {
    let mut d = Decode { rom_sel: [[false; 256]; 2], fe_write: [false; 256], ff_read: [false; 256] };
    let mut cpu = Intel8080::new();
    cpu.load_rom(&[0xA5; 4096]);
    cpu.load_program(&vec![0x5A; 0x10000], 0);
    for ovl in [false, true] {
        cpu.rom_overlay_enabled = ovl;
        for addr in 0..=0xFFFFu16 {
            // ROM or RAM, nothing else: an open-bus FF (an unmapped decode, or an overlay window
            // past the 4 KB image) is neither.
            let byte = cpu.read_byte(addr);
            assert!(byte == 0xA5 || byte == 0x5A, "read of {addr:04X} (overlay {ovl}) is {byte:02X}: neither ROM nor RAM");
            let rom = byte == 0xA5;
            let hi = (addr >> 8) as usize;
            if addr & 0xFF == 0 {
                d.rom_sel[ovl as usize][hi] = rom;
            }
            // The GAL sees A15-A8 only.
            assert_eq!(rom, d.rom_sel[ovl as usize][hi], "ROM select depends on A7-A0 at {addr:04X}");
        }
    }
    let run = |op: u8, port: u8, ovl: bool| {
        let mut c = Intel8080::new();
        c.load_program(&[op, port], 0x8000);
        c.pc = 0x8000;
        c.rom_overlay_enabled = ovl;
        c.execute_one();
        c
    };
    for p in 0..=255u8 {
        d.fe_write[p as usize] = !run(0xD3, p, true).rom_overlay_enabled;
        d.ff_read[p as usize] = (run(0xDB, p, true).a ^ run(0xDB, p, false).a) & 1 == 1;
    }
    d
}

/// Expected level of the pin named `name` (None = not driven), given the input levels.
fn expected(d: &Decode, level: &dyn Fn(&str) -> bool, name: &str) -> Option<bool> {
    let hi = (0..8).fold(0usize, |b, i| b | (level(&format!("A{}", 8 + i)) as usize) << i);
    let ovl = level("OVL");
    let (memr, ior, iow, ststb) = (!level("/MEMR"), !level("/IOR"), !level("/IOW"), !level("/STSTB"));
    let reset = !level("/RESET");
    let rom_sel = d.rom_sel[ovl as usize][hi];
    let window = hi <= 0x6F; // DEVICE_SPECS 1, ARCHITECTURE 6.3: every port 00-6F is the Pi's
    match name {
        "/ROMOE" => Some(!(memr && rom_sel)),                                   // 6.2
        "/RAMOE" => Some(!memr || rom_sel),                                     // 6.2, NOT (MEMR AND NOT ROM_SEL)
        "/WSET" => Some(!(ststb && !reset && (level("D4") || level("D6")) && window)), // 6.4 rule 1
        "/OVLCLR" => Some(!(iow && d.fe_write[hi])),                            // 6.5
        "DB0" => (ior && !reset && d.ff_read[hi]).then_some(ovl),               // 6.5
        "/LATOE" => Some(!(ior && !reset && window)),                           // 6.4 IN cycle
        _ => None, // an input on an I/O pin: never driven
    }
}

#[test]
fn gal_pin_list_classes() {
    let p = pins();
    assert_eq!((p[12], p[24]), ("GND", "VCC"), "pins 12 and 24");
    for (name, cls) in NAMES {
        let at: Vec<usize> = (1..=24).filter(|&i| p[i] == name).collect();
        assert_eq!(at.len(), 1, "{name} must appear exactly once in the pin list, found at {at:?}");
        match cls {
            Cpu => assert!(at[0] <= 13, "{name} is CPU-driven: dedicated input pin only, not pin {}", at[0]),
            Output => assert!(at[0] >= 14, "{name} is an output: I/O pin only, not pin {}", at[0]),
            Input => {}
        }
    }
}

#[test]
fn gal_jed_structure() {
    let body = &JED[JED.find('\x02').unwrap()..];
    assert!(body.split('*').any(|f| f.trim() == "G0"), "security fuse must be off (G0) so the programmer can verify");
    let f = fuses();
    let p = pins();
    for (pin, name) in p.iter().enumerate().skip(14).take(10) {
        assert!(s1(&f, pin), "pin {pin} ({name}): OLMC registered or unused (S1 = 0)");
    }
    // No live row reads an output pin, so one evaluation pass is exact.
    for row in 0..132 {
        if (0..COLS).any(|c| f[row * COLS + c]) {
            for pin in (14..=23).filter(|&i| class(p[i]) == Output) {
                let c = COL[pin].unwrap();
                assert!(f[row * COLS + c] && f[row * COLS + c + 1], "row {row} reads output pin {pin} ({})", p[pin]);
            }
        }
    }
}

#[test]
fn emulator_agrees_with_architecture() {
    let d = emulator_decode();
    for ovl in [false, true] {
        for hi in 0..256 {
            let arch = hi >> 4 == 0xF || (ovl && hi >> 4 == 0); // ARCHITECTURE 6.2 ROM_SEL
            assert_eq!(d.rom_sel[ovl as usize][hi], arch, "ROM_SEL at {hi:02X}xx ovl={ovl}");
        }
    }
    let ports = |t: &[bool; 256]| (0..=255u8).filter(|&p| t[p as usize]).collect::<Vec<_>>();
    assert_eq!(ports(&d.fe_write), [0xFE], "OUT that clears the overlay (6.5)");
    assert_eq!(ports(&d.ff_read), [0xFF], "IN that reads the overlay (6.5)");
}

#[test]
fn gal_jed_matches_model() {
    let d = emulator_decode();
    let f = fuses();
    let p = pins();
    let inputs: Vec<usize> = (1..=24).filter(|&i| i != 12 && i != 24 && class(p[i]) != Output).collect();
    assert_eq!(inputs.len(), 16);
    let mut bad = Vec::new();
    for v in 0..1u32 << 16 {
        let mut level = [false; 25];
        for (k, &i) in inputs.iter().enumerate() {
            level[i] = v >> k & 1 == 1;
        }
        let by_name = |n: &str| level[(1..=24).find(|&i| p[i] == n).unwrap()];
        for (pin, name) in p.iter().enumerate().skip(14).take(10) {
            let o = pin - 14;
            let oe = row_true(&f, OE_ROW[o], &level);
            let sum = (1..ROWS[o]).any(|k| row_true(&f, OE_ROW[o] + k, &level));
            let got = oe.then_some(sum == s0(&f, pin));
            let want = expected(&d, &by_name, name);
            if got != want {
                bad.push(format!("v={v:04X} pin {pin} ({name}): got {got:?} want {want:?}"));
            }
        }
    }
    assert!(bad.is_empty(), "{} mismatches, first: {:#?}", bad.len(), &bad[..bad.len().min(8)]);
}
