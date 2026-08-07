# Anti-patterns and known issues

What **doesn't work** with embedded Rust on AVR + ESP32, and the verified
workarounds. Curated from real attempts (mostly failed) during the
`lzt-algorithms-rs` ↔ ESP32-D0WD-V3 migration (2026-08).

## ESP32 toolchain

### ❌ `target = "xtensa-esp32-espidf"`

```bash
cargo +esp build --target xtensa-esp32-espidf --release
```

Build succeeds, but the binary crashes silently at PC `0x4007958d` (ROM
abort). The crt0 of `-espidf` calls ROM functions like `esp_image_load`
that require the full ESP-IDF SDK linked. Bare-metal needs `none-elf`.

✅ Use `xtensa-esp32-none-elf` + `xtensa-lx-rt 0.22.0`.

### ❌ Raw UART register writes before `esp_hal::init`

```rust
unsafe {
    let uart = &*esp32::UART0::ptr();
    uart.fifo.write(|w| unsafe { w.txfifo_rst().bits(1) });
    // ...
}
```

The ESP32 runs at 40 MHz by default (after reset). The ROM bootloader
clocks it differently than the user code expects. Bare register writes
without `esp_hal::init` (which switches to 240 MHz via `PeriphInit`)
produce garbage on TX.

✅ Always call `esp_hal::init(esp_hal::Config::default().with_cpu_clock(CpuClock::max()))` first.

### ❌ `xtensa-lx-rt = "0.15"`

```toml
xtensa-lx-rt = "0.15"
```

`0.15` uses `asm!` inside `#[naked]` functions, which is rejected with
**E0787** (instruction not supported on this target) under Rust 1.95.

✅ Use `xtensa-lx-rt = "0.22"` (modern `naked_asm!`).

### ❌ `-Tdevice.x` alone (the silent killer)

```rust
// build.rs
println!("cargo:rustc-link-arg=-Tdevice.x");
```

Build **succeeds**, links, **but entry point is 0x0**. The ROM bootloader
tries to jump to address 0x0 (which is the boot ROM, not your app) and
the chip appears dead. No panic, no log, just silence.

