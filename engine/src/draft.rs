//! Hero picks for a draft: which hero to play into the enemies picked so far, alongside the allies, in the lane,
//! and with the player's own record on each hero; which heroes to ban; and how the draft stands as it is.
//!
//! Every factor is a win rate read from recent matches and turned into log-odds, so factors add up. A hero's own
//! strength is its win rate; a matchup counts only what it adds over the two heroes' own strengths (a strong hero
//! beating a weak one is no counter). Each rate is pulled toward what is expected without it, the more the fewer
//! matches it rests on, so a 70% matchup from 20 matches counts for little.
//!
//! The weights were fitted on 8,700 Ascendant+ matches against statistics from the 30 days before them: added up
//! at full weight the factors made the predictions far too sure of themselves. With these weights the predicted
//! chances match what happened (teams given 45% won 45%, teams given 55% won 56%). Pairs of allies did not predict
//! wins at all and are left out. A draft decides little at this level: the best pick moves the chance by a few
//! points.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::data::GameData;
use crate::popular::HeroStat;

/// Matches of prior pulled into each rate: the hero's own rate, a matchup's rate, and the player's own rate on a hero.
const PRIOR_HERO: f64 = 1_000.0;
const PRIOR_PAIR: f64 = 3_000.0;
const PRIOR_MINE: f64 = 25.0;
/// What each factor counts for in a team's chance, fitted on matches (see the module notes). The player's own
/// record on a hero is a hero's win rate like the general one and counts the same.
const W_HERO: f64 = 0.27;
const W_COUNTER: f64 = 0.47;
const W_LANE: f64 = 0.46;
/// A hero needs this many matches in the window to be offered at all: fewer means it is not in the game yet.
const MIN_HERO_MATCHES: u64 = 200;
/// Factors smaller than this (in win rate points) are not listed as reasons.
const NOTEWORTHY_PP: f64 = 0.1;
/// Bans and reasons listed.
const BANS: usize = 5;
const REASONS: usize = 6;

/// A matchup or pairing row of the API: `a`'s wins in matches with or against `b`.
#[derive(Debug, Clone, Deserialize)]
pub struct Pair {
    pub a: u64,
    pub b: u64,
    pub wins: u64,
    pub matches: u64,
}

/// The statistics a draft is read from.
#[derive(Debug, Clone, Default)]
pub struct DraftData {
    pub heroes: Vec<HeroStat>,
    /// Hero against enemy hero, over the whole match.
    pub counters: Vec<Pair>,
    /// Hero against enemy hero, when both were in the same lane.
    pub lane_counters: Vec<Pair>,
    /// The player's own matches on each hero; empty without an account.
    pub mine: Vec<HeroStat>,
    /// How often each hero is banned.
    pub bans: Vec<(u64, u64)>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct DraftInput {
    /// Hero ids.
    pub allies: Vec<u64>,
    pub enemies: Vec<u64>,
    /// The enemies in the player's lane.
    pub lane: Vec<u64>,
    pub banned: Vec<u64>,
}

#[derive(Debug, Clone, Serialize)]
pub struct Reason {
    pub label: String,
    /// Win rate points this adds at an even match.
    pub points: f64,
    pub matches: u64,
}

#[derive(Debug, Clone, Serialize)]
pub struct Pick {
    pub hero_id: u64,
    pub hero: String,
    pub name: String,
    pub icon: String,
    pub hero_type: String,
    /// Win rate points the hero adds to the team's chance, against an even match.
    pub edge: f64,
    /// The team's chance with the hero picked, percent; `None` before both teams have a hero.
    pub team_rate: Option<f64>,
    /// Its win rate in general, percent, and the matches it rests on.
    pub base_rate: f64,
    pub matches: u64,
    /// The parts, in win rate points: the hero's own strength, against the enemies, in the lane, the player's own record.
    pub strength: f64,
    pub counter: f64,
    pub lane: f64,
    pub comfort: f64,
    /// The player's own matches and win rate on the hero.
    pub mine: Option<(u64, f64)>,
    pub reasons: Vec<Reason>,
}

#[derive(Debug, Clone, Serialize)]
pub struct Ban {
    pub hero_id: u64,
    pub name: String,
    pub icon: String,
    /// Win rate points the hero would add to the enemy team against the allies picked so far.
    pub edge: f64,
    /// Its share of all bans, percent.
    pub ban_share: f64,
    pub reasons: Vec<Reason>,
}

/// One enemy already picked and what it does to the allies picked so far.
#[derive(Debug, Clone, Serialize)]
pub struct Threat {
    pub hero_id: u64,
    pub name: String,
    pub icon: String,
    /// Win rate points it adds to the enemy team against the allies picked so far.
    pub edge: f64,
    pub reasons: Vec<Reason>,
}

#[derive(Debug, Clone, Serialize)]
pub struct DraftReport {
    pub picks: Vec<Pick>,
    pub bans: Vec<Ban>,
    pub threats: Vec<Threat>,
    /// The allies' chance to win against the enemies as picked so far, percent; `None` before both have a hero.
    pub team_rate: Option<f64>,
    /// What the allies' hero types say about the team (no frontline, all spirit damage, ...). Not counted.
    pub notes: Vec<String>,
}

fn logit(p: f64) -> f64 {
    let p = p.clamp(1e-4, 1.0 - 1e-4);
    (p / (1.0 - p)).ln()
}

fn sigmoid(x: f64) -> f64 {
    1.0 / (1.0 + (-x).exp())
}

/// Log-odds as win rate points around an even match.
fn points(x: f64) -> f64 {
    (sigmoid(x) - 0.5) * 100.0
}

/// A rate from wins and matches, pulled toward `expected` by `prior` matches.
fn shrunk(wins: u64, matches: u64, expected: f64, prior: f64) -> f64 {
    (wins as f64 + prior * expected) / (matches as f64 + prior)
}

struct Model<'a> {
    data: &'a GameData,
    strength: BTreeMap<u64, (f64, u64)>,
    counters: BTreeMap<(u64, u64), (u64, u64)>,
    lane: BTreeMap<(u64, u64), (u64, u64)>,
}

