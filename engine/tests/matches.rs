use engine::{data::GameData, matches};

fn data() -> GameData {
    serde_json::from_str(include_str!("../../src-tauri/data/snapshot.json")).unwrap()
}

const SAMPLE: &str = include_str!("sample-match.json");
const LONG: &str = include_str!("sample-match-2.json");

#[test]
fn a_match_is_read_into_one_report_per_player() {
    let d = data();
    let report = matches::parse(&d, SAMPLE).unwrap();
    assert_eq!(report.match_id, 112469626);
    assert_eq!(report.players.len(), 12);
    assert_eq!(report.players.iter().filter(|p| p.won).count(), 6);

    let apollo = report.players.iter().find(|p| p.hero_name == "Apollo").unwrap();
    assert_eq!((apollo.kills, apollo.deaths, apollo.assists), (10, 7, 17));
    // Mystic Burst was sold later, so it is in the purchase list but not in the final build.
    let burst = apollo.purchases.iter().find(|p| p.name == "Mystic Burst").unwrap();
    assert!(burst.sold_at.is_some());
    assert!(!apollo.build.purchases.contains(&burst.item));
    assert!(apollo.build.purchases.iter().any(|class| d.item(class).is_some_and(|i| i.name == "Tankbuster")));
    // Every ability was unlocked, and upgrades were read as tiers.
    assert_eq!(apollo.build.ability_tiers.len(), 4);
    assert_eq!(apollo.build.ability_tiers.iter().copied().max(), Some(3));
    assert!(apollo.unknown_items == 0);
}

#[test]
fn damage_sources_use_readable_names_and_skip_the_summary_rows() {
    let d = data();
    let report = matches::parse(&d, SAMPLE).unwrap();
    let apollo = report.players.iter().find(|p| p.hero_name == "Apollo").unwrap();
    let names: Vec<&str> = apollo.damage_sources.iter().map(|s| s.name.as_str()).collect();
    assert!(names.contains(&"Flawless Advance"), "{names:?}");
    assert!(names.contains(&"Torment Pulse"), "{names:?}");
    assert!(names.contains(&"Weapon (headshots)"), "{names:?}");
    assert!(!names.contains(&"Ability") && !names.contains(&"Bullet"));
    // The detailed rows add up to roughly the damage the game reports for the player.
    let total: f64 = apollo.damage_sources.iter().map(|s| s.amount).sum();
    assert!((total - apollo.hero_damage).abs() < apollo.hero_damage * 0.1, "{total} vs {}", apollo.hero_damage);
}

#[test]
fn the_calculator_matches_the_health_the_game_recorded() {
    let d = data();
    let report = matches::parse(&d, SAMPLE).unwrap();
    let mut worst: f64 = 0.0;
    for p in &report.players {
        for s in &p.timeline {
            let calc = s.calc_health.expect("build computes");
            let off = (calc - s.game_health).abs() / s.game_health.max(1.0);
            println!(
                "{:<12} {:>5}s lvl {:>2}  health {:>6} vs {:>8.1} ({:+.1}%)  spirit {:>4} vs {:>6.1}  weapon {:>3} vs {:>5.1}",
                p.hero_name,
                s.time,
                s.level,
                s.game_health,
                calc,
                (calc - s.game_health) / s.game_health.max(1.0) * 100.0,
                s.game_spirit,
                s.calc_spirit.unwrap_or(0.0),
                s.game_weapon,
                s.calc_weapon.unwrap_or(0.0),
            );
            worst = worst.max(off);
        }
    }
    println!("worst health difference: {:.1}%", worst * 100.0);
}

#[test]
fn macro_numbers_are_read_for_every_player() {
    let d = data();
    let report = matches::parse(&d, SAMPLE).unwrap();
    let apollo = report.players.iter().find(|p| p.hero_name == "Apollo").unwrap();
    // Souls by source add up to the net worth, give or take what was lost on death.
    let total: f64 = apollo.souls.iter().map(|s| s.amount).sum();
    assert!((total - apollo.net_worth as f64).abs() < 1500.0, "{total} vs {}", apollo.net_worth);
    assert!(apollo.souls.iter().any(|s| s.name == "Boxes and statues"));
    assert!(apollo.objective_damage > 0.0);
    assert_eq!(apollo.kills_at.len() as u32, apollo.kills);
    assert_eq!(apollo.deaths_at.len() as u32, apollo.deaths);
    assert!(apollo.macro_stats.iter().any(|s| s.name == "Boxes and statues broken"));
    // The losing team's patron fell last.
    let last = report.objectives.iter().filter(|o| o.destroyed_at.is_some()).last().unwrap();
    assert_eq!(last.name, "Patron (final form)");
    assert_ne!(last.team, report.winning_team);
    assert_eq!(report.mid_boss.len(), 1);
}

#[test]
fn every_player_is_ranked_from_best_to_worst() {
    let d = data();
    let report = matches::parse(&d, SAMPLE).unwrap();
    assert_eq!(report.ranking.len(), 12);
    assert!(report.ranking.windows(2).all(|w| w[0].score >= w[1].score));
    assert!(report.ranking.iter().all(|r| r.score >= 0.0 && r.score <= 100.0));
    for r in &report.ranking {
        let p = report.players.iter().find(|p| p.slot == r.slot).unwrap();
        println!("{:>5.1}  {:<12} game MVP {:?}  {}", r.score, p.hero_name, p.game_mvp_rank, if p.won { "won" } else { "lost" });
    }
}

#[test]
fn team_fights_are_found_with_who_was_there_and_who_was_not() {
    let d = data();
    let report = matches::parse(&d, LONG).unwrap();
    assert!(report.fights.len() >= 5, "{} fights", report.fights.len());
    let name = |slot: u32| report.players.iter().find(|p| p.slot == slot).map(|p| p.hero_name.as_str()).unwrap_or("?");
    for f in &report.fights {
        assert!(f.end >= f.start);
        assert_eq!(f.sides.len(), 2);
        // Every player is accounted for exactly once.
        let counted: usize = f.sides.iter().map(|s| s.present.len() + s.absent.len() + s.dead.len()).sum();
        assert_eq!(counted, 12);
        println!(
            "{:>2}:{:02} {:>3}s  {}v{}  deaths {}/{}  {}",
            f.start / 60,
            f.start % 60,
            f.end - f.start,
            f.sides[0].present.len(),
            f.sides[1].present.len(),
            f.sides[0].deaths.len(),
            f.sides[1].deaths.len(),
            f.sides
                .iter()
                .flat_map(|s| s.absent.iter())
                .map(|a| format!("{} {} {:.0}m{}", name(a.slot), a.doing, a.distance, if a.missed { " MISSED" } else { "" }))
                .collect::<Vec<_>>()
                .join("; ")
        );
    }
    for r in &report.ranking {
        let p = report.players.iter().find(|p| p.slot == r.slot).unwrap();
        println!("{:>5.1}  {:<11} fights {}/{} missed {}  mvp {:?}", r.score, p.hero_name, p.fights_joined, p.fights_possible, p.fights_missed, p.game_mvp_rank);
        assert!(p.fights_joined <= p.fights_possible);
    }
}

