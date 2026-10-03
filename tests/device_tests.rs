// device_tests.rs - Console, storage and mount at port level (DEVICE_SPECS 1, 2, 4, 6, 7).
// Every access goes through the bus that build_bus returns (the port map main.rs uses),
// plus the console's host side and the storage directory; no private state.
//
// Not testable here: the I/O error paths (no fault injection) and whether flush,
// unmount and Drop reach the disk (fsync can't be observed). Both are by review.

use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::rc::Rc;

use intel8080_emu::io::build_bus;
use intel8080_emu::io::devices::console::{Console, OUTPUT_CAP};
use intel8080_emu::io::IoBus;

struct Rig {
    dir: tempfile::TempDir,
    bus: IoBus,
    con: Rc<RefCell<Console>>,
}

fn rig() -> Rig {
    let dir = tempfile::tempdir().unwrap();
    let (bus, con) = build_bus(dir.path());
    Rig { dir, bus, con }
}

impl Rig {
    fn inp(&mut self, port: u8) -> u8 {
        self.bus.read(port)
    }
    fn out(&mut self, port: u8, value: u8) {
        self.bus.write(port, value);
    }
    fn send_name(&mut self, name: &[u8]) {
        for &c in name {
            self.out(0x0D, c);
        }
    }
    /// Send `name` to 0D, then mount; returns IN 0F.
    fn mount(&mut self, name: &[u8]) -> u8 {
        self.send_name(name);
        self.out(0x0E, 0x01);
        self.inp(0x0F)
    }
    fn mounted(&mut self) -> bool {
        self.inp(0x0C) & 0x01 != 0
    }
    fn address(&mut self) -> u32 {
        (self.inp(0x0A) as u32) << 16 | (self.inp(0x09) as u32) << 8 | self.inp(0x08) as u32
    }
    fn set_address(&mut self, a: u32) {
        self.out(0x08, a as u8);
        self.out(0x09, (a >> 8) as u8);
        self.out(0x0A, (a >> 16) as u8);
    }
    fn path(&self, name: &str) -> PathBuf {
        self.dir.path().join(name)
    }
    /// File names in the storage directory, exactly as stored.
    fn files(&self) -> Vec<String> {
        let mut names: Vec<String> = std::fs::read_dir(self.dir.path())
            .unwrap()
            .map(|e| e.unwrap().file_name().into_string().unwrap())
            .collect();
        names.sort();
        names
    }
}

// ---------- Port map ----------

#[test]
fn unassigned_pi_window_ports_read_ff() {
    let mut r = rig();
    for port in (0x03..=0x07).chain(0x10..=0x6F) {
        r.out(port, 0x00);
        assert_eq!(r.inp(port), 0xFF, "port {:02X}", port);
    }
}

#[test]
fn write_only_ports_read_ff() {
    let mut r = rig();
    for port in [0x00, 0x0D, 0x0E] {
        assert_eq!(r.inp(port), 0xFF, "port {:02X}", port);
    }
}

#[test]
fn writes_to_read_only_ports_are_ignored() {
    let mut r = rig();
    r.out(0x01, 0x41);
    r.out(0x02, 0x41);
    assert_eq!(r.inp(0x02), 0x02);
    assert!(r.con.borrow().output().is_empty());
    r.out(0x0F, 0x00);
    assert_eq!(r.inp(0x0F), 0x01);
}

// ---------- Console (ports 00-02) ----------

#[test]
fn console_power_on_and_empty_fifo() {
    let mut r = rig();
    assert_eq!(r.inp(0x02), 0x02);
    assert_eq!(r.inp(0x01), 0x00, "IN 01 with the FIFO empty");
    assert_eq!(r.inp(0x02), 0x02, "and it changed nothing");
}

