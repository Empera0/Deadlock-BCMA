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
}

/// One row of the API's hero statistics.
#[derive(Debug, Clone, Deserialize)]
pub struct HeroStat {
    pub hero_id: u64,
    #[serde(default)]
    pub matches: u64,
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
pub fn lifts(general: &[ItemStat], against: &[ItemStat]) -> Vec<(u64, f64)> {
    let top = |rows: &[ItemStat]| rows.iter().map(|r| r.matches).max().unwrap_or(1).max(1) as f64;
    let (g_top, a_top) = (top(general), top(against));
    against
        .iter()
        .filter_map(|a| {
            let g = general.iter().find(|g| g.item_id == a.item_id)?;
            let base = g.matches as f64 / g_top;
            // Items almost nobody buys move a lot by chance.
            (base >= 0.03).then(|| (a.item_id, (a.matches as f64 / a_top) / base))
        })
        .collect()
}

/// Whether a build deals mostly spirit damage: more of its souls in spirit items than in weapon items.
pub fn deals_spirit(data: &GameData, build: &PopularBuild) -> bool {
    let souls = |category: &str| build.core.iter().filter_map(|p| data.item(&p.item)).filter(|i| i.category == category).map(|i| i.cost).sum::<u32>();
    souls("spirit") > souls("weapon")
}

/// What an enemy hero brings, read from its kit and its usual build.
#[derive(Debug, Clone, Serialize)]
pub struct EnemyProfile {
    pub hero: String,
    pub hero_name: String,
    /// Share of the usual build's weapon and spirit souls that is weapon, percent.
    pub weapon_pct: f64,
    /// Abilities and items it heals with.
    pub heals: Vec<String>,
    /// Abilities it moves, flies, turns invisible or escapes with.
    pub mobility: Vec<String>,
    /// Abilities that stun, root, sleep or silence.
    pub lockdown: Vec<String>,
    /// Resist items in its usual build.
    pub resists: Vec<String>,
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
}

/// An item for a counter slot, with every job it does against this team.
#[derive(Debug, Clone, Serialize)]
pub struct CounterPick {
    pub advice: CounterAdvice,
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
/// The team heals a lot once the healing job weighs this much (about three healers): one of the two picks
/// then has to cut healing, and do it reliably.
const HEAVY_HEALING: f64 = 1.3;
/// A pick does a job reliably when its strength times how easily it lands is at least this.
const RELIABLE: f64 = 0.55;
/// A job the first pick already does is worth this much for the second.
const COVERED: f64 = 0.35;

/// An enemy with what it builds and how much more often each item is bought against it.
pub struct Enemy<'a> {
    pub hero_id: u64,
    pub build: &'a PopularBuild,
    pub lifts: &'a [(u64, f64)],
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

#[derive(Clone, Copy, PartialEq)]
enum Role {
    Healing,
    Gun,
    Spirit,
    Mobility,
    Control,
    Shred,
}

/// How easily an item's effect lands for the player's hero (0 to 1), and why when it does not land on its own.
/// A headshot or a burst of spirit damage is harder to bring about than a bullet hit or a cast.
fn ease(item: &Item, my_spirit: Option<bool>) -> (f64, Option<String>) {
    let d = item.description.to_lowercase();
    let caster = my_spirit == Some(true);
    let gunner = my_spirit == Some(false);
    if d.contains("headshot") {
        (0.5, Some("only works on headshots".into()))
    } else if d.contains("dealing significant spirit damage") {
        if gunner { (0.45, Some("needs bursts of spirit damage, which this hero rarely deals".into())) } else { (0.9, None) }
    } else if d.contains("your bullets") {
        if caster { (0.65, Some("works through bullets, and this hero mostly casts".into())) } else if d.contains("chance") { (0.8, None) } else { (1.0, None) }
    } else if d.contains("your spirit damage") {
        if gunner { (0.8, Some("works through spirit damage, and this hero mostly shoots".into())) } else { (1.0, None) }
    } else if item.active {
        (0.85, None)
    } else {
        (1.0, None)
    }
}

/// What an item does for each job, with how strongly (about 1 for an item made for the job).
fn roles_of(item: &Item, my_spirit: Option<bool>) -> Vec<(Role, f64, String)> {
    let mut out = vec![];
    let heal_cut = heal_cut_of(item);
    if reduces_healing(item) && heal_cut > 0.0 {
        out.push((Role::Healing, heal_cut / 45.0, format!("-{heal_cut:.0}% healing on whoever it hits")));
    }
    let fire_slow = prop(item, "FireRateSlow");
    let reflect = prop(item, "BulletDamageReflectedPct");
    // "Disarms enemy target" is a disarm; "immune to ... Disarm" (Unstoppable) is not.
    let disarm = item.description.contains("Disarms");
    let bullet = resist_of(item, false);
    if fire_slow > 0.0 {
        out.push((Role::Gun, fire_slow / 25.0, format!("-{fire_slow:.0}% fire rate on enemies it hits")));
    } else if disarm {
        out.push((Role::Gun, 1.2, "disarms: the enemy cannot shoot for a while".into()));
    } else if reflect > 0.0 {
        out.push((Role::Gun, reflect / 70.0, format!("reflects {reflect:.0}% of the bullet damage taken")));
    } else if bullet >= 10.0 {
        out.push((Role::Gun, bullet / 22.0, format!("{bullet:.0}% bullet resist")));
    }
    let sap = prop(item, "TechPowerReduction");
    let spirit_cut = prop(item, "TechDamageReduction");
    let proc_cut = prop(item, "SpiritDamageReductionProc");
    let spirit = resist_of(item, true);
    if sap > 0.0 {
        out.push((Role::Spirit, sap / 25.0, format!("-{sap:.0} spirit power on the target")));
    } else if spirit_cut > 0.0 {
        out.push((Role::Spirit, spirit_cut / 22.0, format!("-{spirit_cut:.0}% spirit damage dealt by the target")));
    } else if proc_cut > 0.0 {
        out.push((Role::Spirit, proc_cut / 60.0, format!("cuts the next big spirit hit by {proc_cut:.0}%")));
    } else if spirit >= 10.0 {
        out.push((Role::Spirit, spirit / 22.0, format!("{spirit:.0}% spirit resist")));
    }
    let dash = prop(item, "GroundDashReductionPercent");
    let slow = prop(item, "SlowPercent").max(prop(item, "MovementSpeedSlow"));
    let flight = counters::answers(item).contains(&Threat::Flight);
    // A dash penalty only pins enemies down when it comes with a slow on them: on Metal Skin it is the
    // owner's own penalty while the item is active.
    if dash > 0.0 && slow > 0.0 {
        out.push((Role::Mobility, (dash + slow) / 40.0, format!("-{dash:.0}% dash distance{}", if slow > 0.0 { format!(" and {slow:.0}% slow") } else { String::new() })));
    } else if flight {
        out.push((Role::Mobility, 1.0, "brings flying heroes down".into()));
    } else if slow >= 20.0 && item.category != "vitality" {
        out.push((Role::Mobility, slow / 50.0, format!("{slow:.0}% slow")));
    }
    let answers = counters::answers(item);
    if answers.contains(&Threat::Lockdown) || answers.contains(&Threat::Silence) {
        out.push((Role::Control, 1.0, "breaks, shortens or blocks stuns and silences".into()));
    }
    for spirit_damage in [false, true] {
        if my_spirit.is_some_and(|k| k != spirit_damage) {
            continue;
        }
        let shred = shred_of(item, spirit_damage);
        let shreds = if spirit_damage { shreds_spirit(item) } else { shreds_bullet(item) };
        if shreds && shred > 0.0 {
            out.push((Role::Shred, shred / 10.0, format!("-{shred:.0}% {} resist on the target", if spirit_damage { "spirit" } else { "bullet" })));
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
                    Threat::Flight if !mobility.contains(&found.ability) => mobility.push(found.ability.clone()),
                    _ => {}
                }
            }
        }
    }
    EnemyProfile {
        hero: enemy.build.hero.clone(),
        hero_name: enemy.build.hero_name.clone(),
        weapon_pct: if weapon + spirit > 0.0 { weapon / (weapon + spirit) * 100.0 } else { 50.0 },
        heals: heals_with,
        mobility,
        lockdown,
        resists: core.iter().filter(|i| gives_resist(i, false) || gives_resist(i, true)).map(|i| i.name.clone()).collect(),
    }
}

