# Pi Daemon

Normative: the fourth normative spec, with `ARCHITECTURE.md`, `DEVICE_SPECS.md` and `MONITOR_SPEC.md` (decided 2026-10-03, COLLABORATION_LOG Key Decisions; binding). The one home for the software that runs on the Raspberry Pi behind the 8080's I/O ports: `pi8080d`. Implemented 2026-10-03 (`src/pi/`, `src/pi_main.rs`, `tests/pi_daemon_tests.rs`), with the simulated-board mode `--sim` (section 16, decided and built 2026-10-03); what only the built board can show is section 14.

**Scope (one fact, one home):**
- `ARCHITECTURE.md` 6.4 owns the circuit, the 20-GPIO pin map, the IN/OUT handshake and the power and boot rules. `ARCHITECTURE.md` 6.6 owns RESET. `ARCHITECTURE.md` 7.3 owns the port-trace line format and 7.4 the trace diff recipe. This file says how the daemon meets them and cites them; it does not restate them.
- `DEVICE_SPECS.md` owns every port's behavior, rule 2.8 (RESET), rule 2.9 (service restart), section 3 (the READY contract and the Pi obligations), section 4 (console) and section 8 (mailbox, TIME clock, `GET`, `ASK`).
- `HARDWARE_BUILD.md` owns the bring-up steps and the platform decisions (PI-PLATFORM, CONSOLE-TRANSPORT, CONSOLE-OUTPUT, RESET-TIMING, RESET-SOURCE, DEV-RESET, TRACE-FORMAT). Its section 5 keeps the decisions and points here for the service model.

**Conventions:** "MUST" binds the daemon. **[bench]** marks something only a measurement on the built board can close; section 14 is the one list of them. BCM numbers are GPIO numbers as in `ARCHITECTURE.md` 6.4. Hex is uppercase. A bare section number ("5.2") is a section of this file; every other reference names its document ("`DEVICE_SPECS.md` 3.3", meaning section 3 item 3).

The rule behind every section: **the daemon is the emulator's port map behind GPIO instead of behind the CPU model.** Same devices, same `build_bus`, one IoBus access per 8080 access. Everything else in it is transport.

---

## 1. Shape

- One process, **one thread**. The thread owns the IoBus, busy-polls GPLEV0, serves each request inline, and in the gaps runs the console's TCP socket and the trace file. The IoBus and devices stay `Rc<RefCell<..>>` and are never shared across threads (devices are not `Send`: COLLABORATION_LOG Key Decisions, 2026-10-03, Pi Daemon). Under `--sim` (16) a second thread plays the 8080 on the simulated board; this thread does not change.
- No interrupts, no async runtime, no worker threads. `TIME`, `ASM` and `DIS` complete within the execute access, inline on this thread (`DEVICE_SPECS.md` 3.3, 8). A background mailbox command (`GET`) runs as a **child process** (`curl`); `ASK` (Phase 9) is a background command too, its worker a `curl` process as GET's. The mailbox device spawns it. This thread only spawns it, reads its pipe non-blocking, waits for its exit non-blocking and kills it (`DEVICE_SPECS.md` 8, Background commands). The IoBus and the devices never leave this thread.
- **The worker's cores and lifetime.** On Linux the device spawns the worker with a `pre_exec` (`libc`) that does two things:
  - **Affinity.** A child inherits the affinity of the thread that forks it, which is core 3 under the unit (11), and with `isolcpus=3` the kernel never moves it off. The device reads this thread's mask (`sched_getaffinity`) before the fork, and the `pre_exec` sets the worker's affinity to the online CPUs not in it (`sched_setaffinity`). If that set is empty (an unpinned process, as under `--sim` or in tests), the affinity is left alone.
  - **Lifetime.** `prctl(PR_SET_PDEATHSIG, SIGKILL)`: the worker dies with the thread that spawned it, which is this thread, the process's main thread. That covers a crash and a hand-run `--sim`, where systemd's cleanup (11) does not apply.
  On macOS (the emulator) neither applies. A worker orphaned by a killed emulator ends by itself: the stream form at its next write (the pipe is gone) or by the stall limit, the file form when its download ends; its temporary file is removed by the next `GET` to that file.
- The daemon serves every port 00-6F (`ARCHITECTURE.md` 6.3). It never sees 70-FF.

---

## 2. Crate Layout and Build

One crate, a second binary (`HARDWARE_BUILD.md` 5, Code). One crate dependency beyond `crossterm` and `libc`: `serde_json` (Phase 9, ASK), pure Rust, so the cross build is unchanged. One runtime dependency: `/usr/bin/curl` 8.4.0 or later, the `GET` and `ASK` worker (11).

| Path | Contents | Compiled on |
|------|----------|-------------|
| `src/pi/mod.rs` | Pin and register constants (3.2), the `Gpio` trait (3.1), `setup_pins` (3.3), `serve`: the bus loop (4), RESET handling (5), console pass (7). Portable. | every target |
| `src/pi/linux.rs` | `GpioMem` (the real `Gpio`: `/dev/gpiomem` mmap plus the RESET line request), `ntp_local_time` (8). `#[cfg(target_os = "linux")]` | Linux only |
| `src/pi/sim.rs` | `SimBoard`, the simulated board (13.1), and `Bridge`, the 8080 side's `IoDevice` (13.2, 16.2). Portable. In the library since 2026-10-03 so that `--sim` can run it; its fault knobs and test helpers ship in the binary, unused by `--sim`. | every target |
| `src/pi_main.rs` | `pi8080d`: arguments (10), signal flag, startup order (10); calls `pi::serve` with `GpioMem`, or under `--sim` with the simulated board and the 8080 thread (16). `cfg(unix)`. Without `--sim` on a Unix other than Linux it prints `pi8080d: Linux only (or --sim FILE)` and exits 2; on a non-Unix OS, `pi8080d: Unix only`, exit 2. | every target |
| `src/lib.rs` | `pub mod pi;` | every target |
| `tests/pi_daemon_tests.rs` | Fault, RESET, startup, stop and console tests (13.3). | every target |
| `tests/monitor_tests.rs` | Gains the daemon path for the transcripts (13.2), the `--sim` binary test (16.5, Unix) and the ignored `w_command_cycles` helper (12.2). | every target |
| `scripts/pi8080d.service` | The systemd unit (11). | (installed on the Pi) |
| `scripts/pi8080d-sim.conf` | The `--sim` drop-in for the unit (16.4). | (installed on the Pi, or any Linux box) |
| `.cargo/config.toml` | `rust-lld` as the linker for the musl target (below). | (build host) |

`Cargo.toml` gains `default-run`, so plain `cargo run` still runs the emulator:

```toml
[package]
default-run = "intel8080"

[[bin]]
name = "pi8080d"
path = "src/pi_main.rs"
```

The entry point, so tests and `pi_main` call the same thing:

```rust
pub fn serve<G: Gpio>(gpio: G, fsel2: Fsel2, storage: &Path, clock: mailbox::Clock, ask: AskConfig,
                      listener: TcpListener, trace: Option<File>, stop: &AtomicBool)
                      -> Result<(), String>
```

`serve` builds the bus inside the calling thread, so the `Rc` devices never cross threads. `fsel2` comes from `setup_pins` (3.3). `ask` is the ASK settings with the key read at startup (10). `pi_main` passes a static stop flag set by its signal handler; each test passes its own. `serve` returns `Ok(())` when `stop` is set and `Err` only for an error that ends service. Nothing in `src/pi/` calls `exit`: only `pi_main` turns an `Err` into exit 1.

**cfg rule.** Only the code that touches Linux interfaces is behind `cfg(target_os = "linux")`: the mmap, the GPIO character-device ioctls, `adjtimex`. Not `target_arch`: the code compiles on any Linux, and on anything but a BCM2711 it refuses to start (3.3). Everything the tests need (the loop, RESET handling, console, trace) is portable, so `cargo test` on macOS exercises it through the simulated board.

**Keeping the Linux code honest.** `cargo test` on macOS never compiles `src/pi/linux.rs`. The gate that closes that gap: a change under `src/pi/` or to `src/pi_main.rs` is green only when `cargo check` and `cargo clippy` with `--target aarch64-unknown-linux-musl --bins` also pass (one-time `rustup target add aarch64-unknown-linux-musl`). Mike added it to `CLAUDE.md` on 2026-10-03 (Rules and End of Session), where it runs before every commit. `cargo check` does not link, so it needs no cross linker. The kernel-ABI asserts in 5.1 make that gate check the struct layouts.

**Building the Pi binary.** A static musl binary, cross-built on the Mac with the linker Rust already ships:

```toml
# .cargo/config.toml
[target.aarch64-unknown-linux-musl]
linker = "rust-lld"
```

```
cargo build --release --target aarch64-unknown-linux-musl --bin pi8080d
scp target/aarch64-unknown-linux-musl/release/pi8080d pi:/tmp/
ssh pi sudo install -m 0755 /tmp/pi8080d /usr/local/bin/pi8080d
```

