// preprocessing.rs
// Loads and pre-processes raw goal location data by:
//      - Rotating as necessary
//      - Trimming as necessary
use anyhow::{anyhow, Result};

use serde::Deserialize;
use serde_json::{Value};

use std::collections::HashMap;
use std::fmt::Display;
use std::fs::{File, read_dir};
use std::io::BufReader;
use std::path::Path;

use clamped_coord::{clamp_coord, ClampedCoord};

// max values for coordinates
const MAX_X: f64 = 2400.;
const MAX_Y: f64 = 1015.;

// ------------------------------------
// structs for deserializing the location JSON files
// ------------------------------------
#[derive(Deserialize, Debug)]
struct Instance {
    onIce: HashMap<String, Value>
    // onIce: HashMap<String, PlayerData>
    // onIce: HashMap<String, Player>

}

// #[derive(Deserialize, Debug)]
// struct Player {
//     id: u16,
//     x: f32,
//     y: f32
// }

// #[derive(Deserialize, Debug)]
// enum PlayerData {
//     Exists { id: u16, x: f32, y: f32 },
//     Empty
// }

#[derive(Debug, Clone, PartialEq)]
pub struct Coord {
    pub x: f64,
    pub y: f64
}


#[derive(Debug)]
pub struct RawPuckLocationData {
    coords: Vec<Option<Coord>>
}

pub mod clamped_coord {
    use super::{Coord, MAX_X, MAX_Y};

    /// Helper type that's guaranteed to be within the range of valid
    /// coords
    #[derive(Debug)]
    pub struct ClampedCoord {
        x: f64,
        y: f64,
    }

    // Create getters for x and y because don't want to make them public
    // since that could invalidate the constraint of them being valid coords
    impl ClampedCoord {
        pub fn get_x(&self) -> f64 {
            (*self).x
        }

        pub fn get_y(&self) -> f64 {
            (*self).y
        }
    }

    /// Clamps a coordinate to the min and max x and y coordinates
    pub fn clamp_coord(c: &Coord) -> ClampedCoord {
        let x_pos = c.x.min(MAX_X);
        let x_pos = x_pos.max(0.);

        let y_pos = c.y.min(MAX_Y);
        let y_pos = y_pos.max(0.);

        ClampedCoord { x: x_pos, y: y_pos }
    }
}

/// Reads in the goal location data and returns the puck location info
pub fn read_loc_data<P: AsRef<Path> + Display>(path: P) -> Result<RawPuckLocationData> {
    const PUCK_ID: &str = "1";

    let file = File::open(&path)?;
    let reader = BufReader::new(file);

    // println!("*** Reading goal data ***");
    let goal_loc_data: Vec<Instance> = serde_json::from_reader(reader)?;
    // println!("goal loc data: {:?}", goal_loc_data);

    // go through each instance and pull out just the puck data
    let mut all_puck_info = vec![];
    for instance in &goal_loc_data {
        // println!("********");
        let puck_data = instance.onIce.get(PUCK_ID);

        // do an extra check of the id just to be safe
        // let puck_data = match puck_data {
        //     Some(p) => {
        //         if p.id == 1 {
        //             Some(Coord { x: p.x, y: p.y })
        //         }
        //         else {
        //             None
        //         }
        //     },
        //     None => None
        // };
        let puck_coord = match puck_data {
            Some(o) => {
                // sometimes the puck's hash map will be empty
                // so we need to check if the x and y coordinates even exist
                // if they don't, then we should use a None for that instance's
                // puck data
                // need to have None instead of skipping that instance since
                // those instances will still be used when trimming the goal data
                if o.get("x").is_some() && o.get("y").is_some() {
                    // check that both x and y coordinates are floats just to
                    // be safe, and then extract then to create the puck's
                    // coordinate
                    if o["x"].is_number() && o["y"].is_number() {
                        let x = match o["x"].as_f64() {
                            Some(x) => x,
                            None => {
                                continue
                            }
                        };
                        let y = match o["y"].as_f64() {
                            Some(y) => y,
                            None => {
                                continue
                            }
                        };
                        Some(Coord { x, y })
                    // set puck info as None if x and y aren't floats
                    } else {
                        None
                    }
                // set puck info as None if x and y aren't in the puck's 
                // hash map (the hash map is probably empty in this case)
                }  else {
                    None
                }
            },
            // set puck info as None if the puck ID doesn't exist as a key
            // in this instance
            None => None
        };
        // println!("puck_data: {:?}", &puck_data);

        all_puck_info.push(puck_coord);
        
    }
    // println!("all puck data: {:?}", all_puck_info);
    if all_puck_info.len() == 0 {
        let err_msg = format!("Goal JSON file {} has no puck info", &path);
        Err(anyhow!(err_msg))
    // there are cases where a goal has player data but no puck data, so we need
    // to handle those too
    } else if all_puck_info.iter().all(|x| x.is_none()) {
        let err_msg = format!("Goal JSON file {} has no puck info", &path);
        Err(anyhow!(err_msg))
    } else {
        Ok(RawPuckLocationData { coords: all_puck_info })
    }
}


// --------------------------------------
// Structs for deserializing play-by-play data
// --------------------------------------
#[derive(Deserialize, Debug)]
pub struct PbpData {
    goals: Vec<GoalDetails>,
    home_team_id: u16
}

#[derive(Deserialize, Debug)]
struct GoalDetails {
    event_id: u32,
    ppt_replay_url: Option<String>, // could we use String since we don't save goals w/o ppt_replay_url?
    scoring_team_id: u16,
    home_team_defending_side: String,
}

/// Reads in play-by-play data, which has the scoring team id, home team 
/// defending side, and home team id
/// This info is needed to determine if a goal should be rotated or not.
fn read_pbp_data<P: AsRef<Path> + Display>(path: P) -> Result<PbpData> {
    let file = File::open(&path)?;
    let reader = BufReader::new(file);

    // let goal_loc_data: Value = serde_json::from_reader(reader)?;
    let pbp_data: PbpData = serde_json::from_reader(reader)?;
    Ok(pbp_data)
}

#[derive(Debug, PartialEq, Eq, Hash, Copy, Clone)]
pub struct GameId(pub u32);

#[derive(Debug, PartialEq, Eq, Hash, Copy, Clone)]
pub struct GoalId(pub u32);

/// Combines both goal location data and play-by-play data using two hash maps
/// Both maps are keyed by the game id
#[derive(Debug)]
pub struct PbpGoalLocData {
    pub goal_loc_data: HashMap<GameId, HashMap<GoalId, RawPuckLocationData>>,
    pub pbp_data: HashMap<GameId, PbpData>
}

/// Reads in all game data in a folder, both goal location and play-by-play data
pub fn read_folder<P: AsRef<Path> + Display>(dir: P, pbp_goal_data: &mut PbpGoalLocData) -> Result<()> {
    const GAME_ID_LEN: usize = 10; // game id's are always 10 numbers long

    if dir.as_ref().is_dir() {
        for entry in read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();
            let path_stem = match path.file_stem() {
                Some(s) => s,
                None => {
                    continue
                }
            };
            // println!("path: {:?}, path stem: {:?}", &path, &path.file_stem());

            if path.is_dir() && (path_stem.len() == GAME_ID_LEN) {
                // check if the directory is all digits so we know it's a 
                // game director
                let game_id = match path_stem.to_string_lossy().parse::<u32>() {
                    Ok(id) => GameId(id),
                    Err(_) => {
                        read_folder(&path.to_str().ok_or(anyhow!("invalid path"))?, pbp_goal_data)?;
                        continue
                    }
                };
                // println!("game_id: {game_id}, path: {:?}", path);

                // since we know we are in a game directory, go through all
                // the files
                for game_file in read_dir(&path)? {                    
                    let game_file = game_file?;
                    let game_path = game_file.path();
                    
                    let file_name = match game_path.file_name() {
                        Some(s) => s.to_string_lossy(),
                        None => {
                            continue;
                        }
                    };
                    // read in the goal location data files
                    if file_name != "pbp_boxscore.json" {
                        // println!("------ Reading in {file_name} for game {:?}", game_id);
                        let goal_id = match game_path.file_stem() {
                            Some(id) => {
                                // get the goal id
                                match id.to_string_lossy().parse::<u32>() {
                                    Ok(goal_id) => GoalId(goal_id),
                                    Err(e) => {
                                        println!("*** Unable to parse the goal id from file {}: {e}", &game_path.to_string_lossy());
                                        continue
                                    }
                                }
                            }
                            None => {
                                println!("*** Unable to get file stem for {}", &game_path.to_string_lossy());
                                continue
                            }
                        };
                        let loc_data = match read_loc_data(&game_path.to_str().ok_or(anyhow!("invalid path"))?) {
                            Ok(ld) => ld,
                            Err(e) => {
                                println!("*** Error {e} when trying to parse goal data at {}", &game_path.to_string_lossy());
                                continue
                            }
                        };
                        // add goal location data
                        // pbp_goal_data.goal_loc_data.insert(game_id, loc_data);
                        match pbp_goal_data.goal_loc_data.get_mut(&game_id) {
                            Some(map) => {
                                map.insert(goal_id, loc_data);
                            },
                            None => {
                                // need to create a new map if it doesn't exist
                                let mut goals_map = HashMap::new();
                                goals_map.insert(goal_id, loc_data);
                                pbp_goal_data.goal_loc_data.insert(game_id, goals_map);
                            }
                        };
                    }
                    // read in the play-by-play files
                    else {
                        let pbp_data = match read_pbp_data(&game_path.to_str().ok_or(anyhow!("invalid path"))?) {
                            Ok(pd) => pd,
                            Err(e) => {
                                println!("*** Error {e} when trying to parse play-by-play data at {}", &game_path.to_string_lossy());
                                continue
                            }
                        };
                        pbp_goal_data.pbp_data.insert(game_id, pbp_data);
                    }
                }
                
            }
            else if path.is_dir() {
                // println!("keep going");
                read_folder(&path.to_str().ok_or(anyhow!("invalid path"))?, pbp_goal_data)?;
            }
        }
    } else {
        return Err(anyhow!("Can't find directory {}", dir));
    }
    // let entries = read_dir(&dir)?
    //     .collect::<Result<Vec<_>, io::Error>>()?;
    // println!("entries in {}: {:?}", dir, entries);
    Ok(())
}

#[derive(Debug)]
struct RotatedPuckLocations {
    pub coords: Vec<Option<Coord>>
}

/// Rotates a goal's coordinates by 180 degrees
// pub fn rotate_goal_coords(puck_locs: &RawPuckLocationData) -> RotatedPuckLocations {
fn rotate_goal_coords(puck_locs: Vec<Option<ClampedCoord>>) -> RotatedPuckLocations {
    const MAX_X: f64 = 2400.;
    const MAX_Y: f64 = 1015.;
    
    let mut rotated_coords = Vec::with_capacity(puck_locs.len());

    // for coord in &puck_locs.coords {
    //     match coord {
    //         Some(c) => {
    //             // let x_pos = c.x.min(MAX_X);
    //             // let x_pos = x_pos.max(0.);

    //             // let y_pos = c.y.min(MAX_Y);
    //             // let y_pos = y_pos.max(0.);
    //             let clamped_coord = clamp_coord(c);
    //             let rot_x_pos = MAX_X - clamped_coord.get_x();
    //             let rot_y_pos = MAX_Y - clamped_coord.get_y();
    //             let coord = Coord{ x: rot_x_pos, y: rot_y_pos };
                
    //             rotated_coords.push(Some(coord));
    //         },
    //         None => {
    //             rotated_coords.push(None)
    //         }
    //     };
        
    // }

    for coord in puck_locs {
        match coord {
            Some(c) => {
                let rot_x_pos = MAX_X - c.get_x();
                let rot_y_pos = MAX_Y - c.get_y();
                let coord = Coord{ x: rot_x_pos, y: rot_y_pos };
                
                rotated_coords.push(Some(coord));
            },
            None => {
                rotated_coords.push(None)
            }
        }
    }
    RotatedPuckLocations { coords: rotated_coords }
}


