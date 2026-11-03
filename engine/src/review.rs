//! A player's habits in a match, read from the positions and health the game records every second: where on the
//! map they spent each part of the match, what their time went to, how the laning phase went minute by minute,
//! and how the match compares with their own average on the hero.
//!
//! Positions are turned so the player's own base is at the bottom, so a player of either team can be laid over
//! another: the map is a mirror image from top to bottom.

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::fights::{self, Track};
use crate::matches::{damage_over_time, int, list, num, on_map, MatchReport};

/// The parts of a match: the laning phase, the mid game and the late game, as (from, to) seconds.
pub const PHASES: [(u32, u32); 3] = [(0, 600), (600, 1200), (1200, u32::MAX)];
/// Cells on each side of the heat map.
pub const GRID: usize = 24;
/// Faster than this (world units a second) out of combat is moving somewhere; slower is waiting.
const MOVING: f64 = 120.0;
/// Past this share of the minimap from the top (with the own base at the bottom), the player is in the own base.
const BASE_FROM: f64 = 0.86;
/// What the time went to, in the order of `PhaseHabits::time`.
pub const KINDS: [&str; 7] = [
    "Fighting heroes",
    "Troopers and objectives",
    "Neutral camps",
    "Moving",
    "In base",
    "Waiting",
    "Dead",
];
/// The laning phase looked at minute by minute.
const LANE_S: u32 = 600;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PhaseHabits {
    /// Seconds of the match in this part; 0 when the match ended before it.
    pub seconds: u32,
    /// Share of the seconds per kind of time (`KINDS`), 0-1.
    pub time: Vec<f64>,
    /// Share of the seconds alive per cell of the minimap, `GRID` × `GRID` from the top left, own base at the bottom.
    pub heat: Vec<f64>,
    /// Shares of the seconds alive in the own base, the rest of the own half, and the enemy's half.
    pub base: f64,
    pub own_half: f64,
    pub enemy_half: f64,
    /// Souls carried unspent, on average, from the game's own counter; `None` for matches from game versions
    /// that no longer count them.
    pub unspent: Option<f64>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Habits {
    /// One per `PHASES`, then the whole match.
    pub phases: Vec<PhaseHabits>,
    /// Matches averaged: 1 for a player's own match.
    pub matches: u32,
}

/// Unspent souls carried over the match so far at each snapshot, in soul-minutes: the game counts them.
fn unspent_curve(info: &Value, player: &Value) -> Vec<(u32, f64)> {
    let id = list(info, "custom_user_stats")
        .iter()
        .find(|s| s.get("name").and_then(Value::as_str) == Some("Unspent Gold Minutes"))
        .map(|s| int(s, "id"));
    let Some(id) = id else { return vec![] };
    let mut out = vec![(0, 0.0)];
    for s in list(player, "stats") {
        let value = list(s, "custom_user_stats")
            .iter()
            .find(|c| int(c, "id") == id)
            .map(|c| num(c, "value"))
            .unwrap_or(0.0);
        out.push((int(s, "time_stamp_s"), value));
    }
    out
}

/// The curve's value at a time, read along the straight line between snapshots.
fn along(curve: &[(u32, f64)], time: u32) -> f64 {
    match curve.iter().position(|p| p.0 >= time) {
        Some(0) => curve[0].1,
        Some(i) => {
            let (a, b) = (curve[i - 1], curve[i]);
            a.1 + (b.1 - a.1) * (time - a.0) as f64 / (b.0 - a.0).max(1) as f64
        }
        None => curve.last().map(|p| p.1).unwrap_or(0.0),
    }
}

fn phase(track: &Track, team: u32, from: u32, to: u32, curve: &[(u32, f64)]) -> PhaseHabits {
    let to = to.min(track.len() as u32);
    if from >= to {
        return PhaseHabits::default();
    }
    let mut time = [0u32; 7];
    let mut heat = vec![0.0; GRID * GRID];
    let (mut alive, mut base, mut own) = (0u32, 0u32, 0u32);
    for t in from..to {
        if !track.alive_at(t) {
            time[6] += 1;
            continue;
        }
        let (x, y) = track.place(t);
        let (left, top) = on_map(x, y);
        let top = if team == 1 { 1.0 - top } else { top };
        alive += 1;
        let cell = ((top * GRID as f64) as usize).min(GRID - 1) * GRID
            + ((left * GRID as f64) as usize).min(GRID - 1);
        heat[cell] += 1.0;
        let in_base = top >= BASE_FROM;
        base += in_base as u32;
        own += (top >= 0.5 && !in_base) as u32;
        let kind = match track.combat_at(t) {
            1 => 0,
            2 => 1,
            3 => 2,
            _ if in_base => 4,
            _ => {
                let (px, py) = track.place(t.saturating_sub(1));
                if ((x - px).powi(2) + (y - py).powi(2)).sqrt() > MOVING {
                    3
                } else {
                    5
                }
            }
        };
        time[kind] += 1;
    }
    let seconds = to - from;
    let share = |n: u32, of: u32| n as f64 / of.max(1) as f64;
    PhaseHabits {
        seconds,
        time: time.iter().map(|n| share(*n, seconds)).collect(),
        heat: heat.iter().map(|n| n / alive.max(1) as f64).collect(),
        base: share(base, alive),
        own_half: share(own, alive),
        enemy_half: share(alive - base - own, alive),
        unspent: (curve.len() > 1).then(|| (along(curve, to) - along(curve, from)) / (seconds as f64 / 60.0)),
    }
}

/// A player's habits in a match, from the match's metadata (deadlock-api JSON).
pub fn habits(json: &str, slot: u32) -> Result<Habits, String> {
    let root: Value =
        serde_json::from_str(json).map_err(|e| format!("Unreadable match data: {e}"))?;
    let info = root
        .get("match_info")
        .ok_or("The response has no match information")?;
    habits_of(info, slot).ok_or_else(|| "The match has no positions for this player.".into())
}

fn habits_of(info: &Value, slot: u32) -> Option<Habits> {
    let players = list(info, "players");
    let player = players.iter().find(|p| int(p, "player_slot") == slot)?;
    let team = int(player, "team");
    let tracks = fights::tracks(info, |s| {
        players
            .iter()
            .find(|p| int(p, "player_slot") == s)
            .map(|p| int(p, "team"))
    });
    let track = tracks.iter().find(|t| t.slot == slot)?;
    let duration = int(info, "duration_s");
    let curve = unspent_curve(info, player);
    let mut phases: Vec<PhaseHabits> = PHASES
        .iter()
        .map(|(from, to)| phase(track, team, *from, (*to).min(duration), &curve))
        .collect();
    phases.push(phase(track, team, 0, duration, &curve));
    Some(Habits { phases, matches: 1 })
}

/// Several players' habits averaged: each part over the matches that lasted into it.
pub fn average(all: &[Habits]) -> Habits {
    let parts = all.iter().map(|h| h.phases.len()).max().unwrap_or(0);
    let phases = (0..parts)
        .map(|i| {
            let present: Vec<&PhaseHabits> = all
                .iter()
                .filter_map(|h| h.phases.get(i))
                .filter(|p| p.seconds > 0)
                .collect();
            let n = present.len().max(1) as f64;
            let mean =
                |f: &dyn Fn(&PhaseHabits) -> f64| present.iter().map(|p| f(p)).sum::<f64>() / n;
            let mean_of = |f: &dyn Fn(&PhaseHabits) -> &Vec<f64>, len: usize| {
                (0..len)
                    .map(|k| {
                        present
                            .iter()
                            .map(|p| f(p).get(k).copied().unwrap_or(0.0))
                            .sum::<f64>()
                            / n
                    })
                    .collect()
            };
            if present.is_empty() {
                return PhaseHabits::default();
            }
            PhaseHabits {
                seconds: mean(&|p| p.seconds as f64) as u32,
                time: mean_of(&|p| &p.time, KINDS.len()),
                heat: mean_of(&|p| &p.heat, GRID * GRID),
                base: mean(&|p| p.base),
                own_half: mean(&|p| p.own_half),
                enemy_half: mean(&|p| p.enemy_half),
                unspent: {
                    let known: Vec<f64> = present.iter().filter_map(|p| p.unspent).collect();
                    (!known.is_empty()).then(|| known.iter().sum::<f64>() / known.len() as f64)
                },
            }
        })
        .collect();
    Habits {
        phases,
        matches: all.len() as u32,
    }
}

/// Matches of top players on a hero, from the bulk metadata list (deadlock-api `/v1/matches/metadata` with player
/// info): (match id, the hero's slot), the ones whose player was assigned `lane` first.
pub fn reference_matches(
    bulk_json: &str,
    hero_id: u64,
    lane: u32,
    count: usize,
) -> Vec<(u64, u32)> {
    let Ok(Value::Array(rows)) = serde_json::from_str::<Value>(bulk_json) else {
        return vec![];
    };
    let mut found: Vec<(u64, u32, bool)> = rows
        .iter()
        .filter_map(|m| {
            let p = list(m, "players")
                .iter()
                .find(|p| p.get("hero_id").and_then(Value::as_u64) == Some(hero_id))?;
            if p.get("abandon_match_time_s")
                .and_then(Value::as_u64)
                .unwrap_or(0)
                > 0
            {
                return None;
            }
            Some((
                m.get("match_id")?.as_u64()?,
                int(p, "player_slot"),
                int(p, "assigned_lane") == lane,
            ))
        })
        .collect();
    // Stable: the newest of each kind stay first.
    found.sort_by_key(|f| !f.2);
    found.into_iter().take(count).map(|f| (f.0, f.1)).collect()
}

/// The lane a player was assigned, from a match's metadata.
pub fn assigned_lane(json: &str, slot: u32) -> u32 {
    let root: Value = serde_json::from_str(json).unwrap_or(Value::Null);
    let players = root
        .pointer("/match_info/players")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    players
        .iter()
        .find(|p| int(p, "player_slot") == slot)
        .map(|p| int(p, "assigned_lane"))
        .unwrap_or(0)
}

#[derive(Debug, Clone, Serialize)]
pub struct LaneMinute {
    pub minute: u32,
    /// Health lost in the minute, in percent of the bar summed (a death counts what was left): the player's,
    /// and the lane opponents' on average.
    pub lost: f64,
    pub their_lost: f64,
    /// Seconds spent fighting heroes.
    pub fighting: u32,
    pub kills: u32,
    pub deaths: u32,
}

/// The game's snapshot of the lane at a moment.
#[derive(Debug, Clone, Serialize)]
pub struct LaneMark {
    pub time: u32,
    pub souls: f64,
    /// The lane opponents' on average.
    pub their_souls: f64,
    pub last_hits: f64,
    pub their_last_hits: f64,
    /// Hero damage dealt to the lane opponents and taken from them since the previous snapshot.
    pub dealt: f64,
    pub taken: f64,
}

#[derive(Debug, Clone, Serialize)]
pub struct LaneFlow {
    pub opponents: Vec<u32>,
    pub minutes: Vec<LaneMinute>,
    pub marks: Vec<LaneMark>,
    /// Health every 5 seconds: the player's first, then each opponent's, as (slot, values).
    pub health: Vec<(u32, Vec<u8>)>,
    /// The stretch between two snapshots in which the soul gap moved most, and by how much (positive: toward
    /// the player).
    pub turn: Option<(u32, u32, f64)>,
}

/// The laning phase minute by minute against the lane opponents.
pub fn lane_flow(report: &MatchReport, json: &str, slot: u32) -> Result<LaneFlow, String> {
    let root: Value =
        serde_json::from_str(json).map_err(|e| format!("Unreadable match data: {e}"))?;
    let info = root
        .get("match_info")
        .ok_or("The response has no match information")?;
    let me = report
        .players
        .iter()
        .find(|p| p.slot == slot)
        .ok_or("No such player in the match")?;
    let opponents = me.lane_opponents.clone();
    let players = list(info, "players");
    let tracks = fights::tracks(info, |s| {
        players
            .iter()
            .find(|p| int(p, "player_slot") == s)
            .map(|p| int(p, "team"))
    });
    let track_of = |s: u32| tracks.iter().find(|t| t.slot == s);
    let end = LANE_S.min(report.duration_s);
    let lost = |s: u32, from: u32, to: u32| -> f64 {
        let Some(t) = track_of(s) else { return 0.0 };
        (from.max(1)..to)
            .map(|i| t.health_at(i - 1).saturating_sub(t.health_at(i)) as f64)
            .sum()
    };
    let minutes = (0..end.div_ceil(60))
        .map(|m| {
            let (from, to) = (m * 60, ((m + 1) * 60).min(end));
            let theirs: Vec<f64> = opponents.iter().map(|o| lost(*o, from, to)).collect();
            LaneMinute {
                minute: m,
                lost: lost(slot, from, to),
                their_lost: if theirs.is_empty() {
                    0.0
                } else {
                    theirs.iter().sum::<f64>() / theirs.len() as f64
                },
                fighting: track_of(slot)
                    .map(|t| t.seconds_in(1, from, to))
                    .unwrap_or(0),
                kills: me
                    .kills_at
                    .iter()
                    .filter(|k| k.time >= from && k.time < to)
                    .count() as u32,
                deaths: me
                    .deaths_at
                    .iter()
                    .filter(|d| d.time >= from && d.time < to)
                    .count() as u32,
            }
        })
        .collect();
    let (times, totals) =
        damage_over_time(&info.get("damage_matrix").cloned().unwrap_or(Value::Null));
    let hurt = |from: u32, to: u32, i: usize| -> f64 {
        let at = |i: usize| {
            totals
                .iter()
                .filter(|(k, _)| k.0 == from && k.1 == to)
                .map(|(_, v)| v.get(i).copied().unwrap_or(0.0))
                .sum::<f64>()
        };
        at(i) - if i > 0 { at(i - 1) } else { 0.0 }
    };
    let opp_reports: Vec<_> = report
        .players
        .iter()
        .filter(|p| opponents.contains(&p.slot))
        .collect();
    let mut marks = vec![];
    for sample in me.timeline.iter().filter(|s| s.time <= end + 60) {
        let mean = |f: &dyn Fn(&crate::matches::Sample) -> f64| {
            let values: Vec<f64> = opp_reports
                .iter()
                .filter_map(|p| p.timeline.iter().find(|s| s.time == sample.time))
                .map(f)
                .collect();
            if values.is_empty() {
                0.0
            } else {
                values.iter().sum::<f64>() / values.len() as f64
            }
        };
        let index = times.iter().position(|t| *t == sample.time);
        marks.push(LaneMark {
            time: sample.time,
            souls: sample.net_worth as f64,
            their_souls: mean(&|s| s.net_worth as f64),
            last_hits: sample.last_hits as f64,
            their_last_hits: mean(&|s| s.last_hits as f64),
            dealt: index
                .map(|i| opponents.iter().map(|o| hurt(slot, *o, i)).sum())
                .unwrap_or(0.0),
            taken: index
                .map(|i| opponents.iter().map(|o| hurt(*o, slot, i)).sum())
                .unwrap_or(0.0),
        });
    }
    let gap = |m: &LaneMark| m.souls - m.their_souls;
    let mut turn: Option<(u32, u32, f64)> = None;
    let mut previous = (0u32, 0.0f64);
    for m in &marks {
        let swing = gap(m) - previous.1;
        if turn.is_none_or(|t| swing.abs() > t.2.abs()) {
            turn = Some((previous.0, m.time, swing));
        }
        previous = (m.time, gap(m));
    }
    let health = std::iter::once(slot)
        .chain(opponents.iter().copied())
        .filter_map(|s| {
            track_of(s).map(|t| {
                (
                    s,
                    (0..end)
                        .step_by(5)
                        .map(|i| t.health_at(i).min(100) as u8)
                        .collect(),
                )
            })
        })
        .collect();
    Ok(LaneFlow {
        opponents,
        minutes,
        marks,
        health,
        turn: if opp_reports.is_empty() { None } else { turn },
    })
}

#[derive(Debug, Clone, Serialize)]
pub struct AverageRow {
    pub label: String,
    pub this: f64,
    pub average: f64,
    pub higher_is_better: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct VersusAverage {
    pub matches: u32,
    pub wins: u32,
    pub rows: Vec<AverageRow>,
}

/// The match next to the player's own average on the hero (deadlock-api `/v1/players/hero-stats`).
pub fn versus_average(
    report: &MatchReport,
    slot: u32,
    hero_id: u64,
    stats_json: &str,
) -> Option<VersusAverage> {
    let p = report.players.iter().find(|p| p.slot == slot)?;
    let Ok(Value::Array(rows)) = serde_json::from_str::<Value>(stats_json) else {
        return None;
    };
    let row = rows.iter().find(|r| {
        r.get("hero_id").and_then(Value::as_u64) == Some(hero_id)
            && r.get("account_id").and_then(Value::as_u64) == Some(p.account_id)
    })?;
    let matches = int(row, "matches_played");
    if matches == 0 {
        return None;
    }
    let minutes = (report.duration_s as f64 / 60.0).max(1.0);
    let per_min = |v: f64| v / minutes;
    let made = |label: &str, this: f64, average: f64, higher: bool| AverageRow {
        label: label.into(),
        this,
        average,
        higher_is_better: higher,
    };
    let out = vec![
        made(
            "Kills a minute",
            per_min(p.kills as f64),
            num(row, "kills_per_min"),
            true,
        ),
        made(
            "Deaths a minute",
            per_min(p.deaths as f64),
            num(row, "deaths_per_min"),
            false,
        ),
        made(
            "Assists a minute",
            per_min(p.assists as f64),
            num(row, "assists_per_min"),
            true,
        ),
        made(
            "Souls a minute",
            per_min(p.net_worth as f64),
            num(row, "networth_per_min"),
            true,
        ),
        made(
            "Last hits a minute",
            per_min(p.last_hits as f64),
            num(row, "last_hits_per_min"),
            true,
        ),
        made(
            "Denies a minute",
            per_min(p.denies as f64),
            num(row, "denies_per_min"),
            true,
        ),
        made(
            "Hero damage a minute",
            per_min(p.hero_damage),
            num(row, "damage_per_min"),
            true,
        ),
        made(
            "Damage taken a minute",
            per_min(p.damage_taken),
            num(row, "damage_taken_per_min"),
            false,
        ),
        made(
            "Objective damage a minute",
            per_min(p.objective_damage),
            num(row, "obj_damage_per_min"),
            true,
        ),
        made(
            "Hero damage per soul",
            p.hero_damage / (p.net_worth as f64).max(1.0),
            num(row, "damage_per_soul"),
            true,
        ),
    ];
    Some(VersusAverage {
        matches,
        wins: int(row, "wins"),
        rows: out,
    })
}
