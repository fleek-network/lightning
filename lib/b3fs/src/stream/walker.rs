use std::cmp::Ordering;

use crate::utils::{previous_pow_of_two, tree_index};

/// Provides an iterator over a virtual tree of the provided length walking the tree towards
/// the specified target node.
///
/// The iteration happens from the root of the tree downward towards the target node. The [`Mode`]
/// allows you to configure the starting position of the iterator to skip the nodes that have
/// already been visited before.
pub struct TreeWalker {
    /// Index of the node we're looking for.
    target: usize,
    /// Where we're at right now.
    current: usize,
    /// Size of the current sub tree, which is the total number of
    /// leafs under the current node.
    subtree_size: usize,
}

/// The position of a element in an element in a binary tree.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direction {
    /// The element is the current root of the tree, it's neither on the
    /// left or right side.
    Target,
    /// The element is on the left side of the tree.
    Left,
    /// The element is on the right side of the tree.
    Right,
}

/// The visiting mode of the walker.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    /// Indicates that this is the first walk we're doing on this tree and no prior nodes have
    /// been visited. This will walk the tree all the way to the root.
    Initial,
    /// Indicates that there has been a walk prior to this call and that we have visited every
    /// node to the left. This will halt at the common ancesstor.
    Proceeding,
}

impl TreeWalker {
    /// Create a tree with the provided configurations.
    pub fn new(mode: Mode, target: usize, tree_len: usize) -> Self {
        match mode {
            Mode::Initial => Self::initial(target, tree_len),
            Mode::Proceeding => Self::proceeding(target, tree_len),
        }
    }

    /// Construct a new [`TreeWalker`] to walk a tree of `tree_len` items (in the array
    /// representation), looking for the provided `target`-th leaf.
    pub fn initial(target: usize, tree_len: usize) -> Self {
        if tree_len <= 1 {
            return Self::empty();
        }

        let walker = Self {
            target: tree_index(target),
            // Start the walk from the root of the full tree, which is the last item
            // in the array representation of the tree.
            current: tree_len - 1,
            // for `k` number of leaf nodes, the total nodes of the binary tree will
            // be `n = 2k - 1`, therefore for computing the number of leaf nodes given
            // the total number of all nodes, we can use the formula `k = ceil((n + 1) / 2)`
            // and we have `ceil(a / b) = floor((a + b - 1) / b)`.
            subtree_size: (tree_len + 2) / 2,
        };

        if walker.target > walker.current {
            return Self::empty();
        }

        walker
    }

    /// Construct a new [`TreeWalker`] to walk the tree assuming that a previous walk
    /// to the previous block has been made, and does not visit the nodes that the previous
    /// walker has visited.
    ///
    /// # Panics
    ///
    /// If target is zero. It doesn't make sense to call this function with target=zero since
    /// we don't have a -1 block that is already visited.
    pub fn proceeding(target: usize, tree_len: usize) -> Self {
        assert_ne!(target, 0, "Block zero has no previous blocks.");

        // Compute the index of the target in the tree representation.
        let target_index = tree_index(target);
        // If the target is not in this tree (out of bound) or the tree size is not
        // large enough for a resume walk return the empty iterator.
        if target_index >= tree_len || tree_len < 3 {
            return Self::empty();
        }

        let distance_to_ancestor = target.trailing_zeros();
        let subtree_size = (tree_len + 2) / 2;
        let subtree_size = (1 << distance_to_ancestor).min(subtree_size - target);
        let ancestor = target_index + (subtree_size << 1) - 2;

        if subtree_size <= 1 {
            return Self::empty();
        }

        debug_assert!(distance_to_ancestor >= 1);

        Self {
            target: target_index,
            current: ancestor,
            subtree_size,
        }
    }

    #[inline(always)]
    const fn empty() -> Self {
        Self {
            target: 0,
            current: 0,
            subtree_size: 0,
        }
    }

    /// Return the index of the target element in the array representation of the
    /// complete tree.
    pub fn tree_index(&self) -> usize {
        self.target
    }
}

impl Iterator for TreeWalker {
    type Item = (Direction, usize);

    #[inline(always)]
    fn next(&mut self) -> Option<Self::Item> {
        // If we are at a leaf node, we've already finished the traversal, and if the
        // target is greater than the current (which can only happen in the first iteration),
        // the target is already not in this tree anywhere.
        if self.subtree_size == 0 || self.target > self.current {
            return None;
        }

        if self.current == self.target {
            self.subtree_size = 0;
            return Some((Direction::Target, self.current));
        }

        // The left subtree in a blake3 tree is always guaranteed to contain a power of two
        // number of leaf (chunks), therefore the number of items on the left subtree can
        // be easily computed as the previous power of two (less than but not equal to)
        // the current items that we know our current subtree has, anything else goes
        // to the right subtree.
        let left_subtree_size = previous_pow_of_two(self.subtree_size);
        let right_subtree_size = self.subtree_size - left_subtree_size;
        // Use the formula `n = 2k - 1` to compute the total number of items on the
        // right side of this node, the index of the left node will be `n`-th item
        // before where we currently are at.
        let right_subtree_total_nodes = right_subtree_size * 2 - 1;
        let left = self.current - right_subtree_total_nodes - 1;
        let right = self.current - 1;

        match left.cmp(&self.target) {
            // The target is on the left side, so we need to prune the right subtree.
            Ordering::Equal | Ordering::Greater => {
                self.subtree_size = left_subtree_size;
                self.current = left;
                Some((Direction::Right, right))
            },
            // The target is on the right side, prune the left subtree.
            Ordering::Less => {
                self.subtree_size = right_subtree_size;
                self.current = right;
                Some((Direction::Left, left))
            },
        }
    }

    #[inline(always)]
    fn size_hint(&self) -> (usize, Option<usize>) {
        // If we're done iterating return 0.
        if self.subtree_size == 0 {
            return (0, Some(0));
        }

        // Return the upper bound as the result of the size estimation, the actual lower bound
        // can be computed more accurately but we don't really care about the accuracy of the
        // size estimate and the upper bound should be small enough for most use cases we have.
        //
        // This line is basically `ceil(log2(self.subtree_size)) + 1` which is the max depth of
        // the current subtree and one additional element + 1.
        let upper =
            usize::BITS as usize - self.subtree_size.saturating_sub(1).leading_zeros() as usize + 1;
        (upper, Some(upper))
    }
}

impl Mode {
    #[inline]
    pub const fn from_is_initial(is_initial: bool) -> Self {
        if is_initial {
            Mode::Initial
        } else {
            Mode::Proceeding
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn simple_walk() {
        assert_eq!(
            TreeWalker::initial(0, 13).collect::<Vec<_>>(),
            vec![
                (Direction::Right, 11),
                (Direction::Right, 5),
                (Direction::Right, 1),
                (Direction::Target, 0),
            ]
        );

        assert_eq!(
            TreeWalker::initial(0, 3).collect::<Vec<_>>(),
            vec![(Direction::Right, 1), (Direction::Target, 0),]
        );

        assert_eq!(
            TreeWalker::initial(1, 5).collect::<Vec<_>>(),
            vec![
                (Direction::Right, 3),
                (Direction::Left, 0),
                (Direction::Target, 1),
            ]
        );

        assert_eq!(TreeWalker::proceeding(1, 3).collect::<Vec<_>>(), vec![]);
        assert_eq!(TreeWalker::proceeding(2, 3).collect::<Vec<_>>(), vec![]);
    }
}