#[derive(Debug, PartialEq)]
pub struct TrimmedPuckLocations {
    pub coords: Vec<Coord>
}

/// Trim the tracking data for a goal, both at the start and at the end
/// All None's will get removed.
fn trim_goal_coords(goal: &RotatedPuckLocations) -> TrimmedPuckLocations {
    // used for trimming start
    const EPSILON: f64 = 30.0;
    const TRIM_START_EPSILON_X: f64 = 20.; // how many units an instance needs to be close to the first instance by to be considered to trim
    const TRIM_START_EPSILON_Y: f64 = 45.;
    const NUM_FIRST_X_CHECK: usize = 5; // how many of the first x coords to check to trim start
    const FACEOFF_THRESHOLD: f64 = 50.; // if puck is this many units close to a faceoff dot, considered in the faceoff dot
    
    const TOP_DEF_DOT_X: f64 = 370.;
    const TOP_DEF_DOT_Y: f64 = 220.;
    const BOTTOM_DEF_DOT_X: f64 = 370.;
    const BOTTOM_DEF_DOT_Y: f64 = 775.;

    const TOP_LEFT_NEUT_DOT_X: f64 = 950.;
    const TOP_LEFT_NEUT_DOT_Y: f64 = 220.;
    const BOTTOM_LEFT_NEUT_DOT_X: f64 = 950.;
    const BOTTOM_LEFT_NEUT_DOT_Y: f64 = 775.;

    const CENTER_DOT_X: f64 = 1200.;
    const CENTER_DOT_Y: f64 = 510.;

    const TOP_RIGHT_NEUT_DOT_X: f64 = 1440.;
    const TOP_RIGHT_NEUT_DOT_Y: f64 = 220.;
    const BOTTOM_RIGHT_NEUT_DOT_X: f64 = 1440.;
    const BOTTOM_RIGHT_NEUT_DOT_Y: f64 = 775.;

    const TOP_OFF_DOT_X: f64 = 2025.;
    const TOP_OFF_DOT_Y: f64 = 220.;
    const BOTTOM_OFF_DOT_X: f64 = 2025.;
    const BOTTOM_OFF_DOT_Y: f64 = 775.;    
    
    // used for trimming end
    const GOAL_TOP: f64 = 465.;
    const GOAL_BOTTOM: f64 = 550.;
    const GOAL_LEFT: f64 = 2245.;
    const GOAL_RIGHT: f64 = 2307.;

    // vars for trimming start
    let mut first_instance: &Option<Coord> = &None;
    let mut puck_stationary = true;
    let mut in_faceoff = false;
    let mut starting_offset = 0;
    let mut starting_pt = 0;
    
    // vars for trimming end
    let num_instances = goal.coords.len(); 
    // let mut earliest_instant_in_goal: &Option<usize> = &None;
    let mut earliest_perc_in_goal = None;
    let mut curr_interval_first_instant = None;
    let mut curr_interval_last_instant= None;
    let mut cutoff_pt = None;

    let mut trimmed_coords = vec![];
    for (instant_num, instance) in goal.coords.iter().enumerate() {
        if instance.is_none() {
            starting_offset += 1;

            // need to make sure the instant is contiguous w/
            // the starting None block to avoid moving up
            // the starting pt when there is a missing val
            // in the middle
            // also need to handle the case where first instance isn't None, but
            // the second instance is by using the starting offset
            if (instant_num == (starting_pt + 1)) && (starting_offset > 1) {
                starting_pt += 1;
            }
            continue;
        }
        let instance_bare = instance.as_ref().unwrap(); // instance is guaranteed to be Some here due to None check above

        // trim the start
        // get the first instance's coords
        // if the coords don't move about 50 or so units in the 
        // first 5 instances, check if the coords are near any of the faceoff dots
        if (instant_num - starting_offset) <= NUM_FIRST_X_CHECK {
            if first_instance.is_none() {
                first_instance = instance;
            } else {                
                let first_instance_bare = first_instance.as_ref().unwrap();
                // println!("x dists fr first instance: {}", (instance_bare.x - first_instance_bare.x).abs());
                // println!("y dists fr first instance: {}", (instance_bare.y - first_instance_bare.y).abs());
                if ((instance_bare.x - first_instance_bare.x).abs() > EPSILON) ||
                    ((instance_bare.y - first_instance_bare.y).abs() > EPSILON) {
                        puck_stationary = false;
                    }
            }

            // check to see if the puck starts near a faceoff dot
            if (((instance_bare.x - TOP_DEF_DOT_X).abs() <= FACEOFF_THRESHOLD) &&
                ((instance_bare.y - TOP_DEF_DOT_Y).abs() <= FACEOFF_THRESHOLD)) ||

                // bottom defensive dot
                (((instance_bare.x - BOTTOM_DEF_DOT_X).abs() <= FACEOFF_THRESHOLD) &&
                ((instance_bare.y - BOTTOM_DEF_DOT_Y).abs() <= FACEOFF_THRESHOLD)) ||
                
                // top left neutral zone dot
                (((instance_bare.x - TOP_LEFT_NEUT_DOT_X).abs() <= FACEOFF_THRESHOLD) &&
                ((instance_bare.y - TOP_LEFT_NEUT_DOT_Y).abs() <= FACEOFF_THRESHOLD)) ||
                
                // bottom left neutral zone dot
                (((instance_bare.x - BOTTOM_LEFT_NEUT_DOT_X).abs() <= FACEOFF_THRESHOLD) &&
                ((instance_bare.y - BOTTOM_LEFT_NEUT_DOT_Y).abs() <= FACEOFF_THRESHOLD)) ||

                // center ice dot
                (((instance_bare.x - CENTER_DOT_X).abs() <= FACEOFF_THRESHOLD) &&
                ((instance_bare.y - CENTER_DOT_Y).abs() <= FACEOFF_THRESHOLD)) ||
                
                // top right neutral zone dot
                (((instance_bare.x - TOP_RIGHT_NEUT_DOT_X).abs() <= FACEOFF_THRESHOLD) &&
                ((instance_bare.y - TOP_RIGHT_NEUT_DOT_Y).abs() <= FACEOFF_THRESHOLD)) || 
                
                // bottom right neutral zone dot
                (((instance_bare.x - BOTTOM_RIGHT_NEUT_DOT_X).abs() <= FACEOFF_THRESHOLD) &&
                ((instance_bare.y - BOTTOM_RIGHT_NEUT_DOT_Y).abs() <= FACEOFF_THRESHOLD)) ||
                
                // top offensive dot
                (((instance_bare.x - TOP_OFF_DOT_X).abs() <= FACEOFF_THRESHOLD) &&
                ((instance_bare.y - TOP_OFF_DOT_Y).abs() <= FACEOFF_THRESHOLD)) ||
                
                // bottom offensive dot
                (((instance_bare.x - BOTTOM_OFF_DOT_X).abs() <= FACEOFF_THRESHOLD) &&
                ((instance_bare.y - BOTTOM_OFF_DOT_Y).abs() <= FACEOFF_THRESHOLD)) 
            {
                // println!("instance_bare: {:?} in faceoff dot", instance_bare);
                in_faceoff = true;
            }
            
        }
        // println!("instance_bare: {:?}, puck_stationary: {puck_stationary}, in_faceoff: {in_faceoff}", instance_bare);
        if puck_stationary && !first_instance.is_none() && in_faceoff &&
            ((instance_bare.x - first_instance.as_ref().unwrap().x).abs() <= TRIM_START_EPSILON_X) &&
            ((instance_bare.y - first_instance.as_ref().unwrap().y).abs() <= TRIM_START_EPSILON_Y) 
        {
            // need to actually trim the start
        
            // need to find the cutoff pt by seeing when puck moves fr the initial
            // instance's coords
            // so if the puck is still close to the first instant, that will 
            // need to be trimmed
            starting_pt += 1;
            // println!("instance_bare: {:?} to be trimmed", instance_bare);
        }
        
        // trim the end
        if (instance_bare.x >= GOAL_LEFT) && (instance_bare.x <= GOAL_RIGHT) &&
            (instance_bare.y >= GOAL_TOP) && (instance_bare.y <= GOAL_BOTTOM) 
        {
            if earliest_perc_in_goal.is_none() {
                // earliest_instant_in_goal = &Some(instant_num);
                earliest_perc_in_goal = Some((instant_num as f64)/(num_instances as f64));
            }

            let earliest_perc_in_goal_bare = earliest_perc_in_goal.unwrap(); // guaranteed to be Some due to check above

            if (earliest_perc_in_goal_bare < 0.35) || 
                ((earliest_perc_in_goal_bare >= 0.35) && (earliest_perc_in_goal_bare < 0.4) && (num_instances < 200)) ||
                ((earliest_perc_in_goal_bare >= 0.4) && (earliest_perc_in_goal_bare < 0.6))
            {
                // for goals that enter the net area before 35% at the earliest,
                // use the latest interval where puck enters net as the cutoff pt
                // whether that latest interval is before 35% or not
                // but only take the latest interval befeore 60%
                // if enters after 60%, use the first interval that comes after 60%
                
                // 35-40%: if len is < 200
                // if enters b/w 35-40% and never again, then that is the cutoff pt
                // if enters b/w 35-40% but enters again later, then the later entry pt is the cutoff pt
                
                // use same logic for 40-60% 
                let perc = (instant_num as f64)/(num_instances as f64);
                if (perc >= 0.6) && curr_interval_first_instant.is_some() {
                    cutoff_pt = Some(curr_interval_first_instant.unwrap());
                    break
                } else if (perc >= 0.6) && curr_interval_first_instant.is_none() {
                    cutoff_pt = Some(instant_num);
                    break
                }

                // keep track of the current interval
                if curr_interval_first_instant.is_none() {
                    curr_interval_first_instant = Some(instant_num);
                    curr_interval_last_instant = Some(instant_num);
                    cutoff_pt = Some(instant_num);
                } else if curr_interval_last_instant.is_some() && (instant_num == curr_interval_last_instant.unwrap() + 1) {
                    curr_interval_last_instant = Some(instant_num);
                } else {
                    curr_interval_first_instant = None;
                }

            } else if ((earliest_perc_in_goal_bare >= 0.35) && (earliest_perc_in_goal_bare < 0.4) && (num_instances >= 200)) ||
                (earliest_perc_in_goal_bare >= 0.6)
            {
                // if enters b/w 35-40%, then that is the cutoff pt, regardless if it enters again
                cutoff_pt = Some(instant_num);
                break
            }
        // # if puck ends up not in the goal area anymore, have to make
        // # sure to reset the current interval info
        } else {
            curr_interval_first_instant = None;
        }
    }
    
    // cutoff_pt is None if puck never enters the net
    // in this case, we'll just still use the starting point we found
    // but not trim anything from the end
    // if cutoff_pt.is_none() {
    //     // need to filter out None's
    //     for coords in goal.coords[starting_pt..].iter() {
    //         match coords {
    //             Some(c) => {
    //                 trimmed_coords.push(c.clone());
    //             },
    //             None => ()
    //         };
    //     }
    //     // return TrimmedPuckLocations { coords: goal.coords[starting_pt..].iter().flatten().collect::<Vec<_>>() }
    // // trim the end case
    // } else {
    //     // need to filter out None's
    //     for coords in goal.coords[starting_pt..=cutoff_pt.unwrap()].iter() {
    //         match coords {
    //             Some(c) => {
    //                 trimmed_coords.push(c.clone());
    //             },
    //             None => ()
    //         }
    //     }
    // }

    let goal_iter;
    if cutoff_pt.is_none() {
        goal_iter = goal.coords[starting_pt..].iter();
    } else {
        goal_iter = goal.coords[starting_pt..=cutoff_pt.unwrap()].iter();
    }
    // println!("Starting point: {starting_pt}, cutoff pt: {:?}", cutoff_pt);
    for coords in goal_iter {
        match coords {
            Some(c) => {
                trimmed_coords.push(c.clone());
            },
            None => ()
        }
    }
    TrimmedPuckLocations { coords: trimmed_coords }
}