`/usr/local/bin` is root's, so the copy goes through `/tmp` and `sudo install`, which also leaves the binary root-owned: the `pi8080` service user cannot replace it.

No Docker, no zig, no Pi toolchain. Verified 2026-10-03 on macOS (rustc 1.89): it links a static aarch64 ELF of about 760 KiB. Running it on the Pi is a section 14 check. If `rust-lld` ever does not link it, the fallback is to build natively on the Pi (`cargo build --release --bin pi8080d`), which needs no config. `build.rs` runs on the build host either way.

---

## 3. GPIO Front End

### 3.1 The seam

```rust
/// The Pi's view of the board: the BCM2711 GPIO register block and the RESET edge latch.
pub trait Gpio {
    /// 32-bit read of the register at byte offset `off` (volatile on hardware).
    fn read(&self, off: usize) -> u32;
    /// 32-bit write of the register at byte offset `off` (volatile on hardware).
    fn write(&self, off: usize, value: u32);
    /// True if RESET changed level at or after the previous call (the first call: since
    /// the line was requested). Edges from before the previous call are drained and ignored.
    fn reset_edge(&mut self) -> bool;
}
```

- **Register level, not signal level.** The real implementation is two `read_volatile`/`write_volatile` one-liners and a non-blocking `read` of the line-event fd. Every register offset, bit mask, FSEL field and handshake step lives in portable code that the tests run. A signal-level trait (`ack(bool)`, `drive(u8)`) would hide exactly the code most likely to be wrong in the part the tests can't reach.
- **The `reset_edge` contract** is "edges since the previous call", judged by when the edge happened, not when it was delivered. A late-delivered edge from before the previous call is therefore ignored however late it arrives (5.1, 5.2).
- **Two implementations:** `GpioMem` (Linux) and `SimBoard` (the tests and `--sim`, 16). That is short of the rule of three. It is a deliberate exception, the same kind as the mailbox `Clock` seam: without it the bus loop, the code with the hardest failure modes (lost, doubled or hung accesses), could only be tested on the bench, and "nothing commits red" would not cover it. The alternatives are worse: a cfg-swapped concrete type can't run both in one build and still needs the simulator, and record-and-replay of register traffic tests the recording, not the protocol. Decided 2026-10-03: a logged exception to the rule of three (COLLABORATION_LOG Key Decisions).
- `serve` is generic over `G: Gpio`. No `dyn`, no `Box`.

### 3.2 Registers and pins

BCM2711 GPIO block (BCM2711 ARM Peripherals, section 5.2). `/dev/gpiomem` maps it at offset 0. The daemon touches only these:

| Register | Offset | Daemon use |
|----------|--------|------------|
| GPFSEL0 | 00 | Startup only: pins 4-9 to input |
| GPFSEL1 | 04 | Startup only: pins 10-13 to input, 16 (ACK) and 17 (LATCH) to output |
| GPFSEL2 | 08 | D0-D7 (pins 20-27) turnaround: one write per direction change |
| GPSET0 | 1C | Raise ACK, LATCH; drive D bits high |
| GPCLR0 | 28 | Lower ACK, LATCH; drive D bits low |
| GPLEV0 | 34 | Every read. One read is an atomic snapshot of all 20 signals (`ARCHITECTURE.md` 6.4) |
| GPIO_PUP_PDN_CNTRL_REG1 | E8 | Startup only: pull-down (2 bits per pin, `10`) on pins 16, 17, 20-27 |

The daemon MUST NOT write any other register. In particular, the event-detect registers (GPEDS0, GPREN0, GPFEN0, GPHEN0, GPLEN0, GPAREN0, GPAFEN0) belong to the kernel, which uses them for the RESET line request. Latching RESET through GPEDS0 from user space would race the kernel's interrupt handler.

Where each pin of the `ARCHITECTURE.md` 6.4 map sits (bit n of GPLEV0, GPSET0 and GPCLR0 is BCM n):

| Signal | GPLEV0 bits | Decoding |
|--------|-------------|----------|
| A0-A6 | 4-10 | `port = (lev >> 4) & 7F` |
| DIR | 11 | 1 = OUT (/I/OR high), 0 = IN |
| REQ | 12 | 1 = request |
| RESET | 13 | 1 = RESET asserted |
| ACK | 16 | output; read back here |
| LATCH | 17 | output; read back here |
| D0-D7 | 20-27 | `(lev >> 20) & FF` |

GPFSEL2 holds pins 20-29, three bits each (`000` input, `001` output). `setup_pins` reads it once and returns `Fsel2 { input, output }`: `input` has the D fields `000` and pins 28-29 as found, `output = input | 00249249`. The hot path writes those two values and never read-modify-writes.

BCM 18 (optional TEST_RESET, decision TEST-RESET) is not touched in v1.

### 3.3 Pin setup

`pi::setup_pins(&gpio) -> Result<Fsel2, String>` is the first thing the daemon does to the board (`ARCHITECTURE.md` 6.4, Power and boot independence). Startup order is in 10: only argument parsing and opening the register block come before it, so a restart after a crash mid-IN releases D0-D7, ACK and LATCH even if binding the listener then fails.

1. Read GPFSEL0-2. If any of the 20 pins is in an ALT function (field neither `000` nor `001`), return `Err("BCM n is in an ALT function")` before writing anything. That catches an enabled SPI (pins 7-11) or I2S/PCM (pins 18-21) overlay. It does **not** catch a kernel driver that holds one of the 20 pins as a plain GPIO (field `000` or `001`): the `w1-gpio` (1-Wire, pin 4) overlay, `gpio-led`, `gpio-keys`, `gpio-poweroff` and the like pass the check, and the kernel then switches the pin behind the daemon's back. Keeping them off is the installer's job (11). The daemon does not override the kernel.
2. GPFSEL2 = `input`: D0-D7 are inputs.
3. GPCLR0 = ACK | LATCH, so both are low as or before they become outputs.
4. Pull-down on 16, 17 and 20-27 (read-modify-write of REG1).
5. GPFSEL1: 16 and 17 to output, 10-13 to input; other fields unchanged. GPFSEL0: 4-9 to input; other fields unchanged.
6. Read GPLEV0. If ACK or LATCH reads high, return `Err("BCM 16 (ACK) reads high after drive low")` (or `BCM 17 (LATCH)`) and write nothing else. A board-side short would otherwise hang every access while the daemon looked healthy.

