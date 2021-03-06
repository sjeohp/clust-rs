use crate::itertools::Itertools;
use kdtree::distance::squared_euclidean;
use kdtree::KdTree;
use ndarray::prelude::*;
use num_traits::float::Float;
use num_traits::identities::{One, Zero};
use rand::prelude::thread_rng;
use rand::seq::index::sample;

#[derive(Debug)]
pub struct Dbscan<T: Float + One + Zero> {
    pub eps: T,
    pub min_points: usize,
    pub clusters: Vec<usize>,
}

impl<T: Float + One + Zero> Dbscan<T> {
    pub fn new(data: &Array2<T>, eps: T, min_points: usize, borders: bool) -> Dbscan<T> {
        let mut c = 1;
        let mut neighbours = Vec::with_capacity(data.rows());
        let mut sub_neighbours = Vec::with_capacity(data.rows());
        let mut visited = vec![false; data.rows()];
        let mut clusters = vec![0; data.rows()];
        let kdt = kdtree_init(&data);

        let indices = sample(&mut thread_rng(), data.rows(), data.rows());
        for row_idx in indices.iter() {
            let row = data.row(row_idx);
            if !visited[row_idx] {
                visited[row_idx] = true;

                neighbours.clear();
                region_query(row.as_slice().unwrap(), eps, &kdt, &mut neighbours);
                neighbours.sort_unstable_by(|a, b| a.partial_cmp(b).unwrap());
                neighbours.dedup();

                if neighbours.len() >= min_points {
                    clusters[row_idx] = c;
                    while let Some(neighbour_idx) = neighbours.pop() {
                        if borders {
                            clusters[neighbour_idx] = c;
                        }
                        if !visited[neighbour_idx] {
                            visited[neighbour_idx] = true;
                            sub_neighbours.clear();
                            region_query(data.row(neighbour_idx).as_slice().unwrap(), eps, &kdt, &mut sub_neighbours);

                            if sub_neighbours.len() >= min_points {
                                if !borders {
                                    clusters[neighbour_idx] = c;
                                }
                                neighbours.extend_from_slice(&sub_neighbours);
                                neighbours.sort_unstable_by(|a, b| a.partial_cmp(b).unwrap());
                                neighbours.dedup();
                            }
                        }
                    }
                    c += 1;
                }
            }
        }

        Dbscan {
            eps: eps,
            min_points: min_points,
            clusters: clusters,
        }
    }

    pub fn predict(&self, data: &Array2<T>, new_data: &Array2<T>) -> Vec<Vec<usize>> {
        let mut neighbours = Vec::with_capacity(data.rows());
        let kdt = kdtree_init(&data);
        new_data
            .outer_iter()
            .map(|row| {
                neighbours.clear();
                region_query(row.as_slice().unwrap(), self.eps, &kdt, &mut neighbours);
                let neighbour_clusters = neighbours.iter().map(|idx| self.clusters[*idx]).unique().filter(|c| *c > 0).collect::<Vec<usize>>();
                if neighbour_clusters.len() > 0 {
                    neighbour_clusters
                } else {
                    vec![0]
                }
            })
            .collect::<Vec<Vec<usize>>>()
    }
}

fn kdtree_init<'a, T: Float + One + Zero>(data: &'a Array2<T>) -> KdTree<T, usize, &'a [T]> {
    let mut kdt = KdTree::new(data.cols());
    for (idx, row) in data.outer_iter().enumerate() {
        kdt.add(row.into_slice().unwrap(), idx).unwrap();
    }
    kdt
}

fn region_query<'a, T: Float + One + Zero>(row: &'a [T], eps: T, kdt: &KdTree<T, usize, &'a [T]>, neighbours: &mut Vec<usize>) {
    for (_, neighbour_idx) in kdt.within(row, eps.powi(2), &squared_euclidean).expect("KdTree error checking point") {
        neighbours.push(*neighbour_idx);
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum ClusterPrediction {
    Core(Vec<usize>),
    Border(Vec<usize>),
    Noise,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_clusters() {
        let data = Array2::from_shape_vec((8, 2), vec![1.0, 2.0, 1.1, 2.2, 0.9, 1.9, 1.0, 2.1, -2.0, 3.0, -2.2, 3.1, -1.0, -2.0, -2.0, -1.0]).unwrap();
        let model = Dbscan::new(&data, 0.5, 2, false);
        let clustering = dbg!(model.clusters);
        assert!(clustering.iter().take(4).all_equal());
        assert!(clustering.iter().skip(4).take(2).all_equal());
        assert!(clustering.iter().skip(6).all_equal());
        assert!(clustering[0] != clustering[4]);
        assert!(clustering[4] != clustering[6]);
        assert!(clustering[6] != clustering[0]);
    }

    #[test]
    fn test_border_points() {
        let data = Array2::from_shape_vec((5, 1), vec![1.55, 2.0, 2.1, 2.2, 2.65]).unwrap();

        let with = Dbscan::new(&data, 0.5, 3, true);
        let without = Dbscan::new(&data, 0.5, 3, false);
        let with_borders_clustering = dbg!(with.clusters);
        let without_borders_clustering = dbg!(without.clusters);
        assert!(with_borders_clustering.iter().all(|x| *x == 1));
        assert!(without_borders_clustering.iter().take(1).all(|x| *x == 0));
        assert!(without_borders_clustering.iter().skip(1).take(3).all(|x| *x == 1));
        assert!(without_borders_clustering.iter().skip(4).all(|x| *x == 0));
    }

    #[test]
    fn test_prediction() {
        let data = Array2::from_shape_vec((6, 2), vec![1.0, 2.0, 1.1, 2.2, 0.9, 1.9, 1.0, 2.1, -2.0, 3.0, -2.2, 3.1]).unwrap();
        let model = Dbscan::new(&data, 0.5, 2, false);

        let new_data = Array2::from_shape_vec((2, 2), vec![1.0, 2.0, 4.0, 4.0]).unwrap();
        let classes = dbg!(model.predict(&data, &new_data));

        let c0 = classes.get(0).unwrap();
        assert!(c0.iter().any(|c| *c == model.clusters[0]));
        assert!(classes[1] == vec![0]);
    }
}
