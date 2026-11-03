use engine::data::GameData;
use engine::matches;
use engine::review;

fn data() -> GameData {
    serde_json::from_str(include_str!("../../src-tauri/data/snapshot.json")).unwrap()
}

const MATCH: &str = include_str!("sample-match.json");

#[test]
fn habits_add_up() {
    let report = matches::parse(&data(), MATCH).unwrap();
    for p in &report.players {
        let h = review::habits(MATCH, p.slot).unwrap();
        assert_eq!(h.phases.len(), 4);
        for phase in h.phases.iter().filter(|x| x.seconds > 0) {
            let time: f64 = phase.time.iter().sum();
            assert!((time - 1.0).abs() < 1e-6, "time shares add up to {time}");
            let heat: f64 = phase.heat.iter().sum();
            assert!((heat - 1.0).abs() < 1e-6, "heat adds up to {heat}");
            assert!((phase.base + phase.own_half + phase.enemy_half - 1.0).abs() < 1e-6);
        }
        let whole = &h.phases[3];
        assert_eq!(whole.seconds, report.duration_s);
        // The game's own average of unspent souls over the match.
        assert!(
            (whole.unspent.unwrap() - p.avg_unspent_souls).abs() < p.avg_unspent_souls * 0.1 + 50.0,
            "{:?} vs {}",
            whole.unspent,
            p.avg_unspent_souls
        );
        // Dead time is the time the report counts as dead.
        let dead = p
            .time_split
            .iter()
            .find(|s| s.name == "Dead")
            .map(|s| s.amount)
            .unwrap_or(0.0);
        assert!((whole.time[6] * report.duration_s as f64 - dead).abs() <= 1.0);
    }
    let me = &report.players[0];
    let h = review::habits(MATCH, me.slot).unwrap();
    println!(
        "{:?}",
        h.phases
            .iter()
            .map(|p| (
                p.seconds,
                p.time
                    .iter()
                    .map(|t| (t * 100.0).round())
                    .collect::<Vec<_>>(),
                (p.base * 100.0).round(),
                (p.own_half * 100.0).round(),
                p.unspent.map(f64::round)
            ))
            .collect::<Vec<_>>()
    );
}

#[test]
fn players_face_their_own_base_at_the_bottom() {
    let report = matches::parse(&data(), MATCH).unwrap();
    for p in &report.players {
        let laning = &review::habits(MATCH, p.slot).unwrap().phases[0];
        // Every player starts at home and spends the laning phase on their own side.
        assert!(
            laning.base + laning.own_half > laning.enemy_half,
            "slot {} team {}",
            p.slot,
            p.team
        );
    }
}

#[test]
fn averaging_keeps_shares() {
    let report = matches::parse(&data(), MATCH).unwrap();
    let all: Vec<_> = report
        .players
        .iter()
        .map(|p| review::habits(MATCH, p.slot).unwrap())
        .collect();
    let mean = review::average(&all);
    assert_eq!(mean.matches, all.len() as u32);
    let heat: f64 = mean.phases[1].heat.iter().sum();
    assert!((heat - 1.0).abs() < 1e-6);
}

#[test]
fn lane_minutes() {
    let report = matches::parse(&data(), MATCH).unwrap();
    let me = &report.players[0];
    let flow = review::lane_flow(&report, MATCH, me.slot).unwrap();
    assert_eq!(flow.minutes.len(), 10);
    assert_eq!(flow.opponents, me.lane_opponents);
    assert!(flow.marks.iter().any(|m| m.time == 180));
    let deaths: u32 = flow.minutes.iter().map(|m| m.deaths).sum();
    assert_eq!(
        deaths as usize,
        me.deaths_at.iter().filter(|d| d.time < 600).count()
    );
    assert!(flow.health[0].1.iter().all(|h| *h <= 100));
    let dealt: f64 = flow
        .marks
        .iter()
        .filter(|m| m.time <= 540)
        .map(|m| m.dealt)
        .sum();
    assert!(
        (dealt - me.lane_damage_dealt).abs() < 1.0,
        "{dealt} vs {}",
        me.lane_damage_dealt
    );
    println!("{:?}\n{:?}\n{:?}", flow.minutes, flow.marks, flow.turn);
}