/// Rows keyed by (a, b), with the other direction filled in where the API gives only one.
fn by_pair(rows: &[Pair]) -> BTreeMap<(u64, u64), (u64, u64)> {
    let mut out = BTreeMap::new();
    for r in rows.iter().filter(|r| r.matches > 0 && r.a != r.b) {
        out.insert((r.a, r.b), (r.wins, r.matches));
    }
    for r in rows.iter().filter(|r| r.matches > 0 && r.a != r.b) {
        // b's wins are a's losses.
        out.entry((r.b, r.a)).or_insert((r.matches.saturating_sub(r.wins), r.matches));
    }
    out
}

impl Model<'_> {
    /// A hero's own strength in log-odds, and its matches.
    fn strength(&self, id: u64) -> (f64, u64) {
        self.strength.get(&id).copied().unwrap_or((0.0, 0))
    }

    /// What `a` gains against `b` beyond their own strengths, and the matches it rests on.
    fn against(&self, table: &BTreeMap<(u64, u64), (u64, u64)>, a: u64, b: u64) -> (f64, u64) {
        let expected = self.strength(a).0 - self.strength(b).0;
        match table.get(&(a, b)) {
            Some(&(wins, matches)) => (logit(shrunk(wins, matches, sigmoid(expected), PRIOR_PAIR)) - expected, matches),
            None => (0.0, 0),
        }
    }

    fn name(&self, id: u64) -> String {
        self.data.heroes.iter().find(|h| h.id == id).map(|h| h.name.clone()).unwrap_or_else(|| format!("Hero {id}"))
    }
}

fn reason(label: String, x: f64, matches: u64) -> Reason {
    Reason { label, points: points(x), matches }
}

/// The strongest reasons either way, the biggest first.
fn top(mut reasons: Vec<Reason>) -> Vec<Reason> {
    reasons.retain(|r| r.points.abs() >= NOTEWORTHY_PP);
    reasons.sort_by(|a, b| b.points.abs().total_cmp(&a.points.abs()));
    reasons.truncate(REASONS);
    reasons
}

