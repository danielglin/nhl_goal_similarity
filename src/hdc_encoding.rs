// hdc_encoding.rs
// Encodes goals as hypervectors 
use ndarray::{s, Array, Dim};

use rand;
use rand::distr::{Bernoulli, Distribution};
use rand::seq::{IndexedRandom, SliceRandom};
use std::collections::HashSet;

use crate::preprocessing::TrimmedPuckLocations;

/// Calculates the distance between two binary hypervectors, which is the
/// number of different bits
/// Panics if the two hypervectors are of different lengths.
pub fn dist(a: &Array<bool, Dim<[usize; 1]>>, b: &Array<bool, Dim<[usize; 1]>>) -> u32 {
    let diffs = a ^ b;
    let num_diff = diffs.mapv(u32::from).sum();
    // let dist = (num_diff as f32)/(a.shape()[0] as f32);
    // dist
    num_diff
}

/// Makes the grid level hypervecs, which has shape [num_pixels_y_dim, num_pixels_x_dim, hypervec_dim]
pub fn create_grid_level_hypervecs(num_pixels_x_dim: usize, num_pixels_y_dim: usize, hypervec_dim: usize) -> 
Array<bool, Dim<[usize; 3]>> 
{
    let mut rng = rand::rng();
    
    // make the corner level hypervec
    let p = 0.5;
    let bern_distro = Bernoulli::new(p).unwrap();
    let corner_level_hv: Vec<bool> = bern_distro.sample_iter(&mut rng).take(hypervec_dim).collect();
    let corner_level_hv = Array::from_vec(corner_level_hv);
    
    // split the corner level hypervec into an "x" half and a
    // "y" half
    let first_y_index = hypervec_dim / 2;
    
    // create the flip masks now so they are the same
    // across rows and cols
    let num_x_flip = first_y_index / num_pixels_x_dim;
    let num_y_flip = first_y_index / num_pixels_y_dim;
    
    // these are the indices we can choose to flip
    let mut avail_x_flip_indices: HashSet<usize> = (0..first_y_index).collect();
    let mut avail_y_flip_indices: HashSet<usize> = (first_y_index..hypervec_dim).collect();
    
    // create the x flip masks
    let mut x_flip_masks = Vec::with_capacity(num_pixels_x_dim);
    x_flip_masks.push(Array::from_elem(hypervec_dim, false));
    for _ in 0..(num_pixels_x_dim-1) {
        let mut flip_mask = Array::from_elem(hypervec_dim, false);

        // see https://docs.rs/rand/latest/rand/
        let avail_x_flip_indices_cloned = avail_x_flip_indices.clone();
        let v_avail_x_flip_indices: Vec<_> = avail_x_flip_indices_cloned.iter().collect();
        let indices: Vec<_> = v_avail_x_flip_indices.choose_multiple(&mut rng, num_x_flip).collect();
//         println!("indices: {:?}", indices);
        
        for i in indices {
             // update flip_mask based on the inidices we sampled
            flip_mask[**i] = true;

            // update the available flip indices
            avail_x_flip_indices.remove(i);
        }
//         println!("flip mask: {}", flip_mask);

        // build the new flip mask by applying the flip mask to the previous flip mask
        flip_mask = x_flip_masks.last().unwrap() ^ flip_mask;
        x_flip_masks.push(flip_mask);
    }
    
    // create the y flip masks
    let mut y_flip_masks = Vec::with_capacity(num_pixels_y_dim);
    y_flip_masks.push(Array::from_elem(hypervec_dim, false));
    for _ in 0..(num_pixels_y_dim-1) {
        let mut flip_mask = Array::from_elem(hypervec_dim, false);

        // see https://docs.rs/rand/latest/rand/
        let avail_y_flip_indices_cloned = avail_y_flip_indices.clone();
        let v_avail_y_flip_indices: Vec<_> = avail_y_flip_indices_cloned.iter().collect();
        let indices: Vec<_> = v_avail_y_flip_indices.choose_multiple(&mut rng, num_y_flip).collect();
//         println!("indices: {:?}", indices);
        
        for i in indices {
             // update flip_mask based on the inidices we sampled
            flip_mask[**i] = true;

            // update the available flip indices
            avail_y_flip_indices.remove(i);
        }
//         println!("flip mask: {}", flip_mask);

        // build the new flip mask by applying the flip mask to the previous flip mask
        flip_mask = y_flip_masks.last().unwrap() ^ flip_mask;
        y_flip_masks.push(flip_mask);
    }
    
    // init the grid of level hypervecs
    let mut grid_hypervecs = Array::from_elem((num_pixels_y_dim, num_pixels_x_dim, hypervec_dim), false);
    for y in 0..num_pixels_y_dim {
        let y_flip_mask = &y_flip_masks[y];
        for x in 0..num_pixels_x_dim {
            // apply the appropriate flip masks
            let x_flip_mask = &x_flip_masks[x];
            
            let pixel_level_hypervec = &corner_level_hv ^ x_flip_mask;
            let pixel_level_hypervec = pixel_level_hypervec ^ y_flip_mask;
            
            grid_hypervecs.slice_mut(s![y, x, ..]).assign(&pixel_level_hypervec);
        }
    }
    
    grid_hypervecs
}

