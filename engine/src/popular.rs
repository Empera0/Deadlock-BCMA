//! What players actually build, from the match statistics of the community Deadlock API, and what to buy
//! against a team of enemy heroes.
//!
//! The statistics count, per item, the matches a hero bought it in, how often it was sold again, when it was
//! bought on average and how those matches ended. An item bought and kept in most matches is part of the hero's
//! core build; one bought in a fair share of matches but far from all is a situational pick: the counter slot.
//! Against a given enemy, an item bought clearly more often than usual is a counter players reach for, and the
//! enemy's own core build and kit say why.

use crate::calc;
use crate::build::BuildInput;
use crate::counters::{self, Threat};
use crate::data::{GameData, Item};
use crate::matches::{gives_resist, heal_cut_of, reduces_healing, resist_of, shred_of, shreds_bullet, shreds_spirit};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// One row of the API's item statistics.
#[derive(Debug, Clone, Deserialize)]
pub struct ItemStat {
    pub item_id: u64,
    #[serde(default)]
    pub wins: u64,
    #[serde(default)]
    pub losses: u64,
    #[serde(default)]
    pub matches: u64,
    #[serde(default)]
    pub avg_buy_time_s: f64,
    #[serde(default)]
    pub sold: u64,
    /// With a bucketed query: the start of the bucket, e.g. the net worth when the item was bought.
    #[serde(default)]
    pub bucket: Option<u64>,
}

/// One row of the API's hero statistics.
#[derive(Debug, Clone, Deserialize)]
pub struct HeroStat {
    pub hero_id: u64,
    #[serde(default)]
    pub matches: u64,
    #[serde(default)]
    pub wins: u64,
}

#[derive(Debug, Clone, Serialize)]
pub struct PopularItem {
    pub item: String,
    pub name: String,
    pub icon: String,
    pub category: String,
    pub cost: u32,
    pub active: bool,
    pub description: String,
    /// Matches the hero bought it in, and kept it to the end, percent of the hero's matches.
    pub bought_pct: f64,
    pub kept_pct: f64,
    /// Matches won with it, percent.
    pub win_pct: f64,
    /// When it is bought on average, in minutes.
    pub buy_minute: f64,
}

#[derive(Debug, Clone, Serialize)]
pub struct PopularBuild {
    pub hero: String,
    pub hero_name: String,
    /// The hero's matches the numbers come from.
    pub matches: u64,
    /// The items most players buy and keep, in the order they are bought.
    pub core: Vec<PopularItem>,
    /// Bought in a fair share of matches but not most: the picks for the counter slot.
    pub situational: Vec<PopularItem>,
}

/// The core build has this many items; situational picks are kept in at least this share of matches.
pub const CORE_ITEMS: usize = 8;
const SITUATIONAL_ITEMS: usize = 8;
const SITUATIONAL_MIN_KEPT_PCT: f64 = 5.0;

fn popular_item(item: &Item, row: &ItemStat, matches: u64) -> PopularItem {
    let total = matches.max(1) as f64;
    PopularItem {
        item: item.class_name.clone(),
        name: item.name.clone(),
        icon: item.icon.clone(),
        category: item.category.clone(),
        cost: item.cost,
        active: item.active,
        description: item.description.clone(),
        bought_pct: row.matches as f64 / total * 100.0,
        kept_pct: row.matches.saturating_sub(row.sold) as f64 / total * 100.0,
        win_pct: if row.matches > 0 { row.wins as f64 / row.matches as f64 * 100.0 } else { 0.0 },
        buy_minute: row.avg_buy_time_s / 60.0,
    }
}

/// The hero's core build and situational picks from its item statistics and its number of matches.
pub fn popular_build(data: &GameData, hero_id: u64, hero_matches: u64, stats: &[ItemStat]) -> Option<PopularBuild> {
    let hero = data.heroes.iter().find(|h| h.id == hero_id)?;
    let mut rows: Vec<PopularItem> = stats
        .iter()
        .filter_map(|row| data.items.iter().find(|i| i.id == row.item_id && i.cost > 0 && i.tier <= 4).map(|item| popular_item(item, row, hero_matches)))
        .collect();
    rows.sort_by(|a, b| b.kept_pct.total_cmp(&a.kept_pct));
    let mut core: Vec<PopularItem> = rows.iter().take(CORE_ITEMS).cloned().collect();
    core.sort_by(|a, b| a.buy_minute.total_cmp(&b.buy_minute));
    let situational = rows.iter().skip(CORE_ITEMS).filter(|r| r.kept_pct >= SITUATIONAL_MIN_KEPT_PCT).take(SITUATIONAL_ITEMS).cloned().collect();
    Some(PopularBuild { hero: hero.class_name.clone(), hero_name: hero.name.clone(), matches: hero_matches, core, situational })
}

/// How much more often each item is bought against an enemy than in general, from two sets of statistics with
/// the same filters, one of them limited to matches against that enemy. Both are measured against the most
/// bought item of their own set, so the two sets need not be the same size.
/// How an item does against one enemy compared with in general.
#[derive(Debug, Clone, Copy)]
pub struct Lift {
    pub item_id: u64,
    /// How many times as often top players buy it against this enemy.
    pub bought: f64,
    /// How many points more (or fewer) of the matches are won with it against this enemy than with it in general,
    /// shrunk towards zero when there are few matches.
    pub win_pp: f64,
}

/// A win rate from this many matches counts half: fewer move a lot by chance.
pub const WIN_PRIOR: f64 = 300.0;

/// Win rate of rows, percent, and the matches it is from.
pub fn win_rate(rows: &[&ItemStat]) -> (f64, f64) {
    let (w, l) = rows.iter().fold((0.0, 0.0), |(w, l), r| (w + r.wins as f64, l + r.losses as f64));
    if w + l > 0.0 { (w / (w + l) * 100.0, w + l) } else { (0.0, 0.0) }
}

/// A difference in win rate, shrunk towards zero by the size of the sample.
pub fn shrunk(diff_pp: f64, matches: f64) -> f64 {
    diff_pp * matches / (matches + WIN_PRIOR)
}

pub fn lifts(general: &[ItemStat], against: &[ItemStat]) -> Vec<Lift> {
    let top = |rows: &[ItemStat]| rows.iter().map(|r| r.matches).max().unwrap_or(1).max(1) as f64;
    let (g_top, a_top) = (top(general), top(against));
    against
        .iter()
        .filter_map(|a| {
            let g = general.iter().find(|g| g.item_id == a.item_id)?;
            let base = g.matches as f64 / g_top;
            // Items almost nobody buys move a lot by chance.
            if base < 0.03 {
                return None;
            }
            let (wr_a, n_a) = win_rate(&[a]);
            let (wr_g, _) = win_rate(&[g]);
            Some(Lift { item_id: a.item_id, bought: (a.matches as f64 / a_top) / base, win_pp: shrunk(wr_a - wr_g, n_a) })
        })
        .collect()
}

/// Whether a build deals mostly spirit damage: more of its souls in spirit items than in weapon items.
pub fn deals_spirit(data: &GameData, build: &PopularBuild) -> bool {
    match gun_pct(data, build) {
        Some(gun) => gun < 50.0,
        None => {
            let souls = |category: &str| build.core.iter().filter_map(|p| data.item(&p.item)).filter(|i| i.category == category).map(|i| i.cost).sum::<u32>();
            souls("spirit") > souls("weapon")
        }
    }
}

