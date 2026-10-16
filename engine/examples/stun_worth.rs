//! Prints what each stunning item is worth to the player's fight for each playstyle.
use engine::data::GameData;

fn main() {
    let d: GameData = serde_json::from_str(include_str!("../../src-tauri/data/snapshot.json")).unwrap();
    for item in d.items.iter().filter(|i| i.cost > 0) {
        let stun = engine::team::item_lockdown_seconds(item);
        if stun <= 0.0 {
            continue;
        }
        let row: Vec<String> = ["gun", "spirit", "melee"]
            .iter()
            .map(|s| {
                let (w, casts) = engine::master::stun_gain(item, stun, s, false);
                format!("{s} +{:.1}% ({casts:.0}x)", w * 100.0)
            })
            .collect();
        println!("{:<20} {stun:.2}s  {}", item.name, row.join("  "));
    }
}
