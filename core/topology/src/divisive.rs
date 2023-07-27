use std::{collections::BTreeMap, fmt::Display};

use ndarray::Array2;
use rand::Rng;
use serde::Serialize;

use crate::{clustering::constrained_fasterpam, pairing::greedy_pairs};

/// A divisive hierarchy strategy
#[derive(Debug, Clone, Serialize)]
pub enum DivisiveHierarchy {
    Group {
        id: String,
        total: usize,
        children: Vec<DivisiveHierarchy>,
        nodes: Vec<Node>,
    },
    Cluster {
        id: String,
        nodes: Vec<Node>,
    },
}

#[derive(Debug, Clone, Serialize)]
pub struct Node {
    id: usize,
    connections: BTreeMap<usize, Vec<usize>>,
}

#[derive(Debug, Clone)]
pub struct HierarchyPath(Vec<u8>);

impl HierarchyPath {
    pub fn root() -> Self {
        Self(vec![])
    }
    pub fn depth(&self) -> usize {
        self.0.len()
    }
}

impl Display for HierarchyPath {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let strings: Vec<_> = self.0.iter().map(|v| v.to_string()).collect();
        if strings.is_empty() {
            write!(f, "root")
        } else {
            write!(f, "{}", strings.join("."))
        }
    }
}

fn add_connection(indeces: &mut [Node], depth: usize, i: usize, j: usize) {
    let jid = indeces[j].id;
    let left = &mut indeces[i];
    left.connections.entry(depth).or_default().push(jid);
    let iid = left.id;
    let right = &mut indeces[j];
    right.connections.entry(depth).or_default().push(iid);
}

impl DivisiveHierarchy {
    /// Create a new divisive hierarchy using constrained fasterpam and selecting random medoids.
    /// The algorithm divides the nodes into k "superclusters", aka "groups", until it cannot
    /// anymore, and finally divides the last superclusters into an optimal number of final
    /// clusters with k nodes in them.
    pub fn new<R: Rng>(rng: &mut R, dissim_matrix: &Array2<i32>, k: usize) -> Self {
        let nodes: Vec<_> = (0..dissim_matrix.nrows())
            .map(|i| Node {
                id: i,
                connections: BTreeMap::new(),
            })
            .collect();

        Self::new_inner(rng, dissim_matrix, nodes, &HierarchyPath::root(), k)
    }

    fn new_inner<R: Rng>(
        rng: &mut R,
        dissim_matrix: &Array2<i32>,
        mut indeces: Vec<Node>,
        current_path: &HierarchyPath,
        k: usize,
    ) -> Self {
        // calculate the number of clusters
        let depth = current_path.depth();
        let count = indeces.len() / k;
        if count <= 1 {
            // return base cluster
            let ids: Vec<_> = indeces.iter().map(|n| n.id).collect();

            // add connections to every other node in the cluster for each node in the cluster
            for node in indeces.iter_mut() {
                let conns = node.connections.entry(depth).or_default();
                for id in &ids {
                    if id != &node.id {
                        conns.push(*id);
                    }
                }
            }

            Self::Cluster {
                id: current_path.to_string(),
                // collect the top level indeces for the nodes
                nodes: indeces,
            }
        } else {
            // build children
            let (n_clusters, min, max) = if count < k {
                let t = indeces.len() / count;
                (count, t - 1, t + 1)
            } else {
                (k, count - 1, count + 1)
            };

            // find n medoids
            let mut medoids =
                rand::seq::index::sample(rng, dissim_matrix.nrows(), n_clusters).into_vec();

            // find n clusters
            let (_, assignments, _, _) =
                constrained_fasterpam::<_, i32>(dissim_matrix, &mut medoids, 100, min, max);

            let mut clusters: BTreeMap<usize, Vec<usize>> = BTreeMap::new();
            for (node, &assignment) in assignments.iter().enumerate() {
                clusters.entry(assignment).or_default().push(node);
            }

            // greedily pair nodes together
            for a in 0..clusters.len() {
                for b in a + 1..clusters.len() {
                    let pairs = greedy_pairs(dissim_matrix, &clusters[&a], &clusters[&b]);
                    for (i, j) in pairs {
                        add_connection(&mut indeces, depth, i, j);
                    }
                }
            }

            // recurse children
            let mut children = Vec::with_capacity(n_clusters);
            for (path_index, new_indeces) in clusters.values().enumerate() {
                // build new matrix from medoids
                let mut child_matrix = Array2::zeros((new_indeces.len(), new_indeces.len()));

                for (i, &iidx) in new_indeces.iter().enumerate() {
                    for (mut j, &jidx) in new_indeces[i + 1..].iter().enumerate() {
                        j += i + 1;
                        let dissim = dissim_matrix[(iidx, jidx)];
                        child_matrix[(i, j)] = dissim;
                        child_matrix[(j, i)] = dissim;
                    }
                }

                // create a child with the new matrix and indeces
                let mut path = current_path.clone();
                path.0.push(path_index as u8);
                let nodes: Vec<_> = new_indeces.iter().map(|&i| indeces[i].clone()).collect();
                let child = Self::new_inner(rng, &child_matrix, nodes, &path, k);
                children.push(child);
            }

            Self::Group {
                id: current_path.to_string(),
                total: indeces.len(),
                nodes: indeces,
                children,
            }
        }
    }

    /// Get the total number of nodes in the hierarchy
    pub fn n_nodes(&self) -> usize {
        match self {
            Self::Group { total, .. } => *total,
            Self::Cluster { nodes, .. } => nodes.len(),
        }
    }

    /// Collect assignments for each node at each depth of the hierarchy. The last vec of
    /// assignments is the final tree depth.
    pub fn assignments(&self) -> Vec<Vec<usize>> {
        fn inner(
            item: &DivisiveHierarchy,
            data: &mut BTreeMap<usize, (usize, Vec<usize>)>,
            depth: usize,
            total: usize,
        ) {
            let (counter, assignments) = data.entry(depth).or_insert((0, vec![0; total]));
            let current = *counter;
            *counter += 1;
            match item {
                DivisiveHierarchy::Group {
                    children, nodes, ..
                } => {
                    // set assignments
                    for node in nodes {
                        assignments[node.id] = current;
                    }
                    // recurse for each child item
                    for child in children {
                        inner(child, data, depth + 1, total);
                    }
                },
                DivisiveHierarchy::Cluster { nodes, .. } => {
                    for node in nodes {
                        assignments[node.id] = current;
                    }
                },
            }
        }

        let mut data = BTreeMap::new();
        let total = self.n_nodes();
        inner(self, &mut data, 0, total);
        data.into_values().map(|v| v.1).collect()
    }
}
