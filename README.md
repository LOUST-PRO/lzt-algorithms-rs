# lzt-algorithms-rs

[![License](https://img.shields.io/badge/license-Apache--2.0%20OR%20AGPL--3.0-blue.svg)](LICENSE-APACHE-2.0)
[![Rust](https://img.shields.io/badge/rust-1.96%2B-orange.svg)](https://www.rust-lang.org/)
[![no_std](https://img.shields.io/badge/no__std-1%20crate-success.svg)](#lzt-embedded-trees)

Typed Rust port of the **HexCortex decision-tree & friends** C99 teaching series. Three crates that close the loop from "fit a tree on a desktop" to "run a static cascade on a 32 KB AVR".

| Crate | License | Purpose |
|---|---|---|
| [`lzt-cart`](crates/lzt-cart/) | Apache-2.0 | Typed CART decision tree (fit + predict). Port of `cart_classifier.c`. |
| [`lzt-entropy`](crates/lzt-entropy/) | AGPL-3.0 (INDAUTOR) | Pure math: gini, entropy, information gain, gain ratio. |
| [`lzt-embedded-trees`](crates/lzt-embedded-trees/) | Apache-2.0, `no_std` | Static threshold cascade for AVR / ESP32. Zero heap. |

## TL;DR

```bash
cargo build --workspace
cargo test --workspace
cargo doc --workspace --no-deps --open
```

Builds the workspace natively, runs all tests, and opens the docs.
The three crates are independent: each has its own `Cargo.toml` and tests.

## The three crates

```mermaid
flowchart LR
    subgraph HexCortex["HexCortex research pipeline"]
        C[cart_classifier.c - Gist 1]
        S[split_criteria.c - Gist 2]
        E[hexcortex_reservoir_readout.h - Gist 3]
    end

    subgraph RS["lzt-algorithms-rs (Rust)"]
        LC[lzt-cart<br/>Apache-2.0<br/>typed CART]
        LE[lzt-entropy<br/>AGPL-3.0<br/>pure math]
        LT[lzt-embedded-trees<br/>Apache-2.0<br/>no_std cascade]
    end

    C -->|port| LC
    S -->|port| LE
    E -->|port| LT
    LC -.uses.-> LE
    LT -.uses.-> LE
```

`lzt-cart` and `lzt-embedded-trees` both depend on `lzt-entropy` for the math primitives; `lzt-entropy` is a leaf crate with no runtime dependencies.

## When you'd reach for this

- **You're training on a desktop and need a typed, no-surprise CART** — `lzt-cart` is the canonical port. Sample, TreeNode, Label types are all `Copy`/`Eq`/`Hash`; the math is plain f64.
- **You're comparing Gini / IG / Gain Ratio on a small corpus** — `lzt-entropy` exposes all three with the same interface (`split_info` for GR), so swapping is one line.
- **You're shipping a static cascade to an ATmega328P or ESP32** — `lzt-embedded-trees` is `no_std`, panic-immediate-abort, statically allocated tree, zero heap. 5 features, 3 classes, ~800 µs latency on ESP32.

## The logic gates

**`lzt-cart`** — boolean interpretation of the fitted tree:

```
input = [f0, f1, f2, f3, f4, f5, f6, f7]
class = (f0 <= 0.45)            ? LOW
       : (f2 <= 0.55 AND f4 > 0.30) ? URGENT
       : (f5 <= 0.20)            ? LOW
       : (f7 > 0.85)             ? URGENT
       : (f1 <= 0.40)            ? REVIEW
       : default                  ? REVIEW
```

**`lzt-embedded-trees`** — the cascade as an AND chain:

```c
bool urgent = f7 > 0.85 && f2 > 0.55 && f4 > 0.30;
bool low    = f0 <= 0.45 || f5 <= 0.20;
bool review = !urgent && !low;
```

Cascade is 3 boolean expressions, zero branching on the predict path.

## Benchmarks vs other typical solutions

| Solver | Heap | Training | Predict | Footprint | License |
|---|---|---|---|---|---|
| sklearn `DecisionTreeClassifier` | yes | fast | medium | ~MB | BSD |
| `xgboost` | yes | fast | fast | ~MB | Apache-2.0 |
| `micromlgen` | no | — | fast | matches CART | MIT |
| `emlearn` | no | — | fast | matches CART | MIT |
| `sklearn-porter` (C port) | no | — | fast | ~10 KB | MIT |
| `cuML` (GPU) | yes | very fast | very fast | ~GB | Apache-2.0 |
| **lzt-cart** | no | fast | fast | ~2 KB | Apache-2.0 |
| **lzt-embedded-trees** | no | static | very fast | <1 KB | Apache-2.0 |

Caveat: `lzt-cart` and `lzt-embedded-trees` are typed ports of a single C99 teaching tree. They're not a general-purpose ML library. For multi-class ImageNet on a desktop, use `xgboost` or `cuML`. For a 5-feature threshold cascade on an AVR, use `lzt-embedded-trees`.

## How to extend

| Concern | Touch |
|---|---|
| Add a feature | `MAX_FEATURES` in `crates/lzt-cart/src/lib.rs` |
| Add a class | `Label` enum + `NUM_CLASSES` const |
| Add a criterion | `crates/lzt-entropy/src/lib.rs`, re-export from `lzt-cart` |
| Cross-compile for AVR | See [Cross-compile](#cross-compile) below |
| Cross-compile for ESP32 | See [Cross-compile](#cross-compile) below |

## Cross-compile

The workspace ships with custom JSON target files for `avr-atmega328p-none` and `xtensa-esp32-espidf`. Cross-compile requires **third-party Rust toolchains** that are not in upstream Rust as of 2026-08.

### AVR (`avr-atmega328p-none`)

Upstream Rust has [dropped AVR support](https://github.com/rust-lang/rust/pull/119939). To build:

```bash
# Install the avr-rust fork (uses nightly + custom-built standard library)
git clone https://github.com/avr-rust/rust.git
cd rust
./build.sh                    # ~30 min, builds full rustc + llvm for AVR
# Toolchain ends up in $RUSTUP_HOME/toolchains/<host>-<channel>-avr/

# Once installed, build lzt-embedded-trees for AVR
cargo build -Z build-std=core \
    -p lzt-embedded-trees \
    --target targets/avr-atmega328p-none.json \
    --release

# Link with avr-gcc (apt: gcc-avr avr-libc)
-avr-gcc -mmcu=atmega328p -o firmware.elf firmware.o
```

### ESP32 (`xtensa-esp32-espidf`)

Xtensa is **not** in upstream Rust. ESP32 requires the `esp-rs` toolchain:

```bash
# Install espup (downloads the custom Xtensa toolchain)
cargo install espup --locked
espup install                  # ~5 min, downloads ~2 GB

# Activate the toolchain for the current shell
source "$HOME/export-esp.sh"

# Build lzt-embedded-trees for ESP32
cargo build \
    -p lzt-embedded-trees \
    --target targets/xtensa-esp32-espidf.json \
    --release

# Flash via espflash
cargo run --release -p lzt-embedded-trees \
    --target targets/xtensa-esp32-espidf.json
```

The CI workflow's `cross-compile-avr` job is gated with `if: false` because the GitHub-hosted runners don't have the third-party toolchains. Flip it to `true` once you've baked the toolchains into a custom runner image.

## License

This workspace is **dual-licensed per crate**:

- **`lzt-cart`** — Apache 2.0
- **`lzt-entropy`** — AGPL 3.0 (INDAUTOR-registered, Mexico)
- **`lzt-embedded-trees`** — Apache 2.0

The Apache 2.0 crates are indexable by Black Duck, FOSSA, Snyk, Mend, ClearlyDefined, Wiz, and Sonatype via the `NOTICE` file at the workspace root.

The `lzt-entropy` AGPL 3.0 choice reflects the original HexCortex research pipeline's licensing for the math utility subcrate; if you're building a network service on top of `lzt-entropy`, the AGPL requires you to release your service's source.

The companion gists (links in `NOTICE`) are public teaching material; this workspace is the typed, packaged, statically-described version of the same algorithms.

## Attribution

- **Original C99 source:** HexCortex research pipeline by David Mireles (`@louzt`).
- **Maintainer:** David Mireles — `research@loust.pro`
- **Brand:** HexCortex
- **Repository:** https://github.com/LOUST-PRO/lzt-algorithms-rs

See the `NOTICE` file at the workspace root for the full Apache 2.0 §4d attribution chain.