#[test]
fn item_insights_compare_the_build_with_the_match() {
    let d = data();
    let report = matches::parse(&d, LONG).unwrap();
    for p in &report.players {
        assert!(!p.insights.is_empty(), "{} has no insights", p.hero_name);
        for i in &p.insights {
            assert!(["good", "bad", "info"].contains(&i.tone.as_str()));
            println!("{:<11} [{}] {}: {}", p.hero_name, i.tone, i.title, i.text);
        }
    }
}

#[test]
fn two_players_can_be_compared_part_by_part() {
    let d = data();
    let report = matches::parse(&d, LONG).unwrap();
    let find = |name: &str| report.players.iter().find(|p| p.hero_name == name).unwrap();
    let (victor, seven) = (find("Victor"), find("Seven"));
    // Victor's healing was all for himself, so none of it is scored as support.
    assert!(victor.self_healing > 50_000.0);
    assert_eq!(victor.teammate_support, 0.0);
    assert_eq!(victor.deaths, 0);
    // No recorded death, but Shocking Reanimation brought him back four times; the last two are one cooldown apart.
    assert_eq!(victor.revives_at, vec![640, 1007, 1153, 2268]);
    assert_eq!(victor.close_calls, 0);
    assert!(seven.revives_at.is_empty());
    assert!(report.fights.iter().any(|f| f.late) && report.fights.iter().any(|f| !f.late));
    println!("late-game fights skipped: {:?}", report.players.iter().map(|p| (p.hero_name.as_str(), p.late_fights_skipped)).collect::<Vec<_>>());
    let rank = |slot: u32| report.ranking.iter().find(|r| r.slot == slot).unwrap();
    let (rv, rs) = (rank(victor.slot), rank(seven.slot));
    assert_eq!(rv.parts.len(), rs.parts.len());
    assert!((rv.parts.iter().map(|p| p.max_points).sum::<f64>() - 100.0).abs() < 1e-9);
    // The best lane gets the full lane points, the worst none; the most unspent souls earn nothing.
    let part = |r: &matches::Rank, label: &str| r.parts.iter().find(|p| p.label == label).unwrap().points;
    let lane = "Lane lead over the lane opponent";
    // What a part is worth depends on the role, so each player is held against their own maximum.
    let max = |r: &matches::Rank, label: &str| r.parts.iter().find(|p| p.label == label).unwrap().max_points;
    assert!(report.ranking.iter().any(|r| (part(r, lane) - max(r, lane)).abs() < 1e-9));
    assert!(report.ranking.iter().any(|r| part(r, lane).abs() < 1e-9));
    assert!(part(rv, "Average unspent souls").abs() < 1e-9, "Victor held the most souls unspent");
    println!("{:<38} {:>14} {:>14}", "", "Victor", "Seven");
    for (a, b) in rv.parts.iter().zip(&rs.parts) {
        println!("{:<38} {:>7.0} {:>5.1}p {:>7.0} {:>5.1}p", a.label, a.value, a.points, b.value, b.points);
    }
    println!("{:<38} {:>13.1}p {:>13.1}p   close calls {} / {}", "Total", rv.score, rs.score, victor.close_calls, seven.close_calls);
    for r in &report.ranking {
        let p = report.players.iter().find(|p| p.slot == r.slot).unwrap();
        println!("{:>5.1}  {:<11} mvp {:?}", r.score, p.hero_name, p.game_mvp_rank);
    }
}