#[test]
fn console_input_is_a_fifo() {
    let mut r = rig();
    r.con.borrow_mut().push_input(b"AB");
    r.con.borrow_mut().push_input(b"C");
    assert_eq!(r.inp(0x02), 0x03);
    assert_eq!(r.inp(0x02), 0x03, "IN 02 has no side effect");
    assert_eq!([r.inp(0x01), r.inp(0x01)], *b"AB");
    assert_eq!(r.inp(0x02), 0x03);
    assert_eq!(r.inp(0x01), b'C');
    assert_eq!(r.inp(0x02), 0x02);
    assert_eq!(r.inp(0x01), 0x00);
}

#[test]
fn console_host_side_sees_status_polls() {
    let mut r = rig();
    assert!(!r.con.borrow_mut().take_polled(), "power-on");
    r.inp(0x01);
    r.out(0x00, 0x41);
    assert!(!r.con.borrow_mut().take_polled(), "IN 01 and OUT 00 are not polls");
    r.inp(0x02);
    r.inp(0x02);
    assert!(r.con.borrow_mut().take_polled());
    assert!(!r.con.borrow_mut().take_polled(), "take clears");
}

#[test]
fn console_is_8_bit_transparent() {
    let mut r = rig();
    let all: Vec<u8> = (0..=255).collect();
    r.con.borrow_mut().push_input(&all);
    let echoed: Vec<u8> = (0..256).map(|_| r.inp(0x01)).collect();
    assert_eq!(echoed, all);
    for &b in &all {
        r.out(0x00, b);
    }
    assert_eq!(r.con.borrow_mut().take_output(), all);
    assert!(r.con.borrow().output().is_empty(), "take_output drains");
}

#[test]
fn console_output_discards_when_2_mib_are_undrained() {
    let mut r = rig();
    assert!(OUTPUT_CAP >= 2 * 1024 * 1024);
    for i in 0..OUTPUT_CAP + 5 {
        r.out(0x00, i as u8);
    }
    let out = r.con.borrow_mut().take_output();
    assert_eq!(out.len(), OUTPUT_CAP);
    assert_eq!(out[OUTPUT_CAP - 1], (OUTPUT_CAP - 1) as u8, "the oldest bytes are kept");
    r.out(0x00, 0x55);
    assert_eq!(r.con.borrow().output(), [0x55], "room again after a drain");
}

// ---------- Storage (ports 08-0C) ----------

#[test]
fn storage_power_on() {
    let mut r = rig();
    assert_eq!(r.inp(0x0C), 0x82);
    assert_eq!(r.address(), 0);
    assert_eq!(r.inp(0x0F), 0x01, "mount status reads 01, what a query would return");
}

#[test]
fn address_bytes_in_any_order() {
    let mut r = rig();
    r.out(0x0A, 0x12);
    r.out(0x09, 0x34);
    r.out(0x08, 0x56);
    assert_eq!(r.address(), 0x123456);
    r.out(0x09, 0xAB); // replaces only its own byte
    assert_eq!(r.address(), 0x12AB56);
}

#[test]
fn control_00_zeroes_01_decrements_and_wraps() {
    let mut r = rig();
    r.set_address(0x010000);
    r.out(0x0C, 0x01);
    assert_eq!(r.address(), 0x00FFFF, "decrement borrows across bytes");
    r.out(0x0C, 0x00);
    assert_eq!(r.address(), 0);
    r.out(0x0C, 0x01);
    assert_eq!(r.address(), 0xFFFFFF, "decrement wraps 000000 to FFFFFF");
    r.out(0x0C, 0x07); // undefined control value: ignored
    assert_eq!(r.address(), 0xFFFFFF);
}

#[test]
fn status_tracks_mount_and_eof() {
    let mut r = rig();
    assert_eq!(r.mount(b"A.BIN"), 0x00);
    assert_eq!(r.inp(0x0C), 0x83, "mounted, empty file: EOF");
    r.out(0x0B, 0x11);
    assert_eq!(r.inp(0x0C), 0x83, "address 1 = size 1: EOF");
    r.out(0x0C, 0x00);
    assert_eq!(r.inp(0x0C), 0x03, "inside the file");
}

