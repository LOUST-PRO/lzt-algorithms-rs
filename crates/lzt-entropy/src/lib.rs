//! Pure-math impurity and information-theoretic criteria for decision trees.
//!
//! This crate is the math primitive layer of the `lzt-algorithms-rs`
//! workspace. It exposes four functions that together cover the standard
//! decision-tree literature:
//!
//! - [`gini`] — Gini impurity from a probability mass over classes.
//! - [`entropy_shannon`] — Shannon entropy (base-2) from a probability mass.
//! - [`info_gain`] — Information gain of a split: `H(parent) - Σ weighted H(child)`.
//! - [`gain_ratio`] — Quinlan's gain ratio: `info_gain / split_info`.
//! - [`split_info`] — Intrinsic split value: `H(partition sizes)`.
//!
//! The crate is allocation-free (`Vec`/`String` only appear in tests) but
//! not `no_std` — it requires `std` for `f64::log2()` and `f64::log()`.
//! For an `no_std`-only build, port the `log2` calls to `ln / LN_2`.
//!
//! # License
//!
//! AGPL-3.0 (INDAUTOR-registered). See the workspace root for the full text.

#![warn(missing_docs)]

/// Compute Gini impurity from a probability mass over classes.
///
/// `p[k]` is the probability of class `k`. The sum of `p` should be ≈ 1.0
/// (callers may pass counts; the function does not normalize).
///
/// Returns `1 - Σ p[k]^2` ∈ [0, 1 - 1/N] for N classes.
pub fn gini(p: &[f64]) -> f64 {
    let mut acc = 0.0;
    for &pk in p {
        acc += pk * pk;
    }
    1.0 - acc
}

/// Compute Gini impurity from integer class counts and a total.
///
/// Equivalent to `gini(&counts_to_probs(counts, total))` but avoids the
/// division — more numerically stable for large counts.
pub fn gini_from_counts(counts: &[usize], total: usize) -> f64 {
    if total == 0 {
        return 0.0;
    }
    let n = total as f64;
    let mut acc = 0.0;
    for &c in counts {
        let pk = (c as f64) / n;
        acc += pk * pk;
    }
    1.0 - acc
}

/// Compute Shannon entropy (base-2) from a probability mass over classes.
///
/// Returns `H(p) = -Σ p[k] × log2(p[k])` with the convention `0 log 0 = 0`.
pub fn entropy_shannon(p: &[f64]) -> f64 {
    let mut acc = 0.0;
    for &pk in p {
        if pk > 0.0 {
            acc -= pk * pk.log2();
        }
    }
    acc
}

/// Convert class counts to a probability mass of total `1.0`.
fn counts_to_probs(counts: &[usize], total: usize) -> Vec<f64> {
    if total == 0 {
        return vec![0.0; counts.len()];
    }
    let n = total as f64;
    counts.iter().map(|&c| (c as f64) / n).collect()
}

/// Compute information gain from class counts at the parent and at each child.
///
/// `parent_counts` is the empirical class distribution at the parent node;
/// `children_counts` is one slice per child branch. The total sample count
/// is the sum of `parent_counts`; mismatched totals return `0.0`.
pub fn info_gain_from_counts(parent_counts: &[usize], children_counts: &[&[usize]]) -> f64 {
    let parent_total: usize = parent_counts.iter().sum();
    if parent_total == 0 {
        return 0.0;
    }
    let parent_h = gini_from_counts(parent_counts, parent_total);
    let mut weighted = 0.0;
    for child in children_counts {
        let child_total: usize = child.iter().sum();
        if child_total == 0 {
            continue;
        }
        let child_h = gini_from_counts(child, child_total);
        weighted += (child_total as f64) * child_h;
    }
    parent_h * (parent_total as f64) - weighted
}

/// Compute information gain from a probability mass at the parent and at each child.
///
/// `parent` is the probability mass at the parent node; `children` is one
/// probability mass per child branch.
pub fn info_gain(parent: &[f64], children: &[&[f64]]) -> f64 {
    let parent_h = gini(parent);
    let mut weighted = 0.0;
    for child in children {
        let child_h = gini(child);
        // Weight by the sum of the child's probability mass (must be ≤ 1).
        let w: f64 = child.iter().sum();
        weighted += w * child_h;
    }
    parent_h - weighted
}

/// Compute Shannon entropy from integer class counts.
pub fn entropy_from_counts(counts: &[usize], total: usize) -> f64 {
    if total == 0 {
        return 0.0;
    }
    let p = counts_to_probs(counts, total);
    entropy_shannon(&p)
}

/// Compute intrinsic split information (Quinlan) from per-child sample counts.
///
/// `children_counts` is one slice per child branch; the total sample count
/// is the sum across all children. Returns `H(child_fractions)` with `0 log 0 = 0`.
///
/// A high `split_info` means the split is balanced (many children with similar
/// sizes); a low `split_info` means the split is imbalanced (one child dominates).
pub fn split_info(children_counts: &[&[usize]]) -> f64 {
    let total: usize = children_counts.iter().map(|c| c.iter().sum::<usize>()).sum();
    if total == 0 {
        return 0.0;
    }
    let mut acc = 0.0;
    for child in children_counts {
        let child_total: usize = child.iter().sum();
        if child_total == 0 {
            continue;
        }
        let p = (child_total as f64) / (total as f64);
        acc -= p * p.log2();
    }
    acc
}