/// The souls and ability upgrades a hero is taken to have when its damage is split into gun and spirit: late in
/// the match, when the damage that decides fights is dealt.
const SPLIT_SOULS: u32 = 30_000;
const SPLIT_TIERS: [u8; 4] = [2, 2, 2, 2];

/// Share of a hero's damage in a fight that is gun damage, with its usual build late in the match, percent.
/// Item souls are no guide: a hero buying weapon items can still deal most of its damage with abilities.
pub fn gun_pct(data: &GameData, build: &PopularBuild) -> Option<f64> {
    let b = BuildInput {
        hero: build.hero.clone(),
        purchases: build.core.iter().map(|c| c.item.clone()).collect(),
        souls: Some(SPLIT_SOULS),
        ability_tiers: SPLIT_TIERS.to_vec(),
        ..Default::default()
    };
    crate::suggest::spirit_share(data, &b).map(|s| (1.0 - s) * 100.0)
}

/// What an enemy hero brings, read from its kit and its usual build.
#[derive(Debug, Clone, Serialize)]
pub struct EnemyProfile {
    pub hero: String,
    pub hero_name: String,
    /// Share of its damage in a fight that is gun damage (bullets and melee), percent, worked out by the calculator
    /// with its usual build late in the match.
    pub weapon_pct: f64,
    /// Abilities and items it heals with.
    pub heals: Vec<String>,
    /// Abilities it moves, flies, turns invisible or escapes with.
    pub mobility: Vec<String>,
    /// Abilities that stun, root, sleep or silence.
    pub lockdown: Vec<String>,
    /// Resist items in its usual build.
    pub resists: Vec<String>,
    /// Abilities that let it fly.
    pub flying: Vec<String>,
}

/// One item suggested for one job against the enemy team.
#[derive(Debug, Clone, Serialize)]
pub struct CounterAdvice {
    pub item: String,
    pub name: String,
    pub icon: String,
    pub category: String,
    pub cost: u32,
    pub active: bool,
    pub description: String,
    /// What it does for this job, e.g. `-35% healing on the target`.
    pub effect: String,
    pub score: f64,
    /// Why it is suggested: which enemies, and how often top players buy it against them.
    pub reasons: Vec<String>,
    pub when: String,
    pub against: Vec<String>,
}

/// A job items do against the enemy team, how much the team calls for it, and the best items for it.
#[derive(Debug, Clone, Serialize)]
pub struct RoleAdvice {
    pub key: String,
    pub title: String,
    /// Why the team calls for it.
    pub need: String,
    pub weight: f64,
    pub items: Vec<CounterAdvice>,
}

#[derive(Debug, Clone, Serialize)]
pub struct CounterPlan {
    pub enemies: Vec<EnemyProfile>,
    pub roles: Vec<RoleAdvice>,
    /// The player's own hero has a way out of crowd control, so answers to it count for less.
    pub own_escape: Vec<String>,
    /// The two counter items for each stage of the game: the lane, the mid game and the late game.
    pub stages: Vec<CounterStage>,
    /// Every other item that answers something this team does, best first.
    pub others: Vec<CounterPick>,
    /// The player's teammates, read like the enemies.
    pub allies: Vec<EnemyProfile>,
    /// Each enemy, what it threatens and what answers it.
    pub heroes: Vec<HeroAnswer>,
    /// How the player's own team changed the plan.
    pub team: Vec<String>,
}

/// The counter items worth buying at one stage of the game, with what they cost.
#[derive(Debug, Clone, Serialize)]
pub struct CounterStage {
    pub key: String,
    pub title: String,
    /// When, and what the items at this stage are for.
    pub note: String,
    pub picks: Vec<CounterPick>,
    pub alternatives: Vec<CounterPick>,
    /// Every item that answers something at this stage, whatever its price: (item, score, jobs, notes).
    #[serde(skip)]
    pub scored: Vec<(String, f64, Vec<String>, Vec<String>)>,
}

/// An item for a counter slot, with every job it does against this team.
#[derive(Debug, Clone, Serialize)]
pub struct CounterPick {
    pub advice: CounterAdvice,
    /// Already in the player's usual build: against this team it is worth buying early rather than as an extra.
    pub in_core: bool,
    /// The jobs it does, by title, most valuable first.
    pub jobs: Vec<String>,
}

/// A build has room for this many counter items: the rest of the twelve slots are the hero's own build.
pub const COUNTER_SLOTS: usize = 2;
const ALTERNATIVES: usize = 2;
/// The list of other counter items stops at this many.
const OTHERS: usize = 30;
/// The stages of the game the counters are planned for: key, title, the price range of the items, and a note.
const STAGES: [(&str, &str, u32, u32, &str); 3] = [
    ("lane", "In the lane", 0, 1_600, "Cheap answers for the first ten minutes, while the lane is decided"),
    ("mid", "Mid game", 3_200, 3_200, "Once the first fights and objectives start, around 10 to 20 minutes"),
    ("late", "Late game", 6_400, 6_400, "For the big fights from about 20 minutes, often replacing a lane counter"),
];
/// Where each stage's measurements are taken: souls, and the ability upgrades a hero usually has by then.
const MEASURE_AT: [(u32, [u8; 4]); 3] = [(5_000, [0, 0, 0, 0]), (15_000, [1, 1, 1, 0]), (30_000, [2, 2, 2, 2])];
/// What taking a whole enemy's damage away is worth against the other jobs: an item that takes a third off two
/// enemies' damage scores about as an item made for a job the whole team calls for.
const MEASURED_K: f64 = 3.0;
/// A cut this small is not worth mentioning.
const CUT_WORTH_SAYING: f64 = 0.03;

/// The team heals a lot once the healing job weighs this much (about three healers): one of the two picks
/// then has to cut healing, and do it reliably.
const HEAVY_HEALING: f64 = 1.3;
/// A pick does a job reliably when its strength times how easily it lands is at least this.
const RELIABLE: f64 = 0.55;
/// A job the first pick already does is worth this much for the second.
const COVERED: f64 = 0.35;
/// A job an item from the stage before still does is worth this much: that item is still in the build.
const CARRIED: f64 = 0.6;

/// An enemy with what it builds and how much more often each item is bought against it.
pub struct Enemy<'a> {
    pub hero_id: u64,
    pub build: &'a PopularBuild,
    pub lifts: &'a [Lift],
}

