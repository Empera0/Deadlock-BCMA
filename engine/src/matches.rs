//! Reads a match's metadata (deadlock-api `/v1/matches/{id}/metadata`) into a report per player:
//! what they bought and when, how their numbers grew, where their damage came from, and how the
//! calculator's own stats compare with what the game recorded.

use crate::build::BuildInput;
use crate::calc;
use crate::combo::{self, TargetDebuff};
use crate::counters::{self, Threat};
use crate::data::{GameData, Hero, Item};
use crate::fights::{self, Fight};
use serde::Serialize;
use serde_json::Value;
use std::collections::BTreeMap;

#[derive(Debug, Clone, Serialize)]
pub struct MatchReport {
    pub match_id: u64,
    /// Unix seconds.
    pub start_time: u64,
    pub duration_s: u32,
    pub winning_team: u32,
    pub players: Vec<PlayerReport>,
    /// Guardians, walkers, shrines and the patron, in the order they fell.
    pub objectives: Vec<Objective>,
    /// When the mid boss died and which team took the rejuvenator.
    pub mid_boss: Vec<MidBoss>,
    /// Every player from best to worst by the calculator's own score.
    pub ranking: Vec<Rank>,
    /// Team fights found from the players' positions, in order.
    pub fights: Vec<Fight>,
    /// Every death with who was around, where it happened and what it left the team short for.
    pub deaths: Vec<DeathReport>,
    /// The laning phase, lane by lane.
    pub lanes: Vec<LaneReport>,
    /// The match in a few sentences: the lanes, the soul lead, where it turned, who carried and what cost the loser.
    pub story: Vec<Insight>,
    /// Hero damage and kills between every pair of opposing players.
    pub duels: Vec<DuelCell>,
    /// Where things happened, for drawing on the minimap.
    pub map: MapData,
}

#[derive(Debug, Clone, Serialize)]
pub struct DuelCell {
    pub from: u32,
    pub to: u32,
    pub damage: f64,
    pub kills: u32,
}

/// A place on the minimap: both values run 0-1 from the left and from the top.
#[derive(Debug, Clone, Serialize)]
pub struct MapPoint {
    pub time: u32,
    pub left: f64,
    pub top: f64,
}

#[derive(Debug, Clone, Serialize)]
pub struct MapDeath {
    pub time: u32,
    pub victim: u32,
    pub killer: u32,
    pub left: f64,
    pub top: f64,
}

#[derive(Debug, Clone, Serialize)]
pub struct MapPath {
    pub slot: u32,
    /// One point every `MapData::step` seconds; points while dead are left out.
    pub points: Vec<MapPoint>,
}

#[derive(Debug, Clone, Serialize)]
pub struct MapData {
    pub image: String,
    /// Seconds between two points of a path.
    pub step: u32,
    pub paths: Vec<MapPath>,
    pub deaths: Vec<MapDeath>,
    /// The middle of each team fight, in the order of `MatchReport::fights`.
    pub fights: Vec<MapPoint>,
}

/// Where the objectives stand on the minimap, as (owning team, objective id, left, top). From the game's map data;
/// shrines and base guardians are not listed there.
const OBJECTIVE_SPOTS: [(u32, u64, f64, f64); 16] = [
    (0, 0, 0.4405, 0.96),
    (0, 9, 0.4405, 0.8736),
    (0, 5, 0.2083, 0.7202),
    (0, 7, 0.4401, 0.6615),
    (0, 8, 0.7617, 0.7353),
    (0, 1, 0.119, 0.5863),
    (0, 3, 0.5, 0.5833),
    (0, 4, 0.8274, 0.5923),
    (1, 0, 0.5595, 0.04),
    (1, 9, 0.5595, 0.1257),
    (1, 5, 0.2481, 0.2664),
    (1, 7, 0.5599, 0.3385),
    (1, 8, 0.7997, 0.2779),
    (1, 1, 0.1726, 0.4077),
    (1, 3, 0.5, 0.4167),
    (1, 4, 0.878, 0.4137),
];
/// The match counts as even when the teams are within one walker of each other, or when their souls are this
/// close (percent) while the average player is still below a full build's worth of souls.
const EVEN_WALKERS: u32 = 1;
const EVEN_SOULS_PCT: f64 = 5.0;
const FULL_BUILD_SOULS: f64 = 60_000.0;

/// A player this close to an objective in its last seconds helped take it. Objectives without a known position
/// are inside the base, so the patron's position is used with a wider reach.
const TAKER_REACH: f64 = 2000.0;
const TAKER_REACH_IN_BASE: f64 = 3400.0;
const TAKER_WINDOW_S: u32 = 20;
const TAKER_SECONDS: u32 = 3;

/// Minimap position back to world units.
fn in_world(left: f64, top: f64) -> (f64, f64) {
    (left * 2.0 * MAP_RADIUS - MAP_RADIUS, MAP_RADIUS - top * 2.0 * MAP_RADIUS)
}

/// The minimap covers a square of this many world units from the middle of the map to each edge.
const MAP_RADIUS: f64 = 10752.0;
const MAP_IMAGE: &str = "https://assets-bucket.deadlock-api.com/assets-api-res/images/maps/minimap_midtown_mid.png";
const MAP_STEP_S: u32 = 5;

/// World position to minimap position: x grows to the right, y grows upwards.
fn on_map(x: f64, y: f64) -> (f64, f64) {
    (((x + MAP_RADIUS) / (2.0 * MAP_RADIUS)).clamp(0.0, 1.0), ((MAP_RADIUS - y) / (2.0 * MAP_RADIUS)).clamp(0.0, 1.0))
}

/// An observation about how a player's items fit the match.
#[derive(Debug, Clone, Serialize)]
pub struct Insight {
    /// `good` | `bad` | `info`.
    pub tone: String,
    pub title: String,
    pub text: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct Objective {
    /// The team that owned (and lost) it.
    pub team: u32,
    pub name: String,
    pub lane: Option<u32>,
    /// `None` when it survived the match.
    pub destroyed_at: Option<u32>,
    /// Enemy players who were at it in its last seconds: the ones who took it.
    pub takers: Vec<u32>,
    /// Damage it took from players and from troopers over the whole match.
    pub player_damage: f64,
    pub trooper_damage: f64,
    /// Where it stands on the minimap (0-1 from the left and top); `None` when the position is not known.
    pub left: Option<f64>,
    pub top: Option<f64>,
}

/// An objective a player helped destroy.
#[derive(Debug, Clone, Serialize)]
pub struct Taken {
    pub time: u32,
    pub name: String,
}

/// The mid boss sits in the middle of the map. A player within this distance of it for a few of its last
/// seconds was at the pit; deaths a little further out still belong to the fight over it.
const PIT: (f64, f64) = (0.0, 0.0);
const PIT_REACH: f64 = 1800.0;
const PIT_DEATH_REACH: f64 = 3000.0;
const PIT_BEFORE_S: u32 = 30;
const PIT_AFTER_S: u32 = 15;
const PIT_SECONDS: u32 = 3;
/// How long the rejuvenator stays on the team that claimed it.
const REJUVENATOR_S: u32 = 180;

/// A lane: who was in it and how the laning phase went between the two sides.
#[derive(Debug, Clone, Serialize)]
pub struct LaneReport {
    /// The game's number for the lane, and where it runs on the minimap (West, Middle, East).
    pub lane: u32,
    pub name: String,
    /// In team order.
    pub sides: Vec<LaneSide>,
    /// The team ahead in souls by a clear margin at the end of the laning phase.
    pub winner: Option<u32>,
}

#[derive(Debug, Clone, Serialize)]
pub struct LaneSide {
    pub players: Vec<u32>,
    pub souls: f64,
    pub last_hits: u32,
    pub denies: u32,
    /// Kills on the lane opponents and hero damage dealt to them in the laning phase.
    pub kills: u32,
    pub damage: f64,
    /// When this side's guardian in the lane fell.
    pub guardian_lost_at: Option<u32>,
    /// This side's guardian: when it was first hit, and the damage it took from players and from troopers
    /// over the match.
    pub guardian_first_hit: Option<u32>,
    pub guardian_player_damage: f64,
    pub guardian_trooper_damage: f64,
    /// Souls of the side at each snapshot of the laning phase.
    pub souls_at: Vec<f64>,
}

/// One player's laning phase in numbers, from the game's snapshots up to the end of it.
#[derive(Debug, Clone, Default, Serialize)]
pub struct LaneDetail {
    /// Souls at each snapshot of the laning phase (3:00, 6:00, 9:00).
    pub souls_at: Vec<f64>,
    /// Trooper last hits out of the troopers that could have been last hit.
    pub trooper_last_hits: u32,
    pub possible_last_hits: u32,
    pub denies: u32,
    /// Souls from denies; these are shared with the lane partner, so a player without a deny can have some.
    pub deny_souls: f64,
    pub kills: u32,
    pub deaths: u32,
    pub assists: u32,
    pub hero_damage: f64,
    pub damage_taken: f64,
    pub healing: f64,
    /// Damage to objectives: in the laning phase that is the enemy guardian.
    pub guardian_damage: f64,
    /// Neutral camps: creeps killed and the souls they gave.
    pub camp_kills: u32,
    pub camp_souls: f64,
    /// Souls from boxes and statues, and the permanent power-ups picked up from golden statues.
    pub box_souls: f64,
    pub statue_buffs: u32,
    pub souls_lost_on_death: f64,
    /// Shots at enemy heroes that hit, percent; 0 when none were fired.
    pub accuracy_pct: f64,
}

/// A trip to another lane during the laning phase.
#[derive(Debug, Clone, Serialize)]
pub struct Roam {
    pub start: u32,
    pub seconds: u32,
    pub lane: u32,
    pub name: String,
    /// Enemies who died in that lane while the player was there, and how many of them the player killed.
    pub enemy_deaths: u32,
    pub kills: u32,
    pub died: bool,
}

/// A player this close to a lane's line is in that lane.
const LANE_BAND: f64 = 1700.0;
/// A stay in another lane this long is a roam; shorter gaps inside one are bridged.
const ROAM_MIN_S: u32 = 12;
const ROAM_GAP_S: u32 = 4;
/// Roams are looked for from here to the end of the laning phase.
const ROAM_FROM_S: u32 = 120;
/// A lane is won with this much of a soul lead, percent.
const LANE_WON_PCT: f64 = 5.0;
/// After a fight the winner had this many more players left, the next seconds should buy something.
const FOLLOW_LEAD: u32 = 2;
const FOLLOW_S: u32 = 90;
/// A winner who then spends this long on neutral camps went to farm.
const FOLLOW_FARM_S: u32 = 15;

/// Distance from a point to a line drawn through the given points.
fn to_line(point: (f64, f64), line: &[(f64, f64)]) -> f64 {
    line.windows(2)
        .map(|w| {
            let (a, b) = (w[0], w[1]);
            let (dx, dy) = (b.0 - a.0, b.1 - a.1);
            let along = (((point.0 - a.0) * dx + (point.1 - a.1) * dy) / (dx * dx + dy * dy).max(1.0)).clamp(0.0, 1.0);
            ((point.0 - a.0 - along * dx).powi(2) + (point.1 - a.1 - along * dy).powi(2)).sqrt()
        })
        .fold(f64::INFINITY, f64::min)
}

/// Players this close when someone died were part of it.
const DEATH_REACH: f64 = 1600.0;
/// Further than this past the middle of the map, on the enemy's side, is deep.
const DEEP_M: f64 = 50.0;

/// One death: who was around, where it happened, and what the team then had to do without the player.
#[derive(Debug, Clone, Serialize)]
pub struct DeathReport {
    pub time: u32,
    pub slot: u32,
    pub killer: u32,
    /// Living enemies and teammates close by a second before.
    pub enemies: u32,
    pub allies: u32,
    /// Meters to the nearest living teammate; `None` when the whole team was dead.
    pub nearest_ally_m: Option<f64>,
    /// Meters past the middle of the map toward the enemy base; negative on the own side.
    pub depth_m: f64,
    pub alone: bool,
    pub deep: bool,
    /// The death was part of a team fight.
    pub in_fight: bool,
    /// Outside a team fight, with no teammate near, against two or more enemies or deep on the enemy's side.
    pub caught: bool,
    /// Team fights and mid boss kills that began while the player was still dead from this, outside a fight.
    pub cost: Vec<String>,
    pub left: f64,
    pub top: f64,
}

/// A mid boss kill: who was at the pit, who was not, and what the rejuvenator was turned into.
#[derive(Debug, Clone, Serialize)]
pub struct MidBoss {
    pub time: u32,
    pub team_killed: u32,
    pub team_claimed: u32,
    /// One team killed the boss, the other took the rejuvenator.
    pub stolen: bool,
    /// Per team: at the pit, alive but elsewhere, dead, and who died in the fight over it.
    pub sides: Vec<fights::Side>,
    /// Both teams had at least two players at the pit.
    pub contested: bool,
    pub late: bool,
    /// The match was still in the balance when the boss fell.
    pub even: bool,
    pub walkers: Vec<u32>,
    pub team_souls: Vec<f64>,
    /// What happened while the rejuvenator lasted: objectives the claiming team took and lost, kills by each
    /// team, and how the claiming team's soul lead moved.
    pub window_s: u32,
    pub taken: Vec<Taken>,
    pub lost: Vec<Taken>,
    pub kills: Vec<u32>,
    pub lead_change: f64,
}

/// One part of a player's score: what was measured, the player's value and the points it earned.
#[derive(Debug, Clone, Serialize)]
pub struct RankPart {
    pub label: String,
    pub value: f64,
    pub points: f64,
    pub max_points: f64,
}

#[derive(Debug, Clone, Serialize)]
pub struct Rank {
    pub slot: u32,
    /// 0-100.
    pub score: f64,
    pub parts: Vec<RankPart>,
}

/// A kill or a death on the timeline.
#[derive(Debug, Clone, Serialize)]
pub struct Duel {
    pub time: u32,
    /// The other player's slot: the victim of a kill, the killer of a death.
    pub other: u32,
}

#[derive(Debug, Clone, Serialize)]
pub struct Purchase {
    pub time: u32,
    pub item: String,
    pub name: String,
    pub icon: String,
    pub category: String,
    pub cost: u32,
    /// When the item left the inventory: sold, or used up by an upgrade.
    pub sold_at: Option<u32>,
}

#[derive(Debug, Clone, Serialize)]
pub struct AbilityStep {
    pub time: u32,
    /// Index among the hero's abilities.
    pub index: usize,
    pub name: String,
    pub icon: String,
    /// 0 when the ability is unlocked, 1-3 for its upgrades.
    pub tier: u8,
}

/// One of the game's periodic snapshots of a player, next to the calculator's numbers for the same moment.
#[derive(Debug, Clone, Serialize)]
pub struct Sample {
    pub time: u32,
    pub level: u32,
    pub net_worth: u32,
    pub kills: u32,
    pub deaths: u32,
    pub assists: u32,
    pub hero_damage: f64,
    pub damage_taken: f64,
    pub healing: f64,
    /// Damage to guardians, walkers, shrines and the patron.
    pub objective_damage: f64,
    pub creep_damage: f64,
    pub neutral_damage: f64,
    pub mitigated: f64,
    pub last_hits: u32,
    pub denies: u32,
    /// Souls earned so far by where they came from.
    pub souls: Vec<SourceAmount>,
    pub game_health: f64,
    pub game_spirit: f64,
    pub game_weapon: f64,
    /// `None` when the build could not be computed.
    pub calc_health: Option<f64>,
    pub calc_spirit: Option<f64>,
    pub calc_weapon: Option<f64>,
    /// The calculator's resists for the build at this moment, percent.
    pub bullet_resist: f64,
    pub spirit_resist: f64,
    /// Health, spirit power and weapon damage % from permanent pickups by this time (included in the calc values).
    pub pickup_health: f64,
    pub pickup_spirit: f64,
    pub pickup_weapon: f64,
    /// The build as it was at this moment.
    pub build: BuildInput,
}

#[derive(Debug, Clone, Serialize)]
pub struct SourceAmount {
    pub name: String,
    pub amount: f64,
}

#[derive(Debug, Clone, Serialize)]
pub struct PlayerReport {
    pub slot: u32,
    pub account_id: u64,
    /// The player's Steam name; empty until `apply_names` fills it in.
    pub player_name: String,
    pub team: u32,
    pub won: bool,
    pub hero: String,
    pub hero_name: String,
    pub hero_icon: String,
    /// The role the player filled (Frontliner, Support, Assassin, Marksman or Caster) and what decided it.
    pub role: String,
    pub role_why: String,
    pub kills: u32,
    pub deaths: u32,
    pub assists: u32,
    pub net_worth: u32,
    pub level: u32,
    pub last_hits: u32,
    pub denies: u32,
    pub hero_damage: f64,
    pub damage_taken: f64,
    pub healing: f64,
    pub objective_damage: f64,
    pub creep_damage: f64,
    pub neutral_damage: f64,
    pub mitigated: f64,
    /// The game's own MVP placement, for the few players it ranks.
    pub game_mvp_rank: Option<u32>,
    /// Counters the game tracks for its end-of-match awards: boxes broken, urns returned, and so on.
    pub macro_stats: Vec<SourceAmount>,
    /// Souls by where they came from, largest first.
    pub souls: Vec<SourceAmount>,
    pub kills_at: Vec<Duel>,
    pub deaths_at: Vec<Duel>,
    pub heal_prevented: f64,
    /// Healing kept for oneself, and healing plus barriers given to teammates.
    pub self_healing: f64,
    pub teammate_support: f64,
    /// Times the player fell to 10% health or less and survived.
    pub close_calls: u32,
    /// When a revive ability brought the player back instead of a death being recorded.
    pub revives_at: Vec<u32>,
    /// Late-game team fights the player was alive for but not in.
    pub late_fights_skipped: u32,
    /// Enemy objectives the player was at when they fell.
    pub objectives_taken: Vec<Taken>,
    /// Team fights the player took part in that their team won (lost fewer players) and lost.
    pub fights_won: u32,
    pub fights_lost: u32,
    /// Team fights in which the player was the first to die.
    pub first_deaths: u32,
    /// Deaths outside a team fight with no teammate near, to several enemies or deep on the enemy's side; and
    /// deaths outside a team fight that left the team a player short for a fight or a mid boss.
    pub caught_deaths: u32,
    pub costly_deaths: u32,
    /// Against the lane opponents in the laning phase: hero damage dealt to them and taken from them, kills on
    /// them and deaths to them.
    pub lane_damage_dealt: f64,
    pub lane_damage_taken: f64,
    pub lane_kills_on: u32,
    pub lane_deaths_to: u32,
    /// Trips to another lane during the laning phase.
    pub roams: Vec<Roam>,
    pub lane_detail: LaneDetail,
    /// Titles for leading the match in something, good and bad.
    pub titles: Vec<Title>,
    /// The parts of the score the player earned the most points from, and the ones the most points were lost in.
    pub strengths: Vec<SourceAmount>,
    pub gaps: Vec<SourceAmount>,
    /// Temporary power-ups picked up under the bridges, by kind, and the running count at every snapshot.
    pub bridge_buffs: Vec<SourceAmount>,
    pub bridge_buffs_at: Vec<f64>,
    /// Clearly won team fights after which the team took nothing and this player went to neutral camps.
    pub wins_farmed: u32,
    /// Mid boss kills the team went to that the player was alive for, and how many of those the player was at.
    pub mid_boss_alive: u32,
    pub mid_boss_present: u32,
    /// The lane the player played (read from where they stood early on, not from the assignment), and the
    /// enemies who played the same lane.
    pub lane: u32,
    pub lane_opponents: Vec<u32>,
    /// The laning phase, read from the snapshot at `lane_time` seconds.
    pub lane_time: u32,
    pub lane_souls: f64,
    /// Souls ahead of (or behind) the average lane opponent at that moment.
    pub lane_souls_diff: f64,
    pub lane_last_hits: u32,
    pub lane_denies: u32,
    pub lane_kills: u32,
    pub lane_deaths: u32,
    pub lane_hero_damage: f64,
    /// Trooper last hits out of the troopers that could have been last hit, percent.
    pub last_hit_pct: f64,
    pub trooper_last_hits: u32,
    pub possible_last_hits: u32,
    /// Souls and ability points left unspent, averaged over the match.
    pub avg_unspent_souls: f64,
    pub avg_unspent_ap: f64,
    /// Seconds spent waiting to respawn.
    pub dead_seconds: u32,
    pub souls_lost_on_death: f64,
    /// Seconds by what the player was doing: fighting heroes, troopers or objectives, neutral camps, out of combat, dead.
    pub time_split: Vec<SourceAmount>,
    /// Team fights the player was alive for, took part in, and stayed away from while the team needed the numbers.
    pub fights_possible: u32,
    pub fights_joined: u32,
    pub fights_missed: u32,
    /// How the final items fit teammates and enemies.
    pub insights: Vec<Insight>,
    /// When the match started calling for an item, and when it was bought.
    pub build_timing: Vec<Timing>,
    /// How well the build answered the enemy team, percent: the average credit of the entries above that count.
    /// `None` when the match called for nothing.
    pub counter_score: Option<f64>,
    /// Why each item that answers enemies was bought: whom it was aimed at when it was bought.
    pub item_reasons: Vec<ItemReason>,
    /// Extra damage teammates dealt because this player lowered the enemies' resists (an estimate), by source.
    pub enabled: Vec<EnabledDamage>,
    pub enabled_total: f64,
    /// Shots at enemy heroes, how many hit and how many of those were headshots.
    pub hero_shots: f64,
    pub hero_hits: f64,
    pub hero_headshots: f64,
    pub purchases: Vec<Purchase>,
    pub abilities: Vec<AbilityStep>,
    pub timeline: Vec<Sample>,
    /// Damage dealt to enemy heroes by source, largest first.
    pub damage_sources: Vec<SourceAmount>,
    pub heal_sources: Vec<SourceAmount>,
    /// Items the calculator's data does not know (a newer patch).
    pub unknown_items: u32,
    /// The build at the end of the match.
    pub build: BuildInput,
}

fn num(v: &Value, key: &str) -> f64 {
    v.get(key).and_then(Value::as_f64).unwrap_or(0.0)
}

fn int(v: &Value, key: &str) -> u32 {
    num(v, key).max(0.0) as u32
}

fn list<'a>(v: &'a Value, key: &str) -> &'a [Value] {
    v.get(key).and_then(Value::as_array).map(Vec::as_slice).unwrap_or(&[])
}

