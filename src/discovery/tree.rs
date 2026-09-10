//! Deterministic CART classifier with exact integer Gini arithmetic.
//!
//! This module depends only on `std`. It exists because the alternative engine
//! (`linfa-trees`) labels tied leaves through unordered-map iteration order and is therefore
//! not byte-stable across processes; this engine must instead be deterministic by construction
//! on every platform, so every split comparison uses exact integer arithmetic in `u128` rather
//! than floating-point Gini impurity.

/// A node in a fitted [`Tree`].
#[derive(Clone, Debug, PartialEq)]
pub enum Node {
    /// A terminal node predicting a single class.
    Leaf {
        /// The predicted class, chosen as the modal label of `rows` (ties resolve to the
        /// lowest label index).
        class: usize,
        /// Indices into the fitted matrix of the rows that reached this node, in ascending
        /// order.
        rows: Vec<usize>,
    },
    /// An internal node partitioning rows on a single column.
    Split {
        /// The column index the split tests.
        column: usize,
        /// Rows with `x[column] <= threshold` go left; all others go right.
        threshold: f64,
        /// The left child, holding rows with `x[column] <= threshold`.
        left: Box<Node>,
        /// The right child, holding rows with `x[column] > threshold`.
        right: Box<Node>,
        /// Indices into the fitted matrix of the rows that reached this node, in ascending
        /// order.
        rows: Vec<usize>,
    },
}

/// A fitted decision tree.
#[derive(Clone, Debug, PartialEq)]
pub struct Tree {
    /// The root node.
    pub root: Node,
}

impl Tree {
    /// Returns the number of splits on the longest root-to-leaf path. A root-only leaf has
    /// depth 0.
    pub fn depth(&self) -> usize {
        node_depth(&self.root)
    }
}

fn node_depth(node: &Node) -> usize {
    match node {
        Node::Leaf { .. } => 0,
        Node::Split { left, right, .. } => 1 + node_depth(left).max(node_depth(right)),
    }
}

/// Fits a deterministic CART classifier to `matrix`/`labels`.
///
/// `n_classes` is the number of distinct label values (labels are `0..n_classes`). `max_depth`
/// of `0` means unlimited depth; otherwise the root is depth `0` and `max_depth == 1` allows
/// exactly one split. `min_leaf` is the minimum row count a candidate split's child must have to
/// be valid.
///
/// Every row of `matrix` must have the same width. An empty `matrix` returns a lone
/// `Leaf { class: 0, rows: vec![] }`.
pub fn fit(matrix: &[Vec<f64>], labels: &[usize], n_classes: usize, max_depth: usize, min_leaf: usize) -> Tree {
    debug_assert_eq!(matrix.len(), labels.len());
    debug_assert!(matrix.windows(2).all(|w| w[0].len() == w[1].len()));

    if matrix.is_empty() {
        return Tree {
            root: Node::Leaf {
                class: 0,
                rows: Vec::new(),
            },
        };
    }

    let rows: Vec<usize> = (0..matrix.len()).collect();
    let root = build(matrix, labels, n_classes, max_depth, min_leaf, rows, 0);
    Tree { root }
}

/// Per-class row counts as `u128`, along with the purity numerator `A(R) = Σ_k n_k²` and size
/// `n(R) = |R|`.
struct Counts {
    counts: Vec<u128>,
    a: u128,
    n: u128,
}

impl Counts {
    fn from_rows(labels: &[usize], rows: &[usize], n_classes: usize) -> Self {
        let mut counts = vec![0u128; n_classes];
        for &r in rows {
            counts[labels[r]] += 1;
        }
        let a = counts.iter().map(|&c| c * c).sum();
        let n = rows.len() as u128;
        Counts { counts, a, n }
    }

    fn modal_class(&self) -> usize {
        let mut best_class = 0;
        let mut best_count = self.counts.first().copied().unwrap_or(0);
        for (class, &count) in self.counts.iter().enumerate().skip(1) {
            if count > best_count {
                best_count = count;
                best_class = class;
            }
        }
        best_class
    }

    fn is_pure(&self) -> bool {
        self.counts.iter().filter(|&&c| c > 0).count() <= 1
    }
}

