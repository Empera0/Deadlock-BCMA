//! Studies one player of a match from its replay, as the app shows it:
//! `cargo run -p engine --example replay_check -- <match metadata json> <slot> <hits .ndjson.zst> <notes .ndjson> <kills .ndjson>`

use engine::{data::GameData, replay};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let data: GameData = serde_json::from_str(include_str!("../../src-tauri/data/snapshot.json")).unwrap();
    let read = |p: &str| std::fs::read_to_string(p).unwrap();
    let hits = replay::decompress(&std::fs::read(&args[3]).unwrap()).unwrap();
    let s = replay::study(&data, &read(&args[1]), args[2].parse().unwrap(), &hits, &read(&args[4]), &read(&args[5])).unwrap();
    for d in &s.deaths {
        println!("\n{}:{:02} killed by {} — {}", d.time / 60, d.time % 60, d.killer, d.summary);
        for m in &d.hits {
            println!("   {:>5.1}..{:>5.1}s {:<14} {:<22} {:>5.0} ({} hits) -> {:.0}", m.at, m.until, m.hero, m.source, m.damage, m.hits, m.health_after);
        }
        for st in &d.statuses {
            println!("   {:>5.1}s {} {} {:?}", st.at, st.hero, st.source, st.statuses);
        }
    }
    println!();
    for a in &s.abilities {
        println!("{:<14} {} casts, {:.2}/cast (usual {:?}), {:.0} dmg/cast, held {:.2} — {}", a.ability, a.casts, a.enemies_per_cast, a.usual.map(|u| (u * 100.0).round() / 100.0), a.damage_per_cast, a.held_per_cast, a.verdict);
    }
}