#[test]
fn replay_frames_cover_the_match() {
    let report = matches::parse(&data(), MATCH).unwrap();
    assert_eq!(report.map.frames.len(), report.players.len());
    for f in &report.map.frames {
        assert_eq!(
            f.x.len(),
            report.duration_s.div_ceil(report.map.frame_step) as usize
        );
        assert!(f.x.iter().chain(&f.y).all(|v| *v <= 1000));
        assert!(
            f.hp.contains(&0)
                || report
                    .players
                    .iter()
                    .any(|p| p.slot == f.slot && p.deaths == 0)
        );
    }
}

#[test]
fn reference_picks_the_lane_first() {
    let bulk = r#"[
        {"match_id": 3, "players": [{"hero_id": 7, "player_slot": 2, "assigned_lane": 1, "abandon_match_time_s": 0}]},
        {"match_id": 2, "players": [{"hero_id": 7, "player_slot": 5, "assigned_lane": 4, "abandon_match_time_s": 0}]},
        {"match_id": 1, "players": [{"hero_id": 7, "player_slot": 9, "assigned_lane": 4, "abandon_match_time_s": 300}]},
        {"match_id": 0, "players": [{"hero_id": 8, "player_slot": 1, "assigned_lane": 4}]}
    ]"#;
    assert_eq!(
        review::reference_matches(bulk, 7, 4, 2),
        vec![(2, 5), (3, 2)]
    );
}

#[test]
fn ranks_from_the_service() {
    let mut report = matches::parse(&data(), MATCH).unwrap();
    let id = report.players[0].account_id;
    report.players[0].badge = 0;
    report.players[0].badge_from.clear();
    matches::apply_ranks(
        &mut report,
        &format!(r#"[{{"account_id": {id}, "badge": 104}}]"#),
    );
    assert_eq!(
        (
            report.players[0].badge,
            report.players[0].badge_from.as_str()
        ),
        (104, "now")
    );
}

#[test]
fn match_against_own_average() {
    let report = matches::parse(&data(), MATCH).unwrap();
    let me = &report.players[0];
    let hero = data().hero(&me.hero).unwrap().id;
    let stats = format!(
        r#"[{{"account_id": {}, "hero_id": {hero}, "matches_played": 20, "wins": 12, "kills_per_min": 0.3, "deaths_per_min": 0.2, "networth_per_min": 1100}}]"#,
        me.account_id
    );
    let v = review::versus_average(&report, me.slot, hero, &stats).unwrap();
    assert_eq!((v.matches, v.wins), (20, 12));
    let kills = v.rows.iter().find(|r| r.label == "Kills a minute").unwrap();
    assert!((kills.this - me.kills as f64 / (report.duration_s as f64 / 60.0)).abs() < 1e-9);
    assert!(review::versus_average(&report, me.slot, hero + 1000, &stats).is_none());
}

#[test]
fn ranks_recorded_in_the_match() {
    let mut root: serde_json::Value = serde_json::from_str(MATCH).unwrap();
    root["match_info"]["players"][0]["player_rank_data"] = serde_json::json!({ "initial_display_rank": 104 });
    let report = matches::parse(&data(), &root.to_string()).unwrap();
    let slot = root["match_info"]["players"][0]["player_slot"].as_u64().unwrap() as u32;
    let p = report.players.iter().find(|p| p.slot == slot).unwrap();
    assert_eq!((p.badge, p.badge_from.as_str()), (104, "match"));
    // A rank the match records is not replaced by the current one.
    let account = p.account_id;
    let mut report = report;
    matches::apply_ranks(&mut report, &format!(r#"[{{"account_id": {account}, "badge": 63}}]"#));
    assert_eq!(report.players.iter().find(|p| p.slot == slot).unwrap().badge, 104);
}

#[test]
fn three_lessons_at_most() {
    let report = matches::parse(&data(), MATCH).unwrap();
    for p in &report.players {
        let lessons = review::lessons(&report, p.slot);
        assert!(lessons.len() <= 3);
        assert!(lessons.windows(2).all(|w| w[0].points >= w[1].points));
        assert!(lessons.iter().all(|l| l.points >= 1.0 && !l.detail.is_empty()));
    }
    // The match's last player has something to learn.
    let last = report.ranking.last().unwrap().slot;
    let lessons = review::lessons(&report, last);
    assert!(!lessons.is_empty());
    for l in &lessons {
        println!("{} ({:.1}): {}", l.title, l.points, l.detail);
    }
}
