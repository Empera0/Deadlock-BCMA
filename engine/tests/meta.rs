use engine::data::GameData;
use engine::meta;
use engine::popular::ItemStat;
use engine::tiers::HeroRow;

fn data() -> GameData {
    serde_json::from_str(include_str!("../../src-tauri/data/snapshot.json")).unwrap()
}

#[test]
fn patch_days_from_titles() {
    let json = r#"[{"title": "09-29-2026", "pub_date": "2026-10-15T03:12:00Z"}, {"title": "09-16-2026 Update"},
        {"title": "Not a patch"}, {"title": "08-22-2026 Update"}]"#;
    let days = meta::patch_days(json);
    assert_eq!(days.len(), 3);
    assert_eq!(days[0].title, "09-29-2026");
    // 2026-09-29 00:00 UTC.
    assert_eq!(days[0].day, 1_790_640_000);
    assert!(days[1].day < days[0].day && days[2].day < days[1].day);
}

#[test]
fn clear_changes_first() {
    let data = data();
    let (a, b) = (data.heroes[0].id, data.heroes[1].id);
    let row = |hero_id, matches, wins| HeroRow { hero_id, matches, wins, ..Default::default() };
    // Hero a: 48% to 54% over 20,000 matches each: clear. Hero b: 50% to 51% over 400: chance.
    let before = vec![row(a, 20_000, 9_600), row(b, 400, 200)];
    let after = vec![row(a, 20_000, 10_800), row(b, 400, 204)];
    let item = data.items.iter().find(|i| i.cost == 800).unwrap();
    let stat = |matches, wins| ItemStat { item_id: item.id, wins, losses: matches - wins, matches, ..Default::default() };
    let shift = meta::shift(&data, &before, &after, &[stat(5_000, 2_500)], &[stat(5_000, 2_000)]);
    assert_eq!(shift.heroes[0].key, data.heroes[0].class_name);
    assert!(shift.heroes[0].clear && (shift.heroes[0].win_change - 6.0).abs() < 1e-9);
    assert!(!shift.heroes[1].clear);
    assert!(shift.items[0].clear && shift.items[0].win_change < 0.0);
}
