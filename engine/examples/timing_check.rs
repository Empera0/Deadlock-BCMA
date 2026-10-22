//! Compares a player's purchases in a match with when top players of the hero buy each item:
//! `cargo run -p engine --example timing_check -- <match metadata json> <slot> <item stats by minute json>`

use engine::{data::GameData, matches, popular::ItemStat, timing};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let data: GameData = serde_json::from_str(include_str!("../../src-tauri/data/snapshot.json")).unwrap();
    let report = matches::parse(&data, &std::fs::read_to_string(&args[1]).unwrap()).unwrap();
    let slot: u32 = args[2].parse().unwrap();
    let stats: Vec<ItemStat> = serde_json::from_str(&std::fs::read_to_string(&args[3]).unwrap()).unwrap();
    let p = report.players.iter().find(|p| p.slot == slot).unwrap();
    let worth: Vec<(u32, u32)> = p.timeline.iter().map(|s| (s.time, s.net_worth)).collect();
    let t = timing::compare(&data, &stats, &p.purchases, report.duration_s, &worth);
    for i in &t.items {
        let win = i.win_pp_then.map(|w| format!("{w:+.1} pts")).unwrap_or_default();
        let shift = if i.shift_min > 0.0 { format!(" (shifted {:.1} min by {})", i.shift_min, i.shifted_by.join(", ")) } else { String::new() };
        println!(
            "{:<20} {:>5}  top {:>5} ({}-{}) {:<9} {:+.1} min {win}{shift}",
            i.name,
            format!("{}:{:02}", i.bought_at / 60, i.bought_at % 60),
            format!("{}:{:02}", i.median_s / 60, i.median_s % 60),
            i.early_s / 60,
            i.late_s / 60,
            i.tone,
            i.minutes_off
        );
    }
    for m in &t.missing {
        println!("missing {:<20} by {}:{:02} ({:.0}%)", m.name, m.median_s / 60, m.median_s % 60, m.share * 100.0);
    }
    println!("{}", t.summary);
}
