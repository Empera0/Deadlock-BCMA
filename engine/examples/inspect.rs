//! Prints a build's stat breakdown to the terminal:
//! cargo run -p engine --example inspect -- hero_inferno upgrade_headshot_booster upgrade_improved_spirit

use engine::{build::BuildInput, calc, data::GameData};

fn main() -> Result<(), String> {
    let data: GameData =
        serde_json::from_str(include_str!("../../src-tauri/data/snapshot.json")).map_err(|e| e.to_string())?;
    let mut args = std::env::args().skip(1);
    let hero = args.next().ok_or("usage: inspect <hero_class> [item_class ...]")?;
    let purchases: Vec<String> = args.collect();
    let b = BuildInput { hero, active: purchases.clone(), purchases, ..Default::default() };
    let s = calc::compute(&data, &b)?;
    println!("{} | level {} | {} boons | {} souls spent", s.hero, s.level, s.boons, s.spent);
    for e in &s.errors {
        println!("  ! {e}");
    }
    for st in &s.stats {
        if st.value == 0.0 && st.group != "target" {
            continue;
        }
        println!("[{:8}] {:36} {:>10.3} {:4}  = {}", st.group, st.label, st.value, st.unit, st.formula);
        for t in &st.terms {
            println!("{:14}- {}: {:.3}{}", "", t.label, t.value, t.unit);
        }
    }
    for a in &s.abilities {
        println!("\n{} (tier {}/{})", a.name, a.tier, a.max_tier);
        for p in &a.props {
            println!("    {:30} {:>9.2}{:3} = {}", p.label, p.value, p.unit, p.formula);
        }
    }
    for e in &s.item_effects {
        println!("\n{} effects", e.name);
        for p in &e.props {
            println!("    {:30} {:>9.2}{:3} = {} {}", p.label, p.value, p.unit, p.formula, p.note.clone().unwrap_or_default());
        }
    }
    Ok(())
}