/// Rotates and trims all the goals in a folder
pub fn preprocess_folder_data(folder_data: &PbpGoalLocData) -> HashMap<(GameId, GoalId), TrimmedPuckLocations> {
    let mut trimmed_info = HashMap::new();

    for (game_id, pbp_data) in &folder_data.pbp_data {

        // use the game id to get all goal data for that game
        // and iterate through all those goals
        let goal_loc_data_map = match folder_data.goal_loc_data.get(game_id) {
            Some(map) => map,
            None => {
                println!("No goal files found for game {}", game_id.0);
                continue;
            }
        };
        let home_team_id = pbp_data.home_team_id;

        // have to first clamp, rotate, and then trim each goal's tracking data
        for goal_details in &pbp_data.goals {
            let scoring_team_id = goal_details.scoring_team_id;
            let home_team_defending_side = &goal_details.home_team_defending_side;
            let away_team_defending_side;
            let scoring_side;
            let rotated_goal;
            let raw_puck_loc = match goal_loc_data_map.get(&GoalId(goal_details.event_id)) {
                Some(r) => r,
                None => {
                    // println!("No goal location data for game {}, goal {}", game_id.0, goal_details.event_id);
                    continue;
                }
            };
        
            // clamp to valid min and max values
            let mut clamped_coords = Vec::with_capacity(raw_puck_loc.coords.len());
            for coord in &raw_puck_loc.coords {
                let clamped = match coord {
                    Some(c) => Some(clamp_coord(c)),
                    None => None
                };
                clamped_coords.push(clamped);
            }

            // determine if we need to rotate this goal
            if home_team_defending_side == "Left" {
                away_team_defending_side = String::from("Right");
            } else {
                away_team_defending_side = String::from("Left");
            }

            // home team scores
            if scoring_team_id == home_team_id {
                scoring_side = away_team_defending_side;
            // away team scores
            } else {
                scoring_side = home_team_defending_side.to_owned();
            }

            // need to rotate if the goal was scored on the left side of the ice
            if scoring_side == "Left" {
                rotated_goal = rotate_goal_coords(clamped_coords);
            } else {
                // rotated_goal = RotatedPuckLocations { coords: raw_puck_loc.coords.clone() };
                let mut coords = Vec::with_capacity(clamped_coords.len());
                for coord in clamped_coords {
                    match coord {
                        Some(c) => {
                            coords.push(Some(Coord { x: c.get_x(), y: c.get_y() }));
                        }
                        None => {
                            coords.push(None);
                        }
                    };
                }
                rotated_goal = RotatedPuckLocations { coords: coords };
            }

            // now trim the goal
            let trimmed_goal = trim_goal_coords(&rotated_goal);
            trimmed_info.insert((game_id.clone(), GoalId(goal_details.event_id)), trimmed_goal);
        }
    }
    trimmed_info
}

#[cfg(test)]
mod tests {
    use super::*;

    // --------------------------------------------------
    // clamp tests
    // --------------------------------------------------
    #[test]
    fn clamp_valid_coord() {
        let coord = Coord { x: 10., y: 1000. };
        let clamped = clamp_coord(&coord);

        assert_eq!(clamped.get_x(), 10.);
        assert_eq!(clamped.get_y(), 1000.);
    }

    #[test]
    fn clamp_oob_below_coord() {
        let coord = Coord { x: -1., y: -10.};
        let clamped = clamp_coord(&coord);

        assert_eq!(clamped.get_x(), 0.);
        assert_eq!(clamped.get_y(), 0.);
    }

    #[test]
    fn clamp_oob_above_coord() {
        let coord = Coord { x: 2500., y: 1022.};
        let clamped = clamp_coord(&coord);

        assert_eq!(clamped.get_x(), MAX_X);
        assert_eq!(clamped.get_y(), MAX_Y);
    }

    #[test]
    fn clamp_oob_mix_coord() {
        let coord = Coord { x: -1., y: 1022.};
        let clamped = clamp_coord(&coord);

        assert_eq!(clamped.get_x(), 0.);
        assert_eq!(clamped.get_y(), MAX_Y);
    }

    #[test]
    fn clamp_oob_mix_2_coord() {
        let coord = Coord { x: 2410., y: -1.};
        let clamped = clamp_coord(&coord);

        assert_eq!(clamped.get_x(), MAX_X);
        assert_eq!(clamped.get_y(), 0.);
    }

    #[test]
    fn clamp_oob_and_valid_coord() {
        let coord = Coord { x: -1., y: 708.};
        let clamped = clamp_coord(&coord);

        assert_eq!(clamped.get_x(), 0.);
        assert_eq!(clamped.get_y(), 708.);
    }

    #[test]
    fn clamp_oob_and_valid_2_coord() {
        let coord = Coord { x: 1400., y: 1030.};
        let clamped = clamp_coord(&coord);

        assert_eq!(clamped.get_x(), 1400.);
        assert_eq!(clamped.get_y(), MAX_Y);
    }

    // --------------------------------------------------
    // read_loc_data() tests 
    // --------------------------------------------------
    
    // a JSON file that's empty should result in an error
    #[test]
    #[should_panic]
    fn read_loc_data_empty_json() {
        let test_path = "test_data/goal_loc_empty.json";
        let _ = read_loc_data(&test_path).unwrap();
    }

    // a JSON file that is just "[]" should result in an error
    #[test]
    #[should_panic]
    fn read_loc_data_empty_list() {
        let test_path = "test_data/goal_loc_empty_list.json";
        let _ = read_loc_data(&test_path).unwrap();
    }

    // a JSON file that has player but no puck info should result in an error
    #[test]
    #[should_panic]
    fn read_loc_data_no_puck_data() {
        let test_path = "test_data/goal_loc_no_puck_data.json";
        let _ = read_loc_data(&test_path).unwrap();
    }

    // a JSON file that has no instances without puck data
    #[test]
    fn read_loc_data_no_missing() {
        let test_path = "test_data/goal_loc_all_instances_has_puck_data.json";
        let puck_data = read_loc_data(&test_path).unwrap();

        assert_eq!(puck_data.coords.len(), 2);
        let first_coord = puck_data.coords[0].clone().unwrap();
        assert_eq!(first_coord.x, 666.7315);
        assert_eq!(first_coord.y, 394.3952);

        let second_coord = puck_data.coords[1].clone().unwrap();
        assert_eq!(second_coord.x, 670.7315);
        assert_eq!(second_coord.y, 397.3952);
    }

    // a JSON file that has missing puck data at the start
    #[test]
    fn read_loc_data_missing_start() {
        let test_path = "test_data/goal_loc_no_puck_data_start.json";
        let puck_data = read_loc_data(&test_path).unwrap();
        
        assert_eq!(puck_data.coords.len(), 4);
        let first_coord = puck_data.coords[0].clone();
        assert!(first_coord.is_none());

        let second_coord = puck_data.coords[1].clone();
        assert!(second_coord.is_none());

        let third_coord = puck_data.coords[2].clone().unwrap();
        assert_eq!(third_coord.x, 670.7315);
        assert_eq!(third_coord.y, 397.3952);

        let fourth_coord = puck_data.coords[3].clone().unwrap();
        assert_eq!(fourth_coord.x, 671.7315);
        assert_eq!(fourth_coord.y, 398.3952);
    }

    // a JSON file that has missing puck data at the end
    #[test]
    fn read_loc_data_missing_end() {
        let test_path = "test_data/goal_loc_no_puck_data_end.json";
        let puck_data = read_loc_data(&test_path).unwrap();
        
        assert_eq!(puck_data.coords.len(), 5);
        let first_coord = puck_data.coords[0].clone().unwrap();
        assert_eq!(first_coord.x, 440.1);
        assert_eq!(first_coord.y, 591.2114);

        let second_coord = puck_data.coords[1].clone().unwrap();
        assert_eq!(second_coord.x, 2215.1211);
        assert_eq!(second_coord.y, 592.8444);

        let third_coord = puck_data.coords[2].clone();
        assert!(third_coord.is_none());

        let fourth_coord = puck_data.coords[3].clone();
        assert!(fourth_coord.is_none());

        let fifth_coord = puck_data.coords[4].clone();
        assert!(fifth_coord.is_none());

    }

    // a JSON file that has missing puck data in the middle
    #[test]
    fn read_loc_data_missing_middle() {
        let test_path = "test_data/goal_loc_no_puck_data_middle.json";
        let puck_data = read_loc_data(&test_path).unwrap();
        
        assert_eq!(puck_data.coords.len(), 5);
        let first_coord = puck_data.coords[0].clone().unwrap();
        assert_eq!(first_coord.x, 440.1);
        assert_eq!(first_coord.y, 591.2114);

        let second_coord = puck_data.coords[1].clone().unwrap();
        assert_eq!(second_coord.x, 2215.1211);
        assert_eq!(second_coord.y, 592.8444);

        let third_coord = puck_data.coords[2].clone();
        assert!(third_coord.is_none());

        let fourth_coord = puck_data.coords[3].clone();
        assert!(fourth_coord.is_none());

        let fifth_coord = puck_data.coords[4].clone().unwrap();
        assert_eq!(fifth_coord.x, 2216.1211);
        assert_eq!(fifth_coord.y, 593.8444);
    }    

    // a JSON file that has all instances where the puck ID exists but there
    // is no puck information for any of those instances should return an error
    #[test]
    #[should_panic]
    fn read_loc_data_all_puck_empty() {
        let test_path = "test_data/goal_loc_all_puck_info_empty.json";
        let _ = read_loc_data(&test_path).unwrap();
    }

    // a JSON file that has instances at the start where puck ID exists but there
    // is no puck info should include those as None
    #[test]
    fn read_loc_data_start_puck_empty() {
        let test_path = "test_data/goal_loc_start_puck_info_empty.json";
        let puck_data = read_loc_data(&test_path).unwrap();
        
        assert_eq!(puck_data.coords.len(), 5);
        let first_coord = puck_data.coords[0].clone();
        assert!(first_coord.is_none());

        let second_coord = puck_data.coords[1].clone();
        assert!(second_coord.is_none());

        let third_coord = puck_data.coords[2].clone();
        assert!(third_coord.is_none());

        let fourth_coord = puck_data.coords[3].clone().unwrap();
        assert_eq!(fourth_coord.x, 1140.52);
        assert_eq!(fourth_coord.y, 701.8541);

        let fifth_coord = puck_data.coords[4].clone().unwrap();
        assert_eq!(fifth_coord.x, 1145.52);
        assert_eq!(fifth_coord.y, 756.8541);        
    }

    // a JSON file that has instances at the end where puck ID exists but there
    // is no puck info should include those as None
    #[test]
    fn read_loc_data_end_puck_empty() {
        let test_path = "test_data/goal_loc_end_puck_info_empty.json";
        let puck_data = read_loc_data(&test_path).unwrap();
        
        assert_eq!(puck_data.coords.len(), 5);
        let first_coord = puck_data.coords[0].clone().unwrap();
        assert_eq!(first_coord.x, 1145.52);
        assert_eq!(first_coord.y, 756.8541);

        let second_coord = puck_data.coords[1].clone().unwrap();
        assert_eq!(second_coord.x, 1140.52);
        assert_eq!(second_coord.y, 701.8541);

        let third_coord = puck_data.coords[2].clone();
        assert!(third_coord.is_none());

        let fourth_coord = puck_data.coords[3].clone();
        assert!(fourth_coord.is_none());

        let fifth_coord = puck_data.coords[4].clone();
        assert!(fifth_coord.is_none());
    }

