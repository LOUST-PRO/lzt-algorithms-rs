# lzt-entropy

Pure-math impurity and information-theoretic criteria for decision trees.

- `gini` — Gini impurity from class counts.
- `entropy_shannon` — Shannon entropy from class probabilities.
- `info_gain` — Information gain: `H(parent) - weighted_avg(H(child))`.
- `gain_ratio` — Information gain divided by split information (Quinlan).
- `split_info` — Intrinsic value of the split (entropy of the split itself).

This is a leaf crate with no runtime dependencies. It is licensed under
**AGPL 3.0** (INDAUTOR-registered in Mexico) because the math primitives
are the original HexCortex research pipeline's API surface; we want to
preserve copyleft obligations across networked use.

## Quick start

```rust
use lzt_entropy::{gini, entropy_shannon, info_gain, gain_ratio, split_info};

// 4-class probability mass
let p = [0.5, 0.5, 0.0, 0.0];
assert!((gini(&p) - 0.5).abs() < 1e-12);
assert!((entropy_shannon(&p) - 1.0).abs() < 1e-12);

// Binary 50/50 split, IG ≈ 1.0, GR ≈ 1.0
let parent = [1.0, 1.0];
let left = [1.0, 0.0];
let right = [0.0, 1.0];
let ig = info_gain(&parent, &[&left, &right]);
assert!(ig > 0.9);
```

## Why AGPL 3.0

This crate is the math primitive used by the rest of the workspace
to score splits. If you build a network service on top of `lzt-entropy`,
the AGPL requires you to release your service's source.

If you need a more permissive license, use the upstream `linfa` or
`smartcore` instead — they provide similar impurity math under MIT/Apache.

## License

AGPL 3.0 (INDAUTOR-registered).
See the workspace root `LICENSE-AGPL-3.0` for the full text.
