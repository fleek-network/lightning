use derive_more::Deref;

use crate::state::with_node;

/// An address for a node in the simulation. This can be dereferenced to the global
/// index of the node. Global index starts from `0`.
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd, Ord, Eq, Hash, Deref)]
pub struct RemoteAddr(pub(crate) usize);

/// List of all of the nodes in the simulation. The default instance of this type
/// includes all of the nodes in the current simulation.
#[derive(Debug, Clone, Copy)]
pub struct NodeArray {
    size: usize,
}

/// An iterator over a [`NodeArray`].
#[derive(Debug, Clone, Copy)]
pub struct NodeArrayIterator {
    current: usize,
    size: usize,
}

impl RemoteAddr {
    /// Returns the id of the current node.
    pub fn whoami() -> RemoteAddr {
        with_node(|n| RemoteAddr(n.node_id))
    }
}

impl From<RemoteAddr> for usize {
    fn from(value: RemoteAddr) -> Self {
        value.0
    }
}

impl NodeArray {
    /// Returns an array of every node in the simulation.
    pub fn new() -> Self {
        Self {
            size: with_node(|n| n.count_nodes),
        }
    }

    /// Returns the number of items.
    pub fn len(&self) -> usize {
        self.size
    }

    /// Returns true if there is no items.
    pub fn is_empty(&self) -> bool {
        self.size == 0
    }
}

impl Default for NodeArray {
    fn default() -> Self {
        Self::new()
    }
}

impl IntoIterator for NodeArray {
    type Item = RemoteAddr;

    type IntoIter = NodeArrayIterator;

    fn into_iter(self) -> Self::IntoIter {
        NodeArrayIterator {
            current: 0,
            size: self.size,
        }
    }
}

impl Iterator for NodeArrayIterator {
    type Item = RemoteAddr;

    fn next(&mut self) -> Option<Self::Item> {
        if self.current >= self.size {
            return None;
        }

        let current = self.current;
        self.current += 1;

        Some(RemoteAddr(current))
    }

    fn nth(&mut self, n: usize) -> Option<Self::Item> {
        self.current += n;
        self.next()
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        if self.current >= self.size {
            return (0, Some(0));
        }

        let n = self.size - self.current;
        (n, Some(n))
    }
}
