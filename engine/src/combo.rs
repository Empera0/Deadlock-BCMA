//! Combo simulation: plays a sequence of actions against the target and totals the damage.
//!
//! Chance-based procs are deterministic: each shot adds its chance to a counter and the proc
//! fires whenever the counter reaches 1, so the result is the expected damage, not a dice roll.
//!
//! Debuffs (resist shred, damage amp) and self buffs are applied while the combo runs, so the
//! order of actions matters: a hit only benefits from effects that landed before it.

use crate::build::BuildInput;
use crate::calc::AbilitySheet;
use crate::calc::{self, ScaledProp, Sheet};
use crate::data::GameData;
use crate::kit::{self, n, Buff, Context, Effect, Heal, Health, Part, Passive, Plan};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

pub use crate::kit::Kind;

const STEP: f64 = 0.05;
const LIGHT_MELEE_TIME: f64 = 0.6;
const HEAVY_MELEE_TIME: f64 = 1.0;
const BUILDUP_DECAY_DELAY: f64 = 2.0;
const BUILDUP_DECAY_RATE: f64 = 0.2;

#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Action {
    Shoot { seconds: f64 },
    Melee { heavy: bool },
    Ability {
        index: usize,
        /// Overrides how long the ability stays on the target.
        #[serde(default)]
        duration: Option<f64>,
        /// Overrides how many times a multi-hit ability connects.
        #[serde(default)]
        hits: Option<f64>,
    },
    Item { item: String },
    /// Switches a conditional bonus on from this point of the combo.
    Condition {
        item: String,
        /// Stacks or kills for a counted condition.
        #[serde(default)]
        count: Option<f64>,
    },
    Wait { seconds: f64 },
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct ComboInput {
    pub actions: Vec<Action>,
    /// Share of shots that hit the head, 0-100.
    pub headshot_rate: f64,
    /// The hero's own health when the combo starts, 0-100 % of max. Missing-health heals depend on it.
    pub own_health: Option<f64>,
    /// An enemy hero with its items as the target. Its health and resists replace the build's target.
    pub enemy: Option<BuildInput>,
    /// Where the fight happens: map objects that change the target's resists.
    pub zone: Zone,
}

/// Map objects around the fight.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct Zone {
    /// The fight is inside the Unstable Rift; the comeback bonus follows from the minute and the teams' souls.
    pub rift: bool,
    pub minute: f64,
    pub own_souls: f64,
    pub enemy_souls: f64,
    /// The target is near its own Walker and gets its resist aura.
    pub enemy_walker: bool,
    /// The target is near the hero's Walker, whose fireballs reduce its resists.
    pub own_walker: bool,
}

