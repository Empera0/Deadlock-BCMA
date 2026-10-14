//! Finds team fights in a match from the per-second player positions the game records,
//! and works out who was there, who was dead and who was somewhere else.

use serde::Serialize;
use serde_json::Value;

/// Game units per meter.
const UNITS: f64 = 39.37;
/// Players this close to each other while fighting heroes belong to the same fight.
const LINK: f64 = 2200.0;
/// Anyone alive this close to the middle of a fight is part of it.
const REACH: f64 = 2400.0;
/// Seconds without fighting before a new fight starts, and how far its middle may drift.
const GAP: u32 = 15;
const DRIFT: f64 = 4000.0;
/// A revive starts from this health percentage or less.
const REVIVE_FROM: u64 = 12;
/// Health percentage that counts as nearly dead.
const CLOSE_CALL: u64 = 10;
/// A fight with this many enemies in it for this many seconds is one a missing teammate is blamed for.
const MOST_OF_A_TEAM: usize = 4;
const LONG_ENOUGH: u32 = 10;
/// The game's per-second combat state for fighting enemy heroes, troopers/objectives and neutral camps.
const HERO_COMBAT: u64 = 1;

/// One player's second-by-second record.
pub struct Track {
    pub slot: u32,
    pub team: u32,
    xs: Vec<f64>,
    ys: Vec<f64>,
    health: Vec<u64>,
    combat: Vec<u64>,
}

impl Track {
    fn alive(&self, t: usize) -> bool {
        self.health.get(t).copied().unwrap_or(0) > 0
    }

    fn at(&self, t: usize) -> (f64, f64) {
        let i = t.min(self.xs.len().saturating_sub(1));
        (self.xs.get(i).copied().unwrap_or(0.0), self.ys.get(i).copied().unwrap_or(0.0))
    }

    /// Times the player dropped to a sliver of health and did not die in the next seconds.
    pub fn close_calls(&self, deaths: &[u32], duration: u32) -> u32 {
        (1..self.health.len().min(duration as usize))
            .filter(|&t| self.health[t] > 0 && self.health[t] <= CLOSE_CALL && self.health[t - 1] > CLOSE_CALL)
            .filter(|&t| !deaths.iter().any(|d| *d as usize >= t && *d as usize <= t + 8))
            .count() as u32
    }

    /// Moments the hero was brought to almost nothing and then shot back up by a third of their health
    /// per second: the trace a revive leaves (Victor's Shocking Reanimation), since the game records no death for it.
    pub fn revives(&self, duration: u32) -> Vec<u32> {
        let h = &self.health;
        let mut out: Vec<u32> = vec![];
        for t in 1..h.len().saturating_sub(2).min(duration as usize) {
            let fresh = out.last().is_none_or(|last| t as u32 > last + 5);
            if fresh && h[t] <= REVIVE_FROM && h[t + 1] >= h[t] + 20 && h[t + 2] >= h[t + 1] + 15 {
                out.push(t as u32);
            }
        }
        out
    }

    /// The position every `every` seconds up to `duration`, with whether the player was alive.
    pub fn sampled(&self, every: usize, duration: u32) -> Vec<(f64, f64, bool)> {
        (0..self.xs.len().min(duration as usize)).step_by(every.max(1)).map(|t| (self.xs[t], self.ys[t], self.alive(t))).collect()
    }

    pub fn alive_at(&self, time: u32) -> bool {
        self.alive(time as usize)
    }

    pub fn place(&self, time: u32) -> (f64, f64) {
        self.at(time as usize)
    }

    /// The game's combat state at a second: 1 heroes, 2 troopers or objectives, 3 neutral camps, 0 none.
    pub fn combat_at(&self, time: u32) -> u64 {
        self.combat.get(time as usize).copied().unwrap_or(0)
    }

    /// Seconds in `from..to` the player was alive and in the given combat state.
    pub fn seconds_in(&self, state: u64, from: u32, to: u32) -> u32 {
        (from as usize..to as usize).filter(|&t| self.alive(t) && self.combat.get(t).copied() == Some(state)).count() as u32
    }

    /// Seconds in `from..to` the player was alive within `radius` of a point.
    pub fn seconds_near(&self, point: (f64, f64), radius: f64, from: u32, to: u32) -> u32 {
        (from as usize..to as usize).filter(|&t| t < self.xs.len() && self.alive(t) && dist(self.at(t), point) < radius).count() as u32
    }

