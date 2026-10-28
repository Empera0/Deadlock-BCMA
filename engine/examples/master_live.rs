//! Runs the master builder on live statistics, the way the app does, and prints why each counter item won or
//! lost: `cargo run -p engine --example master_live -- <cache dir> <my hero> <enemies,...> [allies,...] [lane enemies,...]`, heroes by
//! name. Answers are fetched with curl and kept in the cache directory.

use engine::data::GameData;
use engine::master::{self, MasterInput};
use engine::popular::{self, Ally, Enemy, HeroStat, ItemStat, Matchup};

const DAYS: u64 = 14;
const ETERNUS: u32 = 111;
const ASCENDANT: u32 = 101;

fn fetch(dir: &str, endpoint: &str, query: &str, badge: u32) -> String {
    let day = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs() / 86_400;
    let filters = format!("min_unix_timestamp={}&min_average_badge={badge}", (day - DAYS) * 86_400);
    let full = if query.is_empty() { filters } else { format!("{query}&{filters}") };
    let name: String = format!("{endpoint}-{full}").chars().map(|c| if c.is_ascii_alphanumeric() { c } else { '_' }).collect();
    let path = format!("{dir}/{name}.json");
    if let Ok(s) = std::fs::read_to_string(&path) {
        return s;
    }
    let out = std::process::Command::new("curl").args(["-s", "--max-time", "120", &format!("https://api.deadlock-api.com/v1/analytics/{endpoint}?{full}")]).output().unwrap();
    let s = String::from_utf8(out.stdout).unwrap();
    std::fs::write(&path, &s).unwrap();
    s
}

fn popular_for(data: &GameData, dir: &str, id: u64) -> popular::PopularBuild {
    for badge in [ETERNUS, ASCENDANT] {
        let heroes: Vec<HeroStat> = serde_json::from_str(&fetch(dir, "hero-stats", "", badge)).unwrap();
        let matches = heroes.iter().find(|h| h.hero_id == id).map(|h| h.matches).unwrap_or(0);
        if matches < 400 && badge == ETERNUS {
            continue;
        }
        let stats: Vec<ItemStat> = serde_json::from_str(&fetch(dir, "item-stats", &format!("hero_id={id}"), badge)).unwrap();
        return popular::popular_build(data, id, matches, &stats).unwrap();
    }
    unreachable!()
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let dir = &args[1];
    std::fs::create_dir_all(dir).unwrap();
    let data: GameData = serde_json::from_str(include_str!("../../src-tauri/data/snapshot.json")).unwrap();
    let hero = |name: &str| data.heroes.iter().find(|h| h.name.eq_ignore_ascii_case(name.trim())).unwrap_or_else(|| panic!("{name}"));
    let me = hero(&args[2]);
    let enemies: Vec<u64> = args[3].split(',').map(|n| hero(n).id).collect();
    let allies: Vec<u64> = args.get(4).filter(|a| !a.is_empty()).map(|a| a.split(',').map(|n| hero(n).id).collect()).unwrap_or_default();
    let lane: Vec<u64> = args.get(5).filter(|a| !a.is_empty()).map(|a| a.split(',').map(|n| hero(n).id).collect()).unwrap_or_default();

    let ally_builds: Vec<_> = allies.iter().map(|id| popular_for(&data, dir, *id)).collect();
    let ally_team: Vec<Ally> = allies.iter().zip(&ally_builds).map(|(id, build)| Ally { hero_id: *id, build }).collect();
    let builds: Vec<_> = enemies.iter().map(|id| popular_for(&data, dir, *id)).collect();
    let lifts: Vec<_> = enemies
        .iter()
        .map(|id| {
            let general: Vec<ItemStat> = serde_json::from_str(&fetch(dir, "item-stats", "", ETERNUS)).unwrap();
            let against: Vec<ItemStat> = serde_json::from_str(&fetch(dir, "item-stats", &format!("enemy_hero_ids={id}"), ETERNUS)).unwrap();
            popular::lifts(&general, &against)
        })
        .collect();
    let team: Vec<Enemy> = enemies.iter().zip(&builds).zip(&lifts).map(|((id, build), lifts)| Enemy { hero_id: *id, build, lifts }).collect();
    let mine = popular_for(&data, dir, me.id);
    let core: Vec<String> = mine.core.iter().map(|c| c.item.clone()).collect();
    let heroes: Vec<HeroStat> = serde_json::from_str(&fetch(dir, "hero-stats", "", ASCENDANT)).unwrap();
    let hero_matches = heroes.iter().find(|h| h.hero_id == me.id).map(|h| h.matches).unwrap_or(0);
    let phase: Vec<ItemStat> = serde_json::from_str(&fetch(dir, "item-stats", &format!("hero_id={}&bucket=net_worth_by_1000", me.id), ASCENDANT)).unwrap();
    let gun_pct = popular::gun_pct(&data, &mine);
    let spirit = popular::deals_spirit(&data, &mine);
    println!("{}: deals_spirit {spirit}, gun {gun_pct:?}%", me.name);
    // DAMAGE=gun|spirit|hybrid|melee builds for that damage instead of the hero's usual one.
    let damage = std::env::var("DAMAGE").unwrap_or_else(|_| "auto".into());
    let input = MasterInput {
        matchup: Matchup { my_hero: Some(&me.class_name), my_spirit: Some(spirit), my_gun_pct: gun_pct, my_core: &core, allies: &ally_team, enemies: &team, lane: &lane, ranged: true },
        hero_id: me.id,
        phase_stats: &phase,
        hero_matches,
        own_build: &mine,
        focus: "auto",
        damage: &damage,
        scaling: &vec![],
        bought: &[],
    };
    let b = master::master_build(&data, &input).unwrap();
    // JSON_OUT=<file> also writes the build and the plan as the app receives them.
    if let Ok(out) = std::env::var("JSON_OUT") {
        let report = serde_json::json!({ "master": &b, "plan": &b.counter, "builds": &builds });
        std::fs::write(out, serde_json::to_string_pretty(&report).unwrap()).unwrap();
    }
    println!("focus {} style {}", b.focus, b.style);
    for role in &b.counter.roles {
        println!("JOB {} weight {:.2}: {}", role.title, role.weight, role.items.iter().map(|i| format!("{} {:.2}", i.name, i.score)).collect::<Vec<_>>().join(", "));
    }
    for (i, p) in b.phases.iter().enumerate() {
        println!("== {} ", p.title);
        for x in b.items.iter().filter(|x| x.phase == i) {
            println!("   BUY{} {:<22} score {:.2} pop {:>4.1}% engine {:>5.1}% jobs {:?}", if x.counter { " [COUNTER]" } else { "" }, x.name, x.score, x.popular_pct, x.engine_pct, x.jobs);
        }
        for c in &p.considered {
            println!("   not {:<22} {:.2} | {}", c.name, c.score, c.verdict);
        }
        for s in &b.counter.stages {
            if (i == 0 && s.key == "lane") || (i > 0 && i < 3 && s.key == "mid") || (i == 3 && s.key == "late") {
                for name in ["Toxic Bullets", "Healbane", "Spirit Burn", "Inhibitor", "Suppressor", "Spirit Sap"] {
                    if let Some(c) = s.scored.iter().find(|c| data.item(&c.0).is_some_and(|it| it.name == name)) {
                        println!("   [{}] {name}: counter {:.2} jobs {:?} why {:?}", s.key, c.1, c.2, c.3);
                    }
                }
            }
        }
    }
}
