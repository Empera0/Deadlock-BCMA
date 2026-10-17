//! Runs the draft helper on statistics saved from the API: `cargo run -p engine --example draft -- <dir>` with
//! heroes.json, counters.json, lane.json and bans.json in the directory. `--json` prints the report as JSON.

use engine::data::GameData;
use engine::draft::{self, DraftData, DraftInput};

fn main() {
    let dir = std::env::args().nth(1).expect("directory");
    let read = |f: &str| std::fs::read_to_string(format!("{dir}/{f}")).unwrap();
    let data: GameData = serde_json::from_str(include_str!("../../src-tauri/data/snapshot.json")).unwrap();
    let d = DraftData {
        heroes: serde_json::from_str(&read("heroes.json")).unwrap(),
        counters: draft::parse_counters(&read("counters.json")).unwrap(),
        lane_counters: draft::parse_counters(&read("lane.json")).unwrap(),
        mine: vec![],
        bans: draft::parse_bans(&read("bans.json")).unwrap(),
    };
    let id = |name: &str| data.heroes.iter().find(|h| h.name == name).map(|h| h.id).unwrap();
    let input = DraftInput {
        allies: vec![id("Haze"), id("Abrams")],
        enemies: vec![id("Seven"), id("McGinnis"), id("Lady Geist")],
        lane: vec![id("Seven")],
        banned: vec![],
    };
    let r = draft::draft(&data, &d, &input);
    if std::env::args().any(|a| a == "--json") {
        println!("{}", serde_json::to_string(&r).unwrap());
        return;
    }
    println!("team {:?}", r.team_rate);
    for p in &r.picks {
        println!("{:16} {:+.2} team {:?} (base {:.1}%, {} m) own {:+.2} counter {:+.2} lane {:+.2}", p.name, p.edge, p.team_rate.map(|r| (r * 10.0).round() / 10.0), p.base_rate, p.matches, p.strength, p.counter, p.lane);
        for x in &p.reasons {
            println!("     {} {:+.2} ({} m)", x.label, x.points, x.matches);
        }
    }
    for b in &r.bans {
        println!("BAN {} {:+.2} share {:.1}%", b.name, b.edge, b.ban_share);
    }
    for t in &r.threats {
        println!("THREAT {} {:+.2} {:?}", t.name, t.edge, t.reasons.iter().map(|x| (&x.label, (x.points * 100.0).round() / 100.0)).collect::<Vec<_>>());
    }
    println!("{:?}", r.notes);
}
