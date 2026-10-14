//! Lists every stat that still lands in the catch-all "other" group, per item:
//! cargo run -p engine --example stat_audit -- src-tauri/data/snapshot.json

use engine::{build::BuildInput, calc, data::GameData};
use std::{collections::BTreeMap, env, fs};

fn main() -> Result<(), String> {
    let path = env::args().nth(1).ok_or("usage: stat_audit <snapshot.json>")?;
    let data: GameData = serde_json::from_str(&fs::read_to_string(&path).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
    let hero = data.heroes.first().ok_or("no heroes")?;
    let mut seen: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for item in &data.items {
        // With the conditional switch on, so passive and active values are included too.
        let b = BuildInput {
            hero: hero.class_name.clone(),
            purchases: vec![item.class_name.clone()],
            active: vec![item.class_name.clone()],
            ..Default::default()
        };
        let sheet = calc::compute(&data, &b)?;
        for s in sheet.stats.iter().filter(|s| s.group == "other") {
            seen.entry(format!("{} [{}]", s.label, s.key)).or_default().push(format!("{} {}{}", item.name, s.value, s.unit));
        }
    }
    for (stat, items) in &seen {
        println!("{stat}: {}", items.join(", "));
    }
    println!("{} stats left in the other group", seen.len());
    Ok(())
}