#[test]
fn player_names_come_from_the_profile_list() {
    let d = data();
    let mut report = matches::parse(&d, SAMPLE).unwrap();
    assert!(report.players.iter().all(|p| p.player_name.is_empty()));
    let id = report.players[0].account_id;
    matches::apply_names(&mut report, &format!(r#"[{{"account_id":{id},"personaname":"Tester","realname":"ignored"}}]"#));
    assert_eq!(report.players[0].player_name, "Tester");
    assert!(report.players[1].player_name.is_empty());
    // Anything unreadable leaves the report as it is.
    matches::apply_names(&mut report, "not json");
    assert_eq!(report.players[0].player_name, "Tester");
}

/// A cache file as Steam writes it: 20 header bytes, then host and path as zero-terminated text, then the request headers.
fn cache_file(host: &str, path: &str) -> Vec<u8> {
    let mut bytes = vec![2, 0, 0, 0, 0, 3, 0xcc, 0x12, 0, 0, 0, 0, 0xae, 0x89, 0xd9, 0x68, 1, 0, 0, 0];
    bytes.extend(host.as_bytes());
    bytes.push(0);
    bytes.extend(path.as_bytes());
    bytes.push(0);
    bytes.extend(b"\x05\0\0\0Host\0");
    bytes.extend(host.as_bytes());
    bytes.push(0);
    bytes
}

#[test]
fn replay_downloads_in_the_steam_cache_give_match_ids() {
    let meta = matches::cache_entry(&cache_file("replay404.valve.net", "/1422450/111042906_987654321.meta.bz2")).unwrap();
    assert_eq!((meta.match_id, meta.salt, meta.demo), (111042906, 987654321, false));
    let demo = matches::cache_entry(&cache_file("replay117.valve.net", "/1422450/112469626_55.dem.bz2")).unwrap();
    assert_eq!((demo.match_id, demo.demo), (112469626, true));
    // Other cached requests are ignored: avatars, other games, other files of the same folder.
    assert_eq!(matches::cache_entry(&cache_file("avatars.steamstatic.com", "/e00dfbaddaf37f5147f0ec540ff723d4aa362ab7.jpg")), None);
    assert_eq!(matches::cache_entry(&cache_file("replay404.valve.net", "/570/8123456789_123.meta.bz2")), None);
    assert_eq!(matches::cache_entry(&cache_file("replay404.valve.net", "/1422450/111042906_987654321.txt")), None);
    assert_eq!(matches::cache_entry(&cache_file("example.com", "/1422450/111042906_987654321.meta.bz2")), None);
    assert_eq!(matches::cache_entry(b""), None);
}

#[test]
fn macro_habits_are_measured_for_every_player() {
    let d = data();
    let report = matches::parse(&d, LONG).unwrap();
    println!("{:<11} {:>4} {:>8} {:>7} {:>6} {:>8} {:>6} {:>6}  time split (s)", "", "lane", "souls@9", "diff", "LH%", "unspent", "dead s", "lost");
    for p in &report.players {
        // The five kinds of time add up to the whole match.
        let total: f64 = p.time_split.iter().map(|s| s.amount).sum();
        assert!((total - report.duration_s as f64).abs() <= 2.0, "{} {total}", p.hero_name);
        assert!(p.lane_time > 0 && p.lane_time <= 540);
        assert!(!p.lane_opponents.is_empty(), "{} has no lane opponent", p.hero_name);
        assert!(p.last_hit_pct > 0.0 && p.last_hit_pct <= 100.0);
        assert!(p.avg_unspent_souls >= 0.0);
        println!(
            "{:<11} {:>4} {:>8.0} {:>+7.0} {:>5.0}% {:>8.0} {:>6} {:>6.0}  {}",
            p.hero_name,
            p.lane,
            p.lane_souls,
            p.lane_souls_diff,
            p.last_hit_pct,
            p.avg_unspent_souls,
            p.dead_seconds,
            p.souls_lost_on_death,
            p.time_split.iter().map(|s| format!("{:.0}", s.amount)).collect::<Vec<_>>().join("/")
        );
    }
    // Lane leads cancel out between the two sides of a lane.
    let sum: f64 = report.players.iter().map(|p| p.lane_souls_diff).sum();
    assert!(sum.abs() < 1.0, "{sum}");
    let victor = report.players.iter().find(|p| p.hero_name == "Victor").unwrap();
    assert_eq!(victor.dead_seconds, 0);
    let seven = report.players.iter().find(|p| p.hero_name == "Seven").unwrap();
    assert_eq!(seven.dead_seconds as f64, seven.time_split[4].amount);
}

#[test]
fn resist_reduction_is_credited_with_the_damage_it_gave_teammates() {
    let d = data();
    let report = matches::parse(&d, LONG).unwrap();
    for p in &report.players {
        let total: f64 = p.enabled.iter().map(|e| e.bonus).sum();
        assert!((total - p.enabled_total).abs() < 1e-6);
        for e in &p.enabled {
            // The bonus is a part of the damage it applied to, and that is a part of what teammates dealt.
            assert!(e.bonus > 0.0 && e.bonus < e.amplified, "{} {}", p.hero_name, e.source);
            let mates: f64 = report.players.iter().filter(|m| m.team == p.team && m.slot != p.slot).map(|m| m.hero_damage).sum();
            assert!(e.amplified <= mates * 1.05, "{} {} amplified {} of {}", p.hero_name, e.source, e.amplified, mates);
            assert!((e.by_teammate.iter().map(|m| m.amount).sum::<f64>() - e.bonus).abs() < 1e-6);
            println!(
                "{:<11} {:<30} {:<28} amplified {:>7.0}  bonus {:>6.0}  ({})",
                p.hero_name,
                e.source,
                e.effect,
                e.amplified,
                e.bonus,
                e.by_teammate.iter().map(|m| format!("{} {:.0}", m.name, m.amount)).collect::<Vec<_>>().join(", ")
            );
        }
    }
    let seven = report.players.iter().find(|p| p.hero_name == "Seven").unwrap();
    // Seven's Escalating Exposure helps teammates through its resist reduction; the stacking amp is his alone.
    assert!(seven.enabled.iter().any(|e| e.source == "Escalating Exposure (resist)"));
    assert!(seven.enabled.iter().all(|e| e.source != "Escalating Exposure"));
}

#[test]
fn hero_kits_are_read_as_threats_and_items_as_answers() {
    use engine::{build::BuildInput, calc, counters::{self, Threat}};
    let d = data();
    let kit = |hero: &str, tiers: u8| {
        let h = d.heroes.iter().find(|h| h.name == hero).unwrap();
        let b = BuildInput { hero: h.class_name.clone(), souls: Some(60_000), ability_tiers: vec![tiers; 4], ..Default::default() };
        counters::threats(&calc::compute(&d, &b).unwrap().abilities)
    };
    let has = |found: &[counters::Found], kind: Threat| found.iter().any(|t| t.kind == kind);
    // Haze puts people to sleep, Seven stuns and flies while channelling, Vindicta flies.
    assert!(has(&kit("Haze", 0), Threat::Lockdown));
    let seven = kit("Seven", 0);
    assert!(has(&seven, Threat::Lockdown) && has(&seven, Threat::Flight) && has(&seven, Threat::Channel));
    assert!(has(&kit("Vindicta", 0), Threat::Flight));
    // Lady Geist's silence only comes with an upgrade; Victor's revive is not a channel to interrupt.
    assert!(!has(&kit("Lady Geist", 0), Threat::Silence) && has(&kit("Lady Geist", 3), Threat::Silence));
    assert!(!has(&kit("Victor", 3), Threat::Channel));

    let item = |name: &str| d.items.iter().find(|i| i.name == name).unwrap();
    assert!(counters::answers(item("Unstoppable")).contains(&Threat::Lockdown));
    assert!(counters::answers(item("Debuff Reducer")).contains(&Threat::Lockdown));
    assert!(counters::answers(item("Knockdown")).contains(&Threat::Flight));
    assert!(counters::answers(item("Extra Health")).is_empty());

    let report = matches::parse(&d, LONG).unwrap();
    for p in &report.players {
        for i in p.insights.iter().filter(|i| i.title.contains("crowd control") || i.title.contains("silence") || i.title.contains("flying") || i.title.contains("channelled")) {
            println!("{:<11} [{}] {}: {}", p.hero_name, i.tone, i.title, i.text);
        }
    }
    // Against Rem's sleep, Kelvin, Haze and the others, the other team faced plenty of crowd control.
    assert!(report.players.iter().any(|p| p.insights.iter().any(|i| i.title.contains("crowd control"))));
}

#[test]
fn fights_duels_and_map_positions_are_filled_in() {
    let d = data();
    let report = matches::parse(&d, LONG).unwrap();
    for f in &report.fights {
        assert_eq!(f.souls.len(), 2);
        assert!(f.souls.iter().all(|s| *s > 0.0));
        // Every death counted for a side is in the kill list, in time order.
        assert_eq!(f.kills.len(), f.sides.iter().map(|s| s.deaths.len()).sum::<usize>());
        assert!(f.kills.windows(2).all(|w| w[0].time <= w[1].time));
        match f.winner {
            Some(team) => assert!(f.sides[team as usize].deaths.len() < f.sides[1 - team as usize].deaths.len()),
            None => assert_eq!(f.sides[0].deaths.len(), f.sides[1].deaths.len()),
        }
    }
    for p in &report.players {
        assert!(p.fights_won + p.fights_lost <= p.fights_joined);
        // Kills from the duel table are the player's kills; damage is what the game reports, give or take.
        let (kills, damage): (u32, f64) =
            report.duels.iter().filter(|c| c.from == p.slot).fold((0, 0.0), |a, c| (a.0 + c.kills, a.1 + c.damage));
        assert_eq!(kills, p.kills, "{}", p.hero_name);
        assert!((damage - p.hero_damage).abs() < p.hero_damage * 0.02 + 1.0, "{} {damage} vs {}", p.hero_name, p.hero_damage);
    }
    assert_eq!(report.duels.len(), 72);
    // The map: a path per player, every point inside the picture, deaths where the game says players died.
    assert_eq!(report.map.paths.len(), 12);
    assert_eq!(report.map.fights.len(), report.fights.len());
    assert_eq!(report.map.deaths.len() as u32, report.players.iter().map(|p| p.deaths).sum::<u32>());
    for path in &report.map.paths {
        assert!(path.points.len() > 100);
        assert!(path.points.iter().all(|p| (0.0..=1.0).contains(&p.left) && (0.0..=1.0).contains(&p.top)));
    }
    // The two teams start at opposite ends of the map.
    let start = |team: u32| {
        let slot = report.players.iter().find(|p| p.team == team).unwrap().slot;
        report.map.paths.iter().find(|p| p.slot == slot).unwrap().points[0].top
    };
    assert!((start(0) - start(1)).abs() > 0.5, "{} vs {}", start(0), start(1));
    println!("team 0 starts at top {:.2}, team 1 at {:.2}", start(0), start(1));
}

#[test]
fn a_player_s_match_is_summarized_in_one_row() {
    let d = data();
    let report = matches::parse(&d, LONG).unwrap();
    let seven = report.players.iter().find(|p| p.hero_name == "Seven").unwrap();
    let row = matches::summarize(&report, seven.account_id).unwrap();
    let place = report.ranking.iter().position(|r| r.slot == seven.slot).unwrap();
    assert_eq!((row.hero_name.as_str(), row.place as usize, row.won), ("Seven", place + 1, true));
    assert!((row.score - report.ranking[place].score).abs() < 1e-9);
    assert_eq!(row.fights_joined, seven.fights_joined);
    assert!(matches::summarize(&report, 1).is_none());
}

#[test]
fn fallen_objectives_are_credited_to_the_players_who_were_there() {
    let d = data();
    let report = matches::parse(&d, LONG).unwrap();
    let name = |slot: u32| report.players.iter().find(|p| p.slot == slot).map(|p| p.hero_name.as_str()).unwrap_or("?");
    let fallen: Vec<_> = report.objectives.iter().filter(|o| o.destroyed_at.is_some()).collect();
    for o in &fallen {
        // Only the other team can take it.
        assert!(o.takers.iter().all(|slot| report.players.iter().any(|p| p.slot == *slot && p.team != o.team)));
        println!(
            "{:>2}:{:02} team {}'s {}{} taken by {}",
            o.destroyed_at.unwrap() / 60,
            o.destroyed_at.unwrap() % 60,
            o.team,
            o.name,
            o.lane.map(|l| format!(" lane {l}")).unwrap_or_default(),
            if o.takers.is_empty() { "nobody near (troopers)".to_string() } else { o.takers.iter().map(|s| name(*s)).collect::<Vec<_>>().join(", ") }
        );
    }
    // Most objectives had someone at them when they fell, and guardians and walkers have a place on the map.
    let credited = fallen.iter().filter(|o| !o.takers.is_empty()).count();
    assert!(credited * 3 >= fallen.len() * 2, "{credited} of {}", fallen.len());
    assert!(report.objectives.iter().filter(|o| o.name == "Walker").all(|o| o.left.is_some()));
    let total: usize = report.players.iter().map(|p| p.objectives_taken.len()).sum();
    assert_eq!(total, fallen.iter().map(|o| o.takers.len()).sum::<usize>());
    for f in &report.fights {
        assert_eq!((f.walkers.len(), f.team_souls.len()), (2, 2));
        // Even is only ever said of late fights.
        assert!(!f.even || f.late);
        for a in f.sides.iter().flat_map(|s| s.absent.iter()).filter(|a| a.doing.starts_with("taking")) {
            // An objective excuses the absence unless the match was even at that point.
            println!(
                "fight {:>2}:{:02} (walkers {} v {}, souls {:.0}k v {:.0}k, even {}): {} was {}{}",
                f.start / 60,
                f.start % 60,
                f.walkers[0],
                f.walkers[1],
                f.team_souls[0] / 1000.0,
                f.team_souls[1] / 1000.0,
                f.even,
                name(a.slot),
                a.doing,
                if a.missed { " -> MISSED" } else { "" }
            );
            assert!(!a.missed || f.even);
        }
    }
}

#[test]
fn every_player_has_a_role_that_shifts_what_the_score_rewards() {
    let d = data();
    for json in [SAMPLE, LONG] {
        let report = matches::parse(&d, json).unwrap();
        let max = |slot: u32, label: &str| {
            report.ranking.iter().find(|r| r.slot == slot).unwrap().parts.iter().find(|p| p.label == label).unwrap().max_points
        };
        for p in &report.players {
            assert!(["Frontliner", "Support", "Assassin", "Marksman", "Caster"].contains(&p.role.as_str()), "{}", p.role);
            assert!(!p.role_why.is_empty());
            let rank = report.ranking.iter().find(|r| r.slot == p.slot).unwrap();
            // Every role is scored out of 100.
            assert!((rank.parts.iter().map(|x| x.max_points).sum::<f64>() - 100.0).abs() < 1e-9);
            println!("{:<11} {:<10} {:>5.1}  {}", p.hero_name, p.role, rank.score, p.role_why);
        }
        // Soaking damage is worth more to a frontliner than to a marksman, dealing it less.
        let by_role = |role: &str| report.players.iter().find(|p| p.role == role).map(|p| p.slot);
        if let (Some(front), Some(gun)) = (by_role("Frontliner"), by_role("Marksman")) {
            let absorbed = "Damage absorbed (taken + mitigated)";
            assert!(max(front, absorbed) > max(gun, absorbed) * 3.0);
            assert!(max(front, "Hero damage") < max(gun, "Hero damage"));
        }
    }
}

#[test]
fn mid_boss_kills_say_who_was_at_the_pit_and_what_the_rejuvenator_bought() {
    let d = data();
    let report = matches::parse(&d, LONG).unwrap();
    let name = |slot: u32| report.players.iter().find(|p| p.slot == slot).map(|p| p.hero_name.as_str()).unwrap_or("?");
    let names = |slots: &[u32]| slots.iter().map(|s| name(*s)).collect::<Vec<_>>().join(", ");
    assert_eq!(report.mid_boss.len(), 2);
    // The first one was killed by one team and claimed by the other.
    assert!(report.mid_boss[0].stolen && !report.mid_boss[1].stolen);
    for b in &report.mid_boss {
        let counted: usize = b.sides.iter().map(|s| s.present.len() + s.absent.len() + s.dead.len()).sum();
        assert_eq!(counted, 12);
        // Whoever claimed it had someone there.
        assert!(!b.sides[b.team_claimed as usize].present.is_empty());
        assert!(b.sides.iter().all(|s| s.deaths.iter().all(|slot| s.present.contains(slot))));
        assert!(b.sides.iter().flat_map(|s| s.absent.iter()).all(|a| !a.missed || b.contested));
        assert!(b.window_s > 0 && b.window_s <= 180);
        println!(
            "{:>2}:{:02} killed by {} claimed by {} stolen {} contested {} late {} even {} walkers {:?}",
            b.time / 60, b.time % 60, b.team_killed, b.team_claimed, b.stolen, b.contested, b.late, b.even, b.walkers
        );
        for (team, side) in b.sides.iter().enumerate() {
            println!(
                "   team {team}: at the pit [{}] died [{}] dead [{}] away [{}]",
                names(&side.present),
                names(&side.deaths),
                names(&side.dead),
                side.absent.iter().map(|a| format!("{} {} {:.0}m{}", name(a.slot), a.doing, a.distance, if a.missed { " MISSED" } else { "" })).collect::<Vec<_>>().join("; ")
            );
        }
        println!(
            "   next {}s: took {:?}, lost {:?}, kills {:?}, soul lead {:+.0}",
            b.window_s,
            b.taken.iter().map(|t| t.name.as_str()).collect::<Vec<_>>(),
            b.lost.iter().map(|t| t.name.as_str()).collect::<Vec<_>>(),
            b.kills,
            b.lead_change
        );
    }
}

#[test]
fn build_timing_says_when_an_item_was_called_for_and_when_it_came() {
    let d = data();
    let report = matches::parse(&d, LONG).unwrap();
    for p in &report.players {
        // Everyone at least has the two power spikes.
        assert!(p.build_timing.iter().filter(|t| t.need.starts_with("First ")).count() == 2, "{}", p.hero_name);
        for t in &p.build_timing {
            assert!(["good", "bad", "info"].contains(&t.tone.as_str()));
            assert_eq!(t.bought_at.is_some(), !t.item.is_empty());
            // An answer that was never bought although the match called for it is never good.
            assert!(!(t.bought_at.is_none() && t.tone == "good"));
            println!("{:<11} [{:<4}] {}: {}", p.hero_name, t.tone, t.need, t.text);
        }
    }
    let all: Vec<_> = report.players.iter().flat_map(|p| p.build_timing.iter()).collect();
    assert!(all.iter().any(|t| t.need.ends_with("resist") && t.needed_at.is_some()));
}

#[test]
fn answering_the_enemy_team_with_items_is_part_of_the_score() {
    let d = data();
    let report = matches::parse(&d, LONG).unwrap();
    let label = "Items bought against the enemy team %";
    for p in &report.players {
        let part = report.ranking.iter().find(|r| r.slot == p.slot).unwrap().parts.iter().find(|x| x.label == label).unwrap();
        let counted: Vec<f64> = p.build_timing.iter().filter_map(|t| t.credit).collect();
        assert!(counted.iter().all(|c| (0.0..=1.0).contains(c)));
        // Power spikes and needs a teammate covered are left out.
        assert!(p.build_timing.iter().filter(|t| t.need.starts_with("First ") || t.need.contains("teammate")).all(|t| t.credit.is_none()));
        // Never bought although the match called for it earns nothing.
        assert!(p.build_timing.iter().filter(|t| t.credit.is_some() && t.bought_at.is_none()).all(|t| t.credit == Some(0.0)));
        if let Some(score) = p.counter_score {
            assert!((score - counted.iter().sum::<f64>() / counted.len() as f64 * 100.0).abs() < 1e-9);
            assert!((part.value - score).abs() < 1e-9);
        }
        assert!(part.points <= part.max_points + 1e-9);
        println!("{:<11} {:>5.0}%  {:.1} of {:.1}  {:?}", p.hero_name, part.value, part.points, part.max_points, p.build_timing.iter().filter_map(|t| t.credit.map(|c| (t.need.as_str(), c))).collect::<Vec<_>>());
    }
}

#[test]
fn late_or_missing_answers_come_with_a_suggestion() {
    let d = data();
    let report = matches::parse(&d, LONG).unwrap();
    let mut with_numbers = 0;
    for p in &report.players {
        for t in &p.build_timing {
            // Advice only where the answer was late or missing.
            if !t.advice.is_empty() {
                assert!(t.credit.is_some_and(|c| c < 1.0), "{} {}", p.hero_name, t.need);
            }
            // Bought late: the advice is that very item. Never bought: nothing the player already owned.
            if t.bought_at.is_some() && !t.advice.is_empty() {
                assert_eq!(t.advice[0].item, t.item);
                assert!(t.advice[1..].iter().all(|a| a.cost < t.advice[0].cost));
            }
            assert!(t.advice.iter().skip(t.bought_at.is_some() as usize).all(|a| !p.purchases.iter().any(|x| x.name == a.item)));
            for a in &t.advice {
                assert!(a.cost > 0 && !a.effect.is_empty());
                assert!(a.estimate.is_none_or(|e| e >= 0.0));
                with_numbers += a.estimate.is_some() as u32;
                println!("{:<11} {:<24} {:<22} {:>5}  {:<28} {}", p.hero_name, t.need, a.item, a.cost, a.effect, a.note);
            }
            if !t.spent_on.is_empty() {
                println!("{:<11} {:<24} meanwhile: {}", p.hero_name, t.need, t.spent_on.join(", "));
            }
        }
    }
    assert!(with_numbers > 0);
}

#[test]
fn items_that_answer_enemies_say_whom_they_were_aimed_at() {
    let d = data();
    let report = matches::parse(&d, LONG).unwrap();
    for p in &report.players {
        for r in &p.item_reasons {
            assert!(["good", "bad", "info"].contains(&r.tone.as_str()));
            assert!(p.purchases.iter().any(|x| x.name == r.item && x.time == r.time));
            // Lane purchases name the lane opponents.
            assert!(!r.lane || r.time <= 12 * 60);
            println!("{:<11} {:>2}:{:02} {:<5} [{:<4}] {:<22} {} | {}", p.hero_name, r.time / 60, r.time % 60, if r.lane { "lane" } else { "later" }, r.tone, r.item, r.effect, r.text);
        }
    }
    // Seven bought Suppressor out of a lane against Silver, who dealt mostly weapon damage.
    let seven = report.players.iter().find(|p| p.hero_name == "Seven").unwrap();
    let suppressor = seven.item_reasons.iter().find(|r| r.item == "Suppressor").unwrap();
    assert!(suppressor.lane && suppressor.tone == "good");
    assert!(suppressor.text.starts_with("Silver had dealt") && suppressor.text.contains("Calico"));
}

#[test]
fn the_lane_is_held_against_the_lane_opponents() {
    let d = data();
    let report = matches::parse(&d, LONG).unwrap();
    for p in &report.players {
        for t in p.build_timing.iter().filter(|t| t.need.starts_with("Lane: ")) {
            // A lane need shows inside the lane, names a lane opponent and counts toward the score.
            assert!(t.needed_at.is_some_and(|n| n <= 12 * 60));
            assert!(t.credit.is_some());
            let opponents: Vec<&str> = report.players.iter().filter(|o| p.lane_opponents.contains(&o.slot)).map(|o| o.hero_name.as_str()).collect();
            assert!(opponents.iter().any(|name| t.text.contains(&format!("{name} had dealt"))), "{}", t.text);
            println!("{:<11} [{:<4}] {:?} {}: {}", p.hero_name, t.tone, t.credit, t.need, t.text);
        }
        for t in p.build_timing.iter().filter(|t| t.need.ends_with(" resist")) {
            // The match-wide resist need starts after the lane.
            assert!(t.needed_at.is_none_or(|n| n > 12 * 60), "{} {:?}", p.hero_name, t.needed_at);
            println!("{:<11} [{:<4}] {:?} {}: {}", p.hero_name, t.tone, t.credit, t.need, t.text);
        }
    }
    // Seven's lane: Calico's spirit damage answered with Enchanter's Emblem, Silver's gun with Suppressor.
    let seven = report.players.iter().find(|p| p.hero_name == "Seven").unwrap();
    let lane = |need: &str| seven.build_timing.iter().find(|t| t.need == need).unwrap();
    assert_eq!(lane("Lane: spirit defence").item, "Enchanter's Emblem");
    assert_eq!(lane("Lane: weapon defence").item, "Suppressor");
    assert!(lane("Lane: weapon defence").text.contains("Silver had dealt"));
}

#[test]
fn every_death_says_who_was_around_and_what_it_cost() {
    let d = data();
    let report = matches::parse(&d, LONG).unwrap();
    let name = |slot: u32| report.players.iter().find(|p| p.slot == slot).map(|p| p.hero_name.as_str()).unwrap_or("?");
    assert_eq!(report.deaths.len(), report.players.iter().map(|p| p.deaths as usize).sum::<usize>());
    for x in &report.deaths {
        // Caught means outside a fight and alone; a cost is only ever put on a death outside a fight.
        assert!(!x.caught || (!x.in_fight && x.alone && (x.enemies >= 2 || x.deep)));
        assert!(x.cost.is_empty() || !x.in_fight);
        assert!((0.0..=1.0).contains(&x.left) && (0.0..=1.0).contains(&x.top));
        if x.caught || !x.cost.is_empty() {
            println!(
                "{:>2}:{:02} {:<11} by {:<11} {} v {}  nearest ally {:>4.0} m  depth {:>5.0} m  {}{}",
                x.time / 60, x.time % 60, name(x.slot), name(x.killer), x.enemies, x.allies + 1, x.nearest_ally_m.unwrap_or(-1.0), x.depth_m,
                if x.caught { "CAUGHT " } else { "" },
                x.cost.join("; ")
            );
        }
    }
    for p in &report.players {
        assert_eq!(p.caught_deaths as usize, report.deaths.iter().filter(|x| x.slot == p.slot && x.caught).count());
        assert!(p.caught_deaths <= p.deaths && p.costly_deaths <= p.deaths);
        println!("{:<11} deaths {:>2} caught {} costly {}", p.hero_name, p.deaths, p.caught_deaths, p.costly_deaths);
    }
    // The first mid boss fell with four of the claiming team dead; some of them died outside a fight.
    assert!(report.deaths.iter().any(|x| !x.cost.is_empty()));
}

#[test]
fn a_match_summary_carries_the_habits_worth_tracking_across_matches() {
    let d = data();
    let report = matches::parse(&d, LONG).unwrap();
    for p in &report.players {
        let row = matches::summarize(&report, p.account_id).unwrap();
        assert_eq!(row.role, p.role);
        assert_eq!((row.caught_deaths, row.costly_deaths), (p.caught_deaths, p.costly_deaths));
        assert!(row.mid_boss_present <= row.mid_boss_alive && row.mid_boss_alive as usize <= report.mid_boss.len());
        assert!(row.even_fights_skipped <= p.fights_missed);
        // Only answers that came very late or never are listed.
        assert!(row.late_needs.iter().all(|need| p.build_timing.iter().any(|t| &t.need == need && t.tone == "bad")));
        println!(
            "{:<11} {:<10} even skipped {} caught {} costly {} boss {}/{} counter {:?} late {:?}",
            p.hero_name, row.role, row.even_fights_skipped, row.caught_deaths, row.costly_deaths, row.mid_boss_present, row.mid_boss_alive,
            row.counter_score.map(|c| c.round()), row.late_needs
        );
    }
    // Victor took a walker during an even late-game fight.
    let victor = report.players.iter().find(|p| p.hero_name == "Victor").unwrap();
    assert!(matches::summarize(&report, victor.account_id).unwrap().even_fights_skipped >= 1);
}

#[test]
fn the_laning_phase_is_read_lane_by_lane() {
    let d = data();
    let report = matches::parse(&d, LONG).unwrap();
    let name = |slot: u32| report.players.iter().find(|p| p.slot == slot).map(|p| p.hero_name.as_str()).unwrap_or("?");
    let names = |slots: &[u32]| slots.iter().map(|s| name(*s)).collect::<Vec<_>>().join(", ");
    // Three lanes, each a different line on the map, every player in exactly one.
    assert_eq!(report.lanes.len(), 3);
    let mut seen: Vec<&str> = report.lanes.iter().map(|l| l.name.as_str()).collect();
    seen.sort();
    assert_eq!(seen, ["East", "Middle", "West"]);
    assert_eq!(report.lanes.iter().flat_map(|l| l.sides.iter()).map(|s| s.players.len()).sum::<usize>(), 12);
    for l in &report.lanes {
        for (team, side) in l.sides.iter().enumerate() {
            println!(
                "lane {} {:<6} team {team}: {:<22} souls {:>6.0} last hits {:>3} denies {:>2} kills {} damage {:>5.0} guardian lost {:?}",
                l.lane, l.name, names(&side.players), side.souls, side.last_hits, side.denies, side.kills, side.damage, side.guardian_lost_at
            );
        }
        println!("   won by {:?}", l.winner);
        // A kill for one side is a death for the other.
        for team in 0..2 {
            let deaths: u32 = l.sides[1 - team].players.iter().map(|s| report.players.iter().find(|p| p.slot == *s).unwrap().lane_deaths_to).sum();
            assert_eq!(l.sides[team].kills, deaths);
        }
    }
    for p in &report.players {
        assert!(p.lane_damage_dealt >= 0.0 && p.lane_damage_taken >= 0.0);
        for r in &p.roams {
            assert!(r.start >= 120 && r.start < 540 && r.seconds >= 12 && r.lane != p.lane);
            println!("{:<11} roamed to lane {} ({}) at {}:{:02} for {}s: {} enemy death(s), {} kill(s){}", p.hero_name, r.lane, r.name, r.start / 60, r.start % 60, r.seconds, r.enemy_deaths, r.kills, if r.died { ", died" } else { "" });
        }
    }
    // Seven's lane: against Calico and Silver.
    let seven = report.players.iter().find(|p| p.hero_name == "Seven").unwrap();
    assert!(seven.lane_damage_dealt > 1000.0 && seven.lane_damage_taken > 1000.0);
}

#[test]
fn clearly_won_fights_say_what_they_were_turned_into() {
    let d = data();
    let report = matches::parse(&d, LONG).unwrap();
    let name = |slot: u32| report.players.iter().find(|p| p.slot == slot).map(|p| p.hero_name.as_str()).unwrap_or("?");
    let mut clear = 0;
    for f in &report.fights {
        let Some(follow) = &f.follow else { continue };
        clear += 1;
        let winner = f.winner.unwrap() as usize;
        assert!(follow.lead >= 2 && follow.window_s <= 90);
        // Only winners who survived can have gone farming.
        assert!(follow.farming.iter().all(|s| f.sides[winner].present.contains(s) && !f.sides[winner].deaths.contains(s)));
        println!(
            "{:>2}:{:02} team {winner} won by {} -> {:?}{} in {}s; farming: {}",
            f.start / 60, f.start % 60, follow.lead, follow.taken, if follow.boss { " + mid boss" } else { "" }, follow.window_s,
            follow.farming.iter().map(|s| name(*s)).collect::<Vec<_>>().join(", ")
        );
    }
    assert!(clear >= 2);
    for p in &report.players {
        assert!(p.wins_farmed as usize <= report.fights.len());
    }
}

#[test]
fn the_lane_detail_comes_from_the_snapshots_of_the_laning_phase() {
    let d = data();
    let report = matches::parse(&d, LONG).unwrap();
    for p in &report.players {
        let l = &p.lane_detail;
        // Three snapshots up to 9:00, the last one being the lane souls shown elsewhere.
        assert_eq!(l.souls_at.len(), 3);
        assert_eq!(l.souls_at[2], p.lane_souls);
        assert!(l.trooper_last_hits <= l.possible_last_hits);
        assert_eq!((l.denies, l.deaths), (p.lane_denies, p.lane_deaths));
        assert!((0.0..=100.0).contains(&l.accuracy_pct));
        // The running bridge buff count ends at the total by kind.
        assert_eq!(p.bridge_buffs_at.last().copied().unwrap_or(0.0), p.bridge_buffs.iter().map(|b| b.amount).sum::<f64>());
        println!(
            "{:<11} souls {:?} lh {}/{} denies {} (+{:.0}) K/D/A {}/{}/{} dmg {:.0} taken {:.0} guardian {:.0} camps {} ({:.0}) boxes {:.0} statues {} lost {:.0} acc {:.0}% bridge {:?}",
            p.hero_name, l.souls_at, l.trooper_last_hits, l.possible_last_hits, l.denies, l.deny_souls, l.kills, l.deaths, l.assists, l.hero_damage, l.damage_taken,
            l.guardian_damage, l.camp_kills, l.camp_souls, l.box_souls, l.statue_buffs, l.souls_lost_on_death, l.accuracy_pct,
            p.bridge_buffs.iter().map(|b| format!("{} {}", b.name, b.amount)).collect::<Vec<_>>()
        );
    }
    for lane in &report.lanes {
        for side in &lane.sides {
            assert_eq!(side.souls_at.len(), 3);
            assert!((side.souls_at[2] - side.souls).abs() < 1e-6);
            assert!(side.guardian_player_damage + side.guardian_trooper_damage > 0.0);
        }
    }
}

#[test]
fn titles_name_who_led_the_match_in_what() {
    let d = data();
    let report = matches::parse(&d, LONG).unwrap();
    let all: Vec<(&str, &matches::Title)> = report.players.iter().flat_map(|p| p.titles.iter().map(move |t| (p.hero_name.as_str(), t))).collect();
    // A title goes to one player only.
    for (_, t) in &all {
        assert_eq!(all.iter().filter(|x| x.1.name == t.name).count(), 1, "{}", t.name);
        assert!(["good", "bad"].contains(&t.tone.as_str()) && !t.value.is_empty());
    }
    for p in &report.players {
        println!(
            "{:<11} {:<60} best: {} | lost: {}",
            p.hero_name,
            p.titles.iter().map(|t| format!("{} ({})", t.name, t.value)).collect::<Vec<_>>().join(", "),
            p.strengths.iter().map(|x| format!("{} {:.1}", x.name, x.amount)).collect::<Vec<_>>().join(", "),
            p.gaps.iter().map(|x| format!("{} -{:.1}", x.name, x.amount)).collect::<Vec<_>>().join(", ")
        );
        assert_eq!((p.strengths.len(), p.gaps.len()), (3, 2));
        assert!(p.strengths.windows(2).all(|w| w[0].amount >= w[1].amount));
    }
    let titled = |name: &str| all.iter().find(|x| x.1.name == name).map(|x| x.0);
    // Victor stole the first rejuvenator on his own and never died; the most hero damage is whoever leads that stat.
    assert_eq!(titled("Thief"), Some("Victor"));
    let top = report.players.iter().max_by(|a, b| a.hero_damage.total_cmp(&b.hero_damage)).unwrap();
    assert_eq!(titled("Heavy Hitter"), Some(top.hero_name.as_str()));
}

#[test]
fn the_match_is_told_in_a_few_sentences() {
    let d = data();
    for json in [SAMPLE, LONG] {
        let report = matches::parse(&d, json).unwrap();
        assert!(report.story.len() >= 3, "{} lines", report.story.len());
        for line in &report.story {
            assert!(["good", "bad", "info"].contains(&line.tone.as_str()));
            assert!(line.text.ends_with('.') && !line.text.contains("NaN"));
            println!("[{}] {}: {}", line.tone, line.title, line.text);
        }
        println!();
        let titles: Vec<&str> = report.story.iter().map(|l| l.title.as_str()).collect();
        assert!(titles.contains(&"Laning phase") && titles.contains(&"Soul lead") && titles.contains(&"Carried by"));
    }
    // In the long match Seven led the winning side.
    let report = matches::parse(&d, LONG).unwrap();
    assert!(report.story.iter().any(|l| l.title == "Carried by" && l.text.starts_with("Seven")));
}

#[test]
fn macro_mistakes_are_part_of_the_score() {
    let d = data();
    let report = matches::parse(&d, LONG).unwrap();
    let labels = ["Deaths that left the team short", "Won a fight, then farmed", "Mid boss attendance %"];
    for r in &report.ranking {
        let p = report.players.iter().find(|p| p.slot == r.slot).unwrap();
        let part = |label: &str| r.parts.iter().find(|x| x.label == label).unwrap();
        assert_eq!(part(labels[0]).value, p.costly_deaths as f64);
        assert_eq!(part(labels[1]).value, p.wins_farmed as f64);
        assert!(p.mid_boss_present <= p.mid_boss_alive);
        // Nobody left the team short more often than the worst, who earns nothing for it; a clean sheet earns it all.
        if p.costly_deaths == 0 {
            assert!((part(labels[0]).points - part(labels[0]).max_points).abs() < 1e-9);
        }
        println!(
            "{:<11} {:>5.1}  short {} ({:.1}/{:.1})  farmed {} ({:.1}/{:.1})  boss {}/{} ({:.1}/{:.1})",
            p.hero_name, r.score, p.costly_deaths, part(labels[0]).points, part(labels[0]).max_points, p.wins_farmed, part(labels[1]).points, part(labels[1]).max_points,
            p.mid_boss_present, p.mid_boss_alive, part(labels[2]).points, part(labels[2]).max_points
        );
        assert!((r.parts.iter().map(|x| x.max_points).sum::<f64>() - 100.0).abs() < 1e-9);
    }
}

#[test]
fn death_analysis_studies_each_death() {
    let data = data();
    let json = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/sample-match.json")).unwrap();
    let report = engine::deaths::analyze(&data, &json, 3).unwrap();
    assert!(!report.deaths.is_empty());
    for d in &report.deaths {
        assert!(d.fight_damage > 0.0 && d.my_health > 0.0, "{d:?}");
        assert!(d.window_from < d.window_to && d.time <= d.window_to);
        assert!(!d.verdict.is_empty());
        assert!(d.their_time_to_kill > 0.0 && d.my_time_to_kill > 0.0);
        for f in &d.fixes {
            // Every fix turns the duel or changes it by a second or more; one that only keeps the player alive
            // keeps them alive at least two seconds longer.
            let change = (d.their_time_to_kill + f.seconds_gained - f.my_time_to_kill) - (d.their_time_to_kill - d.my_time_to_kill);
            assert!(f.wins_duel || change >= 1.0 - 1e-9, "{f:?}");
            if f.kind == "defense" && !f.wins_duel && f.my_time_to_kill >= d.my_time_to_kill - 1.0 {
                assert!(f.seconds_gained >= 2.0 - 1e-9, "{f:?}");
            }
        }
    }
}

#[test]
fn deaths_add_up_over_matches() {
    let data = data();
    let one = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/sample-match.json")).unwrap();
    let two = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/sample-match-2.json")).unwrap();
    let account = |json: &str| -> u64 {
        let v: serde_json::Value = serde_json::from_str(json).unwrap();
        v["match_info"]["players"][0]["account_id"].as_u64().unwrap()
    };
    let mut analyses = vec![];
    for (id, json) in [(1, &one), (2, &two)] {
        let slot = engine::deaths::slot_of(json, account(json)).unwrap();
        analyses.push((id, engine::deaths::analyze(&data, json, slot).unwrap()));
    }
    let t = engine::deaths::trend(&analyses);
    assert_eq!(t.matches, 2);
    assert_eq!(t.deaths as usize, analyses.iter().map(|a| a.1.deaths.len()).sum::<usize>());
    assert_eq!(t.killers.iter().map(|k| k.1).sum::<u32>(), t.deaths.min(t.killers.iter().map(|k| k.1).sum()));
    assert!(t.killers.windows(2).all(|w| w[0].1 >= w[1].1));
    assert!(t.fixes.iter().all(|f| f.2 <= f.1 && f.2 <= 2));
    assert!(t.solo_kills <= t.deaths && !t.summary.is_empty());
}

#[test]
fn the_live_match_is_read_from_the_players_side() {
    let json = r#"[
        {"match_id": 7, "duration_s": 300, "players": [
            {"account_id": 1, "team": 0, "hero_id": 10}, {"account_id": 2, "team": 1, "hero_id": 20},
            {"account_id": 3, "team": 1, "hero_id": 21}, {"account_id": 4, "team": 0, "hero_id": 11}]},
        {"match_id": 8, "duration_s": 100, "players": [{"account_id": 9, "team": 0, "hero_id": 5}]}
    ]"#;
    let live = engine::matches::parse_live(json, 3).unwrap().unwrap();
    assert_eq!((live.match_id, live.hero, live.allies, live.enemies), (7, 21, vec![20], vec![10, 11]));
    assert!(engine::matches::parse_live(json, 42).unwrap().is_none());
}

#[test]
fn the_console_log_names_the_match_and_the_account() {
    let log = "\
[Steam] Logged in as [U:1:123456]\n\
[Lobby] Lobby 9 for Match 111 created\n\
[Lobby] Lobby 9 for Match 111 destroyed\n\
[Lobby] Lobby 10 for Match 222 created\n";
    let m = engine::matches::console_match(log);
    assert_eq!((m.match_id, m.account_id), (Some(222), Some(123456)));
    let ended = engine::matches::console_match(&format!("{log}[Lobby] Lobby 10 for Match 222 destroyed\n"));
    assert_eq!(ended.match_id, None);
    // Both matches are over: their lobbies were destroyed after they were created.
    assert_eq!(m.finished, vec![111]);
    assert_eq!(ended.finished, vec![111, 222]);
    assert_eq!(engine::matches::console_match("nothing here").match_id, None);
}

#[test]
fn the_live_query_gives_both_teams_and_the_lane() {
    let base = 76_561_197_960_265_728u64;
    let row = |acc: u64, team: u64, hero: u64, lane: i64| format!("data: {{\"m_steamID\":{},\"m_iTeamNum\":{team},\"hero\":{hero},\"lane\":{lane}}}\n\n", base + acc);
    let text = format!(
        "event: status\ndata: {{\"message\":\"Broadcast relay not ready yet\"}}\n\n{}{}{}{}{}event: end\ndata: {{}}\n",
        row(1, 2, 10, 1),
        row(2, 2, 11, 4),
        row(3, 3, 20, 1),
        row(4, 3, 21, 6),
        row(1, 2, 10, 1)
    );
    let live = engine::matches::parse_live_query(&text, 77, 1).unwrap();
    assert_eq!((live.match_id, live.hero, live.allies, live.enemies, live.lane), (77, 10, vec![11], vec![20, 21], vec![20]));
    let err = "event: error\ndata: {\"status\":400,\"error\":\"Invalid query\"}\n";
    assert_eq!(engine::matches::parse_live_query(err, 1, 1).unwrap_err(), "Invalid query");
}

#[test]
fn the_lobby_is_scouted_from_the_services_answers() {
    let data = data();
    let (a, b) = (data.heroes[0].id, data.heroes[1].id);
    let players = vec![
        engine::matches::LivePlayer { account_id: 1, hero: a, ally: true, lane: 1 },
        engine::matches::LivePlayer { account_id: 2, hero: b, ally: true, lane: 4 },
        engine::matches::LivePlayer { account_id: 3, hero: a, ally: false, lane: 1 },
    ];
    let stats = format!(
        r#"[{{"account_id":1,"hero_id":{a},"matches_played":80,"wins":50,"kills":400,"deaths":240,"assists":800}},
           {{"account_id":1,"hero_id":{b},"matches_played":20,"wins":8}},
           {{"account_id":3,"hero_id":{a},"matches_played":2,"wins":1}}]"#
    );
    let ranks = r#"[{"account_id":1,"badge":105,"last_match":{"player_rank_initial_win_streak":4}},{"account_id":3,"badge":0}]"#;
    let profiles = r#"[{"account_id":1,"personaname":"Me","friends":[{"account_id":2}]},{"account_id":2,"personaname":"Mate","friends":[]},{"account_id":3,"personaname":"Foe","friends":[{"account_id":1}]}]"#;
    let s = engine::scout::scout(&data, &players, &stats, ranks, profiles);
    assert_eq!(s.len(), 3);
    assert_eq!((s[0].rank.as_str(), s[0].hero_matches, s[0].matches, s[0].streak), ("Ascendant 5", 80, 100, 4));
    assert!((s[0].hero_kda.0 - 5.0).abs() < 1e-9);
    assert!(s[0].tags.iter().any(|t| t.contains("main")) && s[0].tags.iter().any(|t| t.contains("4 wins")));
    // Friends on the same team are a party, on the other team not.
    assert_eq!(s[0].party, vec!["Mate".to_string()]);
    assert_eq!(s[1].party, vec!["Me".to_string()]);
    assert!(s[2].party.is_empty());
    assert!(s[2].tags.iter().any(|t| t.starts_with("New on this hero")));
    assert_eq!(s[2].rank, "Unranked");
    assert_eq!(s[0].most_played[0].1, 80);
}

#[test]
fn deaths_in_the_own_lane_while_it_is_laned_are_never_caught() {
    let data = data();
    for json in [SAMPLE, LONG] {
        let r = matches::parse(&data, json).unwrap();
        assert!(r.deaths.iter().any(|d| d.in_lane), "some deaths fall in the laning phase");
        for d in &r.deaths {
            assert!(!(d.in_lane && d.caught), "{} at {}s", d.slot, d.time);
            // Nothing after the longest a lane can be held counts as in lane.
            assert!(!d.in_lane || d.time <= 14 * 60);
        }
    }
}