/// Where souls come from, by the game's source id. The names of 5 and 9-13 are inferred, not documented.
const SOUL_SOURCES: [(u64, &str); 13] = [
    (1, "Hero kills"),
    (6, "Assists"),
    (2, "Lane troopers"),
    (3, "Neutral camps"),
    (4, "Objectives"),
    (12, "Boxes and statues"),
    (5, "Treasure (urn, vaults)"),
    (7, "Denies"),
    (8, "Team bonus"),
    (10, "Trophy Collector"),
    (11, "Cultist Sacrifice"),
    (9, "Other (9)"),
    (13, "Other (13)"),
];

/// The counters behind the game's end-of-match awards that are not shown elsewhere, by award id.
const MACRO_STATS: [(u64, &str); 16] = [
    (11, "Boxes and statues broken"),
    (14, "Sinner's Sacrifice jackpots"),
    (13, "Urns returned"),
    (12, "Power-ups collected"),
    (9, "Soul orbs secured"),
    (10, "Denies"),
    (6, "Trooper last hits"),
    (7, "Neutral last hits"),
    (16, "Longest kill streak"),
    (28, "Headshots"),
    (22, "Bullet damage"),
    (24, "Ability damage"),
    (23, "Melee damage"),
    (29, "Headshot damage"),
    (30, "Close-range damage"),
    (31, "Long-range damage"),
];

/// Name and lane of an objective id.
fn objective(id: u64) -> (&'static str, Option<u32>) {
    match id {
        0 => ("Patron (final form)", None),
        1..=4 => ("Guardian", Some(id as u32)),
        5..=8 => ("Walker", Some(id as u32 - 4)),
        9 => ("Patron", None),
        10 | 11 => ("Shrine", None),
        12..=15 => ("Base guardians", Some(id as u32 - 11)),
        _ => ("Objective", None),
    }
}

fn soul_sources(sample: &Value) -> Vec<SourceAmount> {
    let rows = list(sample, "gold_sources");
    let mut out: Vec<SourceAmount> = SOUL_SOURCES
        .iter()
        .map(|(id, name)| SourceAmount {
            name: name.to_string(),
            amount: rows
                .iter()
                .filter(|r| r.get("source").and_then(Value::as_u64) == Some(*id))
                .map(|r| num(r, "gold") + num(r, "gold_orbs"))
                .sum(),
        })
        .collect();
    out.retain(|s| s.amount > 0.0);
    out
}

/// The two teams by their number in the match record.
const TEAM_NAMES: [&str; 2] = ["Hidden King", "Archmother"];
/// The turning point is looked for in the soul lead this long after a fight or a mid boss.
const SWING_S: u32 = 180;
/// A swing or a lead smaller than this is not worth a sentence.
const NOTABLE_SOULS: f64 = 2000.0;

fn listed(words: &[String]) -> String {
    match words {
        [] => String::new(),
        [one] => one.clone(),
        [first @ .., last] => format!("{} and {last}", first.join(", ")),
    }
}

/// Tells the match in a few sentences.
#[allow(clippy::too_many_arguments)]
fn story(
    players: &[PlayerReport],
    lanes: &[LaneReport],
    fights: &[Fight],
    mid_boss: &[MidBoss],
    deaths: &[DeathReport],
    ranking: &[Rank],
    winner: u32,
    duration: u32,
) -> Vec<Insight> {
    let (won, lost) = (winner.min(1) as usize, 1 - winner.min(1) as usize);
    let mut out: Vec<Insight> = vec![];
    let mut add = |tone: &str, title: &str, text: String| out.push(Insight { tone: tone.into(), title: title.into(), text });
    // The winner's soul lead at a moment, read off the lines between the snapshots.
    let souls = |p: &PlayerReport, time: u32| -> f64 {
        let mut before = (0.0, 0.0);
        for sample in &p.timeline {
            let point = (sample.time as f64, sample.net_worth as f64);
            if sample.time >= time {
                return before.1 + (point.1 - before.1) * (time as f64 - before.0) / (point.0 - before.0).max(1.0);
            }
            before = point;
        }
        before.1
    };
    let lead = |time: u32| -> f64 { players.iter().map(|p| souls(p, time) * if p.team as usize == won { 1.0 } else { -1.0 }).sum() };
    let times: Vec<u32> = players.first().map(|p| p.timeline.iter().map(|s| s.time).collect()).unwrap_or_default();

    // --- The lanes ---
    if !lanes.is_empty() {
        let of = |team: Option<u32>| -> Vec<String> { lanes.iter().filter(|l| l.winner == team).map(|l| l.name.clone()).collect() };
        let mut parts: Vec<String> = vec![];
        for team in [won, lost] {
            let names = of(Some(team as u32));
            if !names.is_empty() {
                parts.push(format!("{} won {}", TEAM_NAMES[team], listed(&names)));
            }
        }
        let even = of(None);
        if !even.is_empty() {
            parts.push(format!("{} {} even", listed(&even), if even.len() == 1 { "was" } else { "were" }));
        }
        let at_nine = lead(LANE_PHASE_S);
        let ahead = if at_nine >= 0.0 { won } else { lost };
        add("info", "Laning phase", format!("{}. At {} {} led by {} souls.", parts.join("; "), clock(LANE_PHASE_S), TEAM_NAMES[ahead], n(at_nine.abs())));
    }

    // --- The soul lead over the match ---
    if !times.is_empty() {
        let series: Vec<(u32, f64)> = times.iter().map(|t| (*t, lead(*t))).collect();
        let best = series.iter().copied().fold((0, 0.0), |a, b| if b.1 > a.1 { b } else { a });
        let worst = series.iter().copied().fold((0, 0.0), |a, b| if b.1 < a.1 { b } else { a });
        // The last snapshot at which the loser was still ahead.
        let behind_until = series.iter().filter(|x| x.1 < 0.0).map(|x| x.0).max();
        let text = match behind_until {
            Some(until) if worst.1 <= -NOTABLE_SOULS => format!(
                "{} led by up to {} souls (at {}){}. {}'s largest lead was {} at {}.",
                TEAM_NAMES[lost],
                n(-worst.1),
                clock(worst.0),
                if until > worst.0 { format!(" and was still ahead at {}", clock(until)) } else { ", the last time it was ahead".to_string() },
                TEAM_NAMES[won],
                n(best.1.max(0.0)),
                clock(best.0)
            ),
            _ => format!("{} was ahead in souls for practically the whole match; the largest lead was {} at {}.", TEAM_NAMES[won], n(best.1.max(0.0)), clock(best.0)),
        };
        add("info", "Soul lead", text);
    }

    // --- The biggest swing: the fight or mid boss after which the lead moved the most toward the winner ---
    let swing = |from: u32, to: u32| lead((to + SWING_S).min(duration)) - lead(from);
    let fight_turn = fights
        .iter()
        .filter(|f| f.winner == Some(won as u32))
        .map(|f| (swing(f.start, f.end), f))
        .max_by(|a, b| a.0.total_cmp(&b.0));
    let boss_turn = mid_boss.iter().filter(|b| b.team_claimed as usize == won).map(|b| (swing(b.time, b.time), b)).max_by(|a, b| a.0.total_cmp(&b.0));
    match (fight_turn, boss_turn) {
        (Some((moved, f)), boss) if moved >= NOTABLE_SOULS && boss.as_ref().is_none_or(|b| b.0 <= moved) => {
            let (own, other) = (&f.sides[won], &f.sides[lost]);
            let taken: Vec<String> = f.follow.as_ref().map(|x| x.taken.clone()).unwrap_or_default();
            add(
                "good",
                "Biggest swing",
                format!(
                    "The team fight at {} ({} against {}, {} killed for {} lost): over the next three minutes the soul lead moved {} toward {}{}.",
                    clock(f.start),
                    own.present.len(),
                    other.present.len(),
                    other.deaths.len(),
                    own.deaths.len(),
                    n(moved),
                    TEAM_NAMES[won],
                    if taken.is_empty() { String::new() } else { format!(", with {} taken", listed(&taken)) }
                ),
            );
        }
        (_, Some((moved, b))) if moved >= NOTABLE_SOULS => {
            add(
                "good",
                "Biggest swing",
                format!(
                    "The mid boss at {}{}: over the next three minutes the soul lead moved {} toward {}{}.",
                    clock(b.time),
                    if b.stolen { ", stolen from the team that killed it" } else { "" },
                    n(moved),
                    TEAM_NAMES[won],
                    if b.taken.is_empty() { String::new() } else { format!(", with {} taken", listed(&b.taken.iter().map(|t| t.name.clone()).collect::<Vec<_>>())) }
                ),
            );
        }
        _ => {}
    }

    // --- Who carried ---
    let ranked: Vec<&PlayerReport> = ranking.iter().filter_map(|r| players.iter().find(|p| p.slot == r.slot)).collect();
    if let Some(best) = ranked.iter().find(|p| p.team as usize == won) {
        let score = ranking.iter().find(|r| r.slot == best.slot).map(|r| r.score).unwrap_or(0.0);
        let titles: Vec<String> = best.titles.iter().filter(|t| t.tone == "good").map(|t| t.name.clone()).collect();
        let mut text = format!(
            "{} ({}, {:.1} points) led {}: scored most from {}.{}",
            best.hero_name,
            best.role.to_lowercase(),
            score,
            TEAM_NAMES[won],
            listed(&best.strengths.iter().map(|x| x.name.clone()).collect::<Vec<_>>()),
            if titles.is_empty() { String::new() } else { format!(" Titles: {}.", titles.join(", ")) }
        );
        if let Some(top) = ranked.first().filter(|p| p.team as usize == lost) {
            text.push_str(&format!(" The best player of the match, {}, was on the losing side.", top.hero_name));
        }
        add("good", "Carried by", text);
    }

    // --- What cost the loser ---
    let losers: Vec<&PlayerReport> = players.iter().filter(|p| p.team as usize == lost).collect();
    // Named only when one player stands out with it.
    let most = |value: &dyn Fn(&PlayerReport) -> u32| losers.iter().max_by_key(|p| value(p)).filter(|p| value(p) > 1).map(|p| format!("{} {}", p.hero_name, value(p)));
    let mut costs: Vec<String> = vec![];
    let decided = fights.iter().filter(|f| f.winner.is_some()).count();
    let fights_lost = fights.iter().filter(|f| f.winner == Some(won as u32)).count();
    if decided > 0 && fights_lost * 2 > decided {
        costs.push(format!("it lost {fights_lost} of the {decided} team fights that had a winner"));
    }
    let short = deaths.iter().filter(|d| !d.cost.is_empty() && losers.iter().any(|p| p.slot == d.slot)).count();
    if short > 0 {
        costs.push(format!(
            "{short} death(s) outside a fight left it a player short for a fight or a mid boss{}",
            most(&|p| p.costly_deaths).map(|m| format!(" (most: {m})")).unwrap_or_default()
        ));
    }
    let skipped: u32 = losers.iter().map(|p| p.late_fights_skipped).sum();
    if skipped > 0 {
        costs.push(format!(
            "living players stayed out of late-game team fights {skipped} time(s){}",
            most(&|p| p.late_fights_skipped).map(|m| format!(" (most: {m})")).unwrap_or_default()
        ));
    }
    let bosses = mid_boss.iter().filter(|b| b.team_claimed as usize == won).count();
    if bosses > 0 {
        let stolen = mid_boss.iter().filter(|b| b.team_claimed as usize == won && b.stolen).count();
        costs.push(format!("{} took {bosses} of the {} rejuvenator(s){}", TEAM_NAMES[won], mid_boss.len(), if stolen > 0 { format!(", {stolen} of them stolen") } else { String::new() }));
    }
    let lanes_lost = lanes.iter().filter(|l| l.winner == Some(won as u32)).count();
    if lanes_lost >= 2 {
        costs.push(format!("it lost {lanes_lost} of the {} lanes", lanes.len()));
    }
    if !costs.is_empty() {
        let mut text = costs.join("; ");
        if let Some(first) = text.get(..1) {
            text = first.to_uppercase() + &text[1..];
        }
        add("bad", &format!("What cost {}", TEAM_NAMES[lost]), format!("{text}."));
    }
    out
}

/// A title for leading the match in something.
#[derive(Debug, Clone, Serialize)]
pub struct Title {
    pub name: String,
    /// What it was given for, e.g. `Most objective damage`.
    pub what: String,
    /// The player's number, ready to show.
    pub value: String,
    /// `good` | `bad`.
    pub tone: String,
}

/// How many of the best and of the worst parts of a score are named.
const STRENGTHS: usize = 3;
const GAPS: usize = 2;

/// A part of the score as something a player is good at.
fn part_name(label: &str) -> String {
    match label {
        "Late-game fights skipped" => "showing up to late-game fights".to_string(),
        "Deaths and revives" => "staying alive".to_string(),
        "Average unspent souls" => "spending souls".to_string(),
        "Team fights joined %" => "joining team fights".to_string(),
        "Kill participation %" => "kill participation".to_string(),
        "Items bought against the enemy team %" => "items against the enemy team".to_string(),
        "Deaths that left the team short" => "not dying right before fights".to_string(),
        "Won a fight, then farmed" => "following up won fights".to_string(),
        "Mid boss attendance %" => "being at the mid boss".to_string(),
        "Damage absorbed (taken + mitigated)" => "absorbing damage".to_string(),
        other => other.to_lowercase(),
    }
}

/// Gives a title to whoever alone leads the match in each thing worth naming, and says where each player's
/// score came from.
fn award_titles(players: &mut [PlayerReport], ranking: &[Rank], mid_boss: &[MidBoss]) {
    let stat = |p: &PlayerReport, name: &str| p.macro_stats.iter().find(|s| s.name == name).map(|s| s.amount).unwrap_or(0.0);
    // (title, what for, tone, value per player, percent?, less is better?)
    let contests: Vec<(&str, &str, &str, Vec<f64>, bool, bool)> = vec![
        ("Heavy Hitter", "Most hero damage", "good", players.iter().map(|p| p.hero_damage).collect(), false, false),
        ("Siege Engine", "Most objective damage", "good", players.iter().map(|p| p.objective_damage).collect(), false, false),
        ("Demolisher", "At the most enemy objectives when they fell", "good", players.iter().map(|p| p.objectives_taken.len() as f64).collect(), false, false),
        ("Executioner", "Most kills", "good", players.iter().map(|p| p.kills as f64).collect(), false, false),
        ("Playmaker", "Most assists", "good", players.iter().map(|p| p.assists as f64).collect(), false, false),
        ("Iron Wall", "Most damage absorbed (taken and mitigated)", "good", players.iter().map(|p| p.damage_taken + p.mitigated).collect(), false, false),
        ("Lifeline", "Most healing and barriers for teammates", "good", players.iter().map(|p| p.teammate_support).collect(), false, false),
        ("Armor Breaker", "Most damage enabled for teammates", "good", players.iter().map(|p| p.enabled_total).collect(), false, false),
        ("Soul Baron", "Most souls", "good", players.iter().map(|p| p.net_worth as f64).collect(), false, false),
        ("Lane Bully", "Biggest soul lead in the lane", "good", players.iter().map(|p| p.lane_souls_diff).collect(), false, false),
        ("Treasure Hunter", "Most boxes and statues broken", "good", players.iter().map(|p| stat(p, "Boxes and statues broken")).collect(), false, false),
        ("Urn Runner", "Most urns returned", "good", players.iter().map(|p| stat(p, "Urns returned")).collect(), false, false),
        ("Deadeye", "Most headshots on heroes", "good", players.iter().map(|p| p.hero_headshots).collect(), false, false),
        (
            "Always There",
            "Joined the largest share of team fights",
            "good",
            players.iter().map(|p| if p.fights_possible >= 3 { p.fights_joined as f64 / p.fights_possible as f64 * 100.0 } else { 0.0 }).collect(),
            true,
            false,
        ),
        ("Well Prepared", "Best items against the enemy team", "good", players.iter().map(|p| p.counter_score.unwrap_or(0.0)).collect(), true, false),
        ("Untouchable", "Fewest deaths", "good", players.iter().map(|p| (p.deaths as usize + p.revives_at.len()) as f64).collect(), false, true),
        ("Caught Out", "Caught alone the most", "bad", players.iter().map(|p| p.caught_deaths as f64).collect(), false, false),
        ("Sightseer", "Skipped the most late-game team fights", "bad", players.iter().map(|p| p.late_fights_skipped as f64).collect(), false, false),
        ("Hoarder", "Most souls left unspent on average", "bad", players.iter().map(|p| p.avg_unspent_souls).collect(), false, false),
        ("Spectator", "Most time spent dead", "bad", players.iter().map(|p| p.dead_seconds as f64).collect(), false, false),
    ];
    let mut won: Vec<(usize, Title)> = vec![];
    for (name, what, tone, values, percent, less) in contests {
        let best = if less { values.iter().copied().fold(f64::INFINITY, f64::min) } else { values.iter().copied().fold(f64::NEG_INFINITY, f64::max) };
        let leaders: Vec<usize> = (0..values.len()).filter(|i| values[*i] == best).collect();
        // A title needs one clear leader, and something to lead in.
        if leaders.len() != 1 || (!less && best <= 0.0) {
            continue;
        }
        let value = if name == "Spectator" { clock(best as u32) } else if percent { format!("{}%", n(best)) } else { n(best) };
        won.push((leaders[0], Title { name: name.into(), what: what.into(), value, tone: tone.into() }));
    }
    // Taking the rejuvenator from under the team that killed the boss, alone.
    for b in mid_boss.iter().filter(|b| b.stolen) {
        let there = b.sides.get(b.team_claimed as usize).map(|s| s.present.as_slice()).unwrap_or(&[]);
        if let ([slot], Some(i)) = (there, there.first().and_then(|slot| players.iter().position(|p| p.slot == *slot))) {
            let _ = slot;
            let title = Title { name: "Thief".into(), what: "Stole the rejuvenator alone".into(), value: format!("at {}", clock(b.time)), tone: "good".into() };
            if !won.iter().any(|w| w.0 == i && w.1.name == "Thief") {
                won.push((i, title));
            }
        }
    }
    for (i, p) in players.iter_mut().enumerate() {
        p.titles = won.iter().filter(|w| w.0 == i).map(|w| w.1.clone()).collect();
        let Some(rank) = ranking.iter().find(|r| r.slot == p.slot) else { continue };
        let mut earned: Vec<&RankPart> = rank.parts.iter().collect();
        earned.sort_by(|a, b| b.points.total_cmp(&a.points));
        p.strengths = earned.iter().take(STRENGTHS).map(|part| SourceAmount { name: part_name(&part.label), amount: part.points }).collect();
        let mut lost: Vec<&RankPart> = rank.parts.iter().collect();
        lost.sort_by(|a, b| (b.max_points - b.points).total_cmp(&(a.max_points - a.points)));
        p.gaps = lost.iter().take(GAPS).map(|part| SourceAmount { name: part_name(&part.label), amount: part.max_points - part.points }).collect();
    }
}

/// A build with this share of its souls in vitality items is a frontline build; a brawler needs less.
const FRONTLINE_VITALITY_PCT: f64 = 50.0;
const BRAWLER_VITALITY_PCT: f64 = 25.0;
/// A hero built this far against its class plays the other damage role.
const OFF_CLASS_PCT: f64 = 55.0;
/// Team items, or this share of healing and barriers for teammates next to the player's own hero damage, make a support.
const SUPPORT_TEAM_ITEMS: usize = 2;
const SUPPORT_SHARE_PCT: f64 = 25.0;

