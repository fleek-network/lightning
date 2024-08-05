//! Novel constrained fasterpam (k-medoids) implementation for clustering.
//!
//! This file has largely been adapted from the [kmedoids](https://github.com/kno10/rust-kmedoids)
//! crate, which provides a fasterpam implementation based off the ENKI codebase. The original code
//! is licensed under GNU GPLv3 and can be found [here](https://github.com/kno10/rust-kmedoids/blob/main/LICENSE).
//!
//! The fasterpam implementation has been adapted to include a constraint strategy. This is done
//! by building and solving a min cost flow graph similar to [this](https://github.com/joshlk/k-means-constrained)
//! reference implementation of constrained k-means.

use core::ops::AddAssign;
use std::collections::BTreeMap;
use std::convert::From;

use mcmf::{Capacity, Cost, GraphBuilder, Vertex};
use num_traits::{Signed, Zero};

/// Adapter trait for accessing different types of arrays
#[allow(clippy::len_without_is_empty)]
pub trait ArrayAdapter<N> {
    /// Get the length of an array structure
    fn len(&self) -> usize;
    /// Verify that it is a square matrix
    fn is_square(&self) -> bool;
    /// Get the contents at cell x,y
    fn get(&self, x: usize, y: usize) -> N;
}

/// Adapter trait for using `ndarray::Array2` and similar
impl<A, N> ArrayAdapter<N> for ndarray::ArrayBase<A, ndarray::Ix2>
where
    A: ndarray::Data<Elem = N>,
    N: Copy,
{
    #[inline]
    fn len(&self) -> usize {
        self.shape()[0]
    }
    #[inline]
    fn is_square(&self) -> bool {
        self.shape()[0] == self.shape()[1]
    }
    #[inline]
    fn get(&self, x: usize, y: usize) -> N {
        self[[x, y]]
    }
}

#[derive(Debug, Clone)]
pub struct LowerTriangle<N> {
    /// Matrix size
    pub n: usize,
    // Matrix data, lower triangular form without diagonal
    pub data: Vec<N>,
}
/// Adapter implementation for LowerTriangle
impl<N: Copy + num_traits::Zero> ArrayAdapter<N> for LowerTriangle<N> {
    #[inline]
    fn len(&self) -> usize {
        self.n
    }
    #[inline]
    fn is_square(&self) -> bool {
        self.data.len() == (self.n * (self.n - 1)) >> 1
    }
    #[inline]
    fn get(&self, x: usize, y: usize) -> N {
        match x.cmp(&y) {
            std::cmp::Ordering::Less => self.data[((y * (y - 1)) >> 1) + x],
            std::cmp::Ordering::Greater => self.data[((x * (x - 1)) >> 1) + y],
            std::cmp::Ordering::Equal => N::zero(),
        }
    }
}

/// Object id and distance pair
#[derive(Debug, Copy, Clone)]
pub(crate) struct DistancePair<N> {
    pub(crate) i: u32,
    pub(crate) d: N,
}
impl<N> DistancePair<N> {
    pub(crate) fn new(i: u32, d: N) -> Self {
        DistancePair { i, d }
    }
}
impl<N: Zero> DistancePair<N> {
    pub(crate) fn empty() -> Self {
        DistancePair {
            i: u32::MAX,
            d: N::zero(),
        }
    }
}

/// Information kept for each point: two such pairs
#[derive(Debug, Copy, Clone)]
pub(crate) struct Rec<N> {
    pub(crate) near: DistancePair<N>,
    pub(crate) seco: DistancePair<N>,
}
impl<N> Rec<N> {
    pub(crate) fn new(i1: u32, d1: N, i2: u32, d2: N) -> Rec<N> {
        Rec {
            near: DistancePair { i: i1, d: d1 },
            seco: DistancePair { i: i2, d: d2 },
        }
    }
}
impl<N: Zero> Rec<N> {
    pub(crate) fn empty() -> Self {
        Rec {
            near: DistancePair::empty(),
            seco: DistancePair::empty(),
        }
    }
}

/// Find the minimum (index and value)
#[inline]
pub(crate) fn find_min<'a, L, I>(a: &mut I) -> (usize, L)
where
    L: 'a + PartialOrd + Copy + Zero,
    I: 'a + std::iter::Iterator<Item = &'a L>,
{
    let mut a = a.enumerate();
    let mut best: (usize, L) = (0, *a.next().unwrap().1);
    for (ik, iv) in a {
        if *iv < best.1 {
            best = (ik, *iv);
        }
    }
    best
}