#[test]
fn data_reads_its_writes_and_auto_increments() {
    let mut r = rig();
    assert_eq!(r.mount(b"RW.BIN"), 0x00);
    r.set_address(0x000010);
    for b in [0xAA, 0xBB, 0xCC] {
        r.out(0x0B, b);
    }
    assert_eq!(r.address(), 0x000013);
    // The gap before the first write reads back as 00.
    r.out(0x0C, 0x00);
    for _ in 0..0x10 {
        assert_eq!(r.inp(0x0B), 0x00);
    }
    assert_eq!([r.inp(0x0B), r.inp(0x0B), r.inp(0x0B)], [0xAA, 0xBB, 0xCC]);
    assert_eq!(r.address(), 0x000013);
    let disk = std::fs::read(r.path("RW.BIN")).unwrap();
    assert_eq!(disk.len(), 0x13);
    assert_eq!(disk[0x10..], [0xAA, 0xBB, 0xCC]);
}

#[test]
fn past_eof_read_is_ff_and_advances() {
    let mut r = rig();
    std::fs::write(r.path("TWO.BIN"), [0x01, 0x02]).unwrap();
    assert_eq!(r.mount(b"TWO.BIN"), 0x00);
    r.set_address(0x000002);
    assert_eq!([r.inp(0x0B), r.inp(0x0B)], [0xFF, 0xFF]);
    assert_eq!(r.address(), 0x000004);
    assert_eq!(r.inp(0x0C), 0x83, "still mounted, still EOF");
    assert_eq!(std::fs::read(r.path("TWO.BIN")).unwrap(), [0x01, 0x02], "a read never grows the file");
}

#[test]
fn not_mounted_data_is_ff_discards_writes_and_advances() {
    let mut r = rig();
    assert_eq!(r.inp(0x0B), 0xFF);
    assert_eq!(r.address(), 0x000001);
    r.out(0x0B, 0x55);
    assert_eq!(r.address(), 0x000002);
    r.set_address(0xFFFFFF);
    assert_eq!(r.inp(0x0B), 0xFF);
    assert_eq!(r.address(), 0x000000, "increment wraps FFFFFF to 000000");
    assert_eq!(r.inp(0x0C), 0x82);
    assert!(r.files().is_empty(), "a file appeared: {:?}", r.files());
}

#[test]
fn write_at_ffffff_makes_a_16_mb_file_and_wraps() {
    let mut r = rig();
    assert_eq!(r.mount(b"BIG.BIN"), 0x00);
    r.set_address(0xFFFFFF);
    r.out(0x0B, 0x77);
    assert_eq!(r.address(), 0x000000);
    assert_eq!(r.inp(0x0C), 0x03, "address 0 is inside the file");
    r.out(0x0C, 0x01);
    assert_eq!(r.inp(0x0B), 0x77);
    assert_eq!(std::fs::metadata(r.path("BIG.BIN")).unwrap().len(), 0x100_0000);
    assert_eq!(r.mount(b"BIG.BIN"), 0x00, "exactly 16 MB mounts");
}

#[test]
fn flush_keeps_the_file_mounted() {
    let mut r = rig();
    r.out(0x0C, 0x02); // nothing mounted: no effect
    assert_eq!(r.inp(0x0C), 0x82);
    assert_eq!(r.mount(b"F.BIN"), 0x00);
    r.out(0x0B, 0x42);
    r.out(0x0C, 0x02);
    assert_eq!(r.inp(0x0C), 0x83);
    assert_eq!(r.address(), 0x000001, "flush leaves the address alone");
    assert_eq!(std::fs::read(r.path("F.BIN")).unwrap(), [0x42]);
}

#[test]
fn dropping_the_bus_keeps_unflushed_writes() {
    let mut r = rig();
    assert_eq!(r.mount(b"D.BIN"), 0x00);
    r.out(0x0B, 0x99);
    let Rig { dir, bus, con } = r;
    drop((bus, con));
    assert_eq!(std::fs::read(dir.path().join("D.BIN")).unwrap(), [0x99]);
}