/// Creates permutation arrays for binding multiple coordinates together
/// The "outer" vector is num_arrays long.
/// The "inner" vectors, the permutation arrays, are hypervec_dim long.
pub fn _make_perm_arrays(num_arrays: usize, hypervec_dim: usize) -> Vec<Vec<usize>>{
    let indices: Vec<usize> = (0..hypervec_dim).collect();
    let mut perm_arrays = Vec::with_capacity(num_arrays);

    let mut rng = rand::rng();
    for _ in 0..num_arrays {
        let mut perm = indices.clone();
        perm.shuffle(&mut rng);
        perm_arrays.push(perm);
    }

    perm_arrays
}

/// Coord for the grid level hyervecs
#[derive(Debug)]
struct GridLevelHvCoord {
    x: usize,
    y: usize,
}

/// Returns the hypervec for a window of coordinates
/// Is None if the window has length 0
fn encode_window_seq_pos(
    window: &[GridLevelHvCoord],
    grid_hvs: &Array<bool, Dim<[usize; 3]>>,    
    perm_arrays: &[Vec<usize>]
) -> Option<Array<bool, Dim<[usize; 1]>>>  {
    let mut window_hv = None;
    let hypervec_dim = grid_hvs.shape()[2];
    
    for (instant_index, coord) in window.iter().enumerate() {
        let pixel_hv = grid_hvs.slice(s![coord.y, coord.x, ..]);
        
        // do the perm'ing for the pixel hv
        let mut permed_pixel_hv = Vec::with_capacity(hypervec_dim);
        for hv_dim_index in 0..hypervec_dim {
            let permed_index = perm_arrays[instant_index][hv_dim_index];
            permed_pixel_hv.push(pixel_hv[permed_index]);
        }
        let permed_pixel_hv_array = Array::from_vec(permed_pixel_hv);
        
        if instant_index>0 {
            window_hv = Some(window_hv.unwrap() ^ permed_pixel_hv_array);
        } else {
            window_hv = Some(permed_pixel_hv_array); 
        }
    }

    window_hv
}

/// Bundles together a slice of binary hypervecs
/// Bundling is the majority vote operation.  Ties become true.
/// Result is a binary hypervec of the same dimension
fn bundle(hypervecs: &[Array<bool, Dim<[usize; 1]>>])  -> Array<bool, Dim<[usize; 1]>> {
    let hypervec_dim = hypervecs[0].shape()[0];
    let mut sum = Array::from_elem(hypervec_dim, 0.0f32);

    for hv in hypervecs {
        let hv_float = hv.mapv(f32::from);
        sum = sum + hv_float;
    }

    let avg = sum/(hypervecs.len() as f32);
    
    let bundled_rslt = avg.map(|x| *x > 0.5);
    bundled_rslt
}

