//! Typed CART decision tree classifier.
//!
//! This is a Rust port of `cart_classifier.c` from the HexCortex research
//! pipeline. The model is a classic CART binary tree with Gini impurity and
//! majority-label leaves. Sample type is fixed-size for no-heap storage;
//! tree nodes are stored in a `Vec` for ease of inspection.
//!
//! # Example
//!
//! ```
//! use lzt_cart::{Sample, Label, Tree, train};
//!
//! let samples = vec![
//!     Sample::new([0.10, 0.20, 0.30, 0.40, 0.50, 0.60, 0.70, 0.80], Label::Low),
//!     Sample::new([0.90, 0.85, 0.95, 0.80, 0.90, 0.85, 0.95, 0.88], Label::Urgent),
//!     Sample::new([0.50, 0.45, 0.55, 0.50, 0.45, 0.55, 0.50, 0.45], Label::Review),
//! ];
//!
//! let tree = train(&samples);
//! let pred = tree.predict(&[0.10, 0.20, 0.30, 0.40, 0.50, 0.60, 0.70, 0.80]);
//! assert_eq!(pred, Label::Low);
//! ```

#![forbid(unsafe_code)]
#![warn(missing_docs)]

mod train;

pub use train::train;

/// Maximum number of features per sample. Matches the C99 reference.
pub const MAX_FEATURES: usize = 8;

/// Maximum number of samples in the training set. Matches the C99 reference.
pub const MAX_SAMPLES: usize = 64;

/// Maximum number of nodes in the fitted tree. Matches the C99 reference.
pub const MAX_NODES: usize = 31;

/// Maximum depth of the fitted tree. Matches the C99 reference.
pub const MAX_DEPTH: usize = 5;

/// Minimum samples required to attempt a split. Matches the C99 reference.
pub const MIN_SAMPLES_SPLIT: usize = 2;

/// Number of distinct classes in the taxonomy.
pub const NUM_CLASSES: usize = 3;

/// The three taxonomía categories used by the HexCortex reference tree.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Label {
    /// Low-priority signal (`label=0` in the C reference).
    Low = 0,
    /// Review signal — needs human triage (`label=1`).
    Review = 1,
    /// Urgent signal — needs immediate intervention (`label=2`).
    Urgent = 2,
}

impl Label {
    /// Numeric label (`0`, `1`, `2`) matching the C reference.
    pub fn as_usize(self) -> usize {
        self as usize
    }

    /// Human-readable short name (used in test assertions and log lines).
    pub fn name(self) -> &'static str {
        match self {
            Label::Low => "low",
            Label::Review => "review",
            Label::Urgent => "urgent",
        }
    }

    /// Convert a numeric label back to the enum. Panics on unknown values.
    pub fn from_usize(v: usize) -> Self {
        match v {
            0 => Label::Low,
            1 => Label::Review,
            2 => Label::Urgent,
            other => panic!("unknown label index: {other}"),
        }
    }
}

/// One training sample: fixed-length feature vector + class label.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Sample {
    /// The feature values. Indexed `[0..MAX_FEATURES)`.
    pub values: [f64; MAX_FEATURES],
    /// The class label for this sample.
    pub label: Label,
}

impl Sample {
    /// Convenience constructor.
    pub fn new(values: [f64; MAX_FEATURES], label: Label) -> Self {
        Self { values, label }
    }
}

/// A single node in the CART tree. Either a decision node (split) or a leaf.
#[derive(Debug, Clone, PartialEq)]
pub struct TreeNode {
    /// Whether this is a leaf.
    pub is_leaf: bool,
    /// Feature index to test at this node (`None` on leaves).
    pub feature: Option<usize>,
    /// Threshold value for the split (`f64`).
    pub threshold: f64,
    /// Index of the left child (`values[feature] <= threshold`).
    pub left: Option<usize>,
    /// Index of the right child (`values[feature] > threshold`).
    pub right: Option<usize>,
    /// Class label (populated for leaves, also used as fallback).
    pub label: Label,
    /// Number of training samples that reached this node.
    pub samples: usize,
    /// Gini impurity at this node (0 for pure leaves).
    pub impurity: f64,
}

