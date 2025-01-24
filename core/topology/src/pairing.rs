use std::collections::BTreeSet;

use ndarray::Array2;

/// Greedily pair nodes together by finding their closest match, after a heuristic sort. If a
/// cluster is smaller than the other, it may have more than one connection per node.
///
/// In the network, we want to ensure as many low latency connections as we can, and dont care if
/// some other pairings are sub-optimal. This is in contrast to something like the hungarian
/// algorithm, which seeks to minimize the overall latency, and not prioritize the fastest possible
/// connections.
pub fn greedy_pairs(dissim_matrix: &Array2<i32>, a: &[usize], b: &[usize]) -> Vec<(usize, usize)> {
    let (a, b) = if a.len() > b.len() { (a, b) } else { (b, a) };
    let mut a = a.to_vec();

    // sort cluster a for the optimal ordering for each chunk
    hueristic_sort(dissim_matrix, &mut a, b);

    let mut pairs = Vec::new();
    let b_set = BTreeSet::from_iter(b);
    for chunk in a.chunks(b.len()) {
        // store a fresh clone of the b set to remove entries from for this chunk
        let mut b_set_cloned = b_set.clone();
        for i in chunk.iter() {
            // find the index with the lowest latency from the b set
            let best = *b_set_cloned
                .iter()
                .min_by_key(|&j| dissim_matrix[(*i, **j)])
                .unwrap();
            // remove it for the next iteration
            b_set_cloned.remove(best);
            pairs.push((*i, *best));
        }
    }

    pairs
}

/// Heuristically sort a's indeces for greedily pairing.
///
/// First, sort a by the sum of distances to b for each item in a.
///
/// ```text
/// a (sorted): [ 0 1 2 3 4 5 6 7 ]
/// len: 8
/// b (unsorted): [ 8 9 ]
/// len: 2
/// ```
///
/// Now iterate through the sorted items, to form chunks where each next index skips n items.
///
/// ```text
/// n: ceil(a len / b len) = 4
/// [ 0 4 ] [ 1 5 ] [ 2 6 ] [ 3 7 ]
/// ```
///
/// Hypothetically speaking, this should be a good hueristic because each chunks first items,
/// which have the lowest sum latency to b, will have the most number of options and will be
/// able to select the most optimal pairing.
pub fn hueristic_sort(dissim_matrix: &Array2<i32>, a: &mut [usize], b: &[usize]) {
    // 1. compute and sort each node by their sums of dissim to b
    let mut sorted = a.to_vec();
    sorted.sort_by_cached_key(|&i| b.iter().map(|&j| dissim_matrix[(i, j)]).sum::<i32>());

    // 2. reassign indeces
    let len = a.len();
    let n = len.div_ceil(b.len()); // ceiling 
    let mut iter = a.iter_mut();
    for c in 0..n {
        let mut j = c;
        while j < len {
            *iter.next().unwrap() = sorted[j];
            j += n;
        }
    }
}

#[test]
fn test_greedy_pairing() {
    use ndarray_rand::rand_distr::UnitDisc;

    fn sample_cluster(
        x: f32,
        y: f32,
        x_scale: f32,
        y_scale: f32,
        num_points: usize,
    ) -> Array2<i32> {
        let mut cluster = Array2::zeros((num_points, 2));

        for i in 0..num_points {
            let v: [f32; 2] =
                rand::prelude::Distribution::sample(&UnitDisc, &mut rand::thread_rng());
            cluster[[i, 0]] = (x + v[0] * x_scale) as i32;
            cluster[[i, 1]] = (y + v[1] * y_scale) as i32;
        }
        cluster
    }

    fn get_distance_matrix(data: &Array2<i32>) -> Array2<i32> {
        let mut dist = Array2::zeros((data.shape()[0], data.shape()[0]));
        for i in 0..data.shape()[0] {
            for j in 0..data.shape()[0] {
                if i != j {
                    dist[[i, j]] =
                        (data[[i, 0]] - data[[j, 0]]).pow(2) + (data[[i, 1]] - data[[j, 1]]).pow(2);
                }
            }
        }
        dist
    }

    let cluster1 = sample_cluster(1., 1., 1000., 1000., 10);
    let cluster2 = sample_cluster(100., 100., 1000., 1000., 10);
    let data =
        ndarray::concatenate(ndarray::Axis(0), &[(&cluster1).into(), (&cluster2).into()]).unwrap();
    let dis_matrix = get_distance_matrix(&data);

    let indeces: Vec<_> = (0..dis_matrix.nrows()).collect();

    let pairs = greedy_pairs(&dis_matrix, &indeces[0..10], &indeces[10..]);
    println!("{pairs:?}");
}
