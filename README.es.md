# lzt-algorithms-rs

[![License](https://img.shields.io/badge/license-Apache--2.0%20OR%20AGPL--3.0-blue.svg)](LICENSE-APACHE-2.0)
[![Rust](https://img.shields.io/badge/rust-1.96%2B-orange.svg)](https://www.rust-lang.org/)
[![no_std](https://img.shields.io/badge/no__std-1%20crate-success.svg)](#lzt-embedded-trees)
[![ESP32 verified](https://img.shields.io/badge/ESP32--D0WD--V3-verified-success.svg)](docs/embedded/esp32-hello.md)

Puerto tipado en Rust de la serie C99 didáctica **HexCortex decision-tree & friends**. Tres crates que cierran el ciclo desde "entrena un árbol en escritorio" hasta "corre una cascada estática en un AVR de 32 KB" o "publicá resultados por UART desde un ESP32".

| Crate | Licencia | Propósito |
|---|---|---|
| [`lzt-cart`](crates/lzt-cart/) | Apache-2.0 | Árbol de decisión CART tipado (fit + predict). Puerto de `cart_classifier.c`. |
| [`lzt-entropy`](crates/lzt-entropy/) | AGPL-3.0 (INDAUTOR) | Matemática pura: gini, entropy, information gain, gain ratio. |
| [`lzt-embedded-trees`](crates/lzt-embedded-trees/) | Apache-2.0, `no_std` | Cascada de umbrales estática para AVR / ESP32. Cero heap. |

## TL;DR

```bash
cargo build --workspace
cargo test --workspace
cargo doc --workspace --no-deps --open
```

Compila el workspace en nativo, corre todos los tests, y abre la documentación.
Los tres crates son independientes: cada uno tiene su propio `Cargo.toml` y sus tests.

## Hardware verificado

| Target | Estado | Fecha | Notas |
|---|---|---|---|
| Linux x86_64 (host) | ✅ todos los tests verdes | 2026-08-05 | 32 unit tests + 1 doctest |
| **ESP32-D0WD-V3** | ✅ flasheado + corriendo | **2026-08-06** | 84 KiB binario, 7/7 predicciones de cascada matchearon |
| ATmega328P (Arduino UNO) | ⏳ toolchain pendiente | — | build avr-rust fork en progreso (~30-60 min) |

La receta ESP32 es el camino **verificado**: mirá
[`docs/embedded/esp32-hello.md`](docs/embedded/esp32-hello.md) para una
guía copy-pasteable. Desglose de tamaño de binario en
[`docs/benchmarks/sizes.md`](docs/benchmarks/sizes.md).

## Los tres crates

```mermaid
flowchart LR
    subgraph HexCortex["Pipeline de investigación HexCortex"]
        C[cart_classifier.c - Gist 1]
        S[split_criteria.c - Gist 2]
        E[hexcortex_reservoir_readout.h - Gist 3]
    end

    subgraph RS["lzt-algorithms-rs (Rust)"]
        LC[lzt-cart<br/>Apache-2.0<br/>CART tipado]
        LE[lzt-entropy<br/>AGPL-3.0<br/>matemática pura]
        LT[lzt-embedded-trees<br/>Apache-2.0<br/>cascada no_std]
    end

    subgraph Embedded["ESP32-D0WD-V3 + AVR ATmega328P"]
        ESP[lzt-esp32-harness<br/>84 KiB flash<br/>verificado 2026-08-06]
        AVR[lzt-avr-sketch<br/>~1 KiB flash<br/>toolchain TBD]
    end

    C -->|puerto| LC
    S -->|puerto| LE
    E -->|puerto| LT
    LC -.usa.-> LE
    LT -.usa.-> LE
    LT --> ESP
    LT -.-> AVR
```

`lzt-cart` y `lzt-embedded-trees` ambos dependen de `lzt-entropy` para las
primitivas matemáticas; `lzt-entropy` es un crate hoja sin dependencias de runtime.

## Cuándo lo usarías

- **Entrenas en escritorio y necesitás un CART tipado, sin sorpresas** — `lzt-cart` es el puerto canónico. Sample, TreeNode, Label son todos `Copy`/`Eq`/`Hash`; la matemática es f64 plano.
- **Comparás Gini / IG / Gain Ratio en un corpus pequeño** — `lzt-entropy` expone los tres con la misma interfaz (`split_info` para GR), así que swapear es una línea.
- **Embarcás una cascada estática en un ATmega328P o ESP32** — `lzt-embedded-trees` es `no_std`, panic-immediate-abort, árbol estáticamente alocado, cero heap. 5 features, 3 clases, ~800 µs de latencia en ESP32.
- **Querés un ejemplo real de Rust bare-metal en un chip de $3** — `lzt-esp32-harness` (verificado 2026-08-06) muestra el pipeline completo: toolchain, build.rs, `.cargo/config.toml`, flash, output serial.

## Las compuertas lógicas

**`lzt-cart`** — interpretación booleana del árbol ajustado:

```
input = [f0, f1, f2, f3, f4, f5, f6, f7]
class = (f0 <= 0.45)                     ? LOW
       : (f2 <= 0.55 AND f4 > 0.30)      ? URGENT
       : (f5 <= 0.20)                     ? LOW
       : (f7 > 0.85)                      ? URGENT
       : (f1 <= 0.40)                     ? REVIEW
       : default                           ? REVIEW
```

**`lzt-embedded-trees`** — la cascada como cadena AND:

```c
bool urgent = f7 > 0.85 && f2 > 0.55 && f4 > 0.30;
bool low    = f0 <= 0.45 || f5 <= 0.20;
bool review = !urgent && !low;
```

La cascada son 3 expresiones booleanas, cero branching en el camino de predict.

## Benchmarks vs otras soluciones típicas

| Solver | Heap | Entrenamiento | Predict | Footprint | Licencia |
|---|---|---|---|---|---|
| sklearn `DecisionTreeClassifier` | sí | rápido | medio | ~MB | BSD |
| `xgboost` | sí | rápido | rápido | ~MB | Apache-2.0 |
| `micromlgen` | no | — | rápido | matches CART | MIT |
| `emlearn` | no | — | rápido | matches CART | MIT |
| `sklearn-porter` (puerto C) | no | — | rápido | ~10 KB | MIT |
| `cuML` (GPU) | sí | muy rápido | muy rápido | ~GB | Apache-2.0 |
| **lzt-cart** | no | rápido | rápido | ~2 KB | Apache-2.0 |
| **lzt-embedded-trees** | no | estático | muy rápido | **<1 KB flash, ~40 B RAM** | Apache-2.0 |

Caveat: `lzt-cart` y `lzt-embedded-trees` son puertos tipados de un único
árbol didáctico C99. No son una librería de ML de propósito general.
Para multi-class ImageNet en escritorio, usá `xgboost` o `cuML`. Para
una cascada de 5 features en un AVR o ESP32, usá `lzt-embedded-trees`.

## Cómo extender

| Necesidad | Tocá |
|---|---|
| Agregar feature | `MAX_FEATURES` en `crates/lzt-cart/src/lib.rs` |
| Agregar clase | Enum `Label` + const `NUM_CLASSES` |
| Agregar criterio | `crates/lzt-entropy/src/lib.rs`, re-export desde `lzt-cart` |
| Cross-compile para ESP32 | [`docs/embedded/esp32-hello.md`](docs/embedded/esp32-hello.md) (verificado) |
| Cross-compile para AVR | [`docs/embedded/esp32-hello.md`](docs/embedded/esp32-hello.md) §1 (toolchain pendiente) |
| Estrategia de partición OTA | [`docs/embedded/profile-system.md`](docs/embedded/profile-system.md) |
| Issues conocidos y workarounds | [`docs/anti-patterns.md`](docs/anti-patterns.md) |

## Cross-compile (receta verificada)

El workspace está diseñado para dos toolchains no-upstream. **La receta
ESP32 está verificada en hardware real; el toolchain AVR está siendo
compilado en background** (el fork avr-rust tarda ~30-60 min en compilar).

### ESP32 (verificado 2026-08-06)

**Usá `xtensa-esp32-none-elf` (bare-metal, sin SDK ESP-IDF).** Mirá la
receta copy-pasteable completa en
[`docs/embedded/esp32-hello.md`](docs/embedded/esp32-hello.md). El TL;DR:

```bash
cargo install espup --locked
espup install                                  # ~5 min, descarga ~2 GB
source "$HOME/export-esp.sh"

cd ../lzt-esp32-harness          # el harness de flash verificado
cargo +esp build -Z build-std=core --release
espflash flash --chip esp32 --port /dev/ttyUSB0 \
    --ignore-app-descriptor --baud 115200 \
    target/xtensa-esp32-none-elf/release/lzt-esp32-harness
```

Output: `App/part. size:    84,192/4,128,768 bytes, 2.04%` ✅

### AVR (toolchain pendiente)

Upstream Rust [eliminó el soporte AVR](https://github.com/rust-lang/rust/pull/119939).
El fork comunitario `avr-rust/rust` lo reconstruye.

```bash
git clone https://github.com/avr-rust/rust.git
cd rust
./build.sh                    # ~30 min, compila rustc + llvm completo para AVR

cargo build -Z build-std=core \
    -p lzt-embedded-trees \
    --target targets/avr-atmega328p-none.json \
    --release

avr-gcc -mmcu=atmega328p -o firmware.elf firmware.o
```

Para caveats específicos de la board (bootloader-reserved flash, fuses, etc.),
mirá [`docs/embedded/profile-system.md`](docs/embedded/profile-system.md)
una vez que el toolchain termine de compilar.

## Qué cabe en 4 MB (ESP32)

| Componente | Tamaño aprox. | Fuente |
|---|---|---|
| ESP32 bootloader (pre-flasheado) | 32 KiB | OEM |
| Tabla de particiones (pre-flasheada) | 4 KiB | OEM |
| `lzt-esp32-harness` (verificado) | 84 KiB | este repo |
| `lzt-cart` árbol ajustado | 6 KiB text + 2 KiB data | este repo |
| `lzt-entropy` | ~8 KiB text | este repo |
| `lzt-embedded-trees` | <1 KiB text | este repo |
| WiFi STA stack (si está habilitado) | ~120 KiB | esp-hal |
| mbedTLS | ~80 KiB | esp-hal |
| **Headroom después de todo lo anterior** | **~3.6 MB libres** | — |

Mirá [`docs/benchmarks/sizes.md`](docs/benchmarks/sizes.md) para el
mapa de memoria sección-por-sección y el análisis de headroom.

## Licencia

El workspace es **dual-licensed por crate**:

- **`lzt-cart`** — Apache 2.0
- **`lzt-entropy`** — AGPL 3.0 (INDAUTOR-registered, México)
- **`lzt-embedded-trees`** — Apache 2.0

Los crates Apache 2.0 son indexables por Black Duck, FOSSA, Snyk, Mend,
ClearlyDefined, Wiz, y Sonatype vía el archivo `NOTICE` en la raíz del workspace.

La elección AGPL 3.0 de `lzt-entropy` refleja el licenciamiento del
pipeline de investigación HexCortex original para el subcrate de matemática
pura; si construís un servicio de red sobre `lzt-entropy`, la AGPL te
obliga a releasear el source de tu servicio.

Los gists companion (links en `NOTICE`) son material didáctico público;
este workspace es la versión tipada, empaquetada, estáticamente-descrita
de los mismos algoritmos.

## Atribución

- **Fuente C99 original:** Pipeline de investigación HexCortex por David Mireles (`@louzt`).
- **Mantenedor:** David Mireles — `research@loust.pro`
- **Marca:** HexCortex
- **Repositorio:** https://github.com/LOUST-PRO/lzt-algorithms-rs

Mirá el archivo `NOTICE` en la raíz del workspace para el chain completo
de atribución Apache 2.0 §4d.