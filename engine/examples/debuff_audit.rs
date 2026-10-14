//! Lists every item and ability whose data mentions lowering a resist or raising damage taken,
//! and whether the engine turns it into a target debuff:
//! cargo run -p engine --example debuff_audit -- src-tauri/data/snapshot.json

use engine::{build::BuildInput, combo, data::GameData};
use std::{env, fs};

fn mentions(key: &str, modifier: Option<&str>) -> bool {
    let key = key.to_lowercase();
    let named = (key.contains("resist") || key.contains("armor")) && (key.contains("reduc") || key.contains("shred") || key.contains("stolen"));
    let amp = key.contains("damageamp") || key.contains("vulnerab") || key.contains("damagetaken") || key.contains("incomingdamage") || key.contains("magicincrease");
    named || amp || modifier.is_some_and(|m| m.contains("RESIST_REDUCTION") || m.contains("DAMAGE_TAKEN"))
}

fn main() -> Result<(), String> {
    let path = env::args().nth(1).ok_or("usage: debuff_audit <snapshot.json>")?;
    let data: GameData = serde_json::from_str(&fs::read_to_string(&path).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
    let first = &data.heroes[0];

    println!("== Items ==");
    for item in &data.items {
        let props: Vec<String> =
            item.props.iter().filter(|p| mentions(&p.key, p.modifier.as_deref())).map(|p| format!("{}={}", p.key, p.value)).collect();
        if props.is_empty() {
            continue;
        }
        let b = BuildInput { hero: first.class_name.clone(), purchases: vec![item.class_name.clone()], souls: Some(60_000), ..Default::default() };
        let found: Vec<String> = combo::target_debuffs(&data, &b)?
            .into_iter()
            .filter(|d| d.source.starts_with(&item.name))
            .map(|d| format!("{}{}", d.effect.text(), if d.team { "" } else { " (owner only)" }))
            .collect();
        println!("{} {:<24} {:<60} <- {}", if found.is_empty() { "MISSING" } else { "ok     " }, item.name, found.join(" / "), props.join(", "));
    }

    println!("== Abilities (all upgrades taken) ==");
    for hero in &data.heroes {
        let b = BuildInput { hero: hero.class_name.clone(), souls: Some(60_000), ability_tiers: vec![3; hero.abilities.len()], ..Default::default() };
        let debuffs = combo::target_debuffs(&data, &b)?;
        for ability in &hero.abilities {
            let mut props: Vec<String> =
                ability.props.iter().filter(|p| mentions(&p.key, p.modifier.as_deref())).map(|p| format!("{}={}", p.key, p.value)).collect();
            props.extend(ability.upgrades.iter().flatten().filter(|u| mentions(&u.name, None)).map(|u| format!("{}+{}", u.name, u.bonus)));
            if props.is_empty() {
                continue;
            }
            let found: Vec<String> = debuffs
                .iter()
                .filter(|d| d.source == ability.name)
                .map(|d| format!("{}{}", d.effect.text(), if d.team { "" } else { " (owner only)" }))
                .collect();
            println!(
                "{} {:<11} {:<24} {:<52} <- {}",
                if found.is_empty() { "MISSING" } else { "ok     " },
                hero.name,
                ability.name,
                found.join(" / "),
                props.join(", ")
            );
        }
    }
    Ok(())
}
