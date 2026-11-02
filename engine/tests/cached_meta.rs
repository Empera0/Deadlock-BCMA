//! A match read from the file Valve serves (and Steam caches) gives the same analysis as the statistics service's
//! JSON of the same match. Match 124883102: its file as Valve serves it, and the service's JSON (zstd packed).
use std::io::Read;

use engine::{cached_meta, data::GameData, matches};

const FILE: &[u8] = include_bytes!("cached-meta/124883102_846075101.meta.bz2");

fn service_json() -> String {
    let packed = include_bytes!("cached-meta/124883102.service.json.zst");
    let mut out = String::new();
    ruzstd::decoding::StreamingDecoder::new(&packed[..]).unwrap().read_to_string(&mut out).unwrap();
    out
}

/// The file as Steam keeps it on the PC that opened the match: the header naming the request (the response's headers
/// blanked), then the body from byte 1106.
fn cache_file() -> Vec<u8> {
    let mut bytes = include_bytes!("cached-meta/124883102.cache-header.bin").to_vec();
    bytes.extend(FILE);
    bytes
}

#[test]
fn the_cached_file_names_its_match_and_server() {
    let entry = matches::cache_entry(&cache_file()[..512]).unwrap();
    assert_eq!((entry.match_id, entry.salt, entry.cluster, entry.demo), (124883102, 846075101, 429, false));
    assert_eq!(cached_meta::body_start(&cache_file()), Some(1106));
    assert_eq!(cached_meta::body_start(FILE), Some(0));
}

#[test]
fn the_file_reads_as_the_services_json() {
    let ours: serde_json::Value = serde_json::from_str(&cached_meta::metadata_json(&cache_file()).unwrap()).unwrap();
    let mut theirs: serde_json::Value = serde_json::from_str(&service_json()).unwrap();
    // The service adds what it learns elsewhere (bans, builds, picks before the match); the file does not hold it.
    for extra in ["banned_hero_ids", "hero_build_ids", "pregame_hero_ids"] {
        theirs.as_object_mut().unwrap().remove(extra);
    }
    assert_eq!(ours, theirs);
}

#[test]
fn the_analysis_is_the_same_from_either() {
    let data: GameData = serde_json::from_str(include_str!("../../src-tauri/data/snapshot.json")).unwrap();
    let ours = matches::parse(&data, &cached_meta::metadata_json(FILE).unwrap()).unwrap();
    let theirs = matches::parse(&data, &service_json()).unwrap();
    assert_eq!(serde_json::to_value(&ours).unwrap(), serde_json::to_value(&theirs).unwrap());
    assert_eq!(ours.players.len(), 12);
}

#[test]
fn other_files_are_refused() {
    assert!(cached_meta::metadata_json(b"not a match file").is_err());
    assert!(cached_meta::metadata_json(&FILE[..200]).is_err());
}

#[test]
fn older_files_packed_with_bzip2_read_too() {
    let file = include_bytes!("cached-meta/7_1.meta.bz2");
    let json: serde_json::Value = serde_json::from_str(&cached_meta::metadata_json(file).unwrap()).unwrap();
    assert_eq!(json["match_info"]["match_id"], 7);
    assert_eq!(json["match_info"]["duration_s"], 1500);
}