`GpioMem::open()` comes before it: refuse unless `/proc/device-tree/compatible` lists `brcm,bcm2711` (a Pi 5's RP1 has a different register model); open `/dev/gpiomem` read-write with `O_SYNC` and take an exclusive non-blocking `flock` on it (both in `pi::open_gpio`, the lock through `File::try_lock`), refusing with `/dev/gpiomem: another pi8080d holds the GPIO` if another daemon has it; `mmap` 4096 bytes, shared, offset 0. The fd stays open for the daemon's life, so the lock does too. The mapping is shared, so without the lock a second daemon (one run by hand beside the unit) would rewrite the running one's pins in `setup_pins` before the RESET line request or the bind could refuse it, hanging an ACK or D read-back loop mid-access. `gpio.request_reset()` (5.1) comes after `setup_pins`, so a RESET-line failure, the likeliest early-boot error, happens with the pins already released. Each failure returns `Err` with the path and the OS error.

---

## 4. Bus Service Loop

The loop body, in order. "Read" means a GPLEV0 read. The right column is the obligation each step meets.

| Step | Action | Obligation |
|------|--------|------------|
| 1 | If `stop` is set, stop (4.1). `s` = read. If `s` has RESET, go to RESET handling (5.2). | `DEVICE_SPECS.md` 2.8 |
| 2 | If `s` has no REQ: if at least 1 ms has passed since the last console pass, run one (7.3). Go to 1. | `DEVICE_SPECS.md` 3.3: sense requests by level; never wait for REQ low |
| 3 | `port` = A0-A6 from `s`. DIR from `s`. | `DEVICE_SPECS.md` 3.3: sample from a read in which REQ is high (`ARCHITECTURE.md` 6.4, When the Pi may sample) |
| 4 OUT | `v` = D0-D7 from `s`. `bus.write(port, v)`. | `DEVICE_SPECS.md` 3.3: release only after the operation completed |
| 4 IN | `v = bus.read(port)`. Read (abort check). GPCLR0 = `(!v & FF) << 20`, GPSET0 = `v << 20`, then GPFSEL2 = `output` (values before direction, so no wrong byte is ever driven). Read until D0-D7 equal `v`, then read once more. GPSET0 = LATCH; read until LATCH is high. GPCLR0 = LATCH. GPFSEL2 = `input`. | `ARCHITECTURE.md` 6.4, IN cycle steps 1-4; `DEVICE_SPECS.md` 3.3: read-back between dependent steps |
| 5 | Pre-ACK check: read. If it has RESET, or has no REQ, or (more than 1 ms has passed since the last `reset_edge()` call and `gpio.reset_edge()` is true): make sure D0-D7 are inputs and LATCH is low, and go to RESET handling **without raising ACK**. | `ARCHITECTURE.md` 6.6 and `DEVICE_SPECS.md` 3.3: the RESET check before every ACK, with the latch asked behind the 1 ms gate (5.1) |
| 6 | GPSET0 = ACK. Read until ACK is high. Note the time `t`. | `ARCHITECTURE.md` 6.4 rule 2: read ACK back high |
| 7 | Add the trace line (9), if tracing. | Outside the READY window |
| 8 | Spin until 500 ns have passed since `t` (`Instant`). GPCLR0 = ACK. Read until ACK is low. Go to 1. | `ARCHITECTURE.md` 6.4 rule 2: at least 500 ns before REQ is treated as a new access |

**Abort rule.** Between step 3 and ACK, every read in steps 4 and 5 (the abort check before D0-D7 become outputs, each read of the D and LATCH read-back loops, and the pre-ACK read) leaves for RESET handling when it shows RESET high **or REQ low**. An access that has not been ACKed can lose REQ only through RESET (the WAIT flip-flop is cleared only by ACK or RESET, `ARCHITECTURE.md` 6.4), so REQ low means a RESET pulse came and went, even one too short to still see. The step 4 IN reads also leave when they show **DIR high**: during an IN only a new access can show it, after a pulse that came and went during a slow `bus.read` and a reboot whose first Pi-window access (an OUT; the monitor's is always `OUT 00`) is already up. This is what keeps a pulse during an IN from leaving D0-D7 driven, or driven against the data 74LVC245A, or a read-back loop spinning.

Notes:
- The daemon never waits for REQ to go low. After step 8 a high REQ is a new access; the WAIT flip-flop guarantees it (`ARCHITECTURE.md` 6.4 rule 2).
- Step 2 is the only place the daemon does socket or file work while idle, and never between REQ and ACK.
- If D0-D7 have not matched `v` after 1 ms, the daemon logs `D read-back mismatch: drove XX, read YY` to stderr once per access and keeps trying (a board fault). The abort rule and `stop` still end the loop.
- Device work is whatever `bus.read`/`bus.write` do: bounded but not fast (`DEVICE_SPECS.md` 3.3). An fsync holds READY for as long as it takes. That is the contract.
- At startup the loop starts at step 1 with whatever the board shows: a request already pending is served (`ARCHITECTURE.md` 6.4, last rule).

### 4.1 Stop

`stop` is loaded (`Relaxed`) at step 1 and in every wait loop: the RESET release wait (5.2), the D and LATCH read-backs (step 4) and the ACK read-backs (steps 6 and 8). The loops a board fault can make endless (RESET held, a stuck D bit, REQ stuck high) therefore all end on SIGTERM or Ctrl-C, before systemd's SIGKILL could skip Storage's flush. On stop:

1. GPFSEL2 = `input`; GPCLR0 = LATCH | ACK. An access not yet ACKed stays un-ACKed; the next daemon serves it with fresh devices (`DEVICE_SPECS.md` 2.9).
2. Write the trace's pending line.
3. Drop the bus (Storage's `Drop` flushes durably and closes). Return `Ok(())`.

A device call in progress (an fsync) finishes first; `stop` is seen at the next check.

---

## 5. RESET

Contract: `ARCHITECTURE.md` 6.6 and `DEVICE_SPECS.md` 2.8. Decisions RESET-TIMING, RESET-SOURCE and DEV-RESET.

### 5.1 The edge latch

- `GpioMem::request_reset()` finds the BCM2711 controller by scanning `/dev/gpiochip*` with `GPIO_GET_CHIPINFO_IOCTL` for the label `pinctrl-bcm2711`, and returns `Err` naming the labels it found if none matches (kernel updates have renumbered gpiochips before). On that chip it requests line 13 as an input with both-edge events through the v2 line-request ioctl (`GPIO_V2_GET_LINE_IOCTL`) via `libc`. No crate (rppal is archived; `gpiocdev` was declined 2026-10-03, COLLABORATION_LOG Key Decisions). The returned fd is set `O_NONBLOCK`. Event timestamps are the default `CLOCK_MONOTONIC`.
- The kernel ABI is hand-written `#[repr(C)]`, so it is pinned at compile time, which the musl `cargo check` gate (2) evaluates:

  ```rust
  const _: () = assert!(size_of::<GpioV2LineRequest>() == 592 && size_of::<GpioV2LineEvent>() == 48
                        && size_of::<GpiochipInfo>() == 68);
  const GPIO_V2_GET_LINE_IOCTL: u32 = 0xC250_B407;   // _IOWR(0xB4, 0x07, 592)
  const GPIO_GET_CHIPINFO_IOCTL: u32 = 0x8044_B401;  // _IOR(0xB4, 0x01, 68)
  ```

- `reset_edge()` records `clock_gettime(CLOCK_MONOTONIC)` at every call. It reads events until `EAGAIN` and returns true only if one has `timestamp_ns` at or after the previous call's time (3.1). The kernel queues events, so a pulse is seen however short it was and whatever the bus thread was doing, an fsync included.
- The level comes from GPLEV0 bit 13 in every read, for free.
- **The 1 ms gate (step 5).** RESET-SOURCE is a DS1813, which holds RESET for at least 100 ms (typ 150, DS1813 AC table) after any assertion, the button and TEST_RESET included. Within 1 ms of the previous `reset_edge()` call no pulse can both start and end, so one that touches that interval is still high at step 5's level read, or was high at the previous call. Only when more than 1 ms has passed (a slow device call, a deschedule) can a whole pulse hide between level reads, and then step 5 asks the edge latch. That keeps the syscall off the hot path: on a busy bus at most one access per millisecond pays it, and while idle the console pass calls it once per millisecond (7.3). `ARCHITECTURE.md` 6.6 states the rule with this premise (decided 2026-10-03). If RESET-SOURCE is ever changed to a part that can make sub-millisecond pulses, the gate goes and step 5 calls `reset_edge()` on every access.

### 5.2 Handling

Entered from step 1 (level), steps 4-5 (level, REQ lost, or edge) or a console pass (edge while idle):

1. D0-D7 are inputs and LATCH and ACK are low (they already are everywhere but step 4).
2. The access in flight, if any, is dropped. It is never ACKed, now or later.
3. Trace `RESET` (9). Log `RESET` to stderr.
4. Wait for release: read until RESET reads low (checking `stop`, 4.1).
5. Call `reset_edge()` and ignore the result. The edges of this reset, including a release edge the kernel has not delivered yet, are now from before the previous call and will be ignored (3.1).
6. Rebuild: drop the old bus first (Storage's `Drop` flushes durably and closes), then `build_bus` (6). Drop the pending output chunk (7.2). Unless the client has half-closed, read and discard what the client socket holds (`DEVICE_SPECS.md` 4, Power-on and RESET: bytes still buffered in the transport are discarded).
7. Go to step 1. A REQ seen now is a new access; the 8080's first access after reset has been waiting under READY since about 0.12 ms after release.

No settle time: a late release edge is handled by the `reset_edge` contract, not by waiting it out. Every RESET pulse is a full device reset, as `ARCHITECTURE.md` 6.6 requires; a DS1813 does not bounce.

What this gives, by case:

| RESET arrives | Result |
|---------------|--------|
| While idle | Devices rebuilt at release. |
| After the daemon sampled a request, before ACK, still high at step 5 | Level. No ACK. The device call may or may not have taken effect on the old devices, which are then dropped (`DEVICE_SPECS.md` 2.8). |
| Pulse that came and went before step 5, 8080 still in reset or booting | REQ is low at an abort-rule read. Same as above. |
| Pulse that came and went during a slow device call, and the 8080 has rebooted and raised a new REQ | More than 1 ms passed, so step 5 asks the edge latch. Same as above; the new access is then served on fresh devices. During an IN, if the new access is an OUT, DIR high at the abort check before the drive stops it first, so D0-D7 never become outputs; if it is an IN, the drive is to a legal IN cycle and the step 5 latch still stops the ACK. |
| After ACK | The access completed. The reset is handled at the next step 1, step 5 or console pass. |

The one race `ARCHITECTURE.md` 6.6 accepts remains: descheduled between step 5 and step 6 for longer than the RESET pulse plus the boot path. The isolated core (11) makes it rarer still.

---

## 6. Devices

- Exactly `build_bus`, the function `main.rs` and every harness use (`DEVICE_SPECS.md` 10). No device is wrapped, subclassed or re-mapped. Device code is reused unchanged.
- The daemon passes its own TIME clock (8): `build_bus(storage_dir, clock, ask)`, with `main.rs` and the harnesses passing `mailbox::local_time` and the daemon `ntp_local_time` (decided 2026-10-03). `ask` carries the API key: the daemon passes the key it read at startup (10), `main.rs` likewise, the harnesses `AskConfig::default()` (no key).
- Storage directory: `--storage` (10), created by `Storage::new` if missing.
- Device reset = drop and `build_bus` again (5.2). Daemon restart = fresh devices, and the 8080 is not told (`DEVICE_SPECS.md` 2.9).

---

## 7. Console over TCP

Contract: `DEVICE_SPECS.md` 4. Decisions CONSOLE-TRANSPORT and CONSOLE-OUTPUT.

### 7.1 Connection

- One listener, bound at startup (`--listen`), non-blocking. Bind failure is an `Err` from `pi_main` (exit 1).
- A new client replaces the old one: the old socket is closed (its client sees EOF) and its unsent pending output is discarded with it. The new socket is non-blocking with `TCP_NODELAY` (interactive echo).
- **No authentication.** Whoever opens a TCP connection to the listener types monitor commands, and replaces the operator's client. That includes a web browser on any machine that can reach the port: a page can POST to it, the monitor answers each HTTP header line with an error, and the body then runs as commands (`:` records and `G` run any 8080 code). The console reaches everything the 8080 does: storage writes, `GET` from the Pi into its LAN and loopback, `ASK` on the owner's key. Keep the listener on loopback, reach it over the ssh tunnel to a Unix socket (11), and on a machine running `--sim` or a TCP tunnel remember that a browser there can reach it too.
- Raw bytes both ways, 8-bit transparent, nothing added. No telnet negotiation: a telnet client's IAC bytes would arrive as console input. Clients: `socat -,rawer,escape=0x1d TCP:host:port` (Ctrl-] quits; Ctrl-C reaches the 8080 as 03) or `nc` for scripts.
- **Half-close.** `read` returning 0 is input EOF only: the daemon stops reading that client and keeps sending it output. Scripted clients (`cat x.hex | nc pi 8080`, socat with a file source, transcript replay) shut down their write side and then wait for the output. The client is dropped only on a write error (EPIPE, ECONNRESET; Rust already ignores SIGPIPE), a read error other than `WouldBlock`, or a new client.

### 7.2 Buffers

- **Input.** The client socket is read only when the console's FIFO is empty (`!console.has_input()`), up to 4096 bytes per pass, into `push_input`. While the 8080 is not reading, bytes stay in the kernel and TCP flow control stops the sender. That is the out-of-band flow control `DEVICE_SPECS.md` 4 requires; nothing is dropped. No `Console` change is needed. A byte reaches the Pi when the daemon reads it from the socket; at RESET, bytes still in the socket are discarded (`DEVICE_SPECS.md` 4 owns that rule).
- **Output.** The console's own buffer is the output buffer: `OUTPUT_CAP` (2 MiB), bytes past it discarded, as in the emulator. A console pass moves it out with `take_output` only when the daemon's pending chunk is empty, then sends at most 16 KiB of the pending chunk per pass. A slow client therefore fills the console buffer to 2 MiB and the rest is discarded, never blocking `OUT 00`.
- **No client:** every pass takes the console output and drops it (`DEVICE_SPECS.md` 4: no terminal, byte discarded).
- **RESET:** the console is rebuilt with the bus, the pending chunk is dropped, and the socket's unread input is discarded at release (5.2). The client stays connected.

### 7.3 Console pass

Runs from loop step 2: REQ low and at least 1 ms since the last pass. In order:

1. If `gpio.reset_edge()`, go to RESET handling (a pulse missed while idle). This is the pass's only `reset_edge()` call.
2. Accept every pending connection; the last one wins.
3. With a client: read input (7.2) unless it has half-closed, then send output (7.2). Drop it per 7.1.
4. Without a client: drop the console output.

A pass costs a few syscalls (est. 5-20 us) once per millisecond: it delays at most one access. At 4 KiB in and 16 KiB out per millisecond it outruns anything the 8080 can consume or produce.

---

## 8. TIME Clock

Decided: `DEVICE_SPECS.md` 8 (TIME clock). Implementation:

```rust
#[cfg(target_os = "linux")]
pub fn ntp_local_time() -> Option<(u16, u8, u8, u8, u8, u8)> {
    // SAFETY: timex is plain data; modes = 0 only reads the kernel state.
    let mut tx: libc::timex = unsafe { std::mem::zeroed() };
    match unsafe { libc::adjtimex(&mut tx) } {
        -1 | libc::TIME_ERROR => None,
        _ => mailbox::local_time(),
    }
}
```

- A plain `fn`, so it is a `mailbox::Clock`. The formatting stays in the device.
- The time zone comes from `/etc/localtime`, set at install (`raspi-config` or `timedatectl set-timezone`). Changing it later needs a daemon restart.
- The kernel itself marks the clock unsynchronized after a long gap in NTP updates, and TIME then gives 83 until the next sync (`DEVICE_SPECS.md` 8).

---

## 9. Port Trace

- Optional: `--trace FILE`, created or truncated at startup. Off by default.
- Line format and repeat rule: `ARCHITECTURE.md` 7.3. The daemon reuses the debugger's `Trace` writer (`src/debugger.rs`), made `pub(crate)`, so both tools collapse repeats identically.
- One line per ACKed access: `IN pp vv` (the byte driven) or `OUT pp vv` (the byte sampled). One `RESET` line per handled reset (5.2). An access dropped by RESET has no line.
- Lines are added after ACK (loop step 7), outside READY. `Trace` writes and flushes when a run of identical lines ends, so tracing costs one `write` per distinct line on the bus thread. Turn it off for timing measurements.
- A BUSY wait of `N` or `Q` alternates `IN 12 01` with the Esc check's `IN 02 02` (`MONITOR_SPEC.md` 6.18), so its lines never collapse: a trace taken across a long wait grows by one line, and one `write`, per access (a 120 s `Q` wait is millions of lines).
- Diffing against an emulator trace: the `ARCHITECTURE.md` 7.4 recipe, which drops `RESET` lines, ports 70-FF (the daemon never sees them), and the empty-FIFO `IN 02 02` polls.

---

## 10. Configuration

Command-line flags, plus `ANTHROPIC_API_KEY` from the environment (Phase 9). The daemon reads no config file.

| Flag | Default | Meaning |
|------|---------|---------|
| `--listen ADDR:PORT` | `127.0.0.1:8080` | Console listener (7.1). Loopback by default, reached with `ssh -L` (11); pass a wildcard address to open it to the LAN, which gives every LAN host the unauthenticated console (7.1): storage writes, `GET` from the Pi, `ASK` on the key. Under systemd use a loopback or wildcard address (`0.0.0.0`, `[::]`), never an interface address: the unit does not wait for the network (11) |
| `--storage DIR` | required | Storage directory (6) |
| `--trace FILE` | none | Port trace (9) |
| `--sim FILE` | none | Simulated board (16): runs the 8080 model with the 4096-byte ROM image FILE instead of opening the GPIO |

- **API key:** `ANTHROPIC_API_KEY`, read once at startup (`DEVICE_SPECS.md` 8, ASK service). Unset or empty: every `ASK` gives 83. The daemon never writes the key anywhere: not to a log, the trace, a file, a child's arguments or environment, or the 8080. Changing it needs a restart.
- A bad or missing argument prints the usage line and exits 2, like the emulator: `usage: pi8080d --storage DIR [--listen ADDR:PORT] [--trace FILE] [--sim FILE]`.
- **CPU core:** not a flag. systemd `CPUAffinity=` (11) or `taskset -c 3` for a manual run pins the process.
- **Signals:** SIGTERM and SIGINT set a static `AtomicBool` (handler via `libc::signal`), which `pi_main` passes to `serve` as `stop` (4.1).
- **Startup order** (`pi_main`): parse arguments and read `ANTHROPIC_API_KEY`; `GpioMem::open()`, which takes the single-instance lock (3.3); `pi::setup_pins` (3.3); `gpio.request_reset()` (5.1); bind the listener; open the trace; `serve`. Under `--sim`: parse arguments and read the key; read the ROM image (16.4); `SimBoard::new` (16.2); `pi::setup_pins`; bind the listener; open the trace; start the 8080 thread (16.2); `serve`. Any `Err` prints `pi8080d: ` and the message to stderr and exits 1.
- Logging is stderr only (journald under systemd): startup settings, client connect, input EOF and disconnect, each RESET, errors. Nothing per access. The startup line gives the listener's bound address, so `--listen 127.0.0.1:0` reports its port: `pi8080d: console on ADDR:PORT, storage DIR, trace FILE|off, ask on|off`, plus `, board simulated, ROM FILE` under `--sim`. Nothing is logged per `ASK`.

---

## 11. Deployment

- **Pi:** Raspberry Pi 4B, 64-bit Raspberry Pi OS Lite (`HARDWARE_BUILD.md` 5). A heatsink: one core runs at 100% indefinitely.
- **`/boot/firmware/config.txt`:** nothing may claim BCM 4-13, 16, 17 or 20-27. SPI stays off (the default), no `w1-gpio` overlay, no I2S/PCM overlay. The daemon refuses to start on an ALT function (SPI, I2S/PCM), but it does **not** detect 1-Wire or any other overlay or driver that holds one of those pins as a plain GPIO (3.3): check every `dtoverlay=` and `gpio=` line in `config.txt` (and that 1-Wire is off in `raspi-config`) before the first start.
- **`/boot/firmware/cmdline.txt`:** append `isolcpus=3 irqaffinity=0-2`. Core 3 then runs only what is pinned to it. Whether the Pi OS kernel also supports `nohz_full=3 rcu_nocbs=3` is a [bench] item; without it the scheduler tick still interrupts core 3 for a few microseconds every tick, which shows up only as rare slow accesses.
- **Scheduling:** normal priority (`SCHED_OTHER`), no `SCHED_FIFO`. A busy-looping real-time task is throttled by the kernel's RT limit (50 ms of every second by default) and starves the per-CPU kernel threads; isolation, not priority, keeps the core to itself.
- **User:** a system user `pi8080` in group `gpio`. Raspberry Pi OS gives `/dev/gpiomem` and `/dev/gpiochip*` to `root:gpio` mode 0660 through udev, so the daemon needs no root and no capabilities.
- **Time zone:** set at install (8).
- **curl:** `GET`'s worker (`DEVICE_SPECS.md` 8, GET client): `/usr/bin/curl` 8.4.0 or later, which is the Trixie-based Raspberry Pi OS (Bookworm's 7.88.1 does not stop an oversized chunked body). Install it if the image lacks it: `sudo apt install curl`. The unit's default `KillMode=control-group` stops a running worker with the daemon, on stop, restart and crash alike.
- **API key** (Phase 9, optional): `sudo install -d -m 0700 /etc/pi8080d && sudo sh -c 'umask 077; cat > /etc/pi8080d/env'`, then type `ANTHROPIC_API_KEY=sk-ant-...`, Enter, Ctrl-D. The key goes through the terminal only, never a command line or shell history. The file is root's, mode 0600: systemd reads it before dropping to `pi8080`, so the daemon gets the variable but cannot read the file. Without the file the daemon starts anyway (the `-` below) with `ask off`. A spend limit on the key's workspace in the Anthropic Console caps cost; the daemon keeps no count.
- **Unit** `/etc/systemd/system/pi8080d.service` (the repo copy is `scripts/pi8080d.service`):

```ini
[Unit]
Description=8080 Pi daemon (bus service, console, storage)
StartLimitIntervalSec=0

[Service]
User=pi8080
Group=gpio
ExecStart=/usr/local/bin/pi8080d --storage /var/lib/pi8080d
EnvironmentFile=-/etc/pi8080d/env
StateDirectory=pi8080d
CPUAffinity=3
Restart=always
RestartSec=1

[Install]
WantedBy=multi-user.target
```

  No `After=network-online.target`: the listener binds without a network, and the 8080 is stalled until the daemon runs, so the daemon starts as early as it can. An `ASK` before the network is up gives 83, like `T` before NTP. `StartLimitIntervalSec=0` keeps it retrying every second through a transient early-boot failure (udev not yet done with `/dev/gpiomem` or the gpiochip); without it systemd gives up after 5 starts in 10 s and the 8080 stalls until someone logs in. A configuration refusal (ALT function, wrong SoC) then only repeats in the journal. A crash restarts it in a second; the 8080 waits under READY meanwhile (`DEVICE_SPECS.md` 3.4) and its devices come back fresh (`DEVICE_SPECS.md` 2.9).
- **Reaching the console from the Mac** with the loopback default, through a Unix socket so no TCP port opens on the Mac (a browser there cannot connect to a Unix socket; the console has no authentication, 7.1): `ssh -o StreamLocalBindUnlink=yes -L /tmp/pi8080.sock:localhost:8080 pi`, then `socat -,rawer,escape=0x1d UNIX-CONNECT:/tmp/pi8080.sock`. ssh creates the socket mode 0600 (its default `StreamLocalBindMask`), so other users on the Mac cannot use it either. A TCP forward (`-L 8080:localhost:8080`) also works, but then any page open in a browser on the Mac can type commands.
- **Simulated board:** with the 16.4 drop-in installed, the same unit runs `--sim`. Remove it to go back to the board.

---

## 12. Performance

### 12.1 Targets

This is the home of the service-time estimates. All are estimates until measured (14):

| Quantity | Target |
|----------|--------|
| REQ rising to ACK rising, console, mailbox and storage register accesses | sub-microsecond to about 3 us |
| REQ rising to ACK rising, `IN/OUT 0B` (one `pread`/`pwrite`-sized syscall pair) | a few microseconds more |
| `reset_edge()` syscall | at most one access per millisecond pays it (5.1) |
| fsync, mount | bounded, may be seconds (`DEVICE_SPECS.md` 3.3) |
| `GET` execute: spawning `curl` (fork, `pre_exec`, exec; `pre_exec` rules out `posix_spawn`) | est. 1-5 ms **[bench]** |
| `IN 12` while a `GET` runs: one non-blocking `read` (at most 4096 bytes) or `waitpid` | est. a few us more than a register access **[bench]** |
| Abort of a running `GET` (execute, clear, RESET): `kill` and reap | est. under 1 ms |
| The `IN 12` that ends a `GET > FILE`: fsync of up to 16 MiB, rename, directory fsync | bounded, may be seconds, as fsync |
| Blanking after ACK | at least 500 ns (not in the REQ-to-ACK figure) |
| Console pass | at most one per millisecond, est. 5-20 us |
| CPU | one isolated core at 100% |

The 8080 adds 0.4-0.9 us of 8224 resynchronization per access on top (`ARCHITECTURE.md` 6.4 rule 3) and cannot issue Pi accesses closer than about 5 us apart.

### 12.2 Measuring on day one

Bring-up step 5 (`HARDWARE_BUILD.md` 3), with the 8080 board powered (`ARCHITECTURE.md` 6.4: the Pi never drives toward an unpowered board) and `pi8080d` running without `--trace`. No measurement mode in the daemon: the instruments are a scope and the TCP client.

1. **Per-access service time and GPIO latency:** scope or logic analyzer on BCM 12 (REQ), 16 (ACK) and 17 (LATCH), during boot and at the idle prompt (the `IN 02` poll), 10^4 accesses, with and without `stress-ng` on cores 0-2. REQ rise to ACK rise is the service time; record min, typical and max against 12.1. On an IN, the LATCH pulse width is one GPSET0 write, reads until LATCH reads high, and one GPCLR0 write, which bounds the GPIO write-to-read-back latency. Accesses more than 1 ms apart show the `reset_edge()` cost as the difference.
2. **Whole-command overhead, no instrument:** time a port-heavy command at the TCP client, for example `W F000 0 1000` (4096 `OUT 0B`). Its emulator cycle count and access count come from `w_command_cycles`, an `#[ignore]` test in `tests/monitor_tests.rs` that runs the command through the `Mon` harness and prints `cpu.cycles` and the `Mon.ports` count (`ARCHITECTURE.md` 7.2 allows both). Mean overhead per access = (wall time - cycles x 488.28 ns) / accesses.

---

## 13. Tests

Everything here runs in `cargo test` on any OS. The simulated board stands in for the 8080 board and `GpioMem`; nothing in the daemon knows it is simulated. It is the same `SimBoard` that `--sim` runs (16); the `--sim` binary test is 16.5.

### 13.1 The simulated board (`src/pi/sim.rs`)

`SimBoard` implements `Gpio` and models the `ARCHITECTURE.md` 6.4 circuit at the logic level, shared between the daemon thread and the test thread (`Arc`, a `Mutex` for the state and a `Condvar` for every change the other side waits on):

- **Register file:** GPFSEL0-2, the output latch, REG1 pulls. GPSET0/GPCLR0 change the latch. GPLEV0 is composed: pins in output mode read their latch bit; A0-A6, DIR, D0-D7 (data 245 enabled while DIR is high), REQ and RESET come from the 8080 side; D0-D7 read 00 (pull-downs) while DIR is low and they are inputs. Knobs set the initial register state and can force a D bit stuck at a value or ACK stuck high.
- **8080 side:** `begin(port, In | Out(v))` queues an access. The head of the queue sets A, DIR, the OUT data and the WAIT flip-flop Q. `wait()` returns the oldest completion (the IN latch byte, or "aborted by RESET"), or panics with the board state after `wait_timeout` (10 s by default; `--sim` waits forever, 16.2). Outside an access A and D read pseudo-random values and DIR reads high, so sampling without REQ shows up as garbage. The values come from a fixed-seed xorshift32; every SimBoard panic prints the seed.
- **WAIT flip-flop:** cleared by a rising edge of ACK (detected in `write`) or by RESET. **REQ** = Q, or still high for `req_fall` (default 400 ns) after the ACK edge, showing the old access's A, DIR and D, as the real REQ lags the edge.
- **Gap:** the next queued access does not set Q until `gap` (default 1 us) after the previous ACK edge, as the 8080 can't issue the next access sooner. With `gap` = 0 and `req_fall` = 0 the ACK edge loads the next queued access in the same `write`, so GPLEV0 never shows REQ low between queued accesses.
- **IN latch:** captures the D output latch on a LATCH rising edge.
- **RESET:** `reset(on)` sets the level, clears Q, aborts the access in flight and queues an edge event stamped with the current `Instant`. `release_event_delay` (default 0) delays delivery of the falling-edge event, not its stamp, as the kernel does. `reset_edge()` implements the 3.1 contract (true only for delivered events stamped at or after its previous call) and counts its calls (`edge_calls()`).
- **Test helpers:** `pulse_reset()` (13.3), `stick_d(Some((bit, level)))` and `stick_d(None)`, `peek(off)` (a register without a daemon read's side effects), `writes()`, `edge_calls()` and `wait_edge_calls(n)`, and `check()` (fails with a recorded violation). Knobs are a `Knobs` struct: GPFSEL0-2 and the output latch at power-on (by default ALT functions on BCM 0-3 and 14-15, and every latch bit high, so a write that changes another pin or turns ACK or LATCH into an output before driving it low is caught), `req_fall`, `gap`, `release_event_delay`, `ack_stuck_high`, `wait_timeout` (`Option<Duration>`, default 10 s). They ship in the library with the board (2); `--sim` uses none but `wait_timeout`.
- **Unfair lock:** a GPLEV0 read that shows REQ low yields the thread after releasing the `Mutex`. std's `Mutex` is not fair, and the daemon polls in a tight loop; without the yield the test thread waited tens of microseconds per access for the lock (the 100 KiB input test took 16 s instead of 2).
- **Pause hook:** `pause_next_request()` makes the daemon's next GPLEV0 read that shows a new request (REQ high from Q, after a read has shown ACK low: the read that ends step 8 can already show the next request, and the daemon does not sample from it) compute its value and then park on the `Condvar` (releasing the `Mutex`, so `reset` and `begin` can run) until `resume()`. `wait_paused()` blocks the test thread until the daemon is parked (panics after 10 s). Every pause test calls it before touching RESET, so it covers "the daemon has sampled the request and not yet ACKed it", which stands for a slow device call (an fsync) or a descheduled thread.

**Protocol checks.** On the first violation the board records it, calls `notify_all`, drops its guard and then panics the daemon thread. `wait()`, `wait_paused()` and the test's final check look for a recorded violation first and fail with its message, never with a timeout. Each check is one obligation:

| Violation | Obligation |
|-----------|------------|
| D0-D7 set to output while REQ is low or DIR is OUT | No line driven from both sides (`ARCHITECTURE.md` 6.4) |
| LATCH rising with D0-D7 not outputs, or with no GPLEV0 read showing the driven byte since the last D write | Read back until match before LATCH (`ARCHITECTURE.md` 6.4 IN step 2) |
| LATCH falling without a read since it rose | LATCH read back high (`ARCHITECTURE.md` 6.4 IN step 3) |
| ACK rising with D0-D7 still outputs or LATCH high | Release D before ACK (`ARCHITECTURE.md` 6.4 IN steps 4-5) |
| ACK rising on an IN with no LATCH edge during the access | IN data latched |
| ACK rising with no access pending | No phantom access: blanking honored (`ARCHITECTURE.md` 6.4 rule 2) |
| ACK rising while RESET is high, or for an access aborted by RESET | Never ACK across RESET (`ARCHITECTURE.md` 6.6) |
| ACK falling without a read since it rose, or less than 500 ns after the rise | ACK read back, 500 ns blanking (`ARCHITECTURE.md` 6.4 rule 2) |
| A write to any register but GPFSEL0-2, GPSET0, GPCLR0, REG1, or one that changes any field or bit outside the 20 pins | 3.2 |
| A GPFSEL0/1 write that newly puts ACK or LATCH into output mode while its latch bit is high | 3.3 step 3 |

### 13.2 Every transcript through the daemon (`tests/monitor_tests.rs`)

`every_transcript_through_the_daemon` plays every `tests/transcripts/*.txt` (read from the directory, so new transcripts are covered automatically), each from a fresh power-on, through this path:

```
Intel8080 --IN/OUT 00-6F--> Bridge (IoDevice) --begin/wait--> SimBoard <--Gpio-- pi::serve (daemon thread)
                                                                                    |
            test thread <------------------- TCP 127.0.0.1:ephemeral -------------- console
```

- The CPU boots exactly as `power_on` does (junk RAM and registers, `monitor.bin`), but its IoBus maps a `Bridge` (`pi::sim::Bridge`, which `--sim` also uses) on 00-6F: `read` is `begin(port, In)` then `wait()`, `write` is `begin(port, Out(v))` then `wait()`. FE and FF stay in the CPU model; 70-FD stay unmapped.
- The daemon thread runs `setup_pins` and `pi::serve` on the `SimBoard` with a temporary storage directory, `mailbox::local_time`, a listener on `127.0.0.1:0`, a trace file and its own stop flag. The test connects its client **before** starting the daemon, so the banner is never discarded for want of a client.
- Same files, same parser, same `play`. `Mon.con: Rc<RefCell<Console>>` becomes `Mon.side: Side`, with `enum Side { Local(Rc<RefCell<Console>>), Daemon(TcpStream) }`. The only per-mode difference is where input goes (`push_input` or the socket) and where output is read (`take_output`, or the socket until it has as many bytes as the 8080 sent with `OUT 00`). Both modes use one at-prompt rule, read from `Mon.ports`: the 8080 has done at least as many `IN 01` since the step began as bytes were typed, its `OUT 00` bytes since the step began end with `> `, and it stays quiet for 2,000 cycles. The local harness switches to this rule too, so there are not two.
- After the last step: set the stop flag, join the daemon thread, then assert the daemon's trace file equals `Mon.ports` restricted to 00-6F and collapsed by the `ARCHITECTURE.md` 7.3 repeat rule, line for line, `; xN` counts included. With no RESET in the run the two sequences are the same accesses, so they must match exactly.
- Cost: re-measured 2026-10-04 after Phase 12: the 24 transcripts make 222,600-228,500 Pi-window accesses over three runs (the count varies with the `IN 02` polls made while typed input crosses TCP), each two cross-thread handoffs, in about 2.7 s in a debug `cargo test` (about 12 us an access), inside the 10 s budget. `example_burn.txt` adds about 15,000 (its records are pasted byte by byte) and `breakpoint.txt` about 3,000. If it grows past the budget, run fewer transcripts through the daemon rather than add machinery.

Every test that starts a built binary (`CARGO_BIN_EXE_*`) removes `ANTHROPIC_API_KEY` from its environment, so `cargo test` never reaches the API with a developer's key. `sim_mode_plays_transcripts_over_tcp` checks `ask off` in the startup line. (The spawn sites: `tests/monitor_tests.rs` `sim_mode_plays_transcripts_over_tcp`, `tests/terminal_tests.rs` `spawn`, `tests/debugger_tests.rs` the script runner, `tests/mailbox_tests.rs` `the_worker_gets_an_empty_environment`.)

This proves, for every behavior the transcripts cover: one access per instruction, nothing lost, merged or repeated (`DEVICE_SPECS.md` 3.1), the handshake order, the shared devices, the TCP console both ways, and the trace format.

### 13.3 Fault, startup, stop and console tests (`tests/pi_daemon_tests.rs`)

Driven from the test thread with `begin`/`wait` directly, no CPU. Each test gives `serve` its own stop flag, and sets it and joins the daemon thread before reading the trace file. Starting the daemon returns after its first console pass, so a RESET the test makes next finds the daemon running (a thread slow to start would otherwise find the edge only after later accesses; seen once under `cargo mutants`). "Pulse RESET" is `reset(true)`, sleep 2 ms, `reset(false)`, so a pause plus a pulse always exceeds the 1 ms gate. Nothing waits on wall time for a console pass: "after a pass" means `edge_calls()` has risen by 2 since the access completed (7.3 makes exactly one call per pass, and no access is in flight).

| Test | Script | Asserts |
|------|--------|---------|
| `reset_pulse_mid_in_is_never_acked_and_resets_devices` | Mount `A.BIN`; type `xyz` and wait for `IN 02` = 03; `pause_next_request`; `begin(IN 0C)`; `wait_paused`; pulse RESET; `resume`; begin nothing until the daemon's next `reset_edge()` call (its RESET handling), so only the abort rule's REQ-low read can catch the pulse | The access ends aborted, with no ACK. Then `IN 0C` bit 0 = 0, `IN 0F` = 01, `IN 02` = 02, `IN 01` = 00, `IN 12` = 00. One `RESET` trace line; the paused access has none |
| `reset_pulse_mid_out_is_never_acked_and_resets_devices` | Same, paused on `OUT 0B` | No ACK; storage unmounted; the client still connected |
| `reset_and_reboot_during_a_slow_access` | Mount; pause on `OUT 0B`; `wait_paused`; pulse RESET; `begin(OUT 00 'B')`; `resume` | `OUT 0B` aborted; `OUT 00` completes; the client receives `B`; storage unmounted (edge path, 5.2 table row 4) |
| `reset_and_reboot_during_a_slow_in_never_drives_d` | Same, paused on `IN 0F` | `IN 0F` aborted with D0-D7 never outputs (the board's DIR-OUT check); `OUT 00` completes; the client receives `B`; storage unmounted (abort rule, DIR high) |
| `reset_pulse_during_in_readback_does_not_hang` | A D bit forced stuck; `begin(IN 02)`; after 5 ms pulse RESET; release the stuck bit; `begin(IN 0F)` | The IN ends aborted; `IN 0F` = 01 |
| `reset_held_blocks_service_until_release` | Pause on `OUT 00`; `wait_paused`; RESET on; `resume`; wait 20 ms; RESET off; `begin(IN 0F)` | No ACK while held; after release `IN 0F` = 01 |
| `reset_while_idle_resets_devices` | Type `ab`, wait for `IN 02` = 03, type 10 KiB (left in the socket: the FIFO holds input; more than one 4 KiB read), pulse RESET with no access | `IN 02` = 02 afterwards, and still after a pass (all the socket's bytes were discarded, not only the first read's) |
| `reset_discards_output_not_yet_sent` | `gap` = 0, `req_fall` = 0, no trace file; a client that reads nothing; queue 1 MiB of `OUT 00` (letters in a cycle) and let the daemon drain them with no pass between; wait 200 ms (passes fill the socket); pulse RESET; after a pass, `OUT 00 '#'`; read the client up to the `#` | Fewer than 1 MiB bytes before the `#`, and they are the stream's first bytes in order: the pending chunk was dropped at RESET, not sent after it (5.2 step 6) |
| `a_late_release_event_does_not_reset_twice` | `release_event_delay` = 5 ms; pulse RESET; `OUT 00` `a` at once; wait 10 ms; `OUT 00` `b` | The client receives `ab`; exactly one `RESET` trace line |
| `a_request_pending_at_start_is_served` | `begin(OUT 00 'Q')`, then start the daemon | The access completes; the client receives `Q` |
| `back_to_back_requests_never_wait_for_req_low` | `gap` = 0, `req_fall` = 0, no trace file; queue 1,000 accesses at once and let the daemon drain them before the first `wait()` (poll the completion count, so no test thread is woken on each ACK edge) | All complete, and the board's 500 ns ACK-high check holds (a daemon that waits for REQ low hangs and the 10 s timeout fails it) |
| `startup_releases_d_and_drives_ack_and_latch_low` | Board starts with D0-D7 as outputs and ACK and LATCH as outputs latched high | When `setup_pins` returns, D0-D7 are inputs and ACK and LATCH read low; the 13.1 checks pass through a first access |
| `a_pin_in_an_alt_function_refuses_to_start` | Pin 7 (GPFSEL0), 12 (GPFSEL1) and 27 (GPFSEL2), each in turn with every field code 010-111 (ALT0-ALT5) | `setup_pins` returns an error naming the pin; no register written |
| `a_second_daemon_is_refused_by_the_gpio_lock` | `open_gpio` on a temporary file `F`; `open_gpio` on `F` again; drop the first; `open_gpio` on `F` again | The second fails with `F: another pi8080d holds the GPIO`; after the first is closed the third succeeds |
| `ack_stuck_high_refuses_to_start` | ACK forced high | `setup_pins` returns the `BCM 16 (ACK) reads high` error |
| `stop_while_reset_is_held_returns_and_flushes` | Mount, `OUT 0B` a byte; after a pass, RESET on; wait for RESET handling's two register writes (5.2 step 1: the daemon is past step 1's stop check); set stop | `serve` returns `Ok`; the byte is in the file; no ACK raised; the trace ends `RESET` |
| `stop_with_a_stuck_d_bit_returns_and_flushes` | Mount, write a byte; a D bit stuck; `begin(IN 02)`; set stop | `serve` returns `Ok`; the byte is in the file; D0-D7 inputs, ACK and LATCH low |
| `a_new_client_replaces_the_old` | Client 1; `OUT 00 a`; client 1 reads `a`; client 2 connects; `OUT 00 b` | Client 1 got `a`, then EOF; client 2 got `b` only |
| `output_with_no_client_is_discarded` | `OUT 00 x`; after a pass; connect; `OUT 00 y` | The client gets `y` only |
| `half_closed_client_still_receives_output` | Client sends `xyz`, then `shutdown(Write)`; pop 3 bytes with `IN 01`; `OUT 00 a` | The client receives `a`; `IN 01` gave `xyz` |
| `input_arrives_in_order_and_unchanged` | A writer thread sends 100 KiB covering every byte value 00-FF; the test thread pops it with `IN 02`/`IN 01`; join the writer | Same bytes, same order, none lost (flow control through a 4 KiB-at-a-time FIFO) |

A RESET during an fsync is the same as the pause-hook tests: the daemon has sampled the request and not ACKed it when RESET comes and goes.

### 13.4 What the simulator cannot show

Nanosecond timing, the 74HCT374 margins, the order in which posted GPIO writes reach the pins, kernel event latency and the real `GpioMem` mapping and ioctls. Those are the section 14 items.

---

## 14. Bench-Only Checks

The one list. Each closes on the built board, at the bring-up step given (`HARDWARE_BUILD.md` 3).

| Check | Step |
|-------|------|
| `/dev/gpiomem` maps the GPIO block at offset 0; pull-down writes to REG1 take effect | 5 |
| REQ-to-ACK service time and LATCH width against 12.1 (12.2 method 1), with and without `stress-ng` | 5 |
| The `pinctrl-bcm2711` gpiochip is found and the v2 line request succeeds; RESET edges arrive with `CLOCK_MONOTONIC` stamps; a late release event never causes a second reset (trace shows one `RESET` per press over 100 presses) | 5 |
| Boot port trace and banner byte-identical to the emulator's | 5 |
| Either power-on order; daemon started after the 8080: the first access completes | 5 |
| A second `pi8080d` started beside the unit exits 1 with `/dev/gpiomem: another pi8080d holds the GPIO`, and the unit's console keeps working | 5 |
| The musl static binary cross-built on the Mac runs on the Pi; local time is correct under it (musl reads `/etc/localtime`) | 5 |
| Every transcript over TCP; HEX paste loses nothing; whole-command overhead (12.2 method 2) | 6 |
| `T` right after boot with the network down prints `Service error`; after timesyncd syncs it prints the local time | 6 |
| Storage conformance under `stress-ng`; RESET mid-fsync reboots to the banner, storage unmounted, no hang | 7 |
| `/usr/bin/curl --version` is 8.4.0 or later; `N https://example.com` prints the page; `N https://... > F` and `X F`, `L` load it; with the network down, `N` prints `Service error` within about 10 s | 6 |
| The service time of a `GET` execute and of a BUSY `IN 12` against 12.1 (12.2 method 1) | 6 |
| During a long `GET > FILE` the worker runs on cores 0-2 (`ps -o psr= -C curl`) and the 12.2 method 1 service times do not move | 6 |
| `systemctl restart pi8080d` during a `GET`, and `kill -9` of a hand-run `--sim` during a `GET`, leave no `curl` behind (`pgrep curl`) | 6 |
| `Q` prints an answer with the key installed; `Service error` within about 10 s with the network down; with the env file removed the startup line says `ask off` and `Q hi` prints `Service error`; during a `Q`, `ps -ef` shows no key and `journalctl -u pi8080d` never does | 6 |
| Tick jitter on the isolated core; `nohz_full` support; no thermal throttling over an 8080EXM run (about 3.2 h) | 5, 8 |

---

## 15. Not in v1

- TEST_RESET (BCM 18) driven by the daemon: for the unattended test rig only, designed with it (decision TEST-RESET). The board side is a 2N3904 on /RESIN (`ARCHITECTURE.md` 6.6). The rig MUST hold each pulse for at least the DS1813 tPB, at least 1 ms, or the 1 ms gate of 5.1 loses its premise.
- A UART or USB-gadget console: bridge to TCP with `socat` (`HARDWARE_BUILD.md` 5).
- Pi 5 (RP1 registers), Pi 3.
- Hardware single-step (decision HW-STEP).
- A measurement mode (`--measure`): the scope and the TCP client give the step 5 numbers (12.2). Add a tool only if the bench shows one is needed.
- RESET under `--sim` (16.3): restarting the daemon is the power cycle.
- A throttled or cycle-timed `--sim` 8080. The ROM is timing-independent (`ARCHITECTURE.md` 3.2 requirement 6).
- A URL allow-list or deny-list for `GET` (`DEVICE_SPECS.md` 8, GET, Reach).
- Statistics, a status port, a web page. The trace and the logs are the instruments.

---

## 16. Simulated Board (`--sim`)

Decided 2026-10-03 (Mike, COLLABORATION_LOG Key Decisions). `pi8080d --sim FILE` runs the whole Pi software stack with the emulator's CPU model on the simulated board of 13.1 in place of the 8080 board: the daemon, the TCP console, the unit, the storage directory, the TIME clock and the trace, on a real Pi (or any Linux or macOS box) before the board exists. It also runs the RAM test build workflow (`ARCHITECTURE.md` 2.1).

The rule: **only the `Gpio` implementation differs.** `serve` gets a `SimBoard` in place of a `GpioMem`. Nothing in `src/pi/mod.rs` knows the difference.

### 16.1 What is simulated, what is real

| Part | Under `--sim` |
|---|---|
| `pi::setup_pins`, `pi::serve` (bus loop, abort rule, console pass, stop) | Real: the same code, on the main thread |
| Devices (`build_bus`), storage directory (`--storage`) | Real |
| TCP console (`--listen`, 7) | Real |
| TIME clock (8) | Real: `ntp_local_time` on Linux, `mailbox::local_time` elsewhere (no `adjtimex`) |
| Port trace (`--trace`, 9) | Real: the same lines a board gives for the same accesses |
| Signals, stop, Storage flush (4.1, 10) | Real |
| systemd unit (11) | Real, with the 16.4 drop-in |
| GPIO register block, RESET line | Simulated: `SimBoard` (13.1), every protocol check armed |
| 8080, ROM, RAM, overlay, ports FE/FF and 70-FD | Simulated: `Intel8080` with the ROM image FILE, on its own thread (16.2) |
| Timing | Not modeled. Between accesses the 8080 runs at host speed (`ARCHITECTURE.md` 7.2, no throttle); each Pi-window access is a cross-thread handshake (cost: 13.2) |
| `GpioMem`, `/dev/gpiomem`, the gpiochip, the BCM2711 check | Not used: `--sim` never opens them, so it runs on any SoC |

### 16.2 The 8080 thread

`pi_main` starts it after the listener and the trace are open, just before `serve`:

1. `Intel8080::new()` and `load_rom`, as `main.rs` does: RAM and registers 00, PC 0000, overlay set (`ARCHITECTURE.md` 3.1). Unlike 13.2, no junk fill: a real board's RAM is random, which the transcript harness covers.
2. Its IoBus maps `pi::sim::Bridge` on 00-6F: `read` is `begin(port, In)` then `wait()`, `write` is `begin(port, Out(v))` then `wait()`. FE and FF stay in the CPU model; 70-FD stay unmapped. This is the 13.2 wiring with the CPU on its own thread.
3. It runs `execute_one` until a halt, then logs `pi8080d: 8080 halted at PC=xxxx` (the PC after the HLT) and parks for good. v1 has no interrupt source (`ARCHITECTURE.md` 5.8) and `--sim` has no RESET (16.3), so a halted 8080 stays halted until the daemon restarts. The console stays up.

- The board is `SimBoard::new(Knobs { wait_timeout: None, ..Knobs::default() })`. READY has no timeout (`ARCHITECTURE.md` 6.4 rule 4): a slow fsync stalls the 8080, never panics it. Every other knob keeps its default.
- The 8080 thread owns the `Intel8080`; the main thread owns the IoBus and the devices. They share only the `SimBoard` (`Arc`), as in 13.2. Devices still never cross threads (1).
- A `SimBoard` protocol violation (13.1) panics the daemon thread, which under `--sim` is the main thread: the process exits 101 and systemd restarts it. A violation is a daemon bug, and it is meant to be loud.
- On stop (4.1) `serve` flushes Storage and returns as on the board, and the process exits 0. The 8080 thread, running or blocked in `wait()`, ends with it.

### 16.3 RESET and power

- **No RESET under `--sim`.** Nothing drives the simulated RESET line, so the 5.2 code runs only in the tests (13.3). Restarting the daemon (`systemctl restart pi8080d`, or Ctrl-C and run it again) is the power cycle: fresh devices, a fresh 8080 at PC 0000 with the overlay set and RAM 00. A program that hangs (`JMP $`) or halts ends that way.
- The 8080 boots when the daemon starts, so the banner is usually discarded because no client is connected yet (`DEVICE_SPECS.md` 4), as on the board. Press Enter for a prompt.

### 16.4 ROM image, running it, the unit

- `--sim FILE`: FILE is a ROM image of exactly 4096 bytes, run at F000 (`ARCHITECTURE.md` 2: `rom/monitor.bin`), read once at startup. Any other size, or a read error, is an `Err` before anything else starts: `--sim FILE: N bytes, not 4096`, or the OS error (exit 1).
- The other flags (10) mean the same as on the board. `--sim` runs on Linux and macOS; without it the daemon is Linux only (2).
- On the Mac, from the repo: `cargo run --bin pi8080d -- --sim rom/monitor.bin --storage /tmp/pi8080d`, then `socat -,rawer,escape=0x1d TCP:localhost:8080`. While it runs, a web page in a browser on the Mac can reach that port too (7.1, no authentication). Use a storage directory of its own: the emulator's `storage/` works too, but not while the emulator runs.
- On a Pi or any Linux box:
  1. Install the binary and the unit as in 11. On a box with no `gpio` group, `groupadd --system gpio` first: the unit names it, and systemd refuses to start a unit whose group does not exist.
  2. Copy the ROM image to the Pi as the binary (2) and install it: `scp rom/monitor.bin pi:/tmp/`, then `ssh pi sudo install -D -m 0644 /tmp/monitor.bin /usr/local/share/pi8080d/monitor.bin`.
  3. Install `scripts/pi8080d-sim.conf` as `/etc/systemd/system/pi8080d.service.d/sim.conf`.
  4. `systemctl daemon-reload`, then `systemctl restart pi8080d`.

  ```ini
  # pi8080d --sim (docs/PI_DAEMON.md 16.4): the simulated board instead of GPIO.
  # Install as /etc/systemd/system/pi8080d.service.d/sim.conf; delete it to go back to the board.
  [Service]
  ExecStart=
  ExecStart=/usr/local/bin/pi8080d --storage /var/lib/pi8080d --sim /usr/local/share/pi8080d/monitor.bin
  CPUAffinity=
  ```

  `CPUAffinity=` clears the pin to core 3, so the two busy threads do not share one core. `User`, `Group`, `StateDirectory`, `Restart` and `StartLimitIntervalSec` stay as they are. The `/boot/firmware` settings in 11 are not needed. To go back to the board, delete the drop-in and reload.

### 16.5 Tests

- `every_transcript_through_the_daemon` (13.2) is the `--sim` wiring with the CPU on the test thread, and uses the same `SimBoard` and `Bridge`.
- `sim_mode_plays_transcripts_over_tcp` (`tests/monitor_tests.rs`, Unix) runs the built binary (`CARGO_BIN_EXE_pi8080d`) four times, for the transcripts `hex_math`, `storage` and `assemble` and one RAM-image run, with `--sim rom/monitor.bin --listen 127.0.0.1:0 --storage TMP --trace TMP/trace.txt`. For each:
  1. read the bound address from the startup line on stderr, check its `ask off, board simulated` suffix, and connect;
  2. type `H 0 0` and skip the output up to its answer and prompt (the client may connect mid-banner or after the banner was discarded, 16.3);
  3. play the transcript: type each step, read exactly the expected bytes plus the `> ` prompt, and compare as the harness does. The RAM-image run instead pastes `rom/monitor_ram.hex` a line at a time, types `G D000` and `F CFFF D000 00`, and expects what the same steps print on the local path (the ` RAM` banner, `Address out of range`; `ARCHITECTURE.md` 2.1);
  4. send SIGTERM, then assert exit status 0, a trace whose first line is `OUT 00 0D` (the banner's first byte; FE is outside the window) and an `IN 01` line in it.

  If the console closes, a read times out or the exit status is not 0, the test kills the daemon and fails with everything it wrote to stderr (a board violation panics the daemon).

  Every test that starts a built binary (`CARGO_BIN_EXE_*`) removes `ANTHROPIC_API_KEY` from its environment, so `cargo test` never reaches the API with a developer's key. `sim_mode_plays_transcripts_over_tcp` checks `ask off` in the startup line.

  This covers the flags, the RAM-build load workflow, the ROM image, the startup order, the 8080 thread, the TCP console, storage, the mailbox, the trace and the stop path.
- Nothing in 14 closes under `--sim`. It shows that the deployment works, not that the board does.