    // a JSON file that has instances in the middle where puck ID exists but there
    // is no puck info should include those as None
    #[test]
    fn read_loc_data_middle_puck_empty() {
        let test_path = "test_data/goal_loc_middle_puck_info_empty.json";
        let puck_data = read_loc_data(&test_path).unwrap();
        
        assert_eq!(puck_data.coords.len(), 5);
        let first_coord = puck_data.coords[0].clone().unwrap();
        assert_eq!(first_coord.x, 1145.52);
        assert_eq!(first_coord.y, 756.8541);

        let second_coord = puck_data.coords[1].clone().unwrap();
        assert_eq!(second_coord.x, 1140.52);
        assert_eq!(second_coord.y, 701.8541);

        let third_coord = puck_data.coords[2].clone();
        assert!(third_coord.is_none());

        let fourth_coord = puck_data.coords[3].clone();
        assert!(fourth_coord.is_none());
        println!("{:?}", puck_data.coords[4].clone());
        let fifth_coord = puck_data.coords[4].clone().unwrap();
        assert_eq!(fifth_coord.x, 540.5210);
        assert_eq!(fifth_coord.y, 305.);
    }

    // --------------------------------------------------
    // read_pbp_data() tests
    // --------------------------------------------------

    // play-by-play JSON with goals
    #[test]
    fn read_pbp_data_goals() {
        let test_path = "test_data/pbp_goals.json";
        let pbp_data = read_pbp_data(&test_path).unwrap();

        assert_eq!(pbp_data.goals.len(), 6);
        assert_eq!(pbp_data.goals[0].event_id, 332);
        assert_eq!(pbp_data.goals[1].event_id, 369);
        assert_eq!(pbp_data.goals[2].event_id, 73);
        assert_eq!(pbp_data.goals[3].event_id, 418);
        assert_eq!(pbp_data.goals[4].event_id, 165);
        assert_eq!(pbp_data.goals[5].event_id, 1042);

        assert_eq!(pbp_data.goals[0].home_team_defending_side, "Left");
        assert_eq!(pbp_data.goals[1].home_team_defending_side, "Left");
        assert_eq!(pbp_data.goals[2].home_team_defending_side, "Left");
        assert_eq!(pbp_data.goals[3].home_team_defending_side, "Right");
        assert_eq!(pbp_data.goals[4].home_team_defending_side, "Left");
        assert_eq!(pbp_data.goals[5].home_team_defending_side, "Left");

        assert_eq!(pbp_data.home_team_id, 2);
    }

    // play-by-play JSON without goals
    #[test]
    fn read_pbp_data_no_goals() {
        let test_path = "test_data/pbp_no_goals.json";
        let pbp_data = read_pbp_data(&test_path).unwrap();

        assert_eq!(pbp_data.goals.len(), 0);
        assert_eq!(pbp_data.home_team_id, 3);
    }

    // --------------------------------------------------
    // read_folder() tests
    // --------------------------------------------------

    // read in a folder where a game has some missing puck data for some goals
    #[test]
    fn read_folder_some_missing_puck_data() {
        let test_folder = "test_data/test_folder";
        let mut test_data = PbpGoalLocData {
            goal_loc_data: HashMap::new(),
            pbp_data: HashMap::new()
        };
        read_folder(&test_folder, &mut test_data).unwrap();

        // check all games' data: 2024020011
        // play-by-play data
        let pbp_data = &test_data.pbp_data[&GameId(2024020011)];
        assert_eq!(pbp_data.goals.len(), 5);
        assert_eq!(pbp_data.goals[0].event_id, 255);
        assert_eq!(pbp_data.goals[1].event_id, 298);
        assert_eq!(pbp_data.goals[2].event_id, 415);
        assert_eq!(pbp_data.goals[3].event_id, 428);
        assert_eq!(pbp_data.goals[4].event_id, 453);
        assert_eq!(pbp_data.home_team_id, 6);

        assert_eq!(test_data.goal_loc_data[&GameId(2024020011)].len(), 5);

        // game 2024020011, goal 255
        // this goal has no puck data, so it should not be in the game's
        // goal data
        assert!(!test_data.goal_loc_data[&GameId(2024020011)].contains_key(&GoalId(255)));
        
        // game 2024020011, goal 298
        // this goal has puck data for every instance
        let goal_298 = &test_data.goal_loc_data[&GameId(2024020011)][&GoalId(298)];
        assert_eq!(goal_298.coords.len(), 5);
        
        let first_coord = goal_298.coords[0].clone().unwrap();
        assert_eq!(first_coord.x, 1145.52);
        assert_eq!(first_coord.y, 756.8541);
        
        let second_coord = goal_298.coords[1].clone().unwrap();
        assert_eq!(second_coord.x, 1146.);
        assert_eq!(second_coord.y, 757.8541);

        let third_coord = goal_298.coords[2].clone().unwrap();
        assert_eq!(third_coord.x, 1147.81);
        assert_eq!(third_coord.y, 758.8541);

        let fourth_coord = goal_298.coords[3].clone().unwrap();
        assert_eq!(fourth_coord.x, 1204.3541);
        assert_eq!(fourth_coord.y, 759.8541);

        let fifth_coord = goal_298.coords[4].clone().unwrap();
        assert_eq!(fifth_coord.x, 1145.52);
        assert_eq!(fifth_coord.y, 777.8541);

        // game 2024020011, goal 415
        // this goal has some missing puck data at the start
        let goal_415 = &test_data.goal_loc_data[&GameId(2024020011)][&GoalId(415)];
        assert_eq!(goal_415.coords.len(), 6);
        
        let first_coord = goal_415.coords[0].clone();
        assert!(first_coord.is_none());
        
        let second_coord = goal_415.coords[1].clone();
        assert!(second_coord.is_none());

        let third_coord = goal_415.coords[2].clone();
        assert!(third_coord.is_none());

        let fourth_coord = goal_415.coords[3].clone().unwrap();
        assert_eq!(fourth_coord.x, 513.4667);
        assert_eq!(fourth_coord.y, 174.7402);

        let fifth_coord = goal_415.coords[4].clone().unwrap();
        assert_eq!(fifth_coord.x, 540.4667);
        assert_eq!(fifth_coord.y, 100.7402);

        let fifth_coord = goal_415.coords[4].clone().unwrap();
        assert_eq!(fifth_coord.x, 540.4667);
        assert_eq!(fifth_coord.y, 100.7402);

        let sixth_coord = goal_415.coords[5].clone().unwrap();
        assert_eq!(sixth_coord.x, 544.4667);
        assert_eq!(sixth_coord.y, 10.7402);

        // game 2024020011, goal 428
        // this goal has some missing puck data at the end
        let goal_428 = &test_data.goal_loc_data[&GameId(2024020011)][&GoalId(428)];
        assert_eq!(goal_428.coords.len(), 6);
        
        let first_coord = goal_428.coords[0].clone().unwrap();
        assert_eq!(first_coord.x, 513.4667);
        assert_eq!(first_coord.y, 174.7402);
        
        let second_coord = goal_428.coords[1].clone().unwrap();
        assert_eq!(second_coord.x, 523.4667);
        assert_eq!(second_coord.y, 164.7402);

        let third_coord = goal_428.coords[2].clone().unwrap();
        assert_eq!(third_coord.x, 533.4667);
        assert_eq!(third_coord.y, 154.7402);

        let fourth_coord = goal_428.coords[3].clone().unwrap();
        assert_eq!(fourth_coord.x, 543.4667);
        assert_eq!(fourth_coord.y, 144.7402);

        let fifth_coord = goal_428.coords[4].clone();
        assert!(fifth_coord.is_none());

        let sixth_coord = goal_428.coords[5].clone();
        assert!(sixth_coord.is_none());

        // game 2024020011, goal 453
        // this goal has some missing puck data in the middle
        let goal_453 = &test_data.goal_loc_data[&GameId(2024020011)][&GoalId(453)];
        assert_eq!(goal_453.coords.len(), 6);
        
        let first_coord = goal_453.coords[0].clone().unwrap();
        assert_eq!(first_coord.x, 513.4667);
        assert_eq!(first_coord.y, 174.7402);
        
        let second_coord = goal_453.coords[1].clone().unwrap();
        assert_eq!(second_coord.x, 523.4667);
        assert_eq!(second_coord.y, 164.7402);

        let third_coord = goal_453.coords[2].clone();
        assert!(third_coord.is_none());

        let fourth_coord = goal_453.coords[3].clone();
        assert!(fourth_coord.is_none());

        let fifth_coord = goal_453.coords[4].clone().unwrap();
        assert_eq!(fifth_coord.x, 543.4667);
        assert_eq!(fifth_coord.y, 144.7402, );

        let sixth_coord = goal_453.coords[5].clone().unwrap();
        assert_eq!(sixth_coord.x, 560.0);
        assert_eq!(sixth_coord.y, 700.0);

        // check all games' data: 2024020012
        // has no goals
        // play-by-play data
        let pbp_data = &test_data.pbp_data[&GameId(2024020012)];
        assert_eq!(pbp_data.goals.len(), 0);
        assert_eq!(pbp_data.home_team_id, 3);

        // since there are no goal location files for the game, 
        // the game has no map for the goal data
        assert!(!test_data.goal_loc_data.contains_key(&GameId(2024020012)));

        // check all games' data: 2024020013
        // has no goals
        // play-by-play data
        let pbp_data = &test_data.pbp_data[&GameId(2024020013)];
        assert_eq!(pbp_data.goals.len(), 3);
        assert_eq!(pbp_data.goals[0].event_id, 182);
        assert_eq!(pbp_data.goals[1].event_id, 311);
        assert_eq!(pbp_data.goals[2].event_id, 794);
        assert_eq!(pbp_data.home_team_id, 9);

        assert_eq!(test_data.goal_loc_data[&GameId(2024020013)].len(), 3);

        // game 2024020013, goal 182
        let goal_182 = &test_data.goal_loc_data[&GameId(2024020013)][&GoalId(182)];
        assert_eq!(goal_182.coords.len(), 4);
        
        let first_coord = goal_182.coords[0].clone().unwrap();
        assert_eq!(first_coord.x, 513.4667);
        assert_eq!(first_coord.y, 174.7402);
        
        let second_coord = goal_182.coords[1].clone().unwrap();
        assert_eq!(second_coord.x, 523.4667);
        assert_eq!(second_coord.y, 164.7402);

        let third_coord = goal_182.coords[2].clone().unwrap();
        assert_eq!(third_coord.x, 500.);
        assert_eq!(third_coord.y, 600.);

        let fourth_coord = goal_182.coords[3].clone().unwrap();
        assert_eq!(fourth_coord.x, 550.);
        assert_eq!(fourth_coord.y, 610.);

        // game 2024020013, goal 311
        let goal_311 = &test_data.goal_loc_data[&GameId(2024020013)][&GoalId(311)];
        assert_eq!(goal_311.coords.len(), 4);
        
        let first_coord = goal_311.coords[0].clone().unwrap();
        assert_eq!(first_coord.x, 1513.4667);
        assert_eq!(first_coord.y, 174.7402);
        
        let second_coord = goal_311.coords[1].clone().unwrap();
        assert_eq!(second_coord.x, 1523.4667);
        assert_eq!(second_coord.y, 164.7402);

        let third_coord = goal_311.coords[2].clone().unwrap();
        assert_eq!(third_coord.x, 1500.);
        assert_eq!(third_coord.y, 600.);

        let fourth_coord = goal_311.coords[3].clone().unwrap();
        assert_eq!(fourth_coord.x, 1550.);
        assert_eq!(fourth_coord.y, 610.);

        // game 2024020013, goal 794
        let goal_794 = &test_data.goal_loc_data[&GameId(2024020013)][&GoalId(794)];
        assert_eq!(goal_794.coords.len(), 4);
        
        let first_coord = goal_794.coords[0].clone().unwrap();
        assert_eq!(first_coord.x, 1513.4667);
        assert_eq!(first_coord.y, 17.7402);
        
        let second_coord = goal_794.coords[1].clone().unwrap();
        assert_eq!(second_coord.x, 1523.4667);
        assert_eq!(second_coord.y, 16.7402);

        let third_coord = goal_794.coords[2].clone().unwrap();
        assert_eq!(third_coord.x, 1500.);
        assert_eq!(third_coord.y, 60.);

        let fourth_coord = goal_794.coords[3].clone().unwrap();
        assert_eq!(fourth_coord.x, 1550.);
        assert_eq!(fourth_coord.y, 61.);
    }

