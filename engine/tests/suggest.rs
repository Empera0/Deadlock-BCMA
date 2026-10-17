use engine::build::BuildInput;
use engine::data::GameData;
use engine::suggest;

fn data() -> GameData {
    serde_json::from_str(include_str!("../../src-tauri/data/snapshot.json")).unwrap()
}

fn fixtures() -> Vec<String> {
    vec![include_str!("sample-match.json").to_string(), include_str!("sample-match-2.json").to_string()]
}

#[test]
fn the_stages_end_at_the_souls_players_really_have() {
    let s = suggest::stage_souls(&fixtures()).unwrap();
    println!("{s:?}");
    assert_eq!((s.matches, s.players), (2, 24));
    assert!(s.lane > 3_000 && s.lane < 9_000 && s.mid > s.lane && s.late > s.mid);
    assert!(suggest::stage_souls(&[]).is_none());
}

#[test]
fn builds_are_put_together_per_playstyle_stage_by_stage() {
    let d = data();
    let stages = suggest::stage_souls(&fixtures());
    for hero in ["hero_atlas", "hero_haze", "hero_chessmaster"] {
        let b = BuildInput { hero: hero.into(), souls: Some(40_000), ability_tiers: vec![1, 1, 1, 1], ..Default::default() };
        let started = std::time::Instant::now();
        let s = suggest::suggest(&d, &b, &[], stages.clone()).unwrap();
        println!("{hero}: windows {:?} bare {:?} ({:.1}s)", s.windows, s.bare.iter().map(|x| x.round()).collect::<Vec<_>>(), started.elapsed().as_secs_f64());
        assert_eq!(s.builds.len(), 5);
        for build in &s.builds {
            assert!(build.spent <= build.budget, "{} spent {}", build.name, build.spent);
            assert!(!build.purchases.is_empty());
            let mut names: Vec<&str> = build.items.iter().map(|it| it.item.as_str()).collect();
            names.sort();
            names.dedup();
            assert_eq!(names.len(), build.items.len());
            for (i, stage) in build.stages.iter().enumerate() {
                assert!(stage.spent <= stage.souls);
                assert!(build.items.iter().filter(|it| it.stage == i).all(|it| it.cost <= stage.max_cost && it.cost >= stage.min_cost));
            }
            println!("  {:<14} {:>6} souls  after {:?}  ehp {:.0}", build.name, build.spent, build.damage_after.iter().map(|x| x.round()).collect::<Vec<_>>(), build.effective_health_after);
            println!("      {}", build.items.iter().filter(|i| !i.sold && !i.upgraded).map(|i| i.name.as_str()).collect::<Vec<_>>().join(", "));
        }
        let by = |key: &str| s.builds.iter().find(|x| x.key == key).unwrap();
        // Each build leads its own window, or very nearly: items are picked one at a time.
        for (key, window) in [("hybrid", 0), ("gun", 1), ("spirit", 2), ("melee", 3)] {
            let own = by(key).damage_after[window];
            assert!(s.builds.iter().all(|other| own >= other.damage_after[window] * 0.93), "{hero} {key}");
        }
        assert!(by("frontline").effective_health_after > by("hybrid").effective_health_after);
    }
    // A melee build buys melee items.
    let b = BuildInput { hero: "hero_atlas".into(), souls: Some(40_000), ability_tiers: vec![1, 1, 1, 1], ..Default::default() };
    let s = suggest::suggest(&d, &b, &["melee".to_string()], stages).unwrap();
    assert_eq!(s.builds.len(), 1);
    let melee_items = ["Melee Charge", "Lifestrike", "Crushing Fists", "Melee Lifesteal", "Spirit Snatch", "Colossus"];
    assert!(s.builds[0].items.iter().filter(|i| melee_items.contains(&i.name.as_str())).count() >= 2);
}

#[test]
fn the_advisor_buys_upgrades_and_swaps() {
    let d = data();
    let names = [
        "Extra Health", "Extra Regen", "Extra Stamina", "Sprint Boots", "Melee Lifesteal", "Rebuttal",
        "Close Quarters", "Headshot Booster", "High-Velocity Rounds", "Extra Charge", "Extra Spirit", "Mystic Burst",
    ];
    let purchases: Vec<String> = names.iter().map(|n| d.items.iter().find(|i| i.name == *n).unwrap_or_else(|| panic!("{n}")).class_name.clone()).collect();
    let b = BuildInput { hero: "hero_haze".into(), purchases, souls: Some(20_000), ability_tiers: vec![1, 1, 1, 1], ..Default::default() };
    let started = std::time::Instant::now();
    let a = engine::advisor::advise(&d, &b, "hybrid").unwrap();
    println!("{:.1}s free {} slots {}/{}", started.elapsed().as_secs_f64(), a.free_souls, a.slots_used, a.slot_limit);
    for m in a.upgrades.iter().chain(&a.swaps).chain(&a.buys) {
        println!("{} {:?} -> {} paid {} net {} gain {:.3}", m.kind, m.sell_name, m.buy_name, m.paid, m.net_cost, m.gain);
    }
    println!("worst: {:?}", a.worth.iter().map(|w| (&w.name, (w.loss * 1000.0).round())).collect::<Vec<_>>());
    assert_eq!(a.slots_used, 12);
    // The slots are full: nothing new fits without selling, but an owned item can be upgraded.
    assert!(a.buys.is_empty());
    assert!(!a.swaps.is_empty() && !a.upgrades.is_empty());
    for m in &a.swaps {
        assert!(m.sell.is_some() && m.paid <= a.free_souls + m.refund && m.gain > 0.0);
    }
    assert_eq!(a.worth.len(), 12);
    assert!(a.worth.windows(2).all(|w| w[0].loss <= w[1].loss));
}
