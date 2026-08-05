# lzt-cart

Typed CART decision tree classifier in Rust. Port of `cart_classifier.c` from the
HexCortex research pipeline.

## Quick start

```rust
use lzt_cart::{Sample, Label, Tree, train};

let samples = vec![
    Sample::new([0.10, 0.20, 0.30, 0.40, 0.50, 0.60, 0.70, 0.80], Label::Low),
    Sample::new([0.90, 0.85, 0.95, 0.80, 0.90, 0.85, 0.95, 0.88], Label::Urgent),
    Sample::new([0.50, 0.45, 0.55, 0.50, 0.45, 0.55, 0.50, 0.45], Label::Review),
    // ...
];

let tree: Tree = train(&samples);
let pred = tree.predict(&[0.10, 0.20, 0.30, 0.40, 0.50, 0.60, 0.70, 0.80]);
assert_eq!(pred, Label::Low);
```

## License

Apache 2.0. See the workspace root `LICENSE-APACHE-2.0` for the full text.
See `NOTICE` for the per-crate attribution chain.