/// Compute Quinlan's gain ratio: `info_gain / split_info`.
///
/// Returns `0.0` if `split_info` is zero (degenerate split).
pub fn gain_ratio(parent_counts: &[usize], children_counts: &[&[usize]]) -> f64 {
    let ig = info_gain_from_counts(parent_counts, children_counts);
    let si = split_info(children_counts);
    if si <= 0.0 {
        return 0.0;
    }
    ig / si
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gini_pure_node_is_zero() {
        assert_eq!(gini(&[1.0, 0.0, 0.0]), 0.0);
        assert_eq!(gini(&[0.0, 1.0]), 0.0);
    }

    #[test]
    fn gini_balanced_binary_is_half() {
        assert!((gini(&[0.5, 0.5]) - 0.5).abs() < 1e-12);
    }

    #[test]
    fn gini_from_counts_matches_probs() {
        let counts = [3usize, 3];
        let probs = [0.5, 0.5];
        assert!((gini_from_counts(&counts, 6) - gini(&probs)).abs() < 1e-12);
    }

    #[test]
    fn gini_from_counts_zero_total_is_zero() {
        let counts = [0usize, 0];
        assert_eq!(gini_from_counts(&counts, 0), 0.0);
    }

    #[test]
    fn entropy_pure_node_is_zero() {
        assert_eq!(entropy_shannon(&[1.0, 0.0, 0.0]), 0.0);
    }

    #[test]
    fn entropy_balanced_binary_is_one() {
        assert!((entropy_shannon(&[0.5, 0.5]) - 1.0).abs() < 1e-12);
    }

    #[test]
    fn entropy_uniform_4way_is_two() {
        assert!((entropy_shannon(&[0.25, 0.25, 0.25, 0.25]) - 2.0).abs() < 1e-12);
    }

    #[test]
    fn info_gain_perfect_split_is_maximum() {
        let parent = [4usize, 4];
        let left = [4, 0];
        let right = [0, 4];
        let children: &[&[usize]] = &[&left, &right];
        let ig = info_gain_from_counts(&parent, children);
        assert!(ig > 3.9, "expected ~4.0, got {ig}");
    }

    #[test]
    fn info_gain_no_split_is_zero() {
        let parent = [4usize, 4];
        let children: &[&[usize]] = &[&[2, 2], &[2, 2]];
        let ig = info_gain_from_counts(&parent, children);
        assert!(ig.abs() < 1e-12);
    }

    #[test]
    fn split_info_balanced_split_is_one() {
        let left = [3usize, 0];
        let right = [0, 3];
        let children: &[&[usize]] = &[&left, &right];
        let si = split_info(children);
        assert!((si - 1.0).abs() < 1e-12);
    }

    #[test]
    fn split_info_skewed_split_is_low() {
        let left = [7usize, 0];
        let right = [0, 1];
        let children: &[&[usize]] = &[&left, &right];
        let si = split_info(children);
        // 7/8 log + 1/8 log = small
        assert!(si < 0.6, "expected < 0.6, got {si}");
    }

    #[test]
    fn gain_ratio_balanced_is_approximately_info_gain() {
        let parent = [4usize, 4];
        let left = [4, 0];
        let right = [0, 4];
        let children: &[&[usize]] = &[&left, &right];
        let gr = gain_ratio(&parent, children);
        let ig = info_gain_from_counts(&parent, children);
        assert!((gr - ig).abs() < 1e-9, "GR={gr} IG={ig}");
    }

    #[test]
    fn gain_ratio_skewed_is_smaller_than_info_gain() {
        // Balanced perfect split: GR ≈ IG.
        let balanced_parent = [4usize, 4];
        let balanced_children: &[&[usize]] = &[&[4, 0], &[0, 4]];
        let balanced_gr = gain_ratio(&balanced_parent, balanced_children);
        let balanced_ig = info_gain_from_counts(&balanced_parent, balanced_children);
        assert!(balanced_gr <= balanced_ig + 1e-9);

        // Skewed split: GR is much smaller than IG.
        let skewed_parent = [8usize, 8];
        let skewed_children: &[&[usize]] = &[&[8, 0], &[0, 7], &[0, 1]];
        let skewed_gr = gain_ratio(&skewed_parent, skewed_children);
        let skewed_ig = info_gain_from_counts(&skewed_parent, skewed_children);
        assert!(
            skewed_gr < skewed_ig,
            "skewed GR={skewed_gr} should be < IG={skewed_ig}"
        );
    }

    #[test]
    fn gain_ratio_zero_split_info_is_zero() {
        let parent = [4usize, 0];
        let children: &[&[usize]] = &[&[4, 0]];
        assert_eq!(gain_ratio(&parent, children), 0.0);
    }
}
