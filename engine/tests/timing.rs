//! Item timing against top players.

use engine::{data::GameData, matches::Purchase, popular::ItemStat, timing};

fn data() -> GameData {
    serde_json::from_str(include_str!("../../src-tauri/data/snapshot.json")).unwrap()
}

fn row(item_id: u64, minute: u64, matches: u64, wins: u64) -> ItemStat {
    ItemStat { item_id, wins, losses: matches - wins, matches, avg_buy_time_s: 0.0, sold: 0, bucket: Some(minute) }
}

fn bought(data: &GameData, name: &str, time: u32, sold_at: Option<u32>) -> Purchase {
    let i = data.items.iter().find(|i| i.name == name).unwrap();
    Purchase { time, item: i.class_name.clone(), name: i.name.clone(), icon: String::new(), category: i.category.clone(), cost: i.cost, sold_at }
}

#[test]
fn purchases_are_held_against_top_players_minutes() {
    let data = data();
    let id = |name: &str| data.items.iter().find(|i| i.name == name).unwrap().id;
    // Spirit Burn: top players buy it from minute 20 to 30, most at 24 to 27; Extra Spirit at 3 to 5.
    let mut stats: Vec<ItemStat> = (20..=30).map(|m| row(id("Spirit Burn"), m, if (24..=27).contains(&m) { 100 } else { 20 }, 10)).collect();
    stats.extend((3..=5).map(|m| row(id("Extra Spirit"), m, 300, 150)));
    let purchases = vec![
        bought(&data, "Spirit Burn", 40 * 60, None),
        bought(&data, "Superior Cooldown", 30 * 60, Some(30 * 60 + 2)),
    ];
    let worth: Vec<(u32, u32)> = (0..=15).map(|i| (i * 180, i * 180 * 20)).collect();
    let r = timing::compare(&data, &stats, &purchases, 45 * 60, &worth);
    let burn = r.items.iter().find(|i| i.name == "Spirit Burn").unwrap();
    assert_eq!(burn.tone, "very late");
    assert!(burn.median_s >= 24 * 60 && burn.median_s <= 27 * 60 + 59, "{}", burn.median_s);
    assert!(burn.early_s <= burn.median_s && burn.median_s <= burn.late_s);
    // A purchase sold again two seconds later was a misclick.
    assert!(r.items.iter().all(|i| i.name != "Superior Cooldown"));
    // Extra Spirit is what most top players buy, and the player never did.
    assert_eq!(r.missing.first().map(|m| m.name.as_str()), Some("Extra Spirit"));
    assert!(r.summary.contains("Extra Spirit"));
}

#[test]
fn an_item_rushed_ahead_of_its_time_pushes_back_the_ones_after_it() {
    let data = data();
    let id = |name: &str| data.items.iter().find(|i| i.name == name).unwrap().id;
    // Top players buy Extra Spirit at 3 to 5 minutes and Spirit Burn at 24 to 27.
    let mut stats: Vec<ItemStat> = (24..=27).map(|m| row(id("Spirit Burn"), m, 100, 50)).collect();
    stats.extend((3..=5).map(|m| row(id("Extra Spirit"), m, 300, 150)));
    // The player rushes Spirit Burn at 6:00, earning 20 souls a second, and only then buys Extra Spirit, at 12:00.
    let purchases = vec![bought(&data, "Spirit Burn", 6 * 60, None), bought(&data, "Extra Spirit", 12 * 60, None)];
    let worth: Vec<(u32, u32)> = (0..=15).map(|i| (i * 180, i * 180 * 20)).collect();
    let r = timing::compare(&data, &stats, &purchases, 40 * 60, &worth);
    let spirit = r.items.iter().find(|i| i.name == "Extra Spirit").unwrap();
    // 6,400 souls at 20 a second is 5.3 minutes of the delay.
    assert!((spirit.shift_min - 6400.0 / 20.0 / 60.0).abs() < 0.2, "{}", spirit.shift_min);
    assert_eq!(spirit.shifted_by, vec!["Spirit Burn".to_string()]);
    // Rushed itself, Spirit Burn is early and shifted by nothing.
    let burn = r.items.iter().find(|i| i.name == "Spirit Burn").unwrap();
    assert_eq!((burn.tone.as_str(), burn.shift_min), ("early", 0.0));
}
