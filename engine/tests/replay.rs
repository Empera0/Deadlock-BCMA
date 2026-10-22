//! A match studied from its replay: Lash in match 119868134.

use engine::{data::GameData, replay};

fn study() -> replay::ReplayStudy {
    let data: GameData = serde_json::from_str(include_str!("../../src-tauri/data/snapshot.json")).unwrap();
    let hits = replay::decompress(include_bytes!("replay/hits.ndjson.zst")).unwrap();
    replay::study(&data, include_str!("replay/match.json"), 6, &hits, include_str!("replay/notes.ndjson"), include_str!("replay/kills.ndjson")).unwrap()
}

#[test]
fn every_death_shows_its_last_seconds_with_the_statuses_on_the_player() {
    let s = study();
    assert_eq!(s.deaths.len(), 11);
    for d in &s.deaths {
        assert!(!d.summary.is_empty());
        assert!(d.hits.iter().all(|m| m.at <= m.until && m.at >= -10.5 && m.hits > 0));
    }
    // At 9:33 Violet killed Lash alone, stunning him with Still Life a second before.
    let d = s.deaths.iter().find(|d| d.time == 573).unwrap();
    assert_eq!(d.killer, "Violet");
    assert!(d.by_hero.len() == 1 && d.by_hero[0].0 == "Violet");
    assert!(d.statuses.iter().any(|st| st.source == "Still Life" && st.statuses.contains(&"stunned".to_string())));
    // Gun and melee damage, which the data does not name, are told apart.
    assert!(d.hits.iter().any(|m| m.source == "Melee") && d.hits.iter().any(|m| m.source == "Gun"));
}

#[test]
fn each_ability_says_what_its_casts_caught_and_held() {
    let s = study();
    let slam = s.abilities.iter().find(|a| a.ability == "Death Slam").unwrap();
    assert_eq!(slam.casts, 5);
    assert!(slam.held_per_cast >= 1.0, "{}", slam.held_per_cast);
    assert!(slam.usual.is_some());
    // Flog has no lockdown.
    assert_eq!(s.abilities.iter().find(|a| a.ability == "Flog").unwrap().held_per_cast, 0.0);
}
