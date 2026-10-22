//! Prints how many enemies each ability hits in a team fight, at the given upgrade tier, without items and with
//! Greater Expansion: `cargo run -p engine --example area_audit -- [ability tier]`

use engine::{area, build::BuildInput, calc, data::GameData};

fn main() {
    let data: GameData = serde_json::from_str(include_str!("../../src-tauri/data/snapshot.json")).unwrap();
    let tier: u8 = std::env::args().nth(1).and_then(|t| t.parse().ok()).unwrap_or(1);
    for hero in &data.heroes {
        let bare = BuildInput { hero: hero.class_name.clone(), ability_tiers: vec![tier; 4], ..Default::default() };
        let mut wide = bare.clone();
        wide.purchases.push("upgrade_tech_range".into());
        let (a, b) = (calc::compute(&data, &bare).unwrap(), calc::compute(&data, &wide).unwrap());
        let line: Vec<String> = a
            .abilities
            .iter()
            .zip(&b.abilities)
            .filter(|(x, _)| area::targets(x) > 1.0)
            .map(|(x, y)| format!("{} {:.2}→{:.2}", x.name, area::targets(x), area::targets(y)))
            .collect();
        println!("{:<14} {}", hero.name, line.join(" | "));
    }
}
