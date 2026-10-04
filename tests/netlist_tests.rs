// netlist_tests.rs - The board netlist against ARCHITECTURE 6 (HARDWARE_BUILD 2.2).
// hw/board.net.txt is the only home of pin numbers. ARCHITECTURE 6 stays normative for the
// circuits; every circuit check here is pad-anchored: it names pads (ref.PIN through the type
// tables below, or ref.N), never a net name other than a rail, so a swap that keeps the net
// labels cannot pass. Every error starts with its check ID; checker_catches_mutations holds one
// mutant per ID.
//
// The type tables are single-source: S3 proves the netlist agrees with them, not that they agree
// with silicon. Each cites its pinout source; Mike checks them against the datasheets before fab
// (HARDWARE_BUILD 2.2, fab gate). GAL pins come from hw/glue.pld, Pi BCM signals from src/pi.
//
// The test also writes the KiCad netlist, reads it back and compares it with the committed
// hw/board.kicad.net (the file Pcbnew imports), smoke-tests the KiCad project files, and checks
// hw/board.kicad_pcb against the netlist once the board has footprints (it skips until then).
// docs/PARTS_ORDER.md is checked against the netlist too: every refdes on exactly one order line.

use intel8080_emu::pi::{ACK, A_SHIFT, DIR, D_SHIFT, LATCH, PINS, REQ, RESET};
use std::collections::{BTreeMap, BTreeSet, HashMap};

const NETLIST: &str = include_str!("../hw/board.net.txt");
const PLD: &str = include_str!("../hw/glue.pld");
const KICAD_NET: &str = include_str!("../hw/board.kicad.net");
const PCB: &str = include_str!("../hw/board.kicad_pcb");
const DRU: &str = include_str!("../hw/board.kicad_dru");
const PRO: &str = include_str!("../hw/board.kicad_pro");
const PARTS_ORDER: &str = include_str!("../docs/PARTS_ORDER.md");

const RAILS: [&str; 6] = ["GND", "+5V", "+12V", "-5V", "+3V3_PI", "+5V_IN"];
/// TEST_RESET (decision PI-GPIO). Not in src/pi: v1 never drives it (PI_DAEMON 15).
const TEST_RESET_BCM: u32 = 18;