/// How much one enemy calls for a job, 0 to about 1, and a few words on why.
fn need(role: Role, p: &EnemyProfile, own_escape: bool) -> (f64, String) {
    match role {
        Role::Healing if !p.heals.is_empty() => ((p.heals.len() as f64 * 0.45).min(1.2), format!("{} heals with {}", p.hero_name, p.heals.join(", "))),
        // Most heroes mix the two; each side counts from 40% of the build up.
        Role::Gun if p.weapon_pct >= 40.0 => ((p.weapon_pct - 30.0) / 60.0, format!("{} builds {:.0}% weapon items", p.hero_name, p.weapon_pct)),
        Role::Spirit if p.weapon_pct <= 60.0 => ((70.0 - p.weapon_pct) / 60.0, format!("{} builds {:.0}% spirit items", p.hero_name, 100.0 - p.weapon_pct)),
        Role::Mobility if !p.mobility.is_empty() => ((p.mobility.len() as f64 * 0.5).min(1.0), format!("{} gets around with {}", p.hero_name, p.mobility.join(", "))),
        Role::Control if !p.lockdown.is_empty() => {
            let base = (p.lockdown.len() as f64 * 0.35).min(0.9);
            (if own_escape { base * 0.5 } else { base }, format!("{} locks down with {}", p.hero_name, p.lockdown.join(", ")))
        }
        Role::Shred if !p.resists.is_empty() => (0.6, format!("{} builds {}", p.hero_name, p.resists.join(", "))),
        _ => (0.0, String::new()),
    }
}

