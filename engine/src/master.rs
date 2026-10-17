//! The master builder: a full build for the player's hero against an enemy team and alongside the player's own
//! team, phase by phase by souls: the lane (to 6,000), the first rift (to 15,000), the second rift (to 25,000) and
//! the late game with every boon (to 40,000).
//!
//! Everyone else is assumed to build what top players usually build. Every item is weighed on three things at
//! each phase:
//! - how many top players of the hero buy it at that point of the match (by their souls when they bought it);
//! - what it adds to the hero's own numbers for the chosen role, worked out by the engine;
//! - what it answers on the enemy team at that point (the lane opponents in the lane), less what teammates'
//!   builds already do for the team.
//!
//! The role decides how much each of the three counts: a carry builds for its own numbers, a tempo player for
//! power early in the mid game, a support for the team, a frontliner for staying alive.

use crate::build::BuildInput;
use crate::calc;
use crate::data::{GameData, Item};
use crate::matches::TEAM_ITEMS;
use crate::popular::{self, CounterPlan, ItemStat, Matchup};
use crate::suggest;
use crate::team::TeamEffects;
use serde::Serialize;

/// The phases: key, title, souls at the start, souls at the end, the counter stage they use, and the ability
/// upgrades a hero usually has by then.
const PHASES: [(&str, &str, u32, u32, &str, [u8; 4]); 4] = [
    ("lane", "Lane", 0, 6_000, "lane", [0, 0, 0, 0]),
    ("rift1", "First rift", 6_000, 15_000, "mid", [1, 1, 1, 0]),
    ("rift2", "Second rift", 15_000, 25_000, "mid", [2, 2, 1, 1]),
    ("late", "All boons", 25_000, 40_000, "late", [3, 3, 3, 3]),
];

/// The roles a player can take: key, name, what it means, and the weight of (popularity, own numbers, the enemy
/// team).
const FOCUS: [(&str, &str, &str, [f64; 3]); 4] = [
    ("carry", "Carry", "Your own damage first: bigger items that scale into the late game", [0.50, 0.30, 0.20]),
    ("tempo", "Tempo", "Power early in the mid game to fight and take objectives before their carries come online", [0.50, 0.25, 0.25]),
    ("support", "Support", "Items that answer their team and help yours", [0.40, 0.15, 0.45]),
    ("frontline", "Frontline", "Staying alive at the front while your carries deal the damage", [0.50, 0.30, 0.20]),
];

/// A stun of this long counts as a full answer to the enemy team: a stunned enemy deals no damage meanwhile, and
/// the player gets free hits in.
const STUN_FULL: f64 = 1.5;
/// How much of the answer part a stun can be at most.
const STUN_WEIGHT: f64 = 0.6;

/// The fights the engine measures last about this long, seconds: a stun is worth its share of one.
const STUN_FIGHT_S: f64 = 12.0;
/// A heavy melee takes about this long, seconds: the time to build up a melee stun (Crushing Fists).
const HEAVY_MELEE_S: f64 = 1.0;
/// A stun is counted as at most this share of a fight.
const STUN_SHARE_CAP: f64 = 0.5;

/// What an item's stuns are worth to the player's own fight, as a share like the engine's gains: while the enemy
/// is stunned it deals nothing back, so stunning it a share X of the fight is as good as X / (1 - X) more of
/// everything the player does. Counted for how many times it lands in a fight and how easily: a stun that comes
/// with melee hits lands fully for a melee build and half as often otherwise.
pub fn stun_gain(item: &Item, stun: f64, style: &str, again: bool) -> (f64, f64) {
    let get = |key: &str| item.props.iter().find(|p| p.key == key).map(|p| p.value.abs()).unwrap_or(0.0);
    let melee = item.description.to_lowercase().contains("melee");
    // Echo Shard's second cast lands once a fight, and so does a stun with no cooldown of its own (Lightning Scroll
    // comes with the ultimate); an item's own stun lands after its delay or build-up, then every cooldown.
    let cooldown = get("AbilityCooldown");
    let casts = if again || cooldown <= 0.0 {
        1.0
    } else {
        let setup = get("StunDelay") + get("DelayBeforeStun") + if melee { get("MaxStacks") / 2.0 * HEAVY_MELEE_S } else { 0.0 };
        1.0 + ((STUN_FIGHT_S - setup).max(0.0) / cooldown.max(setup).max(1.0)).floor()
    };
    let share = (casts * stun / STUN_FIGHT_S).min(STUN_SHARE_CAP);
    let lands = if melee && !again && style != "melee" { 0.5 } else { 1.0 };
    (lands * share / (1.0 - share), casts)
}