/// Decides the role each player filled, from the game's own hero class, where the souls went and what the
/// player did for teammates.
fn assign_roles(data: &GameData, players: &mut [PlayerReport]) {
    for p in players.iter_mut() {
        let class = data.heroes.iter().find(|h| h.class_name == p.hero).map(|h| h.hero_type.as_str()).unwrap_or("");
        let kept: Vec<&Purchase> = p.purchases.iter().filter(|x| x.sold_at.is_none()).collect();
        let spent = |category: &str| kept.iter().filter(|x| x.category == category).map(|x| x.cost as f64).sum::<f64>() + 0.0;
        let total = (spent("weapon") + spent("vitality") + spent("spirit")).max(1.0);
        let (weapon, vitality, spirit) = (spent("weapon") / total * 100.0, spent("vitality") / total * 100.0, spent("spirit") / total * 100.0);
        let team_items = kept.iter().filter(|x| TEAM_ITEMS.contains(&x.name.as_str())).count();
        let support = p.teammate_support / (p.teammate_support + p.hero_damage).max(1.0) * 100.0;
        let build = format!("souls in items: {weapon:.0}% weapon, {vitality:.0}% vitality, {spirit:.0}% spirit");
        let (role, why) = if team_items >= SUPPORT_TEAM_ITEMS {
            ("Support", format!("{team_items} team items in the final build"))
        } else if support >= SUPPORT_SHARE_PCT {
            ("Support", format!("healing and barriers for teammates were {support:.0}% of what the player put out"))
        } else if vitality >= FRONTLINE_VITALITY_PCT || (class == "brawler" && vitality >= BRAWLER_VITALITY_PCT) {
            ("Frontliner", if class == "brawler" { format!("a brawler; {build}") } else { build.clone() })
        } else if class == "assassin" {
            ("Assassin", format!("an assassin; {build}"))
        } else if class == "marksman" && spirit < OFF_CLASS_PCT {
            ("Marksman", format!("a marksman; {build}"))
        } else if class == "mystic" && weapon < OFF_CLASS_PCT {
            ("Caster", format!("a mystic; {build}"))
        } else if weapon > spirit {
            ("Marksman", format!("built for the gun; {build}"))
        } else {
            ("Caster", format!("built for abilities; {build}"))
        };
        p.role = role.to_string();
        p.role_why = why;
    }
}

/// How much a measure matters for a role, as a multiple of its base weight; measures not listed keep theirs.
/// A frontliner is there to soak damage and be in every fight, a support to keep teammates alive and set up
/// their damage, an assassin to get kills without dying, a marksman to deal damage, take objectives and scale,
/// a caster to deal damage and soften targets.
fn role_weights(role: &str) -> &'static [(&'static str, f64)] {
    match role {
        "Frontliner" => &[
            ("Damage absorbed (taken + mitigated)", 2.5),
            ("Team fights joined %", 1.2),
            ("Hero damage", 0.7),
            ("Deaths and revives", 0.6),
            ("Souls", 0.8),
        ],
        "Support" => &[
            ("Healing and barriers for teammates", 2.5),
            ("Damage enabled for teammates", 1.6),
            ("Kill participation %", 1.3),
            ("Hero damage", 0.6),
            ("Objective damage", 0.8),
            ("Souls", 0.7),
            ("Damage absorbed (taken + mitigated)", 0.8),
        ],
        "Assassin" => &[
            ("Kill participation %", 1.3),
            ("Deaths and revives", 1.3),
            ("Hero damage", 1.1),
            ("Objective damage", 0.8),
            ("Damage absorbed (taken + mitigated)", 0.4),
        ],
        "Marksman" => &[
            ("Hero damage", 1.2),
            ("Objective damage", 1.3),
            ("Souls", 1.2),
            ("Damage absorbed (taken + mitigated)", 0.4),
            ("Healing and barriers for teammates", 0.5),
        ],
        "Caster" => &[("Hero damage", 1.15), ("Damage enabled for teammates", 1.3), ("Damage absorbed (taken + mitigated)", 0.6)],
        _ => &[],
    }
}

/// Scores every player 0-100 from shares of the best value in the match, and orders them best first.
/// What each measure is worth depends on the player's role.
fn rank(players: &[PlayerReport]) -> Vec<Rank> {
    let team_kills = |team: u32| players.iter().filter(|p| p.team == team).map(|p| p.kills).sum::<u32>().max(1) as f64;
    let stat = |p: &PlayerReport, name: &str| p.macro_stats.iter().find(|s| s.name == name).map(|s| s.amount).unwrap_or(0.0);
    // (label, points, value per player, whether less is better)
    // Last hits are left out on purpose: they already show up as souls.
    // Souls and deaths weigh little: farming alone should not carry a score, and some heroes are simply
    // harder to kill. What a player takes from the map for good is scored one measure each.
    let worst_lane = players.iter().map(|p| p.lane_souls_diff).fold(0.0, f64::min);
    // A player the match asked nothing of is given the average of the others rather than nothing.
    let countered: Vec<f64> = players.iter().filter_map(|p| p.counter_score).collect();
    let typical_counter = if countered.is_empty() { 0.0 } else { countered.iter().sum::<f64>() / countered.len() as f64 };
    // The same for the mid boss: a player who was dead for every one of them is not scored below the rest.
    let attendance = |p: &PlayerReport| (p.mid_boss_alive > 0).then(|| p.mid_boss_present as f64 / p.mid_boss_alive as f64 * 100.0);
    let attended: Vec<f64> = players.iter().filter_map(attendance).collect();
    let typical_attendance = if attended.is_empty() { 0.0 } else { attended.iter().sum::<f64>() / attended.len() as f64 };
    let measures: Vec<(&str, f64, Vec<f64>, bool)> = vec![
        ("Hero damage", 13.0, players.iter().map(|p| p.hero_damage).collect(), false),
        ("Objective damage", 11.0, players.iter().map(|p| p.objective_damage).collect(), false),
        (
            "Team fights joined %",
            11.0,
            players.iter().map(|p| if p.fights_possible > 0 { p.fights_joined as f64 / p.fights_possible as f64 * 100.0 } else { 0.0 }).collect(),
            false,
        ),
        (
            "Kill participation %",
            9.0,
            players.iter().map(|p| (p.kills + p.assists) as f64 / team_kills(p.team) * 100.0).collect(),
            false,
        ),
        // Extra damage teammates dealt thanks to this player's resist reductions. Effects that need a headshot,
        // a melee hit or an ability to land count half: the estimate assumes they always applied.
        (
            "Damage enabled for teammates",
            5.0,
            players.iter().map(|p| p.enabled.iter().map(|e| e.bonus * if e.needs.is_empty() { 1.0 } else { 0.5 }).sum()).collect(),
            false,
        ),
        // Resists, resist reduction, healing reduction and answers to enemy abilities: bought, and bought in time.
        ("Items bought against the enemy team %", 6.0, players.iter().map(|p| p.counter_score.unwrap_or(typical_counter)).collect(), false),
        // Staying out of a late-game team fight is the costliest mistake, whatever the player did instead.
        ("Late-game fights skipped", 8.0, players.iter().map(|p| p.late_fights_skipped as f64).collect(), true),
        ("Damage absorbed (taken + mitigated)", 5.0, players.iter().map(|p| p.damage_taken + p.mitigated).collect(), false),
        ("Souls", 12.0, players.iter().map(|p| p.net_worth as f64).collect(), false),
        // A revive counts as going down: the hero was killed once, the ability undid it.
        ("Deaths and revives", 4.0, players.iter().map(|p| (p.deaths as usize + p.revives_at.len()) as f64).collect(), true),
        // Not how often a player died but when: outside a fight, with a fight or a mid boss starting before the respawn.
        ("Deaths that left the team short", 5.0, players.iter().map(|p| p.costly_deaths as f64).collect(), true),
        // A fight won by two or more players that the team took nothing from, with this player off to the camps.
        ("Won a fight, then farmed", 3.0, players.iter().map(|p| p.wins_farmed as f64).collect(), true),
        ("Mid boss attendance %", 3.0, players.iter().map(|p| attendance(p).unwrap_or(typical_attendance)).collect(), false),
        // The lane lead can be negative, so it is measured from the worst lane in the match.
        ("Lane lead over the lane opponent", 5.0, players.iter().map(|p| p.lane_souls_diff - worst_lane).collect(), false),
        // Souls sitting in the pocket are strength the team did not have.
        ("Average unspent souls", 4.0, players.iter().map(|p| p.avg_unspent_souls).collect(), true),
        ("Healing and barriers for teammates", 4.0, players.iter().map(|p| p.teammate_support).collect(), false),
        // All healing, self healing included.
        ("Healing", 5.0, players.iter().map(|p| p.healing).collect(), false),
        ("Boxes and statues broken", 4.0, players.iter().map(|p| stat(p, "Boxes and statues broken")).collect(), false),
        ("Power-ups collected", 4.0, players.iter().map(|p| stat(p, "Power-ups collected")).collect(), false),
        ("Urns returned", 3.0, players.iter().map(|p| stat(p, "Urns returned")).collect(), false),
        ("Sinner's Sacrifice jackpots", 3.0, players.iter().map(|p| stat(p, "Sinner's Sacrifice jackpots")).collect(), false),
    ];
    let mut out: Vec<Rank> = players
        .iter()
        .enumerate()
        .map(|(i, p)| {
            // The weights above are relative: they are shifted for the player's role, then scaled so that a
            // perfect score is 100 for every role.
            let shifts = role_weights(&p.role);
            let weight = |label: &str, points: f64| points * shifts.iter().find(|s| s.0 == label).map(|s| s.1).unwrap_or(1.0);
            let scale = 100.0 / measures.iter().map(|m| weight(m.0, m.1)).sum::<f64>();
            let parts: Vec<RankPart> = measures
                .iter()
                .map(|(label, points, values, less_is_better)| {
                    let max_points = &(weight(label, *points) * scale);
                    let best = values.iter().copied().fold(0.0, f64::max);
                    let share = if best > 0.0 { values[i] / best } else { 0.0 };
                    let share = if *less_is_better { 1.0 - share } else { share };
                    // The lane lead is shown as it was, not shifted.
                    let value = if label.starts_with("Lane lead") { values[i] + worst_lane } else { values[i] };
                    RankPart { label: label.to_string(), value, points: max_points * share, max_points: *max_points }
                })
                .collect();
            Rank { slot: p.slot, score: parts.iter().map(|part| part.points).sum(), parts }
        })
        .collect();
    out.sort_by(|a, b| b.score.total_cmp(&a.score));
    out
}

/// What one resist reduction or damage amplification did for the rest of the team.
#[derive(Debug, Clone, Serialize)]
pub struct EnabledDamage {
    pub source: String,
    /// What it does to the target, e.g. `-8% spirit resist`.
    pub effect: String,
    /// Teammates' damage that landed while it was judged to be on the target.
    pub amplified: f64,
    /// The part of that damage that exists only because of it.
    pub bonus: f64,
    /// The bonus by teammate, largest first.
    pub by_teammate: Vec<SourceAmount>,
    /// What had to land for it to apply (e.g. `headshots`); the estimate assumes it did. Empty for effects any hit applies.
    pub needs: String,
}

/// Running totals of the damage matrix by (dealer, target, bullet damage?), one value per sample time.
fn damage_over_time(matrix: &Value) -> (Vec<u32>, BTreeMap<(u32, u32, bool), Vec<f64>>) {
    let times: Vec<u32> = list(matrix, "sample_time_s").iter().map(|t| t.as_f64().unwrap_or(0.0) as u32).collect();
    let details = matrix.get("source_details").cloned().unwrap_or(Value::Null);
    let (types, names) = (list(&details, "stat_type"), list(&details, "source_name"));
    let mut out: BTreeMap<(u32, u32, bool), Vec<f64>> = BTreeMap::new();
    for dealer in list(matrix, "damage_dealers") {
        for source in list(dealer, "damage_sources") {
            let index = int(source, "source_details_index") as usize;
            let raw = names.get(index).and_then(Value::as_str).unwrap_or("");
            if types.get(index).and_then(Value::as_u64) != Some(0) || AGGREGATES.contains(&raw) {
                continue;
            }
            let bullet = raw.starts_with("citadel_weapon_") || raw.starts_with("ability_melee_");
            for target in list(source, "damage_to_players") {
                let values = list(target, "damage");
                let total = out.entry((int(dealer, "dealer_player_slot"), int(target, "target_player_slot"), bullet)).or_insert_with(|| vec![0.0; times.len()]);
                // A row starts when the first damage was dealt, so it lines up with the end of the time list.
                let skip = times.len().saturating_sub(values.len());
                for (i, v) in values.iter().enumerate().take(times.len()) {
                    total[skip + i] += v.as_f64().unwrap_or(0.0);
                }
            }
        }
    }
    (times, out)
}

/// The largest stack count assumed to be on a target on average.
fn average_stacks(max_stacks: u32) -> f64 {
    if max_stacks <= 1 || max_stacks == u32::MAX {
        // No cap at all: one application at a time is the careful reading.
        1.0
    } else {
        (1.0 + max_stacks as f64) / 2.0
    }
}

/// Estimates, for every player, how much more their teammates dealt because of the resist reductions and
/// amplifications that player put on enemies. The game does not record when a debuff was on someone, so it is
/// taken to be on a target while its owner was fighting that target (and for its duration afterwards).
fn enabled_damage(data: &GameData, players: &mut [PlayerReport], matrix: &Value, tracks: &[fights::Track]) {
    let (times, totals) = damage_over_time(matrix);
    let snapshot: Vec<PlayerReport> = players.to_vec();
    let track = |slot: u32| tracks.iter().find(|t| t.slot == slot);
    // The timeline sample that describes a player at the end of a time bucket.
    let sample_at = |p: &PlayerReport, time: u32| p.timeline.iter().filter(|s| s.time <= time).last().or(p.timeline.first()).cloned();

    for owner in players.iter_mut() {
        let Some(owner_track) = track(owner.slot) else { continue };
        let mut found: Vec<EnabledDamage> = vec![];
        let mut cached: Option<(u32, Vec<TargetDebuff>)> = None;
        for (k, &end) in times.iter().enumerate() {
            let start = if k == 0 { 0 } else { times[k - 1] };
            let Some(sample) = sample_at(owner, end) else { continue };
            if cached.as_ref().is_none_or(|(time, _)| *time != sample.time) {
                cached = Some((sample.time, combo::target_debuffs(data, &sample.build).unwrap_or_default()));
            }
            let debuffs: Vec<TargetDebuff> = cached.as_ref().map(|(_, d)| d.clone()).unwrap_or_default();
            for debuff in debuffs.iter().filter(|d| d.team) {
                let stacks = average_stacks(debuff.max_stacks);
                for mate in snapshot.iter().filter(|m| m.team == owner.team && m.slot != owner.slot) {
                    let Some(mate_track) = track(mate.slot) else { continue };
                    for target in snapshot.iter().filter(|t| t.team != owner.team) {
                        let Some(target_track) = track(target.slot) else { continue };
                        let (mate_seconds, shared) =
                            fights::shared_time(owner_track, mate_track, target_track, start, end, debuff.seconds.min(15.0) as u32);
                        if mate_seconds == 0 || shared == 0 {
                            continue;
                        }
                        let uptime = shared as f64 / mate_seconds as f64 * debuff.duty;
                        let resist = sample_at(target, end).map(|s| (s.bullet_resist, s.spirit_resist)).unwrap_or((0.0, 0.0));
                        for (bullet, resist, shred, amp) in [
                            (true, resist.0, debuff.effect.bullet, debuff.effect.bullet_amp + debuff.effect.all_amp),
                            (false, resist.1, debuff.effect.spirit, debuff.effect.spirit_amp + debuff.effect.all_amp),
                        ] {
                            if shred == 0.0 && amp == 0.0 || resist >= 100.0 {
                                continue;
                            }
                            let dealt = totals
                                .get(&(mate.slot, target.slot, bullet))
                                .map(|v| v[k] - if k == 0 { 0.0 } else { v[k - 1] })
                                .unwrap_or(0.0);
                            // How much harder a hit lands with the debuff than without it.
                            let multiplier = (100.0 - (resist - shred * stacks)) / (100.0 - resist) * (1.0 + amp * stacks / 100.0);
                            if dealt <= 0.0 || multiplier <= 1.0 {
                                continue;
                            }
                            let amplified = dealt * uptime;
                            let bonus = amplified * (1.0 - 1.0 / multiplier);
                            let entry = match found.iter_mut().find(|e| e.source == debuff.source) {
                                Some(entry) => entry,
                                None => {
                                    found.push(EnabledDamage {
                                        source: debuff.source.clone(),
                                        effect: String::new(),
                                        amplified: 0.0,
                                        bonus: 0.0,
                                        by_teammate: vec![],
                                        needs: debuff.needs.clone(),
                                    });
                                    found.last_mut().unwrap()
                                }
                            };
                            entry.effect = if stacks > 1.0 {
                                format!("{} per stack, {} stacks assumed on average", debuff.effect.text(), n(stacks))
                            } else {
                                debuff.effect.text()
                            };
                            entry.amplified += amplified;
                            entry.bonus += bonus;
                            match entry.by_teammate.iter_mut().find(|m| m.name == mate.hero_name) {
                                Some(m) => m.amount += bonus,
                                None => entry.by_teammate.push(SourceAmount { name: mate.hero_name.clone(), amount: bonus }),
                            }
                        }
                    }
                }
            }
        }
        for e in &mut found {
            e.by_teammate.sort_by(|a, b| b.amount.total_cmp(&a.amount));
        }
        found.retain(|e| e.bonus >= 1.0);
        found.sort_by(|a, b| b.bonus.total_cmp(&a.bonus));
        owner.enabled_total = found.iter().map(|e| e.bonus).sum();
        owner.enabled = found;
    }
}

/// One total of the damage matrix: who did it to whom, with what.
struct Row {
    dealer: u32,
    target: u32,
    /// 0 = damage, 1 = healing.
    kind: u64,
    /// Weapon and melee damage is reduced by bullet resist; everything else is spirit damage.
    bullet: bool,
    amount: f64,
}

fn rows(matrix: &Value) -> Vec<Row> {
    let details = matrix.get("source_details").cloned().unwrap_or(Value::Null);
    let (types, names) = (list(&details, "stat_type"), list(&details, "source_name"));
    let mut out = vec![];
    for dealer in list(matrix, "damage_dealers") {
        for source in list(dealer, "damage_sources") {
            let index = int(source, "source_details_index") as usize;
            let raw = names.get(index).and_then(Value::as_str).unwrap_or("");
            let Some(kind) = types.get(index).and_then(Value::as_u64) else { continue };
            if AGGREGATES.contains(&raw) {
                continue;
            }
            for target in list(source, "damage_to_players") {
                out.push(Row {
                    dealer: int(dealer, "dealer_player_slot"),
                    target: int(target, "target_player_slot"),
                    kind,
                    bullet: raw.starts_with("citadel_weapon_") || raw.starts_with("ability_melee_"),
                    amount: list(target, "damage").last().and_then(Value::as_f64).unwrap_or(0.0),
                });
            }
        }
    }
    out
}

/// Items that mainly help the whole team.
const TEAM_ITEMS: [&str; 9] = [
    "Heroic Aura",
    "Healing Nova",
    "Rescue Beam",
    "Divine Barrier",
    "Guardian Ward",
    "Healing Tempo",
    "Mystic Conduit",
    "Celestial Blessing",
    "Knockdown",
];

pub(crate) fn has_modifier(item: &Item, part: &str, negative: bool) -> bool {
    item.props.iter().any(|p| p.modifier.as_deref().is_some_and(|m| m.contains(part)) && (p.value < 0.0) == negative && p.value != 0.0)
}

pub(crate) fn reduces_healing(item: &Item) -> bool {
    // Cheat Death cuts its owner's own healing while it saves them; that is no healing reduction on enemies.
    has_modifier(item, "HEAL_AMP_RECEIVE_PERCENT", true) && !item.props.iter().any(|p| p.key.starts_with("DeathImmunity"))
}

pub(crate) fn shreds_spirit(item: &Item) -> bool {
    item.props.iter().any(|p| {
        p.modifier.as_deref().is_some_and(|m| m.contains("TECH_RESIST_REDUCTION")) || matches!(p.key.as_str(), "TechArmorDamageReduction" | "MagicResistReduction")
    })
}

pub(crate) fn shreds_bullet(item: &Item) -> bool {
    item.props.iter().any(|p| {
        p.modifier.as_deref().is_some_and(|m| m.contains("BULLET_AND_MELEE_RESIST_REDUCTION")) || matches!(p.key.as_str(), "BulletArmorReduction" | "BulletResistReduction")
    })
}

fn pct(part: f64, whole: f64) -> f64 {
    if whole > 0.0 { part / whole * 100.0 } else { 0.0 }
}