/// Encodes a goal as a hypervec, with an adjustable scale factor for the
/// locations
/// `window_size` is the size of the sliding windows to use
/// `perm_arrays` are the permutation arrays used to bind different positions 
/// in the sequence
/// `scale_factor` is the number to divide the raw coordinates by before
/// getting the pixel hypervec from the grid level hypervecs
/// Returns None if the goal has no coords or fewer coords than the window size
pub fn encode_goal_seq_pos_scale(
    goal: &TrimmedPuckLocations,
    window_size: usize,
    grid_hvs: &Array<bool, Dim<[usize; 3]>>,
    perm_arrays: &[Vec<usize>],
    scale_factor: f64
) -> Option<Array<bool, Dim<[usize; 1]>>> { 
    // coord length check
    if goal.coords.len() < window_size {
        return None
    }

    let mut window_hvs = vec![];

    // create the windows
    for window in goal.coords.windows(window_size) {
        // need to convert the coord's floats into ints and scale to get 
        // the indices of the grid level hypervecs
        let mut coord_window = Vec::with_capacity(window_size);
        for instant in window {            
            let scaled_x = (instant.x/scale_factor) as usize;
            let scaled_y = (instant.y/scale_factor) as usize;
            coord_window.push(GridLevelHvCoord { x: scaled_x, y: scaled_y });
        }
        
        // make the window hypervec
        let window_hv = match encode_window_seq_pos(&coord_window, grid_hvs, perm_arrays) {
            Some(w_hv) => w_hv,
            None => {
                // println!("*** There is an empty window. ***");
                continue;
            }
        };

        // don't bundle yet since need to have all the window hypervecs in 
        // order to bundle
        // so just accumulate the window hypervecs
        window_hvs.push(window_hv);
    }

    // since we have all the window hypervecs, now we can bundle them together
    // to form the goal's hypervec
    let goal_hv = bundle(&window_hvs);
    Some(goal_hv)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::preprocessing::Coord;
    use ndarray::array;

    // -----------------------------------
    // dist() tests
    // -----------------------------------
    #[test]
    fn dist_all_same_false() {
        let a_1 = array![false, false, false, false, false];
        let a_2 = array![false, false, false, false, false];
        
        assert_eq!(dist(&a_1, &a_2), 0);
    }

    #[test]
    fn dist_all_same_true() {
        let a_1 = array![true, true, true, true, true];
        let a_2 = array![true, true, true, true, true];        
        
        assert_eq!(dist(&a_1, &a_2), 0);
    }

    #[test]
    fn dist_all_same_mixed() {
        let a_1 = array![true, false, true, true, false];
        let a_2 = array![true, false, true, true, false];        
        
        assert_eq!(dist(&a_1, &a_2), 0);
    }

    #[test]
    fn dist_all_diff_all_false_true() {
        let a_1 = array![false, false, false, false, false];
        let a_2 = array![true, true, true, true, true];        
        
        assert_eq!(dist(&a_1, &a_2), 5);
    }

    #[test]
    fn dist_all_diff_mixed() {
        let a_1 = array![false, true, false, false, true];
        let a_2 = array![true, false, true, true, false];        
        
        assert_eq!(dist(&a_1, &a_2), 5);
    }

    #[test]
    fn dist_mix() {
        let a_1 = array![false, false, false, true, false];
        let a_2 = array![true, false, true, true, true];        
        
        assert_eq!(dist(&a_1, &a_2), 3);
    }

    // -----------------------------------
    // create_grid_level_hypervecs() tests
    // -----------------------------------

    #[test]
    fn create_grid_level_hypervecs_dist_checks() {
        let num_pixels_x_dim = 241;
        let num_pixels_y_dim = 102;
        let hypervec_dim = 5000;

        let grid_hvs = create_grid_level_hypervecs(num_pixels_x_dim, num_pixels_y_dim, hypervec_dim);

        // check shape
        assert_eq!(grid_hvs.shape(), [102, 241, 5000]);

        // check opposite corners
        let corner_dist = dist(
            &grid_hvs.slice(s![0, 0, ..]).into_owned(),
            &grid_hvs.slice(s![101, 240, ..]).into_owned()
        );
        
        assert!(corner_dist>= 4750);

        // check neighbors
        let adjacent_dist = dist(
            &grid_hvs.slice(s![30, 40, ..]).into_owned(),
            &grid_hvs.slice(s![31, 40, ..]).into_owned()
        );
        let diag_dist = dist(
            &grid_hvs.slice(s![30, 40, ..]).into_owned(),
            &grid_hvs.slice(s![31, 41, ..]).into_owned()
        );
        assert!(adjacent_dist<diag_dist);
    }

    // -----------------------------------
    // _make_perm_arrays() tests
    // -----------------------------------

    #[test]
    fn _make_perm_arrays_shape() {
        let num_arrays = 3;
        let hypervec_dim = 5;
        let perm_arrays = _make_perm_arrays(num_arrays, hypervec_dim);

        assert_eq!(perm_arrays.len(), 3);
        assert_eq!(perm_arrays[0].len(), 5);
        assert_eq!(perm_arrays[1].len(), 5);
        assert_eq!(perm_arrays[2].len(), 5);
    }

    #[test]
    fn _make_perm_arrays_vals() {
        let num_arrays = 3;
        let hypervec_dim = 5;
        let perm_arrays = _make_perm_arrays(num_arrays, hypervec_dim);

        let sorted = vec![0, 1, 2, 3, 4];
        let mut first_sorted = perm_arrays[0].clone();
        first_sorted.sort();
        assert_eq!(first_sorted, sorted);

        let mut first_sorted = perm_arrays[1].clone();
        first_sorted.sort();
        assert_eq!(first_sorted, sorted);

        let mut first_sorted = perm_arrays[2].clone();
        first_sorted.sort();
        assert_eq!(first_sorted, sorted);
    }

    // -----------------------------------
    // encode_window_seq_pos() tests
    // -----------------------------------

    // empty window should return None
    #[test]
    fn encode_window_seq_pos_empty_window() {
        let window = vec![];
        let num_pixels_x_dim = 5;
        let num_pixels_y_dim = 5;
        let hypervec_dim = 10;
        let grid_hvs = create_grid_level_hypervecs(num_pixels_x_dim, num_pixels_y_dim, hypervec_dim);
        let perm_arrays = _make_perm_arrays(3, hypervec_dim);

        let window_hv = encode_window_seq_pos(&window, &grid_hvs, &perm_arrays);
        assert!(window_hv.is_none());
    }

    // window with coords
    #[test]
    fn encode_window_seq_pos_nonempty_window() {
        let window = vec![
            GridLevelHvCoord { x: 0, y: 0 },
            GridLevelHvCoord { x: 1, y: 1 },
            GridLevelHvCoord { x: 1, y: 2 },
        ];

        let grid_hvs = array![
            [
                [false, false, false, false, true],
                [false, false, true, false, false],
                [false, false, true, true, false]
            ],
            [
                [false, true, true, true, false],
                [true, false, false, true, false],
                [true, false, true, false, true]
            ],
            [
                [true, true, false, false, false],
                [true, false, true, false, false],
                [true, true, true, false, true]
            ]
        ];
        
        let perm_arrays = vec![
            vec![0, 1, 2, 4, 3],
            vec![2, 3, 4, 0, 1],
            vec![1, 4, 2, 3, 0]
        ];
        
        let window_hv_opt = encode_window_seq_pos(&window, &grid_hvs, &perm_arrays);

        assert!(window_hv_opt.is_some());
        let window_hv = window_hv_opt.unwrap();
        assert_eq!(window_hv.shape()[0], 5);

        // check window hypervec's values
        
        // first coord is (0, 0), which is [false, false, false, false, true]
        // permutation array is [0, 1, 2, 4, 3]
        // permuted pixel hypervec is [false, false, false, true, false]

        // second coord is (1, 1), which is [true, false, false, true, false]
        // permutation array is [2, 3, 4, 0, 1]
        // permuted pixel hypervec is [false, true, false, true, false]
        // XOR of first two permuted pixel hypervecs is [false, true, false, false, false]

        // third coord is (1, 2), which is [true, false, true, false, false]
        // permutation array is [1, 4, 2, 3, 0]
        // permuted pixel hypervec is [false, false, true, false, true]
        // XOR of all three permuted pixel hypervecs is [false, true, true, false, true]
        assert_eq!(window_hv, array![false, true, true, false, true]);
    }

    // -----------------------------------
    // bundle() tests
    // -----------------------------------

    // all false should result in all false
    #[test]
    fn bundle_all_false() {
        let a_1 = array![false, false, false, false, false];
        let a_2 = array![false, false, false, false, false];
        let a_3 = array![false, false, false, false, false];
        let hypervecs = vec![a_1, a_2, a_3];

        let bundled_rslt= bundle(&hypervecs);
        assert_eq!(bundled_rslt, array![false, false, false, false, false]);
    }

    // all true should result in all true
    #[test]
    fn bundle_all_true() {
        let a_1 = array![true, true, true, true, true];
        let a_2 = array![true, true, true, true, true];        
        let hypervecs = vec![a_1, a_2];

        let bundled_rslt= bundle(&hypervecs);
        assert_eq!(bundled_rslt, array![true, true, true, true, true]);
    }

    // mix of true and false
    #[test]
    fn bundle_mix() {
        let a_1 = array![false, true, false, true, false];
        let a_2 = array![true, false, false, true, false];
        let a_3 = array![false, true, false, true, true];
        let hypervecs = vec![a_1, a_2, a_3];

        let bundled_rslt= bundle(&hypervecs);
        assert_eq!(bundled_rslt, array![false, true, false, true, false]);
    }

    // ties become true
    #[test]
    fn bundle_ties() {
        let a_1 = array![false, true];
        let a_2 = array![true, false];
        let hypervecs = vec![a_1, a_2];

        let bundled_rslt= bundle(&hypervecs);        
        assert_eq!(bundled_rslt, array![false, false]);
    }

    // bundling just one hypervec is itself
    #[test]
    fn bundle_single_hv() {
        let a_1 = array![false, true, false, true, false];
        let hypervecs = vec![a_1.clone()];

        let bundled_rslt = bundle(&hypervecs);
        assert_eq!(bundled_rslt, a_1);
    }

    // -----------------------------------
    // encode_goal_seq_pos_scale() tests
    // -----------------------------------

    // goal with no coords returns None
    #[test]
    fn encode_goal_seq_pos_scale_empty_goal() {
        let goal = TrimmedPuckLocations { coords: vec![] };
        let window_size = 3;
        let scale_factor = 10.;

        let grid_hvs = array![
            [
                [false, false, false, false, true],
                [false, false, true, false, false],
                [false, false, true, true, false]
            ],
            [
                [false, true, true, true, false],
                [true, false, false, true, false],
                [true, false, true, false, true]
            ],
            [
                [true, true, false, false, false],
                [true, false, true, false, false],
                [true, true, true, false, true]
            ]
        ];
        
        let perm_arrays = vec![
            vec![0, 1, 2, 4, 3],
            vec![2, 3, 4, 0, 1],
            vec![1, 4, 2, 3, 0]
        ];
        let goal_hv = encode_goal_seq_pos_scale(&goal, window_size, &grid_hvs, &perm_arrays, scale_factor);
        assert!(goal_hv.is_none());
    }

    // goal w/ windows
    #[test]
    fn encode_goal_seq_pos_scale_nonempty_goal() {
        let coords = vec![
            Coord { x: 4., y: 8. },
            Coord { x: 11., y: 17. },
            Coord { x: 14., y: 25. },
            Coord { x: 24., y: 26. }
        ];
        let goal = TrimmedPuckLocations { coords: coords };
        let window_size = 3;
        let scale_factor = 10.;

        let grid_hvs = array![
            [
                [false, false, false, false, true],
                [false, false, true, false, false],
                [false, false, true, true, false]
            ],
            [
                [false, true, true, true, false],
                [true, false, false, true, false],
                [true, false, true, false, true]
            ],
            [
                [true, true, false, false, false],
                [true, false, true, false, false],
                [true, true, true, false, true]
            ]
        ];
        
        let perm_arrays = vec![
            vec![0, 1, 2, 4, 3],
            vec![2, 3, 4, 0, 1],
            vec![1, 4, 2, 3, 0]
        ];
        let goal_hv = encode_goal_seq_pos_scale(&goal, window_size, &grid_hvs, &perm_arrays, scale_factor);

        // first window hypervec is [false, true, true, false, true] from
        // above encode window test

        // second window hypervec calcs:
        // first coord is (11, 17), corresponding to pixel (1, 1): [true, false, false, true, false]
        // permutation array is [0, 1, 2, 4, 3]
        // permuted pixel is [true, false, false, false, true]

        // second coord is (14, 25), corresponding to pixel (1, 2): [true, false, true, false, false]
        // permutation array is [2, 3, 4, 0, 1]
        // permuted pixel is [true, false, false, true, false]
        // first two permuted pixel XOR: [false, false, false, true, true]
        
        // third coord is (24, 26), corresponding to pixel (2, 2): [true, true, true, false, true]
        // permutation array is [1, 4, 2, 3, 0]
        // permuted pixel is [true, true, true, false, true]
        // window hypervec is [true, true, true, true, false]

        // goal hypervec is [false, true, true, false, false] since tie breaks are false
        assert_eq!(goal_hv.unwrap(), array![false, true, true, false, false]);
    }
}