    /// Seconds of the match spent fighting heroes, hitting troopers or objectives, farming neutral camps,
    /// out of combat, and dead, in that order.
    pub fn time_split(&self, duration: u32) -> [u32; 5] {
        let mut out = [0u32; 5];
        for t in 0..self.health.len().min(duration as usize) {
            let state = if !self.alive(t) { 4 } else { [3usize, 0, 1, 2][(self.combat.get(t).copied().unwrap_or(0) as usize).min(3)] };
            out[state] += 1;
        }
        out
    }

    fn fighting(&self, t: usize) -> bool {
        self.alive(t) && self.combat.get(t).copied() == Some(HERO_COMBAT)
    }
}

/// Within this distance and fighting heroes, a player is taken to be fighting that enemy.
const ENGAGED: f64 = 1400.0;

/// For the seconds in `from..to` in which `mate` was fighting `target`: how many there were, and in how many
/// of them `owner` was fighting the same target too, or had been within the last `linger` seconds
/// (a debuff outlasts the hit that applied it).
pub fn shared_time(owner: &Track, mate: &Track, target: &Track, from: u32, to: u32, linger: u32) -> (u32, u32) {
    let on = |who: &Track, t: usize| who.fighting(t) && target.alive(t) && dist(who.at(t), target.at(t)) < ENGAGED;
    let (mut mate_seconds, mut shared) = (0, 0);
    let mut last_owner_hit: Option<usize> = None;
    for t in from.saturating_sub(linger) as usize..to as usize {
        if on(owner, t) {
            last_owner_hit = Some(t);
        }
        if t >= from as usize && on(mate, t) {
            mate_seconds += 1;
            shared += last_owner_hit.is_some_and(|hit| t - hit <= linger as usize) as u32;
        }
    }
    (mate_seconds, shared)
}

fn dist(a: (f64, f64), b: (f64, f64)) -> f64 {
    ((a.0 - b.0).powi(2) + (a.1 - b.1).powi(2)).sqrt()
}

/// Reads `match_paths`; positions are stored as steps between each player's own min and max.
pub fn tracks(info: &Value, team_of: impl Fn(u32) -> Option<u32>) -> Vec<Track> {
    let paths = info.get("match_paths").cloned().unwrap_or(Value::Null);
    let num = |v: &Value, key: &str| v.get(key).and_then(Value::as_f64).unwrap_or(0.0);
    let (x_res, y_res) = (num(&paths, "x_resolution").max(1.0), num(&paths, "y_resolution").max(1.0));
    let ints = |v: &Value, key: &str| -> Vec<u64> {
        v.get(key).and_then(Value::as_array).map(|a| a.iter().map(|x| x.as_u64().unwrap_or(0)).collect()).unwrap_or_default()
    };
    paths
        .get("paths")
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .unwrap_or(&[])
        .iter()
        .filter_map(|p| {
            let slot = num(p, "player_slot") as u32;
            let scale = |key: &str, min: f64, max: f64, res: f64| -> Vec<f64> {
                ints(p, key).iter().map(|v| min + *v as f64 / res * (max - min)).collect()
            };
            Some(Track {
                slot,
                team: team_of(slot)?,
                xs: scale("x_pos", num(p, "x_min"), num(p, "x_max"), x_res),
                ys: scale("y_pos", num(p, "y_min"), num(p, "y_max"), y_res),
                health: ints(p, "health"),
                combat: ints(p, "combat_type"),
            })
        })
        .collect()
}