    // --------------------------------------------------
    // rotate_goal_coords() tests
    // --------------------------------------------------

    // rotating a goal with all None coords should be all None
    #[test]
    fn rotate_goals_all_none() {
        let coords = vec![
            None,
            None,
            None,
        ];
        let puck_locs = RawPuckLocationData{ coords: coords };
        let mut clamped_coords = Vec::with_capacity(puck_locs.coords.len());
        for coord in &puck_locs.coords {
            let clamped = match coord {
                Some(c) => Some(clamp_coord(c)),
                None => None
            };
            clamped_coords.push(clamped);
        }        
        
        let rotated_coords = rotate_goal_coords(clamped_coords);
        assert_eq!(rotated_coords.coords.len(), puck_locs.coords.len());
        assert!(rotated_coords.coords.iter().all(|c| c.is_none()));
    }

    // rotating a goal with coords that all are Some
    #[test]
    fn rotate_goals_all_some() {
        let coords = vec![
            Some(Coord { x: 20., y: 400. }),
            Some(Coord{ x: 2300., y: 701. }),
            Some(Coord{ x: 1200., y: 507. }),
            Some(Coord{ x: 2400., y: 1015. }),
            Some(Coord{ x: 2500., y: 1020. }),
            Some(Coord{ x: -10., y: -20. }),
        ];
        let puck_locs = RawPuckLocationData{ coords: coords };
        let mut clamped_coords = Vec::with_capacity(puck_locs.coords.len());
        for coord in &puck_locs.coords {
            let clamped = match coord {
                Some(c) => Some(clamp_coord(c)),
                None => None
            };
            clamped_coords.push(clamped);
        }
        let rotated_coords = rotate_goal_coords(clamped_coords);
        assert_eq!(rotated_coords.coords.len(), puck_locs.coords.len());

        assert_eq!(rotated_coords.coords[0], Some(Coord{ x: 2380., y: 615. }));
        assert_eq!(rotated_coords.coords[1], Some(Coord{ x: 100., y: 314. }));
        assert_eq!(rotated_coords.coords[2], Some(Coord{ x: 1200., y: 508. }));
        assert_eq!(rotated_coords.coords[3], Some(Coord{ x: 0., y: 0. }));
        assert_eq!(rotated_coords.coords[4], Some(Coord{ x: 0., y: 0. }));
        assert_eq!(rotated_coords.coords[5], Some(Coord{ x: 2400., y: 1015. }));
    }

    // rotating a goal that has Some and None for its coords
    #[test]
    fn rotate_goals_some_none() {
        let coords = vec![
            None,
            None,
            None,
            Some(Coord { x: 20., y: 400. }),
            Some(Coord{ x: 2300., y: 701. }),
            Some(Coord{ x: 1200., y: 507. }),
            None,
            Some(Coord{ x: 2500., y: 1020. }),
            None,
        ];
        let puck_locs = RawPuckLocationData{ coords: coords };
        
        let mut clamped_coords = Vec::with_capacity(puck_locs.coords.len());
        for coord in &puck_locs.coords {
            let clamped = match coord {
                Some(c) => Some(clamp_coord(c)),
                None => None
            };
            clamped_coords.push(clamped);
        }
        let rotated_coords = rotate_goal_coords(clamped_coords);
        assert_eq!(rotated_coords.coords.len(), puck_locs.coords.len());

        assert!(rotated_coords.coords[0].is_none());
        assert!(rotated_coords.coords[1].is_none());
        assert!(rotated_coords.coords[2].is_none());
        assert_eq!(rotated_coords.coords[3], Some(Coord{ x: 2380., y: 615. }));
        assert_eq!(rotated_coords.coords[4], Some(Coord{ x: 100., y: 314. }));
        assert_eq!(rotated_coords.coords[5], Some(Coord{ x: 1200., y: 508. }));
        assert!(rotated_coords.coords[6].is_none());
        assert_eq!(rotated_coords.coords[7], Some(Coord{ x: 0., y: 0. }));
        assert!(rotated_coords.coords[8].is_none());
    }

    // --------------------------------------------------
    // trim_goal_coords() tests
    // --------------------------------------------------

    // goal that never enters the goal area shouldn't be trimmed
    #[test]
    fn trim_goal_never_enters_net() {
        let coords = vec![
            Some(Coord { x: 10., y: 11. }),
            Some(Coord { x: 11., y: 11. }),
            Some(Coord { x: 12., y: 11. }),
        ];
        let rot_goal = RotatedPuckLocations { coords };
        let trimmed = trim_goal_coords(&rot_goal);
        
        assert_eq!(trimmed.coords.len(), rot_goal.coords.len());
        assert_eq!(trimmed.coords[0].x, 10.);
        assert_eq!(trimmed.coords[0].y, 11.);

        assert_eq!(trimmed.coords[1].x, 11.);
        assert_eq!(trimmed.coords[1].y, 11.);
        
        assert_eq!(trimmed.coords[2].x, 12.);
        assert_eq!(trimmed.coords[2].y, 11.);
    }

    // goal that has None's at the start and doesn't enter the goal area
    // should have None's trimmed off the start but nothing trimmed off end
    #[test]
    fn trim_goal_nones_start_never_enters_net() {
        let coords = vec![
            None,
            None,
            None,
            Some(Coord { x: 10., y: 11. }),
            Some(Coord { x: 11., y: 11. }),
            Some(Coord { x: 12., y: 11. }),
        ];
        let rot_goal = RotatedPuckLocations { coords };
        let trimmed = trim_goal_coords(&rot_goal);

        assert_eq!(trimmed.coords.len(), 3);
        assert_eq!(trimmed.coords[0].x, 10.);
        assert_eq!(trimmed.coords[0].y, 11.);

        assert_eq!(trimmed.coords[1].x, 11.);
        assert_eq!(trimmed.coords[1].y, 11.);
        
        assert_eq!(trimmed.coords[2].x, 12.);
        assert_eq!(trimmed.coords[2].y, 11.);
    }

    // goal that has exactly one None at the start should have that None
    // trimmed off
    #[test]
    fn trim_goal_one_none_start_never_enters_net() {
        let coords = vec![
            None,
            Some(Coord { x: 10., y: 11. }),
            Some(Coord { x: 11., y: 11. }),
            Some(Coord { x: 12., y: 11. }),
        ];
        let rot_goal = RotatedPuckLocations { coords };
        let trimmed = trim_goal_coords(&rot_goal);

        assert_eq!(trimmed.coords.len(), 3);
        assert_eq!(trimmed.coords[0].x, 10.);
        assert_eq!(trimmed.coords[0].y, 11.);

        assert_eq!(trimmed.coords[1].x, 11.);
        assert_eq!(trimmed.coords[1].y, 11.);
        
        assert_eq!(trimmed.coords[2].x, 12.);
        assert_eq!(trimmed.coords[2].y, 11.);
    }

    // goal that has None's at the end and doesn't enter the goal area
    // should only have the None's trimmed off
    #[test]
    fn trim_goal_nones_end_never_enters_net() {
        let coords = vec![
            Some(Coord { x: 10., y: 11. }),
            Some(Coord { x: 11., y: 11. }),
            Some(Coord { x: 12., y: 11. }),
            None,
            None,
            None
        ];
        let rot_goal = RotatedPuckLocations { coords };
        let trimmed = trim_goal_coords(&rot_goal);

        assert_eq!(trimmed.coords.len(), 3);
        assert_eq!(trimmed.coords[0].x, 10.);
        assert_eq!(trimmed.coords[0].y, 11.);

        assert_eq!(trimmed.coords[1].x, 11.);
        assert_eq!(trimmed.coords[1].y, 11.);
        
        assert_eq!(trimmed.coords[2].x, 12.);
        assert_eq!(trimmed.coords[2].y, 11.);
    }

    // goal that has None's in the middle and doesn't enter the goal area
    // should only have the None's removed
    #[test]
    fn trim_goal_nones_middle_never_enters_net() {
        let coords = vec![
            Some(Coord { x: 10., y: 11. }),
            None,
            None,
            Some(Coord { x: 11., y: 11. }),
            None,
            Some(Coord { x: 12., y: 11. }),
            Some(Coord { x: 13., y: 11. }),
        ];
        let rot_goal = RotatedPuckLocations { coords };
        let trimmed = trim_goal_coords(&rot_goal);

        assert_eq!(trimmed.coords.len(), 4);
        assert_eq!(trimmed.coords[0].x, 10.);
        assert_eq!(trimmed.coords[0].y, 11.);

        assert_eq!(trimmed.coords[1].x, 11.);
        assert_eq!(trimmed.coords[1].y, 11.);
        
        assert_eq!(trimmed.coords[2].x, 12.);
        assert_eq!(trimmed.coords[2].y, 11.);

        assert_eq!(trimmed.coords[3].x, 13.);
        assert_eq!(trimmed.coords[3].y, 11.);
    }

    // goal that has None's in the beginning, middle, and end and doesn't
    // enter the goal area should only have the None's removed
    #[test]
    fn trim_goal_nones_everywhere_never_enters_net() {
        let coords = vec![
            None,        
            Some(Coord { x: 10., y: 11. }),
            None,
            None,
            Some(Coord { x: 11., y: 11. }),
            None,
            Some(Coord { x: 12., y: 11. }),
            Some(Coord { x: 13., y: 11. }),
            None,
            None,
            None,
        ];
        let rot_goal = RotatedPuckLocations { coords };
        let trimmed = trim_goal_coords(&rot_goal);

        assert_eq!(trimmed.coords.len(), 4);
        assert_eq!(trimmed.coords[0], Coord { x: 10., y: 11. });
        assert_eq!(trimmed.coords[1], Coord { x: 11., y: 11. });
        assert_eq!(trimmed.coords[2], Coord { x: 12., y: 11. });
        assert_eq!(trimmed.coords[3], Coord { x: 13., y: 11. });
    }

    // goal where the puck starts off in the top defensive faceoff dot should
    // have that part trimmed off
    #[test]
    fn trim_goal_faceoff_top_def_dot() {
        let coords = vec![       
            Some(Coord { x: 371., y: 201. }),
            Some(Coord { x: 371.1, y: 201.2 }),
            Some(Coord { x: 371.2, y: 201.1 }),
            Some(Coord { x: 371.3, y: 200.9 }),
            Some(Coord { x: 11., y: 11. }),
            Some(Coord { x: 12., y: 11. }),
            Some(Coord { x: 13., y: 11. }),
        ];
        let rot_goal = RotatedPuckLocations { coords };
        let trimmed = trim_goal_coords(&rot_goal);
        
        assert_eq!(trimmed.coords.len(), 3);        
        assert_eq!(trimmed.coords[0], Coord { x: 11., y: 11. });
        assert_eq!(trimmed.coords[1], Coord { x: 12., y: 11. });
        assert_eq!(trimmed.coords[2], Coord { x: 13., y: 11. });
    }