/// Debug helper function
#[cfg(debug_assertions)]
pub(crate) fn debug_assert_assignment<M, N>(mat: &M, med: &[usize], data: &[Rec<N>])
where
    N: PartialOrd + Copy,
    M: ArrayAdapter<N>,
{
    for o in 0..mat.len() {
        debug_assert!(
            mat.get(o, med[data[o].near.i as usize]) == data[o].near.d,
            "primary assignment inconsistent"
        );
        debug_assert!(
            mat.get(o, med[data[o].seco.i as usize]) == data[o].seco.d,
            "secondary assignment inconsistent"
        );
        debug_assert!(
            data[o].near.d <= data[o].seco.d,
            "nearest is farther than second nearest"
        );
    }
}

/// Run the FasterPAM algorithm.
///
/// If used multiple times, it is better to additionally shuffle the input data,
/// to increase randomness of the solutions found and hence increase the chance
/// of finding a better solution.
///
/// * type `M` - matrix data type such as `ndarray::Array2` or
///   `kmedoids::arrayadapter::LowerTriangle`
/// * type `N` - number data type such as `u32` or `f64`
/// * type `L` - number data type such as `i64` or `f64` for the loss (must be signed)
/// * `mat` - a pairwise distance matrix
/// * `med` - the list of medoids
/// * `maxiter` - the maximum number of iterations allowed
///
/// returns a tuple containing:
/// * the final score
/// * the final cluster assignment
/// * the number of iterations needed
/// * the number of swaps performed
///
/// ## Panics
///
/// * panics when the dissimilarity matrix is not square
/// * panics when k is 0 or larger than N
#[allow(dead_code)]
pub fn constrained_fasterpam<M, L>(
    mat: &M,
    med: &mut [usize],
    maxiter: usize,
    min: usize,
    max: usize,
) -> (L, Vec<usize>, usize, usize)
where
    L: AddAssign + Signed + Zero + PartialOrd + Copy + From<i32>,
    M: ArrayAdapter<i32>,
{
    let (n, k) = (mat.len(), med.len());
    if k == 1 {
        panic!("K is not greater than 1")
    }
    let (mut loss, mut data) = initial_assignment(mat, med);

    #[cfg(debug_assertions)]
    debug_assert_assignment(mat, med, &data);

    let mut removal_loss = vec![L::zero(); k];
    update_removal_loss(&data, &mut removal_loss);
    let (mut lastswap, mut n_swaps, mut iter) = (n, 0, 0);

    while iter < maxiter {
        iter += 1;
        let swaps_before = n_swaps;

        // for each node
        for node_idx in 0..n {
            if node_idx == lastswap {
                break;
            }
            if node_idx == med[data[node_idx].near.i as usize] {
                continue; // This already is a medoid
            }

            let (change, medoid_idx) = find_best_swap(mat, &removal_loss, &data, node_idx);
            if change >= L::zero() {
                continue; // No improvement
            }

            // perform the swap
            n_swaps += 1;
            lastswap = node_idx;
            let newloss = do_swap(mat, med, &mut data, medoid_idx, node_idx);
            if newloss >= loss {
                break; // Probably numerically unstable now.
            }
            loss = newloss;
            update_removal_loss(&data, &mut removal_loss);
        }
        if n_swaps == swaps_before {
            break; // converged
        }
    }

    let assi = build_solve_graph(mat, med, min, max).0;
    (loss, assi, iter, n_swaps)
}

/// Perform the initial assignment to medoids
#[inline]
pub(crate) fn initial_assignment<M, N, L>(mat: &M, med: &[usize]) -> (L, Vec<Rec<N>>)
where
    N: Zero + PartialOrd + Copy,
    L: AddAssign + Zero + PartialOrd + Copy + From<N>,
    M: ArrayAdapter<N>,
{
    let (n, k) = (mat.len(), med.len());
    assert!(mat.is_square(), "Dissimilarity matrix is not square");
    assert!(n <= u32::MAX as usize, "N is too large");
    assert!(k > 0 && k < u32::MAX as usize, "invalid N");
    assert!(k <= n, "k must be at most N");
    let mut data = vec![Rec::<N>::empty(); mat.len()];

    let firstcenter = med[0];
    let loss = data
        .iter_mut()
        .enumerate()
        .map(|(i, cur)| {
            *cur = Rec::new(0, mat.get(i, firstcenter), u32::MAX, N::zero());
            for (m, &me) in med.iter().enumerate().skip(1) {
                let d = mat.get(i, me);
                if d < cur.near.d || i == me {
                    cur.seco = cur.near;
                    cur.near = DistancePair { i: m as u32, d };
                } else if cur.seco.i == u32::MAX || d < cur.seco.d {
                    cur.seco = DistancePair { i: m as u32, d };
                }
            }
            L::from(cur.near.d)
        })
        .reduce(L::add)
        .unwrap();

    (loss, data)
}

