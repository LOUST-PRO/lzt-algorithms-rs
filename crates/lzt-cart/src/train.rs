//! CART training: Gini-based best-split search + recursive tree build.

use lzt_entropy::gini_from_counts;

use crate::{
    Label, Sample, Tree, TreeNode, MAX_DEPTH, MAX_FEATURES, MAX_NODES, MAX_SAMPLES,
    MIN_SAMPLES_SPLIT, NUM_CLASSES,
};

/// Train a CART decision tree on the given samples.
///
/// Returns a [`Tree`] with at most `MAX_NODES` nodes and `MAX_DEPTH` deep.
/// The training set is fit-in-memory; for streaming use, port to your own
/// scorer.
pub fn train(samples: &[Sample]) -> Tree {
    debug_assert!(samples.len() <= MAX_SAMPLES, "max {} samples", MAX_SAMPLES);
    let mut tree = Tree { nodes: Vec::with_capacity(MAX_NODES) };
    let indices: Vec<usize> = (0..samples.len()).collect();
    build_node(samples, &indices, 0, &mut tree);
    tree
}

/// Per-node training routine. Recursive.
///
/// Splits the samples at the index set `indices` using the best (feature,
/// threshold) pair by Gini gain, recursing into the two children until
/// one of the stopping conditions is hit (pure node, too few samples,
/// max depth reached, no good split).
fn build_node(samples: &[Sample], indices: &[usize], depth: usize, tree: &mut Tree) {
    if tree.nodes.len() >= MAX_NODES {
        return;
    }

    // Empty sample set: nothing to push. The caller (or `predict`) will
    // treat the empty tree as "default to Review" via `Label::Review`.
    if indices.is_empty() {
        return;
    }

    let class_counts = tally(samples, indices);
    let impurity = gini_from_counts(&class_counts, indices.len());
    let majority = majority_label(&class_counts);

    let pure = class_counts.iter().all(|&c| c == 0) || indices.len() <= MIN_SAMPLES_SPLIT;
    let too_deep = depth >= MAX_DEPTH;

    if pure || too_deep {
        let node = TreeNode::leaf(majority, indices.len(), impurity);
        tree.nodes.push(node);
        return;
    }

    let Some((feature, threshold, gain)) = find_best_split(samples, indices) else {
        let node = TreeNode::leaf(majority, indices.len(), impurity);
        tree.nodes.push(node);
        return;
    };

    if gain <= 0.0 {
        let node = TreeNode::leaf(majority, indices.len(), impurity);
        tree.nodes.push(node);
        return;
    }

    let (left_idx, right_idx): (Vec<usize>, Vec<usize>) = indices
        .iter()
        .copied()
        .partition(|&i| samples[i].values[feature] <= threshold);

    let this_idx = tree.nodes.len();
    // Reserve a placeholder; we'll fill it after the children are added so
    // child indices are available.
    tree.nodes.push(TreeNode::leaf(majority, indices.len(), impurity));

    let left_child = tree.nodes.len();
    build_node(samples, &left_idx, depth + 1, tree);
    let right_child = tree.nodes.len();
    build_node(samples, &right_idx, depth + 1, tree);

    tree.nodes[this_idx] = TreeNode::split(
        feature,
        threshold,
        left_child,
        right_child,
        majority,
        indices.len(),
        impurity,
    );
}

/// Tally per-class counts across the given sample indices.
fn tally(samples: &[Sample], indices: &[usize]) -> [usize; NUM_CLASSES] {
    let mut counts = [0usize; NUM_CLASSES];
    for &i in indices {
        counts[samples[i].label.as_usize()] += 1;
    }
    counts
}

/// Pick the most frequent label in the counts.
fn majority_label(counts: &[usize; NUM_CLASSES]) -> Label {
    let mut best = 0usize;
    for i in 1..NUM_CLASSES {
        if counts[i] > counts[best] {
            best = i;
        }
    }
    Label::from_usize(best)
}

