use engine::data::GameData;
use engine::tiers::{tiers, HeroRow};

#[test]
fn heroes_are_ranked_by_a_pulled_win_rate() {
    let d: GameData = serde_json::from_str(include_str!("../../src-tauri/data/snapshot.json")).unwrap();
    let ids: Vec<u64> = d.heroes.iter().take(3).map(|h| h.id).collect();
    let row = |hero_id, wins, matches| HeroRow { hero_id, wins, matches, total_kills: matches * 5, ..Default::default() };
    // A lucky hero with few matches, a strong one with many, and an even one; a hero's buckets are added up.
    let rows = vec![row(ids[0], 9, 10), row(ids[1], 3_000, 5_000), row(ids[1], 3_000, 5_000), row(ids[2], 6_000, 12_000)];
    let t = tiers(&d, &rows, &[(ids[1], 3), (ids[2], 1)]);
    assert_eq!(t.len(), 3);
    assert_eq!(t[0].hero_id, ids[1]);
    assert_eq!(t[0].matches, 10_000);
    assert_eq!(t[0].tier, "S");
    let lucky = t.iter().find(|x| x.hero_id == ids[0]).unwrap();
    assert!((lucky.win_rate - 90.0).abs() < 1e-9);
    assert!(lucky.ranked_rate < t[0].ranked_rate);
    assert!((t[0].ban_share - 75.0).abs() < 1e-9);
    assert!((t[0].kills - 5.0).abs() < 1e-9);
    // Twelve heroes to a match: the pick rate is the share of matches a hero is in.
    assert!((t[0].pick_rate - 10_000.0 / (22_010.0 / 12.0) * 100.0).abs() < 1e-6);
}
