//! Downloads what area_calibrate needs from recent high-rank match replays: per match, `<id>-meta.json`,
//! `<id>-hits.ndjson` and `<id>-kills.ndjson`, in a directory:
//! `cargo run --release -p engine --example replay_fetch -- <dir> [matches]`
//!
//! Matches are taken from those that ended between half a day and four days ago at Phantom and above: Valve keeps
//! the replays of only some, which answer "not found" and are skipped. Submissions are spaced out to stay within
//! the service's limit of 200 an hour; several matches are waited on at once. Matches already in the directory
//! are not asked for again.

use std::process::Command;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use engine::{data::GameData, replay};
use serde_json::Value;

const API: &str = "https://api.deadlock-api.com/v1";
/// Seconds between two submissions, across all workers: about 200 an hour.
const SPACING_S: u64 = 18;
const WORKERS: usize = 6;
const MIN_BADGE: u32 = 91;

fn curl(args: &[&str]) -> Result<Vec<u8>, String> {
    let out = Command::new("curl").args(["-s", "-f", "--max-time", "120"]).args(args).output().map_err(|e| e.to_string())?;
    if out.status.success() {
        Ok(out.stdout)
    } else {
        Err(format!("curl failed ({})", out.status))
    }
}

fn json(args: &[&str]) -> Result<Value, String> {
    serde_json::from_slice(&curl(args)?).map_err(|e| e.to_string())
}

/// The answer to a query against a match's replay, unpacked.
fn answer(throttle: &Mutex<Instant>, match_id: u64, query: &str) -> Result<String, String> {
    {
        let mut last = throttle.lock().unwrap();
        let wait = Duration::from_secs(SPACING_S).saturating_sub(last.elapsed());
        std::thread::sleep(wait);
        *last = Instant::now();
    }
    let body = serde_json::json!({ "match_id": match_id, "query": query, "format": "ndjson" }).to_string();
    let job = json(&["-X", "POST", "-H", "content-type: application/json", "-d", &body, &format!("{API}/matches/demo/query")])?;
    let id = job["job_id"].as_str().ok_or("no job")?.to_string();
    let started = Instant::now();
    let url = loop {
        let state = json(&[&format!("{API}/matches/demo/query/{id}")])?;
        match state["status"].as_str() {
            Some("done") => break state["result_url"].as_str().ok_or("no result")?.to_string(),
            Some("failed") | Some("error") => return Err(format!("failed: {}", state["error"])),
            _ if started.elapsed() > Duration::from_secs(300) => return Err("timed out".into()),
            _ => std::thread::sleep(Duration::from_secs(8)),
        }
    };
    replay::decompress(&curl(&[&url])?)
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let dir = args.get(1).expect("usage: replay_fetch <dir> [matches]").clone();
    let wanted: usize = args.get(2).and_then(|n| n.parse().ok()).unwrap_or(150);
    std::fs::create_dir_all(&dir).unwrap();
    let data: GameData = serde_json::from_str(include_str!("../../src-tauri/data/snapshot.json")).unwrap();
    let ids: Vec<String> = data.heroes.iter().flat_map(|h| h.abilities.iter().map(|a| a.id.to_string())).collect();
    let hits_query = format!(
        "SELECT tick, ability_id, entindex_attacker, entindex_victim FROM DamageEvent WHERE attacker_class = 1 AND victim_class = 1 AND damage > 0 AND ability_id IN ({})",
        ids.join(",")
    );
    let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();
    let list = json(&[&format!(
        "{API}/matches/metadata?min_unix_timestamp={}&max_unix_timestamp={}&min_average_badge={MIN_BADGE}&min_duration_s=1500&limit={}",
        now - 4 * 86_400,
        now - 12 * 3600,
        wanted * 8
    )])
    .expect("the match list");
    let have = |id: u64| std::path::Path::new(&format!("{dir}/{id}-meta.json")).exists();
    let pool: Vec<u64> = list.as_array().into_iter().flatten().filter_map(|m| m["match_id"].as_u64()).filter(|id| !have(*id)).collect();
    println!("{} matches to try", pool.len());
    let pool = Arc::new(Mutex::new(pool.into_iter()));
    let done = Arc::new(Mutex::new(0usize));
    let throttle = Arc::new(Mutex::new(Instant::now() - Duration::from_secs(SPACING_S)));
    let workers: Vec<_> = (0..WORKERS)
        .map(|_| {
            let (pool, done, throttle, dir, hits_query) = (pool.clone(), done.clone(), throttle.clone(), dir.clone(), hits_query.clone());
            std::thread::spawn(move || loop {
                if *done.lock().unwrap() >= wanted {
                    return;
                }
                let Some(id) = pool.lock().unwrap().next() else { return };
                let got = answer(&throttle, id, &hits_query).and_then(|hits| Ok((hits, answer(&throttle, id, replay::QUERY_KILLS)?))).and_then(|(hits, kills)| {
                    let meta = curl(&[&format!("{API}/matches/{id}/metadata")])?;
                    Ok((hits, kills, meta))
                });
                match got {
                    Ok((hits, kills, meta)) => {
                        std::fs::write(format!("{dir}/{id}-hits.ndjson"), hits).unwrap();
                        std::fs::write(format!("{dir}/{id}-kills.ndjson"), kills).unwrap();
                        std::fs::write(format!("{dir}/{id}-meta.json"), meta).unwrap();
                        let mut n = done.lock().unwrap();
                        *n += 1;
                        println!("ok {id} ({n})");
                    }
                    Err(e) => println!("skip {id}: {e}"),
                }
            })
        })
        .collect();
    for w in workers {
        let _ = w.join();
    }
    println!("done: {} matches", *done.lock().unwrap());
}
