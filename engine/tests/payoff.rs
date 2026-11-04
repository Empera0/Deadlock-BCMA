use engine::data::GameData;
use engine::{matches, payoff};

fn data() -> GameData {
    serde_json::from_str(include_str!("../../src-tauri/data/snapshot.json")).unwrap()
}

const MATCH: &str = include_str!("sample-match.json");

#[test]
fn items_pay_back() {
    let data = data();
    let report = matches::parse(&data, MATCH).unwrap();
    let mut measured = 0;
    for p in &report.players {
        let rows = payoff::payoff(&data, &report, MATCH, p.slot).unwrap();
        assert_eq!(rows.len(), p.purchases.iter().filter(|x| x.cost > 0).count());
        for r in &rows {
            assert!((r.total - (r.damage + r.healing + r.heal_prevented + r.enabled)).abs() < 1e-6);
            assert!(r.per_1k.is_some() || r.total == 0.0);
        }
        measured += rows.iter().filter(|r| r.per_1k.is_some()).count();
        // What the items dealt cannot be more than the player's hero damage.
        let dealt: f64 = rows.iter().map(|r| r.damage).sum();
        assert!(dealt <= p.hero_damage + 1.0, "{dealt} > {}", p.hero_damage);
    }
    assert!(measured > 10);
    let rows = payoff::payoff(&data, &report, MATCH, report.players[0].slot).unwrap();
    for r in &rows {
        println!("{} {} dmg {:.0} heal {:.0} prev {:.0} en {:.0} per1k {:?} weak {}", r.name, r.cost, r.damage, r.healing, r.heal_prevented, r.enabled, r.per_1k.map(|v| v.round()), r.weak);
    }
}

#[test]
fn order_beside_the_top_players() {
    let data = data();
    let report = matches::parse(&data, MATCH).unwrap();
    let p = &report.players[0];
    let steps: Vec<(usize, u8, u32)> = p.abilities.iter().map(|a| (a.index, a.tier, a.time)).collect();
    let mine: Vec<(usize, u8)> = steps.iter().filter(|s| s.1 > 0).map(|s| (s.0, s.1)).collect();
    // A top order equal to the player's for three upgrades, then different.
    let mut top = mine.clone();
    if top.len() > 3 {
        top[3] = ((top[3].0 + 1) % 4, top[3].1);
    }
    let popular = vec![engine::order::PopularOrder { steps: top, matches: 500, win_pct: 54.0 }];
    let c = engine::order::compare_order(&data, &p.hero, &steps, &popular).unwrap();
    assert_eq!(c.mine.len(), mine.len());
    assert_eq!(c.shared, 3.min(mine.len()));
    assert!(c.mine.iter().all(|s| s.tier >= 1 && !s.ability.is_empty()));
    assert!(engine::order::compare_order(&data, &p.hero, &steps, &[]).unwrap().top.is_empty());
}