    // goal where the puck starts off in the bottom defensive faceoff dot should
    // have that part trimmed off
    #[test]
    fn trim_goal_faceoff_bottom_def_dot() {
        let coords = vec![       
            Some(Coord { x: 371., y: 774. }),
            Some(Coord { x: 371.1, y: 774.2 }),
            Some(Coord { x: 371.2, y: 774.2 }),
            Some(Coord { x: 371.3, y: 774.3 }),
            Some(Coord { x: 11., y: 11. }),
            Some(Coord { x: 12., y: 11. }),
            Some(Coord { x: 13., y: 11. }),
        ];
        let rot_goal = RotatedPuckLocations { coords };
        let trimmed = trim_goal_coords(&rot_goal);
        
        assert_eq!(trimmed.coords.len(), 3);        
        assert_eq!(trimmed.coords[0], Coord { x: 11., y: 11. });
        assert_eq!(trimmed.coords[1], Coord { x: 12., y: 11. });
        assert_eq!(trimmed.coords[2], Coord { x: 13., y: 11. });

    }

    // goal where the puck starts off in the top left neutral zone faceoff dot should
    // have that part trimmed off
    #[test]
    fn trim_goal_faceoff_top_left_neut_dot() {
        let coords = vec![       
            Some(Coord { x: 955., y: 210.2 }),
            Some(Coord { x: 955.4, y: 210.4 }),
            Some(Coord { x: 955.3, y: 210.3 }),
            Some(Coord { x: 955.2, y: 210.1 }),
            Some(Coord { x: 11., y: 11. }),
            Some(Coord { x: 12., y: 11. }),
            Some(Coord { x: 13., y: 11. }),
        ];
        let rot_goal = RotatedPuckLocations { coords };
        let trimmed = trim_goal_coords(&rot_goal);
        
        assert_eq!(trimmed.coords.len(), 3);
        // assert_eq!(trimmed.coords[0], Coord { x: 955.2, y: 210.1 });
        assert_eq!(trimmed.coords[0], Coord { x: 11., y: 11. });
        assert_eq!(trimmed.coords[1], Coord { x: 12., y: 11. });
        assert_eq!(trimmed.coords[2], Coord { x: 13., y: 11. });
    }

    // goal where the puck starts off in the bottom left neutral zone faceoff dot should
    // have that part trimmed off
    #[test]
    fn trim_goal_faceoff_bottom_left_neut_dot() {
        let coords = vec![       
            Some(Coord { x: 955., y: 778.5 }),
            Some(Coord { x: 955.4, y: 778.1 }),
            Some(Coord { x: 955.3, y: 778. }),
            Some(Coord { x: 955.2, y: 778.6 }),
            Some(Coord { x: 11., y: 11. }),
            Some(Coord { x: 12., y: 11. }),
            Some(Coord { x: 13., y: 11. }),
        ];
        let rot_goal = RotatedPuckLocations { coords };
        let trimmed = trim_goal_coords(&rot_goal);
        
        assert_eq!(trimmed.coords.len(), 3);
        // assert_eq!(trimmed.coords[0], Coord { x: 955.2, y: 778.6 });
        assert_eq!(trimmed.coords[0], Coord { x: 11., y: 11. });
        assert_eq!(trimmed.coords[1], Coord { x: 12., y: 11. });
        assert_eq!(trimmed.coords[2], Coord { x: 13., y: 11. });
    }

    // goal where the puck starts off in the center ice faceoff dot should
    // have that part trimmed off
    #[test]
    fn trim_goal_faceoff_center_ice_dot() {
        let coords = vec![       
            Some(Coord { x: 1210., y: 500.2 }),
            Some(Coord { x: 1209.6, y: 500.4 }),
            Some(Coord { x: 1209.7, y: 500.6 }),
            Some(Coord { x: 1210.4, y: 499.9 }),
            Some(Coord { x: 11., y: 11. }),
            Some(Coord { x: 12., y: 11. }),
            Some(Coord { x: 13., y: 11. }),
        ];
        let rot_goal = RotatedPuckLocations { coords };
        let trimmed = trim_goal_coords(&rot_goal);
        
        assert_eq!(trimmed.coords.len(), 3);        
        assert_eq!(trimmed.coords[0], Coord { x: 11., y: 11. });
        assert_eq!(trimmed.coords[1], Coord { x: 12., y: 11. });
        assert_eq!(trimmed.coords[2], Coord { x: 13., y: 11. });
    }

    // goal where the puck starts off in the top right neutral zone faceoff dot should
    // have that part trimmed off
    #[test]
    fn trim_goal_faceoff_top_right_neut_dot() {
        let coords = vec![       
            Some(Coord { x: 1410.5, y: 210.2 }),
            Some(Coord { x: 1410.6, y: 210.4 }),
            Some(Coord { x: 1410.7, y: 210.3 }),
            Some(Coord { x: 1410.8, y: 210.1 }),
            Some(Coord { x: 11., y: 11. }),
            Some(Coord { x: 12., y: 11. }),
            Some(Coord { x: 13., y: 11. }),
        ];
        let rot_goal = RotatedPuckLocations { coords };
        let trimmed = trim_goal_coords(&rot_goal);
        
        assert_eq!(trimmed.coords.len(), 3);        
        assert_eq!(trimmed.coords[0], Coord { x: 11., y: 11. });
        assert_eq!(trimmed.coords[1], Coord { x: 12., y: 11. });
        assert_eq!(trimmed.coords[2], Coord { x: 13., y: 11. });
    }

    // goal where the puck starts off in the bottom right neutral zone faceoff dot should
    // have that part trimmed off
    #[test]
    fn trim_goal_faceoff_bottom_right_neut_dot() {
        let coords = vec![       
            Some(Coord { x: 1410.5, y: 726.4 }),
            Some(Coord { x: 1410.6, y: 726.1 }),
            Some(Coord { x: 1410.7, y: 726.1 }),
            Some(Coord { x: 1410.8, y: 726.2 }),
            Some(Coord { x: 11., y: 11. }),
            Some(Coord { x: 12., y: 11. }),
            Some(Coord { x: 13., y: 11. }),
        ];
        let rot_goal = RotatedPuckLocations { coords };
        let trimmed = trim_goal_coords(&rot_goal);
        
        assert_eq!(trimmed.coords.len(), 3);
        assert_eq!(trimmed.coords[0], Coord { x: 11., y: 11. });
        assert_eq!(trimmed.coords[1], Coord { x: 12., y: 11. });
        assert_eq!(trimmed.coords[2], Coord { x: 13., y: 11. });
    }

    // goal where the puck starts off in the top offensive zone faceoff dot should
    // have that part trimmed off
    #[test]
    fn trim_goal_faceoff_top_offensive_dot() {
        let coords = vec![       
            Some(Coord { x: 2020.1, y: 210.2 }),
            Some(Coord { x: 2019.9, y: 210.4 }),
            Some(Coord { x: 2020.4, y: 210.3 }),
            Some(Coord { x: 2020.3, y: 210.1 }),
            Some(Coord { x: 11., y: 11. }),
            Some(Coord { x: 12., y: 11. }),
            Some(Coord { x: 13., y: 11. }),
        ];
        let rot_goal = RotatedPuckLocations { coords };
        let trimmed = trim_goal_coords(&rot_goal);
        
        assert_eq!(trimmed.coords.len(), 3);
        assert_eq!(trimmed.coords[0], Coord { x: 11., y: 11. });
        assert_eq!(trimmed.coords[1], Coord { x: 12., y: 11. });
        assert_eq!(trimmed.coords[2], Coord { x: 13., y: 11. });
    }

    // goal where the puck starts off in the bottom offensive zone faceoff dot should
    // have that part trimmed off
    #[test]
    fn trim_goal_faceoff_bottom_offensive_dot() {
        let coords = vec![       
            Some(Coord { x: 2020.1, y: 776. }),
            Some(Coord { x: 2019.9, y: 776.4 }),
            Some(Coord { x: 2020.4, y: 775.8 }),
            Some(Coord { x: 2020.3, y: 775.6 }),
            Some(Coord { x: 11., y: 11. }),
            Some(Coord { x: 12., y: 11. }),
            Some(Coord { x: 13., y: 11. }),
        ];
        let rot_goal = RotatedPuckLocations { coords };
        let trimmed = trim_goal_coords(&rot_goal);
        
        assert_eq!(trimmed.coords.len(), 3);
        assert_eq!(trimmed.coords[0], Coord { x: 11., y: 11. });
        assert_eq!(trimmed.coords[1], Coord { x: 12., y: 11. });
        assert_eq!(trimmed.coords[2], Coord { x: 13., y: 11. });
    }

    // goal where the puck is stationary at the start but isn't in a faceoff dot
    // should not be trimmed at the start
    #[test]
    fn trim_goal_stationary_not_dot() {
        let coords = vec![       
            Some(Coord { x: 62.81, y: 522.26 }),
            Some(Coord { x: 62.82, y: 522.27 }),
            Some(Coord { x: 62.83, y: 522.26 }),
            Some(Coord { x: 62.81, y: 522.25 }),
            Some(Coord { x: 62.82, y: 522.24 }),
            Some(Coord { x: 62.80, y: 522.26 }),
            Some(Coord { x: 13., y: 11. }),
        ];
        let rot_goal = RotatedPuckLocations { coords };
        let trimmed = trim_goal_coords(&rot_goal);
        
        assert_eq!(trimmed.coords.len(), 7);
        assert_eq!(trimmed.coords[0], Coord { x: 62.81, y: 522.26 });
        assert_eq!(trimmed.coords[1], Coord { x: 62.82, y: 522.27 });
        assert_eq!(trimmed.coords[2], Coord { x: 62.83, y: 522.26 });
        assert_eq!(trimmed.coords[3], Coord { x: 62.81, y: 522.25 });
        assert_eq!(trimmed.coords[4], Coord { x: 62.82, y: 522.24 });
        assert_eq!(trimmed.coords[5], Coord { x: 62.80, y: 522.26 });
        assert_eq!(trimmed.coords[6], Coord { x: 13., y: 11. });
    }

    // goal where puck enters goal before 35% at earliest should be trimmed there
    #[test]
    fn trim_goal_before_35_perc() {
        let coords = vec![       
            Some(Coord { x: 1010.5, y: 900.5 }),
            Some(Coord { x: 2019.9, y: 776.4 }),
            Some(Coord { x: 2250.4, y: 500.1444 }), // enters here and should be trimmed here
            Some(Coord { x: 2020.3, y: 775.6 }),
            Some(Coord { x: 11., y: 11. }),

            Some(Coord { x: 12., y: 11. }),
            Some(Coord { x: 13., y: 11. }),
            Some(Coord { x: 13., y: 11. }),
            Some(Coord { x: 13., y: 11. }),
            Some(Coord { x: 13., y: 11. }),
        ];
        let rot_goal = RotatedPuckLocations { coords };
        let trimmed = trim_goal_coords(&rot_goal);
        
        assert_eq!(trimmed.coords.len(), 3);
        assert_eq!(trimmed.coords[0], Coord { x: 1010.5, y: 900.5 });
        assert_eq!(trimmed.coords[1], Coord { x: 2019.9, y: 776.4 });
        assert_eq!(trimmed.coords[2], Coord { x: 2250.4, y: 500.1444 });
    }

    // goal where puck enters goal before 35% at earliest but again before 60%
    // should be trimmed when puck enters again
    #[test]
    fn trim_goal_before_35_before_60() {
        let coords = vec![       
            Some(Coord { x: 1010.5, y: 900.5 }),
            Some(Coord { x: 2019.9, y: 776.4 }),
            Some(Coord { x: 2250.4, y: 500.1444 }), // enters here
            Some(Coord { x: 2020.3, y: 775.6 }),
            Some(Coord { x: 2246.1, y: 545.3 }), // enters again here and should be trimmed here

            Some(Coord { x: 12., y: 11. }),
            Some(Coord { x: 13., y: 11. }),
            Some(Coord { x: 13., y: 11. }),
            Some(Coord { x: 13., y: 11. }),
            Some(Coord { x: 13., y: 11. }),
        ];
        let rot_goal = RotatedPuckLocations { coords };
        let trimmed = trim_goal_coords(&rot_goal);

        assert_eq!(trimmed.coords.len(), 5);
        assert_eq!(trimmed.coords[0], Coord { x: 1010.5, y: 900.5 });
        assert_eq!(trimmed.coords[1], Coord { x: 2019.9, y: 776.4 });
        assert_eq!(trimmed.coords[2], Coord { x: 2250.4, y: 500.1444 });
        assert_eq!(trimmed.coords[3], Coord { x: 2020.3, y: 775.6 });
        assert_eq!(trimmed.coords[4], Coord { x: 2246.1, y: 545.3 });
    }

