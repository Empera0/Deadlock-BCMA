use engine::data::GameData;
use engine::master::{self, MasterInput};
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
fn the_master_build_fills_every_phase_within_its_souls() {
    let d = data();
    let haze = popular::popular_build(&d, 13, hero_matches(13), &stats(include_str!("popular/items-haze.json"))).unwrap();
    let infernus = popular::popular_build(&d, 1, hero_matches(1), &stats(include_str!("popular/items-infernus.json"))).unwrap();
    let general = stats(include_str!("popular/items-all.json"));
    let vs_infernus = popular::lifts(&general, &stats(include_str!("popular/items-vs-infernus.json")));
    let enemies = [Enemy { hero_id: 1, build: &infernus, lifts: &vs_infernus }];
    let core: Vec<String> = haze.core.iter().map(|c| c.item.clone()).collect();
    let phase_stats = stats(include_str!("popular/phases-haze.json"));
    let scaling = vec![(1, 6.0)];
    let allies = [Ally { hero_id: 1, build: &infernus }];
    let matchup = Matchup { my_hero: Some("hero_haze"), my_spirit: Some(false), my_gun_pct: None, my_core: &core, allies: &allies, enemies: &enemies, lane: &[1], ranged: true };
    let input = MasterInput {
        matchup,
        hero_id: 13,
        phase_stats: &phase_stats,
        hero_matches: hero_matches(13),
        own_build: &haze,
        focus: "auto",
        damage: "auto",
        scaling: &scaling,
        bought: &[("upgrade_headshot_booster".into(), 2_000), ("upgrade_toxic_bullets".into(), 12_000)],
    };
    let build = master::master_build(&d, &input).unwrap();
    println!("{} as {} ({}) | notes {:?}", build.hero_name, build.focus_name, build.style, build.notes);
    for (i, phase) in build.phases.iter().enumerate() {
        println!("-- {} ({}-{}) sold {:?}", phase.title, phase.souls_from, phase.souls_to, phase.sold);
        for x in build.items.iter().filter(|x| x.phase == i) {
            println!(
                "   {:<22} {:>5} paid {:>5} score {:.2} pop {:>4.1}% engine {:>4.1}% jobs {:?} {}",
                x.name, x.cost, x.paid, x.score, x.popular_pct, x.engine_pct, x.jobs, if x.sold { "SOLD" } else { "" }
            );
        }
    }
    assert_eq!(build.phases.len(), 4);
    // An ally who scales late (Infernus here) makes the build a tempo build when left to the app.
    assert_eq!(build.focus, "tempo");
    // Each phase spends within its souls, and the end build fits the slots.
    for (i, phase) in build.phases.iter().enumerate() {
        let spent: u32 = build.items.iter().filter(|x| x.phase <= i).map(|x| x.paid).sum();
        assert!(spent <= phase.souls_to + 6_400, "{} spent {spent}", phase.title);
        assert!(build.items.iter().any(|x| x.phase == i), "nothing bought in {}", phase.title);
    }
    assert!(build.purchases.len() <= 12 + 4);
    // What the player bought in a match is judged in the phase of their net worth then.
    for j in &build.judged {
        println!("JUDGED {} phase {} pop {:.1}% jobs {:?} notes {:?} in master {}", j.name, j.phase, j.popular_pct, j.jobs, j.notes, j.in_master);
    }
    assert!(build.judged.iter().all(|j| j.phase == if j.net_worth < 6_000 { 0 } else { 1 }));
}

#[test]
fn an_item_whose_job_the_team_already_does_is_worth_less() {
    use engine::team::TeamEffects;
    let d = data();
    let id = |name: &str| d.heroes.iter().find(|h| h.name == name).unwrap().id;
    let capacitor = d.items.iter().find(|i| i.name == "Capacitor").unwrap();
    let alone = TeamEffects::of(&d, &[(id("Seven"), vec![])]);
    let with_slows = TeamEffects::of(&d, &[(id("Seven"), vec![]), (id("Wraith"), vec![]), (id("Bebop"), vec![])]);
    let (before, _) = alone.redundancy(capacitor);
    let (after, why) = with_slows.redundancy(capacitor);
    println!("Capacitor alone {before:.2}, with Wraith and Bebop {after:.2}: {why:?}");
    assert!(after < before, "{why:?}");
    assert!(why.unwrap().contains("Bebop"));
    // Healing reduction does not stack: one teammate buying it is enough to halve its worth.
    let healbane = d.items.iter().find(|i| i.name == "Healbane").unwrap();
    let covered = TeamEffects::of(&d, &[(id("Wraith"), vec!["upgrade_toxic_bullets".into()])]);
    let names: Vec<&str> = d.items.iter().filter(|i| i.name == "Toxic Bullets").map(|i| i.class_name.as_str()).collect();
    let covered = if names.contains(&"upgrade_toxic_bullets") { covered } else { TeamEffects::of(&d, &[(id("Wraith"), names.iter().map(|n| n.to_string()).collect())]) };
    assert!(covered.redundancy(healbane).0 <= 0.5);
}