/// Find the best swap for object j - FastPAM version
#[inline]
pub(crate) fn find_best_swap<M, N, L>(
    mat: &M,
    removal_loss: &[L],
    data: &[Rec<N>],
    j: usize,
) -> (L, usize)
where
    N: Zero + PartialOrd + Copy,
    L: AddAssign + Signed + Zero + PartialOrd + Copy + From<N>,
    M: ArrayAdapter<N>,
{
    let mut ploss = removal_loss.to_vec();
    // Improvement from the journal version:
    let mut acc = L::zero();
    for (o, reco) in data.iter().enumerate() {
        let djo = mat.get(j, o);
        // New medoid is closest:
        if djo < reco.near.d {
            acc += L::from(djo) - L::from(reco.near.d);
            // loss already includes ds - dn, remove
            ploss[reco.near.i as usize] += L::from(reco.near.d) - L::from(reco.seco.d);
        } else if djo < reco.seco.d {
            // loss already includes ds - dn, adjust to d(xo) - dn
            ploss[reco.near.i as usize] += L::from(djo) - L::from(reco.seco.d);
        }
    }
    let (b, bloss) = find_min(&mut ploss.iter());
    (bloss + acc, b) // add the shared accumulator
}

/// Build and solve a min cost max flow graph for the given medoids
///
/// - Non-medoids are supply nodes
/// - medoid indeces do not have a role (hop used for max constraint)
/// - medoid' indeces are demand nodes
/// - one artificial demand node to ensure total demand = total supply
fn build_solve_graph<M: ArrayAdapter<i32>>(
    mat: &M,
    medoids: &[usize],
    min: usize,
    max: usize,
) -> (Vec<usize>, i32) {
    // - Edges
    //   - source -> [supply nodes]
    //     - capacity: 1
    //     - cost: 0
    //   - [supply nodes] -> [medoid]
    //     - capacity: 1
    //     - cost: lookup via DS matrix
    //   - [medoid -> medoid'] (all pairs)
    //     - capacity: size_max
    //     - cost: 0
    //   - [medoid'] -> artificial node
    //     - capacity: n_supply
    //     - cost: 0
    //   - [medoid'] -> sink
    //     - capacity: size_min
    //     - cost: 0
    //   - artificial node -> sink
    //     - capacity:  n_supply - n_medoids * size_min
    //     - cost: 0

    let total = mat.len();
    let k = medoids.len();
    let artificial_index = total + 2 * k;

    let mut graph = GraphBuilder::new();

    // Node Indeces: [0, len(x) - 1]
    for i in 0..total {
        if !medoids.contains(&i) {
            // source -> supply node
            graph.add_edge(Vertex::Source, i, Capacity(1), Cost(0));

            if let Some(self_offset) = medoids.iter().position(|v| v == &i) {
                for offset in 0..k {
                    let cost = if offset == self_offset { 0 } else { i32::MAX };
                    let cluster = offset + total;
                    graph.add_edge(i, cluster, Capacity(1), Cost(cost));
                }
            } else {
                for (offset, &j) in medoids.iter().enumerate() {
                    // supply node -> medoid
                    let cost = mat.get(i, j);
                    graph.add_edge(i, total + offset, Capacity(1), Cost(cost));
                }
            }
        }
    }

    // Cluster Indeces: [len(X), len(X) + len(C) - 1]
    // Cluster' Indeces: [len(X) + len(C), len(X) + 2 * len(C) - 1]
    for offset in 0..k {
        let index = total + offset;
        let prime_idx = index + k;

        // medoid -> medoid'
        graph.add_edge(index, prime_idx, Capacity(max as i32), Cost(0));

        // medoid' -> artificial
        graph.add_edge(prime_idx, artificial_index, Capacity(total as i32), Cost(0));

        // medoid' -> sink
        graph.add_edge(prime_idx, Vertex::Sink, Capacity(min as i32), Cost(0));
    }

    // Artificial Index: len(X) + 2 * len(C)
    // artificial node -> sink
    graph.add_edge(
        artificial_index,
        Vertex::Sink,
        Capacity((total - k * min) as i32),
        Cost(0),
    );

    // solve graph
    let (total_cost, paths) = graph.mcmf();
    let mut mappings: BTreeMap<usize, Vec<usize>> = BTreeMap::new();

    for path in paths {
        let verts = path.vertices();

        let node = verts[1].as_option().unwrap();
        let medoid = verts[2].as_option().unwrap();

        let ids = mappings.entry(medoid).or_default();
        ids.push(node);
    }

    let mut labels = vec![999; total];
    for (cluster, nodes) in mappings.values().enumerate() {
        for node in nodes {
            labels[*node] = cluster;
        }
    }

    // Find any missing nodes, and assign them to their closest cluster.
    for (i, assignment) in labels.iter_mut().enumerate() {
        if assignment == &999 {
            let mut best = 999;
            let mut diff = i32::MAX;
            for (j, &medoid) in medoids.iter().enumerate() {
                let diff2 = mat.get(i, medoid);
                if diff2 < diff {
                    best = j;
                    diff = diff2;
                }
            }
            *assignment = best;
        }
    }
    (labels, total_cost)
}

