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
    let r = timing::compare(&data, &stats, &purchases, 45 * 60);
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