/// Ability properties that mean the ability heals its hero.
const ABILITY_HEAL_KEYS: [&str; 32] = [
    "HealingFactor",
    "AbilityLifestealPercentHero",
    "HealFixedHealth",
    "HealMaxHealth",
    "BeamLifesteal",
    "MissingHPHeal",
    "HealAmount",
    "HealingPerSecond",
    "HealMaxHealthPercent",
    "LifestealPercentHero",
    "LifeStealPercentOnHit",
    "BulletLifestealPercent",
    "BulletLifesteal",
    "TechLifestealPercent",
    "TetherSharedHealPct",
    "LifeDrainPerSecond",
    "HealPctVsHeroes",
    "ExternalBonusHealthRegen",
    "InfestHeal",
    "HealOnHit",
    "TotalHealthRegen",
    "BeamHealPerSecond",
    "HeartHeal",
    "HealthStealPctHero",
    "DamageHealMult",
    "MeleeLifesteal",
    "TargetLifesteal",
    "RegenIncomingDamagePercent",
    "MaxHealthRegen",
    "BonusHealthRegen",
    "HealPerPickup",
    "RakeHealPerKill",
];
/// Ability properties that mean the ability moves its hero fast, lifts it off the ground or hides it.
const ABILITY_MOVE_KEYS: [&str; 20] = [
    "DashSpeed",
    "DashRange",
    "DashAirSpeed",
    "TeleportBackOffsetFromTarget",
    "TeleportCompletedTime",
    "InvisFadeToDuration",
    "DragonTravelRange",
    "FlightControlEnabled",
    "MaxFlyHeight",
    "JumpVelocity",
    "JumpSpeed",
    "JumpForce",
    "JumpVelocityHidden",
    "EndJumpVelocity",
    "MiniJumpVelocity",
    "KnightJumpSpeed",
    "ChargeSpeed",
    "ChargeSpeedMax",
    "LeapRadius",
    "HeatGainAirDash",
];
/// Ability properties that get its hero out of crowd control or out of reach.
const ABILITY_ESCAPE_KEYS: [&str; 6] = ["ImmunityDuration", "InvisFadeToDuration", "TeleportCompletedTime", "DashRange", "DashSpeed", "FlightControlEnabled"];
/// An item bought this much more often against an enemy than in general is a counter top players reach for.
const LIFT_FROM: f64 = 1.1;
/// A difference in win rate against an enemy is mentioned from this many points.
const WIN_SAY_PP: f64 = 2.0;
/// What a point of win rate against an enemy adds to an item's answer: 5 points more is about half a job.
const WIN_K: f64 = 10.0;

fn win_text(pp: f64, enemy: &str) -> String {
    if pp > 0.0 {
        format!("Top players win {pp:.0} points more often with it against {enemy} than with it in general")
    } else {
        format!("Top players win {:.0} points less often with it against {enemy} than with it in general", -pp)
    }
}
/// Each job lists its best items in each of these price ranges: two cheap ones for the lane, then one each.
const PRICE_BANDS: [(u32, u32, usize); 3] = [(0, 1_600, 2), (3_200, 3_200, 1), (6_400, 6_400, 1)];
/// Lifesteal on a weapon or spirit item from this much is a healing item in its own right.
const MAJOR_LIFESTEAL: f64 = 25.0;
const HEAL_KEYS: [&str; 15] = [
    "BulletLifestealPercent",
    "AbilityLifestealPercentHero",
    "LifestealHeal",
    "LifestealHealPercent",
    "HealthStealPctHero",
    "ActiveBonusLifesteal",
    "BonusSpiritLifesteal",
    "ImbueAbilityLifesteal",
    "LifestrikeHeal",
    "HealFromHero",
    "TotalHealthRegen",
    "HealingPerCast",
    "HealPercentAmount",
    "HealPerStack",
    "HealAmount",
];

/// An item that makes its owner a healer worth cutting: real lifesteal or healing. A little lifesteal on a
/// damage item (Active Reload, Spirit Shredder) does not count.
fn heals(item: &Item) -> bool {
    item.props.iter().any(|p| {
        p.value > 0.0
            && HEAL_KEYS.contains(&p.key.as_str())
            && (!p.key.contains("LifestealPercent") || item.category == "vitality" || p.value >= MAJOR_LIFESTEAL)
    })
}

fn clock(minute: f64) -> String {
    let s = (minute * 60.0).round().max(0.0) as u32;
    format!("{}:{:02}", s / 60, s % 60)
}

fn prop(item: &Item, key: &str) -> f64 {
    item.props.iter().filter(|p| p.key == key).map(|p| p.value.abs()).fold(0.0, f64::max)
}

#[derive(Clone, Copy, PartialEq, Debug)]
enum Role {
    Healing,
    Gun,
    Spirit,
    Mobility,
    Control,
    Shred,
    Flight,
}

/// A team fight lasts about this long: an active whose effect lasts this long covers it.
const FIGHT_S: f64 = 10.0;

/// How easily and for how long an item's effect lands for the player's hero (0 to 1), with notes on why.
/// A headshot or a burst of spirit damage is harder to bring about than a bullet hit or a cast; an active
/// that lasts a few seconds covers only part of a fight; an effect that cannot hit the same enemy again for a
/// while is up only part of the time.
/// `gun` is the share of the player's damage that is gun damage (0 to 1), when known.
fn ease(item: &Item, gun: Option<f64>) -> (f64, Vec<String>) {
    let d = item.description.to_lowercase();
    let gun_share = gun.unwrap_or(0.5);
    let spirit_share = 1.0 - gun_share;
    let mut notes = vec![];
    let mut hard = |factor: f64, why: &str| {
        notes.push(format!("Harder to use: {why}"));
        factor
    };
    let trigger = if d.starts_with("landing a headshot") || d.starts_with("your next headshot") {
        hard(0.5, "only works on headshots")
    } else if d.contains("melee attack") {
        hard(0.5, "only works on melee hits")
    } else if d.contains("dealing significant spirit damage") {
        // An effect that comes with the hero's damage works as much as the hero deals that kind of damage.
        let f = 0.3 + 0.6 * spirit_share;
        if f < 0.7 { hard(f, &format!("needs bursts of spirit damage, and only {:.0}% of this hero's damage is spirit", spirit_share * 100.0)) } else { f }
    } else if d.contains("your bullets") {
        let f = (0.4 + 0.6 * gun_share) * if d.contains("chance") { 0.8 } else { 1.0 };
        if gun_share < 0.5 { hard(f, &format!("works through bullets, and only {:.0}% of this hero's damage is the gun", gun_share * 100.0)) } else { f }
    } else if d.contains("your spirit damage") || d.contains("deal spirit damage") {
        let f = 0.4 + 0.6 * spirit_share;
        if spirit_share < 0.5 { hard(f, &format!("works through spirit damage, and only {:.0}% of this hero's damage is spirit", spirit_share * 100.0)) } else { f }
    } else {
        1.0
    };
    let get = |key: &str| item.props.iter().find(|p| p.key == key).map(|p| p.value.abs()).unwrap_or(0.0);
    let (duration, cooldown) = (get("AbilityDuration"), get("AbilityCooldown"));
    let (debuff, immune) = (get("DebuffDuration"), get("ImmunityDuration"));
    let uptime = if item.active && duration > 0.0 {
        notes.push(format!("Timing: lasts {duration:.1}s, every {cooldown:.0}s; cast it when the fight starts"));
        0.4 + 0.6 * (duration / FIGHT_S).min(1.0)
    } else if !item.active && debuff > 0.0 && immune > debuff && get("BuildUpPerShot") == 0.0 {
        // e.g. Spirit Burn: burns for 8s, then cannot burn the same enemy for 20s.
        notes.push(format!("Timing: works for {debuff:.0}s, then not on the same enemy for {immune:.0}s"));
        0.4 + 0.6 * debuff / immune
    } else if get("BuildUpPerShot") > 0.0 {
        notes.push("Timing: builds up over a few seconds of hitting the same enemy".into());
        0.9
    } else {
        1.0
    };
    (trigger * uptime, notes)
}