impl TreeNode {
    /// Leaf node constructor.
    pub fn leaf(label: Label, samples: usize, impurity: f64) -> Self {
        Self {
            is_leaf: true,
            feature: None,
            threshold: 0.0,
            left: None,
            right: None,
            label,
            samples,
            impurity,
        }
    }

    /// Decision node constructor.
    pub fn split(
        feature: usize,
        threshold: f64,
        left: usize,
        right: usize,
        label: Label,
        samples: usize,
        impurity: f64,
    ) -> Self {
        Self {
            is_leaf: false,
            feature: Some(feature),
            threshold,
            left: Some(left),
            right: Some(right),
            label,
            samples,
            impurity,
        }
    }
}

/// A fitted CART tree. Stored as a `Vec<TreeNode>` for easy inspection.
///
/// The tree is binary: `nodes[0]` is the root; interior nodes reference
/// their children by index. The tree is at most `MAX_NODES=31` nodes deep
/// (5 levels + leaves).
#[derive(Debug, Clone, PartialEq)]
pub struct Tree {
    /// The nodes of the tree, in insertion order.
    pub nodes: Vec<TreeNode>,
}

impl Tree {
    /// Predict the class label for a single feature vector.
    ///
    /// Walks the tree from root to leaf, choosing the right child when
    /// `values[feature] > threshold` and the left child otherwise.
    pub fn predict(&self, values: &[f64; MAX_FEATURES]) -> Label {
        if self.nodes.is_empty() {
            return Label::Review;
        }
        let mut idx = 0usize;
        for _ in 0..MAX_NODES {
            let node = match self.nodes.get(idx) {
                Some(n) => n,
                None => return Label::Review,
            };
            if node.is_leaf {
                return node.label;
            }
            let feature = node.feature.expect("non-leaf has feature");
            let next = match node.left.zip(node.right) {
                Some((l, r)) => {
                    if values[feature] <= node.threshold {
                        l
                    } else {
                        r
                    }
                }
                None => return node.label,
            };
            idx = next;
        }
        // Traversal exceeded MAX_NODES — return the last visited node's label.
        self.nodes[idx.min(self.nodes.len() - 1)].label
    }

    /// Number of nodes in the tree.
    pub fn len(&self) -> usize {
        self.nodes.len()
    }

    /// Whether the tree is empty.
    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn label_roundtrip() {
        for (i, name) in [(0usize, "low"), (1usize, "review"), (2usize, "urgent")] {
            let l = Label::from_usize(i);
            assert_eq!(l.as_usize(), i);
            assert_eq!(l.name(), name);
        }
    }

    #[test]
    fn leaf_node_construction() {
        let n = TreeNode::leaf(Label::Urgent, 12, 0.0);
        assert!(n.is_leaf);
        assert_eq!(n.label, Label::Urgent);
        assert_eq!(n.samples, 12);
    }

    #[test]
    fn split_node_construction() {
        let n = TreeNode::split(3, 0.55, 1, 2, Label::Review, 8, 0.42);
        assert!(!n.is_leaf);
        assert_eq!(n.feature, Some(3));
        assert_eq!(n.threshold, 0.55);
    }

    #[test]
    fn empty_tree_predicts_review() {
        let tree = Tree { nodes: vec![] };
        assert_eq!(tree.predict(&[0.0; MAX_FEATURES]), Label::Review);
    }

    #[test]
    fn single_leaf_tree_predicts_constant() {
        let tree = Tree {
            nodes: vec![TreeNode::leaf(Label::Urgent, 5, 0.0)],
        };
        assert_eq!(
            tree.predict(&[0.5; MAX_FEATURES]),
            Label::Urgent,
        );
    }
}
