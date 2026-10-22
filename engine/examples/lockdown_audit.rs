//! Prints each ability's lockdown per cast and the enemies it hits in a team fight, fully upgraded:
//! `cargo run -p engine --example lockdown_audit`

use engine::{area, build::BuildInput, calc, data::GameData, team};

fn main() {
    let data: GameData = serde_json::from_str(include_str!("../../src-tauri/data/snapshot.json")).unwrap();
    for hero in &data.heroes {
        let b = BuildInput { hero: hero.class_name.clone(), ability_tiers: vec![3; 4], ..Default::default() };
        let sheet = calc::compute(&data, &b).unwrap();
        let line: Vec<String> = sheet
            .abilities
            .iter()
            .filter(|a| team::sheet_lockdown_seconds(a) > 0.0)
            .map(|a| format!("{} {:.2}s × {:.1}", a.name, team::sheet_lockdown_seconds(a), area::targets(a)))
            .collect();
        if !line.is_empty() {
            println!("{:<14} {}", hero.name, line.join(" | "));
        }
    }
}