/// What an item does for each job, with how strongly (about 1 for an item made for the job).
/// `shred` is how much bullet and spirit resist reduction are worth to the player's team (0 to 1 each).
fn roles_of(item: &Item, shred: (f64, f64)) -> Vec<(Role, f64, String)> {
    let mut out = vec![];
    let heal_cut = heal_cut_of(item);
    if reduces_healing(item) && heal_cut > 0.0 {
        out.push((Role::Healing, heal_cut / 45.0, format!("-{heal_cut:.0}% healing on whoever it hits")));
    }
    let fire_slow = prop(item, "FireRateSlow");
    let reflect = prop(item, "BulletDamageReflectedPct");
    // "Disarms enemy target" is a disarm; "immune to ... Disarm" (Unstoppable) is not.
    let disarm = crate::team::disarms_enemies(item);
    let bullet = resist_of(item, false);
    let damage_cut = crate::team::enemy_damage_cut(item);
    let deflect = prop(item, "DeflectionPercent");
    if item.description.to_lowercase().contains("immune to bullets") {
        out.push((Role::Gun, 1.4, "immune to bullets while it lasts".into()));
    } else if damage_cut > 0.0 {
        out.push((Role::Gun, damage_cut / 25.0, format!("-{damage_cut:.0}% damage dealt by the target")));
    } else if fire_slow > 0.0 {
        out.push((Role::Gun, fire_slow / 25.0, format!("-{fire_slow:.0}% fire rate on enemies it hits")));
    } else if disarm {
        out.push((Role::Gun, 1.2, "disarms: the enemy cannot shoot for a while".into()));
    } else if deflect > 0.0 {
        out.push((Role::Gun, deflect / 22.0, format!("deflects {deflect:.0}% of the bullets that hit you")));
    } else if reflect > 0.0 {
        out.push((Role::Gun, reflect / 70.0, format!("reflects {reflect:.0}% of the bullet damage taken")));
    } else if bullet >= 10.0 {
        out.push((Role::Gun, bullet / 22.0, format!("{bullet:.0}% bullet resist")));
    }
    let sap = prop(item, "TechPowerReduction");
    let spirit_cut = prop(item, "TechDamageReduction");
    let proc_cut = prop(item, "SpiritDamageReductionProc");
    let spirit = resist_of(item, true);
    if damage_cut > 0.0 {
        out.push((Role::Spirit, damage_cut / 25.0, format!("-{damage_cut:.0}% damage dealt by the target")));
    } else if sap > 0.0 {
        out.push((Role::Spirit, sap / 25.0, format!("-{sap:.0} spirit power on the target")));
    } else if spirit_cut > 0.0 {
        out.push((Role::Spirit, spirit_cut / 22.0, format!("-{spirit_cut:.0}% spirit damage dealt by the target")));
    } else if proc_cut > 0.0 {
        out.push((Role::Spirit, proc_cut / 60.0, format!("cuts the next big spirit hit by {proc_cut:.0}%")));
    } else if crate::team::silences_enemies(item) {
        out.push((Role::Spirit, 1.0, "silences: the enemy cannot cast for a while".into()));
    } else if spirit >= 10.0 {
        out.push((Role::Spirit, spirit / 22.0, format!("{spirit:.0}% spirit resist")));
    }
    let dash = prop(item, "GroundDashReductionPercent");
    // A slow counts when the item's own text says it slows enemies (Glass Cannon carries unused slow numbers).
    let slow = if crate::team::slows_enemies(item) { prop(item, "SlowPercent").max(prop(item, "MovementSpeedSlow")).max(prop(item, "MaxSlowPercent")) } else { 0.0 };
    let flight = counters::answers(item).contains(&Threat::Flight);
    // A dash penalty only pins enemies down when it comes with a slow on them: on Metal Skin it is the
    // owner's own penalty while the item is active.
    if dash > 0.0 && slow > 0.0 {
        out.push((Role::Mobility, (dash + slow) / 40.0, format!("-{dash:.0}% dash distance{}", if slow > 0.0 { format!(" and {slow:.0}% slow") } else { String::new() })));
    } else if slow >= 20.0 {
        out.push((Role::Mobility, slow / 50.0, format!("{slow:.0}% slow")));
    }
    if flight {
        out.push((Role::Flight, 1.0, "brings flying heroes down".into()));
    }
    let answers = counters::answers(item);
    if answers.contains(&Threat::Lockdown) || answers.contains(&Threat::Silence) {
        out.push((Role::Control, 1.0, "breaks, shortens or blocks stuns and silences".into()));
    }
    for spirit_damage in [false, true] {
        let worth = if spirit_damage { shred.1 } else { shred.0 };
        if worth <= 0.0 {
            continue;
        }
        let amount = shred_of(item, spirit_damage);
        let shreds = if spirit_damage { shreds_spirit(item) } else { shreds_bullet(item) };
        if shreds && amount > 0.0 {
            let shred = amount;
            out.push((Role::Shred, shred / 10.0 * worth, format!("-{shred:.0}% {} resist on the target", if spirit_damage { "spirit" } else { "bullet" })));
        }
    }
    out
}

/// What an enemy brings, from its kit and its usual build.
fn profile(data: &GameData, enemy: &Enemy) -> EnemyProfile {
    let core: Vec<&Item> = enemy.build.core.iter().filter_map(|p| data.item(&p.item)).collect();
    let souls = |category: &str| core.iter().filter(|i| i.category == category).map(|i| i.cost as f64).sum::<f64>();
    let (weapon, spirit) = (souls("weapon"), souls("spirit"));
    let mut heals_with: Vec<String> = core.iter().filter(|i| heals(i)).map(|i| i.name.clone()).collect();
    let mut mobility = vec![];
    let mut lockdown = vec![];
    let mut flying = vec![];
    if let Some(hero) = data.heroes.iter().find(|h| h.id == enemy.hero_id) {
        for a in &hero.abilities {
            // A property counts when the ability has it, or when one of its upgrades adds it.
            let has = |keys: &[&str]| {
                a.props.iter().any(|p| p.value > 0.0 && keys.contains(&p.key.as_str()))
                    || a.upgrades.iter().flatten().any(|u| u.bonus > 0.0 && keys.contains(&u.name.as_str()))
            };
            if has(&ABILITY_HEAL_KEYS) {
                heals_with.push(a.name.clone());
            }
            if has(&ABILITY_MOVE_KEYS) {
                mobility.push(a.name.clone());
            }
        }
        let b = BuildInput { hero: hero.class_name.clone(), ability_tiers: vec![1, 1, 1, 1], ..Default::default() };
        if let Ok(sheet) = calc::compute(data, &b) {
            for found in counters::threats(&sheet.abilities) {
                match found.kind {
                    Threat::Lockdown | Threat::Silence if !lockdown.contains(&found.ability) => lockdown.push(found.ability.clone()),
                    Threat::Flight if !flying.contains(&found.ability) => flying.push(found.ability.clone()),
                    _ => {}
                }
            }
        }
        // Stuns and silences the property names do not show (checked against the wiki).
        for a in hero.abilities.iter().filter(|a| crate::team::locks_down(a)) {
            if !lockdown.contains(&a.name) {
                lockdown.push(a.name.clone());
            }
        }
    }
    // Flight is its own job; it is not counted again as getting around.
    mobility.retain(|m| !flying.contains(m));
    EnemyProfile {
        hero: enemy.build.hero.clone(),
        hero_name: enemy.build.hero_name.clone(),
        weapon_pct: gun_pct(data, enemy.build).unwrap_or(if weapon + spirit > 0.0 { weapon / (weapon + spirit) * 100.0 } else { 50.0 }),
        heals: heals_with,
        mobility,
        lockdown,
        resists: core.iter().filter(|i| gives_resist(i, false) || gives_resist(i, true)).map(|i| i.name.clone()).collect(),
        flying,
    }
}