/// The target the combo runs against, after the enemy's build and the zone.
#[derive(Debug, Clone, Serialize)]
pub struct TargetInfo {
    pub health: f64,
    pub bullet_resist: f64,
    pub spirit_resist: f64,
    /// Name of the enemy hero, when the target is one.
    pub hero: Option<String>,
    /// How each part of the target came about.
    pub notes: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct Event {
    pub time: f64,
    pub source: String,
    pub detail: String,
    /// Damage after the target's resists; `None` when it is not a single known amount.
    pub damage: Option<f64>,
    /// Health restored to the hero by this event.
    pub heal: Option<f64>,
}

#[derive(Debug, Clone, Serialize)]
pub struct HealTotal {
    pub source: String,
    pub amount: f64,
}

#[derive(Debug, Clone, Serialize)]
pub struct SourceTotal {
    pub source: String,
    pub kind: Kind,
    pub damage: f64,
}

#[derive(Debug, Clone, Serialize)]
pub struct UsableItem {
    pub item: String,
    pub name: String,
    pub description: String,
    /// True for a condition the player switches on, false for an item that is used.
    pub condition: bool,
    /// Set when the condition takes a count: the highest count that still adds something.
    pub max_count: Option<f64>,
    /// What is counted, e.g. `stacks` or `kills`.
    pub unit: String,
}

/// How the simulation reads one of the hero's abilities.
#[derive(Debug, Clone, Serialize)]
pub struct AbilityInfo {
    pub name: String,
    pub castable: bool,
    pub lines: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ComboResult {
    pub total: f64,
    /// Time until the last action finishes.
    pub duration: f64,
    /// Time until the last damage-over-time effect runs out.
    pub full_duration: f64,
    pub dps: f64,
    pub target_health: f64,
    pub target: TargetInfo,
    pub kill_time: Option<f64>,
    pub sources: Vec<SourceTotal>,
    pub events: Vec<Event>,
    pub abilities: Vec<AbilityInfo>,
    /// Passive item effects that take part in the simulation.
    pub passives: Vec<String>,
    /// Owned items that can be used as a combo action.
    pub usable_items: Vec<UsableItem>,
    /// Owned items whose damage effect is not simulated.
    pub unsupported: Vec<String>,
    /// Health the hero restores to themselves over the combo, before any cap at max health.
    pub healing: f64,
    pub heal_sources: Vec<HealTotal>,
    pub own_max_health: f64,
    pub own_health_start: f64,
    /// Starting health plus the healing, capped at max health.
    pub own_health_end: f64,
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum Trigger {
    Headshot,
    Melee,
    AbilityHit,
    Bullet { chance: f64 },
    /// Each shot adds this fraction; the effect applies at a full bar.
    Buildup { per_shot: f64 },
    Periodic { interval: f64 },
    SpiritThreshold { threshold: f64 },
    Ultimate { delay: f64 },
}

#[derive(Debug, Clone)]
struct Proc {
    item: String,
    trigger: Trigger,
    kind: Kind,
    damage: f64,
    /// Extra damage as a percentage of the target's current health.
    pct_current: f64,
    dot: Option<(f64, f64)>,
    /// Debuff applied together with the proc: (effect, seconds).
    debuff: Option<(Effect, f64)>,
    /// Grows the damage with the target's missing health: (maximum, health % for the maximum).
    ramp: Option<(f64, f64)>,
    /// Self buff granted together with the proc: (buff, seconds).
    buff: Option<(Buff, f64)>,
    /// Imbue items only react to the ability they are imbued into.
    only_ability: Option<usize>,
    /// Multipliers for on-melee procs triggered by a light melee instead of a heavy one.
    light_damage: f64,
    light_cooldown: f64,
    /// While the proc's damage-over-time is running, each shot extends it by this much...
    extend_per_shot: f64,
    /// ...or by this much on a headshot...
    extend_per_headshot: f64,
    /// ...but never beyond this many seconds from now.
    max_seconds: f64,
    cooldown: f64,
    reloads: bool,
    ready_at: f64,
    counter: f64,
    last_stack: f64,
    /// Percent of the target's remaining max health stolen: dealt as damage and gained as health.
    siphon: f64,
}

impl Proc {
    fn new(item: &str, trigger: Trigger, kind: Kind, damage: f64, cooldown: f64) -> Proc {
        Proc {
            item: item.to_string(),
            trigger,
            kind,
            damage,
            pct_current: 0.0,
            dot: None,
            debuff: None,
            ramp: None,
            buff: None,
            only_ability: None,
            light_damage: 1.0,
            light_cooldown: 1.0,
            extend_per_shot: 0.0,
            extend_per_headshot: 0.0,
            max_seconds: 0.0,
            cooldown,
            reloads: false,
            ready_at: 0.0,
            counter: 0.0,
            last_stack: 0.0,
            siphon: 0.0,
        }
    }
}

#[derive(Debug, Clone)]
struct Active {
    item: String,
    name: String,
    damage: f64,
    dot_dps: f64,
    dot_pct_current: f64,
    dot_seconds: f64,
    cooldown: f64,
    ready_at: f64,
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum On {
    SpiritDamage,
    Bullet,
    Headshot,
    Melee,
    Use,
    Ultimate,
    AbilityCast,
    Kill,
    /// Any damage; used with a damage threshold.
    Damage,
}

/// A debuff and the event that applies it.
#[derive(Debug, Clone)]
struct Rule {
    item: String,
    source: String,
    on: On,
    effect: Effect,
    seconds: f64,
    max_stacks: u32,
    cooldown: f64,
    ready_at: f64,
    counter: f64,
}

struct Debuff {
    source: String,
    per_stack: Effect,
    stacks: u32,
    end: f64,
}

struct ActiveBuff {
    source: String,
    buff: Buff,
    end: f64,
}

/// A self buff that gains stacks from shots, e.g. Haze's Fixation.
struct StackBuff {
    source: String,
    proc_every: f64,
    proc_raw: f64,
    /// Stacks gained since the last proc.
    gained: f64,
    per_stack: Buff,
    stacks: f64,
    max_stacks: f64,
    seconds: f64,
    per_shot: f64,
    per_headshot: f64,
    end: f64,
}

fn val(props: &[ScaledProp], key: &str) -> Option<f64> {
    props.iter().find(|p| p.key == key).map(|p| p.value)
}

/// Turns the owned items' effect values into simulated procs and usable actives.
fn item_effects(
    sheet: &Sheet,
    target_health: f64,
    imbues: &BTreeMap<String, usize>,
) -> (Vec<Proc>, Vec<Active>, Vec<String>, Vec<String>) {
    let (mut procs, mut actives, mut passives, mut unsupported) = (vec![], vec![], vec![], vec![]);
    for e in &sheet.item_effects {
        let v = |key: &str| val(&e.props, key);
        let proc = |trigger: Trigger, kind: Kind, damage: f64, cooldown: f64| Proc::new(&e.name, trigger, kind, damage, cooldown);
        let active = |damage: f64, dot_dps: f64, dot_pct_current: f64| Active {
            item: e.item.clone(),
            name: e.name.clone(),
            damage,
            dot_dps,
            dot_pct_current,
            dot_seconds: v("AbilityDuration").unwrap_or(0.0),
            cooldown: v("AbilityCooldown").unwrap_or(0.0),
            ready_at: 0.0,
        };
        let cd = v("AbilityCooldown").unwrap_or(0.0);
        let charge = v("AbilityChargeUpTime").unwrap_or(0.0);
        let proc_cd = v("ProcCooldown").unwrap_or(0.0);
        match e.name.as_str() {
            "Headshot Booster" | "Headhunter" => {
                if let Some(d) = v("HeadShotBonusDamage") {
                    passives.push(format!("{}: +{} bullet damage on a headshot, {}s cooldown", e.name, n(d), n(cd)));
                    procs.push(proc(Trigger::Headshot, Kind::Bullet, d, cd));
                }
            }
            "Spirit Strike" | "Spirit Snatch" => {
                if let Some(d) = v("SpiritDamage") {
                    let mut p = proc(Trigger::Melee, Kind::Spirit, d, cd);
                    // Light melee hits get the weaker version the item text describes.
                    let light = if e.name == "Spirit Strike" {
                        p.light_cooldown = 2.0;
                        "cooldown is twice as long after a light melee"
                    } else {
                        p.light_damage = 0.7;
                        "30% less on a light melee"
                    };
                    passives.push(format!("{}: +{} spirit damage on a melee hit, {}s cooldown ({light})", e.name, n(d), n(cd)));
                    procs.push(p);
                }
            }
            "Mystic Burst" | "Tankbuster" | "Quicksilver Reload" | "Mercurial Magnum" => {
                if let Some(d) = v("Damage") {
                    let mut p = proc(Trigger::AbilityHit, Kind::Spirit, d, charge);
                    p.pct_current = v("CurrentHealthDamage").unwrap_or(0.0);
                    p.reloads = v("AmmoReloadPercent").is_some();
                    if p.reloads {
                        // The reload items are imbued: they only react to their own ability.
                        match imbues.get(&e.item) {
                            Some(index) => p.only_ability = Some(*index),
                            None => continue,
                        }
                    }
                    let extra = if p.pct_current > 0.0 {
                        format!(" + {}% of current health", n(p.pct_current))
                    } else if p.reloads {
                        " and a full reload".to_string()
                    } else {
                        String::new()
                    };
                    passives.push(format!(
                        "{}: +{} spirit damage{extra} when an ability hits, {}s cooldown",
                        e.name,
                        n(d),
                        n(charge)
                    ));
                    procs.push(p);
                }
            }
            "Tesla Bullets" | "Capacitor" => {
                if let (Some(d), Some(chance)) = (v("DamagePerChain"), v("ProcChance")) {
                    passives.push(format!(
                        "{}: {}% chance per shot for {} spirit damage (single target)",
                        e.name,
                        n(chance),
                        n(d)
                    ));
                    procs.push(proc(Trigger::Bullet { chance: chance / 100.0 }, Kind::Spirit, d, proc_cd));
                }
                if e.name == "Capacitor" {
                    if let Some(d) = v("Damage") {
                        actives.push(active(d, 0.0, 0.0));
                    }
                }
            }
            "Mystic Shot" => {
                if let Some(d) = v("ProcBonusMagicDamage") {
                    let cooldown = if cd > 0.0 { cd } else { proc_cd };
                    passives.push(format!("{}: +{} spirit damage on a shot, {}s cooldown", e.name, n(d), n(cooldown)));
                    procs.push(proc(Trigger::Bullet { chance: 1.0 }, Kind::Spirit, d, cooldown));
                }
            }
            "Toxic Bullets" => {
                if let (Some(pct), Some(per_shot)) = (v("DotHealthPercent"), v("BuildUpPerShot")) {
                    // The bleed lasts longer with ability duration bonuses.
                    let seconds = v("DotDuration").unwrap_or(0.0) * (1.0 + sheet.stat("ability_duration").unwrap_or(0.0) / 100.0);
                    let shots = (100.0 / per_shot - 1e-9).ceil();
                    let mut p = proc(Trigger::Buildup { per_shot: per_shot / 100.0 }, Kind::Spirit, 0.0, 0.0);
                    // The bleed is a share of the target's max health, so it is a fixed DPS per target.
                    p.dot = Some((pct / 100.0 * target_health, seconds));
                    passives.push(format!(
                        "{}: {}% buildup per shot ({} shots to apply), then {}% of max health per second for {}s",
                        e.name,
                        n(per_shot),
                        n(shots),
                        n(pct),
                        n(seconds)
                    ));
                    procs.push(p);
                }
            }
            "Torment Pulse" => {
                if let Some(d) = v("DamagePulseAmount") {
                    passives.push(format!("{}: {} spirit damage every {}s", e.name, n(d), n(cd)));
                    procs.push(proc(Trigger::Periodic { interval: cd.max(STEP) }, Kind::Spirit, d, 0.0));
                }
            }
            "Spirit Burn" => {
                if let (Some(threshold), Some(dps)) = (v("DamageThreshold"), v("DPS")) {
                    let seconds = v("DebuffDuration").unwrap_or(0.0);
                    let mut p = proc(
                        Trigger::SpiritThreshold { threshold },
                        Kind::Spirit,
                        v("ExplosionDamage").unwrap_or(0.0),
                        v("ImmunityDuration").unwrap_or(0.0),
                    );
                    p.dot = Some((dps, seconds));
                    passives.push(format!(
                        "{}: after {} spirit damage, {} burst + {} DPS for {}s",
                        e.name,
                        n(threshold),
                        n(p.damage),
                        n(dps),
                        n(seconds)
                    ));
                    procs.push(p);
                }
            }
            "Lightning Scroll" => {
                if let Some(d) = v("Damage") {
                    let delay = v("DelayBeforeStun").unwrap_or(0.0);
                    passives.push(format!("{}: {} spirit damage {}s after the ultimate", e.name, n(d), n(delay)));
                    procs.push(proc(Trigger::Ultimate { delay }, Kind::Spirit, d, 0.0));
                }
            }
            "Siphon Bullets" => {
                if let Some(pct) = v("HealthStealPctHero") {
                    let mut p = proc(Trigger::Bullet { chance: 1.0 }, Kind::Pure, 0.0, proc_cd);
                    p.siphon = pct;
                    passives.push(format!(
                        "{}: a bullet steals {}% of the target's remaining max health as damage that ignores resists and heals you for it, {}s cooldown",
                        e.name,
                        n(pct),
                        n(proc_cd)
                    ));
                    procs.push(p);
                }
            }
            "Mystic Reverb" => passives.push(
                "Mystic Reverb: its echo damages enemies around the target, not the target itself, so it adds nothing against a single target".into(),
            ),
            "Frostbite Charm" => {
                if let (Some(d), Some(index)) = (v("Damage"), imbues.get(&e.item)) {
                    let mut p = proc(Trigger::AbilityHit, Kind::Spirit, d, cd);
                    p.only_ability = Some(*index);
                    passives.push(format!("{}: +{} damage when the imbued ability deals damage, {}s cooldown", e.name, n(d), n(cd)));
                    procs.push(p);
                }
            }
            "Cold Front" | "Silence Wave" | "Arctic Blast" | "Nullification Burst" => {
                if let Some(d) = v("Damage") {
                    actives.push(active(d, 0.0, 0.0));
                }
            }
            "Phantom Strike" => {
                if let Some(d) = v("ImpactDamage") {
                    actives.push(active(d, 0.0, 0.0));
                }
            }
            "Alchemical Fire" => {
                if let (Some(low), Some(high)) = (v("DPS"), v("DPSMax")) {
                    actives.push(active(0.0, (low + high) / 2.0, 0.0));
                }
            }
            "Prism Blast" | "Shadow Strike" => {
                if let Some(dps) = v("DPS") {
                    actives.push(active(0.0, dps, 0.0));
                }
            }
            "Decay" => {
                if let Some(pct) = v("DotHealthPercent") {
                    actives.push(active(0.0, 0.0, pct));
                }
            }
            // Usable only for the debuff they apply.
            "Rusted Barrel" | "Disarming Hex" | "Spirit Sap" | "Focus Lens" | "Shadow Weave" | "Infuser" | "Ethereal Shift"
            | "Unstable Concoction" | "Echo Shard" => actives.push(active(0.0, 0.0, 0.0)),
            _ => {
                if e.props.iter().any(|p| p.key.contains("Damage") || p.key.contains("DPS")) {
                    unsupported.push(e.name.clone());
                }
            }
        }
    }
    (procs, actives, passives, unsupported)
}

/// Debuffs the owned items apply to the target, with the event that applies each.
fn debuff_rules(data: &GameData, sheet: &Sheet) -> (Vec<Rule>, Vec<String>) {
    let (mut rules, mut passives) = (vec![], vec![]);
    for class in &sheet.owned {
        let Some(item) = data.item(class) else { continue };
        let v = |key: &str| item.props.iter().find(|p| p.key == key).map(|p| p.value.abs()).unwrap_or(0.0);
        let mut add = |source: &str, on: On, effect: Effect, seconds: f64, max_stacks: f64, cooldown: f64| {
            if effect.is_empty() {
                return;
            }
            let when = match on {
                On::SpiritDamage => "when you deal spirit damage",
                On::Bullet => "on a bullet hit",
                On::Headshot => "on a headshot",
                On::Melee => "on a melee hit",
                On::Use => "when used",
                On::Ultimate => "when you cast your ultimate",
                On::AbilityCast => "when the ability is used",
                On::Kill => "when the target dies",
                On::Damage => "when you deal damage",
            };
            let stacks = if max_stacks > 1.0 { format!(", stacks up to {}", n(max_stacks)) } else { String::new() };
            passives.push(format!("{source}: {} for {}s {when}{stacks}", effect.text(), n(seconds)));
            rules.push(Rule {
                item: item.name.clone(),
                source: source.to_string(),
                on,
                effect,
                seconds,
                max_stacks: max_stacks.max(1.0) as u32,
                cooldown,
                ready_at: 0.0,
                counter: 0.0,
            });
        };
        let bullet = |x: f64| Effect { bullet: x, ..Default::default() };
        let spirit = |x: f64| Effect { spirit: x, ..Default::default() };
        let name = item.name.as_str();
        match name {
            "Bullet Resist Shredder" => add(name, On::SpiritDamage, bullet(v("BulletArmorReduction")), v("AbilityDuration"), 1.0, 0.0),
            "Mystic Vulnerability" => add(name, On::SpiritDamage, spirit(v("TechArmorDamageReduction")), v("AbilityDuration"), 1.0, 0.0),
            "Escalating Exposure" => {
                add("Escalating Exposure (resist)", On::SpiritDamage, spirit(v("TechArmorDamageReduction")), v("AbilityDuration"), 1.0, 0.0);
                add(
                    name,
                    On::SpiritDamage,
                    Effect { spirit_amp: v("MagicIncreasePerStack"), ..Default::default() },
                    v("AbilityDuration"),
                    v("MaxStacks"),
                    v("ProcCooldown"),
                );
            }
            "Spirit Shredder" => add(name, On::Bullet, spirit(v("TechArmorDamageReduction")), v("DebuffDuration"), 1.0, 0.0),
            "Spirit Rend" => {
                add(name, On::Bullet, spirit(v("TechArmorDamageReduction")), v("DebuffDuration"), 1.0, 0.0);
                add(
                    "Spirit Rend (headshots)",
                    On::Headshot,
                    spirit(v("MagicResistReduction")),
                    v("DebuffDuration"),
                    v("MaxStacks"),
                    v("ProcCooldown"),
                );
            }
            "Hollow Point" => add(name, On::Bullet, bullet(v("BulletArmorReduction")), v("DebuffDuration"), 1.0, 0.0),
            "Stalker" => add(name, On::Bullet, bullet(v("BulletResistReduction")), v("DebuffDuration"), 1.0, v("AbilityCooldown")),
            "Weakening Headshot" => add(name, On::Headshot, bullet(v("BulletResistReduction")), v("DebuffDuration"), 1.0, 0.0),
            "Crippling Headshot" => add(
                name,
                On::Headshot,
                Effect { bullet: v("BulletResistReduction"), spirit: v("MagicResistReduction"), ..Default::default() },
                v("DebuffDuration"),
                1.0,
                0.0,
            ),
            "Spirit Strike" | "Spirit Snatch" => {
                add(name, On::Melee, spirit(v("TechArmorDamageReduction")), v("AbilityDuration"), 1.0, v("AbilityCooldown"))
            }
            "Crushing Fists" => add(name, On::Melee, bullet(v("BulletResistReduction")), v("DebuffDuration"), v("MaxStacks"), 0.0),
            "Rusted Barrel" | "Disarming Hex" | "Alchemical Fire" => {
                add(name, On::Use, bullet(v("BulletArmorReduction")), v("AbilityDuration"), 1.0, 0.0)
            }
            "Spirit Sap" => add(name, On::Use, spirit(v("MagicResistReduction")), v("AbilityDuration"), 1.0, 0.0),
            "Focus Lens" => add(name, On::Use, spirit(v("MagicResistReduction")), v("ResistReductionDuration"), 1.0, 0.0),
            _ => {}
        }
    }
    (rules, passives)
}

/// Something a hero does to an enemy that makes the enemy take more damage.
#[derive(Debug, Clone)]
pub struct TargetDebuff {
    pub source: String,
    pub effect: Effect,
    pub seconds: f64,
    pub max_stacks: u32,
    /// False when only the owner's own damage gains from it.
    pub team: bool,
    /// Share of the time it can be up while the owner keeps fighting: 1 for effects applied by any hit,
    /// duration ÷ cooldown for abilities and for item effects with a cooldown.
    pub duty: f64,
    /// What has to land for it to apply when an ordinary hit is not enough: `headshots`, `melee hits`, or empty.
    pub needs: String,
}

/// Every resist reduction and damage amplification a build puts on its targets, from items and abilities.
pub fn target_debuffs(data: &GameData, b: &BuildInput) -> Result<Vec<TargetDebuff>, String> {
    let b = &BuildInput { active: vec![], ..b.clone() };
    let sheet = calc::compute(data, b)?;
    let hero = data.hero(&b.hero).ok_or_else(|| format!("Unknown hero: {}", b.hero))?;
    let (rules, _) = debuff_rules(data, &sheet);
    let mut out: Vec<TargetDebuff> = rules
        .into_iter()
        .map(|r| TargetDebuff {
            // The stacking amp of Escalating Exposure raises only its owner's spirit damage.
            team: !(r.source == "Escalating Exposure"),
            source: r.source,
            effect: r.effect,
            seconds: r.seconds,
            max_stacks: r.max_stacks,
            duty: if r.cooldown > r.seconds && r.cooldown > 0.0 { r.seconds / r.cooldown } else { 1.0 },
            needs: match r.on {
                On::Headshot => "headshots".into(),
                On::Melee => "melee hits".into(),
                On::Use => "the item to be used".into(),
                _ => String::new(),
            },
        })
        .collect();
    let stat = |key: &str| sheet.stat(key).unwrap_or(0.0);
    let ctx = Context { per_shot: stat("bullet_damage") * hero.weapon.pellets, shots_per_second: stat("shots_per_second"), abilities: &sheet.abilities, flags: &[] };
    for ability in &sheet.abilities {
        let plan = kit::plan(ability, &ctx, None, None);
        // "From the caster" and the hero's own outgoing bonuses do nothing for teammates.
        let personal = ability.props.iter().any(|p| p.key == "IncomingDamagePercentFromCaster");
        let duty = |seconds: f64| if plan.castable && plan.cooldown > seconds && plan.cooldown > 0.0 { seconds / plan.cooldown } else { 1.0 };
        for d in &plan.debuffs {
            out.push(TargetDebuff {
                source: plan.name.clone(),
                effect: d.effect,
                seconds: d.seconds,
                max_stacks: d.max_stacks,
                team: !(personal && d.effect.all_amp != 0.0) && !["Outgoing damage bonus", "Isolated"].contains(&d.label.as_str()),
                duty: duty(d.seconds),
                needs: "the ability to hit".into(),
            });
        }
        for passive in &plan.passives {
            let (effect, seconds, max_stacks, needs, duty) = match passive {
                Passive::BulletDebuff { effect, max_stacks, seconds } => (*effect, *seconds, *max_stacks, "", 1.0),
                Passive::MeleeDebuff { effect, seconds } => (*effect, *seconds, 1, "melee hits", 1.0),
                Passive::HeadshotProc { debuff: Some((effect, seconds)), cooldown, .. } => {
                    (*effect, *seconds, 1, "headshots", if *cooldown > *seconds { seconds / cooldown } else { 1.0 })
                }
                _ => continue,
            };
            out.push(TargetDebuff { source: plan.name.clone(), effect, seconds, max_stacks, team: true, duty, needs: needs.into() });
        }
        // Graves' gravestone lowers the spirit resist of enemies near it, one stack at a time.
        if ability.class_name == "ability_necro_gravestone" {
            let value = |key: &str| ability.props.iter().find(|p| p.key == key).map(|p| p.value).unwrap_or(0.0);
            let (life, cooldown) = (value("AbilityDuration"), value("AbilityCooldown"));
            out.push(TargetDebuff {
                source: plan.name.clone(),
                effect: Effect { spirit: value("TechArmorDamageReductionPerStack").abs(), ..Default::default() },
                seconds: value("StackDuration"),
                max_stacks: value("MaxStacks").max(1.0) as u32,
                team: true,
                duty: if cooldown > life && cooldown > 0.0 { life / cooldown } else { 1.0 },
                needs: "the enemy to stay near the gravestone (how its stacks build is not documented)".into(),
            });
        }
    }
    // Hunter's Aura lowers the bullet resist of every enemy near its owner, with no hit needed.
    for item in sheet.owned.iter().filter_map(|class| data.item(class)).filter(|i| i.name == "Hunter's Aura") {
        let value = |key: &str| item.props.iter().find(|p| p.key == key).map(|p| p.value.abs()).unwrap_or(0.0);
        out.push(TargetDebuff {
            source: item.name.clone(),
            effect: Effect { bullet: value("BulletArmorReduction"), ..Default::default() },
            seconds: 1.0,
            max_stacks: 1,
            team: true,
            duty: 1.0,
            needs: format!("the enemy to be within {} m of the owner (doubled when only one enemy is near, which is not counted)", n(value("Radius"))),
        });
    }
    out.retain(|d| !d.effect.is_empty() && d.seconds > 0.0);
    Ok(out)
}

/// Items whose self buff is applied by an event in the combo. Their conditional stats must not
/// also count as always-on, so the build's own switch for them is ignored in the simulation.
const DYNAMIC_BUFF_ITEMS: [&str; 7] = [
    "Spirit Snatch",
    "Shadow Weave",
    "Infuser",
    "Ethereal Shift",
    "Unstable Concoction",
    "Diviner's Kevlar",
    "Spiritual Overflow",
];

/// A temporary self buff from an item and the event that grants it.
#[derive(Debug, Clone)]
struct BuffRule {
    item: String,
    on: On,
    buff: Buff,
    seconds: f64,
    cooldown: f64,
    /// Seconds the hero cannot act before the buff starts.
    delay: f64,
    /// Share of the buff granted by a light melee hit.
    light: f64,
    ready_at: f64,
    /// Only when this ability is the one being cast.
    ability: Option<usize>,
}

/// Self buffs the owned items grant during a combo: on use, on melee, on the ultimate, or by buildup.
fn buff_rules(data: &GameData, sheet: &Sheet) -> (Vec<BuffRule>, Vec<Proc>, Vec<String>) {
    let (mut rules, mut procs, mut passives) = (vec![], vec![], vec![]);
    for class in &sheet.owned {
        let Some(item) = data.item(class) else { continue };
        let v = |key: &str| item.props.iter().find(|p| p.key == key).map(|p| p.value).unwrap_or(0.0);
        let name = item.name.as_str();
        let mut add = |on: On, buff: Buff, seconds: f64, cooldown: f64, delay: f64, light: f64, when: &str| {
            if buff.is_empty() || seconds <= 0.0 {
                return;
            }
            passives.push(format!("{name}: {} for {}s {when}", buff.text(), n(seconds)));
            rules.push(BuffRule { item: name.to_string(), on, buff, seconds, cooldown, delay, light, ready_at: 0.0, ability: None });
        };
        let spirit = |x: f64| Buff { spirit_power: x, ..Default::default() };
        match name {
            "Spirit Snatch" => add(
                On::Melee,
                spirit(v("TechPowerGain")),
                v("AbilityDuration"),
                v("AbilityCooldown"),
                0.0,
                0.7,
                "on a melee hit (30% less on a light melee)",
            ),
            "Shadow Weave" => add(
                On::Use,
                Buff {
                    spirit_power: v("AmbushBonusTechPower"),
                    fire_rate: v("AmbushBonusFireRate"),
                    melee_pct: v("AmbushBonusMeleeDamage"),
                    ..Default::default()
                },
                v("AmbushDuration"),
                0.0,
                0.0,
                1.0,
                "when used (the ambush is assumed to start right away)",
            ),
            "Infuser" => add(
                On::Use,
                Buff { spirit_power: v("BonusSpirit"), spirit_lifesteal: v("AbilityLifestealPercentHero"), ..Default::default() },
                v("AbilityDuration"),
                0.0,
                0.0,
                1.0,
                "when used",
            ),
            // The debuffed target returns a share of the spirit damage it takes.
            "Spirit Shredder" | "Spirit Rend" => add(
                On::Bullet,
                Buff { spirit_lifesteal: v("AbilityLifestealPercentHero"), ..Default::default() },
                v("DebuffDuration"),
                0.0,
                0.0,
                1.0,
                "against the target your bullets debuffed",
            ),
            "Ethereal Shift" => add(
                On::Use,
                spirit(v("BonusSpirit")),
                v("BuffDuration"),
                0.0,
                v("AbilityDuration"),
                1.0,
                "after the void state ends",
            ),
            "Unstable Concoction" => add(
                On::Use,
                Buff { spirit_power: v("TechPower"), weapon_pct: v("BaseAttackDamagePercent"), ..Default::default() },
                v("AbilityDuration"),
                0.0,
                0.0,
                1.0,
                "when used (the explosion at the end is not simulated)",
            ),
            "Diviner's Kevlar" => add(
                On::Ultimate,
                spirit(v("TechPower")),
                v("BuffDuration"),
                v("AbilityCooldown"),
                0.0,
                1.0,
                "when you cast your ultimate",
            ),
            "Spiritual Overflow" => {
                // The buildup value is already converted to percent per shot for this hero's weapon.
                let per_shot = sheet
                    .item_effects
                    .iter()
                    .find(|e| e.name == name)
                    .and_then(|e| val(&e.props, "BuildUpPerShot"))
                    .unwrap_or(0.0);
                let buff = Buff {
                    spirit_power: v("BonusSpirit"),
                    fire_rate: v("BonusFireRate"),
                    spirit_lifesteal: v("BonusSpiritLifesteal"),
                    ..Default::default()
                };
                if per_shot > 0.0 && !buff.is_empty() {
                    let mut p = Proc::new(name, Trigger::Buildup { per_shot: per_shot / 100.0 }, Kind::Spirit, 0.0, 0.0);
                    p.buff = Some((buff, v("AbilityDuration")));
                    passives.push(format!(
                        "{name}: {}% buildup per shot, then {} for {}s",
                        n(per_shot),
                        buff.text(),
                        n(v("AbilityDuration"))
                    ));
                    procs.push(p);
                }
            }
            _ => {}
        }
    }
    (rules, procs, passives)
}

/// A conditional item bonus the player switches on as a combo action.
#[derive(Debug, Clone)]
struct Condition {
    /// Item class name, or a `hero:` id for a hero state.
    item: String,
    name: String,
    buff: Buff,
    effect: Effect,
    seconds: f64,
    /// Shown instead of the generated text when set.
    note: Option<String>,
    /// Set for a counted condition; the buff and effect are then per stack.
    max_count: Option<f64>,
    unit: String,
}

/// Owned items with conditional stats that no event in the simulation grants on its own.
fn conditions(data: &GameData, sheet: &Sheet) -> Vec<Condition> {
    let mut out = vec![];
    for class in &sheet.owned {
        let Some(item) = data.item(class) else { continue };
        // Imbue items follow their ability, the dynamic ones follow events, and Express Shot is a single shot.
        if !item.imbue.is_empty() || DYNAMIC_BUFF_ITEMS.contains(&item.name.as_str()) || item.name == "Express Shot" {
            continue;
        }
        let (mut buff, mut effect) = (Buff::default(), Effect::default());
        for p in item.props.iter().filter(|p| p.section != "innate" && p.value > 0.0) {
            match p.modifier.as_deref().unwrap_or("").trim_start_matches("MODIFIER_VALUE_") {
                "TECH_POWER" => buff.spirit_power += p.value,
                "TECH_POWER_PERCENT" => buff.spirit_power_pct += p.value,
                "FIRE_RATE" => buff.fire_rate += p.value,
                "WEAPON_DAMAGE_INCREASE" | "CLOSE_RANGE_WEAPON_DAMAGE_INCREASE" | "LONG_RANGE_BULLET_DAMAGE_INCREASE" => {
                    buff.weapon_pct += p.value
                }
                "MELEE_DAMAGE_INCREASE" => buff.melee_pct += p.value,
                "BULLET_LIFESTEAL" => buff.bullet_lifesteal += p.value,
                "TECH_LIFESTEAL" => buff.spirit_lifesteal += p.value,
                // More outgoing damage is the same as the target taking more from this hero.
                "ALL_DAMAGE_MULTIPLIER" => effect.all_amp += p.value,
                _ => {}
            }
        }
        let v = |key: &str| item.props.iter().find(|p| p.key == key).map(|p| p.value).filter(|s| *s > 0.0);
        // Stacking weapon damage that is not listed as a stat (Berserker, Split Shot).
        if let Some(per_stack) = v("WeaponPowerPerStack").or(v("WeaponDamagePerStack")) {
            buff.weapon_pct += per_stack;
        }
        if buff.is_empty() && effect.is_empty() {
            continue;
        }
        out.push(Condition {
            item: class.clone(),
            name: item.name.clone(),
            buff,
            effect,
            seconds: v("BuffDuration").or(v("AbilityDuration")).unwrap_or(f64::INFINITY),
            note: None,
            max_count: v("MaxStacks"),
            unit: "stacks".into(),
        });
    }
    out
}

impl Condition {
    fn text(&self) -> String {
        if let Some(note) = &self.note {
            return format!("{note} until the combo ends");
        }
        let per_stack = match self.max_count {
            Some(max) => format!(" per stack (up to {})", n(max)),
            None => String::new(),
        };
        let mut parts = vec![];
        if !self.buff.is_empty() {
            parts.push(self.buff.text());
        }
        if !self.effect.is_empty() {
            parts.push(self.effect.text());
        }
        let length = if self.seconds.is_finite() { format!("for {}s", n(self.seconds)) } else { "until the combo ends".into() };
        format!("{}{per_stack} {length}", parts.join(", "))
    }
}

/// Buffs that imbue items grant when their ability is used.
fn imbue_buff_rules(data: &GameData, sheet: &Sheet, b: &BuildInput) -> (Vec<BuffRule>, Vec<String>) {
    let (mut rules, mut passives) = (vec![], vec![]);
    for class in &sheet.owned {
        let Some(item) = data.item(class) else { continue };
        if item.imbue.is_empty() {
            continue;
        }
        let v = |key: &str| item.props.iter().find(|p| p.key == key).map(|p| p.value).unwrap_or(0.0);
        let name = item.name.as_str();
        let (buff, seconds, cooldown) = match name {
            "Surge of Power" => (Buff { fire_rate: v("FireRateBonus"), ..Default::default() }, v("MovementSpeedBonusDuration"), v("AbilityCooldown")),
            "Ballistic Enchantment" => (Buff { weapon_pct: v("WeaponPowerPerStack"), ..Default::default() }, v("AbilityDuration"), 0.0),
            "Quicksilver Reload" | "Mercurial Magnum" => {
                (Buff { fire_rate: v("BonusFireRate"), ..Default::default() }, v("BuffDuration"), v("AbilityChargeUpTime"))
            }
            _ => (Buff::default(), 0.0, 0.0),
        };
        let Some(&index) = b.imbues.get(class) else {
            passives.push(format!("{name}: not imbued yet. Pick its ability on the item in the purchase order."));
            continue;
        };
        let ability = sheet.abilities.get(index).map(|a| a.name.as_str()).unwrap_or("?");
        if buff.is_empty() || seconds <= 0.0 {
            passives.push(format!("{name}: imbued into {ability}"));
            continue;
        }
        passives.push(format!("{name}: imbued into {ability}; using it grants {} for {}s", buff.text(), n(seconds)));
        rules.push(BuffRule {
            item: name.to_string(),
            on: On::AbilityCast,
            buff,
            seconds,
            cooldown,
            delay: 0.0,
            light: 1.0,
            ready_at: 0.0,
            ability: Some(index),
        });
    }
    (rules, passives)
}

/// A heal an owned item gives the hero and the event that grants it.
#[derive(Debug, Clone)]
struct HealRule {
    item: String,
    on: On,
    flat: f64,
    /// Percent of the hero's own max / missing health.
    max_pct: f64,
    missing_pct: f64,
    /// The heal is never smaller than this.
    min: f64,
    /// Percent of the melee damage just dealt.
    melee_pct: f64,
    /// Seconds the heal is spread over; 0 heals at once.
    seconds: f64,
    cooldown: f64,
    /// Cooldown multiplier after a light melee hit.
    light_cooldown: f64,
    /// Damage to deal before the heal triggers (with `On::Damage`).
    threshold: f64,
    progress: f64,
    ready_at: f64,
    counter: f64,
}

impl HealRule {
    fn text(&self) -> String {
        let mut parts = vec![];
        if self.flat > 0.0 {
            parts.push(format!("{} health", n(self.flat)));
        }
        if self.max_pct > 0.0 {
            parts.push(format!("{}% of your max health", n(self.max_pct)));
        }
        if self.missing_pct > 0.0 {
            parts.push(format!("{}% of your missing health", n(self.missing_pct)));
        }
        if self.melee_pct > 0.0 {
            parts.push(format!("{}% of the melee damage dealt", n(self.melee_pct)));
        }
        let mut s = parts.join(" + ");
        if self.min > 0.0 {
            s += &format!(" (at least {})", n(self.min));
        }
        if self.seconds > 0.0 {
            s += &format!(" over {}s", n(self.seconds));
        }
        s
    }
}

/// Heals the owned items give their owner during a combo.
fn heal_rules(sheet: &Sheet) -> (Vec<HealRule>, Vec<String>) {
    let (mut rules, mut passives) = (vec![], vec![]);
    for e in &sheet.item_effects {
        let v = |key: &str| val(&e.props, key).unwrap_or(0.0);
        let cd = v("AbilityCooldown");
        // A used item's cooldown is tracked by the item action itself.
        let rule = |on: On, flat: f64| HealRule {
            item: e.name.clone(),
            on,
            flat,
            max_pct: 0.0,
            missing_pct: 0.0,
            min: 0.0,
            melee_pct: 0.0,
            seconds: 0.0,
            cooldown: if on == On::Use { 0.0 } else { cd },
            light_cooldown: 1.0,
            threshold: 0.0,
            progress: 0.0,
            ready_at: 0.0,
            counter: 0.0,
        };
        let regen = || HealRule {
            seconds: v("RegenerationDuration"),
            cooldown: 0.0,
            ..rule(On::SpiritDamage, v("Regeneration") * v("RegenerationDuration"))
        };
        let melee = "on a melee hit";
        let (r, when): (HealRule, String) = match e.name.as_str() {
            "Healing Rite" => (
                HealRule { seconds: v("RegenDuration"), ..rule(On::Use, v("TotalHealthRegen")) },
                "when used (taking damage from an enemy would end it early)".into(),
            ),
            "Healing Nova" => (HealRule { seconds: v("RegenDuration"), ..rule(On::Use, v("TotalHealthRegen")) }, "when used".into()),
            "Dispel Magic" => (rule(On::Use, v("HealOnActivate")), "when used, if it removed an effect".into()),
            "Restorative Locket" => (
                rule(On::Use, v("HealPerStack") * v("MaxStacks")),
                format!("when used with all {} stacks", n(v("MaxStacks"))),
            ),
            "Celestial Blessing" => (
                HealRule { missing_pct: v("HealPercentAmount"), min: v("MinHeal"), ..rule(On::Use, 0.0) },
                "when used".into(),
            ),
            "Rescue Beam" => (
                HealRule { max_pct: v("HealPercentAmount"), seconds: v("AbilityChannelTime"), ..rule(On::Use, 0.0) },
                "when used on yourself".into(),
            ),
            "Restorative Shot" => (rule(On::Bullet, v("HealFromHero")), format!("on a bullet hit, {}s cooldown", n(cd))),
            "Melee Lifesteal" => (
                HealRule { light_cooldown: 1.5, ..rule(On::Melee, v("LifestrikeHeal")) },
                format!("{melee}, {}s cooldown (1.5 times as long after a light melee)", n(cd)),
            ),
            "Lifestrike" => (
                HealRule { melee_pct: v("LifestealHealPercent"), light_cooldown: 1.5, ..rule(On::Melee, v("LifestealHeal")) },
                format!("{melee}, {}s cooldown (1.5 times as long after a light melee)", n(cd)),
            ),
            "Headhunter" => (
                HealRule { max_pct: v("HealPercentPerHeadshot"), ..rule(On::Headshot, 0.0) },
                format!("on a headshot, {}s cooldown", n(cd)),
            ),
            "Mystic Regeneration" => (regen(), "after you deal spirit damage (more spirit damage refreshes it)".into()),
            "Radiant Regeneration" => {
                let r = regen();
                if r.flat > 0.0 {
                    passives.push(format!("{}: heals {} after you deal spirit damage (more spirit damage refreshes it)", e.name, r.text()));
                    rules.push(r);
                }
                (rule(On::AbilityCast, v("HealingPerCast")), format!("when you use an ability, {}s cooldown", n(cd)))
            }
            "Healbane" => (HealRule { cooldown: 0.0, ..rule(On::Kill, v("HealOnKill")) }, "when the target dies".into()),
            "Mystic Conduit" => (
                HealRule { threshold: v("DamageThreshold"), ..rule(On::Damage, v("HealAmount")) },
                format!("after you deal {} damage, {}s cooldown (the 5s window is not checked)", n(v("DamageThreshold")), n(cd)),
            ),
            _ => continue,
        };
        if r.text().is_empty() {
            continue;
        }
        passives.push(format!("{}: heals {} {when}", e.name, r.text()));
        rules.push(r);
    }
    (rules, passives)
}

fn describe(a: &Active, rules: &[Rule], buffs: &[BuffRule], heals: &[HealRule]) -> String {
    let mut parts = vec![];
    if a.damage > 0.0 {
        parts.push(format!("{} spirit damage", n(a.damage)));
    }
    if a.dot_pct_current > 0.0 {
        parts.push(format!("{}% of current health per second for {}s", n(a.dot_pct_current), n(a.dot_seconds)));
    }
    if a.dot_dps > 0.0 {
        parts.push(format!("{} spirit DPS for {}s", n(a.dot_dps), n(a.dot_seconds)));
    }
    for r in rules.iter().filter(|r| r.on == On::Use && r.item == a.name) {
        parts.push(format!("{} for {}s", r.effect.text(), n(r.seconds)));
    }
    for r in buffs.iter().filter(|r| r.on == On::Use && r.item == a.name) {
        parts.push(format!("{} for {}s", r.buff.text(), n(r.seconds)));
    }
    for r in heals.iter().filter(|r| r.on == On::Use && r.item == a.name) {
        parts.push(format!("heals {}", r.text()));
    }
    parts.push(format!("{}s cooldown", n(a.cooldown)));
    parts.join(", ")
}

struct Dot {
    source: String,
    kind: Kind,
    dps: f64,
    pct_current: f64,
    end: f64,
}

/// A hit that lands later: delayed damage or one hit of a multi-hit ability.
struct Pending {
    time: f64,
    source: String,
    label: String,
    kind: Kind,
    raw: f64,
    health: Health,
    low_hp: Option<(f64, f64)>,
    ramp: Option<(f64, f64)>,
    quiet: bool,
    /// Debuffs this hit applies: (source, effect, seconds, max stacks).
    debuffs: Vec<(String, Effect, f64, u32)>,
}

/// A heal over time on the hero.
struct Regen {
    source: String,
    per_second: f64,
    end: f64,
}

struct Gun {
    /// Bullet damage before weapon damage scaling.
    base_bullet: f64,
    weapon_pct: f64,
    pellets: f64,
    base_shots_per_second: f64,
    fire_rate_pct: f64,
    clip: f64,
    reload: f64,
    crit: f64,
}

struct Sim {
    t: f64,
    health: f64,
    dealt: f64,
    bullet_resist: f64,
    spirit_resist: f64,
    kill_time: Option<f64>,
    dots: Vec<Dot>,
    pending: Vec<Pending>,
    procs: Vec<Proc>,
    rules: Vec<Rule>,
    debuffs: Vec<Debuff>,
    buffs: Vec<ActiveBuff>,
    stack_buffs: Vec<StackBuff>,
    next_tick: Vec<f64>,
    spirit_progress: f64,
    sources: Vec<SourceTotal>,
    events: Vec<Event>,
    ammo: f64,
    reload_left: f64,
    /// Share of shots that hit the head, 0-1.
    headshot: f64,
    /// Whether the melee hit being resolved is a heavy one.
    heavy_melee: bool,
    /// Items that charge up the next heavy melee: (name, bonus %, cooldown, ready at).
    heavy_bonus: Vec<(String, f64, f64, f64)>,
    buff_rules: Vec<BuffRule>,
    /// Index of the ability being cast.
    casting: Option<usize>,
    /// Express Shot: (bonus weapon damage %, cooldown, extra ammo, ready at).
    express: Option<(f64, f64, f64, f64)>,
    /// The hero's own max health and current health; heals raise the latter up to the former.
    own_max: f64,
    own_hp: f64,
    healed: f64,
    heals: Vec<HealTotal>,
    regens: Vec<Regen>,
    heal_rules: Vec<HealRule>,
    /// Heals granted when the target dies: (ability, heal).
    kill_heals: Vec<(String, Heal)>,
    /// Bonus to heals and regeneration, percent. Lifesteal is not affected.
    heal_amp: f64,
    base_regen: f64,
    /// Lifesteal from always-on item stats, percent: (bullet, spirit).
    base_lifesteal: (f64, f64),
    /// Multiplier on all lifesteal (Rat King gets less out of it).
    lifesteal_scale: f64,
    /// Damage sources that return a share of their damage as health: (source, percent).
    leeches: Vec<(String, f64)>,
    /// Lifesteal an imbue item adds to its ability: (ability index, percent).
    imbue_leech: Option<(usize, f64)>,
    /// Max health already stolen from the target by Siphon Bullets.
    siphoned: f64,
    /// Damage of the last melee hit, for heals based on it.
    last_melee: f64,
    /// Always-on bonus or penalty to all damage dealt, percent.
    outgoing: f64,
    /// For each charged ability, when each of its charges is ready again.
    charges: Vec<Vec<f64>>,
}

impl Sim {
    fn hp(&self) -> f64 {
        (self.health - self.dealt).max(0.0)
    }

    fn log(&mut self, source: &str, detail: impl Into<String>, damage: Option<f64>) {
        self.events.push(Event { time: self.t, source: source.to_string(), detail: detail.into(), damage, heal: None });
    }

    /// Adds to the healing totals without a timeline entry.
    fn heal_quiet(&mut self, source: &str, amount: f64) {
        if amount <= 0.0 {
            return;
        }
        self.healed += amount;
        self.own_hp = (self.own_hp + amount).min(self.own_max);
        match self.heals.iter_mut().find(|h| h.source == source) {
            Some(h) => h.amount += amount,
            None => self.heals.push(HealTotal { source: source.to_string(), amount }),
        }
    }

    fn heal(&mut self, source: &str, amount: f64, detail: &str) {
        if amount <= 0.0 {
            return;
        }
        self.heal_quiet(source, amount);
        self.events.push(Event { time: self.t, source: source.to_string(), detail: detail.to_string(), damage: None, heal: Some(amount) });
    }

    /// Starts a heal over time; starting it again replaces the one still running.
    fn start_regen(&mut self, source: &str, total: f64, seconds: f64) {
        if total <= 0.0 || seconds <= 0.0 {
            return;
        }
        let (per_second, end) = (total / seconds, self.t + seconds);
        if let Some(r) = self.regens.iter_mut().find(|r| r.source == source) {
            r.per_second = per_second;
            r.end = end;
            return;
        }
        self.regens.push(Regen { source: source.to_string(), per_second, end });
        self.log(source, format!("heal over time: {} health over {}s", n(total), n(seconds)), None);
    }

    /// A heal made of a flat amount and shares of the hero's own max and missing health.
    fn own_amount(&self, flat: f64, max_pct: f64, missing_pct: f64) -> f64 {
        flat + (max_pct * self.own_max + missing_pct * (self.own_max - self.own_hp).max(0.0)) / 100.0
    }

    fn apply_heal(&mut self, source: &str, h: &Heal, times: f64) {
        let amount = self.own_amount(h.flat, h.max_pct, h.missing_pct) * times * (1.0 + self.heal_amp / 100.0);
        let label = format!("{source} (heal)");
        if h.seconds > 0.0 {
            self.start_regen(&label, amount, h.seconds);
        } else {
            self.heal(&label, amount, &h.label);
        }
    }

    fn fire_heal(&mut self, i: usize) {
        let r = self.heal_rules[i].clone();
        let slow = if r.on == On::Melee && !self.heavy_melee { r.light_cooldown } else { 1.0 };
        self.heal_rules[i].ready_at = self.t + r.cooldown * slow;
        self.heal_rules[i].counter = 0.0;
        self.heal_rules[i].progress = 0.0;
        let amount = self.own_amount(r.flat, r.max_pct, r.missing_pct) + r.melee_pct / 100.0 * self.last_melee;
        let amount = amount.max(r.min) * (1.0 + self.heal_amp / 100.0);
        if r.seconds > 0.0 {
            self.start_regen(&r.item, amount, r.seconds);
        } else {
            self.heal(&r.item, amount, "heal");
        }
    }

    /// Grants the item heals tied to this event. `weight` is the share of the event that counts
    /// (the headshot rate for a headshot); the heal lands when the shares add up to one.
    fn trigger_heals(&mut self, on: On, item: Option<&str>, weight: f64) {
        if weight <= 0.0 {
            return;
        }
        for i in 0..self.heal_rules.len() {
            let r = &self.heal_rules[i];
            if r.on != on || self.t < r.ready_at || item.is_some_and(|name| name != r.item) {
                continue;
            }
            let counter = r.counter + weight;
            if counter < 1.0 - 1e-9 {
                self.heal_rules[i].counter = counter;
                continue;
            }
            self.fire_heal(i);
        }
    }

    /// Share of the damage of this type that returns as health. Sources combine multiplicatively.
    fn lifesteal(&self, kind: Kind) -> f64 {
        let pick = |b: &Buff| match kind {
            Kind::Bullet => b.bullet_lifesteal,
            Kind::Spirit => b.spirit_lifesteal,
            Kind::Melee => b.melee_lifesteal,
            Kind::Pure => 0.0,
        };
        let base = match kind {
            Kind::Bullet => self.base_lifesteal.0,
            Kind::Spirit => self.base_lifesteal.1,
            Kind::Melee | Kind::Pure => 0.0,
        };
        let mut keep = 1.0 - base / 100.0;
        for b in &self.buffs {
            keep *= 1.0 - pick(&b.buff) / 100.0;
        }
        for s in self.stack_buffs.iter().filter(|s| s.end > self.t) {
            keep *= 1.0 - pick(&s.per_stack) * s.stacks / 100.0;
        }
        (1.0 - keep.clamp(0.0, 1.0)) * self.lifesteal_scale
    }

    fn on_kill(&mut self) {
        for (source, h) in self.kill_heals.clone() {
            self.apply_heal(&source, &h, 1.0);
        }
        self.trigger_heals(On::Kill, None, 1.0);
    }

    /// Share of raw damage the target takes right now, with every active debuff applied.
    fn taken(&self, kind: Kind) -> f64 {
        let mut e = Effect::default();
        for d in &self.debuffs {
            let stacks = d.stacks as f64;
            e.bullet += d.per_stack.bullet * stacks;
            e.spirit += d.per_stack.spirit * stacks;
            e.spirit_amp += d.per_stack.spirit_amp * stacks;
            e.bullet_amp += d.per_stack.bullet_amp * stacks;
            e.all_amp += d.per_stack.all_amp * stacks;
        }
        let taken = match kind {
            Kind::Bullet => {
                (1.0 - (self.bullet_resist - e.bullet) / 100.0).max(0.0) * (1.0 + (e.all_amp + e.bullet_amp) / 100.0)
            }
            Kind::Spirit => {
                (1.0 - (self.spirit_resist - e.spirit) / 100.0).max(0.0) * (1.0 + (e.all_amp + e.spirit_amp) / 100.0)
            }
            Kind::Melee | Kind::Pure => 1.0 + e.all_amp / 100.0,
        };
        taken * (1.0 + self.outgoing / 100.0)
    }

    /// Sum of every self buff active right now.
    fn buff(&self) -> Buff {
        let mut total = Buff::default();
        for b in &self.buffs {
            total.add(&b.buff, 1.0);
        }
        for s in self.stack_buffs.iter().filter(|s| s.end > self.t) {
            total.add(&s.per_stack, s.stacks);
        }
        total
    }

    fn apply_debuff(&mut self, source: &str, effect: Effect, seconds: f64, max_stacks: u32, stacks: u32) {
        if effect.is_empty() || seconds <= 0.0 {
            return;
        }
        let end = self.t + seconds;
        if let Some(d) = self.debuffs.iter_mut().find(|d| d.source == source) {
            d.stacks = d.stacks.saturating_add(stacks).min(max_stacks);
            d.end = end;
            return;
        }
        self.debuffs.push(Debuff { source: source.to_string(), per_stack: effect, stacks: stacks.min(max_stacks), end });
        let stacking = if max_stacks > 1 { " per stack" } else { "" };
        self.log(source, format!("debuff: {}{stacking} for {}s", effect.text(), n(seconds)), None);
    }

    fn apply_buff(&mut self, source: &str, buff: Buff, seconds: f64) {
        if buff.is_empty() || seconds <= 0.0 {
            return;
        }
        let end = self.t + seconds;
        match self.buffs.iter_mut().find(|b| b.source == source) {
            // Setting a condition again replaces its value (e.g. a new stack count).
            Some(b) => {
                // A refresh is not announced again.
                let same = b.buff.text() == buff.text() && b.end > self.t;
                b.end = end;
                b.buff = buff;
                if same {
                    return;
                }
            }
            None => self.buffs.push(ActiveBuff { source: source.to_string(), buff, end }),
        }
        if seconds.is_finite() {
            self.log(source, format!("self buff: {} for {}s", buff.text(), n(seconds)), None);
        } else if self.t > 0.0 {
            self.log(source, format!("self buff: {} (permanent)", buff.text()), None);
        }
    }

    /// Grants the item self buffs tied to this event.
    fn trigger_buffs(&mut self, on: On, item: Option<&str>) {
        for i in 0..self.buff_rules.len() {
            let r = &self.buff_rules[i];
            if r.on != on || self.t < r.ready_at || item.is_some_and(|name| name != r.item) {
                continue;
            }
            if r.ability.is_some() && r.ability != self.casting {
                continue;
            }
            let share = if on == On::Melee && !self.heavy_melee { r.light } else { 1.0 };
            let mut buff = Buff::default();
            buff.add(&r.buff, share);
            let (name, seconds) = (r.item.clone(), r.seconds);
            self.buff_rules[i].ready_at = self.t + self.buff_rules[i].cooldown;
            self.apply_buff(&name, buff, seconds);
        }
    }

    /// Applies the debuffs tied to this event.
    fn trigger_rules(&mut self, on: On, item: Option<&str>, stacks: u32) {
        for i in 0..self.rules.len() {
            let r = &self.rules[i];
            if r.on != on || self.t < r.ready_at || item.is_some_and(|name| name != r.item) {
                continue;
            }
            let (source, effect, seconds, max_stacks) = (r.source.clone(), r.effect, r.seconds, r.max_stacks);
            self.rules[i].ready_at = self.t + self.rules[i].cooldown;
            self.apply_debuff(&source, effect, seconds, max_stacks, stacks);
        }
    }

    fn deal(&mut self, source: &str, kind: Kind, raw: f64) -> f64 {
        let damage = raw * self.taken(kind);
        if damage <= 0.0 {
            return 0.0;
        }
        self.dealt += damage;
        let killed = self.kill_time.is_none() && self.dealt >= self.health;
        if killed {
            self.kill_time = Some(self.t);
        }
        match self.sources.iter_mut().find(|s| s.source == source && s.kind == kind) {
            Some(s) => s.damage += damage,
            None => self.sources.push(SourceTotal { source: source.to_string(), kind, damage }),
        }
        let stolen = damage * self.lifesteal(kind);
        if stolen > 0.0 {
            let label = match kind {
                Kind::Bullet => "Bullet lifesteal",
                Kind::Spirit => "Spirit lifesteal",
                Kind::Melee | Kind::Pure => "Melee lifesteal",
            };
            self.heal_quiet(label, stolen);
        }
        let leech: f64 = self.leeches.iter().filter(|(s, _)| s == source).map(|(_, pct)| pct).sum();
        if leech > 0.0 {
            self.heal_quiet(&format!("{source} (damage to health)"), damage * leech / 100.0 * (1.0 + self.heal_amp / 100.0));
        }
        for i in 0..self.heal_rules.len() {
            let r = &mut self.heal_rules[i];
            if r.on == On::Damage && self.t >= r.ready_at {
                r.progress += damage;
                if r.progress >= r.threshold {
                    self.fire_heal(i);
                }
            }
        }
        if kind == Kind::Spirit {
            self.trigger_heals(On::SpiritDamage, None, 1.0);
        }
        if killed {
            self.on_kill();
        }
        if kind == Kind::Spirit {
            // Debuffs land after the hit that triggers them.
            self.trigger_rules(On::SpiritDamage, None, 1);
            self.spirit_progress += damage;
            let ready: Vec<usize> = (0..self.procs.len())
                .filter(|&i| match self.procs[i].trigger {
                    Trigger::SpiritThreshold { threshold } => {
                        self.spirit_progress >= threshold && self.t >= self.procs[i].ready_at
                    }
                    _ => false,
                })
                .collect();
            for i in ready {
                self.spirit_progress = 0.0;
                self.fire(i);
            }
        }
        damage
    }

    /// Raw damage of a hit that also scales with the target's health.
    fn with_health(&self, raw: f64, health: &Health) -> f64 {
        let hp = self.hp();
        raw + (health.current * hp + health.max * self.health + health.missing * (self.health - hp)) / 100.0
    }

    fn fire(&mut self, i: usize) {
        let mut p = self.procs[i].clone();
        if p.trigger == Trigger::Melee && !self.heavy_melee {
            p.damage *= p.light_damage;
            p.cooldown *= p.light_cooldown;
        }
        self.procs[i].ready_at = self.t + p.cooldown;
        self.procs[i].counter = 0.0;
        if let Trigger::Ultimate { delay } = p.trigger {
            self.pending.push(Pending {
                time: self.t + delay,
                source: p.item,
                label: "delayed hit".into(),
                kind: p.kind,
                raw: p.damage,
                health: Health::default(),
                low_hp: None,
                ramp: None,
                quiet: false,
                debuffs: vec![],
            });
            return;
        }
        let mut raw = p.damage;
        if let Some((max, full_at)) = p.ramp {
            let missing = 1.0 - self.hp() / self.health.max(1.0);
            raw += (max - raw) * (missing / (1.0 - full_at / 100.0).max(1e-9)).clamp(0.0, 1.0);
        }
        let mut raw = raw + p.pct_current / 100.0 * self.hp();
        if p.siphon > 0.0 {
            // Each application steals a share of the max health the target has left.
            let stolen = p.siphon / 100.0 * (self.health - self.siphoned).max(0.0);
            self.siphoned += stolen;
            raw += stolen;
        }
        if raw > 0.0 {
            let dealt = self.deal(&p.item, p.kind, raw);
            self.log(&p.item, "proc", Some(dealt));
            if p.siphon > 0.0 {
                self.heal_quiet(&p.item, dealt);
            }
        }
        if let Some((dps, seconds)) = p.dot {
            self.start_dot(&p.item, p.kind, dps, 0.0, seconds);
        }
        if let Some((effect, seconds)) = p.debuff {
            self.apply_debuff(&p.item, effect, seconds, 1, 1);
        }
        if let Some((buff, seconds)) = p.buff {
            self.apply_buff(&p.item, buff, seconds);
        }
        if p.reloads {
            self.ammo = f64::INFINITY; // clamped to the magazine size on the next shot
            self.reload_left = 0.0;
        }
    }

    fn start_dot(&mut self, source: &str, kind: Kind, dps: f64, pct_current: f64, seconds: f64) {
        // Reapplying an effect refreshes its duration instead of stacking it.
        let end = self.t + seconds;
        if let Some(d) = self.dots.iter_mut().find(|d| d.source == source) {
            let added = (end - d.end).max(0.0);
            d.end = d.end.max(end);
            d.dps = dps;
            d.pct_current = pct_current;
            self.log(source, format!("refreshed (+{}s)", n(added)), None);
            return;
        }
        let detail = if pct_current > 0.0 && dps > 0.0 {
            format!("{} DPS + {}% of current health per second for {}s", n(dps), n(pct_current), n(seconds))
        } else if pct_current > 0.0 {
            format!("{}% of current health per second for {}s", n(pct_current), n(seconds))
        } else {
            format!("{} DPS for {}s", n(dps), n(seconds))
        };
        self.log(source, detail, None);
        self.dots.push(Dot { source: source.to_string(), kind, dps, pct_current, end });
    }

    /// Fires every ready proc with this trigger.
    fn trigger(&mut self, matches: impl Fn(&Trigger) -> bool) {
        for i in 0..self.procs.len() {
            if matches(&self.procs[i].trigger) && self.t >= self.procs[i].ready_at {
                self.fire(i);
            }
        }
    }

    fn land(&mut self, p: Pending) {
        let mut raw = p.raw;
        if let Some((max, full_at)) = p.ramp {
            // Linear from the base at full health to the maximum at `full_at`% health.
            let missing = 1.0 - self.hp() / self.health.max(1.0);
            let span = (1.0 - full_at / 100.0).max(1e-9);
            raw += (max - raw) * (missing / span).clamp(0.0, 1.0);
        }
        let mut raw = self.with_health(raw, &p.health);
        if let Some((threshold, bonus)) = p.low_hp {
            if self.hp() < self.health * threshold / 100.0 {
                raw *= 1.0 + bonus / 100.0;
            }
        }
        let dealt = self.deal(&p.source, p.kind, raw);
        if !p.quiet {
            self.log(&p.source, p.label, Some(dealt));
        }
        for (source, effect, seconds, max_stacks) in p.debuffs {
            self.apply_debuff(&source, effect, seconds, max_stacks, 1);
        }
    }

    fn advance(&mut self, to: f64) {
        while self.t < to - 1e-9 {
            let dt = STEP.min(to - self.t);
            let hp = self.hp();
            let ticks: Vec<(String, Kind, f64)> = self
                .dots
                .iter()
                .map(|d| {
                    let active = (d.end - self.t).min(dt).max(0.0);
                    (d.source.clone(), d.kind, (d.dps + d.pct_current / 100.0 * hp) * active)
                })
                .collect();
            let regen: Vec<(String, f64)> =
                self.regens.iter().map(|r| (r.source.clone(), r.per_second * (r.end - self.t).min(dt).max(0.0))).collect();
            self.t += dt;
            for (source, amount) in regen {
                self.heal_quiet(&source, amount);
            }
            self.heal_quiet("Health regen", self.base_regen * dt);
            let now = self.t;
            self.regens.retain(|r| r.end > now + 1e-9);
            for (source, kind, raw) in ticks {
                self.deal(&source, kind, raw);
            }
            let now = self.t;
            self.dots.retain(|d| d.end > now + 1e-9);

            let (due, later): (Vec<_>, Vec<_>) =
                std::mem::take(&mut self.pending).into_iter().partition(|p| p.time <= now + 1e-9);
            self.pending = later;
            for p in due {
                self.land(p);
            }
            for i in 0..self.procs.len() {
                if let Trigger::Periodic { interval } = self.procs[i].trigger {
                    while self.next_tick[i] <= self.t + 1e-9 {
                        self.next_tick[i] += interval;
                        self.fire(i);
                    }
                }
            }
            self.debuffs.retain(|d| d.end > now + 1e-9);
            self.buffs.retain(|b| b.end > now + 1e-9);
        }
    }

    /// Adds a shot's chance to each matching proc and fires those that reach a full count.
    fn count_toward(&mut self, add: impl Fn(&Trigger) -> Option<f64>) {
        for i in 0..self.procs.len() {
            let Some(amount) = add(&self.procs[i].trigger) else { continue };
            if self.t < self.procs[i].ready_at {
                continue;
            }
            if matches!(self.procs[i].trigger, Trigger::Buildup { .. }) {
                // A running effect is extended by shots instead of building up again.
                if self.procs[i].extend_per_shot > 0.0 {
                    // The expected extension for the share of shots that hit the head.
                    let p = &self.procs[i];
                    let per_headshot = if p.extend_per_headshot > 0.0 { p.extend_per_headshot } else { p.extend_per_shot };
                    let extend = p.extend_per_shot + self.headshot * (per_headshot - p.extend_per_shot);
                    let max_end = self.t + p.max_seconds;
                    let item = self.procs[i].item.clone();
                    if let Some(d) = self.dots.iter_mut().find(|d| d.source == item) {
                        d.end = (d.end + extend).min(max_end).max(d.end.min(max_end));
                        continue;
                    }
                }
                // The bar drains 20% per second once the target goes 2 seconds without a new stack.
                let idle = self.t - self.procs[i].last_stack - BUILDUP_DECAY_DELAY;
                if idle > 0.0 {
                    self.procs[i].counter = (self.procs[i].counter - BUILDUP_DECAY_RATE * idle).max(0.0);
                }
                self.procs[i].last_stack = self.t;
            }
            self.procs[i].counter += amount;
            if self.procs[i].counter >= 1.0 - 1e-9 {
                self.fire(i);
            }
        }
    }

    /// Headshot debuffs use the same counter as headshot procs: one application per full headshot.
    fn count_headshot_rules(&mut self, headshot: f64) {
        for i in 0..self.rules.len() {
            if self.rules[i].on != On::Headshot || self.t < self.rules[i].ready_at {
                continue;
            }
            self.rules[i].counter += headshot;
            if self.rules[i].counter >= 1.0 - 1e-9 {
                self.rules[i].counter -= 1.0;
                let r = &self.rules[i];
                let (source, effect, seconds, max_stacks) = (r.source.clone(), r.effect, r.seconds, r.max_stacks);
                self.rules[i].ready_at = self.t + self.rules[i].cooldown;
                self.apply_debuff(&source, effect, seconds, max_stacks, 1);
            }
        }
    }

    fn shoot(&mut self, gun: &Gun, seconds: f64, headshot: f64) {
        if gun.base_shots_per_second <= 0.0 || gun.clip <= 0.0 {
            self.advance(self.t + seconds);
            return;
        }
        let (start, end) = (self.t, self.t + seconds);
        let (mut shots, mut reloads, mut damage) = (0u32, 0u32, 0.0);
        self.ammo = self.ammo.min(gun.clip);
        while self.t < end - 1e-9 {
            if self.reload_left > 0.0 || self.ammo < 1.0 {
                if self.reload_left <= 0.0 {
                    self.reload_left = gun.reload;
                    reloads += 1;
                }
                let wait = self.reload_left.min(end - self.t);
                self.advance(self.t + wait);
                self.reload_left -= wait;
                if self.reload_left <= 1e-9 {
                    self.reload_left = 0.0;
                    self.ammo = gun.clip;
                }
                continue;
            }
            let buff = self.buff();
            let per_pellet = gun.base_bullet * (1.0 + (gun.weapon_pct + buff.weapon_pct) / 100.0) + buff.flat_bullet;
            let mut extra_ammo = 0.0;
            match self.express {
                Some((bonus, cooldown, ammo, ready_at)) if self.t >= ready_at => {
                    // Two shots in quick succession, with bonus damage and without headshot damage.
                    let boosted = gun.base_bullet * (1.0 + (gun.weapon_pct + buff.weapon_pct + bonus) / 100.0) + buff.flat_bullet;
                    let dealt = self.deal("Express Shot", Kind::Bullet, boosted * gun.pellets * 2.0);
                    self.log("Express Shot", "double shot", Some(dealt));
                    self.express = Some((bonus, cooldown, ammo, self.t + cooldown));
                    extra_ammo = ammo;
                }
                _ => {
                    let raw = per_pellet * gun.pellets * (1.0 + headshot * (gun.crit - 1.0));
                    damage += self.deal("Weapon", Kind::Bullet, raw);
                }
            }
            if buff.spirit_per_shot > 0.0 {
                self.deal("Weapon (bonus spirit damage)", Kind::Spirit, buff.spirit_per_shot);
            }
            self.ammo -= 1.0 + extra_ammo;
            shots += 1;
            self.trigger_rules(On::Bullet, None, 1);
            self.count_headshot_rules(headshot);
            self.trigger_buffs(On::Bullet, None);
            self.trigger_heals(On::Bullet, None, 1.0);
            self.trigger_heals(On::Headshot, None, headshot);
            self.count_toward(|t| match t {
                Trigger::Headshot => Some(headshot),
                Trigger::Bullet { chance } => Some(*chance),
                Trigger::Buildup { per_shot } => Some(*per_shot),
                _ => None,
            });
            // Stacks gained by this shot count from the next one.
            let now = self.t;
            let mut stack_procs = vec![];
            for s in &mut self.stack_buffs {
                if s.end <= now {
                    s.stacks = 0.0;
                    s.gained = 0.0;
                }
                let gain = s.per_shot + headshot * s.per_headshot;
                s.stacks = (s.stacks + gain).min(s.max_stacks);
                s.end = now + s.seconds;
                if s.proc_every > 0.0 && s.proc_raw > 0.0 {
                    s.gained += gain;
                    while s.gained >= s.proc_every - 1e-9 {
                        s.gained -= s.proc_every;
                        stack_procs.push((s.source.clone(), s.proc_raw));
                    }
                }
            }
            for (source, raw) in stack_procs {
                let dealt = self.deal(&source, Kind::Spirit, raw);
                self.log(&source, "stack proc", Some(dealt));
            }
            self.ammo = self.ammo.min(gun.clip);
            let rate = gun.base_shots_per_second * (1.0 + (gun.fire_rate_pct + buff.fire_rate) / 100.0);
            self.advance((self.t + 1.0 / rate).min(end));
        }
        let now = self.t;
        self.t = start;
        let reload_note = if reloads > 0 { format!(", {reloads} reload(s)") } else { String::new() };
        self.log("Weapon", format!("{shots} shots over {}s{reload_note}", n(seconds)), Some(damage));
        self.t = now;
    }

    fn cast(&mut self, plan: &Plan, index: usize, ready_at: &mut [f64]) {
        let name = plan.name.as_str();
        if !plan.castable {
            self.log(name, "passive: it applies on its own, not as an action", None);
            return;
        }
        // A charged ability needs a ready charge and the wait between two casts to be over.
        let charged = plan.charges > 1 && index < self.charges.len();
        let mut available = ready_at[index];
        if charged {
            self.charges[index].resize(plan.charges as usize, 0.0);
            available = available.max(self.charges[index].iter().copied().fold(f64::INFINITY, f64::min));
        }
        if available > self.t + 1e-9 {
            let wait = available - self.t;
            self.log(name, format!("waited {}s for {}", n(wait), if charged { "a charge" } else { "cooldown" }), None);
            self.advance(available);
        }
        self.advance(self.t + plan.cast_delay);
        if charged {
            let now = self.t;
            let slots = &mut self.charges[index];
            // Charges come back one at a time: this one starts recharging after those already on their way.
            let queue = slots.iter().copied().filter(|ready| *ready > now).fold(now, f64::max);
            let used = (0..slots.len()).min_by(|a, b| slots[*a].total_cmp(&slots[*b])).unwrap_or(0);
            slots[used] = queue + plan.cooldown;
            ready_at[index] = now + plan.between;
        } else {
            ready_at[index] = self.t + plan.cooldown;
        }
        self.casting = Some(index);
        if index == 3 {
            self.trigger_buffs(On::Ultimate, None);
        }
        self.trigger_buffs(On::AbilityCast, None);
        self.trigger_heals(On::AbilityCast, None, 1.0);
        // Damage this ability deals from here on returns a share as health.
        self.leeches.retain(|(source, _)| source != name);
        let leech = plan.leech + self.imbue_leech.filter(|(i, _)| *i == index).map(|(_, pct)| pct).unwrap_or(0.0);
        if leech > 0.0 {
            self.leeches.push((name.to_string(), leech));
        }

        let per_hit: Vec<(String, Effect, f64, u32)> = plan
            .debuffs
            .iter()
            .filter(|d| d.per_hit)
            .map(|d| (format!("{name}: {}", d.label), d.effect, d.seconds, d.max_stacks))
            .collect();

        for part in &plan.parts {
            match part {
                Part::Instant { label, kind, raw, health, delay, low_hp, ramp } => {
                    let hit = Pending {
                        time: self.t + delay,
                        source: name.to_string(),
                        label: label.clone(),
                        kind: *kind,
                        raw: *raw,
                        health: *health,
                        low_hp: *low_hp,
                        ramp: *ramp,
                        quiet: false,
                        debuffs: vec![],
                    };
                    if *delay > 0.0 {
                        self.pending.push(hit);
                    } else {
                        self.land(hit);
                    }
                }
                Part::Dot { kind, dps, seconds, health, .. } => {
                    // Max-health damage is a fixed DPS for a given target; current-health damage shrinks as it ticks.
                    let flat = dps + health.max / 100.0 * self.health;
                    self.start_dot(name, *kind, flat, health.current, *seconds);
                }
                Part::Hits { label, kind, raw, count, spread, falloff, health } => {
                    if *spread > 0.0 {
                        self.log(name, format!("{label}: {count} hits × {} over {}s", n(*raw), n(*spread)), None);
                    }
                    let mut dealt = 0.0;
                    for i in 0..*count {
                        let raw = if i == 0 { *raw } else { raw * falloff };
                        if *spread > 0.0 {
                            self.pending.push(Pending {
                                time: self.t + spread * (i + 1) as f64 / *count as f64,
                                source: name.to_string(),
                                label: label.clone(),
                                kind: *kind,
                                raw,
                                health: *health,
                                low_hp: None,
                                ramp: None,
                                quiet: true,
                                debuffs: per_hit.clone(),
                            });
                        } else {
                            let raw = self.with_health(raw, health);
                            dealt += self.deal(name, *kind, raw);
                            for (source, effect, seconds, max_stacks) in &per_hit {
                                self.apply_debuff(source, *effect, *seconds, *max_stacks, 1);
                            }
                        }
                    }
                    if *spread <= 0.0 {
                        self.log(name, format!("{label}: {count} hits × {}", n(*raw)), Some(dealt));
                    }
                }
            }
        }
        for d in plan.debuffs.iter().filter(|d| !d.per_hit) {
            self.apply_debuff(&format!("{name}: {}", d.label), d.effect, d.seconds, d.max_stacks, 1);
        }
        self.apply_buff(name, plan.buff, plan.buff_seconds);
        let hits: u32 = plan
            .parts
            .iter()
            .map(|p| match p {
                Part::Hits { count, .. } => *count,
                Part::Instant { .. } => 1,
                Part::Dot { .. } => 0,
            })
            .sum();
        for h in plan.heals.iter().filter(|h| !h.on_kill) {
            self.apply_heal(name, h, if h.per_hit { hits as f64 } else { 1.0 });
        }
        if let Some((target, per_hit)) = &plan.feeds {
            let landed: u32 = plan
                .parts
                .iter()
                .map(|p| match p {
                    Part::Hits { count, .. } => *count,
                    Part::Instant { .. } => 1,
                    Part::Dot { .. } => 0,
                })
                .sum();
            self.add_buildup(target, per_hit * landed as f64);
        }

        if !plan.has_damage() {
            if plan.debuffs.is_empty() && plan.buff.is_empty() && plan.heals.is_empty() {
                self.log(name, "no damage to simulate", None);
            }
            return;
        }
        for i in 0..self.procs.len() {
            let p = &self.procs[i];
            if p.trigger == Trigger::AbilityHit && self.t >= p.ready_at && p.only_ability.is_none_or(|a| a == index) {
                self.fire(i);
            }
        }
        if index == 3 {
            self.trigger(|t| matches!(t, Trigger::Ultimate { .. }));
        }
        if plan.melee_hit {
            self.trigger(|t| matches!(t, Trigger::Melee));
            self.trigger_rules(On::Melee, None, 1);
            self.last_melee = 0.0;
            self.trigger_heals(On::Melee, None, 1.0);
        }
    }

    /// Adds to the buildup bar of a passive from something other than a shot.
    fn add_buildup(&mut self, source: &str, amount: f64) {
        for i in 0..self.procs.len() {
            let p = &self.procs[i];
            if p.item != source || !matches!(p.trigger, Trigger::Buildup { .. }) || self.t < p.ready_at {
                continue;
            }
            self.procs[i].last_stack = self.t;
            self.procs[i].counter += amount;
            if self.procs[i].counter >= 1.0 - 1e-9 {
                self.fire(i);
            }
        }
    }

    /// Registers the always-on parts of an ability: procs, stacking buffs and on-hit debuffs.
    fn register_passives(&mut self, plan: &Plan) {
        let name = plan.name.as_str();
        for h in plan.heals.iter().filter(|h| h.on_kill) {
            self.kill_heals.push((name.to_string(), h.clone()));
        }
        for passive in &plan.passives {
            match passive {
                Passive::HeadshotProc { kind, raw, cooldown, debuff } => {
                    let mut p = Proc::new(name, Trigger::Headshot, *kind, *raw, *cooldown);
                    p.debuff = *debuff;
                    self.procs.push(p);
                }
                Passive::BuildupDot { per_shot, dps, seconds, extend_per_shot, extend_per_headshot, max_seconds } => {
                    let mut p = Proc::new(name, Trigger::Buildup { per_shot: *per_shot }, Kind::Spirit, 0.0, 0.0);
                    p.dot = Some((*dps, *seconds));
                    p.extend_per_shot = *extend_per_shot;
                    p.extend_per_headshot = *extend_per_headshot;
                    p.max_seconds = *max_seconds;
                    self.procs.push(p);
                }
                Passive::StackingBuff { per_stack, max_stacks, seconds, per_shot, per_headshot, proc_every, proc_raw } => {
                    self.stack_buffs.push(StackBuff {
                        source: name.to_string(),
                        proc_every: *proc_every,
                        proc_raw: *proc_raw,
                        gained: 0.0,
                        per_stack: *per_stack,
                        stacks: 0.0,
                        max_stacks: *max_stacks,
                        seconds: *seconds,
                        per_shot: *per_shot,
                        per_headshot: *per_headshot,
                        end: 0.0,
                    });
                }
                Passive::Always { buff } => self.apply_buff(name, *buff, f64::INFINITY),
                Passive::BuildupProc { per_shot, raw, cooldown, ramp } => {
                    let mut p = Proc::new(name, Trigger::Buildup { per_shot: *per_shot }, Kind::Spirit, *raw, *cooldown);
                    p.ramp = *ramp;
                    self.procs.push(p);
                }
                Passive::BulletDebuff { effect, max_stacks, seconds } => self.rules.push(Rule {
                    item: name.to_string(),
                    source: name.to_string(),
                    on: On::Bullet,
                    effect: *effect,
                    seconds: *seconds,
                    max_stacks: *max_stacks,
                    cooldown: 0.0,
                    ready_at: 0.0,
                    counter: 0.0,
                }),
                Passive::MeleeDebuff { effect, seconds } => self.rules.push(Rule {
                    item: name.to_string(),
                    source: name.to_string(),
                    on: On::Melee,
                    effect: *effect,
                    seconds: *seconds,
                    max_stacks: 1,
                    cooldown: 0.0,
                    ready_at: 0.0,
                    counter: 0.0,
                }),
            }
        }
    }
}

fn use_item(sim: &mut Sim, actives: &mut [Active], item: &str) {
    let Some(a) = actives.iter_mut().find(|a| a.item == item) else {
        sim.log(item, "not owned or not usable in a combo", None);
        return;
    };
    if a.ready_at > sim.t + 1e-9 {
        let wait = a.ready_at - sim.t;
        sim.log(&a.name, format!("waited {}s for cooldown", n(wait)), None);
        sim.advance(a.ready_at);
    }
    a.ready_at = sim.t + a.cooldown;
    if a.damage > 0.0 {
        let dealt = sim.deal(&a.name, Kind::Spirit, a.damage);
        sim.log(&a.name, "active", Some(dealt));
    }
    if a.dot_dps > 0.0 || a.dot_pct_current > 0.0 {
        sim.start_dot(&a.name, Kind::Spirit, a.dot_dps, a.dot_pct_current, a.dot_seconds);
    }
    let name = a.name.clone();
    sim.trigger_rules(On::Use, Some(&name), 1);
    let delay = sim.buff_rules.iter().find(|r| r.on == On::Use && r.item == name).map(|r| r.delay).unwrap_or(0.0);
    if delay > 0.0 {
        sim.log(&name, format!("unable to act for {}s", n(delay)), None);
        sim.advance(sim.t + delay);
    }
    sim.trigger_buffs(On::Use, Some(&name));
    sim.trigger_heals(On::Use, Some(&name), 1.0);
}

/// The target the combo runs against: the enemy hero's build when one is set, else the build's own target,
/// then the resists the zone adds.
pub fn resolve_target(data: &GameData, b: &BuildInput, combo: &ComboInput) -> Result<TargetInfo, String> {
    let mut t = TargetInfo {
        health: b.target.health,
        bullet_resist: b.target.bullet_resist,
        spirit_resist: b.target.spirit_resist,
        hero: None,
        notes: vec![],
    };
    if let Some(enemy) = combo.enemy.as_ref().filter(|e| !e.hero.is_empty()) {
        let hero = data.hero(&enemy.hero).ok_or_else(|| format!("Unknown enemy hero: {}", enemy.hero))?;
        let sheet = calc::compute(data, enemy)?;
        let stat = |key: &str| sheet.stat(key).unwrap_or(0.0);
        t.health = stat("max_health").max(1.0);
        t.bullet_resist = stat("bullet_resist");
        t.spirit_resist = stat("spirit_resist");
        t.hero = Some(hero.name.clone());
        let items = sheet.owned.iter().filter_map(|c| data.item(c)).map(|i| i.name.as_str()).collect::<Vec<_>>();
        t.notes.push(format!(
            "{} at {} souls with {}: {} health, {}% bullet resist, {}% spirit resist",
            hero.name,
            n(enemy.souls.unwrap_or(0).max(sheet.spent) as f64),
            if items.is_empty() { "no items".to_string() } else { items.join(", ") },
            n(t.health.round()),
            n(t.bullet_resist),
            n(t.spirit_resist)
        ));
        let actives = sheet.owned.iter().filter_map(|c| data.item(c)).filter(|i| i.active).map(|i| i.name.as_str()).collect::<Vec<_>>();
        if !actives.is_empty() {
            t.notes.push(format!("The enemy's active items are assumed unused: {}", actives.join(", ")));
        }
    }
    let z = &combo.zone;
    if z.rift {
        let rift = crate::misc::rift_comeback(z.minute, z.own_souls, z.enemy_souls);
        if rift.resist <= 0.0 {
            t.notes.push("Inside the rift: the teams are even, so neither gets the comeback resist".into());
        } else if rift.own_team_trails {
            t.notes.push(format!(
                "Inside the rift: your team is {}% behind and gets {}% bullet, spirit and debuff resist; it protects you, not the target",
                n(rift.behind_pct),
                n(rift.resist)
            ));
        } else {
            t.bullet_resist = crate::misc::stack_resist(t.bullet_resist, rift.resist);
            t.spirit_resist = crate::misc::stack_resist(t.spirit_resist, rift.resist);
            t.notes.push(format!(
                "Inside the rift: the enemy team is {}% behind at minute {}, so the target gets {}% bullet and spirit resist (stacked with its own)",
                n(rift.behind_pct),
                n(z.minute),
                n(rift.resist)
            ));
        }
    }
    if z.enemy_walker {
        let aura = crate::misc::WALKER_AURA_RESIST;
        t.bullet_resist = crate::misc::stack_resist(t.bullet_resist, aura);
        t.spirit_resist = crate::misc::stack_resist(t.spirit_resist, aura);
        t.notes.push(format!(
            "Near its own Walker: {}% bullet and spirit resist within {} m (stacked with its own)",
            n(aura),
            n(crate::misc::WALKER_AURA_RADIUS_M)
        ));
    }
    Ok(t)
}

pub fn simulate(data: &GameData, b: &BuildInput, combo: &ComboInput) -> Result<ComboResult, String> {
    let target = resolve_target(data, b, combo)?;
    // Conditional item effects are driven by the combo itself (events and condition actions),
    // so the switches set on the build are ignored here.
    let b = &BuildInput {
        active: vec![],
        target: crate::build::Target { health: target.health, bullet_resist: target.bullet_resist, spirit_resist: target.spirit_resist },
        ..b.clone()
    };
    let sheet = calc::compute(data, b)?;
    let hero = data.hero(&b.hero).ok_or_else(|| format!("Unknown hero: {}", b.hero))?;
    let stat = |key: &str| sheet.stat(key).unwrap_or(0.0);

    let (mut procs, mut actives, mut passives, mut unsupported) = item_effects(&sheet, b.target.health, &b.imbues);
    let (mut buff_rules, buff_procs, buff_passives) = buff_rules(data, &sheet);
    procs.extend(buff_procs);
    passives.extend(buff_passives);
    let (imbue_rules, imbue_passives) = imbue_buff_rules(data, &sheet, b);
    buff_rules.extend(imbue_rules);
    passives.extend(imbue_passives);
    let mut conditions = conditions(data, &sheet);
    conditions.extend(kit::hero_conditions(&sheet.abilities).into_iter().map(|c| Condition {
        item: c.id,
        name: c.name,
        buff: c.buff,
        effect: c.effect,
        seconds: f64::INFINITY,
        note: Some(c.text),
        max_count: c.max,
        unit: c.unit,
    }));
    // An item offered as a condition is covered, even if its effect has no dedicated simulation.
    unsupported.retain(|name| !conditions.iter().any(|c| &c.name == name));
    let (rules, rule_passives) = debuff_rules(data, &sheet);
    passives.extend(rule_passives);
    let (heal_rules, heal_passives) = heal_rules(&sheet);
    passives.extend(heal_passives);
    // Items used only for their heal.
    for r in heal_rules.iter().filter(|r| r.on == On::Use) {
        if let Some(e) = sheet.item_effects.iter().find(|e| e.name == r.item) {
            actives.push(Active {
                item: e.item.clone(),
                name: e.name.clone(),
                damage: 0.0,
                dot_dps: 0.0,
                dot_pct_current: 0.0,
                dot_seconds: 0.0,
                cooldown: val(&e.props, "AbilityCooldown").unwrap_or(0.0),
                ready_at: 0.0,
            });
        }
    }
    let usable_items = actives
        .iter()
        .map(|a| UsableItem { item: a.item.clone(), name: a.name.clone(), description: describe(a, &rules, &buff_rules, &heal_rules), condition: false, max_count: None, unit: String::new() })
        .chain(conditions.iter().map(|c| UsableItem {
            item: c.item.clone(),
            name: c.name.clone(),
            description: c.text(),
            condition: true,
            max_count: c.max_count,
            unit: c.unit.clone(),
        }))
        .collect();

    let weapon_pct = stat("weapon_damage_pct");
    let fire_rate_pct = stat("fire_rate_pct");
    let gun = Gun {
        base_bullet: stat("bullet_damage") / (1.0 + weapon_pct / 100.0),
        weapon_pct,
        pellets: hero.weapon.pellets,
        base_shots_per_second: stat("shots_per_second") / (1.0 + fire_rate_pct / 100.0),
        fire_rate_pct,
        clip: stat("clip_size"),
        reload: stat("reload_time"),
        crit: stat("crit_multiplier").max(1.0),
    };
    let headshot = (combo.headshot_rate / 100.0).clamp(0.0, 1.0);
    let ctx = Context {
        per_shot: stat("bullet_damage") * hero.weapon.pellets,
        shots_per_second: stat("shots_per_second"),
        abilities: &sheet.abilities,
        flags: &[],
    };
    // Express Shot: every few seconds one attack fires twice with bonus damage and costs extra ammo.
    let express = sheet.owned.iter().filter_map(|class| data.item(class)).find(|i| i.name == "Express Shot").map(|item| {
        let prop = |key: &str| item.props.iter().find(|p| p.key == key);
        let bonus = prop("ProcBaseAttackDamagePercent")
            .map(|p| p.value + p.scale.as_ref().map(|s| s.coef).unwrap_or(0.0) * sheet.boons as f64)
            .unwrap_or(0.0);
        let value = |key: &str| prop(key).map(|p| p.value).unwrap_or(0.0);
        (bonus, value("AbilityCooldown"), value("ProcAmmoConsumed"), 0.0)
    });
    if let Some((bonus, cooldown, ammo, _)) = express {
        passives.push(format!(
            "Express Shot: every {}s one attack fires twice with +{}% weapon damage, uses {} extra ammo and cannot headshot",
            n(cooldown),
            n(bonus),
            n(ammo)
        ));
    }
    // --- The hero's own healing ---
    let heal_amp: f64 = sheet
        .owned
        .iter()
        .filter_map(|class| data.item(class))
        .flat_map(|item| item.props.iter())
        .filter(|p| p.modifier.as_deref() == Some("MODIFIER_VALUE_HEAL_AMP_CAST_PERCENT") && p.value > 0.0)
        .map(|p| p.value)
        .sum();
    if heal_amp > 0.0 {
        passives.push(format!("Healing effectiveness: +{}% to heals and regeneration (assumed not to affect lifesteal)", n(heal_amp)));
    }
    let base_lifesteal = (stat("bullet_lifesteal"), stat("spirit_lifesteal"));
    let lifesteal_scale = if hero.name == "Rat King" { 0.75 } else { 1.0 };
    if base_lifesteal.0 > 0.0 || base_lifesteal.1 > 0.0 {
        let scale = if lifesteal_scale < 1.0 { format!(" at {}% effectiveness for this hero", n(lifesteal_scale * 100.0)) } else { String::new() };
        passives.push(format!(
            "Lifesteal: {}% of bullet damage and {}% of spirit damage dealt returns as health{scale}",
            n(base_lifesteal.0),
            n(base_lifesteal.1)
        ));
    }
    let imbue_leech = sheet
        .owned
        .iter()
        .filter_map(|class| Some((class, data.item(class)?)))
        .find(|(_, item)| item.name == "Mystic Reverb")
        .and_then(|(class, item)| Some((*b.imbues.get(class)?, item.props.iter().find(|p| p.key == "ImbueAbilityLifesteal")?.value)));
    if let Some((index, pct)) = imbue_leech {
        let ability = sheet.abilities.get(index).map(|a| a.name.as_str()).unwrap_or("?");
        passives.push(format!("Mystic Reverb: {ability} heals you for {}% of the damage it deals", n(pct)));
    }
    let own_max = stat("max_health");
    let own_start = own_max * combo.own_health.unwrap_or(50.0).clamp(0.0, 100.0) / 100.0;
    let mut sim = Sim {
        t: 0.0,
        health: b.target.health,
        dealt: 0.0,
        bullet_resist: sheet.stat("target_bullet_resist").unwrap_or(b.target.bullet_resist),
        spirit_resist: sheet.stat("target_spirit_resist").unwrap_or(b.target.spirit_resist),
        kill_time: None,
        dots: vec![],
        pending: vec![],
        procs,
        rules,
        debuffs: vec![],
        buffs: vec![],
        stack_buffs: vec![],
        next_tick: vec![],
        spirit_progress: 0.0,
        sources: vec![],
        events: vec![],
        ammo: gun.clip,
        reload_left: 0.0,
        headshot,
        heavy_melee: false,
        heavy_bonus: sheet
            .item_effects
            .iter()
            .filter_map(|e| Some((e.name.clone(), val(&e.props, "BonusHeavyMeleeDamage")?, val(&e.props, "AbilityCooldown").unwrap_or(0.0), 0.0)))
            .collect(),
        buff_rules,
        casting: None,
        express,
        own_max,
        own_hp: own_start,
        healed: 0.0,
        heals: vec![],
        regens: vec![],
        heal_rules,
        kill_heals: vec![],
        heal_amp,
        base_regen: stat("health_regen") * (1.0 + heal_amp / 100.0),
        base_lifesteal,
        lifesteal_scale,
        leeches: vec![],
        imbue_leech,
        siphoned: 0.0,
        last_melee: 0.0,
        outgoing: stat("outgoing_damage_pct"),
        charges: vec![vec![]; sheet.abilities.len()],
    };
    for item in sheet.owned.iter().filter_map(|class| data.item(class)).filter(|i| i.name == "Hunter's Aura") {
        let shred = item.props.iter().find(|p| p.key == "BulletArmorReduction").map(|p| p.value.abs()).unwrap_or(0.0) * 2.0;
        if shred > 0.0 {
            sim.debuffs.push(Debuff {
                source: item.name.clone(),
                per_stack: Effect { bullet: shred, ..Default::default() },
                stacks: 1,
                end: f64::INFINITY,
            });
            passives.push(format!("Hunter's Aura: -{}% bullet resist on the target while it is near (doubled: it is the only enemy)", n(shred)));
        }
    }
    if combo.zone.own_walker {
        let shred = crate::misc::WALKER_FIREBALL_SHRED;
        sim.debuffs.push(Debuff {
            source: "Walker fireballs".into(),
            per_stack: Effect { bullet: shred, spirit: shred, ..Default::default() },
            stacks: 1,
            end: f64::INFINITY,
        });
        passives.push(format!(
            "Your Walker: its fireballs keep the target at -{}% bullet and spirit resist (assumed to hit it the whole fight); the Walker's own damage is not counted",
            n(shred)
        ));
    }
    if sim.outgoing != 0.0 {
        passives.push(format!("Outgoing damage: {}{}% to all damage dealt", if sim.outgoing > 0.0 { "+" } else { "" }, n(sim.outgoing)));
    }
    for (name, pct, cooldown, _) in &sim.heavy_bonus {
        passives.push(format!("{name}: the next heavy melee deals +{}% damage, {}s cooldown", n(*pct), n(*cooldown)));
    }

    // The default plan of each ability, for passives and for the summary shown to the user.
    let abilities: Vec<AbilityInfo> = sheet
        .abilities
        .iter()
        .map(|a| {
            let plan = kit::plan(a, &ctx, None, None);
            sim.register_passives(&plan);
            AbilityInfo { name: plan.name.clone(), castable: plan.castable, lines: plan.describe() }
        })
        .collect();
    sim.next_tick = sim
        .procs
        .iter()
        .map(|p| match p.trigger {
            Trigger::Periodic { interval } => interval,
            _ => f64::INFINITY,
        })
        .collect();
    let mut ability_ready = vec![0.0; sheet.abilities.len()];
    // Hero conditions switched on so far.
    let mut flags: Vec<(String, f64)> = vec![];

    for action in &combo.actions {
        match action {
            Action::Shoot { seconds } => sim.shoot(&gun, seconds.clamp(0.0, 120.0), headshot),
            Action::Wait { seconds } => {
                let seconds = seconds.clamp(0.0, 120.0);
                sim.log("Wait", format!("{}s", n(seconds)), None);
                sim.advance(sim.t + seconds);
            }
            Action::Melee { heavy } => {
                let (name, key, time) = if *heavy {
                    ("Heavy melee", "heavy_melee", HEAVY_MELEE_TIME)
                } else {
                    ("Light melee", "light_melee", LIGHT_MELEE_TIME)
                };
                sim.advance(sim.t + time);
                sim.heavy_melee = *heavy;
                let mut raw = stat(key) * (1.0 + sim.buff().melee_pct / 100.0);
                if *heavy {
                    let now = sim.t;
                    for (_, pct, cooldown, ready_at) in &mut sim.heavy_bonus {
                        if now >= *ready_at {
                            raw *= 1.0 + *pct / 100.0;
                            *ready_at = now + *cooldown;
                        }
                    }
                }
                let dealt = sim.deal(name, Kind::Melee, raw);
                sim.log(name, "hit", Some(dealt));
                sim.last_melee = dealt;
                let bonus = sim.buff().heavy_melee_spirit;
                if *heavy && bonus > 0.0 {
                    let dealt = sim.deal("Heavy melee (bonus spirit damage)", Kind::Spirit, bonus);
                    sim.log("Heavy melee (bonus spirit damage)", "hit", Some(dealt));
                }
                sim.trigger(|t| matches!(t, Trigger::Melee));
                sim.trigger_rules(On::Melee, None, if *heavy { 2 } else { 1 });
                sim.trigger_buffs(On::Melee, None);
                sim.trigger_heals(On::Melee, None, 1.0);
            }
            Action::Ability { index, duration, hits } => {
                // An active spirit power buff raises the damage of abilities cast while it lasts.
                let buff = sim.buff();
                let boosted: Option<Vec<AbilitySheet>> = if buff.spirit_power != 0.0 || buff.spirit_power_pct != 0.0 {
                    let with_buff = BuildInput { bonus_spirit: buff.spirit_power, bonus_spirit_pct: buff.spirit_power_pct, ..b.clone() };
                    Some(calc::compute(data, &with_buff)?.abilities)
                } else {
                    None
                };
                let abilities = boosted.as_deref().unwrap_or(&sheet.abilities);
                let cast_ctx = Context { per_shot: ctx.per_shot, shots_per_second: ctx.shots_per_second, abilities, flags: &flags };
                if let Some(ability) = abilities.get(*index) {
                    let plan = kit::plan(ability, &cast_ctx, *duration, *hits);
                    sim.cast(&plan, *index, &mut ability_ready);
                }
            }
            Action::Item { item } => {
                use_item(&mut sim, &mut actives, item);
                if data.item(item).is_some_and(|i| i.name == "Echo Shard") {
                    match b.imbues.get(item).filter(|i| **i < ability_ready.len()) {
                        Some(&i) => {
                            ability_ready[i] = sim.t;
                            sim.log("Echo Shard", format!("reset the cooldown of {}", sheet.abilities[i].name), None);
                        }
                        None => sim.log("Echo Shard", "not imbued into an ability", None),
                    }
                }
            }
            Action::Condition { item, count } => match conditions.iter().find(|c| &c.item == item) {
                Some(c) => {
                    // A counted condition multiplies its per-stack bonus; a plain one counts once.
                    let stacks = match c.max_count {
                        Some(max) => count.unwrap_or(1.0).round().clamp(0.0, max),
                        None => 1.0,
                    };
                    if c.item.starts_with("hero:") {
                        flags.retain(|(f, _)| f != &c.item);
                        flags.push((c.item.clone(), stacks));
                    }
                    let counted = c.max_count.map(|_| format!("{} {}: ", n(stacks), c.unit)).unwrap_or_default();
                    sim.log(&c.name, format!("condition on: {counted}{}", c.text()), None);
                    let mut buff = Buff::default();
                    buff.add(&c.buff, stacks);
                    sim.debuffs.retain(|d| d.source != c.name);
                    sim.buffs.retain(|b| b.source != c.name);
                    if !buff.is_empty() {
                        sim.buffs.push(ActiveBuff { source: c.name.clone(), buff, end: sim.t + c.seconds });
                    }
                    if !c.effect.is_empty() && stacks > 0.0 {
                        sim.debuffs.push(Debuff { source: c.name.clone(), per_stack: c.effect, stacks: stacks as u32, end: sim.t + c.seconds.min(1e9) });
                    }
                }
                None => sim.log(item, "no conditional bonus to switch on", None),
            },
        }
    }

    // Let damage-over-time effects and delayed hits run out.
    let duration = sim.t;
    let tail = sim
        .dots
        .iter()
        .map(|d| d.end)
        .chain(sim.pending.iter().map(|p| p.time))
        .fold(duration, f64::max);
    for i in 0..sim.next_tick.len() {
        sim.next_tick[i] = f64::INFINITY; // periodic procs stop with the last action
    }
    sim.advance(tail);
    // Heals over time still running are credited in full; they do not lengthen the combo.
    let rest: Vec<(String, f64)> = sim.regens.iter().map(|r| (r.source.clone(), r.per_second * (r.end - sim.t).max(0.0))).collect();
    for (source, amount) in rest {
        sim.heal_quiet(&source, amount);
    }
    sim.heals.sort_by(|a, b| b.amount.total_cmp(&a.amount));

    sim.sources.sort_by(|a, b| b.damage.total_cmp(&a.damage));
    sim.events.sort_by(|a, b| a.time.total_cmp(&b.time));
    Ok(ComboResult {
        total: sim.dealt,
        duration,
        full_duration: sim.t,
        dps: if sim.t > 0.0 { sim.dealt / sim.t } else { 0.0 },
        target_health: sim.health,
        target,
        kill_time: sim.kill_time,
        sources: sim.sources,
        events: sim.events,
        abilities,
        passives,
        usable_items,
        unsupported,
        healing: sim.healed,
        heal_sources: sim.heals,
        own_max_health: own_max,
        own_health_start: own_start,
        own_health_end: sim.own_hp,
    })
}