// ---------- Mount (ports 0D-0F) ----------

#[test]
fn storage_dir_is_created_at_startup() {
    let dir = tempfile::tempdir().unwrap();
    let storage = dir.path().join("a/b");
    let (_bus, _con) = build_bus(&storage);
    assert!(storage.is_dir());
}

#[test]
fn every_mount_is_01_when_the_storage_dir_cannot_be_created() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("FILE"), b"").unwrap();
    let (mut bus, _con) = build_bus(&dir.path().join("FILE/storage"));
    for _ in 0..2 {
        for &c in b"A.BIN" {
            bus.write(0x0D, c);
        }
        bus.write(0x0E, 0x01);
        assert_eq!(bus.read(0x0F), 0x01);
        assert_eq!(bus.read(0x0C), 0x82);
    }
}

#[test]
fn mount_opens_the_named_file_in_the_storage_dir() {
    let mut r = rig();
    std::fs::write(r.path("TEST.BIN"), [0xDE, 0xAD, 0xBE, 0xEF]).unwrap();
    assert_eq!(r.mount(b"TEST.BIN"), 0x00);
    assert_eq!(r.inp(0x0C), 0x03, "size is the file's length");
    assert_eq!([r.inp(0x0B), r.inp(0x0B), r.inp(0x0B), r.inp(0x0B)], [0xDE, 0xAD, 0xBE, 0xEF]);
    assert_eq!(r.inp(0x0C), 0x83);
}

#[test]
fn mount_creates_a_missing_file() {
    let mut r = rig();
    assert_eq!(r.mount(b"NEW.BIN"), 0x00);
    assert_eq!(std::fs::metadata(r.path("NEW.BIN")).unwrap().len(), 0);
}

#[test]
fn mount_uppercases_the_name() {
    let mut r = rig();
    assert_eq!(r.mount(b"lower.bin"), 0x00);
    assert_eq!(r.mount(b"MiXeD_1"), 0x00);
    assert_eq!(r.files(), ["LOWER.BIN", "MIXED_1"]);
}

#[test]
fn every_mount_and_unmount_resets_the_address() {
    let mut r = rig();
    r.set_address(0x050505);
    assert_eq!(r.mount(b"A.BIN"), 0x00);
    assert_eq!(r.address(), 0);
    r.set_address(0x050505);
    assert_eq!(r.mount(b"A.BIN"), 0x00, "remount of the same name");
    assert_eq!(r.address(), 0);
    r.set_address(0x050505);
    assert_eq!(r.mount(b"BAD/"), 0x02, "a failed mount too");
    assert_eq!(r.address(), 0);
    r.set_address(0x050505);
    r.out(0x0E, 0x02);
    assert_eq!(r.inp(0x0F), 0x00);
    assert_eq!(r.address(), 0);
    assert_eq!(r.inp(0x0C), 0x82);
}

#[test]
fn unmount_with_nothing_mounted_is_00() {
    let mut r = rig();
    r.out(0x0E, 0x02);
    assert_eq!(r.inp(0x0F), 0x00);
}

#[test]
fn query_reports_mounted() {
    let mut r = rig();
    r.out(0x0E, 0x03);
    assert_eq!(r.inp(0x0F), 0x01);
    assert_eq!(r.mount(b"A.BIN"), 0x00);
    r.set_address(0x000123);
    r.out(0x0E, 0x03);
    assert_eq!(r.inp(0x0F), 0x00);
    assert_eq!(r.address(), 0x000123, "query leaves the address alone");
}

