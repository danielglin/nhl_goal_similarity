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
use std::iter::zip;
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

    let goal_loc_data: Vec<Instance> = serde_json::from_reader(reader)?;

    // go through each instance and pull out just the puck data
    let mut all_puck_info = vec![];
    for instance in &goal_loc_data {
        let puck_data = instance.onIce.get(PUCK_ID);
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

/// Reads in the goal location data and returns both puck and player location info
pub fn read_puck_player_loc_data<P: AsRef<Path> + Display>(path: P) -> Result<GoalRawPuckPlayerData> {
    const PUCK_ID: &str = "1";

    let file = File::open(&path)?;
    let reader = BufReader::new(file);

    let goal_loc_data: Vec<Instance> = serde_json::from_reader(reader)?;

    // go through each instance 
    let mut all_puck_info = vec![];
    let mut all_players_info = vec![];
    for instance in &goal_loc_data {
        // go through each on-ice entity for a timestep
        let mut instance_players_info = HashMap::new();

        for (id, entity_map) in &instance.onIce {
            let coord = if entity_map.get("x").is_some() && entity_map.get("y").is_some() {
                // check that both x and y coordinates are floats just to
                // be safe, and then extract then to create the puck's
                // coordinate
                if entity_map["x"].is_number() && entity_map["y"].is_number() {
                    let x = match entity_map["x"].as_f64() {
                        Some(x) => x,
                        None => {
                            continue
                        }
                    };
                    let y = match entity_map["y"].as_f64() {
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
            } else {
                None
            };

            if id == PUCK_ID {
                all_puck_info.push(coord);
            } else {
                let player_id;
                let team_id;
                // get the player id
                if entity_map.get("playerId").is_some() {
                    player_id = match entity_map["playerId"].as_u64() {
                        Some(player_id) => player_id,
                        None => {
                            continue
                        }
                    };
                } else {
                    continue
                };

                // get the team id
                if entity_map.get("teamId").is_some() {
                    team_id = match entity_map["teamId"].as_u64() {
                        Some(team_id) => team_id,
                        None => {
                            continue
                        }
                    };
                } else {
                    continue
                };
                // if the player's coordinate is None, don't bother adding it
                // to the map
                if coord.is_none() {
                    continue
                }
                let player_info = PlayerInfo { id: player_id as u32, team_id: team_id as u32 };
                instance_players_info.insert(player_info, coord.unwrap()); // coord is guaranteed to be Some
            };
        }
    // if there is no player info (the map is empty), add a None to the player info vec
    if instance_players_info.is_empty() {
        all_players_info.push(None);
    } else {
        all_players_info.push(Some(instance_players_info));
    };
    
    }
    Ok(GoalRawPuckPlayerData{ 
        puck_tracking: RawPuckLocationData { coords: all_puck_info },
        player_tracking: all_players_info
    })
}

// --------------------------------------
// Structs for deserializing play-by-play data
// --------------------------------------
#[derive(Deserialize, Debug)]
pub struct PbpData {
    pub goals: Vec<GoalDetails>,
    pub home_team_id: u16
}

#[derive(Deserialize, Debug)]
pub struct GoalDetails {
    pub event_id: u32,
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

#[derive(Debug)]
pub enum TrackingType {
    PuckOnly { goal_loc_data: HashMap<GameId, HashMap<GoalId, RawPuckLocationData>> },
    PuckAndPlayer { goal_loc_data: HashMap<GameId, HashMap<GoalId, GoalRawPuckPlayerData>> },
}

#[derive(Debug, PartialEq, Eq, Hash, Copy, Clone)]
struct PlayerInfo {
    id: u32,
    team_id: u32
}

/// Raw puck and player tracking data together
/// puck_tracking: each element is a timestep's puck coordinates
/// player_tracking: each element is all the player tracking data for a timestep
/// Each element for both is an Option due to timesteps potentially missing coordinates
/// An element in the player vector would be an option if there is no location
/// data for any player for that timestep
/// The two vectors line up
#[derive(Debug)]
pub struct GoalRawPuckPlayerData {
    puck_tracking: RawPuckLocationData,
    player_tracking: Vec<Option<HashMap<PlayerInfo, Coord>>>
}

/// Combines both goal location data and play-by-play data using two hash maps
/// Both maps are keyed by the game id
#[derive(Debug)]
pub struct PbpGoalLocData {
    // pub goal_loc_data: HashMap<GameId, HashMap<GoalId, RawPuckLocationData>>,
    pub tracking_type: TrackingType,
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
                // game directory
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
                        
                        // getting just the puck data
                        match &mut pbp_goal_data.tracking_type {
                            TrackingType::PuckOnly { goal_loc_data } => {
                                let loc_data = match read_loc_data(&game_path.to_str().ok_or(anyhow!("invalid path"))?) {
                                    Ok(ld) => ld,
                                    Err(e) => {
                                        println!("*** Error {e} when trying to parse goal data at {}", &game_path.to_string_lossy());
                                        continue
                                    }
                                };

                                // add goal location data
                                match goal_loc_data.get_mut(&game_id) {
                                    Some(map) => {
                                        map.insert(goal_id, loc_data);
                                    },
                                    None => {
                                        // need to create a new map if it doesn't exist
                                        let mut goals_map = HashMap::new();
                                        goals_map.insert(goal_id, loc_data);
                                        goal_loc_data.insert(game_id, goals_map);
                                    }
                                };
                            },
                            
                            TrackingType::PuckAndPlayer { goal_loc_data } => {
                                let loc_data = match read_puck_player_loc_data(&game_path.to_str().ok_or(anyhow!("invalid path"))?) {
                                    Ok(ld) => ld,
                                    Err(e) => {
                                        println!("*** Error {e} when trying to parse goal data at {}", &game_path.to_string_lossy());
                                        continue
                                    }
                                };

                                // add goal location data
                                match goal_loc_data.get_mut(&game_id) {
                                    Some(map) => {
                                        map.insert(goal_id, loc_data);
                                    },
                                    None => {
                                        // need to create a new map if it doesn't exist
                                        let mut goals_map = HashMap::new();
                                        goals_map.insert(goal_id, loc_data);
                                        goal_loc_data.insert(game_id, goals_map);
                                    }
                                };
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

    Ok(())
}

#[derive(Debug)]
struct RotatedPuckLocations {
    pub coords: Vec<Option<Coord>>
}

/// Rotate a single coordinate
fn _rotate_coord(coord: ClampedCoord) -> Coord {
    const MAX_X: f64 = 2400.;
    const MAX_Y: f64 = 1015.;

    Coord{ x: MAX_X - coord.get_x(), y: MAX_Y - coord.get_y() }
}

/// Rotates a goal's coordinates by 180 degrees
fn rotate_goal_coords(puck_locs: Vec<Option<ClampedCoord>>) -> RotatedPuckLocations {    
    let mut rotated_coords = Vec::with_capacity(puck_locs.len());

    for coord in puck_locs {
        match coord {
            Some(c) => {
                let rot_coord = _rotate_coord(c);
                
                rotated_coords.push(Some(rot_coord));
            },
            None => {
                rotated_coords.push(None)
            }
        }
    }
    RotatedPuckLocations { coords: rotated_coords }
}


#[derive(Debug, PartialEq, Clone)]
pub struct TrimmedPuckLocations {
    pub coords: Vec<Coord>
}

/// Trim the tracking data for a goal, both at the start and at the end
/// All None's will get removed.
// fn trim_goal_coords(goal: &RotatedPuckLocations) -> TrimmedPuckLocations {
fn trim_goal_coords(goal: &RotatedPuckLocations) -> (usize, Option<usize>) {
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
    let mut in_faceoff = false;  // ever in faceoff
    let mut starting_offset = 0;
    let mut starting_pt = 0;
    let mut none_start_incremented_starting_pt = false; // flag for if moved the starting 
                                                              // pt due to None's at the start
                                                              // used to address issues w/ faceoff goals
                                                              // w/ None's at the start 
                                                              // (like 2025020182/160)
    // let mut ever_in_faceoff = false; // flag for same 2025020182/160 issue
    
    // vars for trimming end
    let num_instances = goal.coords.len(); 
    // let mut earliest_instant_in_goal: &Option<usize> = &None;
    let mut earliest_perc_in_goal = None;
    let mut curr_interval_first_instant = None;
    let mut curr_interval_last_instant= None;
    let mut cutoff_pt = None;

    // let mut trimmed_coords = vec![];
    for (instant_num, instance) in goal.coords.iter().enumerate() {
        if instance.is_none() {
            starting_offset += 1;

            // need to make sure the instant is contiguous w/
            // the starting None block to avoid moving up
            // the starting pt when there is a missing val
            // in the middle
            // also need to handle the case where first instance isn't None, but
            // the second instance is by using the starting offset
            
            // also need to handle if the very first instant is None
            // if ((instant_num == (starting_pt + 1)) && (starting_offset > 1)) || (instant_num == 0) { // old version
            if (((instant_num == (starting_pt)) || (instant_num == (starting_pt+1) && in_faceoff))&& (starting_offset > 1)) || (instant_num == 0) { // new version
                starting_pt += 1;
            }

            // handle cases where if have None's at the start and then puck's
            // in a faceoff dot, starting_pt will be 1 too high
            // happens in 2025020182/160
            if instant_num == 0 {
                none_start_incremented_starting_pt = true;
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
        }
        
        // trim the end
        if (instance_bare.x >= GOAL_LEFT) && (instance_bare.x <= GOAL_RIGHT) &&
            (instance_bare.y >= GOAL_TOP) && (instance_bare.y <= GOAL_BOTTOM) 
        {
            if earliest_perc_in_goal.is_none() {
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
        // println!("at end of instance loop; instance: instant_num: {instant_num}, instance: {instance:?}, starting_pt: {starting_pt}, starting OFFSET: {starting_offset}");
    }
    
    // cutoff_pt is None if puck never enters the net
    // in this case, we'll just still use the starting point we found
    // but not trim anything from the end


    // adjust starting pt if there were None's at the start and was in faceoff
    // to address the 2025020182/160 issue, which is where the starting pt
    // is over by 1 when there are None's at the start and also in a faceoff
    if none_start_incremented_starting_pt && in_faceoff {
        starting_pt -= 1;
    }

    (starting_pt, cutoff_pt)
}

/// Determines if a goal's coordinates need to be rotated
fn need_to_rotate(goal_details: &GoalDetails, home_team_id: u16) -> bool {
    let scoring_team_id = goal_details.scoring_team_id;
    let home_team_defending_side = &goal_details.home_team_defending_side;
    let away_team_defending_side;
    let scoring_side;

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

    if scoring_side == "Left" {
        true
    } else {
        false
    }
}

/// Pre-processes a single goal's puck data
// fn preprocess_goal(raw_puck_tracking: &RawPuckLocationData, need_to_rotate: bool) -> (TrimmedPuckLocations, usize, Option<usize>) {
pub fn preprocess_puck(raw_puck_tracking: &RawPuckLocationData, goal_details: &GoalDetails, home_team_id: u16) -> (TrimmedPuckLocations, usize, Option<usize>, Vec<usize>) {
    // clamp to valid min and max values
    let mut clamped_coords = Vec::with_capacity(raw_puck_tracking.coords.len());
    for coord in &raw_puck_tracking.coords {
        let clamped = match coord {
            Some(c) => Some(clamp_coord(c)),
            None => None
        };
        clamped_coords.push(clamped);
    }

    let rotated_goal;
    let do_rotate = need_to_rotate(&goal_details, home_team_id);

    // need to rotate if the goal was scored on the left side of the ice
    if do_rotate {
        rotated_goal = rotate_goal_coords(clamped_coords);
    } else {
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
    let (starting_pt, cutoff_pt) = trim_goal_coords(&rotated_goal);
    let (trimmed_coords, timesteps) = subset_coords(&rotated_goal, starting_pt, cutoff_pt);
    (trimmed_coords, starting_pt, cutoff_pt, timesteps)
}

/// Pre-processed puck and player location data for a single goal
pub struct PreprocessedPuckPlayerLocationGoalData {
    pub puck_goal_ids: Vec<u32>,
    pub puck_timesteps: Vec<u32>,
    pub puck_xs: Vec<f64>,
    pub puck_ys: Vec<f64>,
    pub player_goal_ids: Vec<u32>,
    pub player_timesteps: Vec<u32>,
    pub player_ids: Vec<u32>,
    pub team_ids: Vec<Option<u32>>,
    pub player_xs: Vec<f64>,
    pub player_ys: Vec<f64>
}

/// Pre-process the player and puck location data for a single goal
pub fn preprocess_player_puck(raw_puck_player_tracking: &GoalRawPuckPlayerData, goal_details: &GoalDetails, home_team_id: u16) -> PreprocessedPuckPlayerLocationGoalData {
    const NUM_PLAYERS_ON_ICE_5V5: usize = 12;

    // need to first pre-process puck data in order to trim the player data correctly
    let (trimmed_puck_coords, starting_pt, cutoff_pt, timesteps) = preprocess_puck(&raw_puck_player_tracking.puck_tracking, goal_details, home_team_id);

    let do_rotate = need_to_rotate(&goal_details, home_team_id);

    let goal_iter;
    let num_rows_estimate;
    
    // take the puck locations and build vecs used for df creation
    let mut puck_goal_ids = Vec::with_capacity(timesteps.len());
    let mut puck_timesteps = Vec::with_capacity(timesteps.len());
    let mut puck_xs = Vec::with_capacity(timesteps.len());
    let mut puck_ys = Vec::with_capacity(timesteps.len());

    for (puck_coord, timestep) in zip(trimmed_puck_coords.coords, timesteps) {
        puck_goal_ids.push(goal_details.event_id);
        puck_timesteps.push(timestep as u32);
        puck_xs.push(puck_coord.x);
        puck_ys.push(puck_coord.y);
    }

    // player logic
    if cutoff_pt.is_none() {
        goal_iter = raw_puck_player_tracking.player_tracking[starting_pt..].iter().enumerate();
        num_rows_estimate = NUM_PLAYERS_ON_ICE_5V5 * raw_puck_player_tracking.player_tracking.len();
    } else {
        goal_iter = raw_puck_player_tracking.player_tracking[starting_pt..=cutoff_pt.unwrap()].iter().enumerate();
        num_rows_estimate = NUM_PLAYERS_ON_ICE_5V5 * (cutoff_pt.unwrap() - starting_pt);
    }

    // output vectors used to build player loc df
    let mut goal_ids = Vec::with_capacity(num_rows_estimate);
    let mut player_timesteps = Vec::with_capacity(num_rows_estimate);
    let mut player_ids = Vec::with_capacity(num_rows_estimate);
    let mut team_ids = Vec::with_capacity(num_rows_estimate);
    let mut xs = Vec::with_capacity(num_rows_estimate);
    let mut ys = Vec::with_capacity(num_rows_estimate);

    for (i, opt_player_loc_map) in goal_iter {
        match opt_player_loc_map {
            Some(player_loc_map) => {
                // go through all players in the time step and pre-process their 
                // coordinates                
                for (player_info, coord) in player_loc_map {
                    // clamp player data
                    let clamped_coord = clamp_coord(coord);

                    // rotate coord if needed
                    let final_coord;
                    if do_rotate {
                        final_coord = _rotate_coord(clamped_coord);
                    } else {
                        final_coord = Coord{ x: clamped_coord.get_x(), y: clamped_coord.get_y() };
                    }

                    // update output vecs
                    goal_ids.push(goal_details.event_id);
                    player_timesteps.push(i as u32);
                    player_ids.push(player_info.id);
                    team_ids.push(Some(player_info.team_id));
                    xs.push(final_coord.x);
                    ys.push(final_coord.y);
                }

            },
            None => {
                continue
            }
        }
    }

    PreprocessedPuckPlayerLocationGoalData { 
        puck_goal_ids,
        puck_timesteps,
        puck_xs,
        puck_ys,
        player_goal_ids: goal_ids, 
        player_timesteps, 
        player_ids, 
        team_ids, 
        player_xs: xs, 
        player_ys: ys 
    }
}

/// Uses indices to cut off starting and ending portions of coordinates
/// The returned vec has the timesteps where the puck's coordinates aren't missing.
/// That vec is used to keep the puck and player data aligned.
/// When `starting_pt` is greater than 0, the vec still starts at 0.
fn subset_coords(rotated_goal: &RotatedPuckLocations, starting_pt: usize, cutoff_pt: Option<usize>) -> (TrimmedPuckLocations, Vec<usize>) {
    let mut trimmed_coords = vec![];
    let mut timesteps = vec![];
    let goal_iter;

    // if the starting point is greater than the cutoff, return an empty
    // TrimmedPuckLocations and vec
    if cutoff_pt.is_some() && starting_pt > cutoff_pt.unwrap() {
        return (
            TrimmedPuckLocations { coords: vec![] },
            vec![]
        )
    }

    if cutoff_pt.is_none() {
        goal_iter = rotated_goal.coords[starting_pt..].iter().enumerate();
    } else {
        goal_iter = rotated_goal.coords[starting_pt..=cutoff_pt.unwrap()].iter().enumerate();
    }
    for (i, coords) in goal_iter {
        match coords {
            Some(c) => {
                trimmed_coords.push(c.clone());
                timesteps.push(i);
            },
            None => ()
        }
    }
    (TrimmedPuckLocations { coords: trimmed_coords }, timesteps)
}

/// Takes only the last n instances of a trimmed goal
/// If the goal is less than n instances long, returns the trimmed goal as-is.
pub fn take_last_n_instances(goal: TrimmedPuckLocations, n: usize) -> TrimmedPuckLocations {
    if n > goal.coords.len() {
        return goal
    }

    let coords = goal.coords[goal.coords.len()-n..].to_vec();
    TrimmedPuckLocations { coords: coords }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

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
            tracking_type: TrackingType::PuckOnly { goal_loc_data: HashMap::new() },
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

        // game 2024020011, goal 255
        // this goal has no puck data, so it should not be in the game's
        // goal data
        if let TrackingType::PuckOnly { ref goal_loc_data } = test_data.tracking_type {
            assert_eq!(goal_loc_data[&GameId(2024020011)].len(), 5);
            assert!(!goal_loc_data[&GameId(2024020011)].contains_key(&GoalId(255)));
        
            // game 2024020011, goal 298
            // this goal has puck data for every instance
            let goal_298 = &goal_loc_data[&GameId(2024020011)][&GoalId(298)];
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
            let goal_415 = &goal_loc_data[&GameId(2024020011)][&GoalId(415)];
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
            let goal_428 = &goal_loc_data[&GameId(2024020011)][&GoalId(428)];
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
            let goal_453 = &goal_loc_data[&GameId(2024020011)][&GoalId(453)];
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
        };

        // check all games' data: 2024020012
        // has no goals
        // play-by-play data
        let pbp_data = &test_data.pbp_data[&GameId(2024020012)];
        assert_eq!(pbp_data.goals.len(), 0);
        assert_eq!(pbp_data.home_team_id, 3);

        // since there are no goal location files for the game, 
        // the game has no map for the goal data
        if let TrackingType::PuckOnly { ref goal_loc_data } = test_data.tracking_type {
            assert!(!goal_loc_data.contains_key(&GameId(2024020012)));
        }

        // check all games' data: 2024020013
        // has no goals
        // play-by-play data
        let pbp_data = &test_data.pbp_data[&GameId(2024020013)];
        assert_eq!(pbp_data.goals.len(), 3);
        assert_eq!(pbp_data.goals[0].event_id, 182);
        assert_eq!(pbp_data.goals[1].event_id, 311);
        assert_eq!(pbp_data.goals[2].event_id, 794);
        assert_eq!(pbp_data.home_team_id, 9);

        if let TrackingType::PuckOnly { ref goal_loc_data } = test_data.tracking_type {
            assert_eq!(goal_loc_data[&GameId(2024020013)].len(), 3);

            // game 2024020013, goal 182
            let goal_182 = &goal_loc_data[&GameId(2024020013)][&GoalId(182)];
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
            let goal_311 = &goal_loc_data[&GameId(2024020013)][&GoalId(311)];
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
            let goal_794 = &goal_loc_data[&GameId(2024020013)][&GoalId(794)];
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
    }

    // read in a folder to get both player and puck data
    #[test]
    fn read_folder_some_missing_player_puck_data() {
        let test_folder = "test_data/test_folder";
        let mut test_data = PbpGoalLocData {
            tracking_type: TrackingType::PuckAndPlayer { goal_loc_data: HashMap::new() },
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

        // game 2024020011, goal 255
        // this goal has no puck data, so it should not be in the game's
        // goal data
        if let TrackingType::PuckAndPlayer { ref goal_loc_data } = test_data.tracking_type {
            assert_eq!(goal_loc_data.len(), 2); // there are 2 games with valid data
            assert_eq!(goal_loc_data[&GameId(2024020011)].len(), 6); // there are 6 goals in this game
            assert!(goal_loc_data[&GameId(2024020011)].contains_key(&GoalId(255)));

            // game 2024020011, goal 255
            let goal_255 = &goal_loc_data[&GameId(2024020011)][&GoalId(255)];

            // puck info
            assert_eq!(goal_255.puck_tracking.coords.len(), 5);
            assert!(goal_255.puck_tracking.coords[0].is_none());
            assert!(goal_255.puck_tracking.coords[1].is_none());
            assert!(goal_255.puck_tracking.coords[2].is_none());
            assert!(goal_255.puck_tracking.coords[3].is_none());
            assert!(goal_255.puck_tracking.coords[4].is_none());

            // player info
            assert_eq!(goal_255.player_tracking.len(), 5); // there are 5 time steps

            // first time step of player info
            assert_eq!(goal_255.player_tracking[0].clone().unwrap().len(), 11); // there are 11 players
            let player_8480280_1_coord = &goal_255.player_tracking[0].clone().unwrap()[&PlayerInfo{id: 8480280, team_id: 6}];
            assert_eq!(player_8480280_1_coord.x, 150.4144);
            assert_eq!(player_8480280_1_coord.y, 516.8132);

            let player_8480051_1_coord = &goal_255.player_tracking[0].clone().unwrap()[&PlayerInfo{id: 8480051, team_id: 8}];
            assert_eq!(player_8480051_1_coord.x, 2236.1672);
            assert_eq!(player_8480051_1_coord.y, 529.4865);
            let player_8477496_1_coord = &goal_255.player_tracking[0].clone().unwrap()[&PlayerInfo{id: 8477496, team_id: 6}];
            assert_eq!(player_8477496_1_coord.x, 2000.9888);
            assert_eq!(player_8477496_1_coord.y, 755.242);
            let player_8473419_1_coord = &goal_255.player_tracking[0].clone().unwrap()[&PlayerInfo{id: 8473419, team_id: 6}];
            assert_eq!(player_8473419_1_coord.x, 1997.5461);
            assert_eq!(player_8473419_1_coord.y, 948.5736);
            let player_8477989_1_coord = &goal_255.player_tracking[0].clone().unwrap()[&PlayerInfo{id: 8477989, team_id: 8}];
            assert_eq!(player_8477989_1_coord.x, 2043.1991);
            assert_eq!(player_8477989_1_coord.y, 752.4976);
            let player_8475233_1_coord = &goal_255.player_tracking[0].clone().unwrap()[&PlayerInfo{id: 8475233, team_id: 8}];
            assert_eq!(player_8475233_1_coord.x, 2062.6813);
            assert_eq!(player_8475233_1_coord.y, 935.107);
            let player_8476875_1_coord = &goal_255.player_tracking[0].clone().unwrap()[&PlayerInfo{id: 8476875, team_id: 8}];
            assert_eq!(player_8476875_1_coord.x, 2164.1038);
            assert_eq!(player_8476875_1_coord.y, 660.754);
            let player_8476981_1_coord = &goal_255.player_tracking[0].clone().unwrap()[&PlayerInfo{id: 8476981, team_id: 8}];
            assert_eq!(player_8476981_1_coord.x, 2051.9758);
            assert_eq!(player_8476981_1_coord.y, 547.9464);
            let player_8478401_1_coord = &goal_255.player_tracking[0].clone().unwrap()[&PlayerInfo{id: 8478401, team_id: 6}];
            assert_eq!(player_8478401_1_coord.x, 1961.5559);
            assert_eq!(player_8478401_1_coord.y, 553.3637);
            let player_8479325_1_coord = &goal_255.player_tracking[0].clone().unwrap()[&PlayerInfo{id: 8479325, team_id: 6}];
            assert_eq!(player_8479325_1_coord.x, 1664.6244);
            assert_eq!(player_8479325_1_coord.y, 921.9213);
            let player_8477956_1_coord = &goal_255.player_tracking[0].clone().unwrap()[&PlayerInfo{id: 8477956, team_id: 6}];
            assert_eq!(player_8477956_1_coord.x, 1687.3648);
            assert_eq!(player_8477956_1_coord.y, 456.4575);

            // second time step of player info
            assert_eq!(goal_255.player_tracking[1].clone().unwrap().len(), 11); // there are 11 players
        
            let player_8480280_2_coord = &goal_255.player_tracking[1].clone().unwrap()[&PlayerInfo{id: 8480280, team_id: 6}];
            assert_eq!(player_8480280_2_coord.x, 151.4144);
            assert_eq!(player_8480280_2_coord.y, 517.8132);
            let player_8480051_2_coord = &goal_255.player_tracking[1].clone().unwrap()[&PlayerInfo{id: 8480051, team_id: 8}];
            assert_eq!(player_8480051_2_coord.x, 2237.1672);
            assert_eq!(player_8480051_2_coord.y, 528.4865);
            let player_8477496_2_coord = &goal_255.player_tracking[1].clone().unwrap()[&PlayerInfo{id: 8477496, team_id: 6}];
            assert_eq!(player_8477496_2_coord.x, 2020.9888);
            assert_eq!(player_8477496_2_coord.y, 765.242);
            let player_8473419_2_coord = &goal_255.player_tracking[1].clone().unwrap()[&PlayerInfo{id: 8473419, team_id: 6}];
            assert_eq!(player_8473419_2_coord.x, 1994.5461);
            assert_eq!(player_8473419_2_coord.y, 941.5736);
            let player_8477989_2_coord = &goal_255.player_tracking[1].clone().unwrap()[&PlayerInfo{id: 8477989, team_id: 8}];
            assert_eq!(player_8477989_2_coord.x, 2041.1991);
            assert_eq!(player_8477989_2_coord.y, 751.4976);
            let player_8475233_2_coord = &goal_255.player_tracking[1].clone().unwrap()[&PlayerInfo{id: 8475233, team_id: 8}];
            assert_eq!(player_8475233_2_coord.x, 2061.6813);
            assert_eq!(player_8475233_2_coord.y, 944.107);
            let player_8476875_2_coord = &goal_255.player_tracking[1].clone().unwrap()[&PlayerInfo{id: 8476875, team_id: 8}];
            assert_eq!(player_8476875_2_coord.x, 2165.1038);
            assert_eq!(player_8476875_2_coord.y, 668.754);
            let player_8476981_2_coord = &goal_255.player_tracking[1].clone().unwrap()[&PlayerInfo{id: 8476981, team_id: 8}];
            assert_eq!(player_8476981_2_coord.x, 2050.9758);
            assert_eq!(player_8476981_2_coord.y, 548.9464);
            let player_8478401_2_coord = &goal_255.player_tracking[1].clone().unwrap()[&PlayerInfo{id: 8478401, team_id: 6}];
            assert_eq!(player_8478401_2_coord.x, 1962.5559);
            assert_eq!(player_8478401_2_coord.y, 554.3637);
            let player_8479325_2_coord = &goal_255.player_tracking[1].clone().unwrap()[&PlayerInfo{id: 8479325, team_id: 6}];
            assert_eq!(player_8479325_2_coord.x, 1665.6244);
            assert_eq!(player_8479325_2_coord.y, 923.9213);
            let player_8477956_2_coord = &goal_255.player_tracking[1].clone().unwrap()[&PlayerInfo{id: 8477956, team_id: 6}];
            assert_eq!(player_8477956_2_coord.x, 1684.3648);
            assert_eq!(player_8477956_2_coord.y, 459.4575);

            // third time step
            assert_eq!(goal_255.player_tracking[2].clone().unwrap().len(), 11); // there are 11 players
            let player_8480280_3_coord = &goal_255.player_tracking[2].clone().unwrap()[&PlayerInfo{id: 8480280, team_id: 6}];
            assert_eq!(player_8480280_3_coord.x, 149.4144);
            assert_eq!(player_8480280_3_coord.y, 513.8132);
            let player_8480051_3_coord = &goal_255.player_tracking[2].clone().unwrap()[&PlayerInfo{id: 8480051, team_id: 8}];
            assert_eq!(player_8480051_3_coord.x, 2231.1672);
            assert_eq!(player_8480051_3_coord.y, 528.4865);
            let player_8477496_3_coord = &goal_255.player_tracking[2].clone().unwrap()[&PlayerInfo{id: 8477496, team_id: 6}];
            assert_eq!(player_8477496_3_coord.x, 2011.9888);
            assert_eq!(player_8477496_3_coord.y, 757.242);
            let player_8473419_3_coord = &goal_255.player_tracking[2].clone().unwrap()[&PlayerInfo{id: 8473419, team_id: 6}];
            assert_eq!(player_8473419_3_coord.x, 1992.5461);
            assert_eq!(player_8473419_3_coord.y, 951.5736);
            let player_8477989_3_coord = &goal_255.player_tracking[2].clone().unwrap()[&PlayerInfo{id: 8477989, team_id: 8}];
            assert_eq!(player_8477989_3_coord.x, 2049.1991);
            assert_eq!(player_8477989_3_coord.y, 754.4976);
            let player_8475233_3_coord = &goal_255.player_tracking[2].clone().unwrap()[&PlayerInfo{id: 8475233, team_id: 8}];
            assert_eq!(player_8475233_3_coord.x, 2060.6813);
            assert_eq!(player_8475233_3_coord.y, 931.107);
            let player_8476875_3_coord = &goal_255.player_tracking[2].clone().unwrap()[&PlayerInfo{id: 8476875, team_id: 8}];
            assert_eq!(player_8476875_3_coord.x, 2166.1038);
            assert_eq!(player_8476875_3_coord.y, 661.754);
            let player_8476981_3_coord = &goal_255.player_tracking[2].clone().unwrap()[&PlayerInfo{id: 8476981, team_id: 8}];
            assert_eq!(player_8476981_3_coord.x, 2058.9758);
            assert_eq!(player_8476981_3_coord.y, 543.9464);
            let player_8478401_3_coord = &goal_255.player_tracking[2].clone().unwrap()[&PlayerInfo{id: 8478401, team_id: 6}];
            assert_eq!(player_8478401_3_coord.x, 1958.5559);
            assert_eq!(player_8478401_3_coord.y, 550.3637);
            let player_8479325_3_coord = &goal_255.player_tracking[2].clone().unwrap()[&PlayerInfo{id: 8479325, team_id: 6}];
            assert_eq!(player_8479325_3_coord.x, 1663.6244);
            assert_eq!(player_8479325_3_coord.y, 920.9213);
            let player_8477956_3_coord = &goal_255.player_tracking[2].clone().unwrap()[&PlayerInfo{id: 8477956, team_id: 6}];
            assert_eq!(player_8477956_3_coord.x, 1682.3648);
            assert_eq!(player_8477956_3_coord.y, 457.4575);

            // fourth time step
            assert_eq!(goal_255.player_tracking[3].clone().unwrap().len(), 11); // there are 11 players
            let player_8480280_4_coord = &goal_255.player_tracking[3].clone().unwrap()[&PlayerInfo{id: 8480280, team_id: 6}];
            assert_eq!(player_8480280_4_coord.x, 147.4144);
            assert_eq!(player_8480280_4_coord.y, 518.8132);
            let player_8480051_4_coord = &goal_255.player_tracking[3].clone().unwrap()[&PlayerInfo{id: 8480051, team_id: 8}];
            assert_eq!(player_8480051_4_coord.x, 2241.1672);
            assert_eq!(player_8480051_4_coord.y, 531.4865);
            let player_8477496_4_coord = &goal_255.player_tracking[3].clone().unwrap()[&PlayerInfo{id: 8477496, team_id: 6}];
            assert_eq!(player_8477496_4_coord.x, 1996.9888);
            assert_eq!(player_8477496_4_coord.y, 757.242);
            let player_8473419_4_coord = &goal_255.player_tracking[3].clone().unwrap()[&PlayerInfo{id: 8473419, team_id: 6}];
            assert_eq!(player_8473419_4_coord.x, 1998.5461);
            assert_eq!(player_8473419_4_coord.y, 949.5736);
            let player_8477989_4_coord = &goal_255.player_tracking[3].clone().unwrap()[&PlayerInfo{id: 8477989, team_id: 8}];
            assert_eq!(player_8477989_4_coord.x, 2046.1991);
            assert_eq!(player_8477989_4_coord.y, 759.4976);
            let player_8475233_4_coord = &goal_255.player_tracking[3].clone().unwrap()[&PlayerInfo{id: 8475233, team_id: 8}];
            assert_eq!(player_8475233_4_coord.x, 2060.6813);
            assert_eq!(player_8475233_4_coord.y, 936.107);
            let player_8476875_4_coord = &goal_255.player_tracking[3].clone().unwrap()[&PlayerInfo{id: 8476875, team_id: 8}];
            assert_eq!(player_8476875_4_coord.x, 2163.1038);
            assert_eq!(player_8476875_4_coord.y, 661.754);
            let player_8476981_4_coord = &goal_255.player_tracking[3].clone().unwrap()[&PlayerInfo{id: 8476981, team_id: 8}];
            assert_eq!(player_8476981_4_coord.x, 2054.9758);
            assert_eq!(player_8476981_4_coord.y, 548.9464);
            let player_8478401_4_coord = &goal_255.player_tracking[3].clone().unwrap()[&PlayerInfo{id: 8478401, team_id: 6}];
            assert_eq!(player_8478401_4_coord.x, 1962.5559);
            assert_eq!(player_8478401_4_coord.y, 554.3637);
            let player_8479325_4_coord = &goal_255.player_tracking[3].clone().unwrap()[&PlayerInfo{id: 8479325, team_id: 6}];
            assert_eq!(player_8479325_4_coord.x, 1666.6244);
            assert_eq!(player_8479325_4_coord.y, 927.9213);
            let player_8477956_4_coord = &goal_255.player_tracking[3].clone().unwrap()[&PlayerInfo{id: 8477956, team_id: 6}];
            assert_eq!(player_8477956_4_coord.x, 1688.3648);
            assert_eq!(player_8477956_4_coord.y, 459.4575);

            // game 2024020011, goal 298
            // this goal has puck data for every instance
            let goal_298 = &goal_loc_data[&GameId(2024020011)][&GoalId(298)];
            assert_eq!(goal_298.puck_tracking.coords.len(), 5);

            let first_coord = goal_298.puck_tracking.coords[0].clone().unwrap();
            assert_eq!(first_coord.x, 1145.52);
            assert_eq!(first_coord.y, 756.8541);
            
            let second_coord = goal_298.puck_tracking.coords[1].clone().unwrap();
            assert_eq!(second_coord.x, 1146.);
            assert_eq!(second_coord.y, 757.8541);

            let third_coord = goal_298.puck_tracking.coords[2].clone().unwrap();
            assert_eq!(third_coord.x, 1147.81);
            assert_eq!(third_coord.y, 758.8541);

            let fourth_coord = goal_298.puck_tracking.coords[3].clone().unwrap();
            assert_eq!(fourth_coord.x, 1204.3541);
            assert_eq!(fourth_coord.y, 759.8541);

            let fifth_coord = goal_298.puck_tracking.coords[4].clone().unwrap();
            assert_eq!(fifth_coord.x, 1145.52);
            assert_eq!(fifth_coord.y, 777.8541);
        }
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
        let (starting_pt, cutoff_pt) = trim_goal_coords(&rot_goal);
        let (trimmed, timesteps) = subset_coords(&rot_goal, starting_pt, cutoff_pt);
        
        assert_eq!(trimmed.coords.len(), rot_goal.coords.len());
        assert_eq!(trimmed.coords[0].x, 10.);
        assert_eq!(trimmed.coords[0].y, 11.);

        assert_eq!(trimmed.coords[1].x, 11.);
        assert_eq!(trimmed.coords[1].y, 11.);
        
        assert_eq!(trimmed.coords[2].x, 12.);
        assert_eq!(trimmed.coords[2].y, 11.);

        assert_eq!(timesteps, vec![
            0, 1, 2
        ]);
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
        let (starting_pt, cutoff_pt) = trim_goal_coords(&rot_goal);
        let (trimmed, timesteps) = subset_coords(&rot_goal, starting_pt, cutoff_pt);

        assert_eq!(trimmed.coords.len(), 3);
        assert_eq!(trimmed.coords[0].x, 10.);
        assert_eq!(trimmed.coords[0].y, 11.);

        assert_eq!(trimmed.coords[1].x, 11.);
        assert_eq!(trimmed.coords[1].y, 11.);
        
        assert_eq!(trimmed.coords[2].x, 12.);
        assert_eq!(trimmed.coords[2].y, 11.);

        assert_eq!(starting_pt, 3);
        assert_eq!(timesteps, vec![
            0, 1, 2
        ]);
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
        let (starting_pt, cutoff_pt) = trim_goal_coords(&rot_goal);
        let (trimmed, timesteps) = subset_coords(&rot_goal, starting_pt, cutoff_pt);

        assert_eq!(trimmed.coords.len(), 3);
        assert_eq!(trimmed.coords[0].x, 10.);
        assert_eq!(trimmed.coords[0].y, 11.);

        assert_eq!(trimmed.coords[1].x, 11.);
        assert_eq!(trimmed.coords[1].y, 11.);
        
        assert_eq!(trimmed.coords[2].x, 12.);
        assert_eq!(trimmed.coords[2].y, 11.);

        assert_eq!(timesteps, vec![
            0, 1, 2
        ]);
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
        let (starting_pt, cutoff_pt) = trim_goal_coords(&rot_goal);
        let (trimmed, timesteps) = subset_coords(&rot_goal, starting_pt, cutoff_pt);

        assert_eq!(trimmed.coords.len(), 3);
        assert_eq!(trimmed.coords[0].x, 10.);
        assert_eq!(trimmed.coords[0].y, 11.);

        assert_eq!(trimmed.coords[1].x, 11.);
        assert_eq!(trimmed.coords[1].y, 11.);
        
        assert_eq!(trimmed.coords[2].x, 12.);
        assert_eq!(trimmed.coords[2].y, 11.);

        assert_eq!(timesteps, vec![
            0, 1, 2
        ]);
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
        let (starting_pt, cutoff_pt) = trim_goal_coords(&rot_goal);
        let (trimmed, timesteps) = subset_coords(&rot_goal, starting_pt, cutoff_pt);

        assert_eq!(trimmed.coords.len(), 4);
        assert_eq!(trimmed.coords[0].x, 10.);
        assert_eq!(trimmed.coords[0].y, 11.);

        assert_eq!(trimmed.coords[1].x, 11.);
        assert_eq!(trimmed.coords[1].y, 11.);
        
        assert_eq!(trimmed.coords[2].x, 12.);
        assert_eq!(trimmed.coords[2].y, 11.);

        assert_eq!(trimmed.coords[3].x, 13.);
        assert_eq!(trimmed.coords[3].y, 11.);

        assert_eq!(timesteps, vec![
            0, 3, 5, 6
        ]);
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
        let (starting_pt, cutoff_pt) = trim_goal_coords(&rot_goal);
        let (trimmed, timesteps) = subset_coords(&rot_goal, starting_pt, cutoff_pt);

        assert_eq!(trimmed.coords.len(), 4);
        assert_eq!(trimmed.coords[0], Coord { x: 10., y: 11. });
        assert_eq!(trimmed.coords[1], Coord { x: 11., y: 11. });
        assert_eq!(trimmed.coords[2], Coord { x: 12., y: 11. });
        assert_eq!(trimmed.coords[3], Coord { x: 13., y: 11. });

        assert_eq!(timesteps, vec![
            0, 3, 5, 6
        ]);
        assert_eq!(starting_pt, 1);
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
            Some(Coord { x: 11., y: 11. }), // starts here
            Some(Coord { x: 12., y: 11. }),
            Some(Coord { x: 13., y: 11. }),
        ];
        let rot_goal = RotatedPuckLocations { coords };
        let (starting_pt, cutoff_pt) = trim_goal_coords(&rot_goal);
        let (trimmed, timesteps) = subset_coords(&rot_goal, starting_pt, cutoff_pt);
        
        assert_eq!(trimmed.coords.len(), 3);        
        assert_eq!(trimmed.coords[0], Coord { x: 11., y: 11. });
        assert_eq!(trimmed.coords[1], Coord { x: 12., y: 11. });
        assert_eq!(trimmed.coords[2], Coord { x: 13., y: 11. });

        assert_eq!(timesteps, vec![
            0 , 1, 2
        ]);
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
            Some(Coord { x: 11., y: 11. }),  // starts here
            Some(Coord { x: 12., y: 11. }),
            Some(Coord { x: 13., y: 11. }),
        ];
        let rot_goal = RotatedPuckLocations { coords };
        let (starting_pt, cutoff_pt) = trim_goal_coords(&rot_goal);
        let (trimmed, timesteps) = subset_coords(&rot_goal, starting_pt, cutoff_pt);
        
        assert_eq!(trimmed.coords.len(), 3);        
        assert_eq!(trimmed.coords[0], Coord { x: 11., y: 11. });
        assert_eq!(trimmed.coords[1], Coord { x: 12., y: 11. });
        assert_eq!(trimmed.coords[2], Coord { x: 13., y: 11. });

        assert_eq!(timesteps, vec![
            0, 1, 2
        ]);
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
            Some(Coord { x: 11., y: 11. }), // starts here
            Some(Coord { x: 12., y: 11. }),
            Some(Coord { x: 13., y: 11. }),
        ];
        let rot_goal = RotatedPuckLocations { coords };
        let (starting_pt, cutoff_pt) = trim_goal_coords(&rot_goal);
        let (trimmed, timesteps) = subset_coords(&rot_goal, starting_pt, cutoff_pt);
        
        assert_eq!(trimmed.coords.len(), 3);
        // assert_eq!(trimmed.coords[0], Coord { x: 955.2, y: 210.1 });
        assert_eq!(trimmed.coords[0], Coord { x: 11., y: 11. });
        assert_eq!(trimmed.coords[1], Coord { x: 12., y: 11. });
        assert_eq!(trimmed.coords[2], Coord { x: 13., y: 11. });

        assert_eq!(timesteps, vec![
            0, 1, 2
        ]);
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
            Some(Coord { x: 11., y: 11. }), // starts here
            Some(Coord { x: 12., y: 11. }),
            Some(Coord { x: 13., y: 11. }),
        ];
        let rot_goal = RotatedPuckLocations { coords };
        let (starting_pt, cutoff_pt) = trim_goal_coords(&rot_goal);
        let (trimmed, timesteps) = subset_coords(&rot_goal, starting_pt, cutoff_pt);
        
        assert_eq!(trimmed.coords.len(), 3);
        assert_eq!(trimmed.coords[0], Coord { x: 11., y: 11. });
        assert_eq!(trimmed.coords[1], Coord { x: 12., y: 11. });
        assert_eq!(trimmed.coords[2], Coord { x: 13., y: 11. });

        assert_eq!(timesteps, vec![
            0, 1, 2
        ]);
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
        let (starting_pt, cutoff_pt) = trim_goal_coords(&rot_goal);
        let (trimmed, timesteps) = subset_coords(&rot_goal, starting_pt, cutoff_pt);
        
        assert_eq!(trimmed.coords.len(), 3);        
        assert_eq!(trimmed.coords[0], Coord { x: 11., y: 11. });
        assert_eq!(trimmed.coords[1], Coord { x: 12., y: 11. });
        assert_eq!(trimmed.coords[2], Coord { x: 13., y: 11. });

        assert_eq!(timesteps, vec![
            0, 1, 2
        ]);
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
            Some(Coord { x: 11., y: 11. }), // starts here
            Some(Coord { x: 12., y: 11. }),
            Some(Coord { x: 13., y: 11. }),
        ];
        let rot_goal = RotatedPuckLocations { coords };
        let (starting_pt, cutoff_pt) = trim_goal_coords(&rot_goal);
        let (trimmed, timesteps) = subset_coords(&rot_goal, starting_pt, cutoff_pt);
        
        assert_eq!(trimmed.coords.len(), 3);        
        assert_eq!(trimmed.coords[0], Coord { x: 11., y: 11. });
        assert_eq!(trimmed.coords[1], Coord { x: 12., y: 11. });
        assert_eq!(trimmed.coords[2], Coord { x: 13., y: 11. });

        assert_eq!(timesteps, vec![
            0, 1, 2
        ]);
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
            Some(Coord { x: 11., y: 11. }), // starts here
            Some(Coord { x: 12., y: 11. }),
            Some(Coord { x: 13., y: 11. }),
        ];
        let rot_goal = RotatedPuckLocations { coords };
        let (starting_pt, cutoff_pt) = trim_goal_coords(&rot_goal);
        let (trimmed, timesteps) = subset_coords(&rot_goal, starting_pt, cutoff_pt);
        
        assert_eq!(trimmed.coords.len(), 3);
        assert_eq!(trimmed.coords[0], Coord { x: 11., y: 11. });
        assert_eq!(trimmed.coords[1], Coord { x: 12., y: 11. });
        assert_eq!(trimmed.coords[2], Coord { x: 13., y: 11. });

        assert_eq!(timesteps, vec![
            0, 1, 2
        ]);
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
            Some(Coord { x: 11., y: 11. }),  // starts here
            Some(Coord { x: 12., y: 11. }),
            Some(Coord { x: 13., y: 11. }),
        ];
        let rot_goal = RotatedPuckLocations { coords };
        let (starting_pt, cutoff_pt) = trim_goal_coords(&rot_goal);
        let (trimmed, timesteps) = subset_coords(&rot_goal, starting_pt, cutoff_pt);
        
        assert_eq!(trimmed.coords.len(), 3);
        assert_eq!(trimmed.coords[0], Coord { x: 11., y: 11. });
        assert_eq!(trimmed.coords[1], Coord { x: 12., y: 11. });
        assert_eq!(trimmed.coords[2], Coord { x: 13., y: 11. });

        assert_eq!(timesteps, vec![
            0, 1, 2
        ]);
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
            Some(Coord { x: 11., y: 11. }), // starts here
            Some(Coord { x: 12., y: 11. }),
            Some(Coord { x: 13., y: 11. }),
        ];
        let rot_goal = RotatedPuckLocations { coords };
        let (starting_pt, cutoff_pt) = trim_goal_coords(&rot_goal);
        let (trimmed, timesteps) = subset_coords(&rot_goal, starting_pt, cutoff_pt);
        
        assert_eq!(trimmed.coords.len(), 3);
        assert_eq!(trimmed.coords[0], Coord { x: 11., y: 11. });
        assert_eq!(trimmed.coords[1], Coord { x: 12., y: 11. });
        assert_eq!(trimmed.coords[2], Coord { x: 13., y: 11. });

        assert_eq!(timesteps, vec![
            0, 1, 2
        ]);
    }

    // faceoff goal w/ None's in the middle of the faceoff data that should be
    // trimmed
    #[test]
    fn trim_goal_faceoff_nones_middle() {
        let coords = vec![
            Some(Coord { x: 2035.4955, y: 216.197 }),
            Some(Coord { x: 2035.8623, y: 218.18690000000004 }),
            Some(Coord { x: 2035.8656, y: 219.24480000000005 }),
            Some(Coord { x: 2035.8115, y: 219.71659999999997 }),
            Some(Coord { x: 2035.7099, y: 219.83069999999998 }),

            Some(Coord { x: 2035.2651, y: 219.85130000000004 }),
            Some(Coord { x: 2035.2201, y: 219.3994 }),
            None,
            None,
            None,

            None,
            None,
            Some(Coord { x: 2021.6512, y: 236.29219999999998 }),
            Some(Coord { x: 2020.2434, y: 239.3139 }),
            Some(Coord { x: 2018.282, y: 242.452 }),

            Some(Coord { x: 2019.4605999999999, y: 247.99440000000004 }), // starts here
            Some(Coord { x: 2028.5012, y: 269.7925 }),
            Some(Coord { x: 2021.4669, y: 270.52189999999996 }),
            Some(Coord { x: 2173.5117, y: 418.5711 }),
            Some(Coord { x: 2172.869, y: 423.44820000000004 }),
            
            Some(Coord { x: 2232.2745, y: 483.49969999999996 }),  // enters goal here
        ];
        let rot_goal = RotatedPuckLocations { coords };
        let (starting_pt, cutoff_pt) = trim_goal_coords(&rot_goal);

        assert_eq!(starting_pt, 15);
        assert!(cutoff_pt.is_none());
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
        let (starting_pt, cutoff_pt) = trim_goal_coords(&rot_goal);
        let (trimmed, timesteps) = subset_coords(&rot_goal, starting_pt, cutoff_pt);
        
        assert_eq!(trimmed.coords.len(), 7);
        assert_eq!(trimmed.coords[0], Coord { x: 62.81, y: 522.26 });
        assert_eq!(trimmed.coords[1], Coord { x: 62.82, y: 522.27 });
        assert_eq!(trimmed.coords[2], Coord { x: 62.83, y: 522.26 });
        assert_eq!(trimmed.coords[3], Coord { x: 62.81, y: 522.25 });
        assert_eq!(trimmed.coords[4], Coord { x: 62.82, y: 522.24 });
        assert_eq!(trimmed.coords[5], Coord { x: 62.80, y: 522.26 });
        assert_eq!(trimmed.coords[6], Coord { x: 13., y: 11. });

        assert_eq!(timesteps, vec![
            0, 1, 2, 3, 4, 5, 6
        ]);
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
        let (starting_pt, cutoff_pt) = trim_goal_coords(&rot_goal);
        let (trimmed, timesteps) = subset_coords(&rot_goal, starting_pt, cutoff_pt);
        
        assert_eq!(trimmed.coords.len(), 3);
        assert_eq!(trimmed.coords[0], Coord { x: 1010.5, y: 900.5 });
        assert_eq!(trimmed.coords[1], Coord { x: 2019.9, y: 776.4 });
        assert_eq!(trimmed.coords[2], Coord { x: 2250.4, y: 500.1444 });

        assert_eq!(timesteps, vec![
            0, 1, 2
        ]);
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
        let (starting_pt, cutoff_pt) = trim_goal_coords(&rot_goal);
        let (trimmed, timesteps) = subset_coords(&rot_goal, starting_pt, cutoff_pt);

        assert_eq!(trimmed.coords.len(), 5);
        assert_eq!(trimmed.coords[0], Coord { x: 1010.5, y: 900.5 });
        assert_eq!(trimmed.coords[1], Coord { x: 2019.9, y: 776.4 });
        assert_eq!(trimmed.coords[2], Coord { x: 2250.4, y: 500.1444 });
        assert_eq!(trimmed.coords[3], Coord { x: 2020.3, y: 775.6 });
        assert_eq!(trimmed.coords[4], Coord { x: 2246.1, y: 545.3 });

        assert_eq!(timesteps, vec![
            0, 1, 2, 3, 4
        ]);
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
        let (starting_pt, cutoff_pt) = trim_goal_coords(&rot_goal);
        let (trimmed, timesteps) = subset_coords(&rot_goal, starting_pt, cutoff_pt);

        assert_eq!(trimmed.coords.len(), 6);
        assert_eq!(trimmed.coords[0], Coord { x: 1010.5, y: 900.5 });
        assert_eq!(trimmed.coords[1], Coord { x: 2019.9, y: 776.4 });
        assert_eq!(trimmed.coords[2], Coord { x: 2250.4, y: 500.1444 });
        assert_eq!(trimmed.coords[3], Coord { x: 2020.3, y: 775.6 });
        assert_eq!(trimmed.coords[4], Coord { x: 2020.4, y: 776.6 });
        assert_eq!(trimmed.coords[5], Coord { x: 2246.1, y: 545.3 });

        assert_eq!(timesteps, vec![
            0, 1, 2, 3, 4, 5
        ]);
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
        let (starting_pt, cutoff_pt) = trim_goal_coords(&rot_goal);
        let (trimmed, timesteps) = subset_coords(&rot_goal, starting_pt, cutoff_pt);

        assert_eq!(trimmed.coords.len(), 7);
        assert_eq!(trimmed.coords[0], Coord { x: 1010.5, y: 900.5 });
        assert_eq!(trimmed.coords[1], Coord { x: 2019.9, y: 776.4 });
        assert_eq!(trimmed.coords[2], Coord { x: 2250.4, y: 500.1444 });
        assert_eq!(trimmed.coords[3], Coord { x: 2020.3, y: 775.6 });
        assert_eq!(trimmed.coords[4], Coord { x: 2260.4, y: 559.2 });
        assert_eq!(trimmed.coords[5], Coord { x: 2000.1, y: 800.2 });
        assert_eq!(trimmed.coords[6], Coord { x: 2246.1, y: 545.3 });

        assert_eq!(timesteps, vec![
            0, 1, 2, 3, 4, 5, 6
        ]);
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
        let (starting_pt, cutoff_pt) = trim_goal_coords(&rot_goal);
        let (trimmed, timesteps) = subset_coords(&rot_goal, starting_pt, cutoff_pt);

        assert_eq!(trimmed.coords.len(), 7);
        assert_eq!(trimmed.coords[0], Coord { x: 1010.5, y: 900.5 });
        assert_eq!(trimmed.coords[1], Coord { x: 2019.9, y: 776.4 });
        assert_eq!(trimmed.coords[2], Coord { x: 2250.4, y: 500.1444 });
        assert_eq!(trimmed.coords[3], Coord { x: 2020.3, y: 775.6 });
        assert_eq!(trimmed.coords[4], Coord { x: 2020.4, y: 775.6 });
        assert_eq!(trimmed.coords[5], Coord { x: 2000.1, y: 800.2 });
        assert_eq!(trimmed.coords[6], Coord { x: 2246.1, y: 545.3 });

        assert_eq!(timesteps, vec![
            0, 1, 2, 3, 4, 5, 6
        ]);
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
        let (starting_pt, cutoff_pt) = trim_goal_coords(&rot_goal);
        let (trimmed, timesteps) = subset_coords(&rot_goal, starting_pt, cutoff_pt);

        assert_eq!(trimmed.coords.len(), 4);
        assert_eq!(trimmed.coords[0], Coord { x: 1010.5, y: 900.5 });
        assert_eq!(trimmed.coords[1], Coord { x: 2019.9, y: 776.4 });
        assert_eq!(trimmed.coords[2], Coord { x: 2020.3, y: 775.6 });
        assert_eq!(trimmed.coords[3], Coord { x: 2250.4, y: 500.1444 });

        assert_eq!(timesteps, vec![
            0, 1, 2, 3
        ]);
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
        let (starting_pt, cutoff_pt) = trim_goal_coords(&rot_goal);
        let (trimmed, timesteps) = subset_coords(&rot_goal, starting_pt, cutoff_pt);

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

        assert_eq!(timesteps, vec![
            0, 1, 2, 3, 4, 5, 6, 7, 8, 9
        ]);
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
        let (starting_pt, cutoff_pt) = trim_goal_coords(&rot_goal);
        let (trimmed, timesteps) = subset_coords(&rot_goal, starting_pt, cutoff_pt);

        assert_eq!(trimmed.coords.len(), 73);
        assert_eq!(trimmed.coords[72], Coord { x: 2250., y: 541. });

        assert_eq!(timesteps, (0..=72).collect::<Vec<usize>>());
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
        let (starting_pt, cutoff_pt) = trim_goal_coords(&rot_goal);
        let (trimmed, timesteps) = subset_coords(&rot_goal, starting_pt, cutoff_pt);

        assert_eq!(trimmed.coords.len(), 4);
        assert_eq!(trimmed.coords[0], Coord { x: 1010.5, y: 900.5 });
        assert_eq!(trimmed.coords[1], Coord { x: 2019.9, y: 776.4 });
        assert_eq!(trimmed.coords[2], Coord { x: 2020.3, y: 775.6 });
        assert_eq!(trimmed.coords[3], Coord { x: 2250.4, y: 500.1444 });

        assert_eq!(timesteps, vec![
            0, 1, 2, 3
        ]);
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
        let (starting_pt, cutoff_pt) = trim_goal_coords(&rot_goal);
        let (trimmed, timesteps) = subset_coords(&rot_goal, starting_pt, cutoff_pt);

        assert_eq!(trimmed.coords.len(), 56);
        assert_eq!(trimmed.coords[55], Coord { x: 2249., y: 540. });

        assert_eq!(timesteps, (0..=55).collect::<Vec<usize>>());
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
        let (starting_pt, cutoff_pt) = trim_goal_coords(&rot_goal);
        let (trimmed, timesteps) = subset_coords(&rot_goal, starting_pt, cutoff_pt);

        assert_eq!(trimmed.coords.len(), 66);
        assert_eq!(trimmed.coords[65], Coord { x: 2249., y: 540.});

        assert_eq!(timesteps, (0..=65).collect::<Vec<usize>>());
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
        let (starting_pt, cutoff_pt) = trim_goal_coords(&rot_goal);
        let (trimmed, timesteps) = subset_coords(&rot_goal, starting_pt, cutoff_pt);

        assert_eq!(trimmed.coords.len(), 66);
        assert_eq!(trimmed.coords[65], Coord { x: 2249., y: 540.});

        assert_eq!(timesteps, (0..=65).collect::<Vec<usize>>());
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
        let (starting_pt, cutoff_pt) = trim_goal_coords(&rot_goal);
        let (trimmed, timesteps) = subset_coords(&rot_goal, starting_pt, cutoff_pt);

        assert_eq!(trimmed.coords.len(), 16);
        assert_eq!(trimmed.coords[15], Coord { x: 2249., y: 540.});

        assert_eq!(timesteps, (0..=15).collect::<Vec<usize>>());
    }

    // start of goal 2025020182/160, where there is a discrepancy between
    // the old code and new code results due to having None's at the start
    // and being in faceoff at start as well
    #[test]
    fn trim_goal_none_start_faceoff() {
        let coords = vec![
            None,
            None,
            Some(Coord { x: 1211.9485, y: 498.5147 }),
            Some(Coord { x: 1230.1508, y: 483.0261 }),
            Some(Coord { x: 1248.6075, y: 467.4361 }),

            Some(Coord { x: 1266.5463, y: 451.9476 }),
            Some(Coord { x: 1283.9319, y: 436.5517 }),
        ];
        let rot_goal = RotatedPuckLocations { coords };
        let (starting_pt, cutoff_pt) = trim_goal_coords(&rot_goal);
        // let (starting_pt, cutoff_pt) = trim_goal_coords_draft(&rot_goal);
        let (trimmed, timesteps) = subset_coords(&rot_goal, starting_pt, cutoff_pt);
        println!("starting_pt: {starting_pt}");
        assert_eq!(trimmed.coords.len(), 4);
        assert_eq!(timesteps.len(), 4);
    }

    // --------------------------------------------------
    // preprocess_folder_data() tests
    // --------------------------------------------------

    // // folder with mix of goals to rotate and to not rotate, as well as missing
    // // goal info
    // #[test]
    // fn preprocess_folder_no_rotate() {
    //     let mut pbp_data_map = HashMap::new();
    //     let goal_details = vec![
    //         GoalDetails { event_id: 10, scoring_team_id: 19, home_team_defending_side: String::from("Left"), ppt_replay_url: Some(String::from("")) },
    //         GoalDetails { event_id: 1001, scoring_team_id: 19, home_team_defending_side: String::from("Right"), ppt_replay_url: Some(String::from("")) },
    //         GoalDetails { event_id: 485, scoring_team_id: 3, home_team_defending_side: String::from("Right"), ppt_replay_url: Some(String::from("")) },
    //         GoalDetails { event_id: 740, scoring_team_id: 3, home_team_defending_side: String::from("Right"), ppt_replay_url: Some(String::from("")) },
    //     ];
    //     let pbp_data = PbpData {
    //         goals: goal_details,
    //         home_team_id: 19
    //     };
    //     let game_id = GameId(202400001);
    //     pbp_data_map.insert(game_id, pbp_data);

    //     let mut game_goals_map = HashMap::new();
    //     let mut goal_id_to_loc_map = HashMap::new();

    //     // create raw puck location data
    //     // not rotated
    //     let goal_10 = RawPuckLocationData { coords: vec![       
    //         Some(Coord { x: 1010.5, y: 900.5 }),
    //         Some(Coord { x: 2019.9, y: 776.4 }),
    //         Some(Coord { x: 2500., y: 1022. }),
    //         Some(Coord { x: 2250.4, y: 500.1444 }), // enters here and should be trimmed here          
    //         Some(Coord { x: 2250.3, y: 500.1443 }),
            
    //         Some(Coord { x: 2000.1, y: 800.2 }),
    //         Some(Coord { x: 2000.1, y: 800.2 }),
    //         Some(Coord { x: 12., y: 11. }),            
    //         Some(Coord { x: 2000.1, y: 800.2 }),
    //         Some(Coord { x: 2000.1, y: 800.2 }),
    //     ]};
    //     goal_id_to_loc_map.insert(GoalId(10), goal_10);

    //     // goal should be rotated
    //     let goal_1001 = RawPuckLocationData { coords: vec![       
    //         Some(Coord { x: 1010., y: 900. }),
    //         Some(Coord { x: 2019., y: 776. }),
    //         Some(Coord { x: 2020., y: 775. }),
    //         Some(Coord { x: -7.1, y: 101. }),           
    //         Some(Coord { x: 701., y: 1020. }),
            
    //         Some(Coord { x: 704., y: 104. }),
    //         Some(Coord { x: 702., y: 150. }),
    //         Some(Coord { x: 12., y: 11. }),                        
    //         Some(Coord { x: 150., y: 501. }), // enters here again and should be trimmed here            
    //         Some(Coord { x: 150., y: 501. }),
    //     ] };
    //     goal_id_to_loc_map.insert(GoalId(1001), goal_1001);

    //     // not rotated
    //     let goal_485 = RawPuckLocationData { coords: vec![       
    //         Some(Coord { x: 1010.5, y: 900.5 }),
    //         Some(Coord { x: 2019.9, y: 776.4 }),
    //         Some(Coord { x: 2250.4, y: 500.1444 }), // enters here
    //         Some(Coord { x: 2020.3, y: 775.6 }),
    //         Some(Coord { x: 2246.1, y: 545.3 }), // enters again here and should be trimmed here

    //         Some(Coord { x: 12., y: 11. }),
    //         Some(Coord { x: 13., y: 11. }),
    //         Some(Coord { x: 13., y: 11. }),
    //         Some(Coord { x: 13., y: 11. }),
    //         Some(Coord { x: 13., y: 11. }),
    //     ] };
    //     goal_id_to_loc_map.insert(GoalId(485), goal_485);

    //     game_goals_map.insert(game_id, goal_id_to_loc_map);
    //     let folder_data = PbpGoalLocData { 
    //         pbp_data: pbp_data_map,
    //         goal_loc_data: game_goals_map
    //     };
    //     let preprocessed_data = preprocess_folder_data(&folder_data);
    //     assert_eq!(preprocessed_data.len(), 3);

    //     assert_eq!(preprocessed_data[&(game_id, GoalId(10))], TrimmedPuckLocations { coords: vec![
    //         Coord { x: 1010.5, y: 900.5 },
    //         Coord { x: 2019.9, y: 776.4 },
    //         Coord { x: 2400., y: 1015. },
    //         Coord { x: 2250.4, y: 500.1444 },
    //     ]});
    //     assert_eq!(preprocessed_data[&(game_id, GoalId(1001))], TrimmedPuckLocations { coords: vec![
    //         Coord { x: 1390., y: 115. },
    //         Coord { x: 381., y: 239. },
    //         Coord { x: 380., y: 240. },
    //         Coord { x: 2400., y: 914. },           
    //         Coord { x: 1699., y: 0. },
            
    //         Coord { x: 1696., y: 911. },
    //         Coord { x: 1698., y: 865. },
    //         Coord { x: 2388., y: 1004. },                        
    //         Coord { x: 2250., y: 514. },
    //     ]});
    //     assert_eq!(preprocessed_data[&(game_id, GoalId(485))], TrimmedPuckLocations { coords: vec![
    //         Coord { x: 1010.5, y: 900.5 },
    //         Coord { x: 2019.9, y: 776.4 },
    //         Coord { x: 2250.4, y: 500.1444 },
    //         Coord { x: 2020.3, y: 775.6 },
    //         Coord { x: 2246.1, y: 545.3 },
    //     ]});
    // }

    // --------------------------------------------------
    // preprocess_puck() tests
    // --------------------------------------------------

    #[test]
    fn preprocess_puck_no_rotate_trim() {
        let goal_details = GoalDetails { event_id: 10, scoring_team_id: 19, home_team_defending_side: String::from("Left"), ppt_replay_url: Some(String::from("")) };
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
        let home_team_id = 19;

        let (trimmed_puck_locs, starting_pt, cutoff_pt, timesteps) = preprocess_puck(&goal_10, &goal_details, home_team_id);
        assert_eq!(trimmed_puck_locs, TrimmedPuckLocations { coords: vec![
            Coord { x: 1010.5, y: 900.5 },
            Coord { x: 2019.9, y: 776.4 },
            Coord { x: 2400., y: 1015. },
            Coord { x: 2250.4, y: 500.1444 },
        ]});
        assert_eq!(starting_pt, 0);
        assert_eq!(cutoff_pt, Some(3));
        assert_eq!(timesteps, vec![
            0, 1, 2, 3
        ])
    }

    #[test]
    fn preprocess_puck_rotate_trim() {
        let goal_details = GoalDetails { event_id: 10, scoring_team_id: 19, home_team_defending_side: String::from("Right"), ppt_replay_url: Some(String::from("")) };
        let goal_10 = RawPuckLocationData { coords: vec![
            Some(Coord { x: 1390., y: 114.5 }),
            Some(Coord { x: 381., y: 239. }),
            Some(Coord { x: 20., y: 1. }),
            Some(Coord { x: 150., y: 515. }),
            Some(Coord { x: 150., y: 515.1443 }),
        ]};
        let home_team_id = 19;

        let (trimmed_puck_locs, starting_pt, cutoff_pt, timesteps) = preprocess_puck(&goal_10, &goal_details, home_team_id);
        assert_eq!(trimmed_puck_locs, TrimmedPuckLocations { coords: vec![
            Coord { x: 1010., y: 900.5 },
            Coord { x: 2019., y: 776. },
            Coord { x: 2380., y: 1014. },
            Coord { x: 2250., y: 500. },
        ]});
        assert_eq!(starting_pt, 0);
        assert_eq!(cutoff_pt, Some(3));

        assert_eq!(timesteps, vec![
            0, 1, 2, 3
        ]);
    }

    // --------------------------------------------------
    // subset_coords() test
    // --------------------------------------------------

    // no timestamps missing puck locations
    #[test]
    fn subset_coords_full() {
        let rot_goal = RotatedPuckLocations { coords: vec![
            Some(Coord { x: 2190., y: 143.2 }),
            Some(Coord { x: 2200.2, y: 141.2 }),
            Some(Coord { x: 2190.1, y: 138.6 }),
            Some(Coord { x: 1987.2, y: 132.7 }),
            Some(Coord { x: 1887.4, y: 120.2 }),

            Some(Coord { x: 2190., y: 143.2 }),
            Some(Coord { x: 2190., y: 143.2 }),
            Some(Coord { x: 2190., y: 143.2 }),
            Some(Coord { x: 2190., y: 143.2 }),
            Some(Coord { x: 2190., y: 143.2 }),
        ]};
        let starting_pt = 0;
        let cutoff_pt = Some(9);
        let (trimmed, timesteps) = subset_coords(&rot_goal, starting_pt, cutoff_pt);

        assert_eq!(trimmed, TrimmedPuckLocations { coords: vec![
            Coord { x: 2190., y: 143.2 },
            Coord { x: 2200.2, y: 141.2 },
            Coord { x: 2190.1, y: 138.6 },
            Coord { x: 1987.2, y: 132.7 },
            Coord { x: 1887.4, y: 120.2 },

            Coord { x: 2190., y: 143.2 },
            Coord { x: 2190., y: 143.2 },
            Coord { x: 2190., y: 143.2 },
            Coord { x: 2190., y: 143.2 },
            Coord { x: 2190., y: 143.2 },
        ]});
        assert_eq!(timesteps, vec![
            0, 1, 2, 3, 4, 5, 6, 7, 8, 9
        ]);
    }

    // one timestamp missing puck location
    #[test]
    fn subset_coords_one_missing() {
        let rot_goal = RotatedPuckLocations { coords: vec![
            None,
            Some(Coord { x: 2200.2, y: 141.2 }),
            Some(Coord { x: 2190.1, y: 138.6 }),
            Some(Coord { x: 1987.2, y: 132.7 }),
            Some(Coord { x: 1887.4, y: 120.2 }),

            Some(Coord { x: 2190., y: 143.2 }),
            Some(Coord { x: 2190., y: 143.2 }),
            Some(Coord { x: 2190., y: 143.2 }),
            Some(Coord { x: 2190., y: 143.2 }),
            Some(Coord { x: 2190., y: 143.2 }),
        ]};
        let starting_pt = 0;
        let cutoff_pt = None;
        let (trimmed, timesteps) = subset_coords(&rot_goal, starting_pt, cutoff_pt);

        assert_eq!(trimmed, TrimmedPuckLocations { coords: vec![
            Coord { x: 2200.2, y: 141.2 },
            Coord { x: 2190.1, y: 138.6 },
            Coord { x: 1987.2, y: 132.7 },
            Coord { x: 1887.4, y: 120.2 },

            Coord { x: 2190., y: 143.2 },
            Coord { x: 2190., y: 143.2 },
            Coord { x: 2190., y: 143.2 },
            Coord { x: 2190., y: 143.2 },
            Coord { x: 2190., y: 143.2 },
        ]});
        assert_eq!(timesteps, vec![
            1, 2, 3, 4, 5, 6, 7, 8, 9
        ]);
    }

    // multiple timestamps missing puck location
    #[test]
    fn subset_coords_many_missing() {
        let rot_goal = RotatedPuckLocations { coords: vec![
            None,
            Some(Coord { x: 2200.2, y: 141.2 }),
            Some(Coord { x: 2190.1, y: 138.6 }),
            Some(Coord { x: 1987.2, y: 132.7 }),
            Some(Coord { x: 1887.4, y: 120.2 }),

            None,
            Some(Coord { x: 2190., y: 143.2 }),
            Some(Coord { x: 2191., y: 143.2 }),
            Some(Coord { x: 2190., y: 144.2 }),
            None
        ]};
        let starting_pt = 0;
        let cutoff_pt = None;
        let (trimmed, timesteps) = subset_coords(&rot_goal, starting_pt, cutoff_pt);

        assert_eq!(trimmed, TrimmedPuckLocations { coords: vec![
            Coord { x: 2200.2, y: 141.2 },
            Coord { x: 2190.1, y: 138.6 },
            Coord { x: 1987.2, y: 132.7 },
            Coord { x: 1887.4, y: 120.2 },

            Coord { x: 2190., y: 143.2 },
            Coord { x: 2191., y: 143.2 },
            Coord { x: 2190., y: 144.2 },
        ]});
        assert_eq!(timesteps, vec![
            1, 2, 3, 4, 6, 7, 8
        ]);
    }

    // test all timesteps are missing puck location
    #[test]
    fn subset_coords_all_missing() {
        let rot_goal = RotatedPuckLocations { coords: vec![
            None,
            None,
            None,
            None
        ]};
        let starting_pt = 0;
        let cutoff_pt = None;
        let (trimmed, timesteps) = subset_coords(&rot_goal, starting_pt, cutoff_pt);

        assert_eq!(trimmed, TrimmedPuckLocations { coords: vec![]});
        let expected_timesteps: Vec<usize> = vec![];
        assert_eq!(timesteps, expected_timesteps);
    }

    // test cutoff all but one timestep
    #[test]
    fn subset_coords_cutoff_all_but_one() {
        let rot_goal = RotatedPuckLocations { coords: vec![
            Some(Coord { x: 2190., y: 143.2 }),
            Some(Coord { x: 2200.2, y: 141.2 }),
            Some(Coord { x: 2190.1, y: 138.6 }),
            Some(Coord { x: 1987.2, y: 132.7 }),
            Some(Coord { x: 1887.4, y: 120.2 }),

            Some(Coord { x: 2190., y: 143.2 }),
            Some(Coord { x: 2190., y: 143.2 }),
            Some(Coord { x: 2190., y: 143.2 }),
            Some(Coord { x: 2190., y: 143.2 }),
            Some(Coord { x: 2190., y: 143.2 }),
        ]};
        let starting_pt = 0;
        let cutoff_pt = Some(0);
        let (trimmed, timesteps) = subset_coords(&rot_goal, starting_pt, cutoff_pt);

        assert_eq!(trimmed, TrimmedPuckLocations { coords: vec![
            Coord { x: 2190., y: 143.2 },
        ]});
        assert_eq!(timesteps, vec![
            0
        ]);
    }

    // test cutoff point being in middle
    #[test]
    fn subset_coords_cutoff_mid() {
        let rot_goal = RotatedPuckLocations { coords: vec![
            Some(Coord { x: 2190., y: 143.2 }),
            Some(Coord { x: 2200.2, y: 141.2 }),
            Some(Coord { x: 2190.1, y: 138.6 }),
            Some(Coord { x: 1987.2, y: 132.7 }),
            Some(Coord { x: 1887.4, y: 120.2 }),

            Some(Coord { x: 2190., y: 143.2 }),
            Some(Coord { x: 2190., y: 143.2 }),
            Some(Coord { x: 2190., y: 143.2 }),
            Some(Coord { x: 2190., y: 143.2 }),
            Some(Coord { x: 2190., y: 143.2 }),
        ]};
        let starting_pt = 0;
        let cutoff_pt = Some(5);
        let (trimmed, timesteps) = subset_coords(&rot_goal, starting_pt, cutoff_pt);

        assert_eq!(trimmed, TrimmedPuckLocations { coords: vec![
            Coord { x: 2190., y: 143.2 },
            Coord { x: 2200.2, y: 141.2 },
            Coord { x: 2190.1, y: 138.6 },
            Coord { x: 1987.2, y: 132.7 },
            Coord { x: 1887.4, y: 120.2 },

            Coord { x: 2190., y: 143.2 },
        ]});
        assert_eq!(timesteps, vec![
            0, 1, 2, 3, 4, 5
        ]);
    }

    // test missing locations with cutoff
    #[test]
    fn subset_coords_cutoff_some_missing() {
        let rot_goal = RotatedPuckLocations { coords: vec![
            Some(Coord { x: 2190., y: 143.2 }),
            Some(Coord { x: 2200.2, y: 141.2 }),
            Some(Coord { x: 2190.1, y: 138.6 }),
            None,
            Some(Coord { x: 1887.4, y: 120.2 }),

            Some(Coord { x: 2190., y: 143.2 }),
            None,
            Some(Coord { x: 2198., y: 150.2 }),
            None,
            Some(Coord { x: 2190., y: 143.2 }),
        ]};
        let starting_pt = 0;
        let cutoff_pt = Some(7);
        let (trimmed, timesteps) = subset_coords(&rot_goal, starting_pt, cutoff_pt);

        assert_eq!(trimmed, TrimmedPuckLocations { coords: vec![
            Coord { x: 2190., y: 143.2 },
            Coord { x: 2200.2, y: 141.2 },
            Coord { x: 2190.1, y: 138.6 },
            Coord { x: 1887.4, y: 120.2 },

            Coord { x: 2190., y: 143.2 },
            Coord { x: 2198., y: 150.2 },
        ]});
        assert_eq!(timesteps, vec![
            0, 1, 2, 4, 5, 7
        ]);
    }

    // test starting point is in the middle
    #[test]
    fn subset_coords_mid_starting_pt() {
        let rot_goal = RotatedPuckLocations { coords: vec![
            Some(Coord { x: 2190., y: 143.2 }),
            Some(Coord { x: 2200.2, y: 141.2 }),
            Some(Coord { x: 2190.1, y: 138.6 }),
            Some(Coord { x: 1987.2, y: 132.7 }),
            Some(Coord { x: 1887.4, y: 120.2 }),

            Some(Coord { x: 2190., y: 143.2 }),
            Some(Coord { x: 2193., y: 141.2 }),
            Some(Coord { x: 2194., y: 142.2 }),
            Some(Coord { x: 2195., y: 145.2 }),
            Some(Coord { x: 2196., y: 146.2 }),
        ]};
        let starting_pt = 1;
        let cutoff_pt = None;
        let (trimmed, timesteps) = subset_coords(&rot_goal, starting_pt, cutoff_pt);

        assert_eq!(trimmed, TrimmedPuckLocations { coords: vec![
            Coord { x: 2200.2, y: 141.2 },
            Coord { x: 2190.1, y: 138.6 },
            Coord { x: 1987.2, y: 132.7 },
            Coord { x: 1887.4, y: 120.2 },

            Coord { x: 2190., y: 143.2 },
            Coord { x: 2193., y: 141.2 },
            Coord { x: 2194., y: 142.2 },
            Coord { x: 2195., y: 145.2 },
            Coord { x: 2196., y: 146.2 },
        ]});
        assert_eq!(timesteps, vec![
            0, 1, 2, 3, 4, 5, 6, 7, 8
        ]);
    }

    // testing starting point at the very end
    #[test]
    fn subset_coords_end_starting_pt() {
        let rot_goal = RotatedPuckLocations { coords: vec![
            Some(Coord { x: 2190., y: 143.2 }),
            Some(Coord { x: 2200.2, y: 141.2 }),
            Some(Coord { x: 2190.1, y: 138.6 }),
            Some(Coord { x: 1987.2, y: 132.7 }),
            Some(Coord { x: 1887.4, y: 120.2 }),

            Some(Coord { x: 2190., y: 143.2 }),
            Some(Coord { x: 2193., y: 141.2 }),
            Some(Coord { x: 2194., y: 142.2 }),
            Some(Coord { x: 2195., y: 145.2 }),
            Some(Coord { x: 2196., y: 146.2 }),
        ]};
        let starting_pt = 9;
        let cutoff_pt = None;
        let (trimmed, timesteps) = subset_coords(&rot_goal, starting_pt, cutoff_pt);

        assert_eq!(trimmed, TrimmedPuckLocations { coords: vec![
            Coord { x: 2196., y: 146.2 },
        ]});
        assert_eq!(timesteps, vec![
            0
        ]);
    }

    // testing a starting point beyond the length of the rotated puck locations
    #[test]
    fn subset_coords_beyond_starting_pt() {
        let rot_goal = RotatedPuckLocations { coords: vec![
            Some(Coord { x: 2190., y: 143.2 }),
            Some(Coord { x: 2200.2, y: 141.2 }),
            Some(Coord { x: 2190.1, y: 138.6 }),
            Some(Coord { x: 1987.2, y: 132.7 }),
            Some(Coord { x: 1887.4, y: 120.2 }),

            Some(Coord { x: 2190., y: 143.2 }),
            Some(Coord { x: 2193., y: 141.2 }),
            Some(Coord { x: 2194., y: 142.2 }),
            Some(Coord { x: 2195., y: 145.2 }),
            Some(Coord { x: 2196., y: 146.2 }),
        ]};
        let starting_pt = 10;
        let cutoff_pt = None;
        let (trimmed, timesteps) = subset_coords(&rot_goal, starting_pt, cutoff_pt);

        assert_eq!(trimmed, TrimmedPuckLocations { coords: vec![]});
        let expected_timesteps: Vec<usize> = vec![];
        assert_eq!(timesteps, expected_timesteps);
    }

    // testing a starting point that is greater than the cutoff point
    #[test]
    fn subset_coords_starting_pt_gt_cutoff() {
        let rot_goal = RotatedPuckLocations { coords: vec![
            Some(Coord { x: 2190., y: 143.2 }),
            Some(Coord { x: 2200.2, y: 141.2 }),
            Some(Coord { x: 2190.1, y: 138.6 }),
            Some(Coord { x: 1987.2, y: 132.7 }),
            Some(Coord { x: 1887.4, y: 120.2 }),

            Some(Coord { x: 2190., y: 143.2 }),
            Some(Coord { x: 2193., y: 141.2 }),
            Some(Coord { x: 2194., y: 142.2 }),
            Some(Coord { x: 2195., y: 145.2 }),
            Some(Coord { x: 2196., y: 146.2 }),
        ]};
        let starting_pt = 7;
        let cutoff_pt = Some(2);
        let (trimmed, timesteps) = subset_coords(&rot_goal, starting_pt, cutoff_pt);

        assert_eq!(trimmed, TrimmedPuckLocations { coords: vec![]});
        let expected_timesteps: Vec<usize> = vec![];
        assert_eq!(timesteps, expected_timesteps);
    }

    // testing having a valid starting point with a cutoff point
    #[test]
    fn subset_coords_starting_pt_cutoff() {
        let rot_goal = RotatedPuckLocations { coords: vec![
            Some(Coord { x: 2190., y: 143.2 }),
            Some(Coord { x: 2200.2, y: 141.2 }),
            Some(Coord { x: 2190.1, y: 138.6 }),
            Some(Coord { x: 1987.2, y: 132.7 }),
            Some(Coord { x: 1887.4, y: 120.2 }),

            Some(Coord { x: 2190., y: 143.2 }),
            Some(Coord { x: 2193., y: 141.2 }),
            Some(Coord { x: 2194., y: 142.2 }),
            Some(Coord { x: 2195., y: 145.2 }),
            Some(Coord { x: 2196., y: 146.2 }),
        ]};
        let starting_pt = 3;
        let cutoff_pt = Some(8);
        let (trimmed, timesteps) = subset_coords(&rot_goal, starting_pt, cutoff_pt);

        assert_eq!(trimmed, TrimmedPuckLocations { coords: vec![
            Coord { x: 1987.2, y: 132.7 },
            Coord { x: 1887.4, y: 120.2 },

            Coord { x: 2190., y: 143.2 },
            Coord { x: 2193., y: 141.2 },
            Coord { x: 2194., y: 142.2 },
            Coord { x: 2195., y: 145.2 },
        ]});
        assert_eq!(timesteps, vec![
            0, 1, 2, 3, 4, 5
        ]);
    }

    // testing having starting pt be at timestep with missing location
    #[test]
    fn subset_coords_starting_pt_missing() {
        let rot_goal = RotatedPuckLocations { coords: vec![
            Some(Coord { x: 2190., y: 143.2 }),
            Some(Coord { x: 2200.2, y: 141.2 }),
            Some(Coord { x: 2190.1, y: 138.6 }),
            None,
            Some(Coord { x: 1887.4, y: 120.2 }),

            Some(Coord { x: 2190., y: 143.2 }),
            None,
            Some(Coord { x: 2194., y: 142.2 }),
            Some(Coord { x: 2195., y: 145.2 }),
            Some(Coord { x: 2196., y: 146.2 }),
        ]};
        let starting_pt = 3;
        let cutoff_pt = None;
        let (trimmed, timesteps) = subset_coords(&rot_goal, starting_pt, cutoff_pt);

        assert_eq!(trimmed, TrimmedPuckLocations { coords: vec![
            Coord { x: 1887.4, y: 120.2 },

            Coord { x: 2190., y: 143.2 },
            Coord { x: 2194., y: 142.2 },
            Coord { x: 2195., y: 145.2 },
            Coord { x: 2196., y: 146.2 }
        ]});
        assert_eq!(timesteps, vec![
            1, 2, 4, 5, 6
        ]);
    }

    // testing having starting pt, cutoff, and missing locations
    #[test]
    fn subset_coords_starting_pt_cutoff_missing() {
        let rot_goal = RotatedPuckLocations { coords: vec![
            Some(Coord { x: 2190., y: 143.2 }),
            Some(Coord { x: 2200.2, y: 141.2 }),
            Some(Coord { x: 2190.1, y: 138.6 }),
            None,
            Some(Coord { x: 1887.4, y: 120.2 }),

            Some(Coord { x: 2190., y: 143.2 }),
            None,
            Some(Coord { x: 2194., y: 142.2 }),
            Some(Coord { x: 2195., y: 145.2 }),
            Some(Coord { x: 2196., y: 146.2 }),
        ]};
        let starting_pt = 2;
        let cutoff_pt = Some(8);
        let (trimmed, timesteps) = subset_coords(&rot_goal, starting_pt, cutoff_pt);

        assert_eq!(trimmed, TrimmedPuckLocations { coords: vec![
            Coord { x: 2190.1, y: 138.6 },
            Coord { x: 1887.4, y: 120.2 },

            Coord { x: 2190., y: 143.2 },
            Coord { x: 2194., y: 142.2 },
            Coord { x: 2195., y: 145.2 },
        ]});
        assert_eq!(timesteps, vec![
            0, 2, 3, 5, 6
        ]);
    }

    // // --------------------------------------------------
    // // testing entire process of pre-processing a folder
    // // --------------------------------------------------

    // #[test]
    // fn read_process_folder() {
    //     let test_path = "test_data/trim_rotate_test_folder/";
    //     // let mut test_data = PbpGoalLocData {
    //     //     goal_loc_data: HashMap::new(),
    //     //     pbp_data: HashMap::new()
    //     // };
    //     read_folder(test_path, &mut test_data).unwrap();
    //     let preprocessed_data = preprocess_folder_data(&test_data);

    //     assert_eq!(preprocessed_data.len(), 3);
    //     let game_id = GameId(2024020011);
    //     assert_eq!(preprocessed_data[&(game_id, GoalId(155))], TrimmedPuckLocations { coords: vec![
    //         Coord { x: 101., y: 202.1 },
    //         Coord { x: 102., y: 203. },
    //         Coord { x: 104., y: 220. },
    //         Coord { x: 150., y: 650. },           
    //         Coord { x: 150., y: 651. },
            
    //         Coord { x: 2300., y: 501. },            
    //     ]});

    //     // check the rotated goal
    //     assert_eq!(preprocessed_data[&(game_id, GoalId(1414))], TrimmedPuckLocations { coords: vec![
    //         Coord { x: 1399., y: 115. },
    //         Coord { x: 1398., y: 116. },
    //         Coord { x: 1397., y: 117. },
    //         Coord { x: 1396., y: 118. },           
    //         Coord { x: 1396., y: 119. },
            
    //         Coord { x: 1395., y: 118. },
    //         Coord { x: 2250., y: 515. },                   
    //     ]});

    //     // check the other game in the folder
    //     let game_id = GameId(2024020012);
    //     assert_eq!(preprocessed_data[&(game_id, GoalId(277))], TrimmedPuckLocations { coords: vec![
    //         Coord { x: 101., y: 202.1 },
    //         Coord { x: 102., y: 203. },
    //         Coord { x: 104., y: 220. },
    //         Coord { x: 150., y: 650. },           
    //         Coord { x: 150., y: 651. },
            
    //         Coord { x: 202., y: 670. },
    //         Coord { x: 2301., y: 502. },
    //     ]});
    // }

    // --------------------------------------------------
    // take_last_n_instances() tests
    // --------------------------------------------------

    // goal with less than n should be returned as-is
    #[test]
    fn take_last_n_instances_lt_n() {
        let trimmed_goal = TrimmedPuckLocations { coords: vec![
            Coord { x: 1., y: 1. },
            Coord { x: 2., y: 2. },
            Coord { x: 3., y: 3. },
        ] };
        let n = 4;
        let rslt = take_last_n_instances(trimmed_goal, n);
        let expected_rslt = TrimmedPuckLocations { coords: vec![
            Coord { x: 1., y: 1. },
            Coord { x: 2., y: 2. },
            Coord { x: 3., y: 3. },
        ] };
        assert_eq!(rslt, expected_rslt);
    }

    // goal with exactly n should be returned as-is
    #[test]
    fn take_last_n_instances_eq_n() {
        let trimmed_goal = TrimmedPuckLocations { coords: vec![
            Coord { x: 1., y: 1. },
            Coord { x: 2., y: 2. },
            Coord { x: 3., y: 3. },
        ] };
        let n = 3;
        let rslt = take_last_n_instances(trimmed_goal, n);
        let expected_rslt = TrimmedPuckLocations { coords: vec![
            Coord { x: 1., y: 1. },
            Coord { x: 2., y: 2. },
            Coord { x: 3., y: 3. },
        ] };
        assert_eq!(rslt, expected_rslt);
    }

    // case where goal is longer than n
    #[test]
    fn take_last_n_instances_gt_n() {
        let trimmed_goal = TrimmedPuckLocations { coords: vec![
            Coord { x: 1., y: 1. },
            Coord { x: 2., y: 2. },
            Coord { x: 3., y: 3. },
            Coord { x: 4., y: 4. },
            Coord { x: 5., y: 5. },
            Coord { x: 6., y: 6. },
        ] };
        let n = 4;
        let rslt = take_last_n_instances(trimmed_goal, n);
        let expected_rslt = TrimmedPuckLocations { coords: vec![
            Coord { x: 3., y: 3. },
            Coord { x: 4., y: 4. },
            Coord { x: 5., y: 5. },
            Coord { x: 6., y: 6. },
        ] };
        assert_eq!(rslt, expected_rslt);
    }

    // --------------------------------------------------
    // read_puck_player_loc_data() tests
    // --------------------------------------------------
    
    // all instances have player and puck data
    #[test]
    fn read_puck_player_loc_no_missing() {
        let test_path = "test_data/test_player_puck_data/two_timesteps.json";
        let puck_player_data = read_puck_player_loc_data(&test_path).unwrap();

        // puck data
        assert_eq!(puck_player_data.puck_tracking.coords.len(), 2);
        let first_coord = puck_player_data.puck_tracking.coords[0].clone().unwrap();
        assert_eq!(first_coord.x, 1932.8914);
        assert_eq!(first_coord.y, 1008.7894);

        let second_coord = puck_player_data.puck_tracking.coords[1].clone().unwrap();
        assert_eq!(second_coord.x, 1930.8914);
        assert_eq!(second_coord.y, 1000.7894);

        // player data
        assert_eq!(puck_player_data.player_tracking.len(), 2); // 2 time steps
        let first_players_map = puck_player_data.player_tracking[0].as_ref().unwrap().clone();
        assert_eq!(first_players_map.len(), 5);
        assert_eq!(first_players_map[&PlayerInfo{ id: 8473507, team_id: 13 }], Coord{ x: 1241.7433, y: 26.8583 });
        assert_eq!(first_players_map[&PlayerInfo{ id: 8481519, team_id: 16 }], Coord{ x: 2232.1698, y: 545.4442 });
        assert_eq!(first_players_map[&PlayerInfo{ id: 8475179, team_id: 13 }], Coord{ x: 1347.0141, y: 77.8799 });
        assert_eq!(first_players_map[&PlayerInfo{ id: 8482113, team_id: 13 }], Coord{ x: 2044.3707, y: 904.3689 });
        assert_eq!(first_players_map[&PlayerInfo{ id: 8484783, team_id: 16 }], Coord{ x: 1832.6556, y: 390.1464 });

        let second_players_map = puck_player_data.player_tracking[1].as_ref().unwrap().clone();
        assert_eq!(second_players_map.len(), 5);
        assert_eq!(second_players_map[&PlayerInfo{ id: 8473507, team_id: 13 }], Coord{ x: 1242.7433, y: 27.8583 });
        assert_eq!(second_players_map[&PlayerInfo{ id: 8481519, team_id: 16 }], Coord{ x: 2233.1698, y: 546.4442 });
        assert_eq!(second_players_map[&PlayerInfo{ id: 8475179, team_id: 13 }], Coord{ x: 1348.0141, y: 80.8799 });
        assert_eq!(second_players_map[&PlayerInfo{ id: 8482113, team_id: 13 }], Coord{ x: 2050.3707, y: 940.3689 });
        assert_eq!(second_players_map[&PlayerInfo{ id: 8484783, team_id: 16 }], Coord{ x: 1835.6556, y: 393.1464 });
    }

    // empty json
    #[test]
    fn read_puck_player_loc_empty() {
        let test_path = "test_data/test_player_puck_data/empty.json";
        let puck_player_data = read_puck_player_loc_data(&test_path).unwrap();

        // puck data
        assert_eq!(puck_player_data.puck_tracking.coords.len(), 0);

        // player data
        assert_eq!(puck_player_data.player_tracking.len(), 0);
    }
    
    // one time step is missing puck data
    #[test]
    fn read_puck_player_loc_one_puck_missing() {
        let test_path = "test_data/test_player_puck_data/one_puck_missing.json";
        let puck_player_data = read_puck_player_loc_data(&test_path).unwrap();

        // puck data
        assert_eq!(puck_player_data.puck_tracking.coords.len(), 3);
        let first_coord = puck_player_data.puck_tracking.coords[0].clone().unwrap();
        assert_eq!(first_coord.x, 1932.8914);
        assert_eq!(first_coord.y, 1008.7894);

        assert!(puck_player_data.puck_tracking.coords[1].is_none());

        let first_coord = puck_player_data.puck_tracking.coords[2].clone().unwrap();
        assert_eq!(first_coord.x, 1930.8914);
        assert_eq!(first_coord.y, 1000.7894);

        // player data
        assert_eq!(puck_player_data.player_tracking.len(), 3); // 3 time steps
        let first_players_map = puck_player_data.player_tracking[0].as_ref().unwrap().clone();
        assert_eq!(first_players_map.len(), 5);
        assert_eq!(first_players_map[&PlayerInfo{ id: 8473507, team_id: 13 }], Coord{ x: 1241.7433, y: 26.8583 });
        assert_eq!(first_players_map[&PlayerInfo{ id: 8481519, team_id: 16 }], Coord{ x: 2232.1698, y: 545.4442 });
        assert_eq!(first_players_map[&PlayerInfo{ id: 8475179, team_id: 13 }], Coord{ x: 1347.0141, y: 77.8799 });
        assert_eq!(first_players_map[&PlayerInfo{ id: 8482113, team_id: 13 }], Coord{ x: 2044.3707, y: 904.3689 });
        assert_eq!(first_players_map[&PlayerInfo{ id: 8484783, team_id: 16 }], Coord{ x: 1832.6556, y: 390.1464 });

        let second_players_map = puck_player_data.player_tracking[1].as_ref().unwrap().clone();
        assert_eq!(second_players_map.len(), 5);
        assert_eq!(second_players_map[&PlayerInfo{ id: 8473507, team_id: 13 }], Coord{ x: 1242.7433, y: 27.8583 });
        assert_eq!(second_players_map[&PlayerInfo{ id: 8481519, team_id: 16 }], Coord{ x: 2233.1698, y: 546.4442 });
        assert_eq!(second_players_map[&PlayerInfo{ id: 8475179, team_id: 13 }], Coord{ x: 1348.0141, y: 80.8799 });
        assert_eq!(second_players_map[&PlayerInfo{ id: 8482113, team_id: 13 }], Coord{ x: 2050.3707, y: 940.3689 });
        assert_eq!(second_players_map[&PlayerInfo{ id: 8484783, team_id: 16 }], Coord{ x: 1835.6556, y: 393.1464 });
        
        let third_players_map = puck_player_data.player_tracking[2].as_ref().unwrap().clone();
        assert_eq!(third_players_map.len(), 3);
        assert_eq!(third_players_map[&PlayerInfo{ id: 8473507, team_id: 13 }], Coord{ x: 1243.7433, y: 28.8583 });
        assert_eq!(third_players_map[&PlayerInfo{ id: 8481519, team_id: 16 }], Coord{ x: 2234.1698, y: 547.4442 });
        assert_eq!(third_players_map[&PlayerInfo{ id: 8484783, team_id: 16 }], Coord{ x: 1836.6556, y: 394.1464 });
    }

    // two time steps are missing puck data
    #[test]
    fn read_puck_player_loc_two_puck_missing() {
        let test_path = "test_data/test_player_puck_data/two_puck_missing.json";
        let puck_player_data = read_puck_player_loc_data(&test_path).unwrap();

        // puck data
        assert_eq!(puck_player_data.puck_tracking.coords.len(), 4);
        let first_coord = puck_player_data.puck_tracking.coords[0].clone().unwrap();
        assert_eq!(first_coord.x, 1932.8914);
        assert_eq!(first_coord.y, 1008.7894);

        assert!(puck_player_data.puck_tracking.coords[1].is_none());

        let third_coord = puck_player_data.puck_tracking.coords[2].clone().unwrap();
        assert_eq!(third_coord.x, 1930.8914);
        assert_eq!(third_coord.y, 1000.7894);

        assert!(puck_player_data.puck_tracking.coords[3].is_none());

        // player data
        assert_eq!(puck_player_data.player_tracking.len(), 4); // 4 time steps
        let first_players_map = puck_player_data.player_tracking[0].as_ref().unwrap().clone();
        assert_eq!(first_players_map.len(), 5);
        assert_eq!(first_players_map[&PlayerInfo{ id: 8473507, team_id: 13 }], Coord{ x: 1241.7433, y: 26.8583 });
        assert_eq!(first_players_map[&PlayerInfo{ id: 8481519, team_id: 16 }], Coord{ x: 2232.1698, y: 545.4442 });
        assert_eq!(first_players_map[&PlayerInfo{ id: 8475179, team_id: 13 }], Coord{ x: 1347.0141, y: 77.8799 });
        assert_eq!(first_players_map[&PlayerInfo{ id: 8482113, team_id: 13 }], Coord{ x: 2044.3707, y: 904.3689 });
        assert_eq!(first_players_map[&PlayerInfo{ id: 8484783, team_id: 16 }], Coord{ x: 1832.6556, y: 390.1464 });

        let second_players_map = puck_player_data.player_tracking[1].as_ref().unwrap().clone();
        assert_eq!(second_players_map.len(), 5);
        assert_eq!(second_players_map[&PlayerInfo{ id: 8473507, team_id: 13 }], Coord{ x: 1242.7433, y: 27.8583 });
        assert_eq!(second_players_map[&PlayerInfo{ id: 8481519, team_id: 16 }], Coord{ x: 2233.1698, y: 546.4442 });
        assert_eq!(second_players_map[&PlayerInfo{ id: 8475179, team_id: 13 }], Coord{ x: 1348.0141, y: 80.8799 });
        assert_eq!(second_players_map[&PlayerInfo{ id: 8482113, team_id: 13 }], Coord{ x: 2050.3707, y: 940.3689 });
        assert_eq!(second_players_map[&PlayerInfo{ id: 8484783, team_id: 16 }], Coord{ x: 1835.6556, y: 393.1464 });
        
        let third_players_map = puck_player_data.player_tracking[2].as_ref().unwrap().clone();
        assert_eq!(third_players_map.len(), 3);
        assert_eq!(third_players_map[&PlayerInfo{ id: 8473507, team_id: 13 }], Coord{ x: 1243.7433, y: 28.8583 });
        assert_eq!(third_players_map[&PlayerInfo{ id: 8481519, team_id: 16 }], Coord{ x: 2234.1698, y: 547.4442 });
        assert_eq!(third_players_map[&PlayerInfo{ id: 8484783, team_id: 16 }], Coord{ x: 1836.6556, y: 394.1464 });

        let fourth_players_map = puck_player_data.player_tracking[3].as_ref().unwrap().clone();
        assert_eq!(fourth_players_map.len(), 2);
        assert_eq!(fourth_players_map[&PlayerInfo{ id: 8473507, team_id: 13 }], Coord{ x: 1244.7433, y: 29.8583 });
        assert_eq!(fourth_players_map[&PlayerInfo{ id: 8481519, team_id: 16 }], Coord{ x: 2235.1698, y: 548.4442 });
    }

    // all time steps are missing puck data
    #[test]
    fn read_puck_player_loc_all_puck_missing() {
        let test_path = "test_data/test_player_puck_data/all_puck_missing.json";
        let puck_player_data = read_puck_player_loc_data(&test_path).unwrap();

        // puck data
        assert_eq!(puck_player_data.puck_tracking.coords.len(), 4);
        assert!(puck_player_data.puck_tracking.coords[0].is_none());
        assert!(puck_player_data.puck_tracking.coords[1].is_none());
        assert!(puck_player_data.puck_tracking.coords[2].is_none());
        assert!(puck_player_data.puck_tracking.coords[3].is_none());

        // player data
        assert_eq!(puck_player_data.player_tracking.len(), 4); // 4 time steps
        let first_players_map = puck_player_data.player_tracking[0].as_ref().unwrap().clone();
        assert_eq!(first_players_map.len(), 5);
        assert_eq!(first_players_map[&PlayerInfo{ id: 8473507, team_id: 13 }], Coord{ x: 1241.7433, y: 26.8583 });
        assert_eq!(first_players_map[&PlayerInfo{ id: 8481519, team_id: 16 }], Coord{ x: 2232.1698, y: 545.4442 });
        assert_eq!(first_players_map[&PlayerInfo{ id: 8475179, team_id: 13 }], Coord{ x: 1347.0141, y: 77.8799 });
        assert_eq!(first_players_map[&PlayerInfo{ id: 8482113, team_id: 13 }], Coord{ x: 2044.3707, y: 904.3689 });
        assert_eq!(first_players_map[&PlayerInfo{ id: 8484783, team_id: 16 }], Coord{ x: 1832.6556, y: 390.1464 });

        let second_players_map = puck_player_data.player_tracking[1].as_ref().unwrap().clone();
        assert_eq!(second_players_map.len(), 5);
        assert_eq!(second_players_map[&PlayerInfo{ id: 8473507, team_id: 13 }], Coord{ x: 1242.7433, y: 27.8583 });
        assert_eq!(second_players_map[&PlayerInfo{ id: 8481519, team_id: 16 }], Coord{ x: 2233.1698, y: 546.4442 });
        assert_eq!(second_players_map[&PlayerInfo{ id: 8475179, team_id: 13 }], Coord{ x: 1348.0141, y: 80.8799 });
        assert_eq!(second_players_map[&PlayerInfo{ id: 8482113, team_id: 13 }], Coord{ x: 2050.3707, y: 940.3689 });
        assert_eq!(second_players_map[&PlayerInfo{ id: 8484783, team_id: 16 }], Coord{ x: 1835.6556, y: 393.1464 });
        
        let third_players_map = puck_player_data.player_tracking[2].as_ref().unwrap().clone();
        assert_eq!(third_players_map.len(), 3);
        assert_eq!(third_players_map[&PlayerInfo{ id: 8473507, team_id: 13 }], Coord{ x: 1243.7433, y: 28.8583 });
        assert_eq!(third_players_map[&PlayerInfo{ id: 8481519, team_id: 16 }], Coord{ x: 2234.1698, y: 547.4442 });
        assert_eq!(third_players_map[&PlayerInfo{ id: 8484783, team_id: 16 }], Coord{ x: 1836.6556, y: 394.1464 });

        let fourth_players_map = puck_player_data.player_tracking[3].as_ref().unwrap().clone();
        assert_eq!(fourth_players_map.len(), 2);
        assert_eq!(fourth_players_map[&PlayerInfo{ id: 8473507, team_id: 13 }], Coord{ x: 1244.7433, y: 29.8583 });
        assert_eq!(fourth_players_map[&PlayerInfo{ id: 8481519, team_id: 16 }], Coord{ x: 2235.1698, y: 548.4442 });
    }

    // one time step is missing all player info
    #[test]
    fn read_puck_player_loc_one_player_missing() {
        let test_path = "test_data/test_player_puck_data/one_player_missing.json";
        let puck_player_data = read_puck_player_loc_data(&test_path).unwrap();

        // puck data
        assert_eq!(puck_player_data.puck_tracking.coords.len(), 3);
        let first_coord = puck_player_data.puck_tracking.coords[0].clone().unwrap();
        assert_eq!(first_coord.x, 1932.8914);
        assert_eq!(first_coord.y, 1008.7894);

        let second_coord = puck_player_data.puck_tracking.coords[1].clone().unwrap();
        assert_eq!(second_coord.x, 1931.8914);
        assert_eq!(second_coord.y, 1004.7894);

        let third_coord = puck_player_data.puck_tracking.coords[2].clone().unwrap();
        assert_eq!(third_coord.x, 1930.8914);
        assert_eq!(third_coord.y, 1000.7894);

        // player data
        assert_eq!(puck_player_data.player_tracking.len(), 3); // 3 time steps
        let first_players_map = puck_player_data.player_tracking[0].as_ref().unwrap().clone();
        assert_eq!(first_players_map.len(), 5);
        assert_eq!(first_players_map[&PlayerInfo{ id: 8473507, team_id: 13 }], Coord{ x: 1241.7433, y: 26.8583 });
        assert_eq!(first_players_map[&PlayerInfo{ id: 8481519, team_id: 16 }], Coord{ x: 2232.1698, y: 545.4442 });
        assert_eq!(first_players_map[&PlayerInfo{ id: 8475179, team_id: 13 }], Coord{ x: 1347.0141, y: 77.8799 });
        assert_eq!(first_players_map[&PlayerInfo{ id: 8482113, team_id: 13 }], Coord{ x: 2044.3707, y: 904.3689 });
        assert_eq!(first_players_map[&PlayerInfo{ id: 8484783, team_id: 16 }], Coord{ x: 1832.6556, y: 390.1464 });

        let second_players_map = puck_player_data.player_tracking[1].as_ref().unwrap().clone();
        assert_eq!(second_players_map.len(), 5);
        assert_eq!(second_players_map[&PlayerInfo{ id: 8473507, team_id: 13 }], Coord{ x: 1242.7433, y: 27.8583 });
        assert_eq!(second_players_map[&PlayerInfo{ id: 8481519, team_id: 16 }], Coord{ x: 2233.1698, y: 546.4442 });
        assert_eq!(second_players_map[&PlayerInfo{ id: 8475179, team_id: 13 }], Coord{ x: 1348.0141, y: 80.8799 });
        assert_eq!(second_players_map[&PlayerInfo{ id: 8482113, team_id: 13 }], Coord{ x: 2050.3707, y: 940.3689 });
        assert_eq!(second_players_map[&PlayerInfo{ id: 8484783, team_id: 16 }], Coord{ x: 1835.6556, y: 393.1464 });
        
        assert!(puck_player_data.player_tracking[2].is_none());
    }

    // multiple time steps are missing all player info
    #[test]
    fn read_puck_player_loc_multi_player_missing() {
        let test_path = "test_data/test_player_puck_data/multi_player_missing.json";
        let puck_player_data = read_puck_player_loc_data(&test_path).unwrap();

        // puck data
        assert_eq!(puck_player_data.puck_tracking.coords.len(), 3);
        let first_coord = puck_player_data.puck_tracking.coords[0].clone().unwrap();
        assert_eq!(first_coord.x, 1932.8914);
        assert_eq!(first_coord.y, 1008.7894);

        let second_coord = puck_player_data.puck_tracking.coords[1].clone().unwrap();
        assert_eq!(second_coord.x, 1931.8914);
        assert_eq!(second_coord.y, 1004.7894);

        let third_coord = puck_player_data.puck_tracking.coords[2].clone().unwrap();
        assert_eq!(third_coord.x, 1930.8914);
        assert_eq!(third_coord.y, 1000.7894);

        // player data
        assert_eq!(puck_player_data.player_tracking.len(), 3); // 3 time steps
        assert!(puck_player_data.player_tracking[0].is_none());

        let second_players_map = puck_player_data.player_tracking[1].as_ref().unwrap().clone();
        assert_eq!(second_players_map.len(), 5);
        assert_eq!(second_players_map[&PlayerInfo{ id: 8473507, team_id: 13 }], Coord{ x: 1242.7433, y: 27.8583 });
        assert_eq!(second_players_map[&PlayerInfo{ id: 8481519, team_id: 16 }], Coord{ x: 2233.1698, y: 546.4442 });
        assert_eq!(second_players_map[&PlayerInfo{ id: 8475179, team_id: 13 }], Coord{ x: 1348.0141, y: 80.8799 });
        assert_eq!(second_players_map[&PlayerInfo{ id: 8482113, team_id: 13 }], Coord{ x: 2050.3707, y: 940.3689 });
        assert_eq!(second_players_map[&PlayerInfo{ id: 8484783, team_id: 16 }], Coord{ x: 1835.6556, y: 393.1464 });
        
        assert!(puck_player_data.player_tracking[2].is_none());
    }

    // all time steps are missing all player info
    #[test]
    fn read_puck_player_loc_all_player_missing() {
        let test_path = "test_data/test_player_puck_data/all_player_missing.json";
        let puck_player_data = read_puck_player_loc_data(&test_path).unwrap();

        // puck data
        assert_eq!(puck_player_data.puck_tracking.coords.len(), 3);
        let first_coord = puck_player_data.puck_tracking.coords[0].clone().unwrap();
        assert_eq!(first_coord.x, 1932.8914);
        assert_eq!(first_coord.y, 1008.7894);

        let second_coord = puck_player_data.puck_tracking.coords[1].clone().unwrap();
        assert_eq!(second_coord.x, 1931.8914);
        assert_eq!(second_coord.y, 1004.7894);

        let third_coord = puck_player_data.puck_tracking.coords[2].clone().unwrap();
        assert_eq!(third_coord.x, 1930.8914);
        assert_eq!(third_coord.y, 1000.7894);

        // player data
        assert_eq!(puck_player_data.player_tracking.len(), 3); // 3 time steps
        assert!(puck_player_data.player_tracking[0].is_none());
        assert!(puck_player_data.player_tracking[1].is_none());
        assert!(puck_player_data.player_tracking[2].is_none());
    }

    // missing both puck and player data for some timesteps
    #[test]
    fn read_puck_player_loc_puck_player_missing() {
        let test_path = "test_data/test_player_puck_data/some_puck_player_missing.json";
        let puck_player_data = read_puck_player_loc_data(&test_path).unwrap();

        // puck data
        assert_eq!(puck_player_data.puck_tracking.coords.len(), 4);
        let first_coord = puck_player_data.puck_tracking.coords[0].clone().unwrap();
        assert_eq!(first_coord.x, 1932.8914);
        assert_eq!(first_coord.y, 1008.7894);

        let second_coord = puck_player_data.puck_tracking.coords[1].clone().unwrap();
        assert_eq!(second_coord.x, 1931.8914);
        assert_eq!(second_coord.y, 1004.7894);

        assert!(puck_player_data.puck_tracking.coords[2].is_none());
        assert!(puck_player_data.puck_tracking.coords[3].is_none());

        // player data
        assert_eq!(puck_player_data.player_tracking.len(), 4); // 4 time steps
        let first_players_map = puck_player_data.player_tracking[0].as_ref().unwrap().clone();
        assert_eq!(first_players_map.len(), 5);
        assert_eq!(first_players_map[&PlayerInfo{ id: 8473507, team_id: 13 }], Coord{ x: 1241.7433, y: 26.8583 });
        assert_eq!(first_players_map[&PlayerInfo{ id: 8481519, team_id: 16 }], Coord{ x: 2232.1698, y: 545.4442 });
        assert_eq!(first_players_map[&PlayerInfo{ id: 8475179, team_id: 13 }], Coord{ x: 1347.0141, y: 77.8799 });
        assert_eq!(first_players_map[&PlayerInfo{ id: 8482113, team_id: 13 }], Coord{ x: 2044.3707, y: 904.3689 });
        assert_eq!(first_players_map[&PlayerInfo{ id: 8484783, team_id: 16 }], Coord{ x: 1832.6556, y: 390.1464 });

        assert!(puck_player_data.player_tracking[1].is_none());
        
        let third_players_map = puck_player_data.player_tracking[2].as_ref().unwrap().clone();
        assert_eq!(third_players_map.len(), 3);
        assert_eq!(third_players_map[&PlayerInfo{ id: 8473507, team_id: 13 }], Coord{ x: 1243.7433, y: 28.8583 });
        assert_eq!(third_players_map[&PlayerInfo{ id: 8481519, team_id: 16 }], Coord{ x: 2234.1698, y: 547.4442 });
        assert_eq!(third_players_map[&PlayerInfo{ id: 8484783, team_id: 16 }], Coord{ x: 1836.6556, y: 394.1464 });

        assert!(puck_player_data.player_tracking[3].is_none());
    }

    // -------------------------------------------
    // preprocess_player_puck() tests
    // -------------------------------------------

    // neither the puck nor player location data needs to be rotated nor trimmed
    #[test]
    fn preprocess_player_puck_passthr() {
        let puck_tracking = RawPuckLocationData{ coords: 
            vec![
                Some(Coord{ x: 10.1211, y: 50.6584 }),
                Some(Coord{ x: 11.1211, y: 52.6584 }),
                Some(Coord{ x: 12.1211, y: 54.6584 }),
            ]
        };
        let mut player_map_1 = HashMap::new(); // first timestep
        player_map_1.insert(PlayerInfo{ id: 8481519, team_id: 16 }, Coord{ x: 507.2463, y: 932.2318 });
        player_map_1.insert(PlayerInfo{ id: 8481518, team_id: 16 }, Coord{ x: 160.1699, y: 543.0000 });
        player_map_1.insert(PlayerInfo{ id: 8473419, team_id: 11 }, Coord{ x: 499.9477, y: 631.1628 });

        let mut player_map_2 = HashMap::new(); // second timestep
        player_map_2.insert(PlayerInfo{ id: 8481519, team_id: 16 }, Coord{ x: 506.2463, y: 931.2318 });
        player_map_2.insert(PlayerInfo{ id: 8481518, team_id: 16 }, Coord{ x: 161.1699, y: 544.5953 });
        player_map_2.insert(PlayerInfo{ id: 8473419, team_id: 11 }, Coord{ x: 500.9477, y: 632.1628 });

        let mut player_map_3 = HashMap::new(); // third timestep
        player_map_3.insert(PlayerInfo{ id: 8481519, team_id: 16 }, Coord{ x: 505.2463, y: 930.2318 });
        player_map_3.insert(PlayerInfo{ id: 8481520, team_id: 16 }, Coord{ x: 865.6758, y: 891.0598 });
        player_map_3.insert(PlayerInfo{ id: 8473419, team_id: 11 }, Coord{ x: 501.9477, y: 633.1628 });

        let player_tracking = vec![
            Some(player_map_1),
            Some(player_map_2),
            Some(player_map_3),
        ];
        let raw_puck_player_tracking = GoalRawPuckPlayerData { 
            puck_tracking, 
            player_tracking
        };
        let goal_details = GoalDetails { 
            event_id: 12, ppt_replay_url: None, scoring_team_id: 16, home_team_defending_side: String::from("Left") 
        };
        let home_team_id = 16;
        let result = preprocess_player_puck(&raw_puck_player_tracking, &goal_details, home_team_id);
        assert_eq!(result.puck_goal_ids, vec![12, 12, 12]);
        assert_eq!(result.puck_timesteps, vec![
            0, 1, 2
        ]);
        assert_eq!(result.puck_xs, vec![
            10.1211, 
            11.1211,
            12.1211
        ]);
        assert_eq!(result.puck_ys, vec![
            50.6584,
            52.6584,
            54.6584
        ]);
        // assert_eq!(result.puck_coords.coords[0], Coord{ x: 10.1211, y: 50.6584 });
        // assert_eq!(result.puck_coords.coords[1], Coord{ x: 11.1211, y: 52.6584 });
        // assert_eq!(result.puck_coords.coords[2], Coord{ x: 12.1211, y: 54.6584 });

        assert_eq!(result.player_timesteps.len(), 9);
        
        // to check output, use a hash set for each "row" of info
        let mut result_map = HashMap::new();
        for i in 0..result.player_goal_ids.len() {
            result_map.insert((
                result.player_goal_ids[i],
                result.player_timesteps[i],
                result.player_ids[i],
                result.team_ids[i].unwrap(),
            ), (result.player_xs[i], result.player_ys[i])
        );
        }
        
        assert_eq!(result_map.len(), 9);
        assert_eq!(result_map[&(12, 0, 8481519, 16)], (507.2463, 932.2318)); // first timestep
        assert_eq!(result_map[&(12, 0, 8481518, 16)], (160.1699, 543.0000)); 
        assert_eq!(result_map[&(12, 0, 8473419, 11)], (499.9477, 631.1628)); 

        assert_eq!(result_map[&(12, 1, 8481519, 16)], (506.2463, 931.2318)); // second timestep
        assert_eq!(result_map[&(12, 1, 8481518, 16)], (161.1699, 544.5953)); 
        assert_eq!(result_map[&(12, 1, 8473419, 11)], (500.9477, 632.1628)); 

        assert_eq!(result_map[&(12, 2, 8481519, 16)], (505.2463, 930.2318)); // third timestep
        assert_eq!(result_map[&(12, 2, 8481520, 16)], (865.6758, 891.0598)); // different player
        assert_eq!(result_map[&(12, 2, 8473419, 11)], (501.9477, 633.1628)); 
    }

    // rotated goal
    #[test]
    fn preprocess_player_puck_rotated() {
        let puck_tracking = RawPuckLocationData{ coords: 
            vec![
                Some(Coord{ x: 10.1211, y: 50.6584 }),
                Some(Coord{ x: 11.1211, y: 52.6584 }),
                Some(Coord{ x: 12.1211, y: 54.6584 }),
            ]
        };
        let mut player_map_1 = HashMap::new(); // first timestep
        player_map_1.insert(PlayerInfo{ id: 8481519, team_id: 16 }, Coord{ x: 507., y: 932. });
        player_map_1.insert(PlayerInfo{ id: 8481518, team_id: 16 }, Coord{ x: 160., y: 543. });
        player_map_1.insert(PlayerInfo{ id: 8473419, team_id: 11 }, Coord{ x: 499., y: 631. });

        let mut player_map_2 = HashMap::new(); // second timestep
        player_map_2.insert(PlayerInfo{ id: 8481519, team_id: 16 }, Coord{ x: 506., y: 931. });
        player_map_2.insert(PlayerInfo{ id: 8481518, team_id: 16 }, Coord{ x: 161., y: 544. });
        player_map_2.insert(PlayerInfo{ id: 8473419, team_id: 11 }, Coord{ x: 500., y: 632. });

        let mut player_map_3 = HashMap::new(); // third timestep
        player_map_3.insert(PlayerInfo{ id: 8481519, team_id: 16 }, Coord{ x: 505., y: 930. });
        player_map_3.insert(PlayerInfo{ id: 8481520, team_id: 16 }, Coord{ x: 865., y: 891. });
        player_map_3.insert(PlayerInfo{ id: 8473419, team_id: 11 }, Coord{ x: 501., y: 633. });

        let player_tracking = vec![
            Some(player_map_1),
            Some(player_map_2),
            Some(player_map_3),
        ];
        let raw_puck_player_tracking = GoalRawPuckPlayerData { 
            puck_tracking, 
            player_tracking
        };
        let goal_details = GoalDetails { 
            event_id: 12, ppt_replay_url: None, scoring_team_id: 16, home_team_defending_side: String::from("Right") 
        };
        let home_team_id = 16;
        let result = preprocess_player_puck(&raw_puck_player_tracking, &goal_details, home_team_id);
        assert_eq!(result.puck_goal_ids, vec![
            12, 12, 12
        ]);
        assert_eq!(result.puck_timesteps, vec![
            0, 1, 2
        ]);
        assert_eq!(result.puck_xs, vec![
            2389.8789, 
            2388.8789,
            2387.8789
        ]);
        assert_eq!(result.puck_ys, vec![
            964.3416, 
            962.3416,
            960.3416
        ]);

        assert_eq!(result.player_timesteps.len(), 9);
        
        // to check output, use a hash set for each "row" of info
        let mut result_map = HashMap::new();
        for i in 0..result.player_goal_ids.len() {
            result_map.insert((
                result.player_goal_ids[i],
                result.player_timesteps[i],
                result.player_ids[i],
                result.team_ids[i].unwrap(),
            ), (result.player_xs[i], result.player_ys[i])
        );
        }
        
        assert_eq!(result_map.len(), 9);
        assert_eq!(result_map[&(12, 0, 8481519, 16)], (1893.0, 83.0)); // first timestep
        assert_eq!(result_map[&(12, 0, 8481518, 16)], (2240.0, 472.0)); 
        assert_eq!(result_map[&(12, 0, 8473419, 11)], (1901.0, 384.0)); 

        assert_eq!(result_map[&(12, 1, 8481519, 16)], (1894.0, 84.0)); // second timestep
        assert_eq!(result_map[&(12, 1, 8481518, 16)], (2239.0, 471.0)); 
        assert_eq!(result_map[&(12, 1, 8473419, 11)], (1900.0, 383.0)); 

        assert_eq!(result_map[&(12, 2, 8481519, 16)], (1895.0, 85.0)); // third timestep
        assert_eq!(result_map[&(12, 2, 8481520, 16)], (1535.0, 124.0)); // different player
        assert_eq!(result_map[&(12, 2, 8473419, 11)], (1899.0, 382.0)); 
    }

    // goal trimmed at the end
    #[test]
    fn preprocess_player_puck_trimmed_end() {
        let puck_tracking = RawPuckLocationData{ coords: 
            vec![
                Some(Coord { x: 1010.5, y: 900.5 }),
                Some(Coord { x: 2019.9, y: 776.4 }),
                Some(Coord { x: 2250.4, y: 500.1444 }), // enters here and should be trimmed here
                Some(Coord { x: 2020.3, y: 775.6 }),
                Some(Coord { x: 11., y: 11. }),

                Some(Coord { x: 12., y: 11. }),
                Some(Coord { x: 13., y: 11. }),
            ]
        };
        let mut player_map_1 = HashMap::new(); // first timestep
        player_map_1.insert(PlayerInfo{ id: 8481519, team_id: 16 }, Coord{ x: 507.2463, y: 932.2318 });
        player_map_1.insert(PlayerInfo{ id: 8481518, team_id: 16 }, Coord{ x: 160.1699, y: 543.0000 });
        player_map_1.insert(PlayerInfo{ id: 8473419, team_id: 11 }, Coord{ x: 499.9477, y: 631.1628 });

        let mut player_map_2 = HashMap::new(); // second timestep
        player_map_2.insert(PlayerInfo{ id: 8481519, team_id: 16 }, Coord{ x: 506.2463, y: 931.2318 });
        player_map_2.insert(PlayerInfo{ id: 8481518, team_id: 16 }, Coord{ x: 161.1699, y: 544.5953 });
        player_map_2.insert(PlayerInfo{ id: 8473419, team_id: 11 }, Coord{ x: 500.9477, y: 632.1628 });

        let mut player_map_3 = HashMap::new(); // third timestep
        player_map_3.insert(PlayerInfo{ id: 8481519, team_id: 16 }, Coord{ x: 505.2463, y: 930.2318 });
        player_map_3.insert(PlayerInfo{ id: 8481520, team_id: 16 }, Coord{ x: 865.6758, y: 891.0598 });
        player_map_3.insert(PlayerInfo{ id: 8473419, team_id: 11 }, Coord{ x: 501.9477, y: 633.1628 });

        let mut player_map_4 = HashMap::new(); // fourth timestep, won't be included in result
        player_map_4.insert(PlayerInfo{ id: 8481519, team_id: 16 }, Coord{ x: 504.2463, y: 929.2318 });
        player_map_4.insert(PlayerInfo{ id: 8481520, team_id: 16 }, Coord{ x: 866.6758, y: 892.0598 });
        player_map_4.insert(PlayerInfo{ id: 8473419, team_id: 11 }, Coord{ x: 502.9477, y: 634.1628 });

        let player_tracking = vec![
            Some(player_map_1),
            Some(player_map_2),
            Some(player_map_3),
            Some(player_map_4),
        ];
        let raw_puck_player_tracking = GoalRawPuckPlayerData { 
            puck_tracking, 
            player_tracking
        };
        let goal_details = GoalDetails { 
            event_id: 12, ppt_replay_url: None, scoring_team_id: 16, home_team_defending_side: String::from("Left") 
        };
        let home_team_id = 16;
        let result = preprocess_player_puck(&raw_puck_player_tracking, &goal_details, home_team_id);
        assert_eq!(result.puck_goal_ids, vec![
            12, 12, 12
        ]);
        assert_eq!(result.puck_timesteps, vec![
            0, 1, 2
        ]);
        assert_eq!(result.puck_xs, vec![
            1010.5,
            2019.9,
            2250.4
        ]);
        assert_eq!(result.puck_ys, vec![
            900.5,
            776.4,
            500.1444
        ]);

        assert_eq!(result.player_timesteps.len(), 9);
        
        // to check output, use a hash set for each "row" of info
        let mut result_map = HashMap::new();
        for i in 0..result.player_goal_ids.len() {
            result_map.insert((
                result.player_goal_ids[i],
                result.player_timesteps[i],
                result.player_ids[i],
                result.team_ids[i].unwrap(),
            ), (result.player_xs[i], result.player_ys[i])
        );
        }
        
        assert_eq!(result_map.len(), 9);
        assert_eq!(result_map[&(12, 0, 8481519, 16)], (507.2463, 932.2318)); // first timestep
        assert_eq!(result_map[&(12, 0, 8481518, 16)], (160.1699, 543.0000)); 
        assert_eq!(result_map[&(12, 0, 8473419, 11)], (499.9477, 631.1628)); 

        assert_eq!(result_map[&(12, 1, 8481519, 16)], (506.2463, 931.2318)); // second timestep
        assert_eq!(result_map[&(12, 1, 8481518, 16)], (161.1699, 544.5953)); 
        assert_eq!(result_map[&(12, 1, 8473419, 11)], (500.9477, 632.1628)); 

        assert_eq!(result_map[&(12, 2, 8481519, 16)], (505.2463, 930.2318)); // third timestep
        assert_eq!(result_map[&(12, 2, 8481520, 16)], (865.6758, 891.0598)); // different player
        assert_eq!(result_map[&(12, 2, 8473419, 11)], (501.9477, 633.1628)); 
    }

    // goal trimmed at the start
    #[test]
    fn preprocess_player_puck_trimmed_start() {
        let puck_tracking = RawPuckLocationData{ coords: 
            vec![
                Some(Coord { x: 955., y: 210.2 }),
                Some(Coord { x: 955.4, y: 210.4 }),
                Some(Coord { x: 955.3, y: 210.3 }),
                Some(Coord { x: 955.2, y: 210.1 }),
                Some(Coord { x: 11., y: 11. }), // starts here

                Some(Coord { x: 12., y: 11. }),
                Some(Coord { x: 13., y: 11. }),
            ]
        };
        let mut player_map_1 = HashMap::new(); // first timestep, won't be included in result
        player_map_1.insert(PlayerInfo{ id: 8481519, team_id: 16 }, Coord{ x: 507.2463, y: 932.2318 });
        player_map_1.insert(PlayerInfo{ id: 8481518, team_id: 16 }, Coord{ x: 160.1699, y: 543.0000 });
        player_map_1.insert(PlayerInfo{ id: 8473419, team_id: 11 }, Coord{ x: 499.9477, y: 631.1628 });

        let mut player_map_2 = HashMap::new(); // second timestep, won't be included in result
        player_map_2.insert(PlayerInfo{ id: 8481519, team_id: 16 }, Coord{ x: 506.2463, y: 931.2318 });
        player_map_2.insert(PlayerInfo{ id: 8481518, team_id: 16 }, Coord{ x: 161.1699, y: 544.5953 });
        player_map_2.insert(PlayerInfo{ id: 8473419, team_id: 11 }, Coord{ x: 500.9477, y: 632.1628 });

        let mut player_map_3 = HashMap::new(); // third timestep, won't be included in result
        player_map_3.insert(PlayerInfo{ id: 8481519, team_id: 16 }, Coord{ x: 505.2463, y: 930.2318 });
        player_map_3.insert(PlayerInfo{ id: 8481520, team_id: 16 }, Coord{ x: 865.6758, y: 891.0598 });
        player_map_3.insert(PlayerInfo{ id: 8473419, team_id: 11 }, Coord{ x: 501.9477, y: 633.1628 });

        let mut player_map_4 = HashMap::new(); // fourth timestep, won't be included in result
        player_map_4.insert(PlayerInfo{ id: 8481519, team_id: 16 }, Coord{ x: 504.2463, y: 929.2318 });
        player_map_4.insert(PlayerInfo{ id: 8481520, team_id: 16 }, Coord{ x: 866.6758, y: 892.0598 });
        player_map_4.insert(PlayerInfo{ id: 8473419, team_id: 11 }, Coord{ x: 502.9477, y: 634.1628 });

        let mut player_map_5 = HashMap::new(); // fifth timestep
        player_map_5.insert(PlayerInfo{ id: 8481519, team_id: 16 }, Coord{ x: 503.2463, y: 928.2318 });
        player_map_5.insert(PlayerInfo{ id: 8481520, team_id: 16 }, Coord{ x: 867.6758, y: 893.0598 });
        player_map_5.insert(PlayerInfo{ id: 8473419, team_id: 11 }, Coord{ x: 503.9477, y: 635.1628 });

        let mut player_map_6 = HashMap::new(); // sixth timestep
        player_map_6.insert(PlayerInfo{ id: 8481519, team_id: 16 }, Coord{ x: 502.2463, y: 927.2318 });
        player_map_6.insert(PlayerInfo{ id: 8481520, team_id: 16 }, Coord{ x: 868.6758, y: 894.0598 });
        player_map_6.insert(PlayerInfo{ id: 8473419, team_id: 11 }, Coord{ x: 504.9477, y: 636.1628 });

        let mut player_map_7 = HashMap::new(); // seventh timestep
        player_map_7.insert(PlayerInfo{ id: 8481519, team_id: 16 }, Coord{ x: 501.2463, y: 926.2318 });
        player_map_7.insert(PlayerInfo{ id: 8481520, team_id: 16 }, Coord{ x: 869.6758, y: 895.0598 });
        player_map_7.insert(PlayerInfo{ id: 8473419, team_id: 11 }, Coord{ x: 505.9477, y: 637.1628 });

        let player_tracking = vec![
            Some(player_map_1),
            Some(player_map_2),
            Some(player_map_3),
            Some(player_map_4),
            Some(player_map_5),
            Some(player_map_6),
            Some(player_map_7),
        ];
        let raw_puck_player_tracking = GoalRawPuckPlayerData { 
            puck_tracking, 
            player_tracking
        };
        let goal_details = GoalDetails { 
            event_id: 12, ppt_replay_url: None, scoring_team_id: 16, home_team_defending_side: String::from("Left") 
        };
        let home_team_id = 16;
        let result = preprocess_player_puck(&raw_puck_player_tracking, &goal_details, home_team_id);
        assert_eq!(result.puck_goal_ids, vec![
            12, 12, 12
        ]);
        assert_eq!(result.puck_timesteps, vec![
            0, 1, 2
        ]);
        assert_eq!(result.puck_xs, vec![
            11., 
            12.,
            13.
        ]);
        assert_eq!(result.puck_ys, vec![
            11., 
            11.,
            11.
        ]);

        assert_eq!(result.player_timesteps.len(), 9);
        
        // to check output, use a hash set for each "row" of info
        let mut result_map = HashMap::new();
        for i in 0..result.player_goal_ids.len() {
            result_map.insert((
                result.player_goal_ids[i],
                result.player_timesteps[i],
                result.player_ids[i],
                result.team_ids[i].unwrap(),
            ), (result.player_xs[i], result.player_ys[i])
        );
        }
        
        assert_eq!(result_map.len(), 9);
        assert_eq!(result_map[&(12, 0, 8481519, 16)], (503.2463, 928.2318)); // fifth timestep
        assert_eq!(result_map[&(12, 0, 8481520, 16)], (867.6758, 893.0598)); 
        assert_eq!(result_map[&(12, 0, 8473419, 11)], (503.9477, 635.1628)); 

        assert_eq!(result_map[&(12, 1, 8481519, 16)], (502.2463, 927.2318)); // sixth timestep
        assert_eq!(result_map[&(12, 1, 8481520, 16)], (868.6758, 894.0598)); 
        assert_eq!(result_map[&(12, 1, 8473419, 11)], (504.9477, 636.1628)); 

        assert_eq!(result_map[&(12, 2, 8481519, 16)], (501.2463, 926.2318)); // seventh timestep
        assert_eq!(result_map[&(12, 2, 8481520, 16)], (869.6758, 895.0598)); 
        assert_eq!(result_map[&(12, 2, 8473419, 11)], (505.9477, 637.1628)); 
    }

    // trimmed at start and end
    #[test]
    fn preprocess_player_puck_trimmed_start_end() {
        let puck_tracking = RawPuckLocationData{ coords: 
            vec![
                Some(Coord { x: 2035.4955, y: 216.197 }),
                Some(Coord { x: 2035.8623, y: 218.187 }),
                Some(Coord { x: 2035.8621, y: 218.186 }),
                Some(Coord { x: 2019.4606, y: 247.9944 }), // starts here
                Some(Coord { x: 2028.5012, y: 269.7925 }),

                Some(Coord { x: 2021.4669, y: 270.5219 }),
                Some(Coord { x: 2173.5117, y: 418.5711 }),
                Some(Coord { x: 2172.869, y: 423.4482 }),
                Some(Coord { x: 2250.2745, y: 483.4997 }),  // enters goal here
                Some(Coord { x: 2250.2745, y: 484.4997 }), 
            ]
        };
        let mut player_map_1 = HashMap::new(); // first timestep, won't be included in result
        player_map_1.insert(PlayerInfo{ id: 8481519, team_id: 16 }, Coord{ x: 507.2463, y: 932.2318 });
        player_map_1.insert(PlayerInfo{ id: 8481518, team_id: 16 }, Coord{ x: 160.1699, y: 543.0000 });
        player_map_1.insert(PlayerInfo{ id: 8473419, team_id: 11 }, Coord{ x: 499.9477, y: 631.1628 });

        let mut player_map_2 = HashMap::new(); // second timestep, won't be included in result
        player_map_2.insert(PlayerInfo{ id: 8481519, team_id: 16 }, Coord{ x: 506.2463, y: 931.2318 });
        player_map_2.insert(PlayerInfo{ id: 8481518, team_id: 16 }, Coord{ x: 161.1699, y: 544.5953 });
        player_map_2.insert(PlayerInfo{ id: 8473419, team_id: 11 }, Coord{ x: 500.9477, y: 632.1628 });

        let mut player_map_3 = HashMap::new(); // third timestep, won't be included in result
        player_map_3.insert(PlayerInfo{ id: 8481519, team_id: 16 }, Coord{ x: 505.2463, y: 930.2318 });
        player_map_3.insert(PlayerInfo{ id: 8481520, team_id: 16 }, Coord{ x: 865.6758, y: 891.0598 });
        player_map_3.insert(PlayerInfo{ id: 8473419, team_id: 11 }, Coord{ x: 501.9477, y: 633.1628 });

        let mut player_map_4 = HashMap::new(); // fourth timestep
        player_map_4.insert(PlayerInfo{ id: 8481519, team_id: 16 }, Coord{ x: 504.2463, y: 929.2318 });
        player_map_4.insert(PlayerInfo{ id: 8481520, team_id: 16 }, Coord{ x: 866.6758, y: 892.0598 });
        player_map_4.insert(PlayerInfo{ id: 8473419, team_id: 11 }, Coord{ x: 502.9477, y: 634.1628 });

        let mut player_map_5 = HashMap::new(); // fifth timestep
        player_map_5.insert(PlayerInfo{ id: 8481519, team_id: 16 }, Coord{ x: 503.2463, y: 928.2318 });
        player_map_5.insert(PlayerInfo{ id: 8481520, team_id: 16 }, Coord{ x: 867.6758, y: 893.0598 });
        player_map_5.insert(PlayerInfo{ id: 8473419, team_id: 11 }, Coord{ x: 503.9477, y: 635.1628 });

        let mut player_map_6 = HashMap::new(); // sixth timestep
        player_map_6.insert(PlayerInfo{ id: 8481519, team_id: 16 }, Coord{ x: 502.2463, y: 927.2318 });
        player_map_6.insert(PlayerInfo{ id: 8481520, team_id: 16 }, Coord{ x: 868.6758, y: 894.0598 });
        player_map_6.insert(PlayerInfo{ id: 8473419, team_id: 11 }, Coord{ x: 504.9477, y: 636.1628 });

        let mut player_map_7 = HashMap::new(); // seventh timestep
        player_map_7.insert(PlayerInfo{ id: 8481519, team_id: 16 }, Coord{ x: 501.2463, y: 926.2318 });
        player_map_7.insert(PlayerInfo{ id: 8481520, team_id: 16 }, Coord{ x: 869.6758, y: 895.0598 });
        player_map_7.insert(PlayerInfo{ id: 8473419, team_id: 11 }, Coord{ x: 505.9477, y: 637.1628 });

        let mut player_map_8 = HashMap::new(); // eight timestep
        player_map_8.insert(PlayerInfo{ id: 8481519, team_id: 16 }, Coord{ x: 500.2463, y: 925.2318 });
        player_map_8.insert(PlayerInfo{ id: 8481520, team_id: 16 }, Coord{ x: 870.6758, y: 896.0598 });
        player_map_8.insert(PlayerInfo{ id: 8473419, team_id: 11 }, Coord{ x: 506.9477, y: 638.1628 });

        let mut player_map_9 = HashMap::new(); // ninth timestep
        player_map_9.insert(PlayerInfo{ id: 8481519, team_id: 16 }, Coord{ x: 499.2463, y: 924.2318 });
        player_map_9.insert(PlayerInfo{ id: 8481520, team_id: 16 }, Coord{ x: 871.6758, y: 897.0598 });
        player_map_9.insert(PlayerInfo{ id: 8473419, team_id: 11 }, Coord{ x: 507.9477, y: 639.1628 });

        let mut player_map_10 = HashMap::new(); // tenth timestep, won't be included in result
        player_map_10.insert(PlayerInfo{ id: 8481519, team_id: 16 }, Coord{ x: 498.2463, y: 923.2318 });
        player_map_10.insert(PlayerInfo{ id: 8481520, team_id: 16 }, Coord{ x: 872.6758, y: 898.0598 });
        player_map_10.insert(PlayerInfo{ id: 8473419, team_id: 11 }, Coord{ x: 508.9477, y: 640.1628 });

        let player_tracking = vec![
            Some(player_map_1),
            Some(player_map_2),
            Some(player_map_3),
            Some(player_map_4),
            Some(player_map_5),
            Some(player_map_6),
            Some(player_map_7),
            Some(player_map_8),
            Some(player_map_9),
            Some(player_map_10),
        ];
        let raw_puck_player_tracking = GoalRawPuckPlayerData { 
            puck_tracking, 
            player_tracking
        };
        let goal_details = GoalDetails { 
            event_id: 12, ppt_replay_url: None, scoring_team_id: 16, home_team_defending_side: String::from("Left") 
        };
        let home_team_id = 16;
        let result = preprocess_player_puck(&raw_puck_player_tracking, &goal_details, home_team_id);
        
        assert_eq!(result.puck_goal_ids, vec![
            12, 12, 12, 12, 12, 12
        ]);
        assert_eq!(result.puck_timesteps, vec![
            0, 1, 2, 3, 4, 5
        ]);
        assert_eq!(result.puck_xs, vec![
            2019.4606,
            2028.5012,
            2021.4669,
            2173.5117,
            2172.869,
            2250.2745
        ]);
        assert_eq!(result.puck_ys, vec![
            247.9944,
            269.7925,
            270.5219,
            418.5711,
            423.4482,
            483.4997
        ]);

        assert_eq!(result.player_timesteps.len(), 18);
        
        // to check output, use a hash set for each "row" of info
        let mut result_map = HashMap::new();
        for i in 0..result.player_goal_ids.len() {
            result_map.insert((
                result.player_goal_ids[i],
                result.player_timesteps[i],
                result.player_ids[i],
                result.team_ids[i].unwrap(),
            ), (result.player_xs[i], result.player_ys[i])
        );
        }
        
        assert_eq!(result_map.len(), 18);
        assert_eq!(result_map[&(12, 0, 8481519, 16)], (504.2463, 929.2318)); // fourth timestep
        assert_eq!(result_map[&(12, 0, 8481520, 16)], (866.6758, 892.0598)); 
        assert_eq!(result_map[&(12, 0, 8473419, 11)], (502.9477, 634.1628)); 

        assert_eq!(result_map[&(12, 1, 8481519, 16)], (503.2463, 928.2318)); // fifth timestep
        assert_eq!(result_map[&(12, 1, 8481520, 16)], (867.6758, 893.0598)); 
        assert_eq!(result_map[&(12, 1, 8473419, 11)], (503.9477, 635.1628)); 

        assert_eq!(result_map[&(12, 2, 8481519, 16)], (502.2463, 927.2318)); // sixth timestep
        assert_eq!(result_map[&(12, 2, 8481520, 16)], (868.6758, 894.0598)); 
        assert_eq!(result_map[&(12, 2, 8473419, 11)], (504.9477, 636.1628)); 

        assert_eq!(result_map[&(12, 3, 8481519, 16)], (501.2463, 926.2318)); // seventh timestep
        assert_eq!(result_map[&(12, 3, 8481520, 16)], (869.6758, 895.0598)); 
        assert_eq!(result_map[&(12, 3, 8473419, 11)], (505.9477, 637.1628));

        assert_eq!(result_map[&(12, 4, 8481519, 16)], (500.2463, 925.2318)); // eighth timestep
        assert_eq!(result_map[&(12, 4, 8481520, 16)], (870.6758, 896.0598)); 
        assert_eq!(result_map[&(12, 4, 8473419, 11)], (506.9477, 638.1628));

        assert_eq!(result_map[&(12, 5, 8481519, 16)], (499.2463, 924.2318)); // ninth timestep
        assert_eq!(result_map[&(12, 5, 8481520, 16)], (871.6758, 897.0598)); 
        assert_eq!(result_map[&(12, 5, 8473419, 11)], (507.9477, 639.1628));
    }

    // some timesteps are missing puck data
    #[test]
    fn preprocess_player_puck_nones() {
        let puck_tracking = RawPuckLocationData{ coords: 
            vec![
                None,
                None,
                Some(Coord { x: 11., y: 12. }),
                None,
                Some(Coord { x: 2021.4669, y: 270.5219 }),

                Some(Coord { x: 2173.5117, y: 418.5711 }),
                None,
                
            ]
        };
        let mut player_map_1 = HashMap::new(); // first timestep, won't be in result
        player_map_1.insert(PlayerInfo{ id: 8481519, team_id: 16 }, Coord{ x: 507.2463, y: 932.2318 });
        player_map_1.insert(PlayerInfo{ id: 8481518, team_id: 16 }, Coord{ x: 160.1699, y: 543.0000 });
        player_map_1.insert(PlayerInfo{ id: 8473419, team_id: 11 }, Coord{ x: 499.9477, y: 631.1628 });

        let mut player_map_2 = HashMap::new(); // second timestep, won't be in result
        player_map_2.insert(PlayerInfo{ id: 8481519, team_id: 16 }, Coord{ x: 506.2463, y: 931.2318 });
        player_map_2.insert(PlayerInfo{ id: 8481518, team_id: 16 }, Coord{ x: 161.1699, y: 544.5953 });
        player_map_2.insert(PlayerInfo{ id: 8473419, team_id: 11 }, Coord{ x: 500.9477, y: 632.1628 });

        let mut player_map_3 = HashMap::new(); // third timestep
        player_map_3.insert(PlayerInfo{ id: 8481519, team_id: 16 }, Coord{ x: 505.2463, y: 930.2318 });
        player_map_3.insert(PlayerInfo{ id: 8481520, team_id: 16 }, Coord{ x: 865.6758, y: 891.0598 });
        player_map_3.insert(PlayerInfo{ id: 8473419, team_id: 11 }, Coord{ x: 501.9477, y: 633.1628 });

        let mut player_map_4 = HashMap::new(); // fourth timestep
        player_map_4.insert(PlayerInfo{ id: 8481519, team_id: 16 }, Coord{ x: 504.2463, y: 929.2318 });
        player_map_4.insert(PlayerInfo{ id: 8481520, team_id: 16 }, Coord{ x: 866.6758, y: 892.0598 });
        player_map_4.insert(PlayerInfo{ id: 8473419, team_id: 11 }, Coord{ x: 502.9477, y: 634.1628 });

        let mut player_map_5 = HashMap::new(); // fifth timestep
        player_map_5.insert(PlayerInfo{ id: 8481519, team_id: 16 }, Coord{ x: 503.2463, y: 928.2318 });
        player_map_5.insert(PlayerInfo{ id: 8481520, team_id: 16 }, Coord{ x: 867.6758, y: 893.0598 });
        player_map_5.insert(PlayerInfo{ id: 8473419, team_id: 11 }, Coord{ x: 503.9477, y: 635.1628 });

        let mut player_map_6 = HashMap::new(); // sixth timestep
        player_map_6.insert(PlayerInfo{ id: 8481519, team_id: 16 }, Coord{ x: 502.2463, y: 927.2318 });
        player_map_6.insert(PlayerInfo{ id: 8481520, team_id: 16 }, Coord{ x: 868.6758, y: 894.0598 });
        player_map_6.insert(PlayerInfo{ id: 8473419, team_id: 11 }, Coord{ x: 504.9477, y: 636.1628 });

        let mut player_map_7 = HashMap::new(); // seventh timestep
        player_map_7.insert(PlayerInfo{ id: 8481519, team_id: 16 }, Coord{ x: 501.2463, y: 926.2318 });
        player_map_7.insert(PlayerInfo{ id: 8481520, team_id: 16 }, Coord{ x: 869.6758, y: 895.0598 });
        player_map_7.insert(PlayerInfo{ id: 8473419, team_id: 11 }, Coord{ x: 505.9477, y: 637.1628 });


        let player_tracking = vec![
            Some(player_map_1),
            Some(player_map_2),
            Some(player_map_3),
            Some(player_map_4),
            Some(player_map_5),
            Some(player_map_6),
            Some(player_map_7),
        ];
        let raw_puck_player_tracking = GoalRawPuckPlayerData { 
            puck_tracking, 
            player_tracking
        };
        let goal_details = GoalDetails { 
            event_id: 12, ppt_replay_url: None, scoring_team_id: 16, home_team_defending_side: String::from("Left") 
        };
        let home_team_id = 16;
        let result = preprocess_player_puck(&raw_puck_player_tracking, &goal_details, home_team_id);

        assert_eq!(result.puck_goal_ids, vec![
            12, 12, 12
        ]);
        assert_eq!(result.puck_timesteps, vec![
            0, 2, 3
        ]);
        assert_eq!(result.puck_xs, vec![
            11.,
            2021.4669,
            2173.5117
        ]);
        assert_eq!(result.puck_ys, vec![
            12.,
            270.5219,
            418.5711
        ]);

        assert_eq!(result.player_timesteps.len(), 15);
        
        // to check output, use a hash set for each "row" of info
        let mut result_map = HashMap::new();
        for i in 0..result.player_goal_ids.len() {
            result_map.insert((
                result.player_goal_ids[i],
                result.player_timesteps[i],
                result.player_ids[i],
                result.team_ids[i].unwrap(),
            ), (result.player_xs[i], result.player_ys[i])
        );
        }
        
        assert_eq!(result_map.len(), 15);
        assert_eq!(result_map[&(12, 0, 8481519, 16)], (505.2463, 930.2318)); // third timestep
        assert_eq!(result_map[&(12, 0, 8481520, 16)], (865.6758, 891.0598)); 
        assert_eq!(result_map[&(12, 0, 8473419, 11)], (501.9477, 633.1628)); 

        assert_eq!(result_map[&(12, 1, 8481519, 16)], (504.2463, 929.2318)); // fourth timestep
        assert_eq!(result_map[&(12, 1, 8481520, 16)], (866.6758, 892.0598)); 
        assert_eq!(result_map[&(12, 1, 8473419, 11)], (502.9477, 634.1628)); 

        assert_eq!(result_map[&(12, 2, 8481519, 16)], (503.2463, 928.2318)); // fifth timestep
        assert_eq!(result_map[&(12, 2, 8481520, 16)], (867.6758, 893.0598)); 
        assert_eq!(result_map[&(12, 2, 8473419, 11)], (503.9477, 635.1628)); 

        assert_eq!(result_map[&(12, 3, 8481519, 16)], (502.2463, 927.2318)); // sixth timestep
        assert_eq!(result_map[&(12, 3, 8481520, 16)], (868.6758, 894.0598)); 
        assert_eq!(result_map[&(12, 3, 8473419, 11)], (504.9477, 636.1628));

        assert_eq!(result_map[&(12, 4, 8481519, 16)], (501.2463, 926.2318)); // seventh timestep
        assert_eq!(result_map[&(12, 4, 8481520, 16)], (869.6758, 895.0598)); 
        assert_eq!(result_map[&(12, 4, 8473419, 11)], (505.9477, 637.1628));
    }

    // trimmed and has missing puck info
    #[test]
    fn preprocess_player_puck_trimmed_nones() {
        let puck_tracking = RawPuckLocationData{ coords: 
            vec![
                None,
                Some(Coord { x: 2035.8623, y: 218.187 }),
                None,
                Some(Coord { x: 2019.4606, y: 247.9944 }), // starts here
                Some(Coord { x: 2028.5012, y: 269.7925 }),

                Some(Coord { x: 2021.4669, y: 270.5219 }),
                None,
                Some(Coord { x: 2172.869, y: 423.4482 }),
                Some(Coord { x: 2250.2745, y: 483.4997 }),  // enters goal here
                Some(Coord { x: 2250.2745, y: 484.4997 }), 
            ]
        };
        let mut player_map_1 = HashMap::new(); // first timestep, won't be included in result
        player_map_1.insert(PlayerInfo{ id: 8481519, team_id: 16 }, Coord{ x: 507.2463, y: 932.2318 });
        player_map_1.insert(PlayerInfo{ id: 8481518, team_id: 16 }, Coord{ x: 160.1699, y: 543.0000 });
        player_map_1.insert(PlayerInfo{ id: 8473419, team_id: 11 }, Coord{ x: 499.9477, y: 631.1628 });

        let mut player_map_2 = HashMap::new(); // second timestep, won't be included in result
        player_map_2.insert(PlayerInfo{ id: 8481519, team_id: 16 }, Coord{ x: 506.2463, y: 931.2318 });
        player_map_2.insert(PlayerInfo{ id: 8481518, team_id: 16 }, Coord{ x: 161.1699, y: 544.5953 });
        player_map_2.insert(PlayerInfo{ id: 8473419, team_id: 11 }, Coord{ x: 500.9477, y: 632.1628 });

        let mut player_map_3 = HashMap::new(); // third timestep, won't be included in result
        player_map_3.insert(PlayerInfo{ id: 8481519, team_id: 16 }, Coord{ x: 505.2463, y: 930.2318 });
        player_map_3.insert(PlayerInfo{ id: 8481520, team_id: 16 }, Coord{ x: 865.6758, y: 891.0598 });
        player_map_3.insert(PlayerInfo{ id: 8473419, team_id: 11 }, Coord{ x: 501.9477, y: 633.1628 });

        let mut player_map_4 = HashMap::new(); // fourth timestep
        player_map_4.insert(PlayerInfo{ id: 8481519, team_id: 16 }, Coord{ x: 504.2463, y: 929.2318 });
        player_map_4.insert(PlayerInfo{ id: 8481520, team_id: 16 }, Coord{ x: 866.6758, y: 892.0598 });
        player_map_4.insert(PlayerInfo{ id: 8473419, team_id: 11 }, Coord{ x: 502.9477, y: 634.1628 });

        let mut player_map_5 = HashMap::new(); // fifth timestep
        player_map_5.insert(PlayerInfo{ id: 8481519, team_id: 16 }, Coord{ x: 503.2463, y: 928.2318 });
        player_map_5.insert(PlayerInfo{ id: 8481520, team_id: 16 }, Coord{ x: 867.6758, y: 893.0598 });
        player_map_5.insert(PlayerInfo{ id: 8473419, team_id: 11 }, Coord{ x: 503.9477, y: 635.1628 });

        let mut player_map_6 = HashMap::new(); // sixth timestep
        player_map_6.insert(PlayerInfo{ id: 8481519, team_id: 16 }, Coord{ x: 502.2463, y: 927.2318 });
        player_map_6.insert(PlayerInfo{ id: 8481520, team_id: 16 }, Coord{ x: 868.6758, y: 894.0598 });
        player_map_6.insert(PlayerInfo{ id: 8473419, team_id: 11 }, Coord{ x: 504.9477, y: 636.1628 });

        let mut player_map_7 = HashMap::new(); // seventh timestep
        player_map_7.insert(PlayerInfo{ id: 8481519, team_id: 16 }, Coord{ x: 501.2463, y: 926.2318 });
        player_map_7.insert(PlayerInfo{ id: 8481520, team_id: 16 }, Coord{ x: 869.6758, y: 895.0598 });
        player_map_7.insert(PlayerInfo{ id: 8473419, team_id: 11 }, Coord{ x: 505.9477, y: 637.1628 });

        let mut player_map_8 = HashMap::new(); // eight timestep
        player_map_8.insert(PlayerInfo{ id: 8481519, team_id: 16 }, Coord{ x: 500.2463, y: 925.2318 });
        player_map_8.insert(PlayerInfo{ id: 8481520, team_id: 16 }, Coord{ x: 870.6758, y: 896.0598 });
        player_map_8.insert(PlayerInfo{ id: 8473419, team_id: 11 }, Coord{ x: 506.9477, y: 638.1628 });

        let mut player_map_9 = HashMap::new(); // ninth timestep
        player_map_9.insert(PlayerInfo{ id: 8481519, team_id: 16 }, Coord{ x: 499.2463, y: 924.2318 });
        player_map_9.insert(PlayerInfo{ id: 8481520, team_id: 16 }, Coord{ x: 871.6758, y: 897.0598 });
        player_map_9.insert(PlayerInfo{ id: 8473419, team_id: 11 }, Coord{ x: 507.9477, y: 639.1628 });

        let mut player_map_10 = HashMap::new(); // tenth timestep, won't be included in result
        player_map_10.insert(PlayerInfo{ id: 8481519, team_id: 16 }, Coord{ x: 498.2463, y: 923.2318 });
        player_map_10.insert(PlayerInfo{ id: 8481520, team_id: 16 }, Coord{ x: 872.6758, y: 898.0598 });
        player_map_10.insert(PlayerInfo{ id: 8473419, team_id: 11 }, Coord{ x: 508.9477, y: 640.1628 });

        let player_tracking = vec![
            Some(player_map_1),
            Some(player_map_2),
            Some(player_map_3),
            Some(player_map_4),
            Some(player_map_5),
            Some(player_map_6),
            Some(player_map_7),
            Some(player_map_8),
            Some(player_map_9),
            Some(player_map_10),
        ];
        let raw_puck_player_tracking = GoalRawPuckPlayerData { 
            puck_tracking, 
            player_tracking
        };
        let goal_details = GoalDetails { 
            event_id: 12, ppt_replay_url: None, scoring_team_id: 16, home_team_defending_side: String::from("Left") 
        };
        let home_team_id = 16;
        let result = preprocess_player_puck(&raw_puck_player_tracking, &goal_details, home_team_id);
        
        assert_eq!(result.puck_goal_ids, vec![
            12, 12, 12, 12, 12
        ]);
        assert_eq!(result.puck_timesteps, vec![
            0, 1, 2, 4, 5
        ]);
        assert_eq!(result.puck_xs, vec![
            2019.4606,
            2028.5012,
            2021.4669,
            2172.869,
            2250.2745,
        ]);
        assert_eq!(result.puck_ys, vec![
            247.9944,
            269.7925,
            270.5219,
            423.4482,
            483.4997
        ]);

        assert_eq!(result.player_timesteps.len(), 18);
        
        // to check output, use a hash set for each "row" of info
        let mut result_map = HashMap::new();
        for i in 0..result.player_goal_ids.len() {
            result_map.insert((
                result.player_goal_ids[i],
                result.player_timesteps[i],
                result.player_ids[i],
                result.team_ids[i].unwrap(),
            ), (result.player_xs[i], result.player_ys[i])
        );
        }
        
        assert_eq!(result_map.len(), 18);
        assert_eq!(result_map[&(12, 0, 8481519, 16)], (504.2463, 929.2318)); // fourth timestep
        assert_eq!(result_map[&(12, 0, 8481520, 16)], (866.6758, 892.0598)); 
        assert_eq!(result_map[&(12, 0, 8473419, 11)], (502.9477, 634.1628)); 

        assert_eq!(result_map[&(12, 1, 8481519, 16)], (503.2463, 928.2318)); // fifth timestep
        assert_eq!(result_map[&(12, 1, 8481520, 16)], (867.6758, 893.0598)); 
        assert_eq!(result_map[&(12, 1, 8473419, 11)], (503.9477, 635.1628)); 

        assert_eq!(result_map[&(12, 2, 8481519, 16)], (502.2463, 927.2318)); // sixth timestep
        assert_eq!(result_map[&(12, 2, 8481520, 16)], (868.6758, 894.0598)); 
        assert_eq!(result_map[&(12, 2, 8473419, 11)], (504.9477, 636.1628)); 

        assert_eq!(result_map[&(12, 3, 8481519, 16)], (501.2463, 926.2318)); // seventh timestep
        assert_eq!(result_map[&(12, 3, 8481520, 16)], (869.6758, 895.0598)); 
        assert_eq!(result_map[&(12, 3, 8473419, 11)], (505.9477, 637.1628));

        assert_eq!(result_map[&(12, 4, 8481519, 16)], (500.2463, 925.2318)); // eighth timestep
        assert_eq!(result_map[&(12, 4, 8481520, 16)], (870.6758, 896.0598)); 
        assert_eq!(result_map[&(12, 4, 8473419, 11)], (506.9477, 638.1628));

        assert_eq!(result_map[&(12, 5, 8481519, 16)], (499.2463, 924.2318)); // ninth timestep
        assert_eq!(result_map[&(12, 5, 8481520, 16)], (871.6758, 897.0598)); 
        assert_eq!(result_map[&(12, 5, 8473419, 11)], (507.9477, 639.1628));
    }

    // ------------------------------------
    // need_to_rotate() tests
    // ------------------------------------

    // don't need to rotate
    #[test]
    fn need_to_rotate_false() {
        let goal_details = GoalDetails { 
            event_id: 12, ppt_replay_url: None, scoring_team_id: 16, home_team_defending_side: String::from("Left") 
        };
        let home_team_id = 16;
        let result = need_to_rotate(&goal_details, home_team_id);
        assert!(!result);
    }

    // need to rotate
    #[test]
    fn need_to_rotate_true() {
        let goal_details = GoalDetails { 
            event_id: 12, ppt_replay_url: None, scoring_team_id: 16, home_team_defending_side: String::from("Right") 
        };
        let home_team_id = 16;
        let result = need_to_rotate(&goal_details, home_team_id);
        assert!(result);
    }
}