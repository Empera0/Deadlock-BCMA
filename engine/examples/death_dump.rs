//! Prints why a player died in a match, death by death, as the app's death study shows it:
//! `cargo run -p engine --example death_dump -- <match metadata json> <player slot>`

use engine::{data::GameData, deaths};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let data: GameData = serde_json::from_str(include_str!("../../src-tauri/data/snapshot.json")).unwrap();
    let json = std::fs::read_to_string(&args[1]).unwrap();
    let report = deaths::analyze(&data, &json, args[2].parse().unwrap()).unwrap();
    println!("{}", serde_json::to_string_pretty(&report).unwrap());
}