fn build(
    matrix: &[Vec<f64>],
    labels: &[usize],
    n_classes: usize,
    max_depth: usize,
    min_leaf: usize,
    rows: Vec<usize>,
    depth: usize,
) -> Node {
    let parent = Counts::from_rows(labels, &rows, n_classes);

    if parent.is_pure() || (max_depth != 0 && depth == max_depth) {
        return Node::Leaf {
            class: parent.modal_class(),
            rows,
        };
    }

    match best_split(matrix, labels, n_classes, min_leaf, &rows, &parent) {
        Some((column, threshold, left_rows, right_rows)) => {
            let left = build(matrix, labels, n_classes, max_depth, min_leaf, left_rows, depth + 1);
            let right = build(matrix, labels, n_classes, max_depth, min_leaf, right_rows, depth + 1);
            Node::Split {
                column,
                threshold,
                left: Box::new(left),
                right: Box::new(right),
                rows,
            }
        }
        None => Node::Leaf {
            class: parent.modal_class(),
            rows,
        },
    }
}

/// Finds the best valid split for `rows`, if any strictly improves on the parent's purity.
/// Returns `(column, threshold, left_rows, right_rows)`.
#[allow(clippy::type_complexity)]
fn best_split(
    matrix: &[Vec<f64>],
    labels: &[usize],
    n_classes: usize,
    min_leaf: usize,
    rows: &[usize],
    parent: &Counts,
) -> Option<(usize, f64, Vec<usize>, Vec<usize>)> {
    let n_columns = matrix.first().map(|r| r.len()).unwrap_or(0);

    // Best candidate found so far: (a_left, n_left, a_right, n_right, column, threshold).
    let mut best: Option<(u128, u128, u128, u128, usize, f64)> = None;

    #[allow(clippy::needless_range_loop)]
    for column in 0..n_columns {
        let mut sorted: Vec<usize> = rows.to_vec();
        sorted.sort_by(|&a, &b| f64::total_cmp(&matrix[a][column], &matrix[b][column]));

        let mut left_counts = vec![0u128; n_classes];
        let mut left_a = 0u128;
        let mut left_n = 0u128;

        let mut i = 0;
        while i < sorted.len() {
            let value = matrix[sorted[i]][column];
            let mut j = i;
            while j < sorted.len() && matrix[sorted[j]][column] == value {
                let class = labels[sorted[j]];
                left_a += 2 * left_counts[class] + 1;
                left_counts[class] += 1;
                left_n += 1;
                j += 1;
            }

            // A threshold at `value` is a candidate only if there are rows remaining on the
            // right, i.e. this is not the largest distinct value.
            if j < sorted.len() {
                let right_n = parent.n - left_n;
                if left_n >= min_leaf as u128 && right_n >= min_leaf as u128 {
                    let right_a = right_a_from_counts(&left_counts, parent);
                    let candidate = (left_a, left_n, right_a, right_n, column, value);
                    best = Some(match best {
                        None => candidate,
                        Some(current) => {
                            if beats(&candidate, &current) {
                                candidate
                            } else {
                                current
                            }
                        }
                    });
                }
            }

            i = j;
        }
    }

    let (al, nl, ar, nr, column, threshold) = best?;

    // Apply only if it strictly beats the parent.
    let improves = (al * nr + ar * nl) * parent.n > parent.a * (nl * nr);
    if !improves {
        return None;
    }

    let mut left_rows = Vec::new();
    let mut right_rows = Vec::new();
    for &r in rows {
        if matrix[r][column] <= threshold {
            left_rows.push(r);
        } else {
            right_rows.push(r);
        }
    }
    Some((column, threshold, left_rows, right_rows))
}

/// Given the running left-side per-class counts, computes `A(right) = Σ_k (n_k - left_k)²`.
fn right_a_from_counts(left_counts: &[u128], parent: &Counts) -> u128 {
    parent
        .counts
        .iter()
        .zip(left_counts.iter())
        .map(|(&n_k, &l_k)| {
            let r_k = n_k - l_k;
            r_k * r_k
        })
        .sum()
}

