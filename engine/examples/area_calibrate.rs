//! Measures how many enemy heroes each ability really hits per cast, from match replays, and fits the enemy density
//! of the team-fight area model (engine/src/area.rs) to it. The directory holds, per match, `<id>-meta.json` (the
//! match metadata), `<id>-hits.ndjson` (the replay's hero-on-hero ability damage events: tick, ability_id,
//! entindex_attacker, entindex_victim) and `<id>-kills.ndjson` (its HeroKilledEvent ticks):
//! `cargo run --release -p engine --example area_calibrate -- <dir>`
//!
//! The replay files come from the statistics service's demo queries (POST /v1/matches/demo/query, format ndjson,
//! the result compressed with zstd), for matches with a replay; the ability ids are every hero ability's id:
//!   SELECT tick, ability_id, entindex_attacker, entindex_victim FROM DamageEvent
//!     WHERE attacker_class = 1 AND victim_class = 1 AND damage > 0 AND ability_id IN (<ability ids>)
//!   SELECT tick, entindex_victim, entindex_attacker FROM HeroKilledEvent
//! Run it again every month or so, with the damage split.
//!
//! A cast is a run of one hero's damage events from one ability with no gap over two seconds; the enemies it hit are
//! the distinct victims in it. Replay ticks are put on the match clock by lining up the kills with the deaths the
//! metadata records. Casts during a team fight are compared with the model at the caster's build of that moment.
//!
//! Each ability with enough casts gets its own density, the one at which the model gives the enemies it really hit
//! on average; the rest use the density fitted over all of them. Both are written to engine/data/area-density.json.

use std::collections::BTreeMap;

use engine::{area, calc, data::GameData, deaths, matches};
use serde_json::Value;

/// A cast ends where one hero's damage from one ability stops for this long, seconds.
const CAST_GAP_S: f64 = 2.0;
/// A cast counts as part of a team fight from this many seconds before it starts to this many after it ends.
const FIGHT_MARGIN_S: f64 = 3.0;
/// Casts in team fights an ability needs for a density of its own, and how many casts' worth of the density over
/// all abilities its own average is pulled toward: with 20 casts it is half its own, with 200 nearly all.
const MIN_CASTS: usize = 10;
const PRIOR_CASTS: f64 = 20.0;
/// The caster's build is read once per this many seconds of the match.
const BUILD_STEP_S: u32 = 15;

struct Cast {
    class: String,
    hero: String,
    ability: String,
    area: Option<(f64, f64)>,
    hit: f64,
    fight: bool,
}

fn rows(path: &str) -> Vec<Value> {
    std::fs::read_to_string(path).unwrap_or_default().lines().filter_map(|l| serde_json::from_str(l).ok()).collect()
}

/// Least squares of the match clock against replay ticks: (seconds per tick, offset).
fn clock(ticks: &[f64], times: &[f64]) -> Option<(f64, f64)> {
    let n = ticks.len().min(times.len());
    if n < 3 {
        return None;
    }
    let (mx, my) = (ticks[..n].iter().sum::<f64>() / n as f64, times[..n].iter().sum::<f64>() / n as f64);
    let sxy: f64 = (0..n).map(|i| (ticks[i] - mx) * (times[i] - my)).sum();
    let sxx: f64 = (0..n).map(|i| (ticks[i] - mx).powi(2)).sum();
    let a = sxy / sxx;
    Some((a, my - a * mx))
}