    // goal where puck enters goal before 35% at earliest but again at 60%
    // should be trimmed when puck enters again
    #[test]
    fn trim_goal_before_35_after_60() {
        let coords = vec![       
            Some(Coord { x: 1010.5, y: 900.5 }),
            Some(Coord { x: 2019.9, y: 776.4 }),
            Some(Coord { x: 2250.4, y: 500.1444 }), // enters here
            Some(Coord { x: 2020.3, y: 775.6 }),
            Some(Coord { x: 2020.4, y: 776.6 }),

            Some(Coord { x: 2246.1, y: 545.3 }), // enters again here and should be trimmed here
            Some(Coord { x: 12., y: 11. }),
            Some(Coord { x: 13., y: 11. }),            
            Some(Coord { x: 13., y: 11. }),
            Some(Coord { x: 13., y: 11. }),
        ];
        let rot_goal = RotatedPuckLocations { coords };
        let trimmed = trim_goal_coords(&rot_goal);

        assert_eq!(trimmed.coords.len(), 6);
        assert_eq!(trimmed.coords[0], Coord { x: 1010.5, y: 900.5 });
        assert_eq!(trimmed.coords[1], Coord { x: 2019.9, y: 776.4 });
        assert_eq!(trimmed.coords[2], Coord { x: 2250.4, y: 500.1444 });
        assert_eq!(trimmed.coords[3], Coord { x: 2020.3, y: 775.6 });
        assert_eq!(trimmed.coords[4], Coord { x: 2020.4, y: 776.6 });
        assert_eq!(trimmed.coords[5], Coord { x: 2246.1, y: 545.3 });
    }

    // goal where puck enters goal before 35%, between 35-60%, and after 60%
    // should be trimmed when puck enters after 60%
    #[test]
    fn trim_goal_35_50_60() {
        let coords = vec![       
            Some(Coord { x: 1010.5, y: 900.5 }),
            Some(Coord { x: 2019.9, y: 776.4 }),
            Some(Coord { x: 2250.4, y: 500.1444 }), // enters here
            Some(Coord { x: 2020.3, y: 775.6 }),
            Some(Coord { x: 2260.4, y: 559.2 }), // enters here

            Some(Coord { x: 2000.1, y: 800.2 }),
            Some(Coord { x: 2246.1, y: 545.3 }), // enters again here and should be trimmed here
            Some(Coord { x: 12., y: 11. }),
            Some(Coord { x: 13., y: 11. }),            
            Some(Coord { x: 13., y: 11. }),            
        ];
        let rot_goal = RotatedPuckLocations { coords };
        let trimmed = trim_goal_coords(&rot_goal);

        assert_eq!(trimmed.coords.len(), 7);
        assert_eq!(trimmed.coords[0], Coord { x: 1010.5, y: 900.5 });
        assert_eq!(trimmed.coords[1], Coord { x: 2019.9, y: 776.4 });
        assert_eq!(trimmed.coords[2], Coord { x: 2250.4, y: 500.1444 });
        assert_eq!(trimmed.coords[3], Coord { x: 2020.3, y: 775.6 });
        assert_eq!(trimmed.coords[4], Coord { x: 2260.4, y: 559.2 });
        assert_eq!(trimmed.coords[5], Coord { x: 2000.1, y: 800.2 });
        assert_eq!(trimmed.coords[6], Coord { x: 2246.1, y: 545.3 });
    }

    // goal where puck enters goal before 35%, and after 60% twice
    // should be trimmed when puck first enters after 60%
    #[test]
    fn trim_goal_before_35_60_twice() {
        let coords = vec![       
            Some(Coord { x: 1010.5, y: 900.5 }),
            Some(Coord { x: 2019.9, y: 776.4 }),
            Some(Coord { x: 2250.4, y: 500.1444 }), // enters here
            Some(Coord { x: 2020.3, y: 775.6 }),
            Some(Coord { x: 2020.4, y: 775.6 }),
            
            Some(Coord { x: 2000.1, y: 800.2 }),
            Some(Coord { x: 2246.1, y: 545.3 }), // enters again here and should be trimmed here
            Some(Coord { x: 12., y: 11. }),            
            Some(Coord { x: 2260.4, y: 559.2 }), // enters here
            Some(Coord { x: 13., y: 11. }),            
        ];
        let rot_goal = RotatedPuckLocations { coords };
        let trimmed = trim_goal_coords(&rot_goal);

        assert_eq!(trimmed.coords.len(), 7);
        assert_eq!(trimmed.coords[0], Coord { x: 1010.5, y: 900.5 });
        assert_eq!(trimmed.coords[1], Coord { x: 2019.9, y: 776.4 });
        assert_eq!(trimmed.coords[2], Coord { x: 2250.4, y: 500.1444 });
        assert_eq!(trimmed.coords[3], Coord { x: 2020.3, y: 775.6 });
        assert_eq!(trimmed.coords[4], Coord { x: 2020.4, y: 775.6 });
        assert_eq!(trimmed.coords[5], Coord { x: 2000.1, y: 800.2 });
        assert_eq!(trimmed.coords[6], Coord { x: 2246.1, y: 545.3 });
    }

    // goal where puck enters between 35-40% and is less than 200 long
    // never enters again should be trimmed when it enters
    #[test]
    fn trim_goal_35_40_lt_200_once() {
        let coords = vec![       
            Some(Coord { x: 1010.5, y: 900.5 }),
            Some(Coord { x: 2019.9, y: 776.4 }),
            Some(Coord { x: 2020.3, y: 775.6 }),
            Some(Coord { x: 2250.4, y: 500.1444 }), // enters here and should be trimmed here            
            Some(Coord { x: 2250.3, y: 500.1443 }),
            
            Some(Coord { x: 2000.1, y: 800.2 }),
            Some(Coord { x: 2000.1, y: 800.2 }),
            Some(Coord { x: 12., y: 11. }),            
            Some(Coord { x: 2000.1, y: 800.2 }),
            Some(Coord { x: 13., y: 11. }),

            Some(Coord { x: 13., y: 11. }),
        ];
        let rot_goal = RotatedPuckLocations { coords };
        let trimmed = trim_goal_coords(&rot_goal);

        assert_eq!(trimmed.coords.len(), 4);
        assert_eq!(trimmed.coords[0], Coord { x: 1010.5, y: 900.5 });
        assert_eq!(trimmed.coords[1], Coord { x: 2019.9, y: 776.4 });
        assert_eq!(trimmed.coords[2], Coord { x: 2020.3, y: 775.6 });
        assert_eq!(trimmed.coords[3], Coord { x: 2250.4, y: 500.1444 });
    }

    // goal where puck enters between 35-40% and is less than 200 long
    // enters again later should be trimmed when it enters again
    #[test]
    fn trim_goal_35_40_lt_200_again() {
        let coords = vec![       
            Some(Coord { x: 1010.5, y: 900.5 }),
            Some(Coord { x: 2019.9, y: 776.4 }),
            Some(Coord { x: 2020.3, y: 775.6 }),
            Some(Coord { x: 2250.4, y: 500.1444 }), // enters here            
            Some(Coord { x: 2250.3, y: 500.1443 }),
            
            Some(Coord { x: 2000.1, y: 800.2 }),
            Some(Coord { x: 2000.1, y: 800.2 }),
            Some(Coord { x: 12., y: 11. }),            
            Some(Coord { x: 2000.1, y: 800.2 }),
            Some(Coord { x: 2251.3, y: 501.1443 }), // enters here again and should be trimmed here

            Some(Coord { x: 13., y: 11. }),
        ];
        let rot_goal = RotatedPuckLocations { coords };
        let trimmed = trim_goal_coords(&rot_goal);

        assert_eq!(trimmed.coords.len(), 10);
        assert_eq!(trimmed.coords[0], Coord { x: 1010.5, y: 900.5 });
        assert_eq!(trimmed.coords[1], Coord { x: 2019.9, y: 776.4 });
        assert_eq!(trimmed.coords[2], Coord { x: 2020.3, y: 775.6 });
        assert_eq!(trimmed.coords[3], Coord { x: 2250.4, y: 500.1444 });
        assert_eq!(trimmed.coords[4], Coord { x: 2250.3, y: 500.1443 });

        assert_eq!(trimmed.coords[5], Coord { x: 2000.1, y: 800.2 });
        assert_eq!(trimmed.coords[6], Coord { x: 2000.1, y: 800.2 });
        assert_eq!(trimmed.coords[7], Coord { x: 12., y: 11. });
        assert_eq!(trimmed.coords[8], Coord { x: 2000.1, y: 800.2 });
        assert_eq!(trimmed.coords[9], Coord { x: 2251.3, y: 501.1443 });
    }

    // goal where puck enters between 35-40% and is over 200 in length
    // should be trimmed when it enters
    #[test]
    fn trim_goal_35_40_gt_200() {
        // build a goal that's over 200 in length
        let len = 210;
        let mut x = 400.;
        let mut y = 600.;
        let mut coords = Vec::with_capacity(len);
        for _ in 1..=len {
            coords.push(Some(Coord { x, y } ));
            x = x + 1.;
            y = y + 1.;
        }
        coords[72] = Some(Coord { x: 2250., y: 541. }); // enters here and should be trimmed here
        coords[73] = Some(Coord { x: 2251., y: 542. });
        coords[74] = Some(Coord { x: 2249., y: 541. });
        let rot_goal = RotatedPuckLocations { coords };
        let trimmed = trim_goal_coords(&rot_goal);

        assert_eq!(trimmed.coords.len(), 73);
        assert_eq!(trimmed.coords[72], Coord { x: 2250., y: 541. });
    }

    // goal where puck enters between 40-60% and never again should be cut off when it enters
    #[test]
    fn trim_goal_40_60_once() {
        let coords = vec![       
            Some(Coord { x: 1010.5, y: 900.5 }),
            Some(Coord { x: 2019.9, y: 776.4 }),
            Some(Coord { x: 2020.3, y: 775.6 }),
            Some(Coord { x: 2250.4, y: 500.1444 }), // enters here and should be trimmed here          
            Some(Coord { x: 2250.3, y: 500.1443 }),
            
            Some(Coord { x: 2000.1, y: 800.2 }),
            Some(Coord { x: 2000.1, y: 800.2 }),
            Some(Coord { x: 12., y: 11. }),            
            Some(Coord { x: 2000.1, y: 800.2 }),
            Some(Coord { x: 2000.1, y: 800.2 }),
        ];
        let rot_goal = RotatedPuckLocations { coords };
        let trimmed = trim_goal_coords(&rot_goal);

        assert_eq!(trimmed.coords.len(), 4);
        assert_eq!(trimmed.coords[0], Coord { x: 1010.5, y: 900.5 });
        assert_eq!(trimmed.coords[1], Coord { x: 2019.9, y: 776.4 });
        assert_eq!(trimmed.coords[2], Coord { x: 2020.3, y: 775.6 });
        assert_eq!(trimmed.coords[3], Coord { x: 2250.4, y: 500.1444 });
    }

