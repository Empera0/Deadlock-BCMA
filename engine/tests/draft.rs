use engine::data::GameData;
use engine::draft::{self, DraftData, DraftInput, Pair};
use engine::popular::HeroStat;

fn data() -> GameData {
    serde_json::from_str(include_str!("../../src-tauri/data/snapshot.json")).unwrap()
}

fn stat(hero_id: u64, wins: u64, matches: u64) -> HeroStat {
    serde_json::from_value(serde_json::json!({ "hero_id": hero_id, "wins": wins, "matches": matches })).unwrap()
}

#[test]
fn picks_follow_strength_matchups_lane_and_own_record() {
    let d = data();
    let ids: Vec<u64> = d.heroes.iter().take(8).map(|h| h.id).collect();
    let (strong, counter, laner, plain, enemy, ally, banned, mine) = (ids[0], ids[1], ids[2], ids[3], ids[4], ids[5], ids[6], ids[7]);
    let even = |id| stat(id, 5_000, 10_000);
    let stats = DraftData {
        heroes: vec![stat(strong, 5_600, 10_000), even(counter), even(laner), even(plain), even(enemy), even(ally), even(banned), even(mine)],
        // `counter` beats `enemy` 60% of 5,000 matches; the API gives only this direction.
        counters: vec![Pair { a: counter, b: enemy, wins: 3_000, matches: 5_000 }],
        lane_counters: vec![Pair { a: laner, b: enemy, wins: 1_800, matches: 3_000 }],
        synergies: vec![Pair { a: plain, b: ally, wins: 700, matches: 1_000 }],
        mine: vec![stat(mine, 40, 50)],
        bans: vec![(strong, 30), (enemy, 10)],
    };
    let input = DraftInput { allies: vec![ally], enemies: vec![enemy], lane: vec![enemy], banned: vec![banned] };
    let r = draft::draft(&d, &stats, &input);
    let edge = |id| r.picks.iter().find(|p| p.hero_id == id).map(|p| p.edge);
    for taken in [ally, enemy, banned] {
        assert!(edge(taken).is_none(), "a picked or banned hero is offered");
    }
    let plain_edge = edge(plain).unwrap();
    assert!(plain_edge.abs() < 0.05);
    for better in [strong, counter, laner, mine] {
        assert!(edge(better).unwrap() > plain_edge + 0.3, "{better}");
    }
    assert!(r.picks.windows(2).all(|w| w[0].edge >= w[1].edge));
    // The matchup is read from the other side too: the enemy is weak into `counter`.
    let threat = r.threats.iter().find(|t| t.hero_id == enemy).unwrap();
    assert!(threat.edge.abs() < 0.05);
    // An even draft is even; the strong pick raises it.
    let team = r.team_rate.unwrap();
    assert!((team - 50.0).abs() < 0.05);
    let with_strong = r.picks.iter().find(|p| p.hero_id == strong).unwrap();
    assert!(with_strong.team_rate.unwrap() > team);
    assert_eq!(with_strong.mine, None);
    let own = r.picks.iter().find(|p| p.hero_id == mine).unwrap();
    assert_eq!(own.mine.map(|m| m.0), Some(50));
    assert!(own.reasons.iter().any(|x| x.label.starts_with("Your record")));
    // The strongest hero left is the first ban.
    assert_eq!(r.bans[0].hero_id, strong);
    assert!((r.bans[0].ban_share - 75.0).abs() < 1e-9);
    // Each placed hero lists how the others fare with it (an ally) or against it (an enemy).
    let fit = |focus: u64, hero: u64| r.fits.iter().find(|f| f.hero_id == focus).unwrap().fits.iter().find(|f| f.hero_id == hero).unwrap().clone();
    let with_ally = fit(ally, plain);
    assert_eq!((with_ally.rate, with_ally.matches), (Some(70.0), 1_000));
    assert!(with_ally.points > 10.0);
    assert!(fit(enemy, counter).points > 5.0);
    assert_eq!(fit(enemy, plain).rate, None);
    assert!(r.fits.iter().find(|f| f.hero_id == ally).unwrap().ally);
    // A hero's page: its best matchup and its best partner first.
    let (against, with) = draft::matchups(&d, &stats, counter);
    assert_eq!(against[0].hero_id, enemy);
    assert!(against[0].points > 5.0);
    let (_, partners) = draft::matchups(&d, &stats, ally);
    assert_eq!(partners[0].hero_id, plain);
    assert!(with.is_empty());
}

#[test]
fn api_rows_are_read() {
    let counters = draft::parse_counters(r#"[{"hero_id":1,"enemy_hero_id":2,"wins":3,"matches_played":5,"kills":9}]"#).unwrap();
    let pairs = draft::parse_synergies(r#"[{"hero_id1":1,"hero_id2":2,"wins":3,"matches_played":5}]"#).unwrap();
    assert_eq!((pairs[0].a, pairs[0].b, pairs[0].wins, pairs[0].matches), (1, 2, 3, 5));
    assert_eq!((counters[0].a, counters[0].b, counters[0].wins, counters[0].matches), (1, 2, 3, 5));
    let bans = draft::parse_bans(r#"[{"hero_id":1,"bucket":0,"bans":4},{"hero_id":1,"bucket":1,"bans":2}]"#).unwrap();
    assert_eq!(bans, vec![(1, 6)]);
}