✅ Use `-Tlinkall.x` (esp-hal's combined script). It sets
`ENTRY(Reset)` + `PROVIDE(DefaultHandler = EspDefaultHandler)`.

### ❌ `-T link.x` alone (xtensa-lx-rt's)

```rust
println!("cargo:rustc-link-arg=-T link.x");
```

Fails at link with:

```
undefined reference to `DefaultHandler'
```

xtensa-lx-rt's `link.x` PROVIDEs every Xtensa interrupt handler, but
they all default to a `DefaultHandler` symbol that is **not defined**
in `link.x` itself.

✅ `-Tlinkall.x` (esp-hal) chains `link.x` + `hal-defaults.x`, which
provides `EspDefaultHandler = loop { continue; }`.

### ❌ `#[inline(always)] unsafe extern "C" fn memcpy`

```rust
#[inline(always)]
unsafe extern "C" fn memcpy(dst: *mut u8, src: *const u8, n: usize) -> *mut u8 {
    let mut d = dst; let mut s = src;
    while n > 0 { *(d as *mut u8) = *(s as *const u8); d = d.add(1); s = s.add(1); n -= 1; }
    dst
}
```

LTO + `#[inline(always)]` lets the linker dead-code-eliminate the shim
when it has zero non-transitive users. Result: linker still complains
"undefined reference to `memcpy`".

✅ Use `#[no_mangle]` + skip the `#[inline(always)]`:

```rust
#[no_mangle]
unsafe extern "C" fn memcpy(dst: *mut u8, src: *const u8, n: usize) -> *mut u8 {
    // ...
}
```

OR (better) enable the `mem` feature in `compiler-builtins`:

```toml
[dependencies]
compiler-builtins = { version = "0.1", features = ["mem"] }
```

(This second approach failed on Xtensa nightly during the migration with
`error: ...(impl_Debug macro...`. Pin Rust 1.95.0.0 with espup and retry
if needed; if it still fails, fall back to the `#[no_mangle]` shim.)

### ❌ `core::arch::asm!("waiti 0")` in DefaultHandler

```rust
#[no_mangle]
unsafe extern "C" fn DefaultHandler() {
    core::arch::asm!("waiti 0");
}
```

Xtensa assembly isn't stable on this target as of `rustc` 1.95.
Compiler emits E0658 (unstable) for some instructions.

✅ Use plain Rust:

```rust
#[no_mangle]
unsafe extern "C" fn DefaultHandler() {
    loop { continue; }
}
```

`waiti 0` saves ~10% power, but correctness > power here.

### ❌ `{:.2}` formatting for f32 / f64

```rust
writeln!(uart, "value = {:.2}", 0.123_f32).ok();
```

Core's f32 Display pulls `Big32x40` → `memcpy` → `memset`. Without
`memcpy` defined (see above), this fails to link. With it defined, it
adds ~20 KiB to the binary.

✅ Use fixed-point integer formatting:

```rust
let v: f64 = 0.123;
let scaled = (v * 100.0) as i32;
writeln!(uart, "value = {}.{:02}", scaled / 100, (scaled % 100).abs()).ok();
```

OR write a tiny helper:

```rust
fn fp(v: f64) -> String {
    let s = (v * 100.0).round() as i32;
    format!("{}.{:02}", s / 100, (s % 100).abs())
}
```

### ❌ Calling `print!` of `f64` from `panic_handler`

```rust
#[panic_handler]
fn panic(info: &core::panic::PanicInfo) -> ! {
    println!("{}", info);
    loop {}
}
```

Same `Big32x40` issue. Worse: `PanicInfo`'s Display can include f64
when formatting `core::fmt::Error` or address values.

✅ Make the panic handler a no-op:

```rust
#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    loop {}
}
```

For QA builds, write to a known register:

```rust
#[panic_handler]
fn panic(info: &core::panic::PanicInfo) -> ! {
    unsafe {
        let uart = &*esp32::UART0::ptr();
        uart.fifo.write(|w| w.txfifo_rst().set_bit());
    }
    let mut uart = Uart::new(unsafe { esp_hal::peripherals::UART0::steal() }, UartConfig::default());
    writeln!(uart, "PANIC: {}", info).ok();
    loop {}
}
```

(But this still pulls Display chains. For now, leave it as `loop {}`
in `release`, log via `esp-println`'s `println!` macro in `dev`.)

### ❌ Using `Vec<T>` or `String`

`alloc` pulls in a heap allocator. ESP32's default heap (`EspHeap`)
adds ~6 KiB and you need `esp-alloc` configured in `esp_hal::init`.

For `lzt-embedded-trees`, use stack-allocated `[T; N]` arrays.

### ❌ `#[derive(Display)]` on big enums

```rust
#[derive(core::fmt::Display)]
enum Decision {
    Urgent,
    Review,
    Low,
}
```

Pulls in `Argument::new_*` machinery for every variant — ~3 KiB.

✅ Manually `impl Display`:

```rust
impl core::fmt::Display for Decision {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str(match self {
            Self::Urgent => "urgent",
            Self::Review => "review",
            Self::Low => "low",
        })
    }
}
```

## AVR

### ❌ Building Rust core for AVR (TBD)

`rust-lang/rust` dropped AVR support in PR #119939. The community fork
`avr-rust/rust` rebuilds it. **No prebuilt binaries** — `x.py build`
takes 30–60 min on a laptop.

### ❌ `panic_immediate_abort` on AVR without `core::fmt::Display`

Adding `panic_immediate_abort` to the release profile causes link errors
because it strips the format machinery that `core::fmt::Error` still
needs in some paths.

✅ Either include `panic-abort-build` only or use `panic = "abort"`
without `panic_immediate_abort`.

## Toolchain noise

### ❌ espup install fails on Python <3.8

espup 0.17 requires Python 3.8+. If `python` is missing or only 2.7 is
installed, install python3 first.

### ❌ `python: not found` from `./configure`

When building avr-rust fork: `x.py` itself uses `#!/usr/bin/env python`
in some hooks. Some `./configure` scripts do too. Fix:

```bash
ln -sf $(which python3) /tmp/lcl-bin/python
export PATH="/tmp/lcl-bin:$PATH"
```

### ❌ espflash monitor "Failed to initialize input reader" on Linux

```text
$ espflash monitor /dev/ttyUSB0
Failed to initialize input reader
```

This is a known espflash 4.x bug on Linux when stdin isn't a TTY.
**Use `esptool.py monitor` instead**, or read raw serial via Python
pyserial (recipe in `embedded/esp32-hello.md` §4).

### ❌ First build hangs forever

`cargo +esp build -Z build-std=core` builds core from source on first
run. This is **expected** — takes 3-5 min. Subsequent builds use
`~/.rustup/toolchains/esp/lib/rustlib/xtensa-esp32-none-elf/lib/*.rlib`
cache.

## Logic-level gotchas

### ❌ Cascade predict returns wrong label on uniform inputs

```rust
// Input: 0.50 0.50 0.50 0.50 0.50
// All values equal — which class wins?
predict(&[0.5; 5])
```

If your tree has equal-thresholds for two nodes, the first matching
branch wins. Document this clearly:

```rust
/// NOTE: For inputs where multiple branches match (uniform inputs),
/// the prediction is deterministic but depends on tree traversal order.
/// Verify behavior with `cargo test` before relying on tie-breaking.
```

### ❌ Mistaking `f32` epsilon comparisons

```rust
let a = 0.1_f32 + 0.2_f32;
let b = 0.3_f32;
assert_eq!(a, b);  // FAILS
```

Use `(a - b).abs() < f32::EPSILON` or `(a - b).abs() < 1e-6`.

## File-system / CI

### ❌ `cargo +esp build` on GitHub-hosted runners

GitHub-hosted Linux runners don't have espup installed. The build
will fail at the toolchain step. Two options:

1. **Bake a custom runner image** with espup pre-installed. (DOCS:
   `~/.claude/rules/automation-risk-asymmetry.md` — high cost, low
   value unless shipping many embedded crates.)
2. **Run CI on your laptop** (`make embedded-ci`) until embedded
   becomes a stable product line.

For now, `docs/embedded/profile-system.md` shows the GitHub workflow
template but it should remain gated with `if: false`.

## Other

### ❌ Mixing `f32` and `f64` in algorithm code

```rust
fn predict(inputs: &[f32]) -> u8 {
    // ...
    let threshold: f64 = 0.5;  // ← type mismatch
    if inputs[0] as f64 > threshold { 1 } else { 0 }
}
```

Generates implicit casts. On AVR, this can pull in f64 soft-float
(~3 KiB).

✅ Pick `f32` (or fixed-point `i32` for tight AVR) and stick with it.

### ❌ Logging in a 1 kHz interrupt

```rust
#[interrupt]
fn TIMER1() {
    writeln!(uart, "tick {}", count).ok();   // ← blocking IO, 1ms+ latency
}
```

`esp-println` + UART write takes 1-3 ms at 115200 baud. Inside a 1 kHz
interrupt, you'll lose ticks.

✅ Use a ring buffer + main-loop drain:

```rust
static LOG_BUF: RingBuffer<U1024> = RingBuffer::new();

#[interrupt]
fn TIMER1() {
    LOG_BUF.push(get_timestamp());
}

#[main]
fn main() -> ! {
    loop {
        if let Some(ts) = LOG_BUF.pop() {
            writeln!(uart, "tick {}", ts).ok();
        }
    }
}
```

## When you're stuck

1. Read `memory/esp-hal-1.1-esp32-bare-metal-linkall-x.md` (recipes).
2. Search `docs/embedded/` for similar setups.
3. Check `xtensa-esp32-none-elf` docs:
   <https://docs.rust-embedded.org/embedonomicon/main.html>
4. Ask in `#esp-rs` Discord with: toolchain version, exact error, the
   3 lines of build.rs.