//! Prints, for every ability, what the engine reads it does to enemies (for the team checks) and how much of
//! the time, next to the ability's own numbers and upgrades, to check against the wiki:
//! cargo run -p engine --example effects_audit -- [first hero index] [last hero index]

use engine::data::GameData;
use engine::team;

fn main() {
    let data: GameData = serde_json::from_str(include_str!("../../src-tauri/data/snapshot.json")).unwrap();
    let args: Vec<usize> = std::env::args().skip(1).filter_map(|a| a.parse().ok()).collect();
    let (from, to) = (args.first().copied().unwrap_or(0), args.get(1).copied().unwrap_or(usize::MAX));
    for (i, hero) in data.heroes.iter().enumerate().filter(|(i, _)| *i >= from && *i <= to) {
        println!("\n## {i} {} ({})", hero.name, hero.class_name);
        for a in &hero.abilities {
            let effects: Vec<String> = team::ability_effects_timed(a).into_iter().map(|(e, up)| format!("{} {:.0}% of the time", e.verb(), up * 100.0)).collect();
            println!("  {} [{}] effects: {}", a.name, a.class_name, if effects.is_empty() { "-".into() } else { effects.join(", ") });
            let props: Vec<String> = a.props.iter().filter(|p| p.value != 0.0).map(|p| format!("{}={}", p.key, p.value)).collect();
            println!("      props: {}", props.join("; "));
            for (t, up) in a.upgrades.iter().enumerate() {
                println!("      T{}: {}", t + 1, up.iter().map(|u| format!("{}{:+}", u.name, u.bonus)).collect::<Vec<_>>().join("; "));
            }
        }
    }
}