    // goal where puck enters between 40-60% and again before 60% should be cut off 
    // at the second time it enters
    #[test]
    fn trim_goal_40_60_again() {
        // build a goal that's 100 in length
        let len = 100;
        let mut x = 400.;
        let mut y = 600.;
        let mut coords = Vec::with_capacity(len);
        for _ in 1..=len {
            coords.push(Some(Coord { x, y } ));
            x = x + 1.;
            y = y + 1.;
        }
        coords[45] = Some(Coord { x: 2250., y: 541. }); // enters here
        coords[46] = Some(Coord { x: 2251., y: 542. });
        coords[47] = Some(Coord { x: 2249., y: 541. });

        coords[55] = Some(Coord { x: 2249., y: 540. }); // enters here and should be trimmed here
        coords[56] = Some(Coord { x: 2248., y: 541. });
        coords[57] = Some(Coord { x: 2250., y: 542. });
        let rot_goal = RotatedPuckLocations { coords };
        let trimmed = trim_goal_coords(&rot_goal);

        assert_eq!(trimmed.coords.len(), 56);
        assert_eq!(trimmed.coords[55], Coord { x: 2249., y: 540. });
    }

    // goal where puck enters between 40-60% and twice afer 60% should be cut off
    // at the first time after 60%
    #[test]
    fn trim_goal_40_60_after_60_twice() {
        // build a goal that's 100 in length
        let len = 100;
        let mut x = 400.;
        let mut y = 600.;
        let mut coords = Vec::with_capacity(len);
        for _ in 1..=len {
            coords.push(Some(Coord { x, y } ));
            x = x + 1.;
            y = y + 1.;
        }
        coords[45] = Some(Coord { x: 2250., y: 541. }); // enters here
        coords[46] = Some(Coord { x: 2251., y: 542. });
        coords[47] = Some(Coord { x: 2249., y: 541. });

        coords[65] = Some(Coord { x: 2249., y: 540. }); // enters here and should be trimmed here
        coords[66] = Some(Coord { x: 2248., y: 541. });
        coords[67] = Some(Coord { x: 2250., y: 542. });

        coords[75] = Some(Coord { x: 2290., y: 498. }); // enters here
        coords[76] = Some(Coord { x: 2291., y: 499. });
        coords[77] = Some(Coord { x: 2289., y: 500. });
        let rot_goal = RotatedPuckLocations { coords };
        let trimmed = trim_goal_coords(&rot_goal);

        assert_eq!(trimmed.coords.len(), 66);
        assert_eq!(trimmed.coords[65], Coord { x: 2249., y: 540.});
    }

    // goal where puck enters after 60% twice should be cutoff at the first time
    #[test]
    fn trim_goal_after_60_twice() {
        // build a goal that's 100 in length
        let len = 100;
        let mut x = 400.;
        let mut y = 600.;
        let mut coords = Vec::with_capacity(len);
        for _ in 1..=len {
            coords.push(Some(Coord { x, y } ));
            x = x + 1.;
            y = y + 1.;
        }
        coords[65] = Some(Coord { x: 2249., y: 540. }); // enters here and should be trimmed here
        coords[66] = Some(Coord { x: 2248., y: 541. });
        coords[67] = Some(Coord { x: 2250., y: 542. });

        coords[75] = Some(Coord { x: 2290., y: 498. }); // enters here
        coords[76] = Some(Coord { x: 2291., y: 499. });
        coords[77] = Some(Coord { x: 2289., y: 500. });
        let rot_goal = RotatedPuckLocations { coords };
        let trimmed = trim_goal_coords(&rot_goal);

        assert_eq!(trimmed.coords.len(), 66);
        assert_eq!(trimmed.coords[65], Coord { x: 2249., y: 540.});
    }

    // goal with None's at the start shouldn't throw off percentages used
    // to trim the end
    // enters twice after 60%
    // if None's were trimmed before figuring out cutoff point,
    // would incorrectly use logic for before 35%
    #[test]
    fn trim_goal_start_nones_60() {
        // build a goal that's 100 in length
        let len = 100;
        let mut x = 400.;
        let mut y = 600.;
        let mut coords = Vec::with_capacity(len);
        for i in 1..=len {
            if i <= 50 {
                coords.push(None);
            } else {
                coords.push(Some(Coord { x, y } ));
                x = x + 1.;
                y = y + 1.;
            }
            
        }

        coords[65] = Some(Coord { x: 2249., y: 540. }); // enters here and should be trimmed here
        coords[66] = Some(Coord { x: 2248., y: 541. });
        coords[67] = Some(Coord { x: 2250., y: 542. });

        coords[75] = Some(Coord { x: 2290., y: 498. }); // enters here
        coords[76] = Some(Coord { x: 2291., y: 499. });
        coords[77] = Some(Coord { x: 2289., y: 500. });
        let rot_goal = RotatedPuckLocations { coords };
        let trimmed = trim_goal_coords(&rot_goal);

        assert_eq!(trimmed.coords.len(), 16);
        assert_eq!(trimmed.coords[15], Coord { x: 2249., y: 540.});
    }

    // --------------------------------------------------
    // preprocess_folder_data() tests
    // --------------------------------------------------

    // folder with mix of goals to rotate and to not rotate, as well as missing
    // goal info
    #[test]
    fn preprocess_folder_no_rotate() {
        let mut pbp_data_map = HashMap::new();
        let goal_details = vec![
            GoalDetails { event_id: 10, scoring_team_id: 19, home_team_defending_side: String::from("Left"), ppt_replay_url: Some(String::from("")) },
            GoalDetails { event_id: 1001, scoring_team_id: 19, home_team_defending_side: String::from("Right"), ppt_replay_url: Some(String::from("")) },
            GoalDetails { event_id: 485, scoring_team_id: 3, home_team_defending_side: String::from("Right"), ppt_replay_url: Some(String::from("")) },
            GoalDetails { event_id: 740, scoring_team_id: 3, home_team_defending_side: String::from("Right"), ppt_replay_url: Some(String::from("")) },
        ];
        let pbp_data = PbpData {
            goals: goal_details,
            home_team_id: 19
        };
        let game_id = GameId(202400001);
        pbp_data_map.insert(game_id, pbp_data);

        let mut game_goals_map = HashMap::new();
        let mut goal_id_to_loc_map = HashMap::new();

        // create raw puck location data
        // not rotated
        let goal_10 = RawPuckLocationData { coords: vec![       
            Some(Coord { x: 1010.5, y: 900.5 }),
            Some(Coord { x: 2019.9, y: 776.4 }),
            Some(Coord { x: 2500., y: 1022. }),
            Some(Coord { x: 2250.4, y: 500.1444 }), // enters here and should be trimmed here          
            Some(Coord { x: 2250.3, y: 500.1443 }),
            
            Some(Coord { x: 2000.1, y: 800.2 }),
            Some(Coord { x: 2000.1, y: 800.2 }),
            Some(Coord { x: 12., y: 11. }),            
            Some(Coord { x: 2000.1, y: 800.2 }),
            Some(Coord { x: 2000.1, y: 800.2 }),
        ]};
        goal_id_to_loc_map.insert(GoalId(10), goal_10);

        // goal should be rotated
        let goal_1001 = RawPuckLocationData { coords: vec![       
            Some(Coord { x: 1010., y: 900. }),
            Some(Coord { x: 2019., y: 776. }),
            Some(Coord { x: 2020., y: 775. }),
            Some(Coord { x: -7.1, y: 101. }),           
            Some(Coord { x: 701., y: 1020. }),
            
            Some(Coord { x: 704., y: 104. }),
            Some(Coord { x: 702., y: 150. }),
            Some(Coord { x: 12., y: 11. }),                        
            Some(Coord { x: 150., y: 501. }), // enters here again and should be trimmed here            
            Some(Coord { x: 150., y: 501. }),
        ] };
        goal_id_to_loc_map.insert(GoalId(1001), goal_1001);

        // not rotated
        let goal_485 = RawPuckLocationData { coords: vec![       
            Some(Coord { x: 1010.5, y: 900.5 }),
            Some(Coord { x: 2019.9, y: 776.4 }),
            Some(Coord { x: 2250.4, y: 500.1444 }), // enters here
            Some(Coord { x: 2020.3, y: 775.6 }),
            Some(Coord { x: 2246.1, y: 545.3 }), // enters again here and should be trimmed here

            Some(Coord { x: 12., y: 11. }),
            Some(Coord { x: 13., y: 11. }),
            Some(Coord { x: 13., y: 11. }),
            Some(Coord { x: 13., y: 11. }),
            Some(Coord { x: 13., y: 11. }),
        ] };
        goal_id_to_loc_map.insert(GoalId(485), goal_485);

        game_goals_map.insert(game_id, goal_id_to_loc_map);
        let folder_data = PbpGoalLocData { 
            pbp_data: pbp_data_map,
            goal_loc_data: game_goals_map
        };
        let preprocessed_data = preprocess_folder_data(&folder_data);
        assert_eq!(preprocessed_data.len(), 3);

        assert_eq!(preprocessed_data[&(game_id, GoalId(10))], TrimmedPuckLocations { coords: vec![
            Coord { x: 1010.5, y: 900.5 },
            Coord { x: 2019.9, y: 776.4 },
            Coord { x: 2400., y: 1015. },
            Coord { x: 2250.4, y: 500.1444 },
        ]});
        assert_eq!(preprocessed_data[&(game_id, GoalId(1001))], TrimmedPuckLocations { coords: vec![
            Coord { x: 1390., y: 115. },
            Coord { x: 381., y: 239. },
            Coord { x: 380., y: 240. },
            Coord { x: 2400., y: 914. },           
            Coord { x: 1699., y: 0. },
            
            Coord { x: 1696., y: 911. },
            Coord { x: 1698., y: 865. },
            Coord { x: 2388., y: 1004. },                        
            Coord { x: 2250., y: 514. },
        ]});
        assert_eq!(preprocessed_data[&(game_id, GoalId(485))], TrimmedPuckLocations { coords: vec![
            Coord { x: 1010.5, y: 900.5 },
            Coord { x: 2019.9, y: 776.4 },
            Coord { x: 2250.4, y: 500.1444 },
            Coord { x: 2020.3, y: 775.6 },
            Coord { x: 2246.1, y: 545.3 },
        ]});
    }

    // --------------------------------------------------
    // testing entire process of pre-processing a folder
    // --------------------------------------------------

    #[test]
    fn read_process_folder() {
        let test_path = "test_data/trim_rotate_test_folder/";
        let mut test_data = PbpGoalLocData {
            goal_loc_data: HashMap::new(),
            pbp_data: HashMap::new()
        };
        read_folder(test_path, &mut test_data).unwrap();
        let preprocessed_data = preprocess_folder_data(&test_data);

        assert_eq!(preprocessed_data.len(), 3);
        let game_id = GameId(2024020011);
        assert_eq!(preprocessed_data[&(game_id, GoalId(155))], TrimmedPuckLocations { coords: vec![
            Coord { x: 101., y: 202.1 },
            Coord { x: 102., y: 203. },
            Coord { x: 104., y: 220. },
            Coord { x: 150., y: 650. },           
            Coord { x: 150., y: 651. },
            
            Coord { x: 2300., y: 501. },            
        ]});

        // check the rotated goal
        assert_eq!(preprocessed_data[&(game_id, GoalId(1414))], TrimmedPuckLocations { coords: vec![
            Coord { x: 1399., y: 115. },
            Coord { x: 1398., y: 116. },
            Coord { x: 1397., y: 117. },
            Coord { x: 1396., y: 118. },           
            Coord { x: 1396., y: 119. },
            
            Coord { x: 1395., y: 118. },
            Coord { x: 2250., y: 515. },                   
        ]});

        // check the other game in the folder
        let game_id = GameId(2024020012);
        assert_eq!(preprocessed_data[&(game_id, GoalId(277))], TrimmedPuckLocations { coords: vec![
            Coord { x: 101., y: 202.1 },
            Coord { x: 102., y: 203. },
            Coord { x: 104., y: 220. },
            Coord { x: 150., y: 650. },           
            Coord { x: 150., y: 651. },
            
            Coord { x: 202., y: 670. },
            Coord { x: 2301., y: 502. },
        ]});
    }
}