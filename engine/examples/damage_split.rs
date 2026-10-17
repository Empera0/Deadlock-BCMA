//! Measures how much of each hero's damage to enemy heroes is gun damage (bullets and melee), from the damage
//! matrices of recent high-rank matches, and writes engine/data/gun-share.json. Run it about once a month:
//! `cargo run -p engine --example damage_split -- <cache dir> [matches]`. Match answers are kept in the cache
//! directory and the requests are spaced out, so a second run asks the service for nothing it already gave.

use std::collections::BTreeMap;
use std::process::Command;

use serde_json::Value;

const MIN_BADGE: u32 = 91;
const PAUSE_MS: u64 = 400;
/// A hero needs this many players in the sample for its share to be written.
const MIN_PLAYERS: u32 = 15;

fn curl(url: &str) -> String {
    let out = Command::new("curl").args(["-s", "--max-time", "120", url]).output().expect("curl");
    String::from_utf8_lossy(&out.stdout).into_owned()
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let dir = &args[1];
    let wanted: usize = args.get(2).and_then(|n| n.parse().ok()).unwrap_or(200);
    std::fs::create_dir_all(dir).unwrap();
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs();
    let list = curl(&format!(
        "https://api.deadlock-api.com/v1/matches/metadata?min_unix_timestamp={}&min_average_badge={MIN_BADGE}&min_duration_s=1200&limit={wanted}",
        now - 3 * 86_400
    ));
    let ids: Vec<u64> = serde_json::from_str::<Vec<Value>>(&list).unwrap().iter().filter_map(|m| m.get("match_id")?.as_u64()).collect();
    // hero id -> (gun damage, all damage, players)
    let mut totals: BTreeMap<u64, (f64, f64, u32)> = BTreeMap::new();
    let mut read = 0;
    for id in ids {
        let path = format!("{dir}/{id}.json");
        let json = match std::fs::read_to_string(&path) {
            Ok(s) => s,
            Err(_) => {
                std::thread::sleep(std::time::Duration::from_millis(PAUSE_MS));
                let s = curl(&format!("https://api.deadlock-api.com/v1/matches/{id}/metadata"));
                std::fs::write(&path, &s).unwrap();
                s
            }
        };
        let Ok(root) = serde_json::from_str::<Value>(&json) else { continue };
        let Some(info) = root.get("match_info") else { continue };
        let players = info.get("players").and_then(Value::as_array).cloned().unwrap_or_default();
        let slot = |p: &Value| p.get("player_slot").and_then(Value::as_u64).unwrap_or(0);
        let team_of: BTreeMap<u64, (u64, String)> = players
            .iter()
            .map(|p| (slot(p), (p.get("hero_id").and_then(Value::as_u64).unwrap_or(0), p.get("team").map(|t| t.to_string()).unwrap_or_default())))
            .collect();
        let Some(matrix) = info.get("damage_matrix") else { continue };
        let names = matrix.pointer("/source_details/source_name").and_then(Value::as_array).cloned().unwrap_or_default();
        let types = matrix.pointer("/source_details/stat_type").and_then(Value::as_array).cloned().unwrap_or_default();
        let mut per: BTreeMap<u64, (f64, f64)> = BTreeMap::new();
        for dealer in matrix.get("damage_dealers").and_then(Value::as_array).into_iter().flatten() {
            let from = dealer.get("dealer_player_slot").and_then(Value::as_u64).unwrap_or(99);
            let Some((hero, team)) = team_of.get(&from) else { continue };
            for source in dealer.get("damage_sources").and_then(Value::as_array).into_iter().flatten() {
                let index = source.get("source_details_index").and_then(Value::as_u64).unwrap_or(0) as usize;
                let name = names.get(index).and_then(Value::as_str).unwrap_or("");
                // The matrix sums each dealer's damage by kind in rows of their own: Bullet (every bullet, also from
                // abilities such as Haze's Fixation and Bullet Dance), Melee, Ability (spirit) and Misc (pure).
                // Resists work by kind, so these rows are what counts; the detailed rows name the source.
                if types.get(index).and_then(Value::as_u64) != Some(0) || !["Ability", "Bullet", "Melee"].contains(&name) {
                    continue;
                }
                let gun = name != "Ability";
                for target in source.get("damage_to_players").and_then(Value::as_array).into_iter().flatten() {
                    let to = target.get("target_player_slot").and_then(Value::as_u64).unwrap_or(99);
                    // Only damage to enemy heroes.
                    if team_of.get(&to).is_none_or(|t| &t.1 == team) {
                        continue;
                    }
                    let amount = target.get("damage").and_then(Value::as_array).and_then(|d| d.last()).and_then(Value::as_f64).unwrap_or(0.0);
                    let e = per.entry(*hero).or_default();
                    e.1 += amount;
                    if gun {
                        e.0 += amount;
                    }
                }
            }
        }
        if per.is_empty() {
            continue;
        }
        read += 1;
        for (hero, (gun, all)) in per {
            let t = totals.entry(hero).or_default();
            t.0 += gun;
            t.1 += all;
            t.2 += 1;
        }
    }
    let shares: BTreeMap<String, f64> = totals
        .iter()
        .filter(|(_, t)| t.2 >= MIN_PLAYERS && t.1 > 0.0)
        .map(|(hero, t)| (hero.to_string(), (t.0 / t.1 * 1000.0).round() / 10.0))
        .collect();
    let out = serde_json::json!({
        "about": "Share of each hero's damage to enemy heroes that is bullet or melee damage (the rest is spirit damage), percent, by hero id. Written by examples/damage_split.rs.",
        "matches": read,
        "min_average_badge": MIN_BADGE,
        "measured_at": now,
        "gun_pct": shares,
    });
    std::fs::write(concat!(env!("CARGO_MANIFEST_DIR"), "/data/gun-share.json"), serde_json::to_string_pretty(&out).unwrap() + "\n").unwrap();
    println!("{read} matches, {} heroes", shares.len());
}