/// Compares each player's final items with what teammates and enemies actually did in the match.
fn insights(data: &GameData, players: &mut [PlayerReport], rows: &[Row]) {
    let owned = |p: &PlayerReport| -> Vec<&Item> { p.build.purchases.iter().filter_map(|class| data.item(class)).collect() };
    let names = |items: Vec<&Item>| items.iter().map(|i| i.name.as_str()).collect::<Vec<_>>().join(", ");
    let resists: Vec<(u32, f64, f64)> = players
        .iter()
        .map(|p| {
            let sheet = calc::compute(data, &p.build).ok();
            let stat = |key: &str| sheet.as_ref().and_then(|s| s.stat(key)).unwrap_or(0.0);
            (p.slot, stat("bullet_resist"), stat("spirit_resist"))
        })
        .collect();
    let team_of: Vec<(u32, u32)> = players.iter().map(|p| (p.slot, p.team)).collect();
    let team = |slot: u32| team_of.iter().find(|(s, _)| *s == slot).map(|(_, t)| *t);
    // Spirit and bullet damage between two groups of players.
    let damage = |from: &dyn Fn(u32) -> bool, to: &dyn Fn(u32) -> bool| -> (f64, f64) {
        let pick = |bullet: bool| rows.iter().filter(|r| r.kind == 0 && r.bullet == bullet && from(r.dealer) && to(r.target)).map(|r| r.amount).sum::<f64>();
        (pick(false), pick(true))
    };
    let snapshot: Vec<PlayerReport> = players.to_vec();
    // What each hero's kit threatens, at the upgrade tiers they finished with.
    let kits: Vec<(u32, Vec<counters::Found>)> = players
        .iter()
        .map(|p| (p.slot, calc::compute(data, &p.build).map(|sheet| counters::threats(&sheet.abilities)).unwrap_or_default()))
        .collect();
    let kit = |slot: u32| kits.iter().find(|k| k.0 == slot).map(|k| k.1.as_slice()).unwrap_or(&[]);

    for p in players.iter_mut() {
        let mine = p.team;
        let enemy = |slot: u32| team(slot).is_some_and(|t| t != mine);
        let ally = |slot: u32| team(slot) == Some(mine) && slot != p.slot;
        let me = |slot: u32| slot == p.slot;
        let items = owned(p);
        let mut out = vec![];
        let mut add = |tone: &str, title: &str, text: String| out.push(Insight { tone: tone.into(), title: title.into(), text });
        let (_, my_bullet_resist, my_spirit_resist) = resists.iter().find(|r| r.0 == p.slot).copied().unwrap_or((0, 0.0, 0.0));

        // --- Defence against what the enemies actually dealt ---
        let (spirit_in, bullet_in) = damage(&enemy, &me);
        let total_in = spirit_in + bullet_in;
        if total_in > 0.0 {
            let share = pct(spirit_in, total_in);
            let mix = format!(
                "{}% of the hero damage taken was spirit and {}% weapon or melee. Final resists: {}% spirit, {}% bullet.",
                n(share),
                n(100.0 - share),
                n(my_spirit_resist),
                n(my_bullet_resist)
            );
            if share >= 60.0 && (my_spirit_resist < 10.0 || my_spirit_resist + 5.0 < my_bullet_resist) {
                add("bad", "Resists did not match the damage taken", format!("{mix} Spirit resist would have covered more of it."));
            } else if share <= 40.0 && (my_bullet_resist < 10.0 || my_bullet_resist + 5.0 < my_spirit_resist) {
                add("bad", "Resists did not match the damage taken", format!("{mix} Bullet resist would have covered more of it."));
            } else if (share >= 60.0 && my_spirit_resist >= 20.0) || (share <= 40.0 && my_bullet_resist >= 20.0) {
                add("good", "Resists matched the damage taken", mix);
            } else {
                add("info", "Damage taken", mix);
            }
        }

        // --- Offence against the resists the enemies built ---
        let (spirit_out, bullet_out) = damage(&me, &enemy);
        let total_out = spirit_out + bullet_out;
        let enemies: Vec<&(u32, f64, f64)> = resists.iter().filter(|r| enemy(r.0)).collect();
        let count = enemies.len().max(1) as f64;
        let (enemy_bullet, enemy_spirit) = (enemies.iter().map(|r| r.1).sum::<f64>() / count, enemies.iter().map(|r| r.2).sum::<f64>() / count);
        let spirit_shred: Vec<&Item> = items.iter().copied().filter(|i| shreds_spirit(i)).collect();
        let bullet_shred: Vec<&Item> = items.iter().copied().filter(|i| shreds_bullet(i)).collect();
        if total_out > 0.0 {
            let share = pct(spirit_out, total_out);
            let (kind, resist, shred) = if share >= 50.0 { ("spirit", enemy_spirit, &spirit_shred) } else { ("bullet", enemy_bullet, &bullet_shred) };
            let base = format!(
                "{}% of the damage dealt to heroes was {kind}. The enemy team finished with {}% {kind} resist on average.",
                n(if share >= 50.0 { share } else { 100.0 - share }),
                n(resist)
            );
            if resist >= 20.0 && shred.is_empty() {
                add("bad", "No answer to the enemy's resists", format!("{base} There was no {kind} resist reduction in the build."));
            } else if !shred.is_empty() {
                add("good", "Resist reduction fits the damage dealt", format!("{base} Reduction from: {}.", names(shred.clone())));
            } else {
                add("info", "Damage dealt", base);
            }
        }

        // --- Resist reduction helps teammates who deal the same kind of damage ---
        let (team_spirit, team_bullet) = damage(&ally, &enemy);
        let team_total = team_spirit + team_bullet;
        if team_total > 0.0 {
            for (kind, shred, share) in [
                ("spirit", &spirit_shred, pct(team_spirit, team_total)),
                ("bullet", &bullet_shred, pct(team_bullet, team_total)),
            ] {
                if shred.is_empty() {
                    continue;
                }
                let text = format!("{} lowers {kind} resist, and {}% of the teammates' hero damage was {kind}.", names(shred.clone()), n(share));
                if share >= 50.0 {
                    add("good", "Resist reduction also helps the team", text);
                } else if share < 30.0 && pct(if kind == "spirit" { spirit_out } else { bullet_out }, total_out) < 40.0 {
                    // Bad only when the player's own damage does not use it either.
                    add("bad", "Resist reduction does little for the team", text);
                } else {
                    add("info", "Resist reduction and the team", text);
                }
            }
        }

        // --- Healing reduction against how much the enemies healed ---
        let foes: Vec<&PlayerReport> = snapshot.iter().filter(|o| o.team != mine).collect();
        let (enemy_healing, enemy_taken) = (foes.iter().map(|o| o.healing).sum::<f64>(), foes.iter().map(|o| o.damage_taken).sum::<f64>());
        let healed = pct(enemy_healing, enemy_taken);
        let top = foes.iter().max_by(|a, b| a.healing.total_cmp(&b.healing));
        let anti: Vec<&Item> = items.iter().copied().filter(|i| reduces_healing(i)).collect();
        let allies_with: Vec<&str> = snapshot
            .iter()
            .filter(|o| o.team == mine && o.slot != p.slot && owned(o).iter().any(|i| reduces_healing(i)))
            .map(|o| o.hero_name.as_str())
            .collect();
        let healing = format!(
            "The enemy team healed {} ({}% of the damage it took){}.",
            n(enemy_healing),
            n(healed),
            top.map(|t| format!(", most of it {} with {}", t.hero_name, n(t.healing))).unwrap_or_default()
        );
        if !anti.is_empty() {
            let tone = if healed >= 25.0 && p.heal_prevented >= 500.0 { "good" } else { "info" };
            add(tone, "Healing reduction", format!("{healing} {} prevented {} healing.", names(anti.clone()), n(p.heal_prevented)));
        } else if healed >= 30.0 && allies_with.is_empty() {
            add("bad", "Nobody reduced the enemy's healing", format!("{healing} No one on the team had a healing reduction item."));
        } else if healed >= 30.0 {
            add("info", "Healing reduction came from teammates", format!("{healing} Healing reduction was covered by {}.", allies_with.join(", ")));
        }

        // --- Answers to what the enemy heroes' kits do ---
        let foes_of_kind = |kind: Threat| -> Vec<String> {
            snapshot
                .iter()
                .filter(|o| o.team != mine)
                .flat_map(|o| kit(o.slot).iter().filter(move |t| t.kind == kind).map(move |t| format!("{}'s {}", o.hero_name, t.ability)))
                .collect()
        };
        let item_answers = |who: &PlayerReport, kind: Threat| -> Vec<String> {
            owned(who).iter().filter(|i| counters::answers(i).contains(&kind)).map(|i| i.name.clone()).collect()
        };
        // Teammates who carry an answer, as "Hero (item)".
        let allies_answering = |kind: Threat| -> Vec<String> {
            snapshot
                .iter()
                .filter(|o| o.team == mine && o.slot != p.slot)
                .filter_map(|o| {
                    let items = item_answers(o, kind);
                    (!items.is_empty()).then(|| format!("{} ({})", o.hero_name, items.join(", ")))
                })
                .collect()
        };
        let me_now: &PlayerReport = snapshot.iter().find(|o| o.slot == p.slot).unwrap_or(&snapshot[0]);

        let lockdowns = foes_of_kind(Threat::Lockdown);
        let own = item_answers(me_now, Threat::Lockdown);
        if lockdowns.len() >= 3 {
            let listed = format!("The enemy team had {} stuns, roots or sleeps: {}.", lockdowns.len(), lockdowns.join(", "));
            if own.is_empty() {
                add("bad", "No answer to crowd control", format!("{listed} Nothing in the build breaks or shortens them."));
            } else {
                add("good", "An answer to crowd control", format!("{listed} Answered with {}.", own.join(", ")));
            }
        }

        let silences = foes_of_kind(Threat::Silence);
        let own = item_answers(me_now, Threat::Silence);
        // A silence hurts most when the damage comes from abilities.
        if !silences.is_empty() && total_out > 0.0 && pct(spirit_out, total_out) >= 60.0 {
            let listed = format!("The enemy team could silence: {}. Most of this hero's damage is spirit damage.", silences.join(", "));
            if own.is_empty() {
                add("bad", "No answer to silences", format!("{listed} Nothing in the build removes or shortens a silence."));
            } else {
                add("good", "An answer to silences", format!("{listed} Answered with {}.", own.join(", ")));
            }
        }

        for (kind, title, what) in [
            (Threat::Flight, "flying heroes", "could take to the air"),
            (Threat::Channel, "channelled ultimates", "had a channelled ultimate that can be interrupted"),
        ] {
            let foes = foes_of_kind(kind);
            if foes.is_empty() {
                continue;
            }
            let mut own = item_answers(me_now, kind);
            if kind == Threat::Channel {
                // A stun of one's own interrupts a channel just as well as an item.
                own.extend(kit(p.slot).iter().filter(|t| t.kind == Threat::Lockdown).map(|t| t.ability.clone()));
            }
            let mut allies = allies_answering(kind);
            if kind == Threat::Channel {
                allies.extend(
                    snapshot
                        .iter()
                        .filter(|o| o.team == mine && o.slot != p.slot && kit(o.slot).iter().any(|t| t.kind == Threat::Lockdown))
                        .map(|o| format!("{} (own stun)", o.hero_name)),
                );
            }
            let listed = format!("The enemy {what}: {}.", foes.join(", "));
            if !own.is_empty() {
                add("good", &format!("An answer to {title}"), format!("{listed} Answered with {}.", own.join(", ")));
            } else if allies.is_empty() {
                add("bad", &format!("Nobody could stop the {title}"), format!("{listed} No one on the team had a way to bring them down or interrupt them."));
            } else if kind == Threat::Flight {
                add("info", &format!("Teammates covered the {title}"), format!("{listed} Covered by {}.", allies.join(", ")));
            }
        }

        // --- Items bought for the team ---
        let team_items: Vec<&Item> = items.iter().copied().filter(|i| TEAM_ITEMS.contains(&i.name.as_str())).collect();
        if !team_items.is_empty() {
            add("info", "Team items", format!("Bought for the team: {}.", names(team_items)));
        }
        p.insights = out;
    }
}

/// When the match started calling for a kind of item, and when the player bought one.
#[derive(Debug, Clone, Serialize)]
pub struct Timing {
    /// `good` | `bad` | `info`.
    pub tone: String,
    pub need: String,
    /// When the match started calling for it; `None` when it never clearly did.
    pub needed_at: Option<u32>,
    pub bought_at: Option<u32>,
    /// What answered it; empty when nothing did.
    pub item: String,
    pub text: String,
    /// How well the player answered the enemy team here, 0-1; `None` when it does not count toward the score
    /// (a teammate covered it, the match never called for it, or it is a power spike).
    pub credit: Option<f64>,
    /// For a need answered late or never: what would have answered it, and what the souls went to meanwhile.
    pub advice: Vec<Advice>,
    pub spent_on: Vec<String>,
}

/// An item that would have answered a need, with an estimate of what it would have changed.
#[derive(Debug, Clone, Serialize)]
pub struct Advice {
    pub item: String,
    pub icon: String,
    pub cost: u32,
    /// What the item brings, e.g. `30% spirit resist`.
    pub effect: String,
    /// Damage saved or gained over the time the need went unanswered; `None` where no number can be put on it.
    pub estimate: Option<f64>,
    pub note: String,
}

/// One reason an item was worth buying when it was bought, held against the enemies it was aimed at.
#[derive(Debug, Clone, Serialize)]
pub struct ItemReason {
    pub time: u32,
    pub item: String,
    pub icon: String,
    /// Bought in or right out of the laning phase, so held against the lane opponents; later purchases are held
    /// against the whole enemy team.
    pub lane: bool,
    /// `good` | `bad` | `info`.
    pub tone: String,
    /// What the item does about the enemies, e.g. `Slows fire rate by 28%`.
    pub effect: String,
    pub text: String,
}

/// Purchases up to this time are answers to the lane; after it, to the whole enemy team.
const LANE_BUILD_S: u32 = LANE_PHASE_S + 3 * 60;
/// After the lane, a purchase is held against the damage of the last this many samples of the damage record.
const RECENT_SAMPLES: usize = 2;
/// A defensive item fits when this share of the damage taken is of its kind, or one enemy dealing that kind
/// accounts for this share of everything taken; it misses when the kind is below the low mark.
const FITS_PCT: f64 = 50.0;
const FITS_ONE_ENEMY_PCT: f64 = 30.0;
const MISSES_PCT: f64 = 25.0;
/// The lane calls for a defence once the lane opponents have dealt this much to the player.
const LANE_FLOOR: f64 = 1000.0;
/// A vitality item with this much resist is bought for the resist.
const DEDICATED_RESIST: f64 = 15.0;

fn fire_rate_slow_of(item: &Item) -> f64 {
    prop_value(item, |p| p.modifier.as_deref() == Some("MODIFIER_VALUE_FIRE_RATE_SLOW"))
}

/// How many items are suggested for a need that was never answered.
const ADVICE_ITEMS: usize = 3;

/// The largest value among an item's properties that pass the test.
fn prop_value(item: &Item, test: impl Fn(&crate::data::Prop) -> bool) -> f64 {
    item.props.iter().filter(|p| test(p)).map(|p| p.value.abs()).fold(0.0, f64::max)
}

pub(crate) fn resist_of(item: &Item, spirit: bool) -> f64 {
    let modifier = if spirit { "MODIFIER_VALUE_TECH_RESIST" } else { "MODIFIER_VALUE_BULLET_ARMOR_DAMAGE_RESIST" };
    prop_value(item, |p| p.modifier.as_deref() == Some(modifier) && p.section != "active" && !p.key.ends_with("Gain"))
}

pub(crate) fn shred_of(item: &Item, spirit: bool) -> f64 {
    if spirit {
        prop_value(item, |p| p.modifier.as_deref().is_some_and(|m| m.contains("TECH_RESIST_REDUCTION")) || matches!(p.key.as_str(), "TechArmorDamageReduction" | "MagicResistReduction"))
    } else {
        prop_value(item, |p| {
            p.modifier.as_deref().is_some_and(|m| m.contains("BULLET_AND_MELEE_RESIST_REDUCTION")) || matches!(p.key.as_str(), "BulletArmorReduction" | "BulletResistReduction")
        })
    }
}

pub(crate) fn heal_cut_of(item: &Item) -> f64 {
    prop_value(item, |p| p.modifier.as_deref().is_some_and(|m| m.contains("HEAL_AMP_RECEIVE_PERCENT")) && p.value < 0.0)
}

/// What an answer to the enemy team is worth by how it came: on time, a little late, very late.
const CREDIT_ON_TIME: f64 = 1.0;
const CREDIT_SLOW: f64 = 0.6;
const CREDIT_LATE: f64 = 0.3;

/// An answer within this long of the need showing is on time; one that takes this long or more is late.
const ON_TIME_S: u32 = 4 * 60;
const LATE_S: u32 = 10 * 60;
/// In the lane an answer has a little longer: the first items have to be saved up for.
const ON_TIME_LANE_S: u32 = 6 * 60;
/// Having no answer to crowd control is only held against a player when the enemy team has this many such abilities.
const HEAVY_LOCKDOWN: usize = 5;
/// The enemy team heals heavily once its healing is this share of the damage it took, past a floor.
const HEAVY_HEALING_PCT: f64 = 25.0;
const HEAVY_HEALING_FLOOR: f64 = 4000.0;
/// The damage a player takes is lopsided once this share of it is one kind, past a floor.
const LOPSIDED_PCT: f64 = 60.0;
const LOPSIDED_FLOOR: f64 = 3000.0;
/// An item counts as a resist item from this much resist; resist reduction is called for once the enemies
/// average this much resist.
const REAL_RESIST: f64 = 10.0;
const ENEMY_RESIST: f64 = 20.0;
/// A power spike that comes this much before the match's median is early, this much after it late.
const SPIKE_EARLY_S: u32 = 60;
const SPIKE_LATE_S: u32 = 120;

pub(crate) fn gives_resist(item: &Item, spirit: bool) -> bool {
    let modifier = if spirit { "MODIFIER_VALUE_TECH_RESIST" } else { "MODIFIER_VALUE_BULLET_ARMOR_DAMAGE_RESIST" };
    // Resist the item is bought for, not what an active grants for a moment or a hit steals.
    item.props.iter().any(|p| p.modifier.as_deref() == Some(modifier) && p.value >= REAL_RESIST && p.section != "active" && !p.key.ends_with("Gain"))
}

fn clock(time: u32) -> String {
    format!("{}:{:02}", time / 60, time % 60)
}

