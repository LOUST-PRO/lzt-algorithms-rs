# Embedded: profile system + OTA partition strategy

Two patterns for shipping `lzt-embedded-trees` (or any algorithm crate) to
production ESP32 boards with zero downtime.

## Profiles in `Cargo.toml`

```toml
[profile.dev]
opt-level = 0
debug = true
lto = false
strip = false
# ~120 KiB text, full debug info, slow

[profile.release]
opt-level = "s"
debug = false
lto = true
codegen-units = 1
strip = true
# ~25 KiB text, 0 debug, fast — production

# Custom: debug binaries with serial-only logger
[profile.release-debug]
inherits = "release"
debug = true
strip = false
# ~32 KiB text, debug info preserved, still fast — pre-prod QA
```

Compile flags per profile:

```bash
cargo +esp build -Z build-std=core --release          # production
cargo +esp build -Z build-std=core --profile release-debug   # QA gate
cargo +esp build -Z build-std=core                    # dev
```

Switch via `cargo make` aliases:

```toml
# .cargo/config.toml
[alias]
esp-prod = "+esp build -Z build-std=core --release"
esp-qa   = "+esp build -Z build-std=core --profile release-debug"
esp-dev  = "+esp build -Z build-std=core"
esp-flash-prod = "espflash flash --monitor /dev/ttyUSB0 --ignore-app-descriptor --baud 115200 --chip esp32"
esp-flash-qa   = "espflash flash --monitor /dev/ttyUSB0 --ignore-app-descriptor --baud 115200 --chip esp32 --before default-reset --after hard-reset"
```

Usage:

```bash
cargo esp-qa           # builds ~32 KiB with debug symbols
cargo esp-flash-qa target/xtensa-esp32-none-elf/release-debug/my-app
```

## OTA partition strategy

Default ESP32 partition table (4 MiB flash):

```text
0x00009000  nvs           24 KiB
0x0000f000  phy_init       4 KiB
0x00010000  factory    4,064 KiB   ← all your app, single slot
```

**Recommended OTA layout** for production (overrides default):

```csv
# Name,    Type,  SubType, Offset,   Size,    Flags
nvs,       data,  nvs,     0x9000,   0x6000,
otadata,   data,  ota,     0xf000,   0x2000,
phy_init,  data,  phy,     0x11000,  0x1000,
factory,   app,   factory, 0x20000,  0x180000,
ota_0,     app,   ota_0,   0x200000, 0x180000,
ota_1,     app,   ota_1,   0x380000, 0x180000,
```

Each OTA slot is 1.5 MiB. New firmware is downloaded to `ota_1` while the
device runs from `ota_0`. On boot, the ESP32 ROM bootloader checks `otadata`
and jumps to the most recent valid slot.

```bash
# Create custom partition CSV
cat > partitions-ota.csv <<'EOF'
# Name,   Type, SubType, Offset,   Size,    Flags
nvs,      data, nvs,     0x9000,   0x6000,
otadata,  data, ota,     0xf000,   0x2000,
phy_init, data, phy,     0x11000,  0x1000,
factory,  app,  factory, 0x20000,  0x180000,
ota_0,    app,  ota_0,   0x200000, 0x180000,
ota_1,    app,  ota_1,   0x380000, 0x180000,
EOF

# Flash with OTA partition table:
espflash flash --partition-table partitions-ota.csv \
    target/xtensa-esp32-none-elf/release/my-app
```

Add to `.cargo/config.toml`:

```toml
[target.xtensa-esp32-none-elf]
linker = "ld"
rustflags = ["-C", "link-arg=-Tlinkall.x"]
runner = "espflash flash --partition-table partitions-ota.csv --monitor /dev/ttyUSB0 ..."
```

## Safe rollback pattern

