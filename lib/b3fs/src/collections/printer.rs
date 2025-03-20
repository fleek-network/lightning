use std::fmt::Debug;

use arrayvec::ArrayString;

use super::hash_tree::HashTree;

/// A hacky implementation of a tree printer for a hash tree. This can be improved a lot.
pub fn print(tree: &HashTree, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    let hashes = tree.as_inner();
    let size = tree.len();
    let mut stack = arrayvec::ArrayVec::<TreeNode, 64>::new();
    let mut iter = hashes.iter();

    for counter in 0..size {
        let hash = iter.next().unwrap();
        stack.push(TreeNode::Leaf(hash));

        let mut total_entries = counter + 1;
        while (total_entries & 1) == 0 {
            let right = stack.pop().unwrap();
            let left = stack.pop().unwrap();
            let hash = iter.next().unwrap();
            let node = TreeNode::Internal {
                hash,
                left: Box::new(left),
                right: Box::new(right),
            };
            stack.push(node);
            total_entries >>= 1;
        }
    }

    while stack.len() > 1 {
        let right = stack.pop().unwrap();
        let left = stack.pop().unwrap();
        let hash = iter.next().unwrap();
        let node = TreeNode::Internal {
            hash,
            left: Box::new(left),
            right: Box::new(right),
        };
        stack.push(node);
    }

    stack.pop().unwrap().fmt(f)
}

enum TreeNode<'t> {
    Internal {
        hash: &'t [u8; 32],
        left: Box<TreeNode<'t>>,
        right: Box<TreeNode<'t>>,
    },
    Leaf(&'t [u8; 32]),
}

impl Debug for TreeNode<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TreeNode::Internal { hash, left, right } => f
                .debug_tuple(&to_short_hex(hash))
                .field(left)
                .field(right)
                .finish(),
            TreeNode::Leaf(hash) => write!(f, "{}", to_short_hex(hash)),
        }
    }
}

fn to_short_hex(slice: &[u8; 32]) -> ArrayString<10> {
    let mut s = ArrayString::new();
    let table = b"0123456789abcdef";
    for &b in slice.iter().take(5) {
        s.push(table[(b >> 4) as usize] as char);
        s.push(table[(b & 0xf) as usize] as char);
    }
    s
}