/// Compares two candidates `(a_left, n_left, a_right, n_right, column, threshold)`: does
/// `candidate` strictly beat `current`?
fn beats(candidate: &(u128, u128, u128, u128, usize, f64), current: &(u128, u128, u128, u128, usize, f64)) -> bool {
    let (al, nl, ar, nr, _, _) = *candidate;
    let (bl, ml, br, mr, _, _) = *current;
    (al * nr + ar * nl) * (ml * mr) > (bl * mr + br * ml) * (nl * nr)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_the_best_split() {
        let matrix = vec![vec![0.0, 5.0], vec![1.0, 5.0], vec![2.0, 5.0], vec![3.0, 5.0]];
        let labels = vec![0, 0, 1, 1];
        let tree = fit(&matrix, &labels, 2, 0, 1);
        match tree.root {
            Node::Split {
                column,
                threshold,
                left,
                right,
                ..
            } => {
                assert_eq!(column, 0);
                assert_eq!(threshold, 1.0);
                match *left {
                    Node::Leaf { class, rows } => {
                        assert_eq!(class, 0);
                        assert_eq!(rows, vec![0, 1]);
                    }
                    _ => panic!("expected leaf"),
                }
                match *right {
                    Node::Leaf { class, rows } => {
                        assert_eq!(class, 1);
                        assert_eq!(rows, vec![2, 3]);
                    }
                    _ => panic!("expected leaf"),
                }
            }
            _ => panic!("expected split"),
        }
    }

    #[test]
    fn ties_break_to_the_lower_column() {
        let matrix = vec![vec![0.0, 0.0], vec![1.0, 1.0], vec![2.0, 2.0], vec![3.0, 3.0]];
        let labels = vec![0, 0, 1, 1];
        let tree = fit(&matrix, &labels, 2, 0, 1);
        match tree.root {
            Node::Split { column, threshold, .. } => {
                assert_eq!(column, 0);
                assert_eq!(threshold, 1.0);
            }
            _ => panic!("expected split"),
        }
    }

    #[test]
    fn min_leaf_blocks_and_allows_splits() {
        let matrix = vec![vec![0.0, 0.0], vec![1.0, 1.0], vec![2.0, 2.0], vec![3.0, 3.0]];
        let labels = vec![0, 0, 1, 1];

        let blocked = fit(&matrix, &labels, 2, 0, 3);
        match blocked.root {
            Node::Leaf { .. } => {}
            _ => panic!("expected leaf"),
        }

        let allowed = fit(&matrix, &labels, 2, 0, 2);
        match allowed.root {
            Node::Split { threshold, .. } => assert_eq!(threshold, 1.0),
            _ => panic!("expected split"),
        }
    }

    #[test]
    fn pure_nodes_stop() {
        let matrix = vec![vec![0.0], vec![1.0], vec![2.0], vec![3.0]];
        let labels = vec![0, 0, 0, 0];
        let tree = fit(&matrix, &labels, 1, 0, 1);
        match tree.root {
            Node::Leaf { class, .. } => assert_eq!(class, 0),
            _ => panic!("expected leaf"),
        }
    }

    #[test]
    fn max_depth_limits_and_zero_is_unlimited() {
        let matrix = vec![vec![0.0], vec![1.0], vec![2.0], vec![3.0], vec![4.0], vec![5.0]];
        let labels = vec![0, 0, 1, 1, 2, 2];

        let limited = fit(&matrix, &labels, 3, 1, 1);
        match limited.root {
            Node::Split {
                threshold, left, right, ..
            } => {
                assert_eq!(threshold, 1.0);
                match *left {
                    Node::Leaf { class, .. } => assert_eq!(class, 0),
                    _ => panic!("expected leaf"),
                }
                match *right {
                    Node::Leaf { class, rows } => {
                        assert_eq!(class, 1);
                        assert_eq!(rows, vec![2, 3, 4, 5]);
                    }
                    _ => panic!("expected leaf"),
                }
            }
            _ => panic!("expected split"),
        }

        let unlimited = fit(&matrix, &labels, 3, 0, 1);
        assert_eq!(unlimited.depth(), 2);
        match unlimited.root {
            Node::Split {
                threshold, left, right, ..
            } => {
                assert_eq!(threshold, 1.0);
                match *left {
                    Node::Leaf { class, rows } => {
                        assert_eq!(class, 0);
                        assert_eq!(rows, vec![0, 1]);
                    }
                    _ => panic!("expected leaf"),
                }
                match *right {
                    Node::Split {
                        threshold, left, right, ..
                    } => {
                        assert_eq!(threshold, 3.0);
                        match *left {
                            Node::Leaf { class, rows } => {
                                assert_eq!(class, 1);
                                assert_eq!(rows, vec![2, 3]);
                            }
                            _ => panic!("expected leaf"),
                        }
                        match *right {
                            Node::Leaf { class, rows } => {
                                assert_eq!(class, 2);
                                assert_eq!(rows, vec![4, 5]);
                            }
                            _ => panic!("expected leaf"),
                        }
                    }
                    _ => panic!("expected split"),
                }
            }
            _ => panic!("expected split"),
        }
    }

    #[test]
    fn fit_twice_is_identical() {
        let matrix = vec![vec![0.0], vec![1.0], vec![2.0], vec![3.0], vec![4.0], vec![5.0]];
        let labels = vec![0, 0, 1, 1, 2, 2];
        assert_eq!(fit(&matrix, &labels, 3, 0, 1), fit(&matrix, &labels, 3, 0, 1));
    }
}
