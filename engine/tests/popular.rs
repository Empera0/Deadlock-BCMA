use engine::data::GameData;
use engine::popular::{self, Ally, Enemy, HeroStat, ItemStat, Matchup};

fn data() -> GameData {
    serde_json::from_str(include_str!("../../src-tauri/data/snapshot.json")).unwrap()
}

fn stats(json: &str) -> Vec<ItemStat> {
    serde_json::from_str(json).unwrap()
}

fn hero_matches(id: u64) -> u64 {
    let heroes: Vec<HeroStat> = serde_json::from_str(include_str!("popular/hero-stats.json")).unwrap();
    heroes.iter().find(|h| h.hero_id == id).unwrap().matches
}

#[test]
fn the_popular_core_build_is_what_most_players_buy_and_keep() {
    let d = data();
    let haze = popular::popular_build(&d, 13, hero_matches(13), &stats(include_str!("popular/items-haze.json"))).unwrap();
    assert_eq!(haze.core.len(), 8);
    assert!(haze.core.windows(2).all(|w| w[0].buy_minute <= w[1].buy_minute));
    let lowest_core = haze.core.iter().map(|c| c.kept_pct).fold(f64::INFINITY, f64::min);
    assert!(haze.situational.iter().all(|s| s.kept_pct <= lowest_core));
}

#[test]
fn counters_are_planned_job_by_job_against_the_enemy_team() {
    let d = data();
    let general = stats(include_str!("popular/items-all.json"));
    let haze = popular::popular_build(&d, 13, hero_matches(13), &stats(include_str!("popular/items-haze.json"))).unwrap();
    let infernus = popular::popular_build(&d, 1, hero_matches(1), &stats(include_str!("popular/items-infernus.json"))).unwrap();
    let vs_haze = popular::lifts(&general, &stats(include_str!("popular/items-vs-haze.json")));
    let vs_infernus = popular::lifts(&general, &stats(include_str!("popular/items-vs-infernus.json")));
    let enemies = [Enemy { hero_id: 13, build: &haze, lifts: &vs_haze }, Enemy { hero_id: 1, build: &infernus, lifts: &vs_infernus }];
    let plan = popular::counter_plan(&d, &Matchup { my_hero: Some("hero_dynamo"), my_spirit: Some(true), my_core: &[], allies: &[], enemies: &enemies, lane: &[], ranged: false });
    // Two counter slots for each stage, two different items at the stage's price.
    assert_eq!(plan.stages.len(), 3);
    for stage in &plan.stages {
        assert_eq!(stage.picks.len(), 2, "{}", stage.title);
        assert_ne!(stage.picks[0].advice.item, stage.picks[1].advice.item);
        for p in stage.picks.iter().chain(&stage.alternatives) {
            println!("{:<12} {:<22} {:>5.2} jobs {:?} | {} | {:?}", stage.title, p.advice.name, p.advice.score, p.jobs, p.advice.effect, p.advice.reasons.first());
        }
    }
    let lane = &plan.stages[0];
    assert!(lane.picks.iter().all(|p| p.advice.cost <= 1_600));
    // Both heal, so each stage cuts their healing.
    for stage in &plan.stages {
        assert!(stage.picks.iter().any(|p| p.jobs.iter().any(|j| j == "Cut their healing")), "{}", stage.title);
    }
    // The other counter items: none of the picks, best first.
    assert!(!plan.others.is_empty());
    assert!(plan.others.iter().all(|o| plan.stages.iter().all(|s| s.picks.iter().all(|p| p.advice.item != o.advice.item))));
    assert!(plan.others.windows(2).all(|w| w[0].advice.score >= w[1].advice.score));
    // An item already in the player's build is not suggested again.
    let first = lane.picks[0].advice.item.clone();
    let core = [first.clone()];
    let again = popular::counter_plan(&d, &Matchup { my_hero: Some("hero_dynamo"), my_spirit: Some(true), my_core: &core, allies: &[], enemies: &enemies, lane: &[], ranged: false });
    assert!(again.stages[0].picks.iter().all(|p| p.advice.item != first));

    // Hero by hero: both enemies have threats, and the plan answers each of them somewhere.
    assert_eq!(plan.heroes.len(), 2);
    for h in &plan.heroes {
        println!("HERO {} threats {:?} answers {:?}", h.hero_name, h.threats, h.answers);
        assert!(!h.threats.is_empty() && !h.answers.is_empty(), "{}", h.hero_name);
    }

    // Only Infernus in the lane: the lane counters answer Infernus alone, Haze comes in the mid game.
    let laned = popular::counter_plan(&d, &Matchup { my_hero: Some("hero_dynamo"), my_spirit: Some(true), my_core: &[], allies: &[], enemies: &enemies, lane: &[1], ranged: false });
    let lane = &laned.stages[0];
    println!("LANE vs Infernus: {:?} | {}", lane.picks.iter().map(|p| p.advice.name.as_str()).collect::<Vec<_>>(), lane.note);
    assert!(lane.picks.iter().all(|p| !p.advice.against.iter().any(|a| a == "Haze")));
    assert!(laned.heroes.iter().any(|h| h.hero_name == "Infernus" && h.in_lane));
    assert!(laned.stages[1].picks.iter().any(|p| p.advice.against.iter().any(|a| a == "Haze")));

    // A teammate who usually buys healing reduction takes some of that job off the player from the mid game.
    let allies = [Ally { hero_id: 1, build: &infernus }];
    let with_team = popular::counter_plan(&d, &Matchup { my_hero: Some("hero_dynamo"), my_spirit: Some(true), my_core: &[], allies: &allies, enemies: &enemies[..1], lane: &[], ranged: false });
    println!("TEAM {:?}", with_team.team);
    assert_eq!(with_team.allies.len(), 1);
    assert!(!with_team.team.is_empty());
    for p in &plan.enemies {
        println!("{:<10} weapon {:>3.0}% heals {:?} moves {:?} locks {:?} resists {:?}", p.hero_name, p.weapon_pct, p.heals, p.mobility, p.lockdown, p.resists);
    }
    let role = |key: &str| plan.roles.iter().find(|r| r.key == key);
    // Both heal (Infernus' Napalm and Spiritual Overflow, Haze's Smoke Bomb lifesteal): healing reduction is a job,
    // with Healbane as the cheap answer.
    let healing = role("healing").expect("healing reduction");
    let names: Vec<&str> = healing.items.iter().map(|i| i.name.as_str()).collect();
    assert!(names.contains(&"Healbane"), "{names:?}");
    // Haze shoots: something against her gun.
    let gun = role("gun").expect("against guns");
    let gun_names: Vec<&str> = gun.items.iter().map(|i| i.name.as_str()).collect();
    assert!(gun_names.contains(&"Suppressor") || gun_names.contains(&"Rusted Barrel"), "{gun_names:?}");
    // Every job has a few items, and no item stands for a job it does not do.
    for r in &plan.roles {
        assert!(!r.items.is_empty() && r.items.len() <= 5);
        assert!(r.items.windows(2).all(|w| w[0].cost <= w[1].cost));
        println!("\n{} (weight {:.2}) - {}", r.title, r.weight, r.need);
        for i in &r.items {
            println!("   {:<22} {:>6.2}  {} | {}", i.name, i.score, i.effect, i.reasons.iter().filter(|x| x.starts_with("Top")).cloned().collect::<Vec<_>>().join("; "));
        }
    }
    // Dynamo can teleport away, which makes answers to crowd control count for less.
    assert!(plan.own_escape.is_empty() || plan.roles.iter().all(|r| r.key != "control" || r.weight < 1.0));
}
