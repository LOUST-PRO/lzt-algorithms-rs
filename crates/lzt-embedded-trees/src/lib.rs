//! `no_std` threshold cascade for AVR / ESP32 — zero heap, statically allocated.
//!
//! This crate is a no-heap port of `hexcortex_reservoir_readout.h` from the
//! HexCortex research pipeline. The model is a hard-coded threshold cascade
//! for 5 features and 3 classes; the cascade is implemented as a series of
//! 3 boolean expressions with no branching on the predict path.
//!
//! # Memory profile
//!
//! - Static binary: `<200 bytes` of `.rodata` (the threshold constants).
//! - Stack: `40 bytes` for the `Features` struct (5 × `f64`).
//! - Heap: zero.
//!
//! # Safety
//!
//! This is a decision-supporting readout. It must not be the sole input to a
//! safety-critical decision without a certified interlock layer.
//!
//! # License
//!
//! Apache-2.0. See the workspace root.

#![no_std]
#![forbid(unsafe_code)]
#![warn(missing_docs)]

/// Number of features used by the cascade.
pub const NUM_FEATURES: usize = 5;

/// Number of output classes in the cascade.
pub const NUM_CLASSES: usize = 3;

/// The five reservoir features consumed by the cascade.
///
/// These are the same five features as the C99 reference:
/// - `active_fraction`: fraction of active reservoir units ∈ [0, 1].
/// - `edge_density`: density of near-saturation edges ∈ [0, 1].
/// - `entropy_proxy`: rough entropy estimate ∈ [0, 1].
/// - `run_compressibility`: compressibility of the active run-length
///   sequence ∈ [0, 1] (higher = more compressible = more structured).
/// - `length_norm`: normalized active-run length ∈ [0, 1].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Features {
    /// `0.0..=1.0` — fraction of active reservoir units.
    pub active_fraction: f64,
    /// `0.0..=1.0` — density of near-saturation edges.
    pub edge_density: f64,
    /// `0.0..=1.0` — rough entropy estimate.
    pub entropy_proxy: f64,
    /// `0.0..=1.0` — compressibility of the active run-length sequence.
    pub run_compressibility: f64,
    /// `0.0..=1.0` — normalized active-run length.
    pub length_norm: f64,
}

/// The three output classes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Label {
    /// Low-priority signal.
    Low,
    /// Review signal — needs human triage.
    Review,
    /// Urgent signal — needs immediate intervention.
    Urgent,
}

/// Return the human-readable short name for the label.
pub fn label_name(label: Label) -> &'static str {
    match label {
        Label::Low => "low",
        Label::Review => "review",
        Label::Urgent => "urgent",
    }
}

/// Run the cascade on a `Features` struct and return the predicted label.
///
/// The cascade is expressed as 3 boolean expressions:
///
/// ```text
/// urgent = f.length_norm > 0.85 && f.entropy_proxy > 0.55 && f.edge_density > 0.30;
/// low    = f.active_fraction <= 0.45 || f.run_compressibility <= 0.20;
/// review = !urgent && !low;
/// ```
///
/// The compiler should turn the `if-else` into a `select`/`cmov` — no
/// branching on the predict path.
pub fn predict(f: &Features) -> Label {
    let urgent = f.length_norm > 0.85 && f.entropy_proxy > 0.55 && f.edge_density > 0.30;
    let low = f.active_fraction <= 0.45 || f.run_compressibility <= 0.20;
    if urgent {
        Label::Urgent
    } else if low {
        Label::Low
    } else {
        Label::Review
    }
}

/// Same as [`predict`], but writes the short name into a caller-provided
/// buffer. Returns the number of bytes written (excluding the NUL).
///
/// This is the no-heap equivalent of `format!("{}", label_name(predict(f)))`.
/// Useful for embedded logging where heap allocation is forbidden.
pub fn predict_to_buf(f: &Features, buf: &mut [u8]) -> usize {
    let label = predict(f);
    let name = label_name(label);
    let bytes = name.as_bytes();
    let n = bytes.len().min(buf.len().saturating_sub(1));
    buf[..n].copy_from_slice(&bytes[..n]);
    if n < buf.len() {
        buf[n] = 0;
    }
    n
}

/// Default thresholds exposed as constants so callers can build their own
/// cascades (or unit-test them as integers).
pub mod thresholds {
    /// `length_norm > URGENT_LENGTH_THRESHOLD` triggers urgency check.
    pub const URGENT_LENGTH_THRESHOLD: f64 = 0.85;
    /// `entropy_proxy > URGENT_ENTROPY_THRESHOLD` enables urgency.
    pub const URGENT_ENTROPY_THRESHOLD: f64 = 0.55;
    /// `edge_density > URGENT_EDGE_THRESHOLD` enables urgency.
    pub const URGENT_EDGE_THRESHOLD: f64 = 0.30;
    /// `active_fraction <= LOW_ACTIVE_THRESHOLD` triggers low-priority check.
    pub const LOW_ACTIVE_THRESHOLD: f64 = 0.45;
    /// `run_compressibility <= LOW_COMPRESS_THRESHOLD` triggers low-priority check.
    pub const LOW_COMPRESS_THRESHOLD: f64 = 0.20;
}

#[cfg(test)]
mod tests {
    use super::*;

    fn midpoint_features() -> Features {
        Features {
            active_fraction: 0.50,
            edge_density: 0.50,
            entropy_proxy: 0.50,
            run_compressibility: 0.50,
            length_norm: 0.50,
        }
    }

    #[test]
    fn urgent_input_classifies_urgent() {
        let f = Features {
            active_fraction: 0.50,
            edge_density: 0.40,
            entropy_proxy: 0.60,
            run_compressibility: 0.50,
            length_norm: 0.90,
        };
        assert_eq!(predict(&f), Label::Urgent);
    }

    #[test]
    fn low_input_classifies_low() {
        let f = Features {
            active_fraction: 0.30,
            edge_density: 0.50,
            entropy_proxy: 0.50,
            run_compressibility: 0.50,
            length_norm: 0.50,
        };
        assert_eq!(predict(&f), Label::Low);
    }

    #[test]
    fn midpoint_input_classifies_review() {
        // All features at 0.50: not urgent (length_norm=0.50 < 0.85),
        // not low (active_fraction=0.50 > 0.45, run_compressibility=0.50 > 0.20),
        // so falls through to Review.
        let f = midpoint_features();
        assert_eq!(predict(&f), Label::Review);
    }

    #[test]
    fn label_names_match_gist_doc() {
        assert_eq!(label_name(Label::Low), "low");
        assert_eq!(label_name(Label::Review), "review");
        assert_eq!(label_name(Label::Urgent), "urgent");
    }

    #[test]
    fn predict_to_buf_writes_nul_terminated_string() {
        let f = midpoint_features();
        let mut buf = [0u8; 16];
        let n = predict_to_buf(&f, &mut buf);
        assert_eq!(n, 6);
        assert_eq!(&buf[..6], b"review");
        assert_eq!(buf[6], 0);
    }

    #[test]
    fn predict_to_buf_handles_short_buffer() {
        let f = midpoint_features();
        let mut buf = [0u8; 4];
        let n = predict_to_buf(&f, &mut buf);
        assert_eq!(n, 3);
        assert_eq!(&buf[..3], b"rev");
        assert_eq!(buf[3], 0);
    }

    #[test]
    fn num_features_and_classes_match_spec() {
        assert_eq!(NUM_FEATURES, 5);
        assert_eq!(NUM_CLASSES, 3);
    }
}