#[test]
fn every_0e_write_clears_the_name_buffer() {
    for cmd in [0x01, 0x02, 0x03, 0x00, 0x07, 0xFF] {
        let mut r = rig();
        r.send_name(b"JUNK");
        r.out(0x0E, cmd);
        assert_eq!(r.mount(b"A.BIN"), 0x00, "after OUT 0E,{:02X}", cmd);
        assert!(r.files().contains(&"A.BIN".to_string()), "OUT 0E,{:02X} kept the name: {:?}", cmd, r.files());
    }
}

#[test]
fn undefined_commands_leave_the_status() {
    let mut r = rig();
    assert_eq!(r.mount(b"BAD/"), 0x02);
    r.out(0x0E, 0x00);
    r.out(0x0E, 0x04);
    assert_eq!(r.inp(0x0F), 0x02);
}

#[test]
fn nul_name_bytes_are_ignored() {
    let mut r = rig();
    assert_eq!(r.mount(b"N\0.BIN"), 0x00);
    assert_eq!(r.files(), ["N.BIN"]);
}

#[test]
fn valid_names_include_12_chars_dash_and_underscore() {
    let mut r = rig();
    for name in ["LONGNAME1234", "A-B_C.BIN", "A.B.C", ".HIDDEN", "X"] {
        assert_eq!(r.mount(name.as_bytes()), 0x00, "{}", name);
        assert!(r.path(name).exists(), "{}", name);
    }
}

#[test]
fn invalid_names_are_02_and_never_truncated() {
    let long = [b'A'; 300];
    let names: [&[u8]; 11] =
        [b"", b".", b"..", b"LONGNAME12345", &long, b"BAD/NAME", b"../X", b"A B", b"A*", b"\xC3\xA9", b"A\x7F"];
    for name in names {
        let mut r = rig();
        assert_eq!(r.mount(name), 0x02, "{:?}", String::from_utf8_lossy(name));
        assert!(!r.mounted());
        assert!(r.files().is_empty(), "{:?}", r.files());
    }
}

#[test]
fn host_open_failure_is_01() {
    let mut r = rig();
    std::fs::create_dir(r.path("DIR")).unwrap();
    assert_eq!(r.mount(b"DIR"), 0x01);
    assert!(!r.mounted());
}

#[test]
fn file_over_16_mb_is_01() {
    let mut r = rig();
    let f = std::fs::File::create(r.path("HUGE.BIN")).unwrap();
    f.set_len(0x100_0001).unwrap(); // sparse
    drop(f);
    assert_eq!(r.mount(b"HUGE.BIN"), 0x01);
    assert!(!r.mounted());
}

#[test]
fn a_failed_mount_unmounts_the_previous_file() {
    let setup: [(&[u8], u8, fn(&Path)); 2] = [
        (b"BAD/", 0x02, |_| {}),
        (b"DIR", 0x01, |d| std::fs::create_dir(d.join("DIR")).unwrap()),
    ];
    for (name, status, prepare) in setup {
        let mut r = rig();
        prepare(r.dir.path());
        assert_eq!(r.mount(b"A.BIN"), 0x00);
        r.out(0x0B, 0x31); // unflushed: the unmount step must keep it
        assert_eq!(r.mount(name), status);
        assert_eq!(r.inp(0x0C), 0x82);
        r.out(0x0E, 0x03);
        assert_eq!(r.inp(0x0F), 0x01);
        assert_eq!(std::fs::read(r.path("A.BIN")).unwrap(), [0x31]);
    }
}

// ---------- Bus ----------

#[test]
fn unassigned_port_reads_ff() {
    let mut bus = IoBus::new();
    bus.write(0x05, 0x12);
    assert_eq!(bus.read(0x05), 0xFF);
}

#[test]
#[should_panic(expected = "port FE belongs to the CPU")]
fn map_port_rejects_fe() {
    IoBus::new().map_port(0xFE, Rc::new(RefCell::new(Console::new())));
}

#[test]
#[should_panic(expected = "port FF belongs to the CPU")]
fn map_port_rejects_ff() {
    IoBus::new().map_port(0xFF, Rc::new(RefCell::new(Console::new())));
}