/// Notes on the allies' team from their hero types.
fn team_notes(data: &GameData, allies: &[u64]) -> Vec<String> {
    let types: Vec<&str> = allies.iter().filter_map(|id| data.heroes.iter().find(|h| h.id == *id)).map(|h| h.hero_type.as_str()).collect();
    let count = |t: &str| types.iter().filter(|x| **x == t).count();
    let mut notes = vec![];
    if types.len() >= 3 && count("brawler") == 0 {
        notes.push("No brawler yet: a frontline hero would give the team someone to start and hold fights.".to_string());
    }
    if types.len() >= 3 && count("mystic") * 2 > types.len() {
        notes.push("Most of the team deals spirit damage: enemies can answer it all with spirit resist, so a gun hero spreads the damage.".to_string());
    }
    if types.len() >= 3 && count("marksman") + count("assassin") == types.len() {
        notes.push("The whole team relies on the gun: bullet resist and fire-rate slows answer all of it, so a spirit hero spreads the damage.".to_string());
    }
    notes
}

pub fn draft(data: &GameData, d: &DraftData, input: &DraftInput) -> DraftReport {
    let all_bans: u64 = d.bans.iter().map(|b| b.1).sum();
    let strength: BTreeMap<u64, (f64, u64)> =
        d.heroes.iter().filter(|h| h.matches > 0).map(|h| (h.hero_id, (logit(shrunk(h.wins, h.matches, 0.5, PRIOR_HERO)), h.matches))).collect();
    let m = Model { data, strength, counters: by_pair(&d.counters), lane: by_pair(&d.lane_counters) };
    let mine: BTreeMap<u64, (u64, u64)> = d.mine.iter().map(|h| (h.hero_id, (h.wins, h.matches))).collect();
    let taken: Vec<u64> = input.allies.iter().chain(&input.enemies).chain(&input.banned).copied().collect();
    let playable: Vec<&crate::data::Hero> =
        data.heroes.iter().filter(|h| m.strength.get(&h.id).is_some_and(|s| s.1 >= MIN_HERO_MATCHES)).collect();

    // What a hero adds to its team against the other team: its strength and its matchups, in team log-odds.
    let against_team = |hero: u64, others: &[u64], reasons: &mut Vec<Reason>, good: &str, bad: &str| -> f64 {
        let mut total = 0.0;
        for o in others {
            let (x, n) = m.against(&m.counters, hero, *o);
            total += W_COUNTER * x;
            reasons.push(reason(format!("{} {}", if x >= 0.0 { good } else { bad }, m.name(*o)), W_COUNTER * x, n));
        }
        total
    };

    // The draft as it stands: both teams' strengths and every matchup between them.
    let team = |allies: &[u64], enemies: &[u64]| -> f64 {
        let own = |t: &[u64]| t.iter().map(|h| W_HERO * m.strength(*h).0).sum::<f64>();
        let matchups: f64 = allies.iter().flat_map(|a| enemies.iter().map(move |e| (*a, *e))).map(|(a, e)| W_COUNTER * m.against(&m.counters, a, e).0).sum();
        own(allies) - own(enemies) + matchups
    };
    let both = !input.allies.is_empty() && !input.enemies.is_empty();
    let team_rate = both.then(|| sigmoid(team(&input.allies, &input.enemies)) * 100.0);

    let mut picks: Vec<Pick> = playable
        .iter()
        .filter(|h| !taken.contains(&h.id))
        .map(|h| {
            let (base, matches) = m.strength(h.id);
            let mut reasons = vec![];
            let own = W_HERO * base;
            reasons.push(reason(format!("Wins {:.1}% of its matches", sigmoid(base) * 100.0), own, matches));
            let counter = against_team(h.id, &input.enemies, &mut reasons, "Good into", "Weak into");
            let mut lane = 0.0;
            for e in &input.lane {
                let (x, n) = m.against(&m.lane, h.id, *e);
                lane += W_LANE * x;
                let word = if x >= 0.0 { "Wins the lane against" } else { "Loses the lane against" };
                reasons.push(reason(format!("{word} {}", m.name(*e)), W_LANE * x, n));
            }
            let record = mine.get(&h.id).copied();
            let comfort = record.map(|(wins, n)| W_HERO * (logit(shrunk(wins, n, sigmoid(base), PRIOR_MINE)) - base)).unwrap_or(0.0);
            if let Some((wins, n)) = record {
                reasons.push(reason(format!("Your record: {:.0}% in {n} matches", wins as f64 / n.max(1) as f64 * 100.0), comfort, n));
            }
            let edge = own + counter + lane + comfort;
            Pick {
                hero_id: h.id,
                hero: h.class_name.clone(),
                name: h.name.clone(),
                icon: h.icon.clone(),
                hero_type: h.hero_type.clone(),
                edge: points(edge),
                team_rate: both.then(|| sigmoid(team(&input.allies, &input.enemies) + edge) * 100.0),
                base_rate: sigmoid(base) * 100.0,
                matches,
                strength: points(own),
                counter: points(counter),
                lane: points(lane),
                comfort: points(comfort),
                mine: record.map(|(wins, n)| (n, wins as f64 / n.max(1) as f64 * 100.0)),
                reasons: top(reasons),
            }
        })
        .collect();
    // Every hero is returned, best first, so the list can be narrowed to the heroes a player plays.
    picks.sort_by(|a, b| b.edge.total_cmp(&a.edge));

    // A ban takes out the hero that would add the most to the enemy team against the allies picked so far.
    let banned_in = |id: u64| d.bans.iter().find(|b| b.0 == id).map(|b| b.1).unwrap_or(0);
    let mut bans: Vec<Ban> = playable
        .iter()
        .filter(|h| !taken.contains(&h.id))
        .map(|h| {
            let (base, matches) = m.strength(h.id);
            let mut reasons = vec![reason(format!("Wins {:.1}% of its matches", sigmoid(base) * 100.0), W_HERO * base, matches)];
            let edge = W_HERO * base + against_team(h.id, &input.allies, &mut reasons, "Strong into", "Weak into");
            reasons.retain(|r| r.points > 0.0);
            Ban {
                hero_id: h.id,
                name: h.name.clone(),
                icon: h.icon.clone(),
                edge: points(edge),
                ban_share: if all_bans > 0 { banned_in(h.id) as f64 / all_bans as f64 * 100.0 } else { 0.0 },
                reasons: top(reasons),
            }
        })
        .collect();
    bans.sort_by(|a, b| b.edge.total_cmp(&a.edge));
    bans.truncate(BANS);

    let mut threats: Vec<Threat> = input
        .enemies
        .iter()
        .map(|e| {
            let (base, matches) = m.strength(*e);
            let mut reasons = vec![reason(format!("Wins {:.1}% of its matches", sigmoid(base) * 100.0), W_HERO * base, matches)];
            let edge = W_HERO * base + against_team(*e, &input.allies, &mut reasons, "Strong into", "Weak into");
            Threat {
                hero_id: *e,
                name: m.name(*e),
                icon: data.heroes.iter().find(|h| h.id == *e).map(|h| h.icon.clone()).unwrap_or_default(),
                edge: points(edge),
                reasons: top(reasons),
            }
        })
        .collect();
    threats.sort_by(|a, b| b.edge.total_cmp(&a.edge));

    DraftReport { picks, bans, threats, team_rate, notes: team_notes(data, &input.allies) }
}

fn rows(json: &str) -> Result<Vec<serde_json::Value>, String> {
    serde_json::from_str(json).map_err(|e| format!("The statistics service answered in an unexpected form: {e}"))
}

fn field(v: &serde_json::Value, key: &str) -> u64 {
    v.get(key).and_then(serde_json::Value::as_u64).unwrap_or(0)
}

/// The API's hero-counter-stats rows.
pub fn parse_counters(json: &str) -> Result<Vec<Pair>, String> {
    Ok(rows(json)?
        .iter()
        .map(|r| Pair { a: field(r, "hero_id"), b: field(r, "enemy_hero_id"), wins: field(r, "wins"), matches: field(r, "matches_played") })
        .collect())
}

/// The API's hero-ban-stats rows, summed over its buckets.
pub fn parse_bans(json: &str) -> Result<Vec<(u64, u64)>, String> {
    let mut out: BTreeMap<u64, u64> = BTreeMap::new();
    for r in rows(json)? {
        *out.entry(field(&r, "hero_id")).or_default() += field(&r, "bans");
    }
    Ok(out.into_iter().collect())
}
