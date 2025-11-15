use anyhow::{anyhow, Result};
use clap::{Parser};

use std::{collections::HashMap};
use std::fmt::Display;
use std::path::Path;


use crate::preprocessing::{GameId, GoalId, PbpGoalLocData, TrimmedPuckLocations, preprocess_folder_data, read_folder, take_last_n_instances};
use crate::hdc_encoding::{_make_perm_arrays, create_grid_level_hypervecs, dist, encode_goal_seq_pos_scale};
use polars::prelude::{Series, df, ParquetWriter, ParquetReader, SerReader, AnyValue};

mod preprocessing;
mod hdc_encoding;

use ndarray_npy::{read_npy, write_npy};
use ndarray::{Array, Dim};
use serde_json;
use std::fs;
use std::io::BufReader;

fn main() -> Result<()> {
    const NUM_PIXELS_X_DIM: usize = 241;
    const NUM_PIXELS_Y_DIM: usize = 102;
    const HYPERVEC_DIM: usize = 5000;
    const WINDOW_SIZE: usize = 7;
    const SCALE_FACTOR: f64 = 10.;

    let args = Args::parse();


    let specified_game_id = GameId(args.game.parse::<u32>().expect(&format!("Invalid game id: {}", args.game)));
    let specified_goal_id = GoalId(args.goal.parse::<u32>().expect(&format!("Invalid game id: {}", args.goal)));


    // import goal hypervecs if the import option is chosen
    // this means we skip reading in the location JSON's and the goal encoding
    // process
    let grid_hvs;
    let perm_arrays;
    let mut goal_hvs_map;
    if args.import_info {
        (grid_hvs, perm_arrays, goal_hvs_map) =  import_hv_info(args.import_dir.expect("Need to specify import directory when importing goal hypervectors"))?;
        println!("*** Finished importing hypervectors ***");

        // for now, can't export anything when we import hypervec info
        // later, consider allowing creating new goal hypervecs that aren't
        // in the imported data
        if args.export_hvs {
            println!("Exporting goal hypervectors when also importing existing goal hypervectors is currently not supported.");
        };
    } else {
        // read in location JSON's, rotate, and trim them
        let input_dir = args.input_dir.expect("!!! If not importing goal hypervectors, need to provide an input directory of goal location JSON files");
        let mut loc_data = PbpGoalLocData {
            goal_loc_data: HashMap::new(),
            pbp_data: HashMap::new()
        };
        
        match read_folder(input_dir, &mut loc_data) {
            Err(e) => {
                return Err(anyhow!("Error when reading in data: {e}"))
            },
            _ => ()
        };
        let mut preprocessed_data = preprocess_folder_data(&loc_data);

        // take the last n instances of each goal if specified
        if args.last_n.is_some() {
            let n = args.last_n.expect("No n set");
            let mut shrunken_data = HashMap::with_capacity(preprocessed_data.len());
            
            for ((game_id, goal_id), trimmed_goal) in &preprocessed_data {
                let last_n = take_last_n_instances(trimmed_goal.clone(), n);
                shrunken_data.insert((game_id.clone(), goal_id.clone()), last_n);
            }
            preprocessed_data = shrunken_data;
        }

        // export preprocessed data if specified
        if args.export_pp_loc_file.is_some() {
            let export_pp_loc = args.export_pp_loc_file.expect("No file location given to export pre-processed goal data to");
            match export_pp_loc_data(export_pp_loc, &preprocessed_data) {
                Ok(_) => {
                    println!("--- Finished exporting pre-processed goal location data");
                },
                Err(e) => {
                    println!("Error when exporting pre-processed goal location data: {e}");
                }
            };
        }

        // check that the specified goal is in the data read in before we spend
        // the time to create the hypervecs
        if !preprocessed_data.contains_key(&(specified_game_id, specified_goal_id)) {
            return Err(anyhow!("Goal not found: game id {}, goal id {}", specified_game_id.0, specified_goal_id.0));
        };

        // use the trimmed location data to encode each goal as an hypervec
        grid_hvs = create_grid_level_hypervecs(NUM_PIXELS_X_DIM, NUM_PIXELS_Y_DIM, HYPERVEC_DIM);
        perm_arrays = _make_perm_arrays(WINDOW_SIZE, HYPERVEC_DIM);

        goal_hvs_map = HashMap::new();

        // set up columns that we'll use for exporting as one big dataframe
        let mut game_ids_for_export = Vec::with_capacity(preprocessed_data.len());
        let mut goal_ids_for_export = Vec::with_capacity(preprocessed_data.len());
        let mut goal_hvs_for_export = Vec::with_capacity(preprocessed_data.len());

        for ((game_id, goal_id), trimmed_goal) in preprocessed_data {
            if trimmed_goal.coords.len() == 0 {
                continue
            }

            // create the goal hypervec and store it in the hash map
            let goal_hv = match encode_goal_seq_pos_scale(
                &trimmed_goal, 
                WINDOW_SIZE, 
                &grid_hvs, 
                &perm_arrays, 
                SCALE_FACTOR
            ) {
                Some(ghv) => ghv,
                None => {
                    println!("------ Skipping goal hypervector creation for game {}, goal {} due to goal having fewer instances than the window size.", game_id.0, goal_id.0);
                    continue;
                }
            };        
            goal_hvs_map.insert((game_id.clone(), goal_id.clone()), goal_hv.clone());

            // add data that will populate the exported dataframe
            game_ids_for_export.push(game_id.0);
            goal_ids_for_export.push(goal_id.0);
            goal_hvs_for_export.push(goal_hv.iter().collect::<Series>());
        }

        // export goal hypervecs, grid level hypervecs, and perm arrays
        if args.export_hvs {
            let output_dir = args.output_dir.expect("Need an output directory in order to export");
            let export_grid_and_perm = args.export_grid_perm;
            export_hv_info(
                &output_dir, export_grid_and_perm, &goal_hvs_for_export, &grid_hvs, 
                &game_ids_for_export, &goal_ids_for_export, &perm_arrays
            )?;
        }
    }; // end of else branch where we read in location JSON's

    // calc all the distances from the specified goal to all other goals
    let specified_goal_hv = match goal_hvs_map.get(&(specified_game_id, specified_goal_id)) {
        Some(ghv) => ghv,
        None => {
            return Err(anyhow!("Unable to create a hypervector for the specified goal."));
        }
    };

    let mut goal_dists = Vec::with_capacity(goal_hvs_map.len()-1);
    for ((other_game_id, other_goal_id), other_goal_hv) in &goal_hvs_map {
        if (*other_game_id == specified_game_id) && (*other_goal_id == specified_goal_id) {
            continue;
        }

        let goal_dist = dist(specified_goal_hv, &other_goal_hv);
        goal_dists.push(GoalDist { game_id: other_game_id.clone(), goal_id: other_goal_id.clone(), dist: goal_dist });
    }

    // sort the distances
    goal_dists.sort_by_key(|goal_dist| goal_dist.dist);

    // print the 2 closest goals
    println!("*******************************");
    println!("*********** Results ***********");
    println!("*******************************");

    println!("2 closest goals to {} {}:", specified_game_id.0, specified_goal_id.0);

    let first_ppt_url = format!("https://www.nhl.com/ppt-replay/goal/{}/{}", goal_dists[0].game_id.0, goal_dists[0].goal_id.0);
    println!(" --- 1. Game {} Goal {}", goal_dists[0].game_id.0, goal_dists[0].goal_id.0);
    println!("        URL: {first_ppt_url}");

    let second_ppt_url = format!("https://www.nhl.com/ppt-replay/goal/{}/{}", goal_dists[1].game_id.0, goal_dists[1].goal_id.0);
    println!(" --- 2. Game {} Goal {}", goal_dists[1].game_id.0, goal_dists[1].goal_id.0);
    println!("        URL: {second_ppt_url}");

    Ok(())
}

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    #[arg(long)]
    game: String,

    #[arg(long)]
    goal: String,

    /// folder to read location JSON's from
    #[arg(long)]
    input_dir: Option<String>,

    /// folder to export goal hypervecs, grid level hypervecs, and permutation arrays to
    #[arg(long)]
    output_dir: Option<String>,

    /// if true export the goal hypervecs
    #[arg(long, requires = "output_dir")]
    export_hvs: bool,

    /// if true export the grid level hypervecs and the permuation arrays
    /// requires exporting goal hypervecs as well since grid level hypervecs
    /// and permutation arrays need to be coupled with them
    #[arg(long, requires = "output_dir", requires = "export_hvs")]
    export_grid_perm: bool,

    /// directory to import goal hypervecs, grid level hypervecs, and permutation arrays from
    #[arg(long)]
    import_dir: Option<String>,

    /// if true imports goal hypervecs, grid level hypervecs, and permutation arrays from
    /// the import directory specified by the import-dir option
    #[arg(long, requires = "import_dir")]
    import_info: bool,

    /// if true exports the pre-processed location data to the given file
    /// won't export if you are importing goal hypervecs, grid level hypervecs,
    /// and permutataion arrays
    #[arg(long)]
    export_pp_loc_file: Option<String>,

    // if set, takes the last n instances of each goal
    #[arg(long)]
    last_n: Option<usize>
}

