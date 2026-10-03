// device_tests.rs - Storage and mount devices at port level (DEVICE_SPECS 2, 6, 7).
// Only IN/OUT (IoDevice read/write) and the storage directory; no private state.

use std::cell::RefCell;
use std::rc::Rc;

use intel8080_emu::io::devices::storage::Storage;
use intel8080_emu::io::devices::storage_mount::StorageMount;
use intel8080_emu::io::{IoBus, IoDevice};

struct Rig {
    dir: tempfile::TempDir,
    storage: Rc<RefCell<Storage>>,
    mount: StorageMount,
}

fn rig() -> Rig {
    let dir = tempfile::tempdir().unwrap();
    let storage = Rc::new(RefCell::new(Storage::new()));
    let mount = StorageMount::new(Rc::clone(&storage), dir.path().to_path_buf());
    Rig { dir, storage, mount }
}

impl Rig {
    fn inp(&mut self, port: u8) -> u8 {
        match port {
            0x08..=0x0C => self.storage.borrow_mut().read(port),
            _ => self.mount.read(port),
        }
    }
    fn out(&mut self, port: u8, value: u8) {
        match port {
            0x08..=0x0C => self.storage.borrow_mut().write(port, value),
            _ => self.mount.write(port, value),
        }
    }
    /// Send `name` to 0D, then mount; returns IN 0F.
    fn mount(&mut self, name: &[u8]) -> u8 {
        for &c in name {
            self.out(0x0D, c);
        }
        self.out(0x0E, 0x01);
        self.inp(0x0F)
    }
    fn address(&mut self) -> u32 {
        (self.inp(0x0A) as u32) << 16 | (self.inp(0x09) as u32) << 8 | self.inp(0x08) as u32
    }
    fn set_address(&mut self, a: u32) {
        self.out(0x08, a as u8);
        self.out(0x09, (a >> 8) as u8);
        self.out(0x0A, (a >> 16) as u8);
    }
}

// ---------- Storage (ports 08-0C) ----------

#[test]
fn power_on_status_is_82() {
    let mut r = rig();
    assert_eq!(r.inp(0x0C), 0x82);
    assert_eq!(r.address(), 0);
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
    assert_eq!(r.inp(0x0B), 0xFF, "past EOF reads FF");
    r.out(0x0C, 0x02); // flush
    let disk = std::fs::read(r.dir.path().join("RW.BIN")).unwrap();
    assert_eq!(disk.len(), 0x13);
    assert_eq!(disk[0x10..], [0xAA, 0xBB, 0xCC]);
}

#[test]
fn data_when_not_mounted_reads_ff_and_writes_nothing() {
    let mut r = rig();
    assert_eq!(r.inp(0x0B), 0xFF);
    r.out(0x0B, 0x55);
    assert_eq!(r.inp(0x0C) & 0x01, 0);
    assert_eq!(std::fs::read_dir(r.dir.path()).unwrap().count(), 0, "a file appeared");
}

// ---------- Mount (ports 0D-0F) ----------

#[test]
fn mount_opens_the_named_file_in_the_storage_dir() {
    let mut r = rig();
    std::fs::write(r.dir.path().join("TEST.BIN"), [0xDE, 0xAD, 0xBE, 0xEF]).unwrap();
    assert_eq!(r.mount(b"TEST.BIN"), 0x00);
    assert_eq!(r.inp(0x0C), 0x03, "size is the file's length");
    assert_eq!([r.inp(0x0B), r.inp(0x0B), r.inp(0x0B), r.inp(0x0B)], [0xDE, 0xAD, 0xBE, 0xEF]);
    assert_eq!(r.inp(0x0C), 0x83);
}

#[test]
fn mount_creates_a_missing_file() {
    let mut r = rig();
    assert_eq!(r.mount(b"NEW.BIN"), 0x00);
    assert_eq!(std::fs::metadata(r.dir.path().join("NEW.BIN")).unwrap().len(), 0);
}

#[test]
fn mount_and_unmount_reset_the_address() {
    let mut r = rig();
    r.set_address(0x050505);
    assert_eq!(r.mount(b"A.BIN"), 0x00);
    assert_eq!(r.address(), 0);
    r.set_address(0x050505);
    assert_eq!(r.mount(b"A.BIN"), 0x00, "remount of the same name");
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
fn name_buffer_is_cleared_after_mount() {
    let mut r = rig();
    assert_eq!(r.mount(b"A.BIN"), 0x00);
    assert_eq!(r.mount(b"B.BIN"), 0x00);
    assert!(r.dir.path().join("B.BIN").exists(), "second mount saw a stale name");
}

#[test]
fn nul_name_bytes_are_ignored() {
    let mut r = rig();
    assert_eq!(r.mount(b"N\0.BIN"), 0x00);
    assert!(r.dir.path().join("N.BIN").exists());
}

#[test]
fn valid_names_include_12_chars_dash_and_underscore() {
    let mut r = rig();
    for name in ["LONGNAME1234", "A-B_C.BIN", "A.B.C"] {
        assert_eq!(r.mount(name.as_bytes()), 0x00, "{}", name);
        assert!(r.dir.path().join(name).exists(), "{}", name);
    }
}

#[test]
fn invalid_names_are_02() {
    for name in [&b""[..], b"BAD/NAME", b"../X", b"A B", b"A*"] {
        let mut r = rig();
        assert_eq!(r.mount(name), 0x02, "{:?}", String::from_utf8_lossy(name));
        assert_eq!(r.inp(0x0C) & 0x01, 0);
    }
}

#[test]
fn host_open_failure_is_01() {
    let mut r = rig();
    std::fs::create_dir(r.dir.path().join("DIR")).unwrap();
    assert_eq!(r.mount(b"DIR"), 0x01);
    assert_eq!(r.inp(0x0C) & 0x01, 0);
}

#[test]
fn write_only_ports_read_ff() {
    let mut r = rig();
    assert_eq!(r.inp(0x0D), 0xFF);
    assert_eq!(r.inp(0x0E), 0xFF);
}

// ---------- Bus ----------

#[test]
fn unassigned_port_reads_ff() {
    let mut bus = IoBus::new();
    bus.write(0x05, 0x12);
    assert_eq!(bus.read(0x05), 0xFF);
}
