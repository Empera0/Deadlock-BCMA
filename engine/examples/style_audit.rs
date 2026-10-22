//! For every hero, the items that add the most to a build of the given damage style at two points of a match,
//! and how many of them are weapon, spirit or vitality items: a spirit build should be spirit items first.
//! `cargo run --release -p engine --example style_audit -- [style: spirit_fight|gun|hybrid|melee]`

use engine::{build::BuildInput, data::GameData, suggest};

/// The items a build has bought by then, and the souls: a few cheap items early, more later.
const POINTS: [(u32, u32); 2] = [(6_000, 1_600), (20_000, 3_200)];
const TOP: usize = 6;

fn main() {
    let data: GameData = serde_json::from_str(include_str!("../../src-tauri/data/snapshot.json")).unwrap();
    let style = std::env::args().nth(1).unwrap_or_else(|| "spirit_fight".into());
    let mut worst: Vec<(usize, String)> = vec![];
    for hero in &data.heroes {
        let mut line = vec![];
        let mut weapons = 0;
        for (souls, cap) in POINTS {
            let b = BuildInput { hero: hero.class_name.clone(), souls: Some(souls), ability_tiers: vec![1, 1, 1, 1], ..Default::default() };
            let candidates: Vec<String> = data.items.iter().filter(|i| i.cost > 0 && i.cost <= cap && i.tier <= 4).map(|i| i.class_name.clone()).collect();
            let mut gains = suggest::gains(&data, &b, &style, &candidates);
            gains.sort_by(|a, b| b.gain.total_cmp(&a.gain));
            let top: Vec<String> = gains
                .iter()
                .take(TOP)
                .filter_map(|g| data.item(&g.item))
                .map(|i| {
                    if i.category == "weapon" {
                        weapons += 1;
                    }
                    format!("{}{}", &i.category[..1].to_uppercase(), i.name)
                })
                .collect();
            line.push(top.join(", "));
        }
        println!("{:<14} weapon {weapons:>2}/{} | {}", hero.name, TOP * POINTS.len(), line.join(" || "));
        worst.push((weapons, hero.name.clone()));
    }
    worst.sort();
    worst.reverse();
    println!("\nMost weapon items in the top: {:?}", &worst[..8]);
}
