//! Prints a match's analysis as JSON, the way the app shows it:
//! `cargo run -p engine --example match_dump -- <match metadata json> <output json>`

use engine::{data::GameData, matches};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let data: GameData = serde_json::from_str(include_str!("../../src-tauri/data/snapshot.json")).unwrap();
    let json = std::fs::read_to_string(&args[1]).unwrap();
    let report = matches::parse(&data, &json).unwrap();
    std::fs::write(&args[2], serde_json::to_string_pretty(&report).unwrap()).unwrap();
}