/// Plans the items to buy against a team of enemies, job by job: cutting their healing, their gun, their spirit,
/// their mobility, answering their crowd control and getting through their resists. Each job lists the items that
/// do it best against this team, weighed by how much each enemy calls for it and by how much more often top
/// players buy the item against those enemies. `my_spirit` says whether the player's own hero deals mostly spirit
/// damage (for resist reduction) and `my_hero` is checked for ways out of crowd control.
/// `my_core` (item class names) is the player's own usual build: those items are not suggested again.
pub fn counter_plan(data: &GameData, my_hero: Option<&str>, my_spirit: Option<bool>, my_core: &[String], enemies: &[Enemy]) -> CounterPlan {
    let profiles: Vec<EnemyProfile> = enemies.iter().map(|e| profile(data, e)).collect();
    let own_escape: Vec<String> = my_hero
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
    let shop: Vec<&Item> = data.items.iter().filter(|i| i.cost > 0 && i.tier <= 4).collect();
    let jobs = [
        (Role::Healing, "healing", "Cut their healing"),
        (Role::Gun, "gun", "Blunt their guns"),
        (Role::Spirit, "spirit", "Blunt their spirit damage"),
        (Role::Mobility, "mobility", "Pin down their movement"),
        (Role::Control, "control", "Answer their crowd control"),
        (Role::Shred, "shred", "Get through their resists"),
    ];
    let mut roles = vec![];
    let mut jobs_needed: Vec<(Role, &str, f64, Vec<(f64, String)>)> = vec![];
    for (role, key, title) in jobs {
        let needs: Vec<(f64, String)> = profiles.iter().map(|p| need(role, p, escape)).collect();
        let weight: f64 = needs.iter().map(|n| n.0).sum();
        if weight <= 0.0 {
            continue;
        }
        jobs_needed.push((role, title, weight, needs.clone()));
        let mut items: Vec<CounterAdvice> = shop
            .iter()
            .filter_map(|item| {
                let (_, strength, effect) = roles_of(item, my_spirit).into_iter().find(|r| r.0 == role)?;
                let mut score = strength * weight;
                let mut reasons: Vec<String> = vec![];
                let mut against: Vec<String> = vec![];
                for ((enemy, profile), (n, why)) in enemies.iter().zip(&profiles).zip(&needs) {
                    if *n <= 0.0 {
                        continue;
                    }
                    against.push(profile.hero_name.clone());
                    reasons.push(why.clone());
                    if let Some((_, lift)) = enemy.lifts.iter().find(|l| l.0 == item.id).filter(|l| l.1 >= LIFT_FROM) {
                        score += (lift - 1.0) * 2.5 * n.max(0.3);
                        reasons.push(format!("Top players buy it {lift:.1}× as often against {}", profile.hero_name));
                    }
                }
                // When: as soon as the enemy usually has what it answers.
                let when = profiles
                    .iter()
                    .zip(enemies)
                    .zip(&needs)
                    .filter(|(_, n)| n.0 > 0.0)
                    .flat_map(|((p, e), _)| {
                        e.build.core.iter().filter(|c| match role {
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
        // The best item at each price: one for the lane, one for the mid game, one for late. A cheap answer
        // bought early often matters more than a stronger one bought too late.
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
        let items = picked;
        let need_text = needs.iter().filter(|n| n.0 > 0.0).map(|n| n.1.clone()).collect::<Vec<_>>().join("; ");
        roles.push(RoleAdvice { key: key.into(), title: title.into(), need: need_text, weight, items });
    }
    roles.sort_by(|a, b| b.weight.total_cmp(&a.weight));

    // The counter slots, stage by stage: every item scored on all the jobs it does against this team at once,
    // so that one item answering two problems beats two answering one each, and by how easily its effect lands.
    // The second pick gets little for a job the first already does. Items in the player's own build are left out.
    let heal_weight = jobs_needed.iter().find(|j| j.0 == Role::Healing).map(|j| j.2).unwrap_or(0.0);
    let heaviest = jobs_needed.iter().map(|j| j.2).fold(0.0, f64::max);
    let must_heal = heal_weight >= HEAVY_HEALING || (heal_weight > 0.0 && heal_weight >= heaviest);
    let value = |item: &Item, covered: &[Role]| -> Option<(f64, CounterPick, Vec<(Role, f64)>)> {
        if my_core.contains(&item.class_name) {
            return None;
        }
        let (easy, hard) = ease(item, my_spirit);
        let mut score = 0.0;
        let mut done: Vec<(f64, &str, String)> = vec![];
        let mut reliable: Vec<(Role, f64)> = vec![];
        let mut reasons: Vec<String> = vec![];
        let mut against: Vec<String> = vec![];
        for (role, strength, effect) in roles_of(item, my_spirit) {
            let Some((_, title, weight, needs)) = jobs_needed.iter().find(|j| j.0 == role) else { continue };
            let part = strength * easy * weight * if covered.contains(&role) { COVERED } else { 1.0 };
            score += part;
            done.push((part, title, effect));
            reliable.push((role, strength * easy));
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
        for (enemy, p) in enemies.iter().zip(&profiles) {
            if let Some((_, lift)) = enemy.lifts.iter().find(|l| l.0 == item.id).filter(|l| l.1 >= LIFT_FROM) {
                score += (lift - 1.0) * 2.5 * easy;
                reasons.push(format!("Top players buy it {lift:.1}× as often against {}", p.hero_name));
            }
        }
        if let Some(hard) = hard {
            reasons.insert(0, format!("Harder to use: {hard}"));
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
        Some((score, CounterPick { advice, jobs: done.iter().map(|d| d.1.to_string()).collect() }, reliable))
    };
    let best = |low: u32, high: u32, covered: &[Role], skip: &[String]| -> Vec<(f64, CounterPick, Vec<(Role, f64)>)> {
        let mut all: Vec<_> = shop
            .iter()
            .filter(|i| i.cost >= low && i.cost <= high && !skip.contains(&i.class_name))
            .filter_map(|i| value(i, covered))
            .collect();
        all.sort_by(|a, b| b.0.total_cmp(&a.0));
        all
    };
    let does = |r: &[(Role, f64)], role: Role| r.iter().any(|x| x.0 == role && x.1 >= RELIABLE);
    let stages: Vec<CounterStage> = STAGES
        .iter()
        .map(|(key, title, low, high, note)| {
            let mut picks: Vec<CounterPick> = vec![];
            let mut covered: Vec<Role> = vec![];
            let mut taken: Vec<String> = vec![];
            let mut healed = false;
            for slot in 0..COUNTER_SLOTS {
                let options = best(*low, *high, &covered, &taken);
                // A team that heals a lot has to have its healing cut: the last slot goes to it if nothing did yet.
                let last = slot + 1 == COUNTER_SLOTS;
                let chosen = if must_heal && last && !healed {
                    options.iter().find(|o| does(&o.2, Role::Healing)).or(options.first())
                } else {
                    options.first()
                };
                let Some((_, pick, done)) = chosen.cloned() else { break };
                healed |= does(&done, Role::Healing);
                covered.extend(done.iter().map(|d| d.0));
                taken.push(pick.advice.item.clone());
                picks.push(pick);
            }
            let alternatives = best(*low, *high, &[], &taken).into_iter().take(ALTERNATIVES).map(|b| b.1).collect();
            CounterStage { key: key.to_string(), title: title.to_string(), note: note.to_string(), picks, alternatives }
        })
        .collect();
    let picked: Vec<String> = stages.iter().flat_map(|s| s.picks.iter().map(|p| p.advice.item.clone())).collect();
    let others = best(0, STAGES[STAGES.len() - 1].3, &[], &picked).into_iter().take(OTHERS).map(|b| b.1).collect();
    CounterPlan { enemies: profiles, roles, own_escape, stages, others }
}
