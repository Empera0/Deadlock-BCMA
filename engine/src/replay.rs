//! A match's replay, read for one player: the last seconds before each death, hit by hit (who, with what, how
//! much health was left) with every stun, sleep, silence or hold put on the player; and each of the player's
//! abilities: how many enemies a cast caught and held, against what a cast of it catches on average at the
//! player's build. Using an ability at the right time matters more than using it often, so casts are judged by
//! what they did, never by how long the ability waited.
//!
//! The replay comes from the statistics service's demo queries, as newline-delimited JSON (zstd compressed):
//! hero-on-hero damage events, ability notifications (with the status they put on the target) and the kills,
//! which put the replay's ticks on the match clock.

use std::collections::BTreeMap;
use std::io::Read;

use serde::Serialize;
use serde_json::Value;

use crate::area;
use crate::calc;
use crate::data::GameData;
use crate::deaths;
use crate::matches::{int, list};

/// The queries a study needs, run against the match's replay.
pub const QUERY_HITS: &str = "SELECT tick, ability_id, entindex_attacker, entindex_victim, damage, victim_health_new FROM DamageEvent WHERE attacker_class = 1 AND victim_class = 1 AND damage > 0";
pub const QUERY_NOTES: &str = "SELECT tick, victim, attacker, ability_id, status_impact FROM AbilityNotifyEvent";
pub const QUERY_KILLS: &str = "SELECT tick, entindex_victim, entindex_attacker FROM HeroKilledEvent";

/// Seconds before a death that are shown.
const BEFORE_DEATH_S: f64 = 10.0;
/// Hits from the same hero and source this close together are one line.
const MERGE_S: f64 = 0.5;
/// A cast is one hero's damage from one ability with no gap over this, seconds.
const CAST_GAP_S: f64 = 2.0;
/// A cast holds the enemies its notifications reach from this long before its first hit (a channel lifts them before
/// the slam lands: Death Slam) to this long after its last, seconds.
const HOLD_BEFORE_S: f64 = 3.5;
const HOLD_AFTER_S: f64 = 2.0;
/// The caster's build is read once per this many seconds.
const BUILD_STEP_S: u32 = 15;
/// A cast catches more or fewer enemies than the usual when it is off by this many.
const USUAL_BAND: f64 = 0.15;
/// Entity handles carry the entity index in their low bits.
const HANDLE_INDEX: u64 = 0x3FFF;

/// Unpacks a zstd-compressed answer.
pub fn decompress(bytes: &[u8]) -> Result<String, String> {
    let mut decoder = ruzstd::decoding::StreamingDecoder::new(bytes).map_err(|e| format!("The replay data could not be unpacked: {e}"))?;
    let mut out = String::new();
    decoder.read_to_string(&mut out).map_err(|e| format!("The replay data could not be unpacked: {e}"))?;
    Ok(out)
}

fn rows(ndjson: &str) -> Vec<Value> {
    ndjson.lines().filter_map(|l| serde_json::from_str(l).ok()).collect()
}

/// The statuses an ability notification puts on its target, by bit.
const STATUSES: [(u64, &str); 5] = [(1, "stunned"), (4, "put to sleep"), (8, "silenced"), (16, "slowed"), (32, "held in place")];

fn status_words(flags: u64) -> Vec<&'static str> {
    STATUSES.iter().filter(|s| flags & s.0 != 0).map(|s| s.1).collect()
}

#[derive(Debug, Clone, Serialize)]
pub struct Moment {
    /// Seconds before the death (negative) of the first hit of the line, and of its last.
    pub at: f64,
    pub until: f64,
    pub hero: String,
    pub source: String,
    pub damage: f64,
    pub hits: u32,
    pub health_after: f64,
}