/// A counter score this high (an item that takes about a third off two enemies' damage, or answers a job the
/// whole team calls for) is the full answer part.
const COUNTER_FULL: f64 = 2.0;
/// The answer part of an item whose jobs the build already does, against one doing a new job.
const DONE_JOB: f64 = 0.35;
/// An item that adds this much to the hero's numbers (0.25 = a quarter more) gets the full engine part.
const ENGINE_FULL: f64 = 0.25;
/// How strongly price is held against an item in each phase: in the lane souls are short, late the slots are.
const PRICE_WEIGHT: [f64; 4] = [0.5, 0.4, 0.25, 0.05];
/// A tempo player wants power sooner, so price counts this much more in the first and second rift.
const TEMPO_PRICE: f64 = 0.15;
/// The weights of (popularity, own numbers, the enemy team) for a damage type the hero's top players do not play.
const OFF_STYLE_WEIGHTS: [f64; 3] = [0.2, 0.6, 0.2];
/// For such a damage type, an item that adds this much to it is in the running however few players buy it, and a
/// damage item gets full credit for its popularity only from this much.
const OFF_STYLE_GAIN: f64 = 0.04;

/// The popular builds are the backbone: an item comes in when top players of the hero buy it at that point at
/// least this often (against the most bought item), or when it is a strong answer to the enemy team.
const POPULAR_FROM: f64 = 0.05;
const STRONG_COUNTER: f64 = 0.7;
/// The cheapest item worth a slot in each phase. Later on souls come fast and slots are what is short, so a
/// cheap item there only takes the place of a better one; upgrades bought on top of an owned item still count.
const MIN_COST: [u32; 4] = [0, 0, 1_600, 3_200];
/// A passive that only works close by counts this much for a hero who fights from range.
const CLOSE_RANGE_FACTOR: f64 = 0.5;
/// A counter pick is not bought for a job that this many items in the build already do.
const COUNTER_JOB_MAX: usize = 2;
/// The prefixes of the notes shown on an item in the build.
const SHOWN_NOTES: [&str; 6] = ["Takes", "Harder", "Timing", "Top players", "Stuns", "Clears"];

/// An item's components, theirs too.
fn parts(data: &GameData, class: &str) -> Vec<String> {
    let mut out = vec![];
    let mut todo: Vec<String> = data.item(class).map(|i| i.components.clone()).unwrap_or_default();
    while let Some(c) = todo.pop() {
        if !out.contains(&c) {
            todo.extend(data.item(&c).map(|i| i.components.clone()).unwrap_or_default());
            out.push(c);
        }
    }
    out
}

/// What a team item (healing, barriers, team auras) adds on top for a support.
const TEAM_ITEM_BONUS: f64 = 0.25;
/// Below this score an item is not worth a slot.
const MIN_SCORE: f64 = 0.12;

/// Pushing the lane: an item that clears the trooper wave faster puts pressure on the lane opponent (they lose
/// souls or have to give up the tower) and frees time to fight or take the other lanes. Its weight in each phase,
/// most in the lane and less in the first rift, nothing after.
const PUSH_WEIGHT: [f64; 4] = [0.35, 0.15, 0.0, 0.0];
/// A wave cleared this much faster is the full push credit; from it the item is in the running in those phases.
const PUSH_FULL: f64 = 0.3;

/// How fast the build clears a trooper wave: its sustained gun damage against troopers, with the weapon damage
/// that only works on them (Monster Rounds, Cultist Sacrifice), and longer for the bullet resist against them that
/// lets it stand in the wave (by the square root of the damage taken, as staying power is only half of clearing).
fn wave_clear(data: &GameData, b: &BuildInput) -> Option<f64> {
    let sheet = calc::compute(data, b).ok()?;
    let npc = crate::farm::npc_damage(data, b, &sheet);
    // Abilities that hit an area clear the wave too: their damage per cast over their cooldown on the troopers
    // they catch, for the hero's three basic abilities. Seven's Static Charge or Infernus' Napalm count as much as
    // a gun does.
    let abilities: f64 = npc
        .abilities
        .iter()
        .filter(|(_, area, _)| *area)
        .map(|(damage, _, cooldown)| damage * WAVE_AREA_HITS / cooldown.max(WAVE_MIN_COOLDOWN))
        .sum();
    Some(npc.gun_dps / (1.0 - npc.npc_bullet_resist.min(90.0) / 100.0).sqrt() + abilities)
}

/// A wave lasts this long at the least: an ability on a shorter cooldown does not hit it more often.
const WAVE_MIN_COOLDOWN: f64 = 5.0;
/// Troopers an area ability catches in a wave, where the gun hits one at a time (an assumption).
const WAVE_AREA_HITS: f64 = 3.0;

/// How much faster the wave is cleared with the item, in the phases where pushing counts.
fn push_gain(data: &GameData, b: &BuildInput, class: &str, phase: usize) -> f64 {
    if PUSH_WEIGHT[phase] <= 0.0 {
        return 0.0;
    }
    let mut trial = b.clone();
    trial.purchases.push(class.to_string());
    match (wave_clear(data, b), wave_clear(data, &trial)) {
        (Some(before), Some(after)) if before > 0.0 => (after / before - 1.0).max(0.0),
        _ => 0.0,
    }
}