/// helper type for keeping track of goal distances
#[derive(Debug)]
struct GoalDist {
    game_id: GameId,
    goal_id: GoalId,
    dist: u32,
}

const GRID_HV_OUTPUT_NAME: &str = "grid_hvs.npy";
const PERM_ARRAYS_OUTPUT_NAME: &str = "perm_arrays.txt";
const GOAL_HVS_OUTPUT_NAME: &str = "goal_hvs.parquet";

/// exports the generated goal hypervecs and optionally the grid level hypervecs
/// and the perm arrays
/// output_dir is the directory where the files get exported to
/// if export_grid_and_perm is true, exports the grid level hypervecs and the perm arrays
///     The grid level hypervecs get saved as `grid_hvs.npy`.
///     The perm arrays get saved as `perm_arrays.txt`.
/// Might not need grid level hypervecs and perm arrays if already have them exported
/// goal_hvs, game_ids, and goal_ids need to be in the same order
///     The goal hypervecs are samed as `goal_hvs.parquet`.
fn export_hv_info<P: AsRef<Path> + Display>(
    output_dir: P, 
    export_grid_and_perm: bool, 
    goal_hvs: &[Series],
    grid_hvs: &Array<bool, Dim<[usize; 3]>>,
    game_ids: &[u32],
    goal_ids: &[u32],
    perm_arrays:&[Vec<usize>],
) -> Result<()> {
    // handle grid level hypervecs and permutation arrays
    if export_grid_and_perm {
        write_npy(format!("{output_dir}/{GRID_HV_OUTPUT_NAME}"), grid_hvs)?;
        let perm_str = serde_json::to_string(perm_arrays)?;
        fs::write(format!("{output_dir}/{PERM_ARRAYS_OUTPUT_NAME}"), perm_str)?;
    }

    // export goal hypervecs to a parquet file by building a dataframe of
    // game id's, goal id's, and goal hypervecs
    let mut export_df = df!(
        "game_id" => game_ids,
        "goal_id" => goal_ids,
        "goal_hv" => goal_hvs
    )?;
    let mut file = std::fs::File::create(format!("{output_dir}/{GOAL_HVS_OUTPUT_NAME}"))?;
    ParquetWriter::new(&mut file).finish(&mut export_df)?;
    Ok(())
}