/// For every player: when the match started calling for healing reduction, a resist, resist reduction or an
/// answer to an enemy ability, and when the item came; plus when the first big items were finished.
fn build_timing(data: &GameData, players: &mut [PlayerReport], matrix: &Value) {
    let (times, totals) = damage_over_time(matrix);
    let snapshot: Vec<PlayerReport> = players.to_vec();
    // The first purchase of an item that passes the test.
    let first = |p: &PlayerReport, test: &dyn Fn(&Item) -> bool| -> Option<(u32, String)> {
        p.purchases.iter().filter(|x| data.item(&x.item).is_some_and(test)).min_by_key(|x| x.time).map(|x| (x.time, x.name.clone()))
    };
    let grade = |needed: u32, bought: Option<u32>| match bought {
        None => "bad",
        Some(b) if b <= needed + ON_TIME_S => "good",
        Some(b) if b >= needed + LATE_S => "bad",
        _ => "info",
    };
    // "3:20 later" or "before that".
    let gap = |needed: u32, bought: u32| if bought > needed { format!("{} later", clock(bought - needed)) } else { "in time".to_string() };
    // What every enemy kit threatens, with the time the ability was unlocked.
    let kits: Vec<(u32, Vec<(Threat, u32, String)>)> = snapshot
        .iter()
        .map(|o| {
            let found = calc::compute(data, &o.build).map(|sheet| counters::threats(&sheet.abilities)).unwrap_or_default();
            let unlocked = |ability: &str| o.abilities.iter().filter(|a| a.name == ability).map(|a| a.time).min().unwrap_or(0);
            (o.slot, found.iter().map(|t| (t.kind, unlocked(&t.ability), format!("{}'s {}", o.hero_name, t.ability))).collect())
        })
        .collect();
    // The match's median time for the first item of a tier or above.
    // When a player's purchases added up to an amount of souls, and with which item.
    let spike = |p: &PlayerReport, souls: u32| -> Option<(u32, String)> {
        let mut bought: Vec<&Purchase> = p.purchases.iter().collect();
        bought.sort_by_key(|x| x.time);
        let mut spent = 0;
        bought.into_iter().find_map(|x| {
            spent += x.cost;
            (spent >= souls).then(|| (x.time, x.name.clone()))
        })
    };
    let median = |tier: u32| -> Option<u32> {
        let mut all: Vec<u32> = snapshot.iter().filter_map(|o| spike(o, tier)).map(|x| x.0).collect();
        all.sort();
        (!all.is_empty()).then(|| all[all.len() / 2])
    };

    for p in players.iter_mut() {
        let mut out: Vec<Timing> = vec![];
        let mut add = |tone: &str, need: &str, needed_at: Option<u32>, bought: Option<(u32, String)>, text: String| {
            // What counts toward the score: needs the match really showed and that were this player's to answer.
            let credit = if need.starts_with("First ") || need.contains("teammate") || needed_at.is_none() {
                None
            } else if bought.is_none() {
                Some(0.0)
            } else if need.starts_with("Answer") {
                // These items are bought for one purpose; having one is the point, whenever it came.
                Some(CREDIT_ON_TIME)
            } else {
                Some(match tone {
                    "good" => CREDIT_ON_TIME,
                    "info" => CREDIT_SLOW,
                    _ => CREDIT_LATE,
                })
            };
            out.push(Timing {
                tone: tone.into(),
                need: need.into(),
                needed_at,
                bought_at: bought.as_ref().map(|b| b.0),
                item: bought.map(|b| b.1).unwrap_or_default(),
                text,
                credit,
                advice: vec![],
                spent_on: vec![],
            })
        };
        let foes: Vec<&PlayerReport> = snapshot.iter().filter(|o| o.team != p.team).collect();
        let mates: Vec<&PlayerReport> = snapshot.iter().filter(|o| o.team == p.team && o.slot != p.slot).collect();
        let foe_sum = |time: u32, value: &dyn Fn(&Sample) -> f64| foes.iter().filter_map(|o| o.timeline.iter().find(|s| s.time == time)).map(value).sum::<f64>();

        // --- Healing reduction ---
        let needed = p.timeline.iter().map(|s| s.time).find(|&t| {
            let (healed, taken) = (foe_sum(t, &|s| s.healing), foe_sum(t, &|s| s.damage_taken));
            healed >= HEAVY_HEALING_FLOOR && pct(healed, taken) >= HEAVY_HEALING_PCT
        });
        let own = first(p, &|i| reduces_healing(i));
        let mate = mates.iter().filter_map(|o| first(o, &|i| reduces_healing(i)).map(|x| (x.0, format!("{}'s {}", o.hero_name, x.1)))).min_by_key(|x| x.0);
        // One player with healing reduction covers the team, so the need is held against the team's first.
        let heavy = |n: u32| format!("By {} the enemy team's healing had passed {HEAVY_HEALING_PCT}% of the damage it took.", clock(n));
        match (needed, own, mate) {
            (Some(n), Some(own), Some(mate)) if mate.0 < own.0 => {
                let text = format!("{} The team's first healing reduction was {} at {}, {}; {} followed at {}.", heavy(n), mate.1, clock(mate.0), gap(n, mate.0), own.1, clock(own.0));
                add(grade(n, Some(mate.0)), "Healing reduction (a teammate had it first)", Some(n), Some(mate), text);
            }
            (Some(n), Some(own), _) => {
                let text = format!("{} {} was the team's first healing reduction, at {}, {}.", heavy(n), own.1, clock(own.0), gap(n, own.0));
                add(grade(n, Some(own.0)), "Healing reduction", Some(n), Some(own), text);
            }
            (Some(n), None, Some(mate)) => {
                let text = format!("{} Not bought; the team's first was {} at {}, {}.", heavy(n), mate.1, clock(mate.0), gap(n, mate.0));
                add(grade(n, Some(mate.0)), "Healing reduction (covered by a teammate)", Some(n), Some(mate), text);
            }
            (Some(n), None, None) => {
                add("bad", "Healing reduction", Some(n), None, format!("{} Nobody on the team ever bought healing reduction.", heavy(n)));
            }
            (None, Some(own), _) => {
                let text = format!("{} came at {}, though the enemy team's healing never reached {HEAVY_HEALING_PCT}% of the damage it took.", own.1, clock(own.0));
                add("info", "Healing reduction", None, Some(own), text);
            }
            _ => {}
        }

        // --- A resist against the kind of damage taken ---
        let dealt = |from: &dyn Fn(u32) -> bool, to: &dyn Fn(u32) -> bool, bullet: bool, i: usize| -> f64 {
            totals.iter().filter(|(k, _)| k.2 == bullet && from(k.0) && to(k.1)).map(|(_, v)| v.get(i).copied().unwrap_or(0.0)).sum()
        };
        let enemy = |slot: u32| foes.iter().any(|o| o.slot == slot);
        let me = |slot: u32| slot == p.slot;
        let last = times.len().saturating_sub(1);
        // The last sample of the damage record that still belongs to the lane; `None` without a lane.
        let lane_end = if p.lane_opponents.is_empty() { None } else { times.iter().rposition(|t| *t <= LANE_BUILD_S) };

        // --- The lane: a defence against what the lane opponents dealt ---
        if let Some(lane_end) = lane_end {
            let name_of = |slot: u32| snapshot.iter().find(|o| o.slot == slot).map(|o| o.hero_name.clone()).unwrap_or_default();
            for bullet in [true, false] {
                let (word, need) = if bullet { ("weapon", "Lane: weapon defence") } else { ("spirit", "Lane: spirit defence") };
                // The first moment the lane opponents' damage was mostly of this kind, or one of them who deals
                // mostly this kind accounted for a large share of everything on their own.
                let found = (0..=lane_end).find_map(|i| {
                    let rows: Vec<(u32, f64, f64)> = p
                        .lane_opponents
                        .iter()
                        .map(|slot| {
                            let by = |s: u32| s == *slot;
                            (*slot, dealt(&by, &me, bullet, i), dealt(&by, &me, !bullet, i))
                        })
                        .collect();
                    let kind: f64 = rows.iter().map(|r| r.1).sum();
                    let total = kind + rows.iter().map(|r| r.2).sum::<f64>();
                    let top = rows.iter().max_by(|a, b| a.1.total_cmp(&b.1))?;
                    let one_enemy = pct(top.1, top.1 + top.2) >= LOPSIDED_PCT && pct(top.1, total) >= FITS_ONE_ENEMY_PCT;
                    (total >= LANE_FLOOR && (pct(kind, total) >= LOPSIDED_PCT || one_enemy)).then(|| (times[i], top.0, top.1, pct(kind, total)))
                });
                let Some((needed, top, amount, share)) = found else { continue };
                let own = first(p, &|i| gives_resist(i, !bullet) || (bullet && fire_rate_slow_of(i) > 0.0));
                let base = format!(
                    "In lane {}, {} had dealt {} {word} damage to {} by {}; {word} damage was {}% of what the lane opponents had dealt.",
                    p.lane,
                    name_of(top),
                    n(amount),
                    p.hero_name,
                    clock(needed),
                    n(share)
                );
                let text = match &own {
                    Some(own) => format!("{base} {} came at {}, {}.", own.1, clock(own.0), gap(needed, own.0)),
                    None => format!("{base} Nothing against {word} damage was ever bought."),
                };
                let tone = match own.as_ref().map(|o| o.0) {
                    None => "bad",
                    Some(b) if b <= needed + ON_TIME_LANE_S => "good",
                    Some(b) if b >= needed + LATE_S => "bad",
                    _ => "info",
                };
                add(tone, need, Some(needed), own, text);
            }
        }

        if !times.is_empty() {
            // After the lane the whole enemy team counts, with the damage dealt from then on.
            let since_lane = |from: &dyn Fn(u32) -> bool, to: &dyn Fn(u32) -> bool, bullet: bool, i: usize| -> f64 {
                (dealt(from, to, bullet, i) - lane_end.map(|l| dealt(from, to, bullet, l)).unwrap_or(0.0)).max(0.0)
            };
            let (spirit, bullet) = (since_lane(&enemy, &me, false, last), since_lane(&enemy, &me, true, last));
            let share = pct(spirit, spirit + bullet);
            let kind = if share >= LOPSIDED_PCT { Some(true) } else if share <= 100.0 - LOPSIDED_PCT { Some(false) } else { None };
            if let Some(is_spirit) = kind {
                let word = if is_spirit { "spirit" } else { "weapon or melee" };
                let resist = if is_spirit { "spirit" } else { "bullet" };
                let needed = (lane_end.map(|l| l + 1).unwrap_or(0)..times.len()).find(|&i| {
                    let (s, b) = (since_lane(&enemy, &me, false, i), since_lane(&enemy, &me, true, i));
                    s + b >= LOPSIDED_FLOOR && pct(if is_spirit { s } else { b }, s + b) >= LOPSIDED_PCT
                });
                if let Some(n) = needed.map(|i| times[i]) {
                    let own = first(p, &|i| gives_resist(i, is_spirit));
                    let base = format!(
                        "{} {} most of the hero damage taken from the enemy team was {word} ({}% {}).",
                        if lane_end.is_some() { "After the lane, by" } else { "By" },
                        clock(n),
                        n_pct(if is_spirit { share } else { 100.0 - share }),
                        if lane_end.is_some() { "from the end of the lane on" } else { "over the match" }
                    );
                    let text = match &own {
                        Some(own) => format!("{base} {} ({resist} resist) came at {}, {}.", own.1, clock(own.0), gap(n, own.0)),
                        None => format!("{base} No item with {REAL_RESIST}% or more {resist} resist was ever bought."),
                    };
                    add(grade(n, own.as_ref().map(|o| o.0)), &format!("{} resist", if is_spirit { "Spirit" } else { "Bullet" }), Some(n), own, text);
                }
            }

            // --- Resist reduction against the resists the enemies built ---
            let (spirit, bullet) = (dealt(&me, &enemy, false, last), dealt(&me, &enemy, true, last));
            if spirit + bullet > 0.0 {
                let is_spirit = spirit >= bullet;
                let kind = if is_spirit { "spirit" } else { "bullet" };
                let needed = p.timeline.iter().map(|s| s.time).find(|&t| {
                    foe_sum(t, &|s| if is_spirit { s.spirit_resist } else { s.bullet_resist }) / foes.len().max(1) as f64 >= ENEMY_RESIST
                });
                let own = first(p, &|i| if is_spirit { shreds_spirit(i) } else { shreds_bullet(i) });
                match (needed, own) {
                    (Some(n), own) => {
                        let base = format!("By {} the enemy team averaged {ENEMY_RESIST}% {kind} resist, the kind of damage this hero mostly deals.", clock(n));
                        let text = match &own {
                            Some(own) => format!("{base} {} came at {}, {}.", own.1, clock(own.0), gap(n, own.0)),
                            None => format!("{base} No {kind} resist reduction was ever bought."),
                        };
                        add(grade(n, own.as_ref().map(|o| o.0)), &format!("{} resist reduction", if is_spirit { "Spirit" } else { "Bullet" }), Some(n), own, text);
                    }
                    (None, Some(own)) => {
                        let text = format!("{} came at {}; the enemy team never averaged {ENEMY_RESIST}% {kind} resist.", own.1, clock(own.0));
                        add("info", &format!("{} resist reduction", if is_spirit { "Spirit" } else { "Bullet" }), None, Some(own), text);
                    }
                    _ => {}
                }
            }
        }

        let spirit_heavy = !times.is_empty() && {
            let (spirit, bullet) = (dealt(&me, &enemy, false, last), dealt(&me, &enemy, true, last));
            pct(spirit, spirit + bullet) >= LOPSIDED_PCT
        };
        // --- Answers to enemy abilities, from the moment the enemies had them ---
        for (kind, need, what, at_least) in [
            (Threat::Lockdown, "Answer to crowd control", "stuns, roots or sleeps", 3),
            (Threat::Silence, "Answer to silences", "a silence", 1),
            (Threat::Flight, "Answer to flying heroes", "a hero who can take to the air", 1),
            (Threat::Channel, "Answer to channelled ultimates", "a channelled ultimate", 1),
        ] {
            let mut theirs: Vec<(u32, String)> =
                kits.iter().filter(|k| enemy(k.0)).flat_map(|k| k.1.iter().filter(|t| t.0 == kind).map(|t| (t.1, t.2.clone()))).collect();
            theirs.sort();
            if theirs.len() < at_least {
                continue;
            }
            let n = theirs[at_least - 1].0;
            let listed = theirs.iter().take(4).map(|t| format!("{} ({})", t.1, clock(t.0))).collect::<Vec<_>>().join(", ");
            match first(p, &|i| counters::answers(i).contains(&kind)) {
                Some(own) => {
                    let text = format!("The enemy team had {what} from {}: {listed}. {} came at {}, {}.", clock(n), own.1, clock(own.0), gap(n, own.0));
                    add("info", need, Some(n), Some(own), text);
                }
                // Crowd control is everyone's problem, a silence that of a hero who lives off abilities. Flying
                // heroes and channels can be left to a teammate, so missing those is not held against anyone.
                None if (kind == Threat::Lockdown && theirs.len() >= HEAVY_LOCKDOWN) || (kind == Threat::Silence && spirit_heavy) => {
                    add("bad", need, Some(n), None, format!("The enemy team had {what} from {}: {listed}. Nothing in the build answers it.", clock(n)));
                }
                None => {}
            }
        }

        // --- Power spikes: the first big items against the rest of the match ---
        for (tier, need) in [(8_000u32, "First 8,000 souls in items"), (20_000, "First 20,000 souls in items")] {
            let (Some(typical), own) = (median(tier), spike(p, tier)) else { continue };
            match own {
                Some(own) => {
                    let tone = if own.0 + SPIKE_EARLY_S <= typical { "good" } else if own.0 >= typical + SPIKE_LATE_S { "bad" } else { "info" };
                    let against = match own.0.cmp(&typical) {
                        std::cmp::Ordering::Less => format!("{} before", clock(typical - own.0)),
                        std::cmp::Ordering::Equal => "right at".to_string(),
                        std::cmp::Ordering::Greater => format!("{} after", clock(own.0 - typical)),
                    };
                    let text = format!("Reached with {} at {}, {against} the match's median of {}.", own.1, clock(own.0), clock(typical));
                    add(tone, need, None, Some(own), text);
                }
                None => add("bad", need, None, None, format!("Never reached; the match's median was {}.", clock(typical))),
            }
        }
        // --- What would have answered the needs that were met late or never ---
        enum Kind {
            Resist(bool),
            Shred(bool),
            Heal,
            Answer(Threat),
        }
        let end_time = times.last().copied().unwrap_or(0);
        let index = |time: u32| times.iter().rposition(|x| *x <= time).unwrap_or(0);
        let healed_by = |time: u32| {
            let sample = p.timeline.iter().map(|s| s.time).filter(|t| *t <= time).max().unwrap_or(0);
            foe_sum(sample, &|s| s.healing)
        };
        let enemy_resist = |spirit: bool| {
            let at = p.timeline.last().map(|s| s.time).unwrap_or(0);
            (foe_sum(at, &|s| if spirit { s.spirit_resist } else { s.bullet_resist }) / foes.len().max(1) as f64).clamp(0.0, 90.0)
        };
        for t in out.iter_mut() {
            let (Some(credit), Some(needed)) = (t.credit, t.needed_at) else { continue };
            if credit >= CREDIT_ON_TIME {
                continue;
            }
            let kind = match t.need.as_str() {
                "Spirit resist" | "Lane: spirit defence" => Kind::Resist(true),
                "Bullet resist" | "Lane: weapon defence" => Kind::Resist(false),
                "Spirit resist reduction" => Kind::Shred(true),
                "Bullet resist reduction" => Kind::Shred(false),
                "Healing reduction" => Kind::Heal,
                "Answer to crowd control" => Kind::Answer(Threat::Lockdown),
                "Answer to silences" => Kind::Answer(Threat::Silence),
                _ => continue,
            };
            let until = t.bought_at.unwrap_or(end_time).max(needed);
            let span = format!("between {} and {}", clock(needed), if t.bought_at.is_some() { clock(until) } else { "the end of the match".to_string() });
            let (from, to) = (index(needed), index(until));
            let window = |from_who: &dyn Fn(u32) -> bool, to_who: &dyn Fn(u32) -> bool, bullet: bool| (dealt(from_who, to_who, bullet, to) - dealt(from_who, to_who, bullet, from)).max(0.0);
            let amount = |i: &Item| match kind {
                Kind::Resist(spirit) => resist_of(i, spirit),
                Kind::Shred(spirit) => shred_of(i, spirit),
                Kind::Heal => heal_cut_of(i),
                Kind::Answer(_) => 0.0,
            };
            let fits = |i: &Item| match kind {
                Kind::Resist(spirit) => gives_resist(i, spirit),
                Kind::Shred(true) => shreds_spirit(i),
                Kind::Shred(false) => shreds_bullet(i),
                Kind::Heal => reduces_healing(i),
                Kind::Answer(threat) => counters::answers(i).contains(&threat),
            };
            // Bought late: the same item earlier, and the cheaper ones that would have done it sooner.
            // Never bought: the cheapest items that would have done it.
            let late: Option<&Item> = data.items.iter().find(|i| t.bought_at.is_some() && i.name == t.item);
            let mut options: Vec<&Item> = data
                .items
                .iter()
                .filter(|i| i.cost > 0 && i.tier <= 4 && fits(i) && !p.purchases.iter().any(|x| x.item == i.class_name))
                .filter(|i| late.is_none_or(|l| i.cost < l.cost))
                .collect();
            options.sort_by(|a, b| a.cost.cmp(&b.cost).then(amount(b).total_cmp(&amount(a))).then(a.name.cmp(&b.name)));
            options.dedup_by(|a, b| a.name == b.name);
            options.splice(0..0, late);
            t.advice = options
                .into_iter()
                .take(ADVICE_ITEMS)
                .map(|i| {
                    let value = amount(i);
                    let (effect, estimate, note) = match kind {
                        Kind::Resist(spirit) => {
                            let word = if spirit { "spirit" } else { "weapon and melee" };
                            // The damage recorded is what got through the resists already there; one more resist
                            // takes its share off that.
                            let saved = window(&enemy, &me, !spirit) * value / 100.0;
                            (format!("{}% {} resist", n(value), if spirit { "spirit" } else { "bullet" }), Some(saved), format!("about {} less {word} damage taken {span}", n(saved)))
                        }
                        Kind::Shred(spirit) => {
                            let word = if spirit { "spirit" } else { "bullet" };
                            let gained = window(&me, &enemy, !spirit) * (value / 100.0) / (1.0 - enemy_resist(spirit) / 100.0);
                            (format!("-{}% {word} resist on the target", n(value)), Some(gained), format!("up to {} more {word} damage dealt {span}, if it was on every target hit", n(gained)))
                        }
                        Kind::Heal => {
                            let healed = (healed_by(until) - healed_by(needed)).max(0.0);
                            (format!("-{}% healing on the target", n(value)), None, format!("the enemy team healed {} {span}; this cuts the healing of whoever it is on", n(healed)))
                        }
                        Kind::Answer(Threat::Lockdown) => ("breaks or shortens crowd control".to_string(), None, "an answer to the enemy's stuns, roots and sleeps".to_string()),
                        Kind::Answer(_) => ("removes or shortens a silence".to_string(), None, "an answer to the enemy's silences".to_string()),
                    };
                    Advice { item: i.name.clone(), icon: i.icon.clone(), cost: i.cost, effect, estimate, note }
                })
                .collect();
            // What the souls went to while the need stood open.
            let open_until = if t.bought_at.is_some() { until } else { needed + LATE_S };
            let mut bought: Vec<&Purchase> = p.purchases.iter().filter(|x| x.time >= needed && x.time < open_until && x.name != t.item).collect();
            bought.sort_by_key(|x| x.time);
            t.spent_on = bought.iter().take(4).map(|x| format!("{} ({}, {} souls)", x.name, clock(x.time), n(x.cost as f64))).collect();
        }

        // --- Why each answering item was bought: the enemies it was aimed at, at the time ---
        let hero = |slot: u32| snapshot.iter().find(|o| o.slot == slot).map(|o| o.hero_name.as_str()).unwrap_or("?");
        let mut reasons: Vec<ItemReason> = vec![];
        let mut seen: Vec<&str> = vec![];
        let mut bought: Vec<&Purchase> = p.purchases.iter().collect();
        bought.sort_by_key(|x| x.time);
        for x in bought {
            let Some(item) = data.item(&x.item) else { continue };
            if seen.contains(&x.item.as_str()) {
                continue;
            }
            seen.push(&x.item);
            let lane = x.time <= LANE_BUILD_S && !p.lane_opponents.is_empty();
            let scope: Vec<u32> = if lane { p.lane_opponents.clone() } else { foes.iter().map(|o| o.slot).collect() };
            let who = if lane { format!("the lane opponents ({})", scope.iter().map(|s| hero(*s)).collect::<Vec<_>>().join(", ")) } else { "the enemy team".to_string() };
            let when = if lane { "by then" } else { "in the minutes before" };
            // Damage one enemy dealt to this player: everything so far in the lane, the recent samples after it.
            let (to, from) = (index(x.time), if lane { None } else { Some(index(x.time).saturating_sub(RECENT_SAMPLES)) });
            let dealt_by = |slot: u32, bullet: bool| -> f64 {
                let by = |s: u32| s == slot;
                let total = dealt(&by, &me, bullet, to);
                (total - from.filter(|f| *f < to).map(|f| dealt(&by, &me, bullet, f)).unwrap_or(0.0)).max(0.0)
            };
            let sample = p.timeline.iter().map(|s| s.time).filter(|t| *t <= x.time).max();
            let at_sample = |slot: u32, value: &dyn Fn(&Sample) -> f64| {
                snapshot.iter().find(|o| o.slot == slot).and_then(|o| o.timeline.iter().find(|s| Some(s.time) == sample)).map(value).unwrap_or(0.0)
            };
            let mut add = |tone: &str, effect: String, text: String| {
                reasons.push(ItemReason { time: x.time, item: x.name.clone(), icon: x.icon.clone(), lane, tone: tone.into(), effect, text })
            };

            // Defence against one kind of damage: a resist, or slowing the enemy's gun.
            for bullet in [true, false] {
                let (resist, slow) = (resist_of(item, !bullet), if bullet { fire_rate_slow_of(item) } else { 0.0 });
                if resist < REAL_RESIST && slow <= 0.0 {
                    continue;
                }
                // Bought for the defence itself, or does the defence come along with something else? Only an
                // item bought for it can be the wrong call.
                let dedicated = !shreds_spirit(item) && !shreds_bullet(item) && (slow > 0.0 || (item.category == "vitality" && resist >= DEDICATED_RESIST));
                let (word, name) = if bullet { ("weapon", "bullet") } else { ("spirit", "spirit") };
                let effect = match (slow > 0.0, resist > 0.0) {
                    (true, true) => format!("Slows the fire rate of enemies hit by {}% and gives {}% bullet resist", n(slow), n(resist)),
                    (true, false) => format!("Slows the fire rate of enemies hit by {}%", n(slow)),
                    _ => format!("{}% {name} resist", n(resist)),
                };
                let rows: Vec<(u32, f64, f64)> = scope.iter().map(|s| (*s, dealt_by(*s, bullet), dealt_by(*s, !bullet))).collect();
                let (kind_total, other_total) = (rows.iter().map(|r| r.1).sum::<f64>(), rows.iter().map(|r| r.2).sum::<f64>());
                let total = kind_total + other_total;
                let Some(top) = rows.iter().max_by(|a, b| a.1.total_cmp(&b.1)).filter(|r| r.1 > 0.0) else {
                    add("info", effect, format!("No {word} damage had come from {who} {when}."));
                    continue;
                };
                let share = pct(kind_total, total);
                let tone = if share >= FITS_PCT || pct(top.1, total) >= FITS_ONE_ENEMY_PCT {
                    "good"
                } else if share < MISSES_PCT && dedicated {
                    "bad"
                } else {
                    "info"
                };
                let text = format!(
                    "{} had dealt {} {word} damage to {} {when} ({}% of {}'s damage). {} damage was {}% of everything {who} had dealt.",
                    hero(top.0),
                    n(top.1),
                    p.hero_name,
                    n(pct(top.1, top.1 + top.2)),
                    hero(top.0),
                    if bullet { "Weapon" } else { "Spirit" },
                    n(share)
                );
                add(tone, effect, text);
            }

            // Healing reduction: who was healing.
            if reduces_healing(item) && sample.is_some() {
                let healers: Vec<(u32, f64)> = scope.iter().map(|s| (*s, at_sample(*s, &|v| v.healing))).collect();
                let taken: f64 = scope.iter().map(|s| at_sample(*s, &|v| v.damage_taken)).sum();
                let healed: f64 = healers.iter().map(|h| h.1).sum();
                if let Some(top) = healers.iter().max_by(|a, b| a.1.total_cmp(&b.1)) {
                    let share = pct(healed, taken);
                    let tone = if share >= HEAVY_HEALING_PCT { "good" } else { "info" };
                    let text = format!("{} had healed {} by then, the most of {who}, which together had healed {}% of the damage taken.", hero(top.0), n(top.1), n(share));
                    add(tone, format!("-{}% healing on the target", n(heal_cut_of(item))), text);
                }
            }

            // Resist reduction: how much resist the enemies had.
            for spirit in [true, false] {
                let value = shred_of(item, spirit);
                if value <= 0.0 || !(if spirit { shreds_spirit(item) } else { shreds_bullet(item) }) || sample.is_none() {
                    continue;
                }
                let kind = if spirit { "spirit" } else { "bullet" };
                let had: Vec<(u32, f64)> = scope.iter().map(|s| (*s, at_sample(*s, &|v| if spirit { v.spirit_resist } else { v.bullet_resist }))).collect();
                let average = had.iter().map(|h| h.1).sum::<f64>() / had.len().max(1) as f64;
                let Some(top) = had.iter().max_by(|a, b| a.1.total_cmp(&b.1)) else { continue };
                let tone = if average >= ENEMY_RESIST || top.1 >= ENEMY_RESIST + 10.0 { "good" } else { "info" };
                let text = format!("{} averaged {}% {kind} resist then; {} had the most with {}%.", capitalized(&who), n(average), hero(top.0), n(top.1));
                add(tone, format!("-{}% {kind} resist on the target", n(value)), text);
            }

            // Answers to what the enemy kits do: which of those abilities the enemies had by then.
            let enemy_has = |kind: Threat| kits.iter().filter(|k| enemy(k.0)).flat_map(|k| k.1.iter()).any(|t| t.0 == kind);
            let answered = counters::answers(item);
            let any_present = answered.iter().any(|kind| enemy_has(*kind));
            for (kind, what) in [
                (Threat::Lockdown, "stuns, roots or sleeps"),
                (Threat::Silence, "silences"),
                (Threat::Flight, "flying heroes"),
                (Threat::Channel, "channelled ultimates"),
            ] {
                // An item that answers several things is held against the ones the enemies had.
                if !answered.contains(&kind) || (any_present && !enemy_has(kind)) {
                    continue;
                }
                let theirs: Vec<&(Threat, u32, String)> = kits.iter().filter(|k| enemy(k.0)).flat_map(|k| k.1.iter()).filter(|t| t.0 == kind).collect();
                let ready: Vec<&str> = theirs.iter().filter(|t| t.1 <= x.time).map(|t| t.2.as_str()).collect();
                let (tone, text) = if !ready.is_empty() {
                    ("good", format!("The enemy team had {} by then: {}.", what, ready.join(", ")))
                } else if !theirs.is_empty() {
                    ("info", format!("No enemy had {what} yet; later: {}.", theirs.iter().map(|t| format!("{} ({})", t.2, clock(t.1))).collect::<Vec<_>>().join(", ")))
                } else {
                    ("info", format!("No enemy hero had {what} in this match."))
                };
                add(tone, format!("An answer to {what}"), text);
            }
        }
        p.item_reasons = reasons;

        let credits: Vec<f64> = out.iter().filter_map(|t| t.credit).collect();
        p.counter_score = (!credits.is_empty()).then(|| credits.iter().sum::<f64>() / credits.len() as f64 * 100.0);
        p.build_timing = out;
    }
}