/// Find the best (feature, threshold) pair by Gini gain.
///
/// Returns `(feature_idx, threshold, gain)`. `gain <= 0.0` means no useful
/// split was found (the caller should turn the node into a leaf).
fn find_best_split(samples: &[Sample], indices: &[usize]) -> Option<(usize, f64, f64)> {
    let parent_counts = tally(samples, indices);
    let n = indices.len();
    if n < 2 {
        return None;
    }
    let parent_gini = gini_from_counts(&parent_counts, n);

    let mut best: Option<(usize, f64, f64)> = None;

    for feature in 0..MAX_FEATURES {
        // Sort the indices by this feature's value once.
        let mut sorted: Vec<usize> = indices.to_vec();
        sorted.sort_by(|&a, &b| {
            samples[a].values[feature]
                .partial_cmp(&samples[b].values[feature])
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        // Walk the sorted list, considering a split between each adjacent
        // pair where the value differs.
        let mut left_counts = [0usize; NUM_CLASSES];
        for w in 0..sorted.len().saturating_sub(1) {
            let idx = sorted[w];
            left_counts[samples[idx].label.as_usize()] += 1;
            let v_cur = samples[idx].values[feature];
            let v_next = samples[sorted[w + 1]].values[feature];
            if (v_cur - v_next).abs() <= f64::EPSILON {
                continue;
            }
            let threshold = (v_cur + v_next) * 0.5;
            let left_n = w + 1;
            let right_n = n - left_n;
            if left_n == 0 || right_n == 0 {
                continue;
            }
            let mut right_counts = [0usize; NUM_CLASSES];
            for c in 0..NUM_CLASSES {
                right_counts[c] = parent_counts[c] - left_counts[c];
            }
            let gini_left = gini_from_counts(&left_counts, left_n);
            let gini_right = gini_from_counts(&right_counts, right_n);
            let weighted =
                (left_n as f64) * gini_left + (right_n as f64) * gini_right;
            let gain = parent_gini * (n as f64) - weighted;
            let candidate = (feature, threshold, gain / (n as f64));
            best = match best {
                Some((bf, bt, bg)) if bg >= candidate.2 => Some((bf, bt, bg)),
                _ => Some(candidate),
            };
        }
    }

    best
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture() -> Vec<Sample> {
        vec![
            Sample::new([0.10, 0.20, 0.30, 0.40, 0.50, 0.60, 0.70, 0.80], Label::Low),
            Sample::new([0.15, 0.18, 0.25, 0.45, 0.55, 0.62, 0.68, 0.78], Label::Low),
            Sample::new([0.50, 0.45, 0.55, 0.50, 0.45, 0.55, 0.50, 0.45], Label::Review),
            Sample::new([0.55, 0.50, 0.50, 0.45, 0.50, 0.50, 0.55, 0.50], Label::Review),
            Sample::new([0.90, 0.85, 0.95, 0.80, 0.90, 0.85, 0.95, 0.88], Label::Urgent),
            Sample::new([0.85, 0.90, 0.92, 0.85, 0.88, 0.92, 0.90, 0.85], Label::Urgent),
        ]
    }

    #[test]
    fn train_produces_nonempty_tree() {
        let tree = train(&fixture());
        assert!(!tree.is_empty());
        assert!(tree.len() <= MAX_NODES);
    }

    #[test]
    fn train_recovers_training_set() {
        let samples = fixture();
        let tree = train(&samples);
        for s in &samples {
            let pred = tree.predict(&s.values);
            assert_eq!(
                pred, s.label,
                "training sample {:?} misclassified as {:?}",
                s.values, pred
            );
        }
    }

    #[test]
    fn train_with_single_sample_produces_leaf() {
        let samples = vec![Sample::new([0.5; MAX_FEATURES], Label::Review)];
        let tree = train(&samples);
        assert_eq!(tree.len(), 1);
        assert!(tree.nodes[0].is_leaf);
        assert_eq!(tree.nodes[0].label, Label::Review);
    }

    #[test]
    fn train_with_empty_samples_produces_empty_tree() {
        let tree = train(&[]);
        assert!(tree.is_empty());
    }

    #[test]
    fn split_node_references_real_children() {
        let tree = train(&fixture());
        for (i, n) in tree.nodes.iter().enumerate() {
            if n.is_leaf {
                continue;
            }
            let l = n.left.expect("split has left child");
            let r = n.right.expect("split has right child");
            assert!(l < tree.nodes.len(), "node {i} left child {l} out of range");
            assert!(r < tree.nodes.len(), "node {i} right child {r} out of range");
        }
    }

    #[test]
    fn finding_best_split_on_separable_data_picks_feature_zero() {
        // Feature 0 cleanly separates the two classes.
        let samples = vec![
            Sample::new([0.10, 0.50, 0.50, 0.50, 0.50, 0.50, 0.50, 0.50], Label::Low),
            Sample::new([0.20, 0.50, 0.50, 0.50, 0.50, 0.50, 0.50, 0.50], Label::Low),
            Sample::new([0.80, 0.50, 0.50, 0.50, 0.50, 0.50, 0.50, 0.50], Label::Urgent),
            Sample::new([0.90, 0.50, 0.50, 0.50, 0.50, 0.50, 0.50, 0.50], Label::Urgent),
        ];
        let tree = train(&samples);
        assert!(!tree.is_empty());
        let root = &tree.nodes[0];
        if !root.is_leaf {
            assert_eq!(root.feature, Some(0));
        }
    }
}