```rust
use esp_hal::delay::Delay;
use esp_hal::uart::Uart;

#[main]
fn main() -> ! {
    // ... init ...
    let mut uart = /* ... */;

    // On boot, verify the new firmware is healthy by ping-checking a sensor
    // or running a self-test. If unhealthy, mark the current slot as bad.
    writeln!(uart, "boot: app version {}", env!("CARGO_PKG_VERSION")).ok();

    if !self_test_passes() {
        // Mark current app as invalid; bootloader will rollback to previous.
        esp_hal::rom::efuse::mark_app_invalid();
        // Reset to trigger rollback
        esp_hal::reset::software_reset();
    }
    // ... normal operation ...
}
```

A safer pattern (used in production at scale):

```rust
const MAX_BOOT_FAILURES: u8 = 3;
static mut BOOT_FAILURE_COUNT: u8 = 0;

#[main]
fn main() -> ! {
    // Increment failure counter; rollback if exceeded
    unsafe { BOOT_FAILURE_COUNT = esp_hal::rom::efuse::read_app_failure_count(); }

    if !self_test_passes() {
        unsafe { BOOT_FAILURE_COUNT += 1; }
        esp_hal::rom::efuse::write_app_failure_count(BOOT_FAILURE_COUNT);
        if BOOT_FAILURE_COUNT >= MAX_BOOT_FAILURES {
            esp_hal::rom::efuse::mark_app_invalid();
            esp_hal::reset::software_reset();
        }
    }
}
```

## Local-only testing without flashing

Use `qemu-xtensa` for emulation:

```bash
cargo install espup --locked
espup install
source "$HOME/export-esp.sh"

# qemu-xtensa is NOT yet mature for esp-hal 1.1.2 — verify on hardware
# as the primary path. Use qemu only for opcode-level smoke checks.
```

For now (2026-08), **test on real hardware** — the qemu story for esp-hal
isn't stable yet.

## Per-board profile tracking

In `src/main.rs`:

```rust
fn main() -> ! {
    // Print full board identity for audit trail (kept in serial logs):
    writeln!(uart, "esp32 mac={:02x}:{:02x}:{:02x}:{:02x}:{:02x}:{:02x}",
        peripherals.MAC.mac0, peripherals.MAC.mac1, peripherals.MAC.mac2,
        peripherals.MAC.mac3, peripherals.MAC.mac4, peripherals.MAC.mac5).ok();
    writeln!(uart, "lzt-algorithms-rs SHA={}", env!("GIT_HASH")).ok();
    writeln!(uart, "build profile={}", build_profile()).ok();
    // ...
}

#[cfg(debug_assertions)]
fn build_profile() -> &'static str { "dev" }
#[cfg(not(debug_assertions))]
fn build_profile() -> &'static str { "release" }
```

Pin the build artifact hash so post-mortem analysis can tie behavior back to
a specific commit:

```bash
GIT_HASH=$(git rev-parse --short HEAD)
RUSTFLAGS="-C link-arg=-Def_macro=GIT_HASH=\\\"$GIT_HASH\\\"" \
  cargo +esp build -Z build-std=core --release
```

## CI integration (when CI is enabled)

`.github/workflows/embedded.yml`:

```yaml
name: cross-compile-esp32
on: [push, pull_request]

jobs:
  esp32:
    runs-on: [self-hosted, linux, esp32]   # custom runner w/ espup
    steps:
      - uses: actions/checkout@v4
      - uses: actions-rs/toolchain@v1
        with: { toolchain: esp, components: rust-src }
      - run: |
          . "$HOME/export-esp.sh"
          cargo +esp build -p lzt-embedded-trees \
              -Z build-std=core \
              --target xtensa-esp32-none-elf \
              --release
          cargo +esp build -p lzt-cart \
              --release
          cargo test --workspace
      - uses: actions/upload-artifact@v4
        with:
          name: esp32-binary
          path: target/xtensa-esp32-none-elf/release/*.elf
```

(For now, leave this gated with `if: false` because GitHub-hosted runners don't
have espup installed — uncomment when you bake a custom runner image.)