fn capitalized(text: &str) -> String {
    let mut chars = text.chars();
    chars.next().map(|c| c.to_uppercase().collect::<String>() + chars.as_str()).unwrap_or_default()
}

fn n_pct(v: f64) -> String {
    format!("{v:.0}")
}

fn n(v: f64) -> String {
    let s = format!("{:.0}", v);
    // Thousands separators for the larger numbers.
    let (sign, digits) = if let Some(rest) = s.strip_prefix('-') { ("-", rest) } else { ("", s.as_str()) };
    let mut out = String::new();
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i) % 3 == 0 {
            out.push(',');
        }
        out.push(c);
    }
    format!("{sign}{out}")
}

/// Abilities that bring their hero back from lethal damage without the game recording a death.
const REVIVE_ABILITIES: [&str; 1] = ["ability_frank_revive"];
/// The laning phase is judged at the last snapshot up to this time.
const LANE_PHASE_S: u32 = 9 * 60;
/// The late game starts here at the latest; it starts earlier once a base objective has fallen.
const LATE_GAME_S: u32 = 25 * 60;

/// Damage-matrix rows that only repeat the sum of the detailed rows.
const AGGREGATES: [&str; 4] = ["Ability", "Bullet", "Melee", "Misc"];

/// Turns a damage source's internal name into what the player would call it.
fn source_label(data: &GameData, raw: &str) -> String {
    if raw.starts_with("citadel_weapon_") {
        return if raw.ends_with("_crit") { "Weapon (headshots)".into() } else { "Weapon".into() };
    }
    if raw.starts_with("ability_melee_") {
        return "Melee".into();
    }
    // The longest known class name the source starts with; what is left over names a part of it.
    let known = data
        .items
        .iter()
        .map(|i| (i.class_name.as_str(), i.name.as_str()))
        .chain(data.heroes.iter().flat_map(|h| h.abilities.iter().map(|a| (a.class_name.as_str(), a.name.as_str()))))
        .filter(|(class, _)| !class.is_empty() && raw.starts_with(class))
        .max_by_key(|(class, _)| class.len());
    match known {
        Some((class, name)) => {
            let rest = raw[class.len()..].trim_matches('_').replace('_', " ");
            if rest.is_empty() { name.to_string() } else { format!("{name} ({rest})") }
        }
        None => raw.to_string(),
    }
}

/// Totals of one kind (0 = damage, 1 = healing) from the damage matrix for one player.
/// `targets` are the player slots that count; the matrix also has rows for non-player targets.
fn sources(data: &GameData, matrix: &Value, slot: u32, kind: u64, targets: &[u32]) -> Vec<SourceAmount> {
    let details = matrix.get("source_details").cloned().unwrap_or(Value::Null);
    let (types, names) = (list(&details, "stat_type"), list(&details, "source_name"));
    let mut totals: BTreeMap<String, f64> = BTreeMap::new();
    for dealer in list(matrix, "damage_dealers").iter().filter(|d| int(d, "dealer_player_slot") == slot) {
        for source in list(dealer, "damage_sources") {
            let index = int(source, "source_details_index") as usize;
            let raw = names.get(index).and_then(Value::as_str).unwrap_or("");
            if types.get(index).and_then(Value::as_u64) != Some(kind) || AGGREGATES.contains(&raw) {
                continue;
            }
            // Each target has a running total over time; its last value is the match total.
            let amount: f64 = list(source, "damage_to_players")
                .iter()
                .filter(|t| targets.contains(&int(t, "target_player_slot")))
                .filter_map(|t| list(t, "damage").last().and_then(Value::as_f64))
                .sum();
            if amount > 0.0 {
                *totals.entry(source_label(data, raw)).or_default() += amount;
            }
        }
    }
    let mut out: Vec<SourceAmount> = totals.into_iter().map(|(name, amount)| SourceAmount { name, amount }).collect();
    out.sort_by(|a, b| b.amount.total_cmp(&a.amount));
    out
}

/// Sum of a permanent pickup's stat up to a point in time.
fn pickups(player: &Value, prefix: &str, time: u32) -> f64 {
    list(player, "power_up_buffs")
        .iter()
        .filter(|b| b.get("type").and_then(Value::as_str).is_some_and(|t| t.starts_with(prefix)))
        .map(|b| {
            let times = list(b, "pickup_times_s");
            list(b, "pickup_stat_values")
                .iter()
                .enumerate()
                .filter(|(i, _)| times.get(*i).and_then(Value::as_f64).unwrap_or(0.0) <= time as f64)
                .filter_map(|(_, v)| v.as_f64())
                .sum::<f64>()
        })
        .sum()
}

struct Events {
    purchases: Vec<Purchase>,
    abilities: Vec<AbilityStep>,
    /// Item class -> ability index, with the time of the purchase.
    imbues: Vec<(u32, String, usize)>,
    unknown: u32,
}

fn events(data: &GameData, hero: &Hero, player: &Value) -> Events {
    let mut out = Events { purchases: vec![], abilities: vec![], imbues: vec![], unknown: 0 };
    let ability_index = |id: u64| hero.abilities.iter().position(|a| a.id == id && id != 0);
    let mut seen = vec![0u8; hero.abilities.len()];
    for e in list(player, "items") {
        let (id, time) = (e.get("item_id").and_then(Value::as_u64).unwrap_or(0), int(e, "game_time_s"));
        if let Some(index) = ability_index(id) {
            let ability = &hero.abilities[index];
            out.abilities.push(AbilityStep { time, index, name: ability.name.clone(), icon: ability.icon.clone(), tier: seen[index].min(3) });
            seen[index] = seen[index].saturating_add(1);
        } else if let Some(item) = data.items.iter().find(|i| i.id == id) {
            let sold = int(e, "sold_time_s");
            out.purchases.push(Purchase {
                time,
                item: item.class_name.clone(),
                name: item.name.clone(),
                icon: item.icon.clone(),
                category: item.category.clone(),
                cost: item.cost,
                sold_at: (sold > 0).then_some(sold),
            });
            let imbued = e.get("imbued_ability_id").and_then(Value::as_u64).unwrap_or(0);
            if let Some(index) = ability_index(imbued) {
                out.imbues.push((time, item.class_name.clone(), index));
            }
        } else if int(e, "upgrade_id") > 0 {
            // An item id the data does not know; ability ids of other kinds are not counted.
            out.unknown += 1;
        }
    }
    out
}

/// The build a player had at a point in the match.
fn build_at(hero: &Hero, events: &Events, time: u32, level: u32) -> BuildInput {
    let mut tiers = vec![0u8; hero.abilities.len().max(4)];
    for step in events.abilities.iter().filter(|s| s.time <= time) {
        tiers[step.index] = tiers[step.index].max(step.tier);
    }
    BuildInput {
        hero: hero.class_name.clone(),
        // An item used up by an upgrade is gone from the inventory; the upgrade then counts at its full price,
        // which is the same total as component plus discounted upgrade.
        purchases: events
            .purchases
            .iter()
            .filter(|p| p.time <= time && p.sold_at.is_none_or(|sold| sold > time))
            .map(|p| p.item.clone())
            .collect(),
        souls: hero.levels.iter().find(|l| l.level == level).map(|l| l.required_souls),
        ability_tiers: tiers,
        imbues: events.imbues.iter().filter(|(t, _, _)| *t <= time).map(|(_, item, index)| (item.clone(), *index)).collect(),
        ..Default::default()
    }
}