/// Imports goal hypervecs, grid level hypervecs, and perm arrays
fn import_hv_info<P: AsRef<Path> + Display>(
    import_dir: P,
) -> Result<(Array<bool, Dim<[usize; 3]>>, Vec<Vec<usize>>, HashMap<(GameId, GoalId), Array<bool, Dim<[usize; 1]>>>)> {
    let grid_hvs: Array<bool, Dim<[usize; 3]>> = read_npy(format!("{import_dir}/{GRID_HV_OUTPUT_NAME}"))?;

    let perm_arrays_file = fs::File::open(format!("{import_dir}/{PERM_ARRAYS_OUTPUT_NAME}"))?;
    let perm_arrays_reader = BufReader::new(perm_arrays_file);
    let perm_arrays: Vec<Vec<usize>> = serde_json::from_reader(perm_arrays_reader)?;

    let goal_hvs_file = fs::File::open(format!("{import_dir}/{GOAL_HVS_OUTPUT_NAME}"))?;
    let goal_hvs_reader = ParquetReader::new(goal_hvs_file);
    let goal_hvs_df = goal_hvs_reader.finish()?;

    // convert the goal hv dataframe into a hash map that maps game and goal
    // id's to goal hv's
    let mut goal_hvs_map = HashMap::with_capacity(goal_hvs_df.height());
    for row_index in 0..goal_hvs_df.height() {
        let row = goal_hvs_df.get_row(row_index)?;
        let game_id = match row.0[0] {
            AnyValue::UInt32(id) => id,
            _ => continue,
        };
        
        let goal_id = match row.0[1] {
            AnyValue::UInt32(id) => id,
            _ => continue,
        };
        let goal_hv: Vec<bool> = match row.0[2].clone() { 
            AnyValue::List(s) => s.bool().unwrap().into_no_null_iter().collect(),
            _ => continue,
        };

        goal_hvs_map.insert((GameId(game_id), GoalId(goal_id)), Array::from_vec(goal_hv));
    }

    Ok((grid_hvs, perm_arrays, goal_hvs_map))
}