#[derive(Clone, Copy, PartialEq, Debug)]
enum Kind {
    In,
    Out,
    Tri,
    Io,
    Od,
    Pwr(&'static str), // power input, on this rail
    Src(&'static str), // power output, on this rail
    Pas,
    Probe, // measurement pin: neither driver nor load
    Nc,    // must stay unconnected
}
use Kind::*;

struct Type {
    fp: &'static str,
    pins: Vec<(String, String, Kind)>, // pad, name ('~' = unnamed), kind
}

// Type tables: pad name kind, in triples. Footprints are KiCad 8 standard-library names,
// UNVERIFIED until the first Pcbnew import (HARDWARE_BUILD 2.2).
const TYPES: &[(&str, &str, &str)] = &[
    // reference/8080_HARDWARE.md 2 (MCS-80 UM p.5-14).
    ("8080A", "Package_DIP:DIP-40_W15.24mm_Socket", "
        1 A10 tri  2 VSS pwr:GND  3 D4 io  4 D5 io  5 D6 io  6 D7 io  7 D3 io  8 D2 io  9 D1 io
        10 D0 io  11 VBB pwr:-5V  12 RESET in  13 HOLD in  14 INT in  15 PHI2 in  16 INTE out
        17 DBIN out  18 WR_N out  19 SYNC out  20 VCC pwr:+5V  21 HLDA out  22 PHI1 in
        23 READY in  24 WAIT out  25 A0 tri  26 A1 tri  27 A2 tri  28 VDD pwr:+12V  29 A3 tri
        30 A4 tri  31 A5 tri  32 A6 tri  33 A7 tri  34 A8 tri  35 A9 tri  36 A15 tri  37 A12 tri
        38 A13 tri  39 A14 tri  40 A11 tri"),
    // reference/8080_HARDWARE.md 11.1 (p.5-1). XTAL pins are the crystal's, passive.
    ("8224", "Package_DIP:DIP-16_W7.62mm_Socket", "
        1 RESET out  2 RESIN_N in  3 RDYIN in  4 READY out  5 SYNC in  6 PHI2_TTL out
        7 STSTB_N out  8 GND pwr:GND  9 VDD pwr:+12V  10 PHI2 out  11 PHI1 out  12 OSC out
        13 TANK in  14 XTAL2 pas  15 XTAL1 pas  16 VCC pwr:+5V"),
    // reference/8080_HARDWARE.md 12.1 (p.5-7).
    ("8228", "Package_DIP:DIP-28_W15.24mm_Socket", "
        1 STSTB_N in  2 HLDA in  3 WR_N in  4 DBIN in  5 DB4 io  6 D4 io  7 DB7 io  8 D7 io
        9 DB3 io  10 D3 io  11 DB2 io  12 D2 io  13 DB0 io  14 GND pwr:GND  15 D0 io  16 DB1 io
        17 D1 io  18 DB5 io  19 D5 io  20 DB6 io  21 D6 io  22 BUSEN_N in  23 INTA_N od
        24 MEMR_N out  25 IOR_N out  26 MEMW_N out  27 IOW_N out  28 VCC pwr:+5V"),
    // AT28C64B DS (Atmel 0270L), pin configuration. Pin 1 is RDY/BSY or NC by die.
    ("AT28C64B", "Package_DIP:DIP-28_W15.24mm_Socket", "
        1 RDY_BSY od  2 A12 in  3 A7 in  4 A6 in  5 A5 in  6 A4 in  7 A3 in  8 A2 in  9 A1 in
        10 A0 in  11 IO0 tri  12 IO1 tri  13 IO2 tri  14 GND pwr:GND  15 IO3 tri  16 IO4 tri
        17 IO5 tri  18 IO6 tri  19 IO7 tri  20 CE_N in  21 A10 in  22 OE_N in  23 A11 in
        24 A9 in  25 A8 in  26 NC nc  27 WE_N in  28 VCC pwr:+5V"),
    // Alliance AS6C62256 v1.0 p.1 (JEDEC 62256), 600 mil PDIP.
    ("AS6C62256", "Package_DIP:DIP-28_W15.24mm_Socket", "
        1 A14 in  2 A12 in  3 A7 in  4 A6 in  5 A5 in  6 A4 in  7 A3 in  8 A2 in  9 A1 in
        10 A0 in  11 DQ0 tri  12 DQ1 tri  13 DQ2 tri  14 VSS pwr:GND  15 DQ3 tri  16 DQ4 tri
        17 DQ5 tri  18 DQ6 tri  19 DQ7 tri  20 CE_N in  21 A10 in  22 OE_N in  23 A11 in
        24 A9 in  25 A8 in  26 A13 in  27 WE_N in  28 VCC pwr:+5V"),
    // TI SCLS169G (SN74HCT74) and SCHS409 (CD74HCT74), same pinout.
    ("74HCT74", "Package_DIP:DIP-14_W7.62mm_Socket", "
        1 1CLR_N in  2 1D in  3 1CLK in  4 1PRE_N in  5 1Q out  6 1Q_N out  7 GND pwr:GND
        8 2Q_N out  9 2Q out  10 2PRE_N in  11 2CLK in  12 2D in  13 2CLR_N in  14 VCC pwr:+5V"),
    // TI SN74HCT14 data sheet, pin configuration.
    ("74HCT14", "Package_DIP:DIP-14_W7.62mm_Socket", "
        1 1A in  2 1Y out  3 2A in  4 2Y out  5 3A in  6 3Y out  7 GND pwr:GND  8 4Y out
        9 4A in  10 5Y out  11 5A in  12 6Y out  13 6A in  14 VCC pwr:+5V"),
    // TI SCLS063G (SN74HCT08).
    ("74HCT08", "Package_DIP:DIP-14_W7.62mm_Socket", "
        1 1A in  2 1B in  3 1Y out  4 2A in  5 2B in  6 2Y out  7 GND pwr:GND  8 3Y out
        9 3A in  10 3B in  11 4Y out  12 4A in  13 4B in  14 VCC pwr:+5V"),
    // TI SCLS171F (SN74HCT138).
    ("74HCT138", "Package_DIP:DIP-16_W7.62mm_Socket", "
        1 A in  2 B in  3 C in  4 G2A_N in  5 G2B_N in  6 G1 in  7 Y7_N out  8 GND pwr:GND
        9 Y6_N out  10 Y5_N out  11 Y4_N out  12 Y3_N out  13 Y2_N out  14 Y1_N out
        15 Y0_N out  16 VCC pwr:+5V"),
    // TI SCLS005E (SN74HCT374).
    ("74HCT374", "Package_DIP:DIP-20_W7.62mm_Socket", "
        1 OE_N in  2 1Q tri  3 1D in  4 2D in  5 2Q tri  6 3Q tri  7 3D in  8 4D in  9 4Q tri
        10 GND pwr:GND  11 CLK in  12 5Q tri  13 5D in  14 6D in  15 6Q tri  16 7Q tri
        17 7D in  18 8D in  19 8Q tri  20 VCC pwr:+5V"),
    // TI SCAS218X (SN74LVC245A). With DIR high (A to B, C64) A is input, B is output.
    ("74LVC245A", "Package_DIP:DIP-20_W7.62mm_Socket", "
        1 DIR in  2 A1 in  3 A2 in  4 A3 in  5 A4 in  6 A5 in  7 A6 in  8 A7 in  9 A8 in
        10 GND pwr:GND  11 B8 tri  12 B7 tri  13 B6 tri  14 B5 tri  15 B4 tri  16 B3 tri
        17 B2 tri  18 B1 tri  19 OE_N in  20 VCC pwr:+3V3_PI"),
    // Intersil FN3072 (ICL7660: pin 1 NC) and the Maxim ICL7660/MAX1044 data sheet (pin 1 BOOST,
    // open = normal). Same pinout otherwise.
    ("7660", "Package_DIP:DIP-8_W7.62mm_Socket", "
        1 BOOST nc  2 CAPP pas  3 GND pwr:GND  4 CAPN pas  5 VOUT src:-5V  6 LV in  7 OSC in
        8 VPLUS pwr:+5V"),
    // Dallas/Maxim DS1813 rev 022306, TO-92: 1 RST, 2 VCC, 3 GND. Open drain, internal pull-up.
    ("DS1813", "Package_TO_SOT_THT:TO-92_Inline", "1 RST_N od  2 VCC pwr:+5V  3 GND pwr:GND"),
    // onsemi 2N3904, TO-92: 1 E, 2 B, 3 C.
    ("2N3904", "Package_TO_SOT_THT:TO-92_Inline", "1 E pwr:GND  2 B in  3 C od"),
    // Vishay SUP53P06-20 (doc 68633 rev C) p.1: TO-220AB G D S, drain on the tab.
    ("PFET_TO220", "Package_TO_SOT_THT:TO-220-3_Vertical", "1 G in  2 D pas  3 S pas"),
    // Pololu #4016 product page: VIN and GND doubled, VOUT, EN (30k pull-up to VIN), no PG.
    // The pad ORDER is UNVERIFIED: not in the page text (HARDWARE_BUILD 2.2).
    ("U3V40F12", "Connector_PinHeader_2.54mm:PinHeader_1x06_P2.54mm_Vertical", "
        1 EN in  2 VIN pwr:+5V  3 VIN pwr:+5V  4 GND pwr:GND  5 GND pwr:GND  6 VOUT src:+12V"),
    // 2.1 mm DC jack: 1 centre pin, 2 sleeve, 3 sleeve switch. Match the bought jack at import.
    ("DCJACK", "Connector_BarrelJack:BarrelJack_Horizontal", "
        1 VIN src:+5V_IN  2 GND pwr:GND  3 SW pas"),
    ("Y", "Crystal:Crystal_HC49-4H_Vertical", "1 ~ pas  2 ~ pas"),
    ("R", "Resistor_THT:R_Axial_DIN0207_L6.3mm_D2.5mm_P10.16mm_Horizontal", "1 ~ pas  2 ~ pas"),
    ("C", "Capacitor_THT:C_Disc_D5.0mm_W2.5mm_P5.00mm", "1 ~ pas  2 ~ pas"),
    ("CP", "Capacitor_THT:CP_Radial_D5.0mm_P2.00mm", "1 + pas  2 - pas"),
    // KiCad diode and LED footprints: pad 1 = cathode.
    ("D", "Diode_THT:D_DO-35_SOD27_P7.62mm_Horizontal", "1 K pas  2 A pas"),
    ("LED", "LED_THT:LED_D3.0mm", "1 K pas  2 A pas"),
    // Bused SIP: pin 1 common. Isolated DIP-16: resistor k from pin k to pin 17 - k.
    ("RN_SIP9", "Resistor_THT:R_Array_SIP9", "
        1 COM pas  2 R1 pas  3 R2 pas  4 R3 pas  5 R4 pas  6 R5 pas  7 R6 pas  8 R7 pas  9 R8 pas"),
    ("RN_DIP16", "Package_DIP:DIP-16_W7.62mm", "
        1 1a pas  2 2a pas  3 3a pas  4 4a pas  5 5a pas  6 6a pas  7 7a pas  8 8a pas
        9 8b pas  10 7b pas  11 6b pas  12 5b pas  13 4b pas  14 3b pas  15 2b pas  16 1b pas"),
    ("JMP2", "Connector_PinHeader_2.54mm:PinHeader_1x02_P2.54mm_Vertical", "1 ~ pas  2 ~ pas"),
    // KiCad SW_PUSH_6mm has four pads numbered 1, 2, 1, 2; the netlist lists each number once.
    ("SW", "Button_Switch_THT:SW_PUSH_6mm", "1 ~ pas  2 ~ pas"),
    ("TP", "Connector_PinHeader_2.54mm:PinHeader_1x01_P2.54mm_Vertical", "1 ~ probe"),
    ("HOLE", "MountingHole:MountingHole_3.2mm_M3", ""),
    // Raspberry Pi 40-pin header: raspberrypi.com, "GPIO and the 40-pin header". The physical
    // pin to BCM map is the only hand table; which BCM carries which signal is src/pi.
    ("PI_HDR", "Connector_IDC:IDC-Header_2x20_P2.54mm_Vertical", "
        1 3V3 src:+3V3_PI  2 5V nc  3 BCM2 io  4 5V nc  5 BCM3 io  6 GND pwr:GND  7 BCM4 io
        8 BCM14 io  9 GND pwr:GND  10 BCM15 io  11 BCM17 io  12 BCM18 io  13 BCM27 io
        14 GND pwr:GND  15 BCM22 io  16 BCM23 io  17 3V3 src:+3V3_PI  18 BCM24 io  19 BCM10 io
        20 GND pwr:GND  21 BCM9 io  22 BCM25 io  23 BCM11 io  24 BCM8 io  25 GND pwr:GND
        26 BCM7 io  27 BCM0 io  28 BCM1 io  29 BCM5 io  30 GND pwr:GND  31 BCM6 io  32 BCM12 io
        33 BCM13 io  34 GND pwr:GND  35 BCM19 io  36 BCM16 io  37 BCM26 io  38 BCM20 io
        39 GND pwr:GND  40 BCM21 io"),
    // GND on pins 17-20: decision LA-HEADER (HARDWARE_BUILD 1). Pin n = channel n - 1 is this
    // table's own convention; the channel order is HARDWARE_BUILD 3.1, checked in C612.
    ("LA_HDR", "Connector_IDC:IDC-Header_2x10_P2.54mm_Vertical", "
        1 CH0 probe  2 CH1 probe  3 CH2 probe  4 CH3 probe  5 CH4 probe  6 CH5 probe  7 CH6 probe
        8 CH7 probe  9 CH8 probe  10 CH9 probe  11 CH10 probe  12 CH11 probe  13 CH12 probe
        14 CH13 probe  15 CH14 probe  16 CH15 probe  17 GND pwr:GND  18 GND pwr:GND
        19 GND pwr:GND  20 GND pwr:GND"),
];

fn kind(s: &'static str) -> Kind {
    match s {
        "in" => In,
        "out" => Out,
        "tri" => Tri,
        "io" => Io,
        "od" => Od,
        "pas" => Pas,
        "probe" => Probe,
        "nc" => Nc,
        _ => match (s.strip_prefix("pwr:"), s.strip_prefix("src:")) {
            (Some(r), _) => Pwr(r),
            (_, Some(r)) => Src(r),
            _ => panic!("unknown pin kind {s}"),
        },
    }
}

/// The GAL's table from hw/glue.pld: the pin list (the first 24 tokens after the two header
/// lines, as tests/gal_tests.rs reads it), /X as X_N. A name on the left of an equation is an
/// output, three-state if it has a .E term.
fn gal_pins() -> Vec<(String, String, Kind)> {
    let names: Vec<&str> = PLD.lines().skip(2)
        .map(|l| l.split(';').next().unwrap())
        .flat_map(str::split_whitespace)
        .take(24)
        .collect();
    let mut outs: HashMap<&str, bool> = HashMap::new();
    for l in PLD.lines().skip(2).take_while(|l| l.trim() != "DESCRIPTION") {
        if let Some((lhs, _)) = l.split(';').next().unwrap().split_once('=') {
            let lhs = lhs.trim().trim_start_matches('/');
            let (name, ext) = lhs.split_once('.').unwrap_or((lhs, ""));
            *outs.entry(name).or_insert(false) |= ext == "E";
        }
    }
    names.iter().enumerate().map(|(i, &n)| {
        let base = n.trim_start_matches('/');
        let k = match (n, outs.get(base)) {
            ("GND", _) => Pwr("GND"),
            ("VCC", _) => Pwr("+5V"),
            (_, Some(true)) => Tri,
            (_, Some(false)) => Out,
            _ => In,
        };
        let name = if n.starts_with('/') { format!("{base}_N") } else { n.to_string() };
        ((i + 1).to_string(), name, k)
    }).collect()
}

fn types() -> HashMap<&'static str, Type> {
    let mut t = HashMap::new();
    for &(name, fp, table) in TYPES {
        let tok: Vec<&'static str> = table.split_whitespace().collect();
        assert!(tok.len() % 3 == 0, "type {name}: table is not in triples");
        let pins = tok.chunks(3).map(|c| (c[0].to_string(), c[1].to_string(), kind(c[2]))).collect();
        t.insert(name, Type { fp, pins });
    }
    t.insert("ATF22V10C", Type { fp: "Package_DIP:DIP-24_W7.62mm_Socket", pins: gal_pins() });
    t
}

// ---------------------------------------------------------------- netlist parser

struct Part {
    r: String,
    ty: String,
    value: String,
    pads: Vec<(String, String, String, usize)>, // pad, pin name, net, line
}

fn parse(text: &str) -> (Vec<Part>, Vec<String>) {
    let mut parts: Vec<Part> = Vec::new();
    let mut errs = Vec::new();
    for (i, raw) in text.lines().enumerate() {
        let t: Vec<&str> = raw.split('#').next().unwrap().split_whitespace().collect();
        match t.as_slice() {
            [] => {}
            ["part", r, ty, value] => parts.push(Part {
                r: r.to_string(), ty: ty.to_string(), value: value.to_string(), pads: Vec::new(),
            }),
            [pad, name, net] if *pad != "part" && !parts.is_empty() => {
                parts.last_mut().unwrap().pads.push((pad.to_string(), name.to_string(), net.to_string(), i + 1));
            }
            _ => errs.push(format!("S1 line {}: cannot parse '{}'", i + 1, raw.trim())),
        }
    }
    (parts, errs)
}

/// Natural order: U2 before U10, pad 2 before pad 10.
fn nat(s: &str) -> (String, u64, String) {
    let alpha: String = s.chars().take_while(|c| !c.is_ascii_digit()).collect();
    let rest = &s[alpha.len()..];
    let digits: String = rest.chars().take_while(|c| c.is_ascii_digit()).collect();
    (alpha, digits.parse().unwrap_or(0), rest[digits.len()..].to_string())
}

// ---------------------------------------------------------------- the checker

type Pad = (String, String); // (ref, pad number)

struct Ck<'a> {
    parts: &'a [Part],
    types: &'a HashMap<&'static str, Type>,
    ix: HashMap<String, usize>,
    net_of: HashMap<Pad, String>,
    nets: BTreeMap<String, Vec<Pad>>, // NC excluded
    errs: Vec<String>,
    id: &'static str,
}

impl<'a> Ck<'a> {
    fn new(parts: &'a [Part], types: &'a HashMap<&'static str, Type>) -> Ck<'a> {
        let mut ck = Ck {
            parts, types, ix: HashMap::new(), net_of: HashMap::new(), nets: BTreeMap::new(),
            errs: Vec::new(), id: "S1",
        };
        for (i, p) in parts.iter().enumerate() {
            if ck.ix.insert(p.r.clone(), i).is_some() {
                ck.err(format!("{}: refdes used twice", p.r));
            }
            for (pad, _, net, _) in &p.pads {
                ck.net_of.insert((p.r.clone(), pad.clone()), net.clone());
                if net != "NC" {
                    ck.nets.entry(net.clone()).or_default().push((p.r.clone(), pad.clone()));
                }
            }
        }
        ck
    }

    fn err(&mut self, msg: String) {
        self.errs.push(format!("{} {msg}", self.id));
    }

    fn ty(&self, r: &str) -> Option<&'a Type> {
        self.ix.get(r).and_then(|&i| self.types.get(self.parts[i].ty.as_str()))
    }

    fn pin(&self, p: &Pad) -> Option<&'a (String, String, Kind)> {
        self.ty(&p.0)?.pins.iter().find(|x| x.0 == p.1)
    }

    fn kind(&self, p: &Pad) -> Option<Kind> {
        self.pin(p).map(|x| x.2)
    }

    /// "U1.A10" by pin name, "R4.1" by pad number.
    fn pad(&mut self, spec: &str) -> Option<Pad> {
        let (r, pin) = spec.split_once('.').expect("spec is ref.pin");
        let found = self.ty(r).and_then(|t| {
            t.pins.iter().find(|x| x.1 == pin).or_else(|| t.pins.iter().find(|x| x.0 == pin))
        });
        match found {
            Some(x) => Some((r.to_string(), x.0.clone())),
            None => {
                self.err(format!("{spec}: no such part or pin"));
                None
            }
        }
    }

    /// The net of a pad spec, or a rail named directly. "NC" if unconnected.
    fn net(&mut self, spec: &str) -> Option<String> {
        if RAILS.contains(&spec) {
            return Some(spec.to_string());
        }
        let p = self.pad(spec)?;
        match self.net_of.get(&p) {
            Some(n) => Some(n.clone()),
            None => {
                self.err(format!("{spec}: pad {} not listed", p.1));
                None
            }
        }
    }

    /// The pads are all on one net, and it is not NC.
    fn same<S: AsRef<str>>(&mut self, specs: &[S]) {
        let nets: Vec<Option<String>> = specs.iter().map(|s| self.net(s.as_ref())).collect();
        if nets.iter().any(Option::is_none) {
            return;
        }
        let first = nets[0].clone().unwrap();
        for (s, n) in specs.iter().zip(&nets) {
            let n = n.as_ref().unwrap();
            if n == "NC" {
                self.err(format!("{} is NC", s.as_ref()));
            } else if *n != first {
                self.err(format!("{} ({n}) is not joined to {} ({first})", s.as_ref(), specs[0].as_ref()));
            }
        }
    }

    fn on(&mut self, spec: &str, rail: &str) {
        if let Some(n) = self.net(spec) {
            if n != rail {
                self.err(format!("{spec} is on {n}, not {rail}"));
            }
        }
    }

    fn nc(&mut self, spec: &str) {
        if let Some(n) = self.net(spec) {
            if n != "NC" {
                self.err(format!("{spec} is on {n}, not NC"));
            }
        }
    }

    /// The two pads of resistor "R4" or of resistor array channel "RN6:3".
    fn res(&mut self, r: &str) -> Option<[Pad; 2]> {
        let (rf, ch) = match r.split_once(':') {
            Some((a, c)) => (a, c.parse::<usize>().ok()),
            None => (r, None),
        };
        let ty = self.ix.get(rf).map(|&i| self.parts[i].ty.as_str());
        let pads = match (ty, ch) {
            (Some("R"), None) => ["1".to_string(), "2".to_string()],
            (Some("RN_SIP9"), Some(k @ 1..=8)) => ["1".to_string(), (k + 1).to_string()],
            (Some("RN_DIP16"), Some(k @ 1..=8)) => [k.to_string(), (17 - k).to_string()],
            _ => {
                self.err(format!("{r}: not a resistor or resistor channel"));
                return None;
            }
        };
        Some(pads.map(|p| (rf.to_string(), p)))
    }

    /// Resistor (channel) r joins the nets of a and b (b may be a rail).
    fn via(&mut self, a: &str, r: &str, b: &str) {
        let (Some(na), Some(nb), Some(rp)) = (self.net(a), self.net(b), self.res(r)) else { return };
        let mut want = [na, nb];
        let mut got = rp.map(|p| self.net_of.get(&p).cloned().unwrap_or_default());
        want.sort();
        got.sort();
        if want.iter().any(|n| n == "NC") || want != got {
            self.err(format!("{r} ({} - {}) does not join {a} ({}) and {b} ({})", got[0], got[1], want[0], want[1]));
        }
    }

    /// The net of a holds exactly a and these pads.
    fn net_is(&mut self, a: &str, others: &[&str]) {
        let Some(n) = self.net(a) else { return };
        let mut want: BTreeSet<Pad> = others.iter().filter_map(|s| self.pad(s)).collect();
        want.extend(self.pad(a));
        let got: BTreeSet<Pad> = self.nets.get(&n).map(|v| v.iter().cloned().collect()).unwrap_or_default();
        if want != got {
            let extra: Vec<String> = got.difference(&want).map(|p| format!("{}.{}", p.0, p.1)).collect();
            let missing: Vec<String> = want.difference(&got).map(|p| format!("{}.{}", p.0, p.1)).collect();
            self.err(format!("net of {a} ({n}): extra {extra:?}, missing {missing:?}"));
        }
    }

    /// Every resistor and resistor array channel: (label, pad, pad).
    fn channels(&self) -> Vec<(String, Pad, Pad)> {
        let mut v = Vec::new();
        for p in self.parts {
            let pr = |n: usize| (p.r.clone(), n.to_string());
            match p.ty.as_str() {
                "R" => v.push((p.r.clone(), pr(1), pr(2))),
                "RN_SIP9" => v.extend((1..=8).map(|k| (format!("{}:{k}", p.r), pr(1), pr(k + 1)))),
                "RN_DIP16" => v.extend((1..=8).map(|k| (format!("{}:{k}", p.r), pr(k), pr(17 - k)))),
                _ => {}
            }
        }
        v
    }

    /// Jumpers and switches, all closed: (pad, pad).
    fn closables(&self) -> Vec<(Pad, Pad)> {
        self.parts.iter().filter(|p| p.ty == "SW" || p.ty == "JMP2")
            .map(|p| ((p.r.clone(), "1".to_string()), (p.r.clone(), "2".to_string())))
            .collect()
    }

    fn netp(&self, p: &Pad) -> String {
        self.net_of.get(p).cloned().unwrap_or_else(|| "NC".to_string())
    }

    /// Parts whose logic supply is +5V.
    fn is_5v(&self, r: &str) -> bool {
        self.ty(r).is_some_and(|t| t.pins.iter().any(|x| x.2 == Pwr("+5V")))
    }
}

fn check(text: &str) -> Vec<String> {
    let types = types();
    let (parts, mut errs) = parse(text);
    let mut ck = Ck::new(&parts, &types);
    ck.errs.append(&mut errs);
    structure(&mut ck);
    power(&mut ck);
    circuits(&mut ck);
    ck.errs
}

fn structure(ck: &mut Ck) {
    let parts = ck.parts;
    ck.id = "S1";
    for p in parts {
        if !ck.types.contains_key(p.ty.as_str()) {
            ck.err(format!("{}: unknown type {}", p.r, p.ty));
        }
        for (_, _, net, line) in &p.pads {
            if net.is_empty() || !net.chars().all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || "_+-".contains(c)) {
                ck.err(format!("line {line}: bad net name {net}"));
            }
        }
    }
    ck.id = "S2";
    for p in parts {
        let Some(t) = ck.ty(&p.r) else { continue };
        let want: BTreeSet<&str> = t.pins.iter().map(|x| x.0.as_str()).collect();
        let mut seen = BTreeSet::new();
        for (pad, _, _, line) in &p.pads {
            if !seen.insert(pad.as_str()) {
                ck.err(format!("line {line}: {}.{pad} listed twice", p.r));
            }
            if !want.contains(pad.as_str()) {
                ck.err(format!("line {line}: {} has no pad {pad}", p.r));
            }
        }
        for missing in want.difference(&seen) {
            ck.err(format!("{}.{missing} missing", p.r));
        }
    }
    ck.id = "S3";
    for p in parts {
        for (pad, name, _, line) in &p.pads {
            if let Some(x) = ck.pin(&(p.r.clone(), pad.clone())) {
                if x.1 != *name {
                    ck.err(format!("line {line}: {}.{pad} is {} in its table, not {name}", p.r, x.1));
                }
            }
        }
    }
    ck.id = "S4";
    let singles: Vec<String> = ck.nets.iter().filter(|(_, v)| v.len() == 1).map(|(n, v)| format!("{n} ({}.{})", v[0].0, v[0].1)).collect();
    for s in singles {
        ck.err(format!("net {s} has one pad"));
    }
    // S5: no input left open, except these, which are open by their datasheets and must stay open.
    const OPEN_INPUTS: [&str; 4] = [
        "U2.13",  // 8224 TANK: overtone crystals only (6.1)
        "PS1.1",  // U3V40F12 EN: 30k on-board pull-up to VIN
        "U17.6",  // 7660 LV: open for V+ > 3.5 V (FN3072)
        "U17.7",  // 7660 OSC: open = internal oscillator
    ];
    ck.id = "S5";
    let mut found = Vec::new();
    for (pad, net) in &ck.net_of {
        if net == "NC" && ck.kind(pad) == Some(In) && !OPEN_INPUTS.contains(&format!("{}.{}", pad.0, pad.1).as_str()) {
            found.push(format!("{}.{}: input on NC", pad.0, pad.1));
        }
    }
    for spec in OPEN_INPUTS {
        let (r, pad) = spec.split_once('.').unwrap();
        let n = ck.netp(&(r.to_string(), pad.to_string()));
        if n != "NC" {
            found.push(format!("{spec}: open by its datasheet, is on {n}"));
        }
    }
    found.sort();
    for f in found {
        ck.err(f);
    }
    ck.id = "S6";
    let mut found = Vec::new();
    for (pad, net) in &ck.net_of {
        if net != "NC" && ck.kind(pad) == Some(Nc) {
            found.push(format!("{}.{}: must be NC, is on {net}", pad.0, pad.1));
        }
    }
    found.sort();
    for f in found {
        ck.err(f);
    }
    // S7: a net with an input has a driver, or a resistor to a rail or to a driven net.
    ck.id = "S7";
    let drives = |ck: &Ck, n: &str| {
        RAILS.contains(&n) || ck.nets.get(n).is_some_and(|v| v.iter().any(|p| matches!(ck.kind(p), Some(Out | Tri | Io | Od | Src(_)))))
    };
    let chans = ck.channels();
    let mut found = Vec::new();
    for (n, pads) in &ck.nets {
        if RAILS.contains(&n.as_str()) || !pads.iter().any(|p| ck.kind(p) == Some(In)) || drives(ck, n) {
            continue;
        }
        let pulled = chans.iter().any(|(_, a, b)| {
            let (na, nb) = (ck.netp(a), ck.netp(b));
            (na == *n && drives(ck, &nb)) || (nb == *n && drives(ck, &na))
        });
        if !pulled {
            found.push(format!("net {n}: inputs and no driver or pull"));
        }
    }
    for f in found {
        ck.err(f);
    }
    // S8: a totem-pole output owns its net: no second output and no three-state, bidirectional
    // or open-drain pad beside it (one totem pole on a bus is permanent contention).
    ck.id = "S8";
    let mut found = Vec::new();
    for (n, pads) in &ck.nets {
        let outs: Vec<&Pad> = pads.iter().filter(|p| ck.kind(p) == Some(Out)).collect();
        let others = pads.iter().filter(|p| matches!(ck.kind(p), Some(Tri | Io | Od))).count();
        if outs.len() > 1 {
            found.push(format!("net {n}: {} totem-pole outputs", outs.len()));
        } else if outs.len() == 1 && others > 0 {
            found.push(format!("net {n}: totem-pole {}.{} with {others} three-state, bidirectional or open-drain pads", outs[0].0, outs[0].1));
        }
        if !outs.is_empty() && RAILS.contains(&n.as_str()) {
            found.push(format!("{}.{}: output on rail {n}", outs[0].0, outs[0].1));
        }
        let srcs: BTreeSet<&str> = pads.iter().filter(|p| matches!(ck.kind(p), Some(Src(_)))).map(|p| p.0.as_str()).collect();
        if srcs.len() > 1 {
            found.push(format!("net {n}: power outputs of {srcs:?}"));
        }
    }
    for f in found {
        ck.err(f);
    }
}

fn power(ck: &mut Ck) {
    let parts = ck.parts;
    // P1: power pins on their rails.
    ck.id = "P1";
    let mut found = Vec::new();
    for (pad, net) in &ck.net_of {
        if let Some(Pwr(r) | Src(r)) = ck.kind(pad) {
            if net != r {
                found.push(format!("{}.{} belongs on {r}, is on {net}", pad.0, pad.1));
            }
        }
    }
    found.sort();
    for f in found {
        ck.err(f);
    }
    // P2: the closable parts, each with its line.
    ck.id = "P2";
    let closables: BTreeSet<&str> = parts.iter().filter(|p| p.ty == "SW" || p.ty == "JMP2").map(|p| p.r.as_str()).collect();
    if closables != BTreeSet::from(["SW1", "JP1", "JP2"]) {
        ck.err(format!("closable parts are {closables:?}, not SW1, JP1, JP2"));
    }
    ck.same(&["SW1.1", "U2.RESIN_N"]);
    ck.on("SW1.2", "GND");
    ck.same(&["JP1.1", "U12.Y7_N"]);
    ck.same(&["JP1.2", "U4.WE_N"]);
    ck.same(&["JP2.1", "RN4.COM"]);
    ck.on("JP2.2", "GND");
    // Each closable is a real break: open, its two pads are on different nets, and the
    // jumpered nets hold nothing else. ROM /WE reaches the 138 only through JP-WE (6.2,
    // 6.10), and the 2.2k pull-down reaches GND only through JP-PD (6.13).
    for (a, b) in ck.closables() {
        let (na, nb) = (ck.netp(&a), ck.netp(&b));
        if na == nb {
            ck.err(format!("{}: both pads on {na}, shorted for good", a.0));
        }
    }
    ck.net_is("JP1.1", &["U12.Y7_N", "J4.CH12"]);
    ck.net_is("JP1.2", &["U4.WE_N", "R5.1"]);
    ck.net_is("JP2.1", &["RN4.COM"]);
    // P3: electrolytics the right way round.
    ck.id = "P3";
    ck.on("C22.+", "+5V");
    ck.on("C22.-", "GND");
    ck.on("C23.+", "+12V");
    ck.on("C23.-", "GND");
    ck.same(&["C24.+", "U17.CAPP"]);
    ck.same(&["C24.-", "U17.CAPN"]);
    ck.on("C25.+", "GND");
    ck.on("C25.-", "-5V");
    // P4: one 100nF per IC per rail (6.9).
    ck.id = "P4";
    for rail in ["+5V", "+12V", "-5V", "+3V3_PI"] {
        let caps = parts.iter().filter(|p| p.ty == "C" && p.value == "100nF").filter(|p| {
            let mut n: Vec<&str> = p.pads.iter().map(|x| x.2.as_str()).collect();
            n.sort();
            let mut w = vec![rail, "GND"];
            w.sort();
            n == w
        }).count();
        let ics = parts.iter().filter(|p| p.r.starts_with('U'))
            .filter(|p| p.pads.iter().any(|x| x.2 == rail && ck.kind(&(p.r.clone(), x.0.clone())) == Some(Pwr(rail))))
            .count();
        if caps < ics {
            ck.err(format!("{rail}: {caps} decoupling capacitors for {ics} ICs"));
        }
    }
    // P5: bulk on +5V, +12V, -5V.
    ck.id = "P5";
    for rail in ["+5V", "+12V", "-5V"] {
        let bulk = parts.iter().any(|p| p.ty == "CP" && p.value == "10uF" && {
            let n: BTreeSet<&str> = p.pads.iter().map(|x| x.2.as_str()).collect();
            n == BTreeSet::from([rail, "GND"])
        });
        if !bulk {
            ck.err(format!("no 10uF bulk capacitor on {rail}"));
        }
    }
    // P6: the VBB clamp (6.9).
    ck.id = "P6";
    ck.on("D1.K", "GND");
    ck.on("D1.A", "-5V");
    // P7: the reverse-polarity switch (6.9): drain on the jack, source on the board, gate to
    // GND through its resistor, and nothing else on the jack side.
    ck.id = "P7";
    ck.net_is("J5.VIN", &["Q2.D"]);
    ck.on("Q2.S", "+5V");
    ck.via("Q2.G", "R14", "GND");
    // P8: no two rails joined through the closables.
    ck.id = "P8";
    let mut group: HashMap<String, String> = ck.nets.keys().map(|n| (n.clone(), n.clone())).collect();
    fn root(g: &HashMap<String, String>, n: &str) -> String {
        let mut n = n.to_string();
        while g[&n] != n {
            n = g[&n].clone();
        }
        n
    }
    for (a, b) in ck.closables() {
        let (na, nb) = (ck.netp(&a), ck.netp(&b));
        if na != "NC" && nb != "NC" {
            let (ra, rb) = (root(&group, &na), root(&group, &nb));
            group.insert(ra, rb);
        }
    }
    let present: Vec<&str> = RAILS.iter().copied().filter(|r| group.contains_key(*r)).collect();
    for (i, a) in present.iter().enumerate() {
        for b in &present[i + 1..] {
            if root(&group, a) == root(&group, b) {
                ck.err(format!("{a} and {b} are joined through a closed jumper or switch"));
            }
        }
    }
}

fn circuits(ck: &mut Ck) {
    // C128: the MCS-80 standard interconnect (reference 12.8).
    ck.id = "C128";
    for (a, b) in [("U2.PHI1", "U1.PHI1"), ("U2.PHI2", "U1.PHI2"), ("U2.READY", "U1.READY"),
                   ("U2.RESET", "U1.RESET"), ("U1.SYNC", "U2.SYNC"), ("U2.STSTB_N", "U3.STSTB_N"),
                   ("U1.DBIN", "U3.DBIN"), ("U1.WR_N", "U3.WR_N"), ("U1.HLDA", "U3.HLDA")] {
        ck.same(&[a, b]);
    }
    for k in 0..8 {
        ck.same(&[format!("U1.D{k}"), format!("U3.D{k}")]);
    }

    // CBUS: memory, IN latch and 245s on the buses (6.2, 6.4).
    ck.id = "CBUS";
    for m in ["U4", "U5", "U6"] {
        let names: Vec<String> = ck.ty(m).map(|t| t.pins.iter().map(|x| x.1.clone()).collect()).unwrap_or_default();
        for name in names {
            let num = |p: &str| name.strip_prefix(p).and_then(|k| k.parse::<u32>().ok());
            if let Some(k) = num("A") {
                if m == "U4" && k == 12 {
                    ck.on("U4.A12", "GND");
                } else {
                    ck.same(&[format!("{m}.{name}"), format!("U1.A{k}")]);
                }
            } else if let Some(k) = num("IO").or(num("DQ")) {
                ck.same(&[format!("{m}.{name}"), format!("U3.DB{k}")]);
            }
        }
    }
    for n in 1..=8 {
        ck.same(&[format!("U13.{n}Q"), format!("U3.DB{}", n - 1)]);
        ck.same(&[format!("U15.A{n}"), format!("U3.DB{}", n - 1)]);
    }
    for n in 1..=7 {
        ck.same(&[format!("U14.A{n}"), format!("U1.A{}", n - 1)]);
    }

    // CGAL: every .pld pin name on its board pads (6.2-6.6). A rename in the .pld needs this
    // table edited; a pin move needs the netlist edited (S3).
    ck.id = "CGAL";
    const GAL: &[(&str, &[&str])] = &[
        ("STSTB_N", &["U2.STSTB_N"]), ("MEMR_N", &["U3.MEMR_N"]), ("IOR_N", &["U3.IOR_N"]),
        ("IOW_N", &["U3.IOW_N"]), ("D4", &["U1.D4"]), ("D6", &["U1.D6"]), ("OVL", &["U8.1Q"]),
        ("ROMOE_N", &["U4.OE_N"]), ("RAMOE_N", &["U5.OE_N", "U6.OE_N"]), ("DB0", &["U3.DB0"]),
        ("OVLCLR_N", &["U8.1CLR_N"]), ("WSET_N", &["U8.2PRE_N"]), ("LATOE_N", &["U13.OE_N"]),
        ("RESET_N", &["U9.1Y"]), ("A8", &["U1.A8"]), ("A9", &["U1.A9"]), ("A10", &["U1.A10"]),
        ("A11", &["U1.A11"]), ("A12", &["U1.A12"]), ("A13", &["U1.A13"]), ("A14", &["U1.A14"]),
        ("A15", &["U1.A15"]),
    ];
    for (_, name, k) in gal_pins() {
        if matches!(k, Pwr(_)) {
            continue;
        }
        match GAL.iter().find(|g| g.0 == name) {
            Some((_, pads)) => {
                let mut specs = vec![format!("U7.{name}")];
                specs.extend(pads.iter().map(|p| p.to_string()));
                ck.same(&specs);
            }
            None => ck.err(format!("glue.pld pin {name} has no board anchor")),
        }
    }

    // C61: crystal and its 510R pair; OSC and TANK open (6.1).
    ck.id = "C61";
    ck.same(&["Y1.1", "U2.XTAL1"]);
    ck.same(&["Y1.2", "U2.XTAL2"]);
    ck.via("U2.XTAL1", "R1", "GND");
    ck.via("U2.XTAL2", "R2", "GND");
    ck.nc("U2.OSC");
    ck.nc("U2.TANK");

    // C62: chip enables from address only (6.2).
    ck.id = "C62";
    ck.on("U4.CE_N", "GND");
    ck.same(&["U5.CE_N", "U1.A15"]);
    ck.same(&["U6.CE_N", "U9.3Y"]);
    ck.same(&["U9.3A", "U1.A15"]);
    ck.same(&["U5.WE_N", "U6.WE_N", "U3.MEMW_N"]);
    for ce in ["U4.CE_N", "U5.CE_N", "U6.CE_N"] {
        let Some(n) = ck.net(ce) else { continue };
        let gal_out = ck.nets.get(&n).is_some_and(|v| v.iter().any(|p| p.0 == "U7" && matches!(ck.kind(p), Some(Out | Tri))));
        if gal_out {
            ck.err(format!("{ce} is driven by a GAL output"));
        }
    }

    // C635: the overlay flip-flop and the port FF read (6.3, 6.5).
    ck.id = "C635";
    ck.same(&["U8.1PRE_N", "U9.1Y"]);
    ck.on("U8.1D", "GND");
    ck.on("U8.1CLK", "GND");
    if ck.pad("U7.DB0").and_then(|p| ck.kind(&p)) != Some(Tri) {
        ck.err("U7.DB0 is not a three-state GAL output (glue.pld DB0.E)".to_string());
    }

    // C64: WAIT flip-flop, REQ, the 245s and the IN latch (6.4).
    ck.id = "C64";
    ck.same(&["U8.2Q_N", "U2.RDYIN"]);
    ck.via("U2.RDYIN", "R13", "+5V");
    ck.same(&["U8.2Q", "U10.1A"]);
    ck.same(&["U10.1B", "U1.WAIT"]);
    ck.same(&["U8.2CLR_N", "U9.1Y"]);
    ck.on("U8.2D", "GND");
    ck.same(&["U8.2CLK", "U9.4Y"]);
    ck.same(&["U9.4A", "U9.5Y"]);
    ck.same(&["U16.A1", "U10.1Y"]);
    ck.same(&["U16.A2", "U2.RESET"]);
    ck.same(&["U14.A8", "U3.IOR_N"]);
    ck.same(&["U15.OE_N", "U9.2Y"]);
    ck.same(&["U9.2A", "U3.IOR_N"]);
    for u in ["U14", "U15", "U16"] {
        ck.on(&format!("{u}.DIR"), "+3V3_PI");
    }
    ck.on("U14.OE_N", "GND");
    ck.on("U16.OE_N", "GND");
    ck.via("U13.CLK", "R7", "GND");
    ck.via("U9.5A", "R6", "GND");
    for n in 1..=8 {
        ck.via(&format!("U15.B{n}"), &format!("RN6:{n}"), &format!("U13.{n}D"));
    }

    // CPI: the Pi header from the src/pi constants (6.4 table).
    ck.id = "CPI";
    let bit = |m: u32| m.trailing_zeros();
    for k in 0..7 {
        ck.same(&[format!("J1.BCM{}", A_SHIFT + k), format!("U14.B{}", k + 1)]);
    }
    ck.same(&[format!("J1.BCM{}", bit(DIR)), "U14.B8".to_string()]);
    ck.same(&[format!("J1.BCM{}", bit(REQ)), "U16.B1".to_string()]);
    ck.same(&[format!("J1.BCM{}", bit(RESET)), "U16.B2".to_string()]);
    ck.same(&[format!("J1.BCM{}", bit(ACK)), "U9.5A".to_string()]);
    ck.same(&[format!("J1.BCM{}", bit(LATCH)), "U13.CLK".to_string()]);
    for k in 0..8 {
        ck.same(&[format!("J1.BCM{}", D_SHIFT + k), format!("U13.{}D", k + 1)]);
    }
    ck.via(&format!("J1.BCM{TEST_RESET_BCM}"), "R12", "Q1.B");
    ck.same(&["Q1.C", "U2.RESIN_N"]);
    let mut used = 0u32;
    let j1: Vec<(String, String)> = ck.ty("J1").map(|t| t.pins.iter().map(|x| (x.0.clone(), x.1.clone())).collect()).unwrap_or_default();
    for (pad, name) in j1 {
        let n = ck.netp(&("J1".to_string(), pad));
        if let Some(b) = name.strip_prefix("BCM").and_then(|b| b.parse::<u32>().ok()) {
            if n != "NC" && !RAILS.contains(&n.as_str()) {
                used |= 1 << b;
            }
        }
    }
    if used != PINS | 1 << TEST_RESET_BCM {
        ck.err(format!("J1 uses BCM mask {used:08X}, src/pi PINS + TEST_RESET is {:08X}", PINS | 1 << TEST_RESET_BCM));
    }

    // CISO: no 5 V source reaches a Pi pin, through resistors and closed jumpers (6.4).
    ck.id = "CISO";
    let mut seen: BTreeSet<String> = BTreeSet::new();
    let mut todo: Vec<String> = vec!["+5V".to_string()];
    for (n, pads) in &ck.nets {
        if pads.iter().any(|p| ck.is_5v(&p.0) && matches!(ck.kind(p), Some(Out | Tri | Io | Od))) {
            todo.push(n.clone());
        }
    }
    let mut links: Vec<(Pad, Pad)> = ck.channels().into_iter().map(|(_, a, b)| (a, b)).collect();
    links.extend(ck.closables());
    while let Some(n) = todo.pop() {
        if n == "GND" || n == "NC" || !seen.insert(n.clone()) {
            continue;
        }
        for (a, b) in &links {
            let (na, nb) = (ck.netp(a), ck.netp(b));
            if na == n {
                todo.push(nb);
            } else if nb == n {
                todo.push(na);
            }
        }
    }
    let mut found = Vec::new();
    for n in &seen {
        for p in ck.nets.get(n).into_iter().flatten().filter(|p| p.0 == "J1") {
            found.push(format!("5 V reaches J1.{} (net {n})", p.1));
        }
    }
    for f in found {
        ck.err(f);
    }

    // C66: RESET has its three loads plus the LA-B tap; /RESIN its three sources plus LA-C (6.6).
    ck.id = "C66";
    ck.net_is("U2.RESET", &["U1.RESET", "U9.1A", "U16.A2", "J3.CH15"]);
    ck.net_is("U2.RESIN_N", &["U18.RST_N", "SW1.1", "Q1.C", "J4.CH14"]);
    ck.on("Q1.E", "GND");

    // C67: other CPU pins (6.7).
    ck.id = "C67";
    ck.via("U1.INT", "R4", "GND");
    ck.on("U1.HOLD", "GND");
    ck.on("U3.BUSEN_N", "GND");
    ck.via("U3.INTA_N", "R3", "+12V");

    // C610: the ROM write-enable gate (6.10).
    ck.id = "C610";
    for (a, b) in [("U12.A", "U1.A12"), ("U12.B", "U1.A13"), ("U12.C", "U1.A14"), ("U12.G1", "U1.A15"),
                   ("U12.G2A_N", "U3.MEMW_N")] {
        ck.same(&[a, b]);
    }
    ck.on("U12.G2B_N", "GND");
    for k in 0..7 {
        ck.nc(&format!("U12.Y{k}_N"));
    }
    ck.same(&["JP1.1", "U12.Y7_N", "J4.CH12"]);
    ck.same(&["JP1.2", "U4.WE_N"]);
    ck.via("U4.WE_N", "R5", "+5V");

    // C611: status LEDs on 74HCT08 buffers, never on an 8080A pin (6.7, 6.11).
    ck.id = "C611";
    for (buf, r, led) in [("U10.2Y", "R8", "D2"), ("U11.3Y", "R9", "D3"), ("U10.3Y", "R10", "D4"), ("U10.4Y", "R11", "D5")] {
        ck.via(buf, r, &format!("{led}.A"));
        ck.on(&format!("{led}.K"), "GND");
    }
    ck.same(&["U10.2A", "U1.WAIT"]);
    ck.same(&["U10.3A", "U1.INTE"]);
    ck.same(&["U10.4A", "U1.HLDA"]);
    for b in ["U10.2B", "U10.3B", "U10.4B"] {
        ck.on(b, "+5V");
    }
    ck.same(&["U11.1A", "U10.2Y"]);
    ck.same(&["U11.1B", "U8.2Q_N"]);
    ck.same(&["U11.2A", "U3.IOR_N"]);
    ck.same(&["U11.2B", "U3.IOW_N"]);
    ck.same(&["U11.3A", "U11.1Y"]);
    ck.same(&["U11.3B", "U11.2Y"]);
    ck.on("U11.4A", "GND");
    ck.on("U11.4B", "GND");
    let cpu_net = |ck: &Ck, n: &str| !RAILS.contains(&n) && ck.nets.get(n).is_some_and(|v| v.iter().any(|p| p.0 == "U1"));
    let chans = ck.channels();
    let mut found = Vec::new();
    for p in ck.parts.iter().filter(|p| p.ty == "LED") {
        for (pad, _, n, _) in p.pads.iter().filter(|x| !RAILS.contains(&x.2.as_str())) {
            let through = chans.iter().find_map(|(c, a, b)| {
                let (na, nb) = (ck.netp(a), ck.netp(b));
                if na == *n && cpu_net(ck, &nb) || nb == *n && cpu_net(ck, &na) { Some(c.clone()) } else { None }
            });
            if cpu_net(ck, n) {
                found.push(format!("{}.{pad} is on an 8080A net ({n})", p.r));
            } else if let Some(c) = through {
                found.push(format!("{}.{pad} reaches an 8080A net through {c}", p.r));
            }
        }
    }
    for f in found {
        ck.err(f);
    }

    // C612: analyzer headers (6.12, HARDWARE_BUILD 3.1 channel order).
    ck.id = "C612";
    for k in 0..16 {
        ck.same(&[format!("J2.CH{k}"), format!("U1.A{k}")]);
    }
    for k in 0..8 {
        ck.via(&format!("J3.CH{k}"), &format!("RN5:{}", k + 1), &format!("U1.D{k}"));
    }
    for (ch, pad) in [(8, "U1.SYNC"), (9, "U1.DBIN"), (10, "U1.WR_N"), (11, "U2.READY"), (12, "U1.WAIT"),
                      (13, "U2.STSTB_N"), (14, "U2.PHI2_TTL"), (15, "U2.RESET")] {
        ck.same(&[format!("J3.CH{ch}"), pad.to_string()]);
    }
    for (ch, pad) in [(0, "U3.MEMR_N"), (1, "U3.MEMW_N"), (2, "U3.IOR_N"), (3, "U3.IOW_N"), (4, "U1.INTE"),
                      (5, "U1.HLDA"), (6, "U8.2Q_N"), (7, "U10.1Y"), (8, "U9.5A"), (9, "U13.CLK"),
                      (10, "U8.1Q"), (11, "U4.OE_N"), (12, "U12.Y7_N"), (13, "U11.3Y"), (14, "U2.RESIN_N")] {
        ck.same(&[format!("J4.CH{ch}"), pad.to_string()]);
    }
    ck.nc("J4.CH15");
    for clk in ["U2.PHI1", "U2.PHI2", "U3.INTA_N"] {
        let Some(n) = ck.net(clk) else { continue };
        if ck.nets.get(&n).is_some_and(|v| v.iter().any(|p| p.0.starts_with('J') || p.0.starts_with("TP"))) {
            ck.err(format!("the net of {clk} ({n}) is on a header or test point"));
        }
    }

    // C613: bus pull-ups; none on CPU-side D0-D7 (6.13).
    ck.id = "C613";
    for k in 0..8 {
        ck.via(&format!("U1.A{k}"), &format!("RN1:{}", k + 1), "+5V");
        ck.via(&format!("U1.A{}", k + 8), &format!("RN2:{}", k + 1), "+5V");
        ck.via(&format!("U3.DB{k}"), &format!("RN3:{}", k + 1), "+5V");
        ck.via(&format!("U3.DB{k}"), &format!("RN4:{}", k + 1), "RN4.COM");
    }
    for k in 0..8 {
        let Some(n) = ck.net(&format!("U1.D{k}")) else { continue };
        let bad: Vec<String> = ck.nets.get(&n).into_iter().flatten()
            .filter(|p| p.0 != "RN5" && ck.ix.get(&p.0).is_some_and(|&i| ck.parts[i].ty.starts_with('R')))
            .map(|p| format!("{}.{}", p.0, p.1))
            .collect();
        if !bad.is_empty() || RAILS.contains(&n.as_str()) {
            ck.err(format!("CPU-side D{k} ({n}) carries {bad:?}"));
        }
    }

    // CVAL: part values. The circuit checks above are topology only, and the golden KiCad file
    // agrees with any value once regenerated, so each value is asserted against its source.
    ck.id = "CVAL";
    const VALUES: &[(&str, &str)] = &[
        ("Y1", "18.432MHz"),           // 6.1
        ("R1 R2", "510R"),             // 6.1, 8224 note 1
        ("R3", "1k"),                  // 6.7, INTA strap
        ("R4", "10k"),                 // 6.7, INT pull-down
        ("R5", "10k"),                 // 6.10, ROM /WE pull-up
        ("R6 R7", "4.7k"),             // 6.4, ACK and LATCH pull-downs
        ("R8 R9 R10 R11", "1k"),       // 6.11, LED resistors
        ("R12", "4.7k"),               // 6.6, TEST_RESET base resistor
        ("R13", "10k"),                // HARDWARE_BUILD 2 (decision Q-NET-RDYIN), RDYIN pull-up
        ("R14", "10k"),                // 6.9, P-FET gate to GND
        ("RN1 RN2 RN3", "10k"),        // 6.13, bus pull-ups
        ("RN4", "2.2k"),               // 6.13 Bring-up, DB pull-down
        ("RN5", "1k"),                 // 6.12 rule 2, analyzer isolation
        ("RN6", "330R"),               // 6.4, Pi D0-D7 series array
        ("Q2", "SUP53P06-20"),         // 6.9, the fitted P-MOSFET
        ("D1", "SCHOTTKY"),            // 6.9, VBB clamp
        ("C22 C23 C24 C25", "10uF"),   // 6.9 bulk; HARDWARE_BUILD 2, the pump capacitors
    ];
    for (refs, v) in VALUES {
        for r in refs.split_whitespace() {
            match ck.ix.get(r).map(|&i| ck.parts[i].value.as_str()) {
                Some(got) if got == *v => {}
                Some(got) => ck.err(format!("{r} is {got}, not {v}")),
                None => ck.err(format!("{r}: no such part")),
            }
        }
    }

    // CTP: test points on their rails and probe nets (HARDWARE_BUILD 2, 2.3, 3 steps 0, 4).
    // None on phi1/phi2: C612.
    ck.id = "CTP";
    for (tp, rail) in [("TP1.1", "GND"), ("TP2.1", "GND"), ("TP3.1", "+5V"), ("TP4.1", "+12V"), ("TP5.1", "-5V"), ("TP6.1", "+3V3_PI")] {
        ck.on(tp, rail);
    }
    ck.same(&["TP7.1", "U8.2PRE_N"]);
}

// ---------------------------------------------------------------- KiCad files

#[derive(Debug)]
enum Sx {
    A(String),
    L(Vec<Sx>),
}

/// A minimal s-expression reader: lists, bare atoms, quoted strings with backslash escapes.
fn sx(text: &str) -> Result<Vec<Sx>, String> {
    let mut stack: Vec<Vec<Sx>> = vec![Vec::new()];
    let mut it = text.chars().peekable();
    while let Some(c) = it.next() {
        match c {
            '(' => stack.push(Vec::new()),
            ')' => {
                let l = stack.pop().unwrap();
                stack.last_mut().ok_or("unbalanced )")?.push(Sx::L(l));
            }
            '"' => {
                let mut s = String::new();
                loop {
                    match it.next() {
                        Some('\\') => s.extend(it.next()),
                        Some('"') => break,
                        Some(ch) => s.push(ch),
                        None => return Err("unterminated string".to_string()),
                    }
                }
                stack.last_mut().unwrap().push(Sx::A(s));
            }
            c if c.is_whitespace() => {}
            c => {
                let mut s = String::from(c);
                while let Some(&d) = it.peek() {
                    if d.is_whitespace() || d == '(' || d == ')' {
                        break;
                    }
                    s.push(d);
                    it.next();
                }
                stack.last_mut().unwrap().push(Sx::A(s));
            }
        }
    }
    if stack.len() != 1 {
        return Err("unbalanced (".to_string());
    }
    Ok(stack.pop().unwrap())
}

fn atom(x: &Sx) -> Option<&str> {
    match x {
        Sx::A(s) => Some(s),
        Sx::L(_) => None,
    }
}

impl Sx {
    fn eq_atom(&self, s: &str) -> bool {
        atom(self) == Some(s)
    }
}

/// The child lists of l whose head is key.
fn kids<'a>(l: &'a [Sx], key: &'a str) -> impl Iterator<Item = &'a [Sx]> + 'a {
    l.iter().filter_map(move |x| match x {
        Sx::L(v) if v.first().and_then(atom) == Some(key) => Some(v.as_slice()),
        _ => None,
    })
}

/// The first atom after key in the first child list headed key.
fn val<'a>(l: &'a [Sx], key: &'a str) -> Option<&'a str> {
    kids(l, key).next().and_then(|v| v.get(1)).and_then(atom)
}

fn pintype(k: Kind) -> &'static str {
    match k {
        In => "input",
        Out => "output",
        Tri => "tri_state",
        Io => "bidirectional",
        Od => "open_collector",
        Pwr(_) => "power_in",
        Src(_) => "power_out",
        Pas | Probe => "passive",
        Nc => "no_connect",
    }
}

/// KiCad's s-expression netlist (version E). Deterministic: components in natural refdes
/// order, nets by name, nodes by (ref, pad). tstamps: the refdes ASCII bytes in hex.
fn kicad_netlist(parts: &[Part], types: &HashMap<&'static str, Type>) -> String {
    let ck = Ck::new(parts, types);
    let mut sorted: Vec<&Part> = parts.iter().collect();
    sorted.sort_by_key(|p| nat(&p.r));
    let mut s = String::from("(export (version \"E\")\n  (design\n    (source \"hw/board.net.txt\")\n    (tool \"intel8080_emu tests/netlist_tests.rs\"))\n  (components");
    for p in sorted {
        let hex: String = p.r.bytes().map(|b| format!("{b:02X}")).collect();
        s += &format!("\n    (comp (ref \"{}\")\n      (value \"{}\")\n      (footprint \"{}\")\n      (libsource (lib \"intel8080_emu\") (part \"{}\") (description \"\"))\n      (sheetpath (names \"/\") (tstamps \"/\"))\n      (tstamps \"00000000-0000-0000-0000-{hex:0>12}\"))",
                      p.r, p.value, types[p.ty.as_str()].fp, p.ty);
    }
    s += ")\n  (nets";
    for (code, (net, pads)) in ck.nets.iter().enumerate() {
        s += &format!("\n    (net (code \"{}\") (name \"{net}\")", code + 1);
        let mut pads = pads.clone();
        pads.sort_by_key(|p| (nat(&p.0), nat(&p.1)));
        for p in &pads {
            let (_, name, k) = ck.pin(p).unwrap();
            let func = if name == "~" { String::new() } else { format!(" (pinfunction \"{name}\")") };
            s += &format!("\n      (node (ref \"{}\") (pin \"{}\"){func} (pintype \"{}\"))", p.0, p.1, pintype(*k));
        }
        s += ")";
    }
    s += "))\n";
    s
}

/// Footprint pads and their nets from a .kicad_pcb: ref -> (footprint, pad -> nets).
type PcbParts = BTreeMap<String, (String, BTreeMap<String, BTreeSet<String>>)>;

fn pcb_parts(text: &str) -> Result<PcbParts, String> {
    let root = sx(text)?;
    let Some(Sx::L(top)) = root.first() else { return Err("no top-level list".to_string()) };
    let mut out = PcbParts::new();
    for fp in kids(top, "footprint") {
        let lib = fp.get(1).and_then(atom).unwrap_or("").to_string();
        let r = kids(fp, "property").find(|v| v.get(1).and_then(atom) == Some("Reference"))
            .or_else(|| kids(fp, "fp_text").find(|v| v.get(1).and_then(atom) == Some("reference")))
            .and_then(|v| v.get(2)).and_then(atom).ok_or(format!("footprint {lib} without a reference"))?;
        let mut pads: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
        for pad in kids(fp, "pad") {
            let num = pad.get(1).and_then(atom).unwrap_or("");
            if num.is_empty() {
                continue; // unnumbered pads: mounting holes, mechanical
            }
            let net = kids(pad, "net").next().and_then(|v| v.last()).and_then(atom).filter(|n| !n.is_empty()).unwrap_or("NC");
            pads.entry(num.to_string()).or_default().insert(net.to_string());
        }
        out.insert(r.to_string(), (lib, pads));
    }
    Ok(out)
}

/// The routed board against the netlist (Q-NET-PCBCHECK): same refs, footprints and pad nets.
fn pcb_check(pcb: &PcbParts, parts: &[Part], types: &HashMap<&'static str, Type>) -> Vec<String> {
    let mut errs = Vec::new();
    let want: BTreeSet<&str> = parts.iter().map(|p| p.r.as_str()).collect();
    let got: BTreeSet<&str> = pcb.keys().map(String::as_str).collect();
    for r in want.symmetric_difference(&got) {
        errs.push(format!("PCB {r}: in only one of the netlist and the board"));
    }
    for p in parts {
        let Some((lib, pads)) = pcb.get(&p.r) else { continue };
        let fp = types.get(p.ty.as_str()).map(|t| t.fp).unwrap_or("");
        if lib != fp {
            errs.push(format!("PCB {}: footprint {lib}, netlist {fp}", p.r));
        }
        for (pad, _, net, _) in &p.pads {
            let on = pads.get(pad).cloned().unwrap_or_default();
            if on != BTreeSet::from([net.clone()]) {
                errs.push(format!("PCB {}.{pad}: board {on:?}, netlist {net}", p.r));
            }
        }
    }
    errs
}

// ---------------------------------------------------------------- parts order

/// docs/PARTS_ORDER.md against the netlist (decision "parts set", PARTS_ORDER 1): in every table
/// whose first column is Refs, each netlist refdes sits in exactly one Refs cell, every Refs entry
/// is a netlist refdes, Need is the refdes count and Order is at least that. '-' = no refdes.
fn order_check(order: &str, netlist: &str) -> Vec<String> {
    let (parts, _) = parse(netlist);
    let board: BTreeSet<String> = parts.iter().map(|p| p.r.clone()).collect();
    let mut lines_of: BTreeMap<String, Vec<usize>> = BTreeMap::new();
    let mut errs = Vec::new();
    let mut cols: Option<(usize, usize)> = None; // Need, Order
    for (i, line) in order.lines().enumerate() {
        let Some(row) = line.trim().strip_prefix('|') else {
            cols = None;
            continue;
        };
        let cells: Vec<&str> = row.trim_end_matches('|').split('|').map(str::trim).collect();
        if cells[0] == "Refs" {
            let at = |h: &str| cells.iter().position(|c| *c == h);
            cols = at("Need").zip(at("Order"));
            if cols.is_none() {
                errs.push(format!("ORD line {}: Refs table without Need and Order", i + 1));
            }
            continue;
        }
        let Some((need, ord)) = cols else { continue };
        if cells[0].chars().all(|c| c == '-' || c == ':') {
            continue; // separator row, or a line that serves no refdes
        }
        let mut refs = Vec::new();
        for item in cells[0].split(',').map(str::trim) {
            let (a, b) = item.split_once('-').unwrap_or((item, item));
            let ((pa, na, ra), (pb, nb, rb)) = (nat(a), nat(b));
            if pa.is_empty() || pa != pb || !ra.is_empty() || !rb.is_empty() || na > nb {
                errs.push(format!("ORD line {}: bad Refs entry '{item}'", i + 1));
                continue;
            }
            refs.extend((na..=nb).map(|n| format!("{pa}{n}")));
        }
        let num = |c: usize| cells.get(c).and_then(|v| v.parse::<usize>().ok());
        match (num(need), num(ord)) {
            (Some(n), Some(o)) if n == refs.len() && o >= n => {}
            (n, o) => errs.push(format!("ORD line {}: {} refs, Need {n:?}, Order {o:?}", i + 1, refs.len())),
        }
        for r in refs {
            lines_of.entry(r).or_default().push(i + 1);
        }
    }
    for r in &board {
        match lines_of.get(r).map(Vec::len) {
            Some(1) => {}
            None => errs.push(format!("ORD {r}: on no order line")),
            Some(_) => errs.push(format!("ORD {r}: on order lines {:?}", lines_of[r])),
        }
    }
    for (r, at) in &lines_of {
        if !board.contains(r) {
            errs.push(format!("ORD lines {at:?}: {r} is not in hw/board.net.txt"));
        }
    }
    errs
}

// ---------------------------------------------------------------- tests

#[test]
fn board_netlist_is_consistent() {
    let errs = check(NETLIST);
    assert!(errs.is_empty(), "hw/board.net.txt:\n{}", errs.join("\n"));
}

#[test]
fn kicad_netlist_is_current() {
    let types = types();
    let (parts, errs) = parse(NETLIST);
    assert!(errs.is_empty(), "{errs:?}");
    let text = kicad_netlist(&parts, &types);
    let path = std::path::Path::new(env!("CARGO_TARGET_TMPDIR")).join("board.kicad.net");
    std::fs::write(&path, &text).unwrap();

    // Read back what was written: (ref, pin) -> net equals the netlist minus NC.
    let back = std::fs::read_to_string(&path).unwrap();
    let root = sx(&back).unwrap();
    let Some(Sx::L(export)) = root.first() else { panic!("no export list") };
    let mut got: BTreeMap<Pad, String> = BTreeMap::new();
    for net in kids(export, "nets").next().unwrap().iter().skip(1) {
        let Sx::L(net) = net else { panic!("net is not a list") };
        let name = val(net, "name").unwrap().to_string();
        for node in kids(net, "node") {
            let p = (val(node, "ref").unwrap().to_string(), val(node, "pin").unwrap().to_string());
            assert!(got.insert(p.clone(), name.clone()).is_none(), "{p:?} on two nets");
        }
    }
    let want: BTreeMap<Pad, String> = parts.iter()
        .flat_map(|p| p.pads.iter().filter(|x| x.2 != "NC").map(|x| ((p.r.clone(), x.0.clone()), x.2.clone())))
        .collect();
    assert_eq!(got, want, "read-back nets differ from hw/board.net.txt");
    let comps: BTreeMap<String, String> = kids(export, "components").next().unwrap().iter().skip(1)
        .filter_map(|c| match c {
            Sx::L(c) => Some((val(c, "ref")?.to_string(), val(c, "footprint")?.to_string())),
            Sx::A(_) => None,
        })
        .collect();
    let want: BTreeMap<String, String> = parts.iter().map(|p| (p.r.clone(), types[p.ty.as_str()].fp.to_string())).collect();
    assert_eq!(comps, want, "read-back components differ");

    assert!(text == KICAD_NET, "hw/board.kicad.net is stale: cp {} hw/board.kicad.net", path.display());
}

#[test]
fn kicad_files_parse() {
    let (parts, _) = parse(NETLIST);
    let nets: BTreeSet<&str> = parts.iter().flat_map(|p| p.pads.iter().map(|x| x.2.as_str())).collect();

    // Design rules: known constraints, and every net a condition names exists (a renamed net
    // must not silently disable a rule).
    let uncommented: Vec<&str> = DRU.lines().filter(|l| !l.trim_start().starts_with('#')).collect();
    let dru = sx(&uncommented.join("\n")).expect("hw/board.kicad_dru");
    let heads: Vec<Option<&str>> = dru.iter().map(|x| match x { Sx::L(v) => v.first().and_then(atom), Sx::A(_) => None }).collect();
    assert!(heads.iter().all(|h| matches!(h, Some("version" | "rule"))), "kicad_dru top level: {heads:?}");
    let mut rules = 0;
    let mut width: BTreeMap<&str, f64> = BTreeMap::new(); // net -> widest track_width min naming it
    for rule in kids(&dru, "rule") {
        rules += 1;
        let mut min_width = None;
        for c in kids(rule, "constraint") {
            let k = c.get(1).and_then(atom).unwrap_or("");
            assert!(["track_width", "clearance", "via_diameter", "hole_size", "edge_clearance"].contains(&k), "unknown constraint {k}");
            if k == "track_width" {
                min_width = val(c, "min").and_then(|v| v.strip_suffix("mm")).and_then(|v| v.parse::<f64>().ok());
            }
        }
        if let Some(cond) = val(rule, "condition") {
            for name in cond.split("NetName == '").skip(1).map(|s| s.split('\'').next().unwrap()) {
                assert!(nets.contains(name), "kicad_dru names net {name}, which hw/board.net.txt does not have");
                if let Some(w) = min_width {
                    let e = width.entry(name).or_insert(0.0);
                    *e = e.max(w);
                }
            }
        }
    }
    assert!(rules >= 3, "kicad_dru has {rules} rules");
    // Rail widths (decision T-PCB): every rail has a width rule; the about 1 A path (6.9) and
    // its return at least 0.8 mm, the others at least 0.4 mm, as hw/board.kicad_dru states.
    for rail in RAILS {
        let want = if ["+5V", "+5V_IN", "GND"].contains(&rail) { 0.8 } else { 0.4 };
        let got = width.get(rail).copied().unwrap_or(0.0);
        assert!(got >= want, "kicad_dru: track_width min for {rail} is {got} mm, needs {want} mm");
    }

    // The board: 160 x 120 mm outline on Edge.Cuts (Q-NET-OUTLINE).
    let pcb = sx(PCB).expect("hw/board.kicad_pcb");
    let Some(Sx::L(top)) = pcb.first() else { panic!("no kicad_pcb list") };
    assert!(top[0].eq_atom("kicad_pcb"));
    let rect = kids(top, "gr_rect").find(|r| val(r, "layer") == Some("Edge.Cuts")).expect("Edge.Cuts gr_rect");
    let xy = |k: &str| -> (f64, f64) {
        let v = kids(rect, k).next().unwrap();
        (atom(&v[1]).unwrap().parse().unwrap(), atom(&v[2]).unwrap().parse().unwrap())
    };
    let ((x0, y0), (x1, y1)) = (xy("start"), xy("end"));
    assert_eq!(((x1 - x0).abs(), (y1 - y0).abs()), (160.0, 120.0));

    // The project: valid JSON naming itself (KiCad reads board.kicad_dru only beside it).
    let pro: serde_json::Value = serde_json::from_str(PRO).expect("hw/board.kicad_pro");
    assert_eq!(pro["meta"]["filename"], "board.kicad_pro");
}

#[test]
fn kicad_pcb_matches_netlist() {
    let pcb = pcb_parts(PCB).expect("hw/board.kicad_pcb");
    if pcb.is_empty() {
        return; // outline only: nothing placed yet (Q-NET-PCBCHECK)
    }
    let (parts, _) = parse(NETLIST);
    let errs = pcb_check(&pcb, &parts, &types());
    assert!(errs.is_empty(), "hw/board.kicad_pcb:\n{}", errs.join("\n"));
}

#[test]
fn parts_order_covers_the_netlist() {
    let errs = order_check(PARTS_ORDER, NETLIST);
    assert!(errs.is_empty(), "docs/PARTS_ORDER.md:\n{}", errs.join("\n"));

    // Each edit of the order (anchor, replacement), or of the netlist, must raise an error.
    let order_mutants: &[(&str, &str)] = &[
        ("| U18 |", "| - |"),             // a refdes on no line
        ("| U17 |", "| U17, U18 |"),      // on two lines (and Need wrong)
        ("| C1-C21 |", "| C1-C22 |"),     // C22 twice, Need wrong
        ("| J2-J4 | 2x10 shrouded box header, 2.54 mm | Wurth 61202021621 | 710-61202021621 (confirm) | 3 | 3 |",
         "| J2-J4 | 2x10 shrouded box header, 2.54 mm | Wurth 61202021621 | 710-61202021621 (confirm) | 3 | 2 |"),
        ("| H1-H4 |", "| H1-H5 |"),       // a refdes the board does not have
    ];
    let mut survived = Vec::new();
    for (anchor, repl) in order_mutants {
        assert_eq!(PARTS_ORDER.matches(anchor).count(), 1, "order anchor {anchor:?}");
        if order_check(&PARTS_ORDER.replacen(anchor, repl, 1), NETLIST).is_empty() {
            survived.push(format!("order edit {anchor:?} -> {repl:?}"));
        }
    }
    let added = NETLIST.replace("part H4 HOLE M3", "part H4 HOLE M3\npart H5 HOLE M3");
    if order_check(PARTS_ORDER, &added).is_empty() {
        survived.push("netlist part H5 not on any order line".to_string());
    }
    assert!(survived.is_empty(), "survived: {survived:?}");
}

#[test]
fn pi_board_outputs_use_pull_down_pins() {
    // CGPIO (6.4): every Pi pin that drives the board defaults to a pull-down at boot:
    // BCM 9-27, never 14 or 15.
    let bit = |m: u32| m.trailing_zeros();
    let mut driven = vec![bit(ACK), bit(LATCH), TEST_RESET_BCM];
    driven.extend(D_SHIFT..D_SHIFT + 8);
    for b in driven {
        assert!((9..=27).contains(&b) && b != 14 && b != 15, "BCM {b} has no boot pull-down");
    }
}

/// One mutant per check ID (and the dead-board cases): each edit's anchor (consecutive lines,
/// compared with comments stripped and whitespace collapsed) must match exactly once, and the
/// mutant must raise an error starting with its ID.
#[test]
fn checker_catches_mutations() {
    const MUTANTS: &[(&str, &[(&str, &str)])] = &[
        ("S1", &[("part U10 74HCT08 74HCT08", "part U10 74HCT09 74HCT08")]),
        ("S2", &[("19 SYNC SYNC\n20 VCC +5V", "19 SYNC SYNC")]),
        ("S3", &[("1 A10 A10", "1 A11 A10")]),
        ("S3", &[("17 DB0 DB0", "17 FFRD_N DB0")]),
        ("S4", &[("4 G2A_N MEMW_N", "4 G2A_N MEMWN")]),
        ("S5", &[("13 6A GND", "13 6A NC")]),
        // A datasheet-open input strapped: the charge pump's oscillator stopped.
        ("S5", &[("7 OSC NC", "7 OSC GND")]),
        ("S6", &[("26 NC NC", "26 NC A13")]),
        ("S7", &[("12 4A GND\n13 4B GND", "12 4A X4\n13 4B X4")]),
        ("S8", &[("6 2Y WAIT_B", "6 2Y REQ")]),
        // A spare inverter (input on GND) driving DB5 high for ever.
        ("S8", &[("12 6Y NC", "12 6Y DB5")]),
        ("P1", &[("19 OE_N DOE_N\n20 VCC +3V3_PI", "19 OE_N DOE_N\n20 VCC +5V")]),
        ("P2", &[("part SW1 SW RESET\n1 ~ RESIN_N\n2 ~ GND", "part SW1 SW RESET\n1 ~ RESIN_N\n2 ~ +5V")]),
        ("P3", &[("part C22 CP 10uF\n1 + +5V\n2 - GND", "part C22 CP 10uF\n1 + GND\n2 - +5V")]),
        ("P4", &[("part C19 C 100nF\n1 ~ +3V3_PI\n2 ~ GND", "")]),
        ("P5", &[("part C23 CP 10uF\n1 + +12V\n2 - GND", "")]),
        ("P6", &[("1 K GND\n2 A -5V", "1 K -5V\n2 A GND")]),
        ("P7", &[("2 D +5V_IN\n3 S +5V", "2 D +5V\n3 S +5V_IN")]),
        // JP-WE shorted for good: ROM /WE wired straight to the 138's /Y7 (6.2, 6.10).
        ("P2", &[("27 WE_N ROM_WE_N", "27 WE_N Y7_N"),
                 ("2 ~ ROM_WE_N\npart R5 R 10k\n1 ~ ROM_WE_N", "2 ~ Y7_N\npart R5 R 10k\n1 ~ Y7_N")]),
        // JP-PD shorted for good: the 2.2k pull-down on DB next to RN3 (6.13).
        ("P2", &[("1 COM PD_COM", "1 COM GND"), ("1 ~ PD_COM\n2 ~ GND", "1 ~ GND\n2 ~ GND")]),
        ("P8", &[("part JP2 JMP2 JP-PD\n1 ~ PD_COM", "part JP2 JMP2 JP-PD\n1 ~ +5V")]),
        ("C128", &[("15 PHI2 PHI2", "15 PHI2 PHI1"), ("22 PHI1 PHI1", "22 PHI1 PHI2")]),
        ("CBUS", &[("2 A12 GND\n3 A7 A7\n4 A6 A6", "2 A12 GND\n3 A7 A6\n4 A6 A7")]),
        // GAL pin 11 now on the CPU's D5: the 8080A and 8228 stay consistent with each other.
        ("CGAL", &[("3 D4 D4\n4 D5 D5", "3 D4 D5\n4 D5 D4"), ("6 D4 D4", "6 D4 D5"), ("19 D5 D5", "19 D5 D4")]),
        ("C61", &[("part R1 R 510R\n1 ~ XTAL1\n2 ~ GND", "part R1 R 510R\n1 ~ XTAL1\n2 ~ +5V")]),
        ("C62", &[("20 CE_N GND", "20 CE_N ROMOE_N")]),
        ("C635", &[("4 1PRE_N RESET_N", "4 1PRE_N WSET_N")]),
        // Q drives RDYIN, /Q drives REQ: a label-preserving swap.
        ("C64", &[("8 2Q_N RDYIN\n9 2Q WAIT_Q", "8 2Q_N WAIT_Q\n9 2Q RDYIN")]),
        ("C64", &[("1 DIR +3V3_PI\n2 A1 A0", "1 DIR GND\n2 A1 A0")]),
        // The Pi blinded to REQ and RESET.
        ("C64", &[("18 B1 PI_REQ\n19 OE_N GND", "18 B1 PI_REQ\n19 OE_N +3V3_PI")]),
        ("C64", &[("part R13 R 10k\n1 ~ RDYIN\n2 ~ +5V", "")]),
        ("CPI", &[("7 BCM4 PI_A0", "7 BCM4 PI_A1"), ("29 BCM5 PI_A1", "29 BCM5 PI_A0")]),
        // REQ bypasses the 245; a pull-up from +5V to a Pi net; a 5 V output behind RN6.
        ("CISO", &[("32 BCM12 PI_REQ", "32 BCM12 REQ")]),
        ("CISO", &[("part H4 HOLE M3", "part H4 HOLE M3\npart R99 R 10k\n1 ~ PI_REQ\n2 ~ +5V")]),
        ("CISO", &[("2 1Q DB0", "2 1Q BD0")]),
        ("C66", &[("12 4A GND", "12 4A RESET")]),
        ("C67", &[("13 HOLD GND", "13 HOLD +5V")]),
        ("C610", &[("4 G2A_N MEMW_N", "4 G2A_N MEMR_N")]),
        ("C611", &[("2 A LED_WAIT", "2 A INTE")]),
        ("C612", &[("15 CH14 PHI2_TTL", "15 CH14 PHI2")]),
        ("C612", &[("1 CH0 LAD0", "1 CH0 D0")]),
        ("C612", &[("9 CH8 ACK\n10 CH9 LATCH", "9 CH8 LATCH\n10 CH9 ACK")]),
        ("C612", &[("part H4 HOLE M3", "part H4 HOLE M3\npart TP99 TP PHI1\n1 ~ PHI1")]),
        ("C613", &[("part H4 HOLE M3", "part H4 HOLE M3\npart R99 R 10k\n1 ~ D3\n2 ~ +5V")]),
        ("CVAL", &[("part RN6 RN_DIP16 330R", "part RN6 RN_DIP16 33k")]),
        ("CTP", &[("part TP7 TP WSET\n1 ~ WSET_N", "part TP7 TP WSET\n1 ~ RDYIN")]),
    ];
    assert!(check(NETLIST).is_empty(), "the unmutated board must be clean first");
    let norm = |l: &str| l.split('#').next().unwrap().split_whitespace().collect::<Vec<_>>().join(" ");
    let mut failures = Vec::new();
    for (i, (id, edits)) in MUTANTS.iter().enumerate() {
        let mut lines: Vec<String> = NETLIST.lines().map(str::to_string).collect();
        for (anchor, repl) in edits.iter() {
            let a: Vec<String> = anchor.lines().map(norm).collect();
            let hits: Vec<usize> = (0..=lines.len() - a.len())
                .filter(|&s| (0..a.len()).all(|j| norm(&lines[s + j]) == a[j]))
                .collect();
            assert_eq!(hits.len(), 1, "mutant {i} ({id}): anchor {anchor:?} matches {} times", hits.len());
            lines.splice(hits[0]..hits[0] + a.len(), repl.lines().map(str::to_string));
        }
        let errs = check(&lines.join("\n"));
        if !errs.iter().any(|e| e.starts_with(&format!("{id} "))) {
            failures.push(format!("mutant {i} ({id}) survived: {errs:?}"));
        }
    }

    // PCB: a board built from the netlist passes, and one pad moved to another net fails.
    let types = types();
    let (parts, _) = parse(NETLIST);
    let board = |moved: Option<(&str, &str)>| {
        let mut s = String::from("(kicad_pcb (version 20240108)\n");
        for p in &parts {
            s += &format!("  (footprint \"{}\" (property \"Reference\" \"{}\")", types[p.ty.as_str()].fp, p.r);
            for (pad, _, net, _) in &p.pads {
                let net = match moved {
                    Some((r, n)) if format!("{}.{pad}", p.r) == r => n,
                    _ if net == "NC" => "",
                    _ => net,
                };
                s += &format!(" (pad \"{pad}\" thru_hole circle (net 1 \"{net}\"))");
            }
            s += ")\n";
        }
        s + ")\n"
    };
    let ok = pcb_check(&pcb_parts(&board(None)).unwrap(), &parts, &types);
    assert!(ok.is_empty(), "PCB check fails a board equal to the netlist: {ok:?}");
    let bad = pcb_check(&pcb_parts(&board(Some(("U1.15", "PHI1")))).unwrap(), &parts, &types);
    if !bad.iter().any(|e| e.starts_with("PCB ")) {
        failures.push("PCB mutant survived".to_string());
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}