pub fn parse(data: &GameData, json: &str) -> Result<MatchReport, String> {
    let root: Value = serde_json::from_str(json).map_err(|e| format!("Unreadable match data: {e}"))?;
    let info = root.get("match_info").ok_or("The response has no match information")?;
    let winning_team = int(info, "winning_team");
    let matrix = info.get("damage_matrix").cloned().unwrap_or(Value::Null);
    // The ids of the per-player accuracy counters.
    let custom_id = |name: &str| {
        list(info, "custom_user_stats")
            .iter()
            .find(|s| s.get("name").and_then(Value::as_str) == Some(name))
            .map(|s| int(s, "id"))
    };
    let (shots_id, hits_id, heads_id) = (
        custom_id("Enemy Hero Accuracy##Shots"),
        custom_id("Enemy Hero Accuracy##Hits"),
        custom_id("Enemy Hero Accuracy##Headshots"),
    );
    let (parry_id, parry_miss_id) = (custom_id("Parry Success"), custom_id("Parry Miss"));
    let (unspent_souls_id, unspent_ap_id) = (custom_id("Unspent Gold Minutes"), custom_id("Unspent AP Minutes"));
    let (bridge_id, permanent_id) = (custom_id("PowerUp Temp"), custom_id("PowerUp Permanent"));
    let minutes = (num(info, "duration_s") / 60.0).max(1.0);

    let slots_of = |enemy_of: Option<u32>| -> Vec<u32> {
        list(info, "players")
            .iter()
            .filter(|p| enemy_of.is_none_or(|team| int(p, "team") != team))
            .map(|p| int(p, "player_slot"))
            .collect()
    };
    let everyone = slots_of(None);

    let mut players = vec![];
    for p in list(info, "players") {
        let hero_id = p.get("hero_id").and_then(Value::as_u64).unwrap_or(0);
        let Some(hero) = data.heroes.iter().find(|h| h.id == hero_id) else { continue };
        let (slot, team) = (int(p, "player_slot"), int(p, "team"));
        let ev = events(data, hero, p);
        let stats = list(p, "stats");
        let last = stats.last().cloned().unwrap_or(Value::Null);
        // The last snapshot of the laning phase.
        let lane_sample = stats.iter().filter(|s| int(s, "time_stamp_s") <= LANE_PHASE_S).last().cloned().unwrap_or(Value::Null);
        let custom = |id: Option<u32>| {
            id.and_then(|id| list(&last, "custom_user_stats").iter().find(|s| int(s, "id") == id).map(|s| num(s, "value")))
                .unwrap_or(0.0)
        };

        let timeline: Vec<Sample> = stats
            .iter()
            .map(|s| {
                let (time, level) = (int(s, "time_stamp_s"), int(s, "level"));
                let build = build_at(hero, &ev, time, level);
                let sheet = calc::compute(data, &build).ok();
                let (pickup_health, pickup_spirit, pickup_weapon) =
                    (pickups(p, "hp_permanent", time), pickups(p, "spirit_permanent", time), pickups(p, "wp_permanent", time));
                let stat = |key: &str, extra: f64| sheet.as_ref().and_then(|sh| sh.stat(key)).map(|v| v + extra);
                Sample {
                    time,
                    level,
                    net_worth: int(s, "net_worth"),
                    kills: int(s, "kills"),
                    deaths: int(s, "deaths"),
                    assists: int(s, "assists"),
                    hero_damage: num(s, "player_damage"),
                    damage_taken: num(s, "player_damage_taken"),
                    healing: num(s, "player_healing"),
                    objective_damage: num(s, "boss_damage"),
                    creep_damage: num(s, "creep_damage"),
                    neutral_damage: num(s, "neutral_damage"),
                    mitigated: num(s, "damage_mitigated"),
                    last_hits: int(s, "creep_kills") + int(s, "neutral_kills"),
                    denies: int(s, "denies"),
                    souls: soul_sources(s),
                    game_health: num(s, "max_health"),
                    game_spirit: num(s, "tech_power"),
                    game_weapon: num(s, "weapon_power"),
                    calc_health: stat("max_health", pickup_health),
                    calc_spirit: stat("spirit_power", pickup_spirit),
                    calc_weapon: stat("weapon_damage_pct", pickup_weapon),
                    bullet_resist: stat("bullet_resist", 0.0).unwrap_or(0.0),
                    spirit_resist: stat("spirit_resist", 0.0).unwrap_or(0.0),
                    pickup_health,
                    pickup_spirit,
                    pickup_weapon,
                    build,
                }
            })
            .collect();

        let level = int(p, "level");
        players.push(PlayerReport {
            slot,
            account_id: p.get("account_id").and_then(Value::as_u64).unwrap_or(0),
            player_name: String::new(),
            team,
            won: team == winning_team,
            hero: hero.class_name.clone(),
            hero_name: hero.name.clone(),
            hero_icon: hero.icon.clone(),
            kills: int(p, "kills"),
            deaths: int(p, "deaths"),
            assists: int(p, "assists"),
            net_worth: int(p, "net_worth"),
            level,
            last_hits: int(p, "last_hits"),
            denies: int(p, "denies"),
            hero_damage: num(&last, "player_damage"),
            damage_taken: num(&last, "player_damage_taken"),
            healing: num(&last, "player_healing"),
            objective_damage: num(&last, "boss_damage"),
            creep_damage: num(&last, "creep_damage"),
            neutral_damage: num(&last, "neutral_damage"),
            mitigated: num(&last, "damage_mitigated"),
            game_mvp_rank: p.get("mvp_rank").and_then(Value::as_u64).filter(|r| *r > 0).map(|r| r as u32),
            macro_stats: {
                let awards = list(p, "accolades");
                let mut stats: Vec<SourceAmount> = MACRO_STATS
                    .iter()
                    .filter_map(|(id, name)| {
                        let award = awards.iter().find(|a| a.get("accolade_id").and_then(Value::as_u64) == Some(*id))?;
                        Some(SourceAmount { name: name.to_string(), amount: num(award, "accolade_stat_value") })
                    })
                    .collect();
                for (name, id) in [("Parries landed", parry_id), ("Parries missed", parry_miss_id)] {
                    stats.push(SourceAmount { name: name.into(), amount: custom(id) });
                }
                stats
            },
            souls: {
                let mut souls = soul_sources(&last);
                souls.sort_by(|a, b| b.amount.total_cmp(&a.amount));
                souls
            },
            kills_at: list(info, "players")
                .iter()
                .flat_map(|victim| list(victim, "death_details").iter().map(move |d| (victim, d)))
                .filter(|(_, d)| int(d, "killer_player_slot") == slot)
                .map(|(victim, d)| Duel { time: int(d, "game_time_s"), other: int(victim, "player_slot") })
                .collect(),
            deaths_at: list(p, "death_details")
                .iter()
                .map(|d| Duel { time: int(d, "game_time_s"), other: int(d, "killer_player_slot") })
                .collect(),
            heal_prevented: num(&last, "heal_prevented"),
            self_healing: num(&last, "self_healing"),
            teammate_support: num(&last, "teammate_healing") + num(&last, "teammate_barriering"),
            close_calls: 0,
            revives_at: vec![],
            role: String::new(),
            role_why: String::new(),
            late_fights_skipped: 0,
            objectives_taken: vec![],
            fights_won: 0,
            fights_lost: 0,
            first_deaths: 0,
            caught_deaths: 0,
            costly_deaths: 0,
            lane_damage_dealt: 0.0,
            lane_damage_taken: 0.0,
            lane_kills_on: 0,
            lane_deaths_to: 0,
            roams: vec![],
            wins_farmed: 0,
            mid_boss_alive: 0,
            mid_boss_present: 0,
            titles: vec![],
            strengths: vec![],
            gaps: vec![],
            lane_detail: {
                let at = |key: &str| num(&lane_sample, key);
                let source = |id: u64| list(&lane_sample, "gold_sources").iter().filter(|r| r.get("source").and_then(Value::as_u64) == Some(id)).map(|r| num(r, "gold") + num(r, "gold_orbs")).sum::<f64>();
                let lane_custom = |id: Option<u32>| id.and_then(|id| list(&lane_sample, "custom_user_stats").iter().find(|s| int(s, "id") == id).map(|s| num(s, "value"))).unwrap_or(0.0);
                LaneDetail {
                    souls_at: stats.iter().filter(|s| int(s, "time_stamp_s") <= LANE_PHASE_S).map(|s| num(s, "net_worth")).collect(),
                    trooper_last_hits: at("creep_kills") as u32,
                    possible_last_hits: at("possible_creeps") as u32,
                    denies: at("denies") as u32,
                    deny_souls: at("gold_denied"),
                    kills: at("kills") as u32,
                    deaths: at("deaths") as u32,
                    assists: at("assists") as u32,
                    hero_damage: at("player_damage"),
                    damage_taken: at("player_damage_taken"),
                    healing: at("player_healing"),
                    guardian_damage: at("boss_damage"),
                    camp_kills: at("neutral_kills") as u32,
                    camp_souls: source(3),
                    box_souls: source(12),
                    // The game's own count of permanent power-ups; one statue can grant several stat lines.
                    statue_buffs: lane_custom(permanent_id) as u32,
                    souls_lost_on_death: at("gold_death_loss"),
                    accuracy_pct: pct(lane_custom(hits_id), lane_custom(shots_id)),
                }
            },
            bridge_buffs: list(p, "power_up_buffs")
                .iter()
                .filter(|b| b.get("is_permanent").and_then(Value::as_bool) == Some(false) && num(b, "value") > 0.0)
                .map(|b| SourceAmount {
                    name: match b.get("type").and_then(Value::as_str).unwrap_or("") {
                        "gun_powerup_pickup" => "Gun".to_string(),
                        "casting_powerup_pickup" => "Casting".to_string(),
                        "survival_powerup_pickup" => "Survival".to_string(),
                        "movement_powerup_pickup" => "Movement".to_string(),
                        other => other.trim_end_matches("_powerup_pickup").to_string(),
                    },
                    amount: num(b, "value"),
                })
                .collect(),
            bridge_buffs_at: stats
                .iter()
                .map(|s| bridge_id.and_then(|id| list(s, "custom_user_stats").iter().find(|c| int(c, "id") == id).map(|c| num(c, "value"))).unwrap_or(0.0))
                .collect(),
            lane: int(p, "assigned_lane"),
            lane_opponents: vec![],
            lane_time: int(&lane_sample, "time_stamp_s"),
            lane_souls: num(&lane_sample, "net_worth"),
            lane_souls_diff: 0.0,
            lane_last_hits: int(&lane_sample, "creep_kills") + int(&lane_sample, "neutral_kills"),
            lane_denies: int(&lane_sample, "denies"),
            lane_kills: int(&lane_sample, "kills"),
            lane_deaths: int(&lane_sample, "deaths"),
            lane_hero_damage: num(&lane_sample, "player_damage"),
            last_hit_pct: pct(num(&last, "creep_kills"), num(&last, "possible_creeps")),
            trooper_last_hits: int(&last, "creep_kills"),
            possible_last_hits: int(&last, "possible_creeps"),
            // The game adds up "souls held × minutes"; dividing by the length gives the average held.
            avg_unspent_souls: custom(unspent_souls_id) / minutes,
            avg_unspent_ap: custom(unspent_ap_id) / minutes,
            dead_seconds: list(p, "death_details").iter().map(|d| int(d, "death_duration_s")).sum(),
            souls_lost_on_death: num(&last, "gold_death_loss"),
            time_split: vec![],
            fights_possible: 0,
            fights_joined: 0,
            fights_missed: 0,
            insights: vec![],
            build_timing: vec![],
            counter_score: None,
            item_reasons: vec![],
            enabled: vec![],
            enabled_total: 0.0,
            hero_shots: custom(shots_id),
            hero_hits: custom(hits_id),
            hero_headshots: custom(heads_id),
            build: build_at(hero, &ev, u32::MAX, level),
            purchases: ev.purchases,
            abilities: ev.abilities,
            timeline,
            damage_sources: sources(data, &matrix, slot, 0, &slots_of(Some(team))),
            heal_sources: sources(data, &matrix, slot, 1, &everyone),
            unknown_items: ev.unknown,
        });
    }
    if players.is_empty() {
        return Err("No players with a known hero in this match".into());
    }
    players.sort_by_key(|p| (p.team, p.slot));
    for p in &mut players {
        p.kills_at.sort_by_key(|k| k.time);
    }
    // Team fights from the recorded positions.
    let deaths: Vec<fights::Death> = list(info, "players")
        .iter()
        .flat_map(|p| {
            list(p, "death_details").iter().map(move |d| {
                let pos = d.get("death_pos").cloned().unwrap_or(Value::Null);
                (int(d, "game_time_s"), int(p, "player_slot"), num(&pos, "x"), num(&pos, "y"))
            })
        })
        .collect();
    let fallen: Vec<(u32, u32, String)> = list(info, "objectives")
        .iter()
        .filter(|o| int(o, "destroyed_time_s") > 0)
        .map(|o| {
            let (name, _) = objective(o.get("team_objective_id").and_then(Value::as_u64).unwrap_or(u64::MAX));
            // The destroying team is the other one.
            (int(o, "destroyed_time_s"), 1 - int(o, "team").min(1), name.to_string())
        })
        .collect();
    let tracks = fights::tracks(info, |slot| players.iter().find(|p| p.slot == slot).map(|p| p.team));
    let duration = int(info, "duration_s");
    let late_from = fallen
        .iter()
        .filter(|o| ["Base guardians", "Shrine", "Patron", "Patron (final form)"].contains(&o.2.as_str()))
        .map(|o| o.0)
        .fold(LATE_GAME_S, u32::min);
    // Who took each objective: the enemies standing at it in its last seconds.
    struct Fall {
        team: u32,
        id: u64,
        spot: Option<(f64, f64)>,
        takers: Vec<u32>,
    }
    let falls: Vec<Fall> = list(info, "objectives")
        .iter()
        .map(|o| {
            let (team, id, time) = (int(o, "team"), o.get("team_objective_id").and_then(Value::as_u64).unwrap_or(u64::MAX), int(o, "destroyed_time_s"));
            let spot = OBJECTIVE_SPOTS.iter().find(|s| s.0 == team && s.1 == id).map(|s| (s.2, s.3));
            let patron = OBJECTIVE_SPOTS.iter().find(|s| s.0 == team && s.1 == 9).map(|s| (s.2, s.3));
            let (place, reach) = match (spot, patron) {
                (Some(spot), _) => (Some(spot), TAKER_REACH),
                (None, patron) => (patron, TAKER_REACH_IN_BASE),
            };
            let takers = match place {
                Some((left, top)) if time > 0 => tracks
                    .iter()
                    .filter(|t| t.team != team)
                    .filter(|t| t.seconds_near(in_world(left, top), reach, time.saturating_sub(TAKER_WINDOW_S), time + 1) >= TAKER_SECONDS)
                    .map(|t| t.slot)
                    .collect(),
                _ => vec![],
            };
            Fall { team, id, spot, takers }
        })
        .collect();
    let describe = |team: u32, id: u64| {
        let (name, lane) = objective(id);
        format!("the enemy's {name}{}", lane.map(|l| format!(" (lane {l})")).unwrap_or_default()).replacen(&team.to_string(), &team.to_string(), 0)
    };
    let credited: Vec<(u32, u32, String)> = list(info, "objectives")
        .iter()
        .zip(&falls)
        .filter(|(o, _)| int(o, "destroyed_time_s") > 0)
        .flat_map(|(o, fall)| fall.takers.iter().map(move |slot| (int(o, "destroyed_time_s"), *slot, fall.team, fall.id)))
        .map(|(time, slot, team, id)| (time, slot, describe(team, id)))
        .collect();
    for p in &mut players {
        p.objectives_taken = credited
            .iter()
            .filter(|c| c.1 == p.slot)
            .map(|c| Taken { time: c.0, name: c.2.trim_start_matches("the enemy's ").to_string() })
            .collect();
        p.objectives_taken.sort_by_key(|t| t.time);
    }
    // Souls at a moment between two snapshots, read off the line between them.
    let souls_at = |p: &PlayerReport, time: u32| -> f64 {
        let mut before = (0.0, 0.0);
        for s in &p.timeline {
            let point = (s.time as f64, s.net_worth as f64);
            if s.time >= time {
                let span = (point.0 - before.0).max(1.0);
                return before.1 + (point.1 - before.1) * (time as f64 - before.0) / span;
            }
            before = point;
        }
        before.1
    };
    // How the match stood at a moment: walkers each team still had, and each team's souls.
    let walkers_at = |time: u32| -> Vec<u32> {
        (0..2u32)
            .map(|team| {
                list(info, "objectives")
                    .iter()
                    .filter(|o| int(o, "team") == team && (5..=8).contains(&o.get("team_objective_id").and_then(Value::as_u64).unwrap_or(0)))
                    .filter(|o| int(o, "destroyed_time_s") == 0 || int(o, "destroyed_time_s") > time)
                    .count() as u32
            })
            .collect()
    };
    let team_souls_at = |time: u32| -> Vec<f64> {
        (0..2u32).map(|team| players.iter().filter(|p| p.team == team).map(|p| souls_at(p, time)).sum()).collect()
    };
    let even_at = |time: u32| -> bool {
        let (walkers, souls) = (walkers_at(time), team_souls_at(time));
        let average = (souls[0] + souls[1]) / players.len().max(1) as f64;
        let souls_apart = (souls[0] - souls[1]).abs() / souls[0].max(souls[1]).max(1.0) * 100.0;
        walkers[0].abs_diff(walkers[1]) <= EVEN_WALKERS || (souls_apart <= EVEN_SOULS_PCT && average < FULL_BUILD_SOULS)
    };
    let mut fights = fights::detect(&tracks, &deaths, &fallen, &credited, duration, late_from, &even_at);
    for f in &mut fights {
        f.walkers = walkers_at(f.start);
        f.team_souls = team_souls_at(f.start);
    }
    let mid_boss: Vec<MidBoss> = list(info, "mid_boss")
        .iter()
        .map(|b| {
            let (time, team_killed, team_claimed) = (int(b, "destroyed_time_s"), int(b, "team_killed"), int(b, "team_claimed"));
            let (from, to) = (time.saturating_sub(PIT_BEFORE_S), time + PIT_AFTER_S);
            let died: Vec<u32> = deaths.iter().filter(|d| d.0 >= from && d.0 <= to && ((d.2 - PIT.0).powi(2) + (d.3 - PIT.1).powi(2)).sqrt() < PIT_DEATH_REACH).map(|d| d.1).collect();
            let at_pit = |t: &fights::Track| t.seconds_near(PIT, PIT_REACH, from, to) >= PIT_SECONDS || died.contains(&t.slot);
            let count = |team: u32| tracks.iter().filter(|t| t.team == team && at_pit(t)).count();
            let contested = count(0) >= 2 && count(1) >= 2;
            let late = time >= late_from;
            let even = late && even_at(time);
            let sides: Vec<fights::Side> = (0..2u32)
                .map(|team| {
                    let mut side = fights::Side { present: vec![], absent: vec![], dead: vec![], deaths: vec![] };
                    for t in tracks.iter().filter(|t| t.team == team) {
                        let then = time.saturating_sub(5);
                        if at_pit(t) {
                            side.present.push(t.slot);
                            if died.contains(&t.slot) {
                                side.deaths.push(t.slot);
                            }
                        } else if !t.alive_at(then) {
                            side.dead.push(t.slot);
                        } else {
                            let trade = credited.iter().find(|c| c.1 == t.slot && c.0 + PIT_AFTER_S >= from && c.0 <= to + 20);
                            let state = t.combat_at(then);
                            let doing = match (trade, state) {
                                (Some(c), _) => format!("taking {}", c.2),
                                (None, 1) => "fighting elsewhere".to_string(),
                                (None, 2) => "hitting troopers or an objective".to_string(),
                                (None, 3) => "farming a neutral camp".to_string(),
                                (None, _) => "out of combat".to_string(),
                            };
                            let (x, y) = t.place(then);
                            side.absent.push(fights::Absence {
                                slot: t.slot,
                                doing,
                                distance: ((x - PIT.0).powi(2) + (y - PIT.1).powi(2)).sqrt() / crate::raw::UNITS_PER_METER,
                                // Needed when both teams showed up: another fight is an excuse, an objective only
                                // while the match is not in the balance.
                                missed: contested && state != 1 && (trade.is_none() || even),
                            });
                        }
                    }
                    side
                })
                .collect();
            let until = (time + REJUVENATOR_S).min(duration);
            let after = |own: bool| -> Vec<Taken> {
                fallen.iter().filter(|o| o.0 > time && o.0 <= until && (o.1 == team_claimed) == own).map(|o| Taken { time: o.0, name: o.2.clone() }).collect()
            };
            let kills: Vec<u32> = (0..2u32)
                .map(|team| players.iter().filter(|p| p.team != team).flat_map(|p| p.deaths_at.iter()).filter(|d| d.time > time && d.time <= until).count() as u32)
                .collect();
            let lead = |at: u32| {
                let souls = team_souls_at(at);
                souls[team_claimed.min(1) as usize] - souls[1 - team_claimed.min(1) as usize]
            };
            MidBoss {
                time,
                team_killed,
                team_claimed,
                stolen: team_killed != team_claimed,
                sides,
                contested,
                late,
                even,
                walkers: walkers_at(time),
                team_souls: team_souls_at(time),
                window_s: until - time,
                taken: after(true),
                lost: after(false),
                kills,
                lead_change: lead(until) - lead(time),
            }
        })
        .collect();
    for f in &mut fights {
        let mut kills: Vec<fights::FightKill> = players
            .iter()
            .filter(|p| f.sides.iter().any(|s| s.deaths.contains(&p.slot)))
            .flat_map(|p| {
                p.deaths_at
                    .iter()
                    .filter(|d| d.time + 3 >= f.start && d.time <= f.end + 6)
                    .map(move |d| fights::FightKill { time: d.time, victim: p.slot, killer: d.other })
            })
            .collect();
        kills.sort_by_key(|k| k.time);
        f.kills = kills;
        f.souls = f
            .sides
            .iter()
            .map(|side| side.present.iter().filter_map(|slot| players.iter().find(|p| p.slot == *slot)).map(|p| souls_at(p, f.start)).sum())
            .collect();
        // Sides are in team order; the team that lost fewer players took the fight.
        let lost: Vec<usize> = f.sides.iter().map(|side| side.deaths.len()).collect();
        f.winner = match (lost.first(), lost.get(1)) {
            (Some(a), Some(b)) if a < b => Some(0),
            (Some(a), Some(b)) if b < a => Some(1),
            _ => None,
        };
    }
    for p in &mut players {
        let track = tracks.iter().find(|t| t.slot == p.slot);
        if let Some(track) = track {
            let split = track.time_split(duration);
            // The recorded respawn timers can run past the end of the match; the position record does not.
            p.dead_seconds = split[4];
            p.time_split = ["Fighting heroes", "Troopers and objectives", "Neutral camps", "Out of combat", "Dead"]
                .iter()
                .zip(split)
                .map(|(name, seconds)| SourceAmount { name: name.to_string(), amount: seconds as f64 })
                .collect();
        }
        let can_revive = data.hero(&p.hero).is_some_and(|h| h.abilities.iter().any(|a| REVIVE_ABILITIES.contains(&a.class_name.as_str())));
        if can_revive {
            p.revives_at = track.map(|t| t.revives(duration)).unwrap_or_default();
        }
        // A revive is not a close call: the hero did go down.
        let mut down: Vec<u32> = p.deaths_at.iter().map(|d| d.time).collect();
        down.extend(&p.revives_at);
        p.close_calls = track.map(|t| t.close_calls(&down, duration)).unwrap_or(0);
        for f in &fights {
            p.first_deaths += f.kills.first().is_some_and(|k| k.victim == p.slot) as u32;
            for (team, side) in f.sides.iter().enumerate() {
                if side.present.contains(&p.slot) {
                    match f.winner {
                        Some(winner) if winner as usize == team => p.fights_won += 1,
                        Some(_) => p.fights_lost += 1,
                        None => {}
                    }
                    p.fights_possible += 1;
                    p.fights_joined += 1;
                } else if let Some(absence) = side.absent.iter().find(|a| a.slot == p.slot) {
                    p.fights_possible += 1;
                    p.fights_missed += absence.missed as u32;
                    p.late_fights_skipped += (absence.missed && f.late) as u32;
                }
            }
        }
    }
    // The lanes: a line from walker to guardian to guardian to walker for every lane the map has.
    let spot = |team: u32, id: u64| OBJECTIVE_SPOTS.iter().find(|s| s.0 == team && s.1 == id).map(|s| in_world(s.2, s.3));
    let lines: Vec<(u64, Vec<(f64, f64)>)> = (1..=4u64)
        .filter_map(|id| Some((id, vec![spot(0, id + 4)?, spot(0, id)?, spot(1, id)?, spot(1, id + 4)?])))
        .collect();
    let line_name = |id: u64| match OBJECTIVE_SPOTS.iter().find(|s| s.0 == 0 && s.1 == id).map(|s| s.2) {
        Some(left) if left < 0.33 => "West",
        Some(left) if left > 0.66 => "East",
        _ => "Middle",
    };
    // The lane a player really played: the line they stood closest to in the first minutes. Players swap lanes
    // at the start often enough that the assigned lane cannot be taken at its word.
    let played: Vec<(u32, u64)> = players
        .iter()
        .filter_map(|p| {
            let track = tracks.iter().find(|t| t.slot == p.slot)?;
            let stood: Vec<(f64, f64)> = (60..=420u32).step_by(10).filter(|s| track.alive_at(*s)).map(|s| track.place(s)).collect();
            if stood.is_empty() {
                return None;
            }
            // Capped, so that walking back from the base does not outweigh the time spent in the lane.
            let far = |line: &[(f64, f64)]| stood.iter().map(|at| to_line(*at, line).min(LANE_BAND * 3.0)).sum::<f64>();
            lines.iter().map(|(id, line)| (*id, far(line))).min_by(|a, b| a.1.total_cmp(&b.1)).map(|nearest| (p.slot, nearest.0))
        })
        .collect();
    // A line goes by the lane number most of its players were assigned.
    let mut lane_lines: Vec<(u32, u64)> = vec![];
    for (id, _) in &lines {
        let assigned: Vec<u32> = players.iter().filter(|p| played.iter().any(|x| x.0 == p.slot && x.1 == *id)).map(|p| p.lane).collect();
        let Some(number) = assigned.iter().copied().max_by_key(|n| assigned.iter().filter(|x| *x == n).count()) else { continue };
        let number = if lane_lines.iter().any(|l| l.0 == number) { 100 + *id as u32 } else { number };
        lane_lines.push((number, *id));
    }
    for p in &mut players {
        let line = played.iter().find(|x| x.0 == p.slot).and_then(|x| lane_lines.iter().find(|l| l.1 == x.1));
        if let Some(line) = line {
            p.lane = line.0;
        }
    }
    // Lane opponents: the enemies who played the same lane.
    let lanes: Vec<(u32, u32, u32, f64)> = players.iter().map(|p| (p.slot, p.team, p.lane, p.lane_souls)).collect();
    for p in &mut players {
        let opponents: Vec<&(u32, u32, u32, f64)> = lanes.iter().filter(|o| o.1 != p.team && o.2 == p.lane).collect();
        p.lane_opponents = opponents.iter().map(|o| o.0).collect();
        if !opponents.is_empty() {
            p.lane_souls_diff = p.lane_souls - opponents.iter().map(|o| o.3).sum::<f64>() / opponents.len() as f64;
        }
    }
    // What the winner made of the fights it clearly won.
    let fight_starts: Vec<u32> = fights.iter().map(|f| f.start).collect();
    for f in &mut fights {
        let Some(winner) = f.winner else { continue };
        let lost = |team: usize| f.sides.get(team).map(|s| s.deaths.len() as u32).unwrap_or(0);
        let lead = lost(1 - winner as usize).saturating_sub(lost(winner as usize));
        if lead < FOLLOW_LEAD {
            continue;
        }
        let next = fight_starts.iter().copied().filter(|s| *s > f.end).min().unwrap_or(u32::MAX);
        let until = (f.end + FOLLOW_S).min(next).min(duration);
        let mut taken: Vec<String> = f.objectives.iter().filter(|o| o.0 == winner).map(|o| o.1.clone()).collect();
        taken.extend(fallen.iter().filter(|o| o.1 == winner && o.0 > f.end + 20 && o.0 <= until).map(|o| o.2.clone()));
        let side = &f.sides[winner as usize];
        let farming = side
            .present
            .iter()
            .copied()
            .filter(|slot| !side.deaths.contains(slot))
            .filter(|slot| tracks.iter().find(|t| t.slot == *slot).is_some_and(|t| t.seconds_in(3, f.end, until) >= FOLLOW_FARM_S))
            .collect();
        f.follow = Some(fights::FollowUp {
            window_s: until.saturating_sub(f.end),
            lead,
            taken,
            boss: mid_boss.iter().any(|b| b.team_claimed == winner && b.time > f.start && b.time <= until),
            farming,
        });
    }
    for p in &mut players {
        p.wins_farmed = fights
            .iter()
            .filter_map(|f| f.follow.as_ref())
            .filter(|follow| follow.taken.is_empty() && !follow.boss && follow.farming.contains(&p.slot))
            .count() as u32;
    }

    let (matrix_times, matrix_totals) = damage_over_time(&matrix);
    let lane_index = matrix_times.iter().rposition(|t| *t <= LANE_PHASE_S);
    let hurt = |from: u32, to: u32| -> f64 {
        lane_index.map(|i| matrix_totals.iter().filter(|(k, _)| k.0 == from && k.1 == to).map(|(_, v)| v.get(i).copied().unwrap_or(0.0)).sum()).unwrap_or(0.0)
    };
    let teams: Vec<(u32, u32)> = players.iter().map(|p| (p.slot, p.team)).collect();
    for p in &mut players {
        p.lane_damage_dealt = p.lane_opponents.iter().map(|o| hurt(p.slot, *o)).sum();
        p.lane_damage_taken = p.lane_opponents.iter().map(|o| hurt(*o, p.slot)).sum();
        p.lane_kills_on = p.kills_at.iter().filter(|k| k.time <= LANE_PHASE_S && p.lane_opponents.contains(&k.other)).count() as u32;
        p.lane_deaths_to = p.deaths_at.iter().filter(|d| d.time <= LANE_PHASE_S && p.lane_opponents.contains(&d.other)).count() as u32;
        // Roams: stretches of the laning phase spent in another lane, away from the own one.
        let (Some(track), Some(home)) = (tracks.iter().find(|t| t.slot == p.slot), lane_lines.iter().find(|l| l.0 == p.lane).map(|l| l.1)) else { continue };
        let home_line = lines.iter().find(|l| l.0 == home).map(|l| l.1.as_slice()).unwrap_or(&[]);
        let visiting = |time: u32| -> Option<u64> {
            if !track.alive_at(time) || to_line(track.place(time), home_line) < LANE_BAND {
                return None;
            }
            lines.iter().filter(|l| l.0 != home).find(|l| to_line(track.place(time), &l.1) < LANE_BAND).map(|l| l.0)
        };
        let mut runs: Vec<(u32, u32, u64)> = vec![];
        for time in ROAM_FROM_S..LANE_PHASE_S.min(duration) {
            let Some(line) = visiting(time) else { continue };
            match runs.last_mut() {
                Some(run) if run.2 == line && time <= run.1 + ROAM_GAP_S => run.1 = time,
                _ => runs.push((time, time, line)),
            }
        }
        p.roams = runs
            .into_iter()
            .filter(|run| run.1 - run.0 >= ROAM_MIN_S)
            .map(|(start, end, line)| {
                let there = lines.iter().find(|l| l.0 == line).map(|l| l.1.as_slice()).unwrap_or(&[]);
                let died_there: Vec<&fights::Death> = deaths
                    .iter()
                    .filter(|d| d.0 >= start && d.0 <= end + 8 && to_line((d.2, d.3), there) < LANE_BAND * 1.3)
                    .filter(|d| teams.iter().any(|t| t.0 == d.1 && t.1 != p.team))
                    .collect();
                Roam {
                    start,
                    seconds: end - start + 1,
                    lane: lane_lines.iter().find(|l| l.1 == line).map(|l| l.0).unwrap_or(0),
                    name: line_name(line).to_string(),
                    enemy_deaths: died_there.len() as u32,
                    kills: p.kills_at.iter().filter(|k| k.time >= start && k.time <= end + 8).count() as u32,
                    died: p.deaths_at.iter().any(|d| d.time >= start && d.time <= end + 8),
                }
            })
            .collect();
    }
    let lane_reports: Vec<LaneReport> = lane_lines
        .iter()
        .map(|(lane, line)| {
            let guardian = |team: u32| list(info, "objectives").iter().find(|o| int(o, "team") == team && o.get("team_objective_id").and_then(Value::as_u64) == Some(*line));
            let sides: Vec<LaneSide> = (0..2u32)
                .map(|team| {
                    let here: Vec<&PlayerReport> = players.iter().filter(|p| p.team == team && p.lane == *lane).collect();
                    LaneSide {
                        players: here.iter().map(|p| p.slot).collect(),
                        souls: here.iter().map(|p| p.lane_souls).sum(),
                        last_hits: here.iter().map(|p| p.lane_last_hits).sum(),
                        denies: here.iter().map(|p| p.lane_denies).sum(),
                        kills: here.iter().map(|p| p.lane_kills_on).sum(),
                        damage: here.iter().map(|p| p.lane_damage_dealt).sum(),
                        guardian_lost_at: guardian(team).map(|o| int(o, "destroyed_time_s")).filter(|t| *t > 0),
                        guardian_first_hit: guardian(team).map(|o| int(o, "first_damage_time_s")).filter(|t| *t > 0),
                        guardian_player_damage: guardian(team).map(|o| num(o, "player_damage")).unwrap_or(0.0),
                        guardian_trooper_damage: guardian(team).map(|o| num(o, "creep_damage")).unwrap_or(0.0),
                        souls_at: (0..here.iter().map(|p| p.lane_detail.souls_at.len()).max().unwrap_or(0))
                            .map(|i| here.iter().map(|p| p.lane_detail.souls_at.get(i).copied().unwrap_or(0.0)).sum())
                            .collect(),
                    }
                })
                .collect();
            let (a, b) = (sides[0].souls, sides[1].souls);
            let winner = if (a - b).abs() / a.max(b).max(1.0) * 100.0 < LANE_WON_PCT { None } else { Some((b > a) as u32) };
            LaneReport { lane: *lane, name: line_name(*line).to_string(), sides, winner }
        })
        .collect();

    let matrix_rows = rows(&matrix);
    insights(data, &mut players, &matrix_rows);
    // Who hurt and killed whom.
    let mut duels: Vec<DuelCell> = vec![];
    for from in &players {
        for to in players.iter().filter(|to| to.team != from.team) {
            duels.push(DuelCell {
                from: from.slot,
                to: to.slot,
                damage: matrix_rows.iter().filter(|r| r.kind == 0 && r.dealer == from.slot && r.target == to.slot).map(|r| r.amount).sum(),
                kills: to.deaths_at.iter().filter(|d| d.other == from.slot).count() as u32,
            });
        }
    }
    let map = MapData {
        image: MAP_IMAGE.to_string(),
        step: MAP_STEP_S,
        paths: tracks
            .iter()
            .map(|t| MapPath {
                slot: t.slot,
                points: t
                    .sampled(MAP_STEP_S as usize, duration)
                    .into_iter()
                    .enumerate()
                    .filter(|(_, (_, _, alive))| *alive)
                    .map(|(i, (x, y, _))| {
                        let (left, top) = on_map(x, y);
                        MapPoint { time: i as u32 * MAP_STEP_S, left, top }
                    })
                    .collect(),
            })
            .collect(),
        deaths: {
            let killers: Vec<(u32, u32, u32)> =
                players.iter().flat_map(|p| p.deaths_at.iter().map(move |d| (d.time, p.slot, d.other))).collect();
            let mut out: Vec<MapDeath> = deaths
                .iter()
                .map(|(time, victim, x, y)| {
                    let (left, top) = on_map(*x, *y);
                    let killer = killers.iter().find(|k| k.0 == *time && k.1 == *victim).map(|k| k.2).unwrap_or(0);
                    MapDeath { time: *time, victim: *victim, killer, left, top }
                })
                .collect();
            out.sort_by_key(|d| d.time);
            out
        },
        fights: fights
            .iter()
            .map(|f| {
                let (left, top) = on_map(f.x, f.y);
                MapPoint { time: f.start, left, top }
            })
            .collect(),
    };
    enabled_damage(data, &mut players, &matrix, &tracks);
    build_timing(data, &mut players, &matrix);

    let mut objectives: Vec<Objective> = list(info, "objectives")
        .iter()
        .map(|o| {
            let (name, lane) = objective(o.get("team_objective_id").and_then(Value::as_u64).unwrap_or(u64::MAX));
            let destroyed = int(o, "destroyed_time_s");
            let id = o.get("team_objective_id").and_then(Value::as_u64).unwrap_or(u64::MAX);
            let fall = falls.iter().find(|f| f.team == int(o, "team") && f.id == id);
            Objective {
                team: int(o, "team"),
                name: name.to_string(),
                lane,
                destroyed_at: (destroyed > 0).then_some(destroyed),
                takers: fall.map(|f| f.takers.clone()).unwrap_or_default(),
                player_damage: num(o, "player_damage"),
                trooper_damage: num(o, "creep_damage"),
                left: fall.and_then(|f| f.spot).map(|s| s.0),
                top: fall.and_then(|f| f.spot).map(|s| s.1),
            }
        })
        .collect();
    objectives.sort_by_key(|o| o.destroyed_at.unwrap_or(u32::MAX));
    // Every death: who was around, where, and what began while the player was still down.
    let mut death_reports: Vec<DeathReport> = deaths
        .iter()
        .filter_map(|d| {
            let (time, slot, spot) = (d.0, d.1, (d.2, d.3));
            let me = players.iter().find(|p| p.slot == slot)?;
            let before = time.saturating_sub(1);
            let away = |t: &fights::Track| {
                let (x, y) = t.place(before);
                ((x - spot.0).powi(2) + (y - spot.1).powi(2)).sqrt()
            };
            let living: Vec<&fights::Track> = tracks.iter().filter(|t| t.slot != slot && t.alive_at(before)).collect();
            let near = |own: bool| living.iter().filter(|t| (t.team == me.team) == own && away(t) < DEATH_REACH).count() as u32;
            let nearest = living.iter().filter(|t| t.team == me.team).map(|t| away(t)).fold(f64::INFINITY, f64::min);
            let (enemies, allies) = (near(false), near(true));
            // The first team's base is at the bottom of the map, the second's at the top.
            let depth_m = spot.1 * if me.team == 0 { 1.0 } else { -1.0 } / crate::raw::UNITS_PER_METER;
            let in_fight = fights.iter().any(|f| time + 3 >= f.start && time <= f.end + 6 && f.sides.iter().any(|s| s.deaths.contains(&slot)));
            let (alone, deep) = (allies == 0, depth_m > DEEP_M);
            // Still dead from this very death when something began: no later death of the player lies in between.
            let down_for = |start: u32| start > time && !deaths.iter().any(|o| o.1 == slot && o.0 > time && o.0 < start);
            let mut cost: Vec<String> = vec![];
            if !in_fight {
                for f in fights.iter().filter(|f| f.sides.get(me.team as usize).is_some_and(|s| s.dead.contains(&slot)) && down_for(f.start)) {
                    let (own, other) = (f.sides[me.team as usize].present.len(), f.sides[1 - me.team as usize].present.len());
                    cost.push(format!("the team fight at {} ({own} against {other})", clock(f.start)));
                }
                for b in mid_boss.iter().filter(|b| b.sides.get(me.team as usize).is_some_and(|s| s.dead.contains(&slot)) && down_for(b.time)) {
                    cost.push(format!("the mid boss at {}", clock(b.time)));
                }
            }
            Some(DeathReport {
                time,
                slot,
                killer: me.deaths_at.iter().find(|x| x.time == time).map(|x| x.other).unwrap_or(0),
                enemies,
                allies,
                nearest_ally_m: nearest.is_finite().then_some(nearest / crate::raw::UNITS_PER_METER),
                depth_m,
                alone,
                deep,
                in_fight,
                caught: !in_fight && alone && (enemies >= 2 || deep),
                cost,
                left: (spot.0 + MAP_RADIUS) / (2.0 * MAP_RADIUS),
                top: (MAP_RADIUS - spot.1) / (2.0 * MAP_RADIUS),
            })
        })
        .collect();
    death_reports.sort_by_key(|d| d.time);
    for p in &mut players {
        p.caught_deaths = death_reports.iter().filter(|d| d.slot == p.slot && d.caught).count() as u32;
        p.costly_deaths = death_reports.iter().filter(|d| d.slot == p.slot && !d.cost.is_empty()).count() as u32;
    }
    for p in &mut players {
        // Only the kills the own team showed up for: leaving a mid boss alone is a team's call, not a player's.
        let went: Vec<&fights::Side> = mid_boss.iter().filter_map(|b| b.sides.get(p.team as usize)).filter(|s| !s.present.is_empty()).collect();
        p.mid_boss_alive = went.iter().filter(|s| !s.dead.contains(&p.slot)).count() as u32;
        p.mid_boss_present = went.iter().filter(|s| s.present.contains(&p.slot)).count() as u32;
    }
    assign_roles(data, &mut players);
    let ranking = rank(&players);
    award_titles(&mut players, &ranking, &mid_boss);
    let story = story(&players, &lane_reports, &fights, &mid_boss, &death_reports, &ranking, winning_team, duration);
    Ok(MatchReport {
        story,
        deaths: death_reports,
        lanes: lane_reports,
        objectives,
        mid_boss,
        ranking,
        fights,
        duels,
        map,
        match_id: info.get("match_id").and_then(Value::as_u64).unwrap_or(0),
        start_time: info.get("start_time").and_then(Value::as_u64).unwrap_or(0),
        duration_s: int(info, "duration_s"),
        winning_team,
        players,
    })
}