fn main() {
    let dir = std::env::args().nth(1).expect("usage: area_calibrate <dir>");
    let data: GameData = serde_json::from_str(include_str!("../../src-tauri/data/snapshot.json")).unwrap();
    let owner: BTreeMap<u64, (u64, String, String)> = data
        .heroes
        .iter()
        .flat_map(|h| h.abilities.iter().map(move |a| (a.id, (h.id, h.name.clone(), a.class_name.clone()))))
        .collect();
    let mut casts: Vec<Cast> = vec![];
    let mut ids: Vec<String> = std::fs::read_dir(&dir)
        .unwrap()
        .filter_map(|e| e.ok()?.file_name().into_string().ok())
        .filter_map(|n| n.strip_suffix("-meta.json").map(String::from))
        .collect();
    ids.sort();
    for id in &ids {
        let json = std::fs::read_to_string(format!("{dir}/{id}-meta.json")).unwrap();
        let Ok(report) = matches::parse(&data, &json) else { continue };
        let root: Value = serde_json::from_str(&json).unwrap();
        let players = root["match_info"]["players"].as_array().cloned().unwrap_or_default();
        let mut deaths: Vec<f64> = players
            .iter()
            .flat_map(|p| p["death_details"].as_array().cloned().unwrap_or_default())
            .filter_map(|d| d["game_time_s"].as_f64())
            .collect();
        deaths.sort_by(f64::total_cmp);
        let mut kills: Vec<f64> = rows(&format!("{dir}/{id}-kills.ndjson")).iter().filter_map(|r| r["tick"].as_f64()).collect();
        kills.sort_by(f64::total_cmp);
        let Some((per_tick, offset)) = clock(&kills, &deaths) else { continue };
        let slot_of = |hero: u64| players.iter().find(|p| p["hero_id"].as_u64() == Some(hero)).and_then(|p| p["player_slot"].as_u64());
        // One hero's events from one ability, in order.
        let mut runs: BTreeMap<(u64, u64), Vec<(f64, u64)>> = BTreeMap::new();
        for r in rows(&format!("{dir}/{id}-hits.ndjson")) {
            let (Some(tick), Some(ability), Some(attacker), Some(victim)) =
                (r["tick"].as_f64(), r["ability_id"].as_u64(), r["entindex_attacker"].as_u64(), r["entindex_victim"].as_u64())
            else {
                continue;
            };
            runs.entry((attacker, ability)).or_default().push((tick * per_tick + offset, victim));
        }
        let mut found = 0;
        let mut sheets: BTreeMap<(u64, u32), Option<calc::Sheet>> = BTreeMap::new();
        for ((_, ability), mut events) in runs {
            let Some((hero_id, hero, class)) = owner.get(&ability) else { continue };
            let Some(slot) = slot_of(*hero_id) else { continue };
            events.sort_by(|a, b| a.0.total_cmp(&b.0));
            let mut start = 0;
            for i in 1..=events.len() {
                if i < events.len() && events[i].0 - events[i - 1].0 <= CAST_GAP_S {
                    continue;
                }
                let cast = &events[start..i];
                start = i;
                let (from, to) = (cast[0].0, cast[cast.len() - 1].0);
                let mut victims: Vec<u64> = cast.iter().map(|e| e.1).collect();
                victims.sort();
                victims.dedup();
                let fight = report.fights.iter().any(|f| from >= f.start as f64 - FIGHT_MARGIN_S && to <= f.end as f64 + FIGHT_MARGIN_S);
                let at = from.max(0.0) as u32 / BUILD_STEP_S * BUILD_STEP_S;
                let sheet = sheets
                    .entry((slot, at))
                    .or_insert_with(|| deaths::build_in_match(&data, &root, slot as u32, at).and_then(|b| calc::compute(&data, &b).ok()));
                let Some(a) = sheet.as_ref().and_then(|s| s.abilities.iter().find(|a| &a.class_name == class)) else { continue };
                casts.push(Cast { class: class.clone(), hero: hero.clone(), ability: a.name.clone(), area: area::covered(a), hit: victims.len() as f64, fight });
                found += 1;
            }
        }
        println!("{id}: {:.4} s/tick ({:.0} ticks/s), {} kills, {found} casts", per_tick, 1.0 / per_tick, kills.len());
    }

    let fights: Vec<&Cast> = casts.iter().filter(|c| c.fight).collect();
    let shaped: Vec<&&Cast> = fights.iter().filter(|c| c.area.is_some_and(|a| a.0 > 0.0)).collect();
    let single: Vec<&&Cast> = fights.iter().filter(|c| c.area.is_none()).collect();
    let mean = |v: &[f64]| if v.is_empty() { 0.0 } else { v.iter().sum::<f64>() / v.len() as f64 };
    let sse = |d: f64| shaped.iter().map(|c| (c.hit - c.area.map(|(a, m)| area::enemies_in(a, m, d)).unwrap_or(1.0)).powi(2)).sum::<f64>();
    let best = (1..=400).map(|i| i as f64 * 0.0001).min_by(|a, b| sse(*a).total_cmp(&sse(*b))).unwrap_or(0.007);
    println!(
        "\n{} casts, {} in team fights: {} by area abilities, {} by single-target ones (they hit {:.2} enemies on average)",
        casts.len(),
        fights.len(),
        shaped.len(),
        single.len(),
        mean(&single.iter().map(|c| c.hit).collect::<Vec<_>>())
    );
    println!("Fitted density {best:.4} per m² (error {:.1})\n", sse(best));
    let mut by: BTreeMap<String, Vec<&&Cast>> = BTreeMap::new();
    for c in &shaped {
        by.entry(c.class.clone()).or_default().push(c);
    }
    let mut own: BTreeMap<String, f64> = BTreeMap::new();
    for (class, list) in &by {
        let model = |d: f64| mean(&list.iter().map(|c| c.area.map(|(a, m)| area::enemies_in(a, m, d)).unwrap_or(1.0)).collect::<Vec<_>>());
        let seen = mean(&list.iter().map(|c| c.hit).collect::<Vec<_>>());
        // Few casts say little: their average is pulled toward what the density over all abilities gives them.
        let n = list.len() as f64;
        let hits = (n * seen + PRIOR_CASTS * model(best)) / (n + PRIOR_CASTS);
        // The density at which the model's average over these casts is that many enemies.
        let (mut lo, mut hi) = (0.0, 1.0);
        for _ in 0..60 {
            let mid = (lo + hi) / 2.0;
            if model(mid) < hits { lo = mid } else { hi = mid }
        }
        let d = (lo + hi) / 2.0;
        if list.len() >= MIN_CASTS {
            own.insert(class.clone(), (d * 1e5).round() / 1e5);
        }
        println!(
            "{:<14} {:<24} {:>4} casts: hits {seen:.2} (kept {hits:.2}), model {:.2} at the fit; own density {d:.4}{}",
            list[0].hero,
            list[0].ability,
            list.len(),
            model(best),
            if list.len() >= MIN_CASTS { "" } else { " (too few casts, not kept)" }
        );
    }
    // Abilities set as single-target that hit more than one enemy often enough to need a shape.
    let mut flat: BTreeMap<String, Vec<&&Cast>> = BTreeMap::new();
    for c in &single {
        flat.entry(c.class.clone()).or_default().push(c);
    }
    for list in flat.values().filter(|l| l.len() >= MIN_CASTS) {
        let hits = mean(&list.iter().map(|c| c.hit).collect::<Vec<_>>());
        if hits >= 1.15 {
            println!("single-target {:<14} {:<24} {:>4} casts: hits {hits:.2}", list[0].hero, list[0].ability, list.len());
        }
    }
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs();
    let out = serde_json::json!({
        "about": "Enemy heroes per square meter around a team-fight ability's target, fitted to the enemies each cast really hit in match replays: one density over all area abilities, and one per ability with enough casts. Written by examples/area_calibrate.rs.",
        "matches": ids.len(),
        "casts": shaped.len(),
        "measured_at": now,
        "density": (best * 1e5).round() / 1e5,
        "abilities": own,
    });
    std::fs::write(concat!(env!("CARGO_MANIFEST_DIR"), "/data/area-density.json"), serde_json::to_string_pretty(&out).unwrap() + "\n").unwrap();
    println!("\nWrote engine/data/area-density.json: {} abilities with their own density", own.len());
}
