//! Writes what the engine says about matches, as the app receives it, to one JSON file per match: the match
//! analysis, every player's death study, and for each player's final build the stats, the combo, the advisor, the
//! ability order and the farm report. The texts in it are what a translation has to cover:
//! `cargo run --release -p engine --example text_dump -- <out dir> <match metadata json>...`

use engine::{advisor, build::BuildInput, calc, combo, data::GameData, deaths, farm, matches, order, replay, suggest};
use serde_json::{json, Value};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let out = &args[1];
    std::fs::create_dir_all(out).unwrap();
    let data: GameData = serde_json::from_str(include_str!("../../src-tauri/data/snapshot.json")).unwrap();
    // The replay study of the engine's test match.
    let fixture = |f: &str| std::fs::read(format!("engine/tests/replay/{f}")).unwrap();
    let text = String::from_utf8(fixture("match.json")).unwrap();
    let hits = replay::decompress(&fixture("hits.ndjson.zst")).unwrap();
    let (notes, kills) = (String::from_utf8(fixture("notes.ndjson")).unwrap(), String::from_utf8(fixture("kills.ndjson")).unwrap());
    let study = replay::study(&data, &text, 6, &hits, &notes, &kills).ok();
    std::fs::write(format!("{out}/replay.json"), serde_json::to_string(&study).unwrap()).unwrap();
    let mut analyses = vec![];
    for path in &args[2..] {
        let text = std::fs::read_to_string(path).unwrap();
        let Ok(report) = matches::parse(&data, &text) else { continue };
        let mut players = vec![];
        for p in &report.players {
            let deaths = deaths::analyze(&data, &text, p.slot).ok();
            if let Some(d) = &deaths {
                analyses.push((report.match_id, d.clone()));
            }
            let summary = matches::summarize(&report, p.account_id);
            let mut builds = vec![];
            // The build at a third, two thirds and the end of the match.
            let samples: Vec<&BuildInput> = p.timeline.iter().map(|x| &x.build).collect();
            for b in [samples.get(samples.len() / 3), samples.get(samples.len() * 2 / 3), samples.last()].into_iter().flatten() {
                let b: &BuildInput = b;
                let mut on = b.clone();
                on.active = on.purchases.clone();
                // Every ability, shooting, a heavy melee and every owned item, against an enemy of the match.
                let mut actions = vec![json!({ "kind": "shoot", "seconds": 3 }), json!({ "kind": "melee", "heavy": true }), json!({ "kind": "parry" })];
                actions.extend((0..4).map(|i| json!({ "kind": "ability", "index": i })));
                actions.extend(on.purchases.iter().map(|c| json!({ "kind": "item", "item": c })));
                actions.extend(on.purchases.iter().map(|c| json!({ "kind": "condition", "item": c, "count": 3 })));
                let enemy = report.players.iter().find(|e| e.team != p.team).map(|e| e.build.clone());
                let combo: combo::ComboInput =
                    serde_json::from_value(json!({ "actions": actions, "headshot_rate": 20, "own_health": 50, "enemy": enemy, "zone": { "rift": true, "minute": 25, "own_souls": 30000, "enemy_souls": 36000, "enemy_walker": true } })).unwrap();
                builds.push(json!({
                    "sheet": calc::compute(&data, &on).ok(),
                    "ranked": calc::rank_next(&data, &on, "dps_sustained", true).ok(),
                    "combo": combo::simulate(&data, &on, &combo).ok(),
                    "fight": suggest::fight(&data, &on),
                    "advice": advisor::advise(&data, &on, "hybrid").ok(),
                    "order": order::ability_order(&data, &on).ok(),
                    "farm": farm::farm(&data, &on, 20.0).ok(),
                }));
            }
            players.push(json!({ "slot": p.slot, "deaths": deaths, "summary": summary, "builds": builds }));
        }
        let id = report.match_id;
        let all: Value = json!({ "report": report, "players": players });
        std::fs::write(format!("{out}/{id}.json"), serde_json::to_string(&all).unwrap()).unwrap();
        println!("{id}");
    }
    std::fs::write(format!("{out}/trend.json"), serde_json::to_string(&deaths::trend(&analyses)).unwrap()).unwrap();
}