/// Update the loss when removing each medoid
pub(crate) fn update_removal_loss<N, L>(data: &[Rec<N>], loss: &mut [L])
where
    N: Zero + Copy,
    L: AddAssign + Signed + Copy + Zero + From<N>,
{
    loss.fill(L::zero()); // stable since 1.50
    for rec in data.iter() {
        loss[rec.near.i as usize] += L::from(rec.seco.d) - L::from(rec.near.d);
        // as N might be unsigned
    }
}

/// Update the second nearest medoid information
/// Called after each swap.
#[inline]
pub(crate) fn update_second_nearest<M, N>(
    mat: &M,
    med: &[usize],
    n: usize,
    b: usize,
    o: usize,
    djo: N,
) -> DistancePair<N>
where
    N: PartialOrd + Copy,
    M: ArrayAdapter<N>,
{
    let mut s = DistancePair::new(b as u32, djo);
    for (i, &mi) in med.iter().enumerate() {
        if i == n || i == b {
            continue;
        }
        let d = mat.get(o, mi);
        if d < s.d {
            s = DistancePair::new(i as u32, d);
        }
    }
    s
}

/// Perform a single swap
#[inline]
pub(crate) fn do_swap<M, N, L>(
    mat: &M,
    med: &mut [usize],
    data: &mut [Rec<N>],
    idx: usize,
    new: usize,
) -> L
where
    N: Zero + PartialOrd + Copy,
    L: AddAssign + Signed + Zero + PartialOrd + Copy + From<N>,
    M: ArrayAdapter<N>,
{
    let n = mat.len();
    assert!(idx < med.len(), "invalid medoid number");
    assert!(new < n, "invalid object number");
    med[idx] = new;
    data.iter_mut()
        .enumerate()
        .map(|(o, reco)| {
            if o == new {
                if reco.near.i != idx as u32 {
                    reco.seco = reco.near;
                }
                reco.near = DistancePair::new(idx as u32, N::zero());
                return L::zero();
            }
            let djo = mat.get(new, o);
            // Nearest medoid is gone:
            if reco.near.i == idx as u32 {
                if djo < reco.seco.d {
                    reco.near = DistancePair::new(idx as u32, djo);
                } else {
                    reco.near = reco.seco;
                    reco.seco = update_second_nearest(mat, med, reco.near.i as usize, idx, o, djo);
                }
            } else {
                // nearest not removed
                if djo < reco.near.d {
                    reco.seco = reco.near;
                    reco.near = DistancePair::new(idx as u32, djo);
                } else if djo < reco.seco.d {
                    reco.seco = DistancePair::new(idx as u32, djo);
                } else if reco.seco.i == idx as u32 {
                    // second nearest was replaced
                    reco.seco = update_second_nearest(mat, med, reco.near.i as usize, idx, o, djo);
                }
            }
            L::from(reco.near.d)
        })
        .reduce(L::add)
        .unwrap()
}