fn push_note(push: f64) -> Option<String> {
    (push >= 0.1).then(|| format!("Clears the wave {:.0}% faster: pressure on the lane", push * 100.0))
}
/// A win rate this many points higher in long matches than in short ones makes a hero a late-game carry.
pub const LATE_SCALER_PP: f64 = 4.0;
/// When the slots are full, a cheap item is sold for a new one that scores at least this share of what the cheap
/// item scored when it was bought, phase by phase: a lane item is worth less and less as the match goes on.
const SELL_FOR: [f64; 4] = [1.0, 1.0, 0.8, 0.5];
/// At most this many items are sold in one phase.
const MAX_SALES: usize = 4;
/// Cheap items, the ones sold to make room.
const SELLABLE_COST: u32 = 1_600;
/// Selling gives back half of the price.
const SELL_BACK: f64 = 0.5;

/// How each hero does in long matches against short ones: (hero id, win rate in long minus short, percent points).
pub type Scaling = Vec<(u64, f64)>;

/// What the master builder is asked for.
pub struct MasterInput<'a> {
    pub matchup: Matchup<'a>,
    /// The player's hero id.
    pub hero_id: u64,
    /// Top players of the hero: what they bought by their net worth when they bought it, and how many matches.
    pub phase_stats: &'a [ItemStat],
    pub hero_matches: u64,
    /// The hero's own popular build, for its usual share of gun and spirit.
    pub own_build: &'a popular::PopularBuild,
    /// "auto", "carry", "tempo", "support" or "frontline".
    pub focus: &'a str,
    /// "auto", "gun", "spirit", "hybrid" or "melee".
    pub damage: &'a str,
    pub scaling: &'a Scaling,
    /// What the player bought in a match, with their net worth at the time: judged on the same terms.
    pub bought: &'a [(String, u32)],
}

