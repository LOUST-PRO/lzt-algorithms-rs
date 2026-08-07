# Embedded: ESP32-D0WD-V3 hello world (bare-metal Rust)

Verified working recipe for cross-compiling and flashing `lzt-embedded-trees`
to an **ESP32-D0WD-V3** development board using `esp-hal` 1.1.2 (no ESP-IDF SDK
required). Last verified 2026-08-06 on a board with MAC address in the
`38:18:2b:8a:49:xx` block.

```text
ESP32-D0WD-V3 specs
  CPU:        Tensilica Xtensa LX6 dual-core @ 240 MHz + LP core
  SRAM:       520 KiB total (320 KiB usable for app after bootloader)
  Flash:      4 MiB typical (default partition: factory @ 0x10000, 3.94 MiB)
  WiFi:       802.11 b/g/n
  BT:         v4.2 BR/EDR + BLE
  Price:      ~$3 USD (~$60 MXN)
```

## What you get

84 KiB binary flashed, 5 features → 3 classes → <1 ms latency on the cascade.

## 1. Install the toolchain (one-time, ~5 min)

```bash
cargo install espup --locked          # downloads espup v0.17+
espup install                         # downloads ~2 GB of pre-built Xtensa
                                      # rust 1.95.0.0 + clang 20.1.1 + GCC 15.2.0
source "$HOME/export-esp.sh"          # exposes cargo +esp, sets PATH
```

Then flash tools:

```bash
cargo install cargo-espflash --locked
cargo install espflash --locked       # standalone binary
```

And serial-read fallback (because `espflash monitor` has Linux issues):
pipx install esptool  OR  `python3 -m venv /tmp/esptool-venv && /tmp/esptool-venv/bin/pip install esptool`

## 2. Project layout

```text
my-esp32-app/
├── Cargo.toml
├── build.rs                 # the critical line is below
├── .cargo/
│   └── config.toml
└── src/
    └── main.rs
```

### `Cargo.toml`

```toml
[package]
name = "my-esp32-app"
version = "0.1.0"
edition = "2021"
authors = ["..."]
publish = false

[dependencies]
esp-hal    = { version = "1.1", features = ["esp32", "unstable"] }
esp32      = { version = "0.40", features = ["rt"] }   # `rt` enables device.x
esp-println = { version = "0.13", default-features = false, features = ["esp32", "uart"] }
# + your no_std crate (e.g. lzt-embedded-trees = "0.1")

[profile.release]
opt-level = "s"
lto = true
codegen-units = 1
strip = true
```

### `build.rs` — THE critical piece (one line)

```rust
fn main() {
    println!("cargo:rustc-link-arg=-Tlinkall.x");
    println!("cargo:rerun-if-changed=build.rs");
}
```

`-Tlinkall.x` is esp-hal's combined linker script. It sets
`ENTRY(Reset)` + `PROVIDE(DefaultHandler = EspDefaultHandler)` + defines
the MEMORY regions for `.vectors` (0x40080000) and `.rwtext` (0x40080400).

**Do NOT use `-Tdevice.x` alone**: linker succeeds but entry point is 0x0.
**Do NOT use `-T link.x` alone**: undefined `DefaultHandler` reference.

### `.cargo/config.toml`

```toml
[build]
target = "xtensa-esp32-none-elf"

[target.xtensa-esp32-none-elf]
runner = "espflash flash --monitor /dev/ttyUSB0 --ignore-app-descriptor --baud 115200"
```

### `src/main.rs`

```rust
#![no_std]
#![no_main]

use esp_hal::clock::CpuClock;
use esp_hal::main;
use esp_hal::uart::{Config as UartConfig, Uart};
use core::fmt::Write;

#[main]
fn main() -> ! {
    let peripherals = esp_hal::init(esp_hal::Config::default()
        .with_cpu_clock(CpuClock::max()));
    let mut uart = Uart::new(peripherals.UART0, UartConfig::default())
        .expect("UART0 init failed")
        .with_tx(peripherals.GPIO1)
        .with_rx(peripherals.GPIO3);
    writeln!(uart, "\nhello from ESP32-D0WD-V3").ok();
    loop {}
}

#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! { loop {} }
```