#[derive(Debug, Clone, Serialize)]
pub struct StatusMoment {
    pub at: f64,
    pub hero: String,
    pub source: String,
    pub statuses: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct DeathMoment {
    pub time: u32,
    pub killer: String,
    /// Health when the first hit of the window landed.
    pub health_start: Option<f64>,
    pub hits: Vec<Moment>,
    pub statuses: Vec<StatusMoment>,
    /// Damage taken in the window by hero, most first, with each hero's sources.
    pub by_hero: Vec<(String, f64, Vec<(String, f64)>)>,
    pub summary: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct AbilityCard {
    pub ability: String,
    pub icon: String,
    /// Casts that reached an enemy hero.
    pub casts: u32,
    pub enemies_per_cast: f64,
    /// What a cast of it catches on average at the player's build, for an area ability.
    pub usual: Option<f64>,
    pub damage_per_cast: f64,
    /// Enemies a cast stunned, slept, silenced or held.
    pub held_per_cast: f64,
    pub verdict: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct ReplayStudy {
    pub deaths: Vec<DeathMoment>,
    pub abilities: Vec<AbilityCard>,
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
    (sxx > 0.0).then(|| {
        let a = sxy / sxx;
        (a, my - a * mx)
    })
}

struct Hit {
    t: f64,
    ability: u64,
    attacker: u64,
    victim: u64,
    damage: f64,
    health: f64,
}

struct Note {
    t: f64,
    ability: u64,
    attacker: u64,
    victim: u64,
    flags: u64,
}

/// Studies one player of a match from its replay: `json` is the match metadata, the rest the answers to
/// `QUERY_HITS`, `QUERY_NOTES` and `QUERY_KILLS`.
pub fn study(data: &GameData, json: &str, slot: u32, hits: &str, notes: &str, kills: &str) -> Result<ReplayStudy, String> {
    let root: Value = serde_json::from_str(json).map_err(|e| format!("Unreadable match data: {e}"))?;
    let info = root.get("match_info").ok_or("The match data has no match information")?;
    let players = list(info, "players");
    let player = players.iter().find(|p| int(p, "player_slot") == slot).ok_or("No such player in the match")?;
    let hero_of_slot = |p: &Value| p.get("hero_id").and_then(Value::as_u64).and_then(|id| data.heroes.iter().find(|h| h.id == id));
    let me = hero_of_slot(player).ok_or("The player's hero is not in the data")?;

    // The replay's ticks on the match clock, from the kills against the deaths the metadata records.
    let mut death_times: Vec<f64> = players.iter().flat_map(|p| list(p, "death_details")).filter_map(|d| d.get("game_time_s").and_then(Value::as_f64)).collect();
    death_times.sort_by(f64::total_cmp);
    let mut kill_ticks: Vec<f64> = rows(kills).iter().filter_map(|r| r["tick"].as_f64()).collect();
    kill_ticks.sort_by(f64::total_cmp);
    let (per_tick, offset) = clock(&kill_ticks, &death_times).ok_or("The replay has too few kills to line it up with the match")?;
    let at = |tick: f64| tick * per_tick + offset;

    let hits: Vec<Hit> = rows(hits)
        .iter()
        .filter_map(|r| {
            Some(Hit {
                t: at(r["tick"].as_f64()?),
                ability: r["ability_id"].as_u64().unwrap_or(0),
                attacker: r["entindex_attacker"].as_u64()?,
                victim: r["entindex_victim"].as_u64()?,
                damage: r["damage"].as_f64().unwrap_or(0.0),
                health: r["victim_health_new"].as_f64().unwrap_or(0.0),
            })
        })
        .collect();
    let notes: Vec<Note> = rows(notes)
        .iter()
        .filter_map(|r| {
            Some(Note {
                t: at(r["tick"].as_f64()?),
                ability: r["ability_id"].as_u64().unwrap_or(0),
                attacker: r["attacker"].as_u64()? & HANDLE_INDEX,
                victim: r["victim"].as_u64()? & HANDLE_INDEX,
                flags: r["status_impact"].as_u64().unwrap_or(0),
            })
        })
        .collect();

    // Who each entity is: the hero whose abilities it uses.
    let owner: BTreeMap<u64, u64> = data.heroes.iter().flat_map(|h| h.abilities.iter().map(move |a| (a.id, h.id))).collect();
    let mut votes: BTreeMap<u64, BTreeMap<u64, u32>> = BTreeMap::new();
    for h in &hits {
        if let Some(hero) = owner.get(&h.ability) {
            *votes.entry(h.attacker).or_default().entry(*hero).or_default() += 1;
        }
    }
    let hero_of: BTreeMap<u64, u64> = votes.iter().filter_map(|(e, v)| v.iter().max_by_key(|x| x.1).map(|x| (*e, *x.0))).collect();
    let hero_name = |e: u64| hero_of.get(&e).and_then(|id| data.heroes.iter().find(|h| h.id == *id)).map(|h| h.name.clone()).unwrap_or_else(|| "?".into());
    let my_ent = hero_of.iter().find(|(_, h)| **h == me.id).map(|(e, _)| *e).ok_or("The player is not in the replay's damage")?;

    // Damage the data does not name is the gun or melee: per hero, the most used unknown source is the gun, the next
    // melee; one used by several heroes (the zipline) is something else.
    let known = |id: u64| owner.contains_key(&id) || data.items.iter().any(|i| i.id == id);
    let mut unknown: BTreeMap<u64, BTreeMap<u64, u32>> = BTreeMap::new();
    for h in hits.iter().filter(|h| !known(h.ability)) {
        *unknown.entry(h.attacker).or_default().entry(h.ability).or_default() += 1;
    }
    let shared: Vec<u64> = {
        let mut users: BTreeMap<u64, u32> = BTreeMap::new();
        for ids in unknown.values() {
            for id in ids.keys() {
                *users.entry(*id).or_default() += 1;
            }
        }
        users.into_iter().filter(|(_, n)| *n > 1).map(|(id, _)| id).collect()
    };
    let source = |attacker: u64, ability: u64| -> String {
        if let Some(a) = data.heroes.iter().flat_map(|h| h.abilities.iter()).find(|a| a.id == ability) {
            return a.name.clone();
        }
        if let Some(i) = data.items.iter().find(|i| i.id == ability) {
            return i.name.clone();
        }
        if shared.contains(&ability) {
            return "Other".into();
        }
        let mut own: Vec<(u64, u32)> = unknown.get(&attacker).map(|m| m.iter().filter(|x| !shared.contains(x.0)).map(|x| (*x.0, *x.1)).collect()).unwrap_or_default();
        own.sort_by(|a, b| b.1.cmp(&a.1));
        match own.iter().position(|x| x.0 == ability) {
            Some(0) => "Gun".into(),
            Some(1) => "Melee".into(),
            _ => "Other".into(),
        }
    };

    // The last seconds before each death.
    let mut deaths_out = vec![];
    for d in list(player, "death_details") {
        let time = d.get("game_time_s").and_then(Value::as_f64).unwrap_or(0.0);
        let killer = players.iter().find(|p| int(p, "player_slot") == int(d, "killer_player_slot")).and_then(hero_of_slot).map(|h| h.name.clone()).unwrap_or_default();
        let window: Vec<&Hit> = hits.iter().filter(|h| h.victim == my_ent && h.t >= time - BEFORE_DEATH_S && h.t <= time + MERGE_S).collect();
        let mut moments: Vec<Moment> = vec![];
        for h in &window {
            let (hero, src, t) = (hero_name(h.attacker), source(h.attacker, h.ability), h.t - time);
            // A run of hits from the same hero and source is one line, even with other hits in between (Bullet
            // Dance and Fixation land together).
            match moments.iter_mut().rev().find(|m| m.hero == hero && m.source == src && t - m.until <= MERGE_S) {
                Some(m) => {
                    m.damage += h.damage;
                    m.hits += 1;
                    m.until = t;
                    m.health_after = h.health;
                }
                None => moments.push(Moment { at: t, until: t, hero, source: src, damage: h.damage, hits: 1, health_after: h.health }),
            }
        }
        let statuses: Vec<StatusMoment> = notes
            .iter()
            .filter(|n| n.victim == my_ent && n.flags > 0 && n.t >= time - BEFORE_DEATH_S && n.t <= time + MERGE_S)
            .map(|n| StatusMoment {
                at: n.t - time,
                hero: hero_name(n.attacker),
                source: source(n.attacker, n.ability),
                statuses: status_words(n.flags).into_iter().map(String::from).collect(),
            })
            .collect();
        let mut by: BTreeMap<String, (f64, BTreeMap<String, f64>)> = BTreeMap::new();
        for m in &moments {
            let e = by.entry(m.hero.clone()).or_default();
            e.0 += m.damage;
            *e.1.entry(m.source.clone()).or_default() += m.damage;
        }
        let mut by_hero: Vec<(String, f64, Vec<(String, f64)>)> = by
            .into_iter()
            .map(|(h, (total, sources))| {
                let mut s: Vec<(String, f64)> = sources.into_iter().collect();
                s.sort_by(|a, b| b.1.total_cmp(&a.1));
                (h, total, s)
            })
            .collect();
        by_hero.sort_by(|a, b| b.1.total_cmp(&a.1));
        let health_start = window.first().map(|h| h.health + h.damage);
        let total: f64 = by_hero.iter().map(|b| b.1).sum();
        let mut summary = if by_hero.is_empty() {
            "The replay shows no hero damage in the last seconds.".to_string()
        } else {
            let who: Vec<String> = by_hero.iter().take(3).map(|(h, d, s)| format!("{h} {d:.0} ({})", s.first().map(|x| x.0.as_str()).unwrap_or(""))).collect();
            let span = moments.first().map(|m| -m.at).unwrap_or(0.0);
            format!("{total:.0} damage in the last {span:.1}s, from {} hero{}: {}.", by_hero.len(), if by_hero.len() == 1 { "" } else { "es" }, who.join(", "))
        };
        if let Some(s) = statuses.iter().find(|s| s.statuses.iter().any(|w| w != "slowed")) {
            summary += &format!(" {}'s {} had you {} {:.1}s before you died.", s.hero, s.source, s.statuses.join(" and "), -s.at);
        }
        deaths_out.push(DeathMoment { time: time as u32, killer, health_start, hits: moments, statuses, by_hero, summary });
    }

    // Each of the player's abilities: what its casts caught and held.
    let mut sheets: BTreeMap<u32, Option<calc::Sheet>> = BTreeMap::new();
    let mut cards = vec![];
    for a in me.abilities.iter().take(4) {
        let mut own: Vec<&Hit> = hits.iter().filter(|h| h.attacker == my_ent && h.ability == a.id && h.victim != my_ent).collect();
        own.sort_by(|x, y| x.t.total_cmp(&y.t));
        let mut casts: Vec<(f64, f64, Vec<u64>, f64)> = vec![];
        for h in own {
            match casts.last_mut() {
                Some(c) if h.t - c.1 <= CAST_GAP_S => {
                    c.1 = h.t;
                    if !c.2.contains(&h.victim) {
                        c.2.push(h.victim);
                    }
                    c.3 += h.damage;
                }
                _ => casts.push((h.t, h.t, vec![h.victim], h.damage)),
            }
        }
        if casts.is_empty() {
            continue;
        }
        let n = casts.len() as f64;
        let held: f64 = casts
            .iter()
            .map(|c| {
                let mut v: Vec<u64> =
                    notes.iter().filter(|x| x.attacker == my_ent && x.ability == a.id && x.flags > 0 && x.t >= c.0 - HOLD_BEFORE_S && x.t <= c.1 + HOLD_AFTER_S).map(|x| x.victim).collect();
                v.sort();
                v.dedup();
                v.len() as f64
            })
            .sum::<f64>()
            / n;
        // What a cast catches on average at the build of its time.
        let usual: Vec<f64> = casts
            .iter()
            .filter_map(|c| {
                let step = c.0.max(0.0) as u32 / BUILD_STEP_S * BUILD_STEP_S;
                let sheet = sheets.entry(step).or_insert_with(|| deaths::build_in_match(data, &root, slot, step).and_then(|b| calc::compute(data, &b).ok()));
                let s = sheet.as_ref()?.abilities.iter().find(|x| x.class_name == a.class_name)?;
                area::covered(s).map(|_| area::targets(s))
            })
            .collect();
        let usual = (!usual.is_empty()).then(|| usual.iter().sum::<f64>() / usual.len() as f64);
        let per_cast = casts.iter().map(|c| c.2.len() as f64).sum::<f64>() / n;
        let verdict = match usual {
            Some(u) if per_cast >= u + USUAL_BAND => format!("Caught {per_cast:.1} enemies a cast, more than the {u:.1} a cast of it usually catches at your build."),
            Some(u) if per_cast <= u - USUAL_BAND => format!("Caught {per_cast:.1} enemies a cast, fewer than the {u:.1} a cast of it usually catches at your build: aim it where they group up."),
            Some(u) => format!("Caught {per_cast:.1} enemies a cast, about the {u:.1} a cast of it usually catches."),
            None => format!("Single-target: {per_cast:.1} enemies a cast."),
        };
        let verdict = if held > 0.0 { format!("{verdict} It held {held:.1} enemies a cast.") } else { verdict };
        cards.push(AbilityCard {
            ability: a.name.clone(),
            icon: a.icon.clone(),
            casts: casts.len() as u32,
            enemies_per_cast: per_cast,
            usual,
            damage_per_cast: casts.iter().map(|c| c.3).sum::<f64>() / n,
            held_per_cast: held,
            verdict,
        });
    }
    Ok(ReplayStudy { deaths: deaths_out, abilities: cards })
}
