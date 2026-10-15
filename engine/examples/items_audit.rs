//! Prints how the engine reads every shop item: its own description and numbers, the counter jobs it does, its
//! effects for the team checks, and how easily it lands. For checking items one by one:
//! cargo run -p engine --example items_audit -- [first index] [last index]

use engine::data::GameData;
use engine::{popular, team};

fn main() {
    let data: GameData = serde_json::from_str(include_str!("../../src-tauri/data/snapshot.json")).unwrap();
    let args: Vec<usize> = std::env::args().skip(1).filter_map(|a| a.parse().ok()).collect();
    let (from, to) = (args.first().copied().unwrap_or(0), args.get(1).copied().unwrap_or(usize::MAX));
    let mut shop: Vec<&engine::data::Item> = data.items.iter().filter(|i| i.cost > 0 && i.tier <= 4).collect();
    shop.sort_by_key(|i| (i.category.clone(), i.cost, i.name.clone()));
    for (n, item) in shop.iter().enumerate().filter(|(n, _)| *n >= from && *n <= to) {
        let effects: Vec<&str> = team::item_effects(item).into_iter().map(|e| e.verb()).collect();
        println!("\n#{n} {} [{} {} {}]{}", item.name, item.category, item.cost, item.class_name, if item.active { " ACTIVE" } else { "" });
        println!("   text: {}", item.description.replace('\n', " "));
        let props: Vec<String> = item.props.iter().filter(|p| p.value != 0.0).map(|p| format!("{}={}{}", p.key, p.value, if p.section.is_empty() { String::new() } else { format!("[{}]", p.section) })).collect();
        println!("   props: {}", props.join("; "));
        println!("   team: {}", if effects.is_empty() { "-".into() } else { effects.join(", ") });
        println!("   {}", popular::item_reading(item));
    }
}