/// How much one enemy calls for a job, 0 to about 1, and a few words on why.
fn need(role: Role, p: &EnemyProfile, own_escape: bool) -> (f64, String) {
    match role {
        Role::Healing if !p.heals.is_empty() => ((p.heals.len() as f64 * 0.45).min(1.2), format!("{} heals with {}", p.hero_name, p.heals.join(", "))),
        // Most heroes mix the two; each side counts from 40% of the build up.
        Role::Gun if p.weapon_pct >= 40.0 => ((p.weapon_pct - 30.0) / 60.0, format!("{} deals about {:.0}% of its damage with the gun", p.hero_name, p.weapon_pct)),
        Role::Spirit if p.weapon_pct <= 60.0 => ((70.0 - p.weapon_pct) / 60.0, format!("{} deals about {:.0}% spirit damage", p.hero_name, 100.0 - p.weapon_pct)),
        Role::Mobility if !p.mobility.is_empty() => ((p.mobility.len() as f64 * 0.3).min(0.6), format!("{} gets around with {}", p.hero_name, p.mobility.join(", "))),
        Role::Control if !p.lockdown.is_empty() => {
            let base = (p.lockdown.len() as f64 * 0.35).min(0.9);
            (if own_escape { base * 0.5 } else { base }, format!("{} locks down with {}", p.hero_name, p.lockdown.join(", ")))
        }
        Role::Flight if !p.flying.is_empty() => (0.8, format!("{} flies with {}", p.hero_name, p.flying.join(", "))),
        Role::Shred if !p.resists.is_empty() => (0.6, format!("{} builds {}", p.hero_name, p.resists.join(", "))),
        _ => (0.0, String::new()),
    }
}

/// A hero on the player's own team, with what it usually builds.
pub struct Ally<'a> {
    pub hero_id: u64,
    pub build: &'a PopularBuild,
}

/// Who the counters are planned for and against.
pub struct Matchup<'a> {
    /// The player's hero (class name), checked for ways out of crowd control.
    pub my_hero: Option<&'a str>,
    /// Whether the player's hero deals mostly spirit damage.
    pub my_spirit: Option<bool>,
    /// Share of the player's damage in a fight that is gun damage, percent, when it is known.
    pub my_gun_pct: Option<f64>,
    /// The player's own usual build (item class names): those items are marked when suggested.
    pub my_core: &'a [String],
    /// The rest of the player's team.
    pub allies: &'a [Ally<'a>],
    pub enemies: &'a [Enemy<'a>],
    /// The enemies in the player's lane (hero ids). The lane counters answer them alone; empty means the whole team.
    pub lane: &'a [u64],
    /// The player's hero fights from range: items that only work close by count for less.
    pub ranged: bool,
}

/// Jobs a teammate's items do for the whole team: healing reduction does not stack, and one slow or one way to
/// bring flyers down is often enough. Defence against damage and crowd control is everyone's own.
const SHARED_JOBS: [Role; 3] = [Role::Healing, Role::Mobility, Role::Flight];
/// A passive that works on enemies within this many meters only helps heroes who fight close.
const CLOSE_RANGE_M: f64 = 20.0;

/// The range of a passive item that only works on enemies close by, if it is one.
pub(crate) fn close_range(item: &Item) -> Option<f64> {
    if item.active {
        return None;
    }
    // A radius is not enough (Mystic Reverb's is around its target, Mystic Shot's is the bullet): the text has to say
    // the effect is on enemies near you.
    let t = item.description.to_lowercase();
    if !(t.contains("nearby") || t.contains("close range")) {
        return None;
    }
    let radius = ["Radius", "CloseRangeBonusDamageRange", "ProcRadius", "DamagePulseRadius"].iter().filter_map(|k| item.props.iter().find(|p| p.key == *k)).map(|p| p.value).fold(0.0, f64::max);
    Some(if radius > 0.0 && radius <= CLOSE_RANGE_M { radius } else { CLOSE_RANGE_M })
}

/// One enemy hero: what it threatens and which of the planned items answer it, stage by stage.
#[derive(Debug, Clone, Serialize)]
pub struct HeroAnswer {
    pub hero: String,
    pub hero_name: String,
    pub in_lane: bool,
    pub threats: Vec<String>,
    /// e.g. "Mid game: Decay (Cut their healing)".
    pub answers: Vec<String>,
}

/// The jobs against the enemy team, in the order they are listed.
const JOBS: [(Role, &str, &str); 7] = [
    (Role::Healing, "healing", "Cut their healing"),
    (Role::Gun, "gun", "Blunt their guns"),
    (Role::Spirit, "spirit", "Blunt their spirit damage"),
    (Role::Mobility, "mobility", "Pin down their movement"),
    (Role::Control, "control", "Answer their crowd control"),
    (Role::Shred, "shred", "Get through their resists"),
    (Role::Flight, "flight", "Bring their flyers down"),
];
/// When a teammate usually brings healing reduction, the job is worth this much for the player.
const ALLY_COVERED: f64 = 0.55;

/// A job against some of the enemies: its role, key, title, weight, and each enemy's need with why.
type Job = (Role, &'static str, &'static str, f64, Vec<(f64, String)>);

/// How much each kind of resist reduction is worth (bullet, spirit), from the damage the player's team deals.
/// Resist reduction on a target helps everyone who hits it with that kind of damage.
fn shred_weights(my_spirit: Option<bool>, allies: &[EnemyProfile]) -> (f64, f64) {
    if allies.is_empty() {
        return match my_spirit {
            Some(true) => (0.0, 1.0),
            Some(false) => (1.0, 0.0),
            None => (1.0, 1.0),
        };
    }
    // The player's own damage counts twice: the item is on their hero.
    let mine = match my_spirit {
        Some(true) => 0.0,
        Some(false) => 1.0,
        None => 0.5,
    };
    let bullet = (mine * 2.0 + allies.iter().map(|a| a.weapon_pct / 100.0).sum::<f64>()) / (2.0 + allies.len() as f64);
    ((bullet * 2.0).min(1.0), ((1.0 - bullet) * 2.0).min(1.0))
}

