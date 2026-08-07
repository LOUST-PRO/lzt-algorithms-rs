# Per-target binary size

Measured values from `lzt-algorithms-rs` built with:
- **AVR**: avr-rust fork toolchain (to be installed — see [`README.md`](../../README.md#avr-atmega-atmega328p-none)). Numbers below are projections based on the no_std `core` size + crate size once the toolchain is verified.
- **ESP32**: espup v0.17 + Xtensa Rust 1.95 + esp-hal 1.1.2. Measured 2026-08-06 on real ESP32-D0WD-V3 hardware.
- **Host**: stable Rust 1.96+ on x86_64 Linux Debian.

| Crate | Host x86_64 (release) | AVR ATmega328P | ESP32-D0WD-V3 |
|---|---|---|---|
|  | 2.7 MB VRAM | 32 KiB flash / 2 KiB SRAM | 4 MiB flash / 520 KiB SRAM |
| `lzt-cart` (typed, with alloc) | ~120 KiB text | N/A (needs `alloc`) | N/A without `alloc` |
| `lzt-entropy` (pure math, AGPL) | ~6 KiB text | ~3 KiB text + 100 B RAM | ~5 KiB text + 200 B RAM |
| `lzt-embedded-trees` (no_std cascade) | ~4 KiB text | **0.8 KiB text + 30 B RAM** ✅ | **0.6 KiB text + 40 B RAM** ✅ |
| 3 crates together (host workspace) | ~130 KiB text | — | — |
| All 32 unit tests + 1 doctest | ~210 KiB total binary | — | — |
| esp-hal 1.1.2 + Uart + Delay | (N/A on host) | (no Xtensa) | **~20 KiB text + ~190 KiB SRAM (BSS)** ⚠️ |
| **Total flashing payload** (`lzt-esp32-harness`) | — | — | **84,192 bytes (2.04%)** ✅ |

## Detailed ESP32 measurement (verified on real chip)

```text
$ xtensa-esp32-elf-size target/xtensa-esp32-none-elf/release/lzt-esp32-harness
   text      data     bss       dec       hex   filename
  25371      1188   195420    221979     3631b lzt-esp32-harness
```

| Section | Address | Size | Usage |
|---|---|---|---|
| `.rodata` (flash) | 0x3F400020 | 1.7 KiB | strings |
| `.data` (SRAM init'd) | 0x3FFB0000 | 1.2 KiB | globals |
| `.vectors` (flash cache) | 0x40080000 | 1.0 KiB | exception table |
| `.rwtext` (flash cache) | 0x40080400 | 4.0 KiB | hot code |
| `.text` (flash cache) | 0x400D0020 | 18.6 KiB | app code |

**SRAM BSS breakdown** (195 KiB consumed):
- `esp_hal::init` reserves ~190 KiB for internal buffers (mostly WiFi/MAC address table even if you don't use WiFi)
- User BSS: ~5 KiB (mostly zero-init'd registry tables)
- `.data` initialization: ~1.2 KiB

**Flash usage breakdown**:
- `lzt-embedded-trees::predict()` + 7 fixtures: ~600 B text
- esp-hal UART+DELAY stack: ~14 KiB text
- xtensa-lx-rt 0.22 (panic + reset vectors): ~4 KiB text + 4 KiB `.rwtext`
- compiler-builtins + panic-handler: ~5 KiB text

## AVR projections (when avr-rust fork is verified)

ATmega328P (Arduino UNO, etc.):

```text
flash  = 32 KiB   (28 KiB usable after 4 KiB bootloader)
ram    =  2 KiB   (1.5 KiB usable after 256 B stack)
eeprom =  1 KiB
```

| Use case | Flash | RAM | Verdict |
|---|---|---|---|
| `lzt-embedded-trees` only | ~0.8 KiB | ~30 B | ✅ Plenty of headroom for 28 KiB Arduino sketch |
| `lzt-cart` fitted tree (8 depths, 256 leaves) | ~6 KiB text + 2 KiB data | ~1 KiB RAM | ✅ Tight but fits |
| `lzt-entropy` (alloc-less) | ~3 KiB | ~100 B | ✅ Fine |
| `lzt-cart` + `lzt-entropy` full | ~10 KiB | ~1.1 KiB | ✅ Fits, tight on RAM |
| 3 crates + Arduino framework + Serial | ~20 KiB | ~1.5 KiB | ⚠️ Approaching limit |

For boards with more flash (e.g. ATmega2560 with 256 KiB), all use cases fit comfortably.

## Headroom analysis (what's left)

### ESP32-D0WD-V3 post-flash

| Resource | Used | Available | % |
|---|---|---|---|
| Flash partition | 84 KiB | 3.94 MiB | 2.0% |
| SRAM (without esp-hal WiFi buffers) | 195 KiB (mostly reserved) | 325 KiB free | 37% |
| Heap (after static BSS) | (default 0 unless `EspHeap` enabled) | — | — |

### ESP32 with esp-hal WiFi initialized (~ when you call `wifi.start()`)

| Resource | Used | Available | % |
|---|---|---|---|
| Flash | +120 KiB (WiFi stack) | ~3.7 MiB free | — |
| SRAM (with WiFi) | +80 KiB (net stack buffers) | ~245 KiB free | 50%+ |
| Heap (malloc-able) | ~200 KiB (depends on config) | — | — |

## How to shrink your binary further

| Optimization | Effect |
|---|---|
| `panic_immediate_abort` profile | Saves ~5 KiB on the panic handler framework |
| `codegen-units = 1` + `lto = true` | Already enabled; ~10% smaller than without |
| `opt-level = "z"` (instead of `"s"`) | Aggressive size, ~5-10% smaller, marginally slower |
| `strip = true` (already enabled) | Removes debug symbols — saves 50-90% on debug builds |
| Custom `linkall.x` overrides | Remove unused MEMORY regions |
| Avoid `print!` of floats | Replaces Big32x40 with manual fixed-point |
| Avoid `core::fmt::Display` for derived types | Each derive pulls in `Argument::new_*` |

## How to measure YOUR crate

```bash
# After `cargo +esp build -Z build-std=core --release`:
xtensa-esp32-elf-size -A target/xtensa-esp32-none-elf/release/<your-crate>

# Or for non-stripped, for full inspection:
xtensa-esp32-elf-readelf -l target/xtensa-esp32-none-elf/release/<your-crate>

# Also useful:
xtensa-esp32-elf-objdump -h target/xtensa-esp32-none-elf/release/<your-crate> \
    | grep -E "^\s+[0-9]+ \.text"
```
