# lzt-embedded-trees

`no_std` threshold cascade for AVR / ESP32 — zero heap, statically allocated,
binary inference. Port of `hexcortex_reservoir_readout.h` (and the surrounding
suite of C99 / MicroPython / ESP32 profile files) from the HexCortex research
pipeline.

## Quick start

```rust
#![no_std]
#![no_main]

use lzt_embedded_trees::{Features, predict, Label};

const F: Features = Features {
    active_fraction: 0.40,
    edge_density: 0.30,
    entropy_proxy: 0.55,
    run_compressibility: 0.50,
    length_norm: 0.20,
};

#[panic_handler]
fn panic(_: &core::panic::PanicInfo) -> ! {
    loop {}
}

#[no_mangle]
pub extern "C" fn entry() -> ! {
    let label = predict(&F);
    // ...
    loop {}
}
```

## What it does

The crate implements a 5-feature threshold cascade as a series of 3 boolean
expressions:

```text
urgent = f.length_norm > 0.85 && f.entropy_proxy > 0.55 && f.edge_density > 0.30;
low    = f.active_fraction <= 0.45 || f.run_compressibility <= 0.20;
review = !urgent && !low;
```

The cascade is implemented as a static method that does **no branching on the
predict path**: the compiler should turn the `if-else` into a CMOV or a
conditional select instruction. There is no heap, no allocation, and no
runtime initialization.

## Cross-compile

The crate is `no_std` and `panic_immediate_abort` (via the `panic-handler`
above). See the workspace root `README.md` for the AVR/ESP32 cross-compile
instructions.

## License

Apache 2.0. See the workspace root `LICENSE-APACHE-2.0` for the full text.
