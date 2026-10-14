//! Prints how the combo simulation reads every ability:
//! cargo run -p engine --example kit_audit -- [first hero index] [last hero index] [ability tier]

use engine::{build::BuildInput, calc, data::GameData, kit};

fn main() -> Result<(), String> {
    let data: GameData =
        serde_json::from_str(include_str!("../../src-tauri/data/snapshot.json")).map_err(|e| e.to_string())?;
    let args: Vec<usize> = std::env::args().skip(1).filter_map(|a| a.parse().ok()).collect();
    let (from, to, tier) = (
        args.first().copied().unwrap_or(0),
        args.get(1).copied().unwrap_or(usize::MAX),
        args.get(2).copied().unwrap_or(0) as u8,
    );
    for (i, hero) in data.heroes.iter().enumerate().filter(|(i, _)| *i >= from && *i <= to) {
        let b = BuildInput { hero: hero.class_name.clone(), ability_tiers: vec![tier; 4], ..Default::default() };
        let sheet = calc::compute(&data, &b)?;
        let ctx = kit::Context {
            per_shot: sheet.stat("bullet_damage").unwrap_or(0.0) * hero.weapon.pellets,
            shots_per_second: sheet.stat("shots_per_second").unwrap_or(0.0),
            abilities: &sheet.abilities,
            flags: &[],
        };
        println!("\n## {i} {}", hero.name);
        for a in &sheet.abilities {
            let plan = kit::plan(a, &ctx, None, None);
            let tag = if plan.castable { "" } else { " [passive]" };
            println!("  {}{tag}", a.name);
            for line in plan.describe() {
                println!("      {line}");
            }
        }
    }
    Ok(())
}