/// Exports the pre-processed goal data to a parquet file
/// The x and y coordinates have separate columns.
fn export_pp_loc_data<P: AsRef<Path> + Display>(
    output_file: P,
    preprocessed_data: &HashMap<(GameId, GoalId), TrimmedPuckLocations>
) -> Result<()> {
    // set up columns that we'll use for exporting as one big dataframe
    let mut game_ids_for_export = Vec::with_capacity(preprocessed_data.len());
    let mut goal_ids_for_export = Vec::with_capacity(preprocessed_data.len());
    let mut pp_goal_data_x_for_export = Vec::with_capacity(preprocessed_data.len()); // x coordinates
    let mut pp_goal_data_y_for_export = Vec::with_capacity(preprocessed_data.len()); // y coordinates

    for ((game_id, goal_id), trimmed_goal) in preprocessed_data {
        game_ids_for_export.push(game_id.0);
        goal_ids_for_export.push(goal_id.0);

        // get the x and y coordinates
        let mut x_coords_single_goal = Vec::with_capacity(trimmed_goal.coords.len());
        let mut y_coords_single_goal = Vec::with_capacity(trimmed_goal.coords.len());

        for coord in &trimmed_goal.coords {
            x_coords_single_goal.push(coord.x);
            y_coords_single_goal.push(coord.y);
        }
        pp_goal_data_x_for_export.push(x_coords_single_goal.iter().collect::<Series>());
        pp_goal_data_y_for_export.push(y_coords_single_goal.iter().collect::<Series>());
    }

    // export pre-processed goal location data to a parquet file by building a dataframe of
    // game id's, goal id's, x coordinates, and y coordinates
    let mut export_df = df!(
        "game_id" => game_ids_for_export,
        "goal_id" => goal_ids_for_export,
        "x_coordinates" => pp_goal_data_x_for_export,
        "y_coordinates" => pp_goal_data_y_for_export
    )?;
    let mut file = std::fs::File::create(format!("{output_file}"))?;
    ParquetWriter::new(&mut file).finish(&mut export_df)?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use ndarray::array;

    // --------------------------------------------------
    // import_hv_info() tests
    // --------------------------------------------------
    #[test]
    fn import_hv_info_valid() {
        let import_dir = "test_data/import_tests/valid_info/"; 
        let (grid_hvs, perm_arrays, goal_hvs_map) = import_hv_info(import_dir).unwrap();

        // check the contents of everything we just imported
        let expected_grid_hvs = array![
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
        assert_eq!(grid_hvs, expected_grid_hvs);

        let expected_perm_arrays = vec![
            vec![0, 1, 2, 4, 3],
            vec![2, 3, 4, 0, 1],
            vec![1, 4, 2, 3, 0]
        ];
        assert_eq!(perm_arrays, expected_perm_arrays);

        assert_eq!(goal_hvs_map.len(), 2);
        let first_goal_hv = goal_hvs_map.get(&(GameId(2024020080), GoalId(899))).unwrap();
        let expected_first_goal_hv = array![true, false, false, true, true];
        assert_eq!(first_goal_hv, expected_first_goal_hv);

        let second_goal_hv = goal_hvs_map.get(&(GameId(2024020081), GoalId(231))).unwrap();
        let expected_second_goal_hv = array![false, false, false, true, false];
        assert_eq!(second_goal_hv, expected_second_goal_hv);
    }
}