/// Plans the counter items against a team of enemies, stage by stage. Each job (cutting their healing, their
/// gun, their spirit, their mobility, answering their crowd control, getting through their resists) is weighed by
/// how much each enemy calls for it. The lane counters answer the lane opponents alone; the mid and late game
/// answer the whole team, minus the healing reduction a teammate usually brings. Resist reduction is weighed by
/// the damage the player's whole team deals.
pub fn counter_plan(data: &GameData, m: &Matchup) -> CounterPlan {
    let (enemies, my_spirit, my_core) = (m.enemies, m.my_spirit, m.my_core);
    let profiles: Vec<EnemyProfile> = enemies.iter().map(|e| profile(data, e)).collect();
    let allies: Vec<EnemyProfile> = m.allies.iter().map(|a| profile(data, &Enemy { hero_id: a.hero_id, build: a.build, lifts: &[] })).collect();
    let own_escape: Vec<String> = m
        .my_hero
        .and_then(|h| data.hero(h))
        .map(|h| {
            h.abilities
                .iter()
                .filter(|a| a.props.iter().any(|p| p.value > 0.0 && ABILITY_ESCAPE_KEYS.contains(&p.key.as_str())))
                .map(|a| a.name.clone())
                .collect()
        })
        .unwrap_or_default();
    let escape = !own_escape.is_empty();
    let shred = shred_weights(my_spirit, &allies);
    let my_gun = m.my_gun_pct.map(|g| g / 100.0).or(my_spirit.map(|s| if s { 0.2 } else { 0.8 }));
    // What teammates' usual builds already do to the enemies for the whole team.
    let ally_covers: Vec<(Role, String)> = m
        .allies
        .iter()
        .zip(&allies)
        .flat_map(|(a, p)| {
            a.build.core.iter().filter_map(|c| data.item(&c.item)).flat_map(move |i| {
                let easy = ease(i, None).0;
                roles_of(i, shred)
                    .into_iter()
                    .filter(move |r| SHARED_JOBS.contains(&r.0) && r.1 * easy >= RELIABLE)
                    .map(move |r| (r.0, format!("{} usually buys {}", p.hero_name, i.name)))
            })
        })
        .collect();
    let covered_by = |role: Role| -> Vec<String> { ally_covers.iter().filter(|c| c.0 == role).map(|c| c.1.clone()).collect() };
    let mut team = vec![];
    if !allies.is_empty() {
        let names: Vec<&str> = allies.iter().map(|a| a.hero_name.as_str()).collect();
        team.push(format!(
            "With {}: resist reduction is weighed {:.0}% bullet / {:.0}% spirit by the damage your team deals",
            names.join(", "),
            shred.0 / (shred.0 + shred.1).max(0.01) * 100.0,
            shred.1 / (shred.0 + shred.1).max(0.01) * 100.0
        ));
    }
    for (role, _, title) in JOBS.iter().filter(|j| SHARED_JOBS.contains(&j.0)) {
        let by = covered_by(*role);
        if !by.is_empty() {
            team.push(format!("{}: \"{}\" counts for less from the mid game", by.join("; "), title));
        }
    }
    let in_lane: Vec<bool> = enemies.iter().map(|e| m.lane.contains(&e.hero_id)).collect();
    let everyone = vec![true; enemies.len()];
    let shop: Vec<&Item> = data.items.iter().filter(|i| i.cost > 0 && i.tier <= 4).collect();

    let jobs_for = |included: &[bool], with_allies: bool| -> Vec<Job> {
        JOBS.iter()
            .filter_map(|(role, key, title)| {
                let needs: Vec<(f64, String)> =
                    profiles.iter().zip(included).map(|(p, on)| if *on { need(*role, p, escape) } else { (0.0, String::new()) }).collect();
                let mut weight: f64 = needs.iter().map(|n| n.0).sum();
                if weight <= 0.0 {
                    return None;
                }
                if with_allies && ally_covers.iter().any(|c| c.0 == *role) {
                    weight *= ALLY_COVERED;
                }
                Some((*role, *key, *title, weight, needs))
            })
            .collect()
    };

    // Every job against the whole team, with the best items for it by price (the folded list).
    let full = jobs_for(&everyone, true);
    let mut roles = vec![];
    for (role, key, title, weight, needs) in &full {
        let mut items: Vec<CounterAdvice> = shop
            .iter()
            .filter_map(|item| {
                let (_, strength, effect) = roles_of(item, shred).into_iter().find(|r| r.0 == *role)?;
                let mut score = strength * weight;
                let mut reasons: Vec<String> = vec![];
                let mut against: Vec<String> = vec![];
                for ((enemy, profile), (n, why)) in enemies.iter().zip(&profiles).zip(needs) {
                    if *n <= 0.0 {
                        continue;
                    }
                    against.push(profile.hero_name.clone());
                    reasons.push(why.clone());
                    if let Some(l) = enemy.lifts.iter().find(|l| l.item_id == item.id) {
                        if l.bought >= LIFT_FROM {
                            score += (l.bought - 1.0) * 2.5 * n.max(0.3);
                            reasons.push(format!("Top players buy it {:.1}× as often against {}", l.bought, profile.hero_name));
                        }
                        if l.win_pp.abs() >= WIN_SAY_PP {
                            score += l.win_pp / 100.0 * WIN_K * n.max(0.3);
                            reasons.push(win_text(l.win_pp, &profile.hero_name));
                        }
                    }
                }
                // When: as soon as the enemy usually has what it answers.
                let when = profiles
                    .iter()
                    .zip(enemies)
                    .zip(needs)
                    .filter(|(_, n)| n.0 > 0.0)
                    .flat_map(|((p, e), _)| {
                        e.build
                            .core
                            .iter()
                            .filter(|c| match role {
                                Role::Healing => data.item(&c.item).is_some_and(heals),
                                Role::Shred => p.resists.contains(&c.name),
                                _ => false,
                            })
                            .map(|c| c.buy_minute)
                    })
                    .fold(f64::INFINITY, f64::min);
                let when = match role {
                    Role::Control | Role::Mobility => "From the lane: these come with their abilities".to_string(),
                    Role::Gun | Role::Spirit => "As soon as their damage starts to hurt; it grows with their items".to_string(),
                    _ if when.is_finite() => format!("By about {}, when they usually have it", clock(when)),
                    _ => "From the lane: their abilities do it from the start".to_string(),
                };
                Some(CounterAdvice {
                    item: item.class_name.clone(),
                    name: item.name.clone(),
                    icon: item.icon.clone(),
                    category: item.category.clone(),
                    cost: item.cost,
                    active: item.active,
                    description: item.description.clone(),
                    effect,
                    score,
                    reasons,
                    when,
                    against,
                })
            })
            .collect();
        items.sort_by(|a, b| b.score.total_cmp(&a.score));
        // The best item at each price: cheap ones for the lane, one for the mid game, one for late.
        let mut picked: Vec<CounterAdvice> = PRICE_BANDS
            .iter()
            .flat_map(|(low, high, count)| items.iter().filter(|i| i.cost >= *low && i.cost <= *high).take(*count).cloned().collect::<Vec<_>>())
            .collect();
        // And whatever top players lean on most, if it is not there yet.
        let leaned = items
            .iter()
            .filter(|i| !picked.iter().any(|p| p.item == i.item) && i.reasons.iter().any(|r| r.starts_with("Top players")))
            .max_by(|a, b| a.score.total_cmp(&b.score))
            .cloned();
        picked.extend(leaned);
        picked.sort_by_key(|i| i.cost);
        let mut need_text = needs.iter().filter(|n| n.0 > 0.0).map(|n| n.1.clone()).collect::<Vec<_>>().join("; ");
        let by = covered_by(*role);
        if !by.is_empty() {
            need_text = format!("{need_text}; {}", by.join("; "));
        }
        roles.push(RoleAdvice { key: key.to_string(), title: title.to_string(), need: need_text, weight: *weight, items: picked });
    }
    roles.sort_by(|a, b| b.weight.total_cmp(&a.weight));

    // The counter slots: every item scored on all the jobs it does against the enemies of the stage at once, so
    // that one item answering two problems beats two answering one each, and by how easily and how long its
    // effect lands. The second pick gets little for a job the first already does. Items in the player's own
    // build are left out.
    // What each item that blunts damage takes off each enemy's damage in a fight against the player's hero, stage by
    // stage, worked out by the calculator with everyone's usual items, levels, boons and spirit power at those souls.
    let my_build_at = |souls: u32, tiers: &[u8]| -> Option<BuildInput> {
        let hero = m.my_hero?;
        let mut spent = 0;
        let purchases = my_core
            .iter()
            .filter(|c| {
                spent += data.item(c).map(|i| i.cost).unwrap_or(0);
                spent <= souls
            })
            .cloned()
            .collect();
        Some(BuildInput { hero: hero.to_string(), purchases, souls: Some(souls), ability_tiers: tiers.to_vec(), ..Default::default() })
    };
    let measured_items: Vec<&Item> = shop.iter().copied().filter(|i| roles_of(i, shred).iter().any(|r| matches!(r.0, Role::Gun | Role::Spirit))).collect();
    let stage_cuts: Vec<HashMap<String, Vec<f64>>> = MEASURE_AT
        .iter()
        .map(|(souls, tiers)| {
            let me = my_build_at(*souls, tiers).map(|b| crate::measure::target_of(data, &b)).unwrap_or_default();
            let per_enemy: Vec<HashMap<String, f64>> = std::thread::scope(|scope| {
                let handles: Vec<_> = enemies
                    .iter()
                    .map(|e| {
                        let (me, items) = (&me, &measured_items);
                        scope.spawn(move || {
                            let b = crate::measure::build_at(e.build, *souls, tiers);
                            let Some(fight) = crate::measure::Fight::new(data, &b, me) else { return HashMap::new() };
                            items.iter().filter_map(|i| fight.cut(data, i).map(|c| (i.class_name.clone(), c))).collect()
                        })
                    })
                    .collect();
                handles.into_iter().map(|h| h.join().unwrap_or_default()).collect()
            });
            measured_items
                .iter()
                .map(|i| (i.class_name.clone(), per_enemy.iter().map(|m| m.get(&i.class_name).copied().unwrap_or(0.0)).collect()))
                .collect()
        })
        .collect();

    // Items of the player's usual build count like any other: an item the hero buys anyway can still be the best
    // answer to this team, and then it is worth buying early. They are marked as such.
    // `covered`: jobs already done, how much they are still worth, and the item doing them (an item is never held
    // against itself).
    let value = |item: &Item, jobs: &[Job], included: &[bool], covered: &[(Role, f64, String)], cuts: &HashMap<String, Vec<f64>>| -> Option<(f64, CounterPick, Vec<(Role, f64)>)> {
        let in_core = my_core.contains(&item.class_name);
        let (mut easy, mut notes) = ease(item, my_gun);
        if let Some(radius) = close_range(item).filter(|_| m.ranged) {
            easy *= 0.5;
            notes.insert(0, format!("Harder to use: works within {radius:.0}m, and this hero fights from range"));
        }
        let mut score = 0.0;
        let mut done: Vec<(f64, &str, String)> = vec![];
        let mut reliable: Vec<(Role, f64)> = vec![];
        let mut reasons: Vec<String> = vec![];
        let mut against: Vec<String> = vec![];
        // Measured: the share of each enemy's damage it takes away, counted once even when it blunts both.
        let measured = cuts.get(&item.class_name);
        let mut measured_done = false;
        for (role, strength, effect) in roles_of(item, shred) {
            let Some((_, _, title, weight, needs)) = jobs.iter().find(|j| j.0 == role) else { continue };
            let factor = covered.iter().filter(|c| c.0 == role && c.2 != item.class_name).map(|c| c.1).fold(1.0, f64::min);
            let (part, reach) = match (role, measured) {
                (Role::Gun | Role::Spirit, Some(cut)) => {
                    let on: Vec<f64> = cut.iter().zip(included).filter(|(_, on)| **on).map(|(c, _)| *c).collect();
                    let part = if measured_done { 0.0 } else { MEASURED_K * on.iter().sum::<f64>() * easy * factor };
                    measured_done = true;
                    (part, MEASURED_K * on.iter().cloned().fold(0.0, f64::max) * easy)
                }
                _ => (strength * easy * weight * factor, strength * easy),
            };
            score += part;
            done.push((part, title, effect));
            reliable.push((role, reach));
            for ((n, why), p) in needs.iter().zip(&profiles) {
                if *n > 0.0 {
                    if !reasons.contains(why) {
                        reasons.push(why.clone());
                    }
                    if !against.contains(&p.hero_name) {
                        against.push(p.hero_name.clone());
                    }
                }
            }
        }
        if done.is_empty() {
            return None;
        }
        // How much more often top players buy it against an enemy counts as far as the item answers that enemy.
        let item_roles: Vec<Role> = reliable.iter().map(|r| r.0).collect();
        for (i, ((enemy, p), on)) in enemies.iter().zip(&profiles).zip(included).enumerate() {
            let answers = jobs.iter().filter(|j| item_roles.contains(&j.0)).map(|j| j.4[i].0).fold(0.0, f64::max).min(1.0);
            if !on || answers <= 0.0 {
                continue;
            }
            if let Some(l) = enemy.lifts.iter().find(|l| l.item_id == item.id) {
                if l.bought >= LIFT_FROM {
                    score += (l.bought - 1.0) * 2.5 * easy * answers;
                    reasons.push(format!("Top players buy it {:.1}× as often against {}", l.bought, p.hero_name));
                }
                // Winning more with it against this enemy than with it in general is the item working against them.
                if l.win_pp.abs() >= WIN_SAY_PP {
                    score += l.win_pp / 100.0 * WIN_K * answers;
                    reasons.push(win_text(l.win_pp, &p.hero_name));
                }
            }
        }
        if let Some(cut) = measured.filter(|_| measured_done) {
            let mut said: Vec<(f64, String)> = cut
                .iter()
                .zip(&profiles)
                .zip(included)
                .filter(|((c, _), on)| **on && **c >= CUT_WORTH_SAYING)
                .map(|((c, p), _)| (*c, p.hero_name.clone()))
                .collect();
            said.sort_by(|a, b| b.0.total_cmp(&a.0));
            for (i, (c, name)) in said.iter().enumerate() {
                reasons.insert(i, format!("Takes {:.0}% off {name}'s damage in a fight", c * 100.0));
                if !against.contains(name) {
                    against.push(name.clone());
                }
            }
        }
        for note in notes.into_iter().rev() {
            reasons.insert(0, note);
        }
        if in_core {
            reasons.insert(0, "Already in your usual build: against this team, buy it early".into());
        }
        done.sort_by(|a, b| b.0.total_cmp(&a.0));
        let advice = CounterAdvice {
            item: item.class_name.clone(),
            name: item.name.clone(),
            icon: item.icon.clone(),
            category: item.category.clone(),
            cost: item.cost,
            active: item.active,
            description: item.description.clone(),
            effect: done.iter().map(|d| d.2.clone()).collect::<Vec<_>>().join("; "),
            score,
            reasons,
            when: String::new(),
            against,
        };
        let mut titles: Vec<String> = vec![];
        for d in &done {
            if !titles.iter().any(|t| t == d.1) {
                titles.push(d.1.to_string());
            }
        }
        Some((score, CounterPick { advice, in_core, jobs: titles }, reliable))
    };
    let best = |low: u32, high: u32, jobs: &[Job], included: &[bool], covered: &[(Role, f64, String)], skip: &[String], cuts: &HashMap<String, Vec<f64>>| -> Vec<(f64, CounterPick, Vec<(Role, f64)>)> {
        let mut all: Vec<_> = shop
            .iter()
            .filter(|i| i.cost >= low && i.cost <= high && !skip.contains(&i.class_name))
            .filter_map(|i| value(i, jobs, included, covered, cuts))
            .collect();
        all.sort_by(|a, b| b.0.total_cmp(&a.0));
        all
    };
    let job_titles: Vec<(Role, &str)> = JOBS.iter().map(|j| (j.0, j.2)).collect();
    let picks_roles = |picks: &[CounterPick]| -> Vec<(Role, f64, String)> {
        picks
            .iter()
            .flat_map(|p| p.jobs.iter().map(move |t| (t, p.advice.item.clone())))
            .filter_map(|(t, item)| job_titles.iter().find(|j| j.1 == t).map(|j| (j.0, CARRIED, item)))
            .collect()
    };
    let does = |r: &[(Role, f64)], role: Role| r.iter().any(|x| x.0 == role && x.1 >= RELIABLE);
    let lane_names: Vec<String> = profiles.iter().zip(&in_lane).filter(|(_, on)| **on).map(|(p, _)| p.hero_name.clone()).collect();
    let mut stage_jobs: Vec<Vec<Job>> = vec![];
    // What the player's own usual build already does counts for every stage, and what the items of the stage
    // before still do: they stay in the build until they are sold.
    let own: Vec<(Role, f64, String)> = my_core
        .iter()
        .filter_map(|c| data.item(c))
        .flat_map(|i| {
            let easy = ease(i, my_gun).0;
            roles_of(i, shred).into_iter().filter(move |r| r.1 * easy >= RELIABLE).map(move |r| (r.0, CARRIED, i.class_name.clone()))
        })
        .collect();
    let own_heal_cut: Vec<String> =
        my_core.iter().filter_map(|c| data.item(c)).filter(|i| reduces_healing(i) && heal_cut_of(i) > 0.0).map(|i| i.name.clone()).collect();
    if !own_heal_cut.is_empty() {
        team.push(format!("Your usual build already cuts healing with {}", own_heal_cut.join(", ")));
    }
    let mut carried: Vec<(Role, f64, String)> = vec![];
    let stages: Vec<CounterStage> = STAGES
        .iter()
        .enumerate()
        .map(|(si, (key, title, low, high, note))| {
            let cuts = &stage_cuts[si];
            let lane_only = *key == "lane" && !lane_names.is_empty();
            let included: &[bool] = if lane_only { &in_lane[..] } else { &everyone[..] };
            let jobs = jobs_for(included, *key != "lane");
            let heal_weight = jobs.iter().find(|j| j.0 == Role::Healing).map(|j| j.3).unwrap_or(0.0);
            let heaviest = jobs.iter().map(|j| j.3).fold(0.0, f64::max);
            let must_heal = heal_weight >= HEAVY_HEALING || (heal_weight > 0.0 && heal_weight >= heaviest);
            let mut picks: Vec<CounterPick> = vec![];
            let mut covered: Vec<(Role, f64, String)> = carried.iter().chain(&own).cloned().collect();
            let mut taken: Vec<String> = vec![];
            let mut healed = !own_heal_cut.is_empty();
            for slot in 0..COUNTER_SLOTS {
                let options = best(*low, *high, &jobs, included, &covered, &taken, cuts);
                // A team that heals a lot has to have its healing cut: the last slot goes to it if nothing did yet.
                let last = slot + 1 == COUNTER_SLOTS;
                let chosen = if must_heal && last && !healed { options.iter().find(|o| does(&o.2, Role::Healing)).or(options.first()) } else { options.first() };
                let Some((_, pick, done)) = chosen.cloned() else { break };
                healed |= does(&done, Role::Healing);
                covered.extend(done.iter().filter(|d| d.1 >= RELIABLE).map(|d| (d.0, COVERED, pick.advice.item.clone())));
                taken.push(pick.advice.item.clone());
                picks.push(pick);
            }
            let alternatives = best(*low, *high, &jobs, included, &[], &taken, cuts).into_iter().take(ALTERNATIVES).map(|b| b.1).collect();
            let note = if lane_only {
                format!("Against your lane: {}. The rest of their team comes later and is answered in the mid game", lane_names.join(" and "))
            } else {
                note.to_string()
            };
            carried = picks_roles(&picks);
            let scored = best(0, u32::MAX, &jobs, included, &[], &[], cuts)
                .into_iter()
                .map(|(score, p, _)| (p.advice.item.clone(), score, p.jobs.clone(), p.advice.reasons.clone()))
                .collect();
            stage_jobs.push(jobs);
            CounterStage { key: key.to_string(), title: title.to_string(), note, picks, alternatives, scored }
        })
        .collect();
    let picked: Vec<String> = stages.iter().flat_map(|s| s.picks.iter().map(|p| p.advice.item.clone())).collect();
    let others = best(0, STAGES[STAGES.len() - 1].3, &full, &everyone, &[], &picked, &stage_cuts[STAGES.len() - 1]).into_iter().take(OTHERS).map(|b| b.1).collect();

    // Hero by hero: what each enemy threatens, and which planned item answers it at which stage.
    let heroes = profiles
        .iter()
        .enumerate()
        .map(|(i, p)| {
            let threats = full.iter().filter(|j| j.4[i].0 > 0.0).map(|j| j.4[i].1.clone()).collect();
            let answers = stages
                .iter()
                .zip(&stage_jobs)
                .flat_map(|(stage, jobs)| {
                    let titles: Vec<&str> = jobs.iter().filter(|j| j.4[i].0 > 0.0).map(|j| j.2).collect();
                    stage
                        .picks
                        .iter()
                        .filter_map(|pick| {
                            let hits: Vec<&str> = pick.jobs.iter().map(String::as_str).filter(|t| titles.contains(t)).collect();
                            (!hits.is_empty()).then(|| format!("{}: {} ({})", stage.title, pick.advice.name, hits.join(", ")))
                        })
                        .collect::<Vec<_>>()
                })
                .collect();
            HeroAnswer { hero: p.hero.clone(), hero_name: p.hero_name.clone(), in_lane: in_lane[i], threats, answers }
        })
        .collect();
    CounterPlan { enemies: profiles, allies, roles, own_escape, stages, others, heroes, team }
}

/// How the counter planner reads an item, in words: the jobs it does with their strength and effect, how easily it
/// lands for a gun hero and for a spirit hero, and its range. For checking items one by one.
pub fn item_reading(item: &Item) -> String {
    let jobs: Vec<String> = roles_of(item, (1.0, 1.0)).into_iter().map(|(r, s, e)| format!("{r:?} {s:.2} ({e})")).collect();
    let (gun, gun_notes) = ease(item, Some(0.8));
    let (spirit, spirit_notes) = ease(item, Some(0.2));
    let mut notes: Vec<String> = gun_notes.into_iter().chain(spirit_notes).collect();
    notes.dedup();
    format!(
        "jobs: {} | ease gun {:.2} spirit {:.2}{} | {}",
        if jobs.is_empty() { "-".to_string() } else { jobs.join("; ") },
        gun,
        spirit,
        close_range(item).map(|r| format!(" | close range {r:.0}m")).unwrap_or_default(),
        notes.join(" / ")
    )
}