## 3. Build, flash, verify

```bash
. /home/lou/export-esp.sh                   # PATH + LIBCLANG for espup
cargo +esp build -Z build-std=core --release
espflash flash --chip esp32 --port /dev/ttyUSB0 \
    --ignore-app-descriptor --baud 115200 \
    target/xtensa-esp32-none-elf/release/my-esp32-app
```

Expected output:

```text
Serial port: '/dev/ttyUSB0'
Connecting...
Using flash stub
Chip type:         esp32 (revision v3.1)
Flash size:        4MB
App/part. size:    84,192/4,128,768 bytes, 2.04%
Flashing has completed!

ets Jul 29 2019 12:21:46
...
I (137) boot: Loaded app from partition at offset 0x10000

hello from ESP32-D0WD-V3
```

## 4. Read serial output

`espflash monitor` may return `Failed to initialize input reader` on Linux.
Use this Python snippet instead:

```python
import serial, time
s = serial.Serial('/dev/ttyUSB0', 115200, timeout=3)
s.setDTR(False); s.setRTS(False); time.sleep(0.1)
s.setDTR(False); s.setRTS(True); time.sleep(0.5)
s.setDTR(False); s.setRTS(False); time.sleep(1.0)
buf = bytearray(); end = time.time() + 5
while time.time() < end:
    d = s.read(64)
    if d: buf.extend(d)
print(buf.decode('utf-8', errors='replace'))
s.close()
```

## Why this works (the non-obvious bits)

| Decision | Reason |
|---|---|
| `xtensa-esp32-none-elf` (NOT `-espidf`) | The `-espidf` target's crt0 calls ROM functions that abort without the full ESP-IDF SDK linked. Bare-metal needs `none-elf` + `xtensa-lx-rt`. |
| `xtensa-lx-rt 0.22` (NOT `0.15`) | `0.15` uses `asm!` in naked functions — `E0787` under Rust 1.95. `0.22` uses modern `naked_asm!`. |
| `esp-hal` `unstable` feature | Required for `esp_hal::delay::Delay` and other APIs under active development. |
| `esp32` PAC with `rt` feature | The `rt` feature makes the PAC's `build.rs` emit `device.x` (PROVIDE all Xtensa exception/interrupt symbols as DefaultHandler). Without it, missing-handler linker errors flood the build. |
| `-Tlinkall.x` (not `-Tdevice.x`) | esp-hal's `linkall.x` chains `memory.x + alias.x + esp32.x + hal-defaults.x` — needed for entry-point `Reset`, `DefaultHandler`, MEMORY regions. |
| Avoid `{:.2}` for f32/f64 | Core's f32 Display pulls Big32x40 → memcpy/memset. Use fixed-point integer formatting. |
| `loop { continue; }` (NOT `asm!("waiti 0")`) | Xtensa asm! isn't stable on this target. |

## What fits in 4 MB flash

| Component | Approx. size |
|---|---|
| ESP32 second-stage bootloader | ~32 KiB (pre-flashed, don't touch) |
| Partition table | 4 KiB (pre-flashed) |
| `lzt-esp32-harness` (this recipe) | 84 KiB |
| `lzt-cart` fitted tree (8 depths, 256 leaves) | ~6 KiB text + 256 × 8 B = 2 KiB data |
| `lzt-entropy` (AGPL math) | ~8 KiB text (when cross-compiled) |
| `lzt-embedded-trees` cascade | <1 KiB text |
| WiFi STA stack (if enabled) | ~120 KiB |
| mbedTLS | ~80 KiB |
| **Total headroom after all of the above** | ~3.6 MB free |

See [`../benchmarks/sizes.md`](../benchmarks/sizes.md) for more.

## Next steps

- Profile + OTA strategy: [`profile-system.md`](profile-system.md)
- Known issues and workarounds: [`../anti-patterns.md`](../anti-patterns.md)