#[derive(Debug, Clone, Serialize)]
pub struct Absence {
    pub slot: u32,
    /// What the player was doing instead.
    pub doing: String,
    /// Meters from the middle of the fight.
    pub distance: f64,
    /// True when the team was not ahead in numbers, so the player was needed.
    pub missed: bool,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct Side {
    pub present: Vec<u32>,
    pub absent: Vec<Absence>,
    /// Dead or respawning when the fight broke out.
    pub dead: Vec<u32>,
    /// Members who died in the fight.
    pub deaths: Vec<u32>,
}

#[derive(Debug, Clone, Serialize)]
pub struct Fight {
    pub start: u32,
    pub end: u32,
    /// One entry per team.
    pub sides: Vec<Side>,
    /// Objectives that fell while it was going on, as (destroying team, name).
    pub objectives: Vec<(u32, String)>,
    /// A late-game fight: staying away from it is a mistake whatever else the player was doing.
    pub late: bool,
    /// Where it happened, in world units.
    pub x: f64,
    pub y: f64,
    /// The deaths in it, in order.
    pub kills: Vec<FightKill>,
    /// Souls of the players present on each side when it started.
    pub souls: Vec<f64>,
    /// The team that lost fewer players; `None` when it was even.
    pub winner: Option<u32>,
    /// A late-game fight with the match still in the balance: nothing excuses staying away, not even an objective.
    pub even: bool,
    /// Walkers each team still had, and each team's total souls, when it started.
    pub walkers: Vec<u32>,
    pub team_souls: Vec<f64>,
    /// What the winner made of a fight it clearly won; `None` for fights without a clear winner.
    pub follow: Option<FollowUp>,
}

/// The time right after a fight one team clearly won.
#[derive(Debug, Clone, Serialize)]
pub struct FollowUp {
    /// Seconds looked at: up to the next team fight at most.
    pub window_s: u32,
    /// How many more players the loser lost.
    pub lead: u32,
    /// Objectives the winner took in the fight and in the time after it, and whether it took the mid boss.
    pub taken: Vec<String>,
    pub boss: bool,
    /// Winners who survived and went to neutral camps instead.
    pub farming: Vec<u32>,
}

#[derive(Debug, Clone, Serialize)]
pub struct FightKill {
    pub time: u32,
    pub victim: u32,
    pub killer: u32,
}

/// A death: (time, victim slot, x, y).
pub type Death = (u32, u32, f64, f64);

struct Draft {
    start: u32,
    end: u32,
    seconds: u32,
    middle: (f64, f64),
    /// Seconds each slot spent in the fight.
    seen: Vec<(u32, u32)>,
}

/// `fallen` are destroyed objectives as (time, destroying team, name).
/// Fights from `late_from` on are late-game fights.
/// `credited` are objectives a player demonstrably helped destroy, as (time, player slot, what).
pub fn detect(
    tracks: &[Track],
    deaths: &[Death],
    fallen: &[(u32, u32, String)],
    credited: &[(u32, u32, String)],
    duration: u32,
    late_from: u32,
    // Whether the match was still in the balance at a given time.
    even_at: &dyn Fn(u32) -> bool,
) -> Vec<Fight> {
    let mut drafts: Vec<Draft> = vec![];
    for t in 0..duration as usize {
        let active: Vec<&Track> = tracks.iter().filter(|p| p.fighting(t)).collect();
        // Group the players fighting heroes into clusters of nearby players.
        let mut group: Vec<usize> = (0..active.len()).collect();
        for i in 0..active.len() {
            for j in i + 1..active.len() {
                if dist(active[i].at(t), active[j].at(t)) < LINK {
                    let (a, b) = (group[i], group[j]);
                    for g in &mut group {
                        if *g == b {
                            *g = a;
                        }
                    }
                }
            }
        }
        let mut best: Option<((f64, f64), Vec<u32>)> = None;
        for id in 0..active.len() {
            let members: Vec<&&Track> = active.iter().enumerate().filter(|(i, _)| group[*i] == id).map(|(_, p)| p).collect();
            if members.len() < 3 || members.iter().all(|p| p.team == members[0].team) {
                continue;
            }
            let n = members.len() as f64;
            let middle = (members.iter().map(|p| p.at(t).0).sum::<f64>() / n, members.iter().map(|p| p.at(t).1).sum::<f64>() / n);
            // Everyone alive around the middle counts, also those not exchanging damage this very second.
            let involved: Vec<&Track> = tracks.iter().filter(|p| p.alive(t) && dist(p.at(t), middle) < REACH).collect();
            let first_team = involved.iter().filter(|p| p.team == involved[0].team).count();
            let other_team = involved.len() - first_team;
            // A team fight: at least five players, at least two on each side.
            if involved.len() >= 5 && first_team >= 2 && other_team >= 2 && best.as_ref().is_none_or(|(_, b)| involved.len() > b.len()) {
                best = Some((middle, involved.iter().map(|p| p.slot).collect()));
            }
        }
        let Some((middle, slots)) = best else { continue };
        let t = t as u32;
        let same = drafts.last().is_some_and(|d| t - d.end <= GAP && dist(d.middle, middle) < DRIFT);
        if !same {
            drafts.push(Draft { start: t, end: t, seconds: 0, middle, seen: vec![] });
        }
        let d = drafts.last_mut().unwrap();
        d.end = t;
        d.seconds += 1;
        let n = d.seconds as f64;
        d.middle = (d.middle.0 + (middle.0 - d.middle.0) / n, d.middle.1 + (middle.1 - d.middle.1) / n);
        for slot in slots {
            match d.seen.iter_mut().find(|(s, _)| *s == slot) {
                Some((_, count)) => *count += 1,
                None => d.seen.push((slot, 1)),
            }
        }
    }

    let teams: Vec<u32> = {
        let mut teams: Vec<u32> = tracks.iter().map(|p| p.team).collect();
        teams.sort();
        teams.dedup();
        teams
    };
    drafts
        .into_iter()
        .filter_map(|d| {
            let died: Vec<&Death> =
                deaths.iter().filter(|x| x.0 + 3 >= d.start && x.0 <= d.end + 6 && dist((x.2, x.3), d.middle) < REACH * 1.5).collect();
            // A few seconds of contact without a death is a poke, not a fight.
            if d.seconds < 6 && died.len() < 2 {
                return None;
            }
            let end = died.iter().map(|x| x.0).fold(d.end, u32::max);
            let needed = (d.seconds / 5).max(2);
            let present_slot = |slot: u32| d.seen.iter().any(|(s, n)| *s == slot && *n >= needed) || died.iter().any(|x| x.1 == slot);
            let objectives: Vec<(u32, String)> =
                fallen.iter().filter(|o| o.0 + 10 >= d.start && o.0 <= end + 20).map(|o| (o.1, o.2.clone())).collect();
            let mid = ((d.start + end) / 2) as usize;
            let late = d.start >= late_from;
            let even = late && even_at(d.start);
            let mut sides: Vec<Side> = teams
                .iter()
                .map(|team| {
                    let mut side = Side::default();
                    for p in tracks.iter().filter(|p| p.team == *team) {
                        if present_slot(p.slot) {
                            side.present.push(p.slot);
                        } else if !p.alive(d.start as usize) && !p.alive(mid) {
                            side.dead.push(p.slot);
                        } else {
                            // What the player spent most of the fight doing.
                            let mut counts = [0u32; 4];
                            for t in d.start as usize..=end as usize {
                                counts[(p.combat.get(t).copied().unwrap_or(0) as usize).min(3)] += 1;
                            }
                            let state = (0..4).max_by_key(|i| counts[*i]).unwrap_or(0);
                            let took_objective = objectives.iter().any(|(by, _)| by == team);
                            // An objective this very player was at when it fell is a trade, at any stage of the game.
                            let trade = credited.iter().find(|c| c.1 == p.slot && c.0 + 10 >= d.start && c.0 <= end + 20);
                            let doing = match (trade, state) {
                                (Some(c), _) if even => format!("taking {} while the match was even, instead of joining the fight", c.2),
                                (Some(c), _) => format!("taking {}", c.2),
                                (None, 1) => "fighting elsewhere".to_string(),
                                (None, 2) if late => "hitting troopers or an objective that did not fall, instead of joining a late-game fight".to_string(),
                                (None, 2) if took_objective => "hitting troopers or an objective (one fell for the team)".to_string(),
                                (None, 2) => "hitting troopers or an objective".to_string(),
                                (None, 3) => "farming a neutral camp".to_string(),
                                (None, _) => "out of combat".to_string(),
                            };
                            side.absent.push(Absence {
                                slot: p.slot,
                                doing,
                                distance: dist(p.at(mid), d.middle) / UNITS,
                                // Decided below, once both sides are counted.
                                // Being in another fight or taking an objective for the team is not staying away.
                                // Late in the game the fight comes first, unless an objective was really taken.
                                // With the match even, an objective is no excuse: losing the fight costs more than it gains.
                                missed: if trade.is_some() { even } else { state != 1 && (late || !(state == 2 && took_objective)) },
                            });
                        }
                    }
                    side.deaths = died.iter().filter(|x| tracks.iter().any(|p| p.slot == x.1 && p.team == *team)).map(|x| x.1).collect();
                    side
                })
                .collect();
            let counts: Vec<usize> = sides.iter().map(|s| s.present.len()).collect();
            for (i, side) in sides.iter_mut().enumerate() {
                let enemy = counts.iter().enumerate().filter(|(j, _)| *j != i).map(|(_, n)| *n).max().unwrap_or(0);
                for a in &mut side.absent {
                    // Only a problem in a real team fight (most of the enemy team there, long enough to reach)
                    // in which the team did not already have the numbers.
                    // Late in the game every living player belongs in it, numbers or not.
                    a.missed = a.missed && (late || counts[i] <= enemy) && enemy >= MOST_OF_A_TEAM && end - d.start >= LONG_ENOUGH;
                }
            }
            Some(Fight { start: d.start, end, sides, objectives, late, x: d.middle.0, y: d.middle.1, kills: vec![], souls: vec![], winner: None, even, walkers: vec![], team_souls: vec![], follow: None })
        })
        .collect()
}