/// An item the player bought in a match, judged on the same terms as the master build's items.
#[derive(Debug, Clone, Serialize)]
pub struct JudgedItem {
    pub item: String,
    pub name: String,
    pub icon: String,
    pub category: String,
    pub cost: u32,
    pub active: bool,
    pub description: String,
    /// The phase it was bought in, by the player's net worth then.
    pub phase: usize,
    pub net_worth: u32,
    /// Share of the hero's top players who buy it in this phase, percent.
    pub popular_pct: f64,
    pub win_pct: f64,
    pub win_pp: f64,
    /// The jobs it does against the enemy team at this phase, and notes on it.
    pub jobs: Vec<String>,
    pub notes: Vec<String>,
    /// The master build has it too, in any phase.
    pub in_master: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct MasterItem {
    pub item: String,
    pub name: String,
    pub icon: String,
    pub category: String,
    pub cost: u32,
    pub active: bool,
    pub description: String,
    /// The phase it is bought in.
    pub phase: usize,
    /// What it really cost: less when it upgrades an item already owned.
    pub paid: u32,
    pub score: f64,
    /// Share of the hero's top players who buy it in this phase, percent.
    pub popular_pct: f64,
    /// Matches won when it is bought in this phase, percent, and how many points that is above the hero's win rate
    /// (shrunk for small samples).
    pub win_pct: f64,
    pub win_pp: f64,
    /// How much it adds to the hero's own numbers for the role, percent.
    pub engine_pct: f64,
    /// The jobs it does against the enemy team at this phase, and notes on them.
    pub jobs: Vec<String>,
    pub notes: Vec<String>,
    pub sold: bool,
    /// The owned item it is bought on top of, if it is an upgrade.
    pub upgrades: Option<String>,
    /// Bought into a bigger item later.
    pub upgraded: bool,
    /// The ability it is imbued into, for an item that goes into one.
    pub imbued: Option<String>,
    /// Bought to answer the enemy team: one of the counter plan's picks.
    pub counter: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct MasterPhase {
    pub key: String,
    pub title: String,
    pub souls_from: u32,
    pub souls_to: u32,
    /// Items sold in this phase to make room.
    pub sold: Vec<String>,
    /// The best items not bought in this phase, with their scores: why they lost.
    pub considered: Vec<Considered>,
}

/// An item weighed in a phase and not bought, with the three things it was weighed on.
#[derive(Debug, Clone, Serialize)]
pub struct Considered {
    pub item: String,
    pub name: String,
    pub icon: String,
    pub category: String,
    pub cost: u32,
    pub score: f64,
    pub popular_pct: f64,
    pub win_pct: f64,
    pub win_pp: f64,
    pub engine_pct: f64,
    /// Its answer to the enemy team against the best answer at this point, percent.
    pub counter_pct: f64,
    pub jobs: Vec<String>,
    pub notes: Vec<String>,
    /// Few top players buy it at this point and it is no strong answer, so it was not in the running.
    pub left_out: bool,
}

/// This many items not bought are shown for each phase.
const CONSIDERED: usize = 8;

#[derive(Debug, Clone, Serialize)]
pub struct MasterBuild {
    pub hero: String,
    pub hero_name: String,
    /// The role it was built for, and the engine playstyle.
    pub focus: String,
    pub focus_name: String,
    pub focus_goal: String,
    pub style: String,
    pub phases: Vec<MasterPhase>,
    pub items: Vec<MasterItem>,
    /// What is owned at the end, in buying order, for the builder, and the abilities imbued items go into.
    pub purchases: Vec<String>,
    pub imbues: std::collections::BTreeMap<String, usize>,
    /// How the teams shaped the build.
    pub notes: Vec<String>,
    /// What the player bought in the match, judged.
    pub judged: Vec<JudgedItem>,
    /// The counter plan the build was weighed with.
    #[serde(skip)]
    pub counter: CounterPlan,
}

/// The roles a player can pick: (key, name, goal).
pub fn focuses() -> Vec<(String, String, String)> {
    FOCUS.iter().map(|f| (f.0.into(), f.1.into(), f.2.into())).collect()
}

/// Share of the hero's matches in which top players bought the item between the two net worths.
fn share(stats: &[ItemStat], item_id: u64, from: u32, to: u32, matches: u64) -> f64 {
    stats
        .iter()
        .filter(|r| r.item_id == item_id && r.bucket.is_some_and(|b| b >= from as u64 && b < to as u64))
        .map(|r| r.matches as f64)
        // An empty sum is -0.0.
        .fold(0.0, |a, b| a + b)
        / matches.max(1) as f64
}

/// Win rate when the item is bought between the two net worths, percent, and the matches it is from.
fn phase_win(stats: &[ItemStat], item_id: u64, from: u32, to: u32) -> (f64, f64) {
    let rows: Vec<&ItemStat> = stats.iter().filter(|r| r.item_id == item_id && r.bucket.is_some_and(|b| b >= from as u64 && b < to as u64)).collect();
    popular::win_rate(&rows)
}

/// Win rate of every purchase the hero's top players make between the two net worths, percent. Players who get
/// far are often already winning, so an item bought late is held against what is bought at the same point, not
/// against the hero's win rate overall.
fn phase_base(stats: &[ItemStat], from: u32, to: u32) -> f64 {
    let rows: Vec<&ItemStat> = stats.iter().filter(|r| r.bucket.is_some_and(|b| b >= from as u64 && b < to as u64)).collect();
    popular::win_rate(&rows).0
}

/// What a point of win rate above the phase's does to an item's score: 5 points more is about 15% more.
const WIN_WEIGHT: f64 = 0.03;
/// The win rate moves an item's score by at most this share either way.
const WIN_CAP: f64 = 0.3;

fn phase_of(net_worth: u32) -> usize {
    PHASES.iter().position(|p| net_worth < p.3).unwrap_or(PHASES.len() - 1)
}

fn hero_name(data: &GameData, id: u64) -> String {
    data.heroes.iter().find(|h| h.id == id).map(|h| h.name.clone()).unwrap_or_default()
}

/// Puts the build together.
pub fn master_build(data: &GameData, input: &MasterInput) -> Result<MasterBuild, String> {
    let m = &input.matchup;
    let hero = data.heroes.iter().find(|h| h.id == input.hero_id).ok_or("Unknown hero")?;
    let mut notes = vec![];

    // The role: what the player asked for, or worked out from the teams.
    let my_scale = input.scaling.iter().find(|s| s.0 == input.hero_id).map(|s| s.1).unwrap_or(0.0);
    let late_allies: Vec<(String, f64)> = m
        .allies
        .iter()
        .filter_map(|a| input.scaling.iter().find(|s| s.0 == a.hero_id))
        .filter(|s| s.1 >= LATE_SCALER_PP)
        .map(|s| (hero_name(data, s.0), s.1))
        .collect();
    for (name, pp) in &late_allies {
        notes.push(format!("{name} wins {pp:.0} points more of the long matches than of the short ones: your team's late game"));
    }
    let late_enemies: Vec<String> = m
        .enemies
        .iter()
        .filter_map(|e| input.scaling.iter().find(|s| s.0 == e.hero_id))
        .filter(|s| s.1 >= LATE_SCALER_PP)
        .map(|s| hero_name(data, s.0))
        .collect();
    if !late_enemies.is_empty() {
        notes.push(format!("{} grow strong late: the mid game is the time to end it or get far ahead", late_enemies.join(" and ")));
    }
    let focus_key = match input.focus {
        "auto" | "" => {
            if late_allies.iter().any(|a| a.1 > my_scale) {
                notes.push(format!("Tempo picked for you: {} carries late, so your strength is the mid game", late_allies[0].0));
                "tempo"
            } else {
                "carry"
            }
        }
        key => key,
    };
    let focus = FOCUS.iter().find(|f| f.0 == focus_key).unwrap_or(&FOCUS[0]);
    let [w_pop, w_engine, w_counter] = focus.3;

    // The damage: what was asked, or what the hero's top players build.
    let gun_share = popular::gun_pct(data, input.own_build).map(|g| g / 100.0).unwrap_or_else(|| {
        let core: Vec<&Item> = input.own_build.core.iter().filter_map(|c| data.item(&c.item)).collect();
        let souls = |category: &str| core.iter().filter(|i| i.category == category).map(|i| i.cost as f64).sum::<f64>();
        souls("weapon") / (souls("weapon") + souls("spirit")).max(1.0)
    });
    let usual = if gun_share >= 0.6 {
        "gun"
    } else if gun_share <= 0.4 {
        "spirit"
    } else {
        "hybrid"
    };
    let style = if focus.0 == "frontline" {
        "frontline".to_string()
    } else {
        match input.damage {
            "gun" | "spirit" | "hybrid" | "melee" => input.damage.to_string(),
            _ => usual.into(),
        }
    };
    // A spirit hero still shoots between casts: its items are measured over the abilities and that shooting, or
    // no gun item would ever add anything.
    let gains_style = if style == "spirit" { "spirit_fight" } else { style.as_str() };
    // A damage type the hero's top players do not build for (melee Ivy): what they buy says little about it, so
    // the engine leads and items that do nothing for it get no credit for being popular.
    let off_style = matches!(input.damage, "gun" | "spirit" | "hybrid" | "melee") && input.damage != usual && focus.0 != "frontline";
    if off_style {
        notes.push(format!(
            "Built for {} damage, which {}'s top players do not usually play: the build follows what the items add to your own numbers more than what is popular",
            input.damage,
            hero.name
        ));
    }
    let [w_pop, w_engine, w_counter] = if off_style { OFF_STYLE_WEIGHTS } else { [w_pop, w_engine, w_counter] };

    let counter = popular::counter_plan(data, m);
    // What the team already does: the player's own kit, and the teammates' kits and usual builds.
    let mut members: Vec<(u64, Vec<String>)> = vec![(input.hero_id, vec![])];
    members.extend(m.allies.iter().map(|a| (a.hero_id, a.build.core.iter().map(|c| c.item.clone()).collect())));
    let team = TeamEffects::of(data, &members);
    let stage_scores = |stage: &str| counter.stages.iter().find(|s| s.key == stage).map(|s| s.scored.clone()).unwrap_or_default();
    // The counter plan's picks are bought into the build and marked as counters there.
    let counter_items: Vec<String> = counter.stages.iter().flat_map(|s| s.picks.iter().map(|p| p.advice.item.clone())).collect();

    let shop: Vec<&Item> = data.items.iter().filter(|i| i.cost > 0 && i.tier <= 4).collect();
    let mut b = BuildInput { hero: hero.class_name.clone(), ..Default::default() };
    let mut items: Vec<MasterItem> = vec![];
    let mut phases: Vec<MasterPhase> = vec![];
    let mut paid_total: u32 = 0;
    for (index, (key, title, from, to, stage, tiers)) in PHASES.iter().enumerate() {
        b.souls = Some(*to);
        b.ability_tiers = tiers.to_vec();
        let counter_scores = stage_scores(stage);
        // The answer to the enemy team on a fixed scale: one outstanding answer must not make every other look small.
        let counter_top = COUNTER_FULL;
        // Share of the hero's matches in which top players bought the item during this phase.
        let popular = |item: &Item| -> f64 { share(input.phase_stats, item.id, *from, *to, input.hero_matches) };
        let pop_top = shop.iter().map(|i| popular(i)).fold(0.0, f64::max).max(1e-9);
        let price_weight = PRICE_WEIGHT[index] + if focus.0 == "tempo" && (1..=2).contains(&index) { TEMPO_PRICE } else { 0.0 };
        // The item's score: its popularity in the phase (against the most popular item), what it adds to the
        // hero's own numbers, and what it answers on the enemy team (against the best answer).
        // How much more often the hero wins when it buys the item at this point, against its usual win rate.
        let base = phase_base(input.phase_stats, *from, *to);
        let win = |item: &Item| -> (f64, f64) {
            let (wr, n) = phase_win(input.phase_stats, item.id, *from, *to);
            (wr, if n > 0.0 { popular::shrunk(wr - base, n) } else { 0.0 })
        };
        let score = |item: &Item, pop: f64, gain: f64, ctr: f64, push: f64| -> f64 {
            // Off-style, a damage item that does nothing for the chosen damage is not helped by being popular.
            let pop_credit = if off_style && item.category != "vitality" { (gain.max(0.0) / OFF_STYLE_GAIN).min(1.0) } else { 1.0 };
            let mut s = w_pop * pop / pop_top * pop_credit + w_engine * (gain.max(0.0) / ENGINE_FULL).min(1.0) + w_counter * ctr;
            s += PUSH_WEIGHT[index] * (push / PUSH_FULL).min(1.0);
            s *= 1.0 + (win(item).1 * WIN_WEIGHT).clamp(-WIN_CAP, WIN_CAP);
            if focus.0 == "support" && TEAM_ITEMS.contains(&item.name.as_str()) {
                s += TEAM_ITEM_BONUS;
            }
            if m.ranged && (popular::close_range(item).is_some() || popular::dives_in(item)) {
                s *= CLOSE_RANGE_FACTOR;
            }
            s * team.redundancy(item).0
        };
        let eligible = |pop: f64, ctr: f64, gain: f64, push: f64| {
            pop / pop_top >= POPULAR_FROM || ctr >= STRONG_COUNTER || (off_style && gain >= OFF_STYLE_GAIN) || (PUSH_WEIGHT[index] > 0.0 && push >= PUSH_FULL)
        };
        let mut sold_here: Vec<String> = vec![];
        let mut considered: Vec<Considered> = vec![];
        let mut first = true;
        // The counter picks of the stage come first, in the first phase of the stage that pays for them.
        let stage_picks: Vec<String> = counter.stages.iter().filter(|s| s.key == *stage).flat_map(|s| s.picks.iter().map(|p| p.advice.item.clone())).collect();
        for class in &stage_picks {
            if b.purchases.contains(class) || items.iter().any(|x| &x.item == class) {
                continue;
            }
            let Some(scored) = counter_scores.iter().find(|c| &c.0 == class) else { continue };
            let done: Vec<String> = items.iter().filter(|x| !x.sold).flat_map(|x| x.jobs.clone()).collect();
            if !scored.2.is_empty() && scored.2.iter().all(|j| done.iter().filter(|d| *d == j).count() >= COUNTER_JOB_MAX) {
                continue;
            }
            let Some(g) = suggest::gains(data, &b, gains_style, std::slice::from_ref(class)).into_iter().next() else { continue };
            if g.paid > to.saturating_sub(paid_total) {
                continue;
            }
            let item = data.item(class).ok_or("Unknown item")?;
            let pop = popular(item);
            let ctr = (scored.1 / counter_top).min(1.0);
            let upgrades = item.components.iter().find(|c| b.purchases.contains(c)).cloned();
            if let Some(x) = upgrades.as_ref().and_then(|c| items.iter_mut().find(|x| &x.item == c)) {
                x.upgraded = true;
            }
            b.purchases.push(class.clone());
            if let Some(i) = g.imbue {
                b.imbues.insert(class.clone(), i);
            }
            paid_total += g.paid;
            items.push(MasterItem {
                item: class.clone(),
                name: item.name.clone(),
                icon: item.icon.clone(),
                category: item.category.clone(),
                cost: item.cost,
                active: item.active,
                description: item.description.clone(),
                phase: index,
                paid: g.paid,
                score: score(item, pop, g.gain, ctr, 0.0),
                popular_pct: pop * 100.0,
                win_pct: win(item).0,
                win_pp: win(item).1,
                engine_pct: g.gain.max(0.0) * 100.0,
                jobs: scored.2.clone(),
                notes: team.redundancy(item).1.into_iter().chain(scored.3.iter().filter(|w| SHOWN_NOTES.iter().any(|p| w.starts_with(p))).cloned()).collect(),
                sold: false,
                upgrades: upgrades.and_then(|c| data.item(&c)).map(|i| i.name.clone()),
                upgraded: false,
                imbued: g.imbue.and_then(|i| hero.abilities.get(i)).map(|a| a.name.clone()),
                counter: true,
            });
        }
        loop {
            // A job items already bought do is worth less a second time: never two items for the same job.
            let done_jobs: Vec<String> = items.iter().filter(|x| !x.sold).flat_map(|x| x.jobs.clone()).collect();
            let answer = |c: &(String, f64, Vec<String>, Vec<String>)| -> f64 {
                // A job two items in the build already do earns a third nothing (Unstoppable on top of Debuff
                // Reducer and Reactive Barrier).
                if !c.2.is_empty() && c.2.iter().all(|j| done_jobs.iter().filter(|d| *d == j).count() >= COUNTER_JOB_MAX) {
                    return 0.0;
                }
                let fresh = if c.2.is_empty() { 1.0 } else { c.2.iter().filter(|j| !done_jobs.contains(j)).count() as f64 / c.2.len() as f64 };
                (c.1 / counter_top).min(1.0) * (DONE_JOB + (1.0 - DONE_JOB) * fresh)
            };
            let left = to.saturating_sub(paid_total);
            let owned = calc::compute(data, &b).map(|s| s.owned).unwrap_or_default();
            // A component of an item already bought is never bought after it (Extra Stamina after Arcane Surge).
            let inside: Vec<String> = b.purchases.iter().flat_map(|c| parts(data, c)).collect();
            let candidates: Vec<String> = shop
                .iter()
                .filter(|i| i.cost <= left.max(1) * 2 && !b.purchases.contains(&i.class_name) && !items.iter().any(|x| x.item == i.class_name))
                .filter(|i| !inside.contains(&i.class_name))
                .map(|i| i.class_name.clone())
                .collect();
            let gains = suggest::gains(data, &b, gains_style, &candidates);
            let push_of = |class: &str| push_gain(data, &b, class, index);
            // The item's answer to the enemy team, with the stuns it lands: its own (Crushing Fists, Knockdown) or a
            // second cast of a stunning ability it resets (Echo Shard on Stone Form).
            let answer_of = |class: &str, imbue: Option<usize>| -> (f64, Vec<String>, Vec<String>, f64) {
                let (ctr, jobs, mut why) = counter_scores
                    .iter()
                    .find(|c| c.0 == class)
                    .map(|c| (answer(c), c.2.clone(), c.3.clone()))
                    .unwrap_or((0.0, vec![], vec![]));
                let Some(item) = data.item(class) else { return (ctr, jobs, why, 0.0) };
                let mut stun = crate::team::item_lockdown_seconds(item);
                let mut by = String::new();
                if item.name == "Echo Shard" {
                    if let Some(a) = imbue.and_then(|i| hero.abilities.get(i)) {
                        let again = crate::team::ability_lockdown_seconds(a);
                        if again > 0.0 {
                            stun = stun.max(again);
                            by = format!(" with a second {}", a.name);
                        }
                    }
                }
                if stun <= 0.0 {
                    return (ctr, jobs, why, 0.0);
                }
                let (worth, casts) = stun_gain(item, stun, &style, !by.is_empty());
                let times = if casts > 1.0 { format!(", {casts:.0} times a fight") } else { String::new() };
                why.insert(0, format!("Stuns{by} for {stun:.2}s{times}: the enemy cannot fight back meanwhile, worth +{:.0}% to your fight", worth * 100.0));
                ((ctr + STUN_WEIGHT * (stun / STUN_FULL).min(1.0)).min(1.0), jobs, why, worth)
            };
            let score_of = |g: &suggest::Gain| -> Option<(f64, f64, Vec<String>, Vec<String>, f64)> {
                let item = data.item(&g.item)?;
                let pop = popular(item);
                let (ctr, jobs, mut why, stun) = answer_of(&g.item, g.imbue);
                let gain = g.gain + stun;
                let push = push_of(&g.item);
                if !eligible(pop, ctr, gain, push) {
                    return None;
                }
                why.extend(push_note(push));
                Some((score(item, pop, gain, ctr, push), pop, jobs, why, gain))
            };
            // The best item the souls left pay for, by its score with price held against it as the role says.
            if first {
                first = false;
                // Only what the phase could have bought instead: affordable, and not too cheap for it.
                for g in gains.iter().filter(|g| g.paid <= left) {
                    let Some(item) = data.item(&g.item) else { continue };
                    if item.cost < MIN_COST[index] && g.paid >= item.cost {
                        continue;
                    }
                    let pop = popular(item);
                    let (ctr, jobs, mut why, stun) = answer_of(&g.item, g.imbue);
                    let gain = g.gain + stun;
                    let push = push_of(&g.item);
                    why.extend(push_note(push));
                    let mut notes: Vec<String> = team.redundancy(item).1.into_iter().collect();
                    notes.extend(why.into_iter().filter(|w| SHOWN_NOTES.iter().any(|p| w.starts_with(p))));
                    considered.push(Considered {
                        item: g.item.clone(),
                        name: item.name.clone(),
                        icon: item.icon.clone(),
                        category: item.category.clone(),
                        cost: item.cost,
                        score: score(item, pop, gain, ctr, push),
                        popular_pct: pop * 100.0,
                        win_pct: win(item).0,
                        win_pp: win(item).1,
                        engine_pct: gain.max(0.0) * 100.0,
                        counter_pct: ctr * 100.0,
                        jobs,
                        notes,
                        left_out: !eligible(pop, ctr, gain, push),
                    });
                }
                considered.sort_by(|a, b| b.score.total_cmp(&a.score));
            }
            let best = gains
                .iter()
                .filter(|g| g.paid <= left)
                .filter(|g| data.item(&g.item).is_some_and(|i| i.cost >= MIN_COST[index] || g.paid < i.cost))
                .filter_map(|g| score_of(g).map(|s| (s.0 / (g.paid as f64 / 3_200.0).max(0.25).powf(price_weight), s, g)))
                .filter(|(_, s, _)| s.0 >= MIN_SCORE)
                .max_by(|a, b| a.0.total_cmp(&b.0));
            if let Some((_, (score, pop, jobs, why, gain), g)) = best {
                let item = data.item(&g.item).ok_or("Unknown item")?;
                let upgrades = item.components.iter().find(|c| b.purchases.contains(c)).cloned();
                if let Some(x) = upgrades.as_ref().and_then(|c| items.iter_mut().find(|x| &x.item == c)) {
                    x.upgraded = true;
                }
                b.purchases.push(g.item.clone());
                if let Some(i) = g.imbue {
                    b.imbues.insert(g.item.clone(), i);
                }
                paid_total += g.paid;
                items.push(MasterItem {
                    item: g.item.clone(),
                    name: item.name.clone(),
                    icon: item.icon.clone(),
                    category: item.category.clone(),
                    cost: item.cost,
                    active: item.active,
                    description: item.description.clone(),
                    phase: index,
                    paid: g.paid,
                    score,
                    popular_pct: pop * 100.0,
                    win_pct: win(item).0,
                    win_pp: win(item).1,
                    engine_pct: gain.max(0.0) * 100.0,
                    jobs,
                    notes: team
                        .redundancy(item)
                        .1
                        .into_iter()
                        .chain(why.into_iter().filter(|w| SHOWN_NOTES.iter().any(|p| w.starts_with(p))))
                        .collect(),
                    sold: false,
                    upgrades: upgrades.and_then(|c| data.item(&c)).map(|i| i.name.clone()),
                    upgraded: false,
                    imbued: g.imbue.and_then(|i| hero.abilities.get(i)).map(|a| a.name.clone()),
                    counter: counter_items.contains(&g.item),
                });
                continue;
            }
            // Nothing fits: with the slots full, sell the weakest cheap item for something clearly better.
            let sheet = calc::compute(data, &b).map_err(|e| e.to_string())?;
            if sheet.slots_used < sheet.slot_limit || sold_here.len() >= MAX_SALES {
                break;
            }
            let Some(weakest) = items
                .iter()
                .filter(|x| !x.sold && x.cost <= SELLABLE_COST && owned.contains(&x.item))
                // Counters are sold last: they answer the enemy team, which the score alone does not show.
                .min_by(|a, b| a.counter.cmp(&b.counter).then(a.score.total_cmp(&b.score)))
                .map(|x| (x.item.clone(), x.score, x.cost, x.name.clone()))
            else {
                break;
            };
            let mut without = b.clone();
            without.purchases.retain(|c| c != &weakest.0);
            let refund = (weakest.2 as f64 * SELL_BACK) as u32;
            let gains = suggest::gains(data, &without, gains_style, &candidates);
            let better = gains
                .iter()
                .filter(|g| g.paid <= left + refund)
                .filter(|g| data.item(&g.item).is_some_and(|i| i.cost >= MIN_COST[index] || g.paid < i.cost))
                .filter_map(|g| {
                    let item = data.item(&g.item)?;
                    let (ctr, _, _, stun) = answer_of(&g.item, g.imbue);
                    let gain = g.gain + stun;
                    let pop = popular(item);
                    let push = push_gain(data, &without, &g.item, index);
                    let score = score(item, pop, gain, ctr, push);
                    (eligible(pop, ctr, gain, push) && score >= weakest.1 * SELL_FOR[index] && score >= MIN_SCORE).then_some(score)
                })
                .next()
                .is_some();
            if !better {
                break;
            }
            b.purchases.retain(|c| c != &weakest.0);
            b.imbues.remove(&weakest.0);
            paid_total = paid_total.saturating_sub(refund);
            if let Some(x) = items.iter_mut().find(|x| x.item == weakest.0) {
                x.sold = true;
            }
            sold_here.push(weakest.3);
        }
        considered.retain(|c| !items.iter().any(|x| x.item == c.item));
        considered.truncate(CONSIDERED);
        phases.push(MasterPhase { key: key.to_string(), title: title.to_string(), souls_from: *from, souls_to: *to, sold: sold_here, considered });
    }
    let sheet = calc::compute(data, &b).map_err(|e| e.to_string())?;
    // An item bought into a bigger one is no longer owned at the end, but it was not sold either.
    let purchases: Vec<String> = items.iter().filter(|x| !x.sold && sheet.owned.contains(&x.item)).map(|x| x.item.clone()).collect();

    // The player's own purchases in a match, on the same terms: how many top players buy them at that point,
    // what they answer, and whether the team already did it.
    let judged = input
        .bought
        .iter()
        .filter_map(|(class, net_worth)| {
            let item = data.item(class)?;
            let phase = phase_of(*net_worth);
            let (_, _, from, to, stage, _) = PHASES[phase];
            let (jobs, why) = stage_scores(stage).into_iter().find(|c| &c.0 == class).map(|c| (c.2, c.3)).unwrap_or_default();
            let mut notes: Vec<String> = team.redundancy(item).1.into_iter().collect();
            if m.ranged {
                if let Some(radius) = popular::close_range(item) {
                    notes.push(format!("Works within {radius:.0}m, and this hero fights from range"));
                }
            }
            notes.extend(why.into_iter().filter(|w| w.starts_with("Harder") || w.starts_with("Timing") || w.starts_with("Top players")));
            Some(JudgedItem {
                item: class.clone(),
                name: item.name.clone(),
                icon: item.icon.clone(),
                category: item.category.clone(),
                cost: item.cost,
                active: item.active,
                description: item.description.clone(),
                phase,
                net_worth: *net_worth,
                popular_pct: share(input.phase_stats, item.id, from, to, input.hero_matches) * 100.0,
                win_pct: phase_win(input.phase_stats, item.id, from, to).0,
                win_pp: {
                    let (wr, n) = phase_win(input.phase_stats, item.id, from, to);
                    if n > 0.0 { popular::shrunk(wr - phase_base(input.phase_stats, from, to), n) } else { 0.0 }
                },
                jobs,
                notes,
                in_master: items.iter().any(|x| &x.item == class),
            })
        })
        .collect();
    Ok(MasterBuild {
        hero: hero.class_name.clone(),
        hero_name: hero.name.clone(),
        focus: focus.0.into(),
        focus_name: focus.1.into(),
        focus_goal: focus.2.into(),
        style,
        phases,
        items,
        imbues: b.imbues.iter().filter(|(c, _)| purchases.contains(c)).map(|(c, i)| (c.clone(), *i)).collect(),
        purchases,
        notes,
        judged,
        counter,
    })
}