/// Fills in player names from a Steam profile list (deadlock-api `/v1/players/steam`).
/// Only the display name is read; a profile that is missing just leaves the name empty.
pub fn apply_names(report: &mut MatchReport, profiles_json: &str) {
    let Ok(Value::Array(profiles)) = serde_json::from_str::<Value>(profiles_json) else { return };
    for p in &mut report.players {
        let profile = profiles.iter().find(|x| x.get("account_id").and_then(Value::as_u64) == Some(p.account_id));
        if let Some(name) = profile.and_then(|x| x.get("personaname")).and_then(Value::as_str) {
            p.player_name = name.to_string();
        }
    }
}

/// One player's match boiled down to a row, for looking at several matches together.
#[derive(Debug, Clone, Serialize)]
pub struct MatchSummary {
    pub match_id: u64,
    pub start_time: u64,
    pub duration_s: u32,
    pub hero_name: String,
    pub hero_icon: String,
    pub won: bool,
    pub kills: u32,
    pub deaths: u32,
    pub assists: u32,
    pub net_worth: u32,
    pub score: f64,
    /// 1 is the best of the players in the match.
    pub place: u32,
    pub parts: Vec<RankPart>,
    pub hero_damage: f64,
    pub objective_damage: f64,
    pub lane_souls_diff: f64,
    pub last_hit_pct: f64,
    pub fights_joined: u32,
    pub fights_possible: u32,
    pub late_fights_skipped: u32,
    pub avg_unspent_souls: f64,
    pub dead_seconds: u32,
    pub enabled_total: f64,
    pub role: String,
    /// Late-game team fights with the match even that the player stayed out of.
    pub even_fights_skipped: u32,
    pub caught_deaths: u32,
    pub costly_deaths: u32,
    /// Mid boss kills the player was alive for, and how many of those the player was at the pit for.
    pub mid_boss_alive: u32,
    pub mid_boss_present: u32,
    /// How well the build answered the enemy team, percent.
    pub counter_score: Option<f64>,
    /// What the match called for that came late or never, by name (e.g. `Lane: weapon defence`).
    pub late_needs: Vec<String>,
    /// The names of the titles earned in the match, good and bad.
    pub titles: Vec<String>,
    pub bad_titles: Vec<String>,
    /// Against the lane opponents in the laning phase.
    pub lane_damage_dealt: f64,
    pub lane_damage_taken: f64,
    pub lane_kills_on: u32,
    pub lane_deaths_to: u32,
    pub roams: u32,
    /// Roams in which an enemy died in the lane visited.
    pub roams_paid: u32,
    pub wins_farmed: u32,
}

/// The row for the player with this account in a match, if they played in it.
pub fn summarize(report: &MatchReport, account_id: u64) -> Option<MatchSummary> {
    let p = report.players.iter().find(|p| p.account_id == account_id)?;
    let place = report.ranking.iter().position(|r| r.slot == p.slot)?;
    let rank = &report.ranking[place];
    Some(MatchSummary {
        match_id: report.match_id,
        start_time: report.start_time,
        duration_s: report.duration_s,
        hero_name: p.hero_name.clone(),
        hero_icon: p.hero_icon.clone(),
        won: p.won,
        kills: p.kills,
        deaths: p.deaths,
        assists: p.assists,
        net_worth: p.net_worth,
        score: rank.score,
        place: place as u32 + 1,
        parts: rank.parts.clone(),
        hero_damage: p.hero_damage,
        objective_damage: p.objective_damage,
        lane_souls_diff: p.lane_souls_diff,
        last_hit_pct: p.last_hit_pct,
        fights_joined: p.fights_joined,
        fights_possible: p.fights_possible,
        late_fights_skipped: p.late_fights_skipped,
        avg_unspent_souls: p.avg_unspent_souls,
        dead_seconds: p.dead_seconds,
        enabled_total: p.enabled_total,
        role: p.role.clone(),
        even_fights_skipped: report
            .fights
            .iter()
            .filter(|f| f.even && f.sides.get(p.team as usize).is_some_and(|s| s.absent.iter().any(|a| a.slot == p.slot && a.missed)))
            .count() as u32,
        caught_deaths: p.caught_deaths,
        costly_deaths: p.costly_deaths,
        mid_boss_alive: p.mid_boss_alive,
        mid_boss_present: p.mid_boss_present,
        counter_score: p.counter_score,
        // A need a teammate covered is not this player's; the first big items are not answers to anyone.
        late_needs: p.build_timing.iter().filter(|t| t.credit.is_some_and(|c| c < CREDIT_SLOW)).map(|t| t.need.clone()).collect(),
        titles: p.titles.iter().filter(|t| t.tone == "good").map(|t| t.name.clone()).collect(),
        bad_titles: p.titles.iter().filter(|t| t.tone == "bad").map(|t| t.name.clone()).collect(),
        lane_damage_dealt: p.lane_damage_dealt,
        lane_damage_taken: p.lane_damage_taken,
        lane_kills_on: p.lane_kills_on,
        lane_deaths_to: p.lane_deaths_to,
        roams: p.roams.len() as u32,
        roams_paid: p.roams.iter().filter(|r| r.enemy_deaths > 0).count() as u32,
        wins_farmed: p.wins_farmed,
    })
}

/// A Deadlock replay or match-metadata download found in Steam's HTTP cache.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct CacheEntry {
    pub match_id: u64,
    pub salt: u64,
    /// True for the replay file, false for the match metadata.
    pub demo: bool,
}

/// Deadlock's Steam app id, the folder its replays are served from.
const REPLAY_FOLDER: &[u8] = b"/1422450/";

/// Reads the start of a Steam `httpcache` file. Each file begins with a short binary header followed by
/// the host and the path of the request as zero-terminated text; Deadlock fetches
/// `replayNNN.valve.net/1422450/<match id>_<salt>.meta.bz2` (and `.dem.bz2`) when a match is opened.
pub fn cache_entry(head: &[u8]) -> Option<CacheEntry> {
    let at = head.windows(REPLAY_FOLDER.len()).position(|w| w == REPLAY_FOLDER)?;
    if !head[..at].windows(9).any(|w| w == b"valve.net") {
        return None;
    }
    let rest = &head[at + REPLAY_FOLDER.len()..];
    let digits = |bytes: &[u8]| bytes.iter().take_while(|b| b.is_ascii_digit()).count();
    let number = |bytes: &[u8]| std::str::from_utf8(bytes).ok()?.parse::<u64>().ok();
    let id_len = digits(rest);
    let after_id = &rest[id_len..];
    if id_len == 0 || after_id.first() != Some(&b'_') {
        return None;
    }
    let salt_len = digits(&after_id[1..]);
    let tail = &after_id[1 + salt_len..];
    let demo = tail.starts_with(b".dem.bz2");
    if salt_len == 0 || !(demo || tail.starts_with(b".meta.bz2")) {
        return None;
    }
    Some(CacheEntry { match_id: number(&rest[..id_len])?, salt: number(&after_id[1..1 + salt_len])?, demo })
}

/// One row of a player's match history (deadlock-api `/v1/players/{id}/match-history`).
#[derive(Debug, Clone, Serialize)]
pub struct HistoryEntry {
    pub match_id: u64,
    pub start_time: u64,
    pub duration_s: u32,
    pub hero_name: String,
    pub hero_icon: String,
    pub kills: u32,
    pub deaths: u32,
    pub assists: u32,
    pub net_worth: u32,
    pub won: bool,
}

pub fn parse_history(data: &GameData, json: &str, limit: usize) -> Result<Vec<HistoryEntry>, String> {
    let root: Value = serde_json::from_str(json).map_err(|e| format!("Unreadable match history: {e}"))?;
    let rows = root.as_array().ok_or("The match history is not a list")?;
    let mut out: Vec<HistoryEntry> = rows
        .iter()
        .map(|r| {
            let hero = data.heroes.iter().find(|h| Some(h.id) == r.get("hero_id").and_then(Value::as_u64));
            HistoryEntry {
                match_id: r.get("match_id").and_then(Value::as_u64).unwrap_or(0),
                start_time: r.get("start_time").and_then(Value::as_u64).unwrap_or(0),
                duration_s: int(r, "match_duration_s"),
                hero_name: hero.map(|h| h.name.clone()).unwrap_or_else(|| "Unknown hero".into()),
                hero_icon: hero.map(|h| h.icon.clone()).unwrap_or_default(),
                kills: int(r, "player_kills"),
                deaths: int(r, "player_deaths"),
                assists: int(r, "player_assists"),
                net_worth: int(r, "net_worth"),
                won: match r.get("player_match_outcome").and_then(Value::as_u64) {
                    Some(outcome) if outcome > 0 => outcome == 1,
                    _ => r.get("match_result").and_then(Value::as_u64) == r.get("player_team").and_then(Value::as_u64),
                },
            }
        })
        .filter(|e| e.match_id > 0)
        .collect();
    out.sort_by(|a, b| b.start_time.cmp(&a.start_time));
    out.truncate(limit);
    Ok(out)
}
