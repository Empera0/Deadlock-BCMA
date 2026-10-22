//! Stat calculation. Every stat carries its formula and contributing terms next to its value.

use crate::build::{resolve, BuildInput, Step, SLOT_LIMIT};
use crate::data::{Ability, GameData, Hero, InvestTier, Prop, Scale};
use serde::Serialize;
use std::collections::BTreeMap;

const AP_COST: [u32; 3] = [1, 2, 5];

#[derive(Debug, Clone, Serialize)]
pub struct Term {
    pub label: String,
    pub value: f64,
    pub unit: String,
}

fn term(label: impl Into<String>, value: f64, unit: &str) -> Term {
    Term { label: label.into(), value, unit: unit.to_string() }
}

#[derive(Debug, Clone, Serialize)]
pub struct Stat {
    pub key: String,
    pub label: String,
    /// `weapon` | `vitality` | `spirit` | `target` | `other`.
    pub group: String,
    pub value: f64,
    pub unit: String,
    pub formula: String,
    pub terms: Vec<Term>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ScaledProp {
    pub key: String,
    pub label: String,
    pub value: f64,
    pub unit: String,
    pub formula: String,
    /// Value after the target's spirit resist (spirit damage only).
    pub vs_target: Option<f64>,
    /// `spirit`, `bullet` or `melee` when the value is damage.
    pub damage_kind: Option<String>,
    /// The value before scaling, and the scaling coefficient (0 when it does not scale).
    pub base: f64,
    pub coef: f64,
    /// `MODIFIER_VALUE_*` when the value is a stat or debuff the ability applies.
    pub modifier: Option<String>,
    pub note: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct AbilitySheet {
    pub class_name: String,
    pub name: String,
    pub icon: String,
    pub tier: u8,
    pub max_tier: u8,
    pub props: Vec<ScaledProp>,
    /// Names of the items imbued into this ability.
    pub imbued: Vec<String>,
    /// The hero's own ability in this slot; differs from `class_name` when an ultimate is copied.
    pub slot_class: String,
    /// The game's own description of the ability and its one-line summary.
    pub description: String,
    pub quip: String,
    /// What each upgrade tier changes, in words, in tier order.
    pub upgrade_notes: Vec<String>,
}

/// Sinclair's ultimate: it becomes a copy of another hero's ultimate.
pub const COPY_ULTIMATE: &str = "ability_magician_copyult";

#[derive(Debug, Clone, Serialize)]
pub struct ItemEffects {
    pub item: String,
    pub name: String,
    pub props: Vec<ScaledProp>,
}

#[derive(Debug, Clone, Serialize)]
pub struct Sheet {
    pub hero: String,
    pub level: u32,
    pub boons: u32,
    pub souls: u32,
    pub spent: u32,
    pub spent_weapon: u32,
    pub spent_vitality: u32,
    pub spent_spirit: u32,
    pub ability_points: u32,
    pub ability_points_used: u32,
    pub slots_used: usize,
    pub slot_limit: usize,
    pub owned: Vec<String>,
    pub steps: Vec<Step>,
    pub errors: Vec<String>,
    pub stats: Vec<Stat>,
    pub abilities: Vec<AbilitySheet>,
    pub item_effects: Vec<ItemEffects>,
}

impl Sheet {
    pub fn stat(&self, key: &str) -> Option<f64> {
        self.stats.iter().find(|s| s.key == key).map(|s| s.value)
    }
}

/// `MODIFIER_VALUE_*` -> contributions.
#[derive(Default)]
struct Bag {
    map: BTreeMap<String, Vec<Term>>,
    labels: BTreeMap<String, String>,
}

impl Bag {
    fn push(&mut self, modifier: &str, source: &str, value: f64, label: &str, unit: &str) {
        self.map.entry(modifier.to_string()).or_default().push(term(source, value, unit));
        if !label.is_empty() {
            self.labels.entry(modifier.to_string()).or_insert_with(|| label.to_string());
        }
    }

    /// The sum collected for a modifier so far, without taking it.
    fn peek(&self, modifier: &str) -> f64 {
        self.map.get(modifier).map(|terms| total(terms)).unwrap_or(0.0)
    }

    fn take(&mut self, modifiers: &[&str]) -> Vec<Term> {
        modifiers.iter().filter_map(|m| self.map.remove(*m)).flatten().collect()
    }
}

fn total(terms: &[Term]) -> f64 {
    // +0.0 keeps an empty sum from being -0.0
    terms.iter().map(|t| t.value).sum::<f64>() + 0.0
}

/// Resists combine multiplicatively: 1 − Π(1 − rᵢ).
/// Fire rate with its bonuses and slows (deadlock.wiki, Fire Rate): bonuses add up, slows combine by what each
/// lets through, and the total is the bonuses less the combined slow. A positive total raises the shots per second;
/// a negative one lengthens the time between shots instead, and is never below -50%.
pub fn fire_rate(base: f64, bonus_pct: f64, slow_pct: f64) -> f64 {
    let total = (bonus_pct - slow_pct) / 100.0;
    if total >= 0.0 {
        base * (1.0 + total)
    } else {
        base / (1.0 + total.abs().min(MAX_FIRE_RATE_SLOW))
    }
}

/// Heroes whose headshots deal less than the usual +65%: their crit bonus scale, percent (deadlock.wiki, Weapon
/// Damage). The game data the app reads does not carry it.
const CRIT_BONUS_SCALE: [(&str, f64); 9] = [
    ("hero_baba", -20.0),
    ("hero_punkgoat", -20.0),
    ("hero_unicorn", -25.0),
    ("hero_doorman", -30.0),
    ("hero_drifter", -45.0),
    ("hero_kelvin", -25.0),
    ("hero_familiar", -20.0),
    ("hero_viper", -30.0),
    ("hero_artist", -20.0),
];
/// Heroes whose weapon cannot deal critical damage (Paige can still trigger headshot items).
const NO_CRIT: [&str; 3] = ["hero_deadpack", "hero_necro", "hero_bookworm"];

/// The weapon's headshot multiplier for a hero: 1 + (0.65 + crit bonus scale), or the weapon's own when it differs.
pub fn crit_multiplier(hero: &str, weapon: f64) -> f64 {
    if NO_CRIT.contains(&hero) {
        return 1.0;
    }
    match CRIT_BONUS_SCALE.iter().find(|(h, _)| *h == hero) {
        Some((_, scale)) => 1.0 + (0.65 + scale / 100.0),
        None => weapon,
    }
}

/// Powerups (deadlock.wiki, Powerups): (at minute 5, at minute 40 and later).
const POWERUP_FROM_MINUTE: f64 = 5.0;
const POWERUP_FULL_MINUTE: f64 = 40.0;
const POWERUP_CASTING_SPIRIT: (f64, f64) = (15.0, 65.0);
const POWERUP_CASTING_CDR: (f64, f64) = (12.0, 20.0);
const POWERUP_GUN_FIRE_RATE: (f64, f64) = (12.0, 35.0);
const POWERUP_GUN_AMMO: (f64, f64) = (35.0, 70.0);
/// Permanent statue buffs (deadlock.wiki, Permanent Buff): per buff, before minute 10, from 10, from 30.
const STATUE_FIRE_RATE: [f64; 3] = [1.5, 2.0, 2.5];
const STATUE_AMMO: [f64; 3] = [3.0, 5.0, 7.0];
const STATUE_CDR: [f64; 3] = [0.5, 0.75, 1.0];
const STATUE_WEAPON: [f64; 3] = [3.0, 4.0, 6.0];
const STATUE_HEALTH: [f64; 3] = [15.0, 20.0, 30.0];
const STATUE_SPIRIT: [f64; 3] = [2.0, 3.0, 4.0];
const STATUE_RESIST: [f64; 3] = [0.5, 0.75, 1.0];

/// Share of the weapon damage bonus that melee damage gets.
pub const MELEE_WEAPON_SHARE: f64 = 0.5;

/// The largest fire rate penalty, as a share.
pub const MAX_FIRE_RATE_SLOW: f64 = 0.5;

fn combine_resist(terms: &[Term]) -> f64 {
    100.0 * (1.0 - terms.iter().map(|t| 1.0 - t.value / 100.0).product::<f64>())
}

fn invest_bonus(tiers: &[InvestTier], spent: u32) -> f64 {
    tiers
        .iter()
        .filter(|t| t.threshold <= spent)
        .map(|t| t.bonus)
        .fold(0.0, f64::max)
}

enum ScaleKind {
    Spirit,
    Boons,
    WeaponDamage,
    LightMelee,
    HeavyMelee,
    Unsupported,
}

fn scale_kind(s: &Scale) -> ScaleKind {
    match (s.stat.as_str(), s.class.as_str()) {
        ("ETechPower", _) => ScaleKind::Spirit,
        ("ELevelUpBoons", _) => ScaleKind::Boons,
        ("ELightMeleeDamage", _) => ScaleKind::LightMelee,
        ("EHeavyMeleeDamage", _) => ScaleKind::HeavyMelee,
        // Kinetic Carbine multiplies bullet damage instead; the combo simulation handles it.
        (_, "scale_function_kinetic_carbine_damage") => ScaleKind::Unsupported,
        ("EBaseWeaponDamageIncrease" | "EWeaponPower", _) => ScaleKind::WeaponDamage,
        ("", "scale_function_tech_damage" | "scale_function_healing_spirit_scale") => ScaleKind::Spirit,
        ("", "scale_function_healing_boon_scale") => ScaleKind::Boons,
        _ => ScaleKind::Unsupported,
    }
}

#[derive(Clone, Copy)]
struct Ctx {
    spirit_power: f64,
    boons: f64,
    cooldown_reduction: f64,
    /// Applies to the ultimate on top of the general cooldown reduction.
    ult_cooldown_reduction: f64,
    /// Shortens the time between charges of a charged ability.
    charge_reduction: f64,
    /// Extra charges for abilities that already have charges.
    bonus_charges: f64,
    /// Cooldown reduction and spirit power that only charged abilities get (Omnicharge Signet).
    charged_cdr: f64,
    charged_spirit: f64,
    duration_bonus: f64,
    /// Ability range and ability radius bonuses, percent: sizes marked to grow with them.
    range_bonus: f64,
    radius_bonus: f64,
    spirit_taken: f64,
    weapon_damage_pct: f64,
    light_melee: f64,
    heavy_melee: f64,
}

fn is_spirit_damage(p: &Prop) -> bool {
    p.scale.as_ref().is_some_and(|s| s.class == "scale_function_tech_damage")
}

/// Damage type: the spirit-damage scaling class wins, otherwise the game's display class decides.
fn damage_kind(p: &Prop) -> Option<String> {
    let class = p.scale.as_ref().map(|s| s.class.as_str()).unwrap_or("");
    let kind = match (class, p.css.as_str()) {
        ("scale_function_tech_damage", _) | (_, "tech_damage") => "spirit",
        (_, "melee_damage") => "melee",
        (_, "bullet_damage") => "bullet",
        (c, _) if c.contains("weapon_damage") => "bullet",
        _ => return None,
    };
    Some(kind.into())
}

fn scaled(p: &Prop, c: &Ctx) -> ScaledProp {
    let mut note = None;
    let (mut value, mut formula) = match &p.scale {
        Some(s) if s.coef != 0.0 => match scale_kind(s) {
            ScaleKind::Spirit => (
                p.value + s.coef * c.spirit_power,
                format!("{} + {} × {} spirit power", fmt(p.value), fmt(s.coef), fmt(c.spirit_power)),
            ),
            ScaleKind::Boons => (
                p.value + s.coef * c.boons,
                format!("{} + {} × {} boons", fmt(p.value), fmt(s.coef), fmt(c.boons)),
            ),
            // Assumed to work like spirit scaling: the coefficient is added per point of weapon damage %.
            ScaleKind::WeaponDamage => (
                p.value + s.coef * c.weapon_damage_pct,
                format!("{} + {} × {}% weapon damage", fmt(p.value), fmt(s.coef), fmt(c.weapon_damage_pct)),
            ),
            ScaleKind::LightMelee => (
                p.value + s.coef * c.light_melee,
                format!("{} + {} × {} light melee", fmt(p.value), fmt(s.coef), fmt(c.light_melee)),
            ),
            ScaleKind::HeavyMelee => (
                p.value + s.coef * c.heavy_melee,
                format!("{} + {} × {} heavy melee", fmt(p.value), fmt(s.coef), fmt(c.heavy_melee)),
            ),
            ScaleKind::Unsupported => {
                note = Some(format!("Scaling not computed yet ({} × {})", fmt(s.coef), s.stat));
                (p.value, fmt(p.value))
            }
        },
        _ => (p.value, fmt(p.value)),
    };
    if p.key == "AbilityCooldown" && c.cooldown_reduction != 0.0 {
        value *= 1.0 - c.cooldown_reduction / 100.0;
        formula = format!("({formula}) × (1 − {}%)", fmt(c.cooldown_reduction));
    }
    // The time between charges has its own reduction; cooldown reduction does not shorten it.
    if p.key == "AbilityCooldownBetweenCharge" && c.charge_reduction != 0.0 && value > 0.0 {
        value *= 1.0 - c.charge_reduction / 100.0;
        formula = format!("({formula}) × (1 − {}% faster charges)", fmt(c.charge_reduction));
    }
    if p.key == "AbilityDuration" && c.duration_bonus != 0.0 {
        value *= 1.0 + c.duration_bonus / 100.0;
        formula = format!("({formula}) × (1 + {}%)", fmt(c.duration_bonus));
    }
    let size = match p.grows.as_str() {
        "range" => c.range_bonus,
        "radius" => c.radius_bonus,
        _ => 0.0,
    };
    if size != 0.0 && value > 0.0 {
        value *= 1.0 + size / 100.0;
        formula = format!("({formula}) × (1 + {}% ability {})", fmt(size), p.grows);
    }
    ScaledProp {
        key: p.key.clone(),
        label: p.label.clone(),
        value,
        unit: p.postfix.clone(),
        formula,
        vs_target: is_spirit_damage(p).then_some(value * c.spirit_taken),
        damage_kind: damage_kind(p),
        base: p.value,
        coef: p.scale.as_ref().map(|s| s.coef).unwrap_or(0.0),
        modifier: p.modifier.clone(),
        note,
    }
}

fn fmt(v: f64) -> String {
    let s = format!("{v:.3}");
    let s = s.trim_end_matches('0').trim_end_matches('.');
    if s.is_empty() || s == "-0" { "0".into() } else { s.into() }
}

/// What the items imbued into one ability add to it.
#[derive(Default, Clone)]
struct Imbue {
    spirit: f64,
    cooldown: f64,
    /// Omnicharge Signet: bonus charges for an ability that has charges, and for one that has none.
    enable: Option<(f64, f64)>,
    duration: f64,
    range: f64,
    radius: f64,
    items: Vec<String>,
}

/// Two reductions of the same thing combine multiplicatively: 1 − (1 − a)(1 − b).
fn stack(a: f64, b: f64) -> f64 {
    100.0 * (1.0 - (1.0 - a / 100.0) * (1.0 - b / 100.0))
}

/// An ability counts as charged from two charges up; one charge is an ordinary cooldown.
const CHARGED_FROM: f64 = 2.0;
/// Assumed wait between two casts of an ability that only has charges because of an item.
const NEW_CHARGE_DELAY: f64 = 1.0;

const BETWEEN_CHARGES: &str = "MODIFIER_VALUE_COOLDOWN_BETWEEN_CHARGE_REDUCTION_PERCENTAGE";

/// Listed as passive effects in the data but always on, so they need no switch.
const ALWAYS_ON: [&str; 3] = [
    "MODIFIER_VALUE_ITEM_COOLDOWN_REDUCTION_PERCENTAGE",
    "MODIFIER_VALUE_HEAL_AMP_CAST_PERCENT",
    "MODIFIER_VALUE_HEAL_AMP_REGEN_PERCENT",
];

/// Stats an imbue item gives to its ability only; they never count as hero-wide stats.
const IMBUE_MODIFIERS: [&str; 5] = [
    "MODIFIER_VALUE_TECH_POWER",
    "MODIFIER_VALUE_COOLDOWN_REDUCTION_PERCENTAGE",
    "MODIFIER_VALUE_BONUS_ABILITY_DURATION_PERCENTAGE",
    "MODIFIER_VALUE_TECH_RANGE_PERCENT",
    "MODIFIER_VALUE_TECH_RADIUS_PERCENT",
];

fn abilities(hero: &Hero, b: &BuildInput, c: &Ctx, imbues: &[Imbue], copied: Option<&Ability>) -> (Vec<AbilitySheet>, u32) {
    let mut used = 0;
    let sheets = hero
        .abilities
        .iter()
        .enumerate()
        .map(|(i, own)| {
            // A copied ultimate keeps the copier's upgrade tier and uses the copier's stats.
            let copy = copied.filter(|_| own.class_name == COPY_ULTIMATE);
            let ab = copy.unwrap_or(own);
            let max_tier = own.upgrades.len().min(AP_COST.len());
            let tier = (b.ability_tiers.get(i).copied().unwrap_or(0) as usize).min(max_tier);
            used += AP_COST[..tier].iter().sum::<u32>();
            let imbue = imbues.get(i).cloned().unwrap_or_default();
            let mut props = ab.props.clone();
            for up in ab.upgrades.iter().take(tier).flatten() {
                let Some(p) = props.iter_mut().find(|p| p.key == up.name) else { continue };
                match up.kind.as_str() {
                    "" | "EAddToBase" => p.value += up.bonus,
                    // The bonus is a percentage increase: 33.33 means ×1.3333.
                    "EMultiplyBase" => p.value *= 1.0 + up.bonus / 100.0,
                    "EAddToScale" => match &mut p.scale {
                        Some(s) => s.coef += up.bonus,
                        None => {
                            p.scale = Some(Scale {
                                class: String::new(),
                                stat: up.stat_filter.clone().unwrap_or_else(|| "ETechPower".into()),
                                coef: up.bonus,
                            })
                        }
                    },
                    "EMultiplyScale" => {
                        if let Some(s) = &mut p.scale {
                            s.coef *= up.bonus;
                        }
                    }
                    _ => {}
                }
            }
            let value_of = |props: &[Prop], key: &str| props.iter().find(|p| p.key == key).map(|p| p.value).unwrap_or(0.0);
            let set = |props: &mut Vec<Prop>, key: &str, label: &str, value: f64| match props.iter_mut().find(|p| p.key == key) {
                Some(p) => p.value = value,
                None => props.push(Prop {
                    key: key.into(),
                    label: label.into(),
                    value,
                    postfix: String::new(),
                    section: String::new(),
                    modifier: None,
                    scale: None,
                    css: String::new(),
                    grows: String::new(),
                }),
            };
            // Omnicharge Signet gives its ability charges, and more of them if it already had some.
            if let Some((more, fresh)) = imbue.enable {
                let had = value_of(&props, "AbilityCharges");
                if had >= CHARGED_FROM {
                    set(&mut props, "AbilityCharges", "Charges", had + more);
                } else {
                    set(&mut props, "AbilityCharges", "Charges", 1.0 + fresh);
                    if value_of(&props, "AbilityCooldownBetweenCharge") <= 0.0 {
                        set(&mut props, "AbilityCooldownBetweenCharge", "Charge Delay", NEW_CHARGE_DELAY);
                    }
                }
            }
            // Bonus charges only count for abilities that are charged at this upgrade tier.
            let charged = value_of(&props, "AbilityCharges") >= CHARGED_FROM;
            if charged && c.bonus_charges > 0.0 {
                let had = value_of(&props, "AbilityCharges");
                set(&mut props, "AbilityCharges", "Charges", had + c.bonus_charges);
            }
            let c = &Ctx {
                spirit_power: c.spirit_power + imbue.spirit + if charged { c.charged_spirit } else { 0.0 },
                // The ultimate (fourth ability) also gets its own cooldown reduction.
                cooldown_reduction: stack(
                    stack(stack(c.cooldown_reduction, imbue.cooldown), if charged { c.charged_cdr } else { 0.0 }),
                    if i == 3 { c.ult_cooldown_reduction } else { 0.0 },
                ),
                duration_bonus: c.duration_bonus + imbue.duration,
                range_bonus: c.range_bonus + imbue.range,
                radius_bonus: c.radius_bonus + imbue.radius,
                ..*c
            };
            let mut sheet = AbilitySheet {
                slot_class: own.class_name.clone(),
                class_name: ab.class_name.clone(),
                name: if copy.is_some() { format!("{}: {}", own.name, ab.name) } else { ab.name.clone() },
                icon: ab.icon.clone(),
                tier: tier as u8,
                max_tier: max_tier as u8,
                // A negative value in Ability* fields means none (e.g. charge delay on an ability without charges).
                props: props
                    .iter()
                    .map(|p| scaled(p, c))
                    .filter(|p| p.value != 0.0 && !(p.value < 0.0 && p.key.starts_with("Ability")))
                    .collect(),
                imbued: imbue.items,
                description: ab.description.clone(),
                quip: ab.quip.clone(),
                upgrade_notes: ab
                    .upgrades
                    .iter()
                    .map(|tier| {
                        tier.iter()
                            .map(|u| {
                                let prop = ab.props.iter().find(|p| p.key == u.name);
                                let label = prop.map(|p| p.label.as_str()).filter(|l| !l.is_empty()).unwrap_or(u.name.as_str());
                                let rounded = (u.bonus * 100.0).round() / 100.0;
                                let amount = format!("{}{rounded}", if u.bonus > 0.0 { "+" } else { "" });
                                match u.kind.as_str() {
                                    "EMultiplyBase" | "EMultiplyScale" => format!("{label} ×{rounded}"),
                                    "EAddToScale" => format!("{label} {amount} scaling"),
                                    _ => format!("{label} {amount}{}", prop.map(|p| p.postfix.as_str()).unwrap_or("")),
                                }
                            })
                            .collect::<Vec<_>>()
                            .join(", ")
                    })
                    .collect(),
            };
            // The copy comes off cooldown faster than the original.
            let copy_cooldown = own.props.iter().find(|p| p.key == "CopyCooldownPercentage").map(|p| p.value);
            if let (Some(_), Some(pct)) = (copy, copy_cooldown) {
                for p in sheet.props.iter_mut().filter(|p| p.key == "AbilityCooldown") {
                    p.value *= pct / 100.0;
                    p.formula = format!("({}) × {}% copied cooldown", p.formula, fmt(pct));
                }
            }
            sheet
        })
        .collect();
    (sheets, used)
}

pub fn compute(data: &GameData, b: &BuildInput) -> Result<Sheet, String> {
    compute_at(data, b, None)
}

fn compute_at(data: &GameData, b: &BuildInput, force_level: Option<u32>) -> Result<Sheet, String> {
    let hero = data.hero(&b.hero).ok_or_else(|| format!("Unknown hero: {}", b.hero))?;
    let r = resolve(data, b);
    let spent_in = |cat: &str| r.owned.iter().filter(|i| i.category == cat).map(|i| i.cost).sum::<u32>();
    let (spent_w, spent_v, spent_s) = (spent_in("weapon"), spent_in("vitality"), spent_in("spirit"));

    let souls = b.souls.unwrap_or(0).max(r.spent);
    let level = force_level.unwrap_or_else(|| {
        hero.levels
            .iter()
            .filter(|l| l.required_souls <= souls)
            .map(|l| l.level)
            .max()
            .unwrap_or(1)
    });
    let reached = || hero.levels.iter().filter(|l| l.level <= level);
    let boon_count = reached().filter(|l| l.boon).count() as u32;
    let boons = boon_count as f64;
    let ability_points = reached().map(|l| l.ability_points).sum::<u32>();

    let base = |k: &str| hero.base.get(k).copied().unwrap_or(0.0);
    let per_boon = |k: &str| hero.level_up.get(k).copied().unwrap_or(0.0);

    let mut bag = Bag::default();
    const LEVEL_HANDLED: [&str; 5] = [
        "MODIFIER_VALUE_BASE_BULLET_DAMAGE_FROM_LEVEL",
        "MODIFIER_VALUE_BASE_HEALTH_FROM_LEVEL",
        "MODIFIER_VALUE_BASE_MELEE_DAMAGE_FROM_LEVEL",
        "MODIFIER_VALUE_TECH_POWER",
        "MODIFIER_VALUE_BOON_COUNT",
    ];
    for (k, v) in &hero.level_up {
        if *v != 0.0 && boons > 0.0 && !LEVEL_HANDLED.contains(&k.as_str()) && !k.contains("ALT_FIRE") {
            bag.push(k, &format!("Level ({boon_count} boons)"), v * boons, "", "");
        }
    }
    // Map buffs: powerups grow in a straight line from minute 5 to minute 40; statue buffs have three levels.
    let m = &b.map;
    let grown = ((m.minute - POWERUP_FROM_MINUTE) / (POWERUP_FULL_MINUTE - POWERUP_FROM_MINUTE)).clamp(0.0, 1.0);
    let lerp = |(lo, hi): (f64, f64)| lo + (hi - lo) * grown;
    if m.casting {
        bag.push("MODIFIER_VALUE_TECH_POWER", "Casting powerup", lerp(POWERUP_CASTING_SPIRIT), "", "");
        bag.push("MODIFIER_VALUE_COOLDOWN_REDUCTION_PERCENTAGE", "Casting powerup", lerp(POWERUP_CASTING_CDR), "", "%");
    }
    if m.gun {
        bag.push("MODIFIER_VALUE_FIRE_RATE", "Gun powerup", lerp(POWERUP_GUN_FIRE_RATE), "", "%");
        bag.push("MODIFIER_VALUE_AMMO_CLIP_SIZE_PERCENT", "Gun powerup", lerp(POWERUP_GUN_AMMO), "", "%");
    }
    let statue_level = if m.minute >= 30.0 { 2 } else if m.minute >= 10.0 { 1 } else { 0 };
    let st = &m.statues;
    for (count, key, per, unit) in [
        (st.fire_rate, "MODIFIER_VALUE_FIRE_RATE", STATUE_FIRE_RATE, "%"),
        (st.ammo, "MODIFIER_VALUE_AMMO_CLIP_SIZE_PERCENT", STATUE_AMMO, "%"),
        (st.weapon, "MODIFIER_VALUE_WEAPON_POWER", STATUE_WEAPON, "%"),
        (st.health, "MODIFIER_VALUE_HEALTH_MAX", STATUE_HEALTH, ""),
        (st.spirit, "MODIFIER_VALUE_TECH_POWER", STATUE_SPIRIT, ""),
    ] {
        if count > 0 {
            bag.push(key, &format!("Statue buffs ×{count}"), per[statue_level] * count as f64, "", unit);
        }
    }
    // Resists and cooldown reduction combine by what each leaves: one term per buff.
    for (count, key, per) in [
        (st.cooldown, "MODIFIER_VALUE_COOLDOWN_REDUCTION_PERCENTAGE", STATUE_CDR),
        (st.bullet_resist, "MODIFIER_VALUE_BULLET_ARMOR_DAMAGE_RESIST", STATUE_RESIST),
        (st.spirit_resist, "MODIFIER_VALUE_TECH_RESIST", STATUE_RESIST),
    ] {
        for _ in 0..count.min(100) {
            bag.push(key, "Statue buff", per[statue_level], "", "%");
        }
    }
    if b.bonus_spirit != 0.0 {
        bag.push("MODIFIER_VALUE_TECH_POWER", "Active buff", b.bonus_spirit, "", "");
    }
    if b.bonus_spirit_pct != 0.0 {
        bag.push("MODIFIER_VALUE_TECH_POWER_PERCENT", "Active buff", b.bonus_spirit_pct, "", "%");
    }
    for item in &r.owned {
        let on = b.active.iter().any(|a| a == &item.class_name);
        // The data leaves the ability radius bonus without a section; it goes with the item's ability range bonus.
        let range_section = item.props.iter().find(|p| p.modifier.as_deref() == Some("MODIFIER_VALUE_TECH_RANGE_PERCENT")).map(|p| p.section.as_str());
        for p in &item.props {
            let Some(m) = &p.modifier else { continue };
            let section = match (p.section.as_str(), m.as_str(), range_section) {
                ("", "MODIFIER_VALUE_TECH_RADIUS_PERCENT", Some(s)) => s,
                (s, _, _) => s,
            };
            if !item.imbue.is_empty() && section != "innate" && IMBUE_MODIFIERS.contains(&m.as_str()) {
                continue;
            }
            // Bonuses for charged abilities only are applied per ability, not hero-wide.
            if p.key.ends_with("ChargedAbilities") {
                continue;
            }
            if section == "innate" || (p.value > 0.0 && ALWAYS_ON.contains(&m.as_str())) {
                bag.push(m, &item.name, p.value, &p.label, &p.postfix);
            } else if on {
                let source = format!("{} (conditional)", item.name);
                // Conditional negative values are debuffs applied to the enemy; they don't count toward our own stats.
                if p.value < 0.0 && !m.contains("RESIST_REDUCTION") {
                    bag.push(&format!("ENEMY:{m}"), &source, p.value, &format!("On enemy: {}", p.label), &p.postfix);
                } else {
                    bag.push(m, &source, p.value, &p.label, &p.postfix);
                }
            }
        }
    }

    let mut stats: Vec<Stat> = vec![];
    let mut add = |key: &str, label: &str, group: &str, value: f64, unit: &str, formula: &str, terms: Vec<Term>| {
        stats.push(Stat {
            key: key.into(),
            label: label.into(),
            group: group.into(),
            value,
            unit: unit.into(),
            formula: formula.into(),
            terms,
        });
    };
    // Some base stats of some heroes grow with spirit power, so spirit power is needed before anything else.
    // Same formula as the spirit power stat further down.
    let spirit_now = (boons * per_boon("MODIFIER_VALUE_TECH_POWER") + invest_bonus(&hero.invest.spirit, spent_s) + bag.peek("MODIFIER_VALUE_TECH_POWER"))
        * (1.0 + bag.peek("MODIFIER_VALUE_TECH_POWER_PERCENT") / 100.0);
    let spirit_now = spirit_now.max(0.0);
    let from_spirit = |stat: &str| hero.spirit_scaling.get(stat).copied().unwrap_or(0.0) * spirit_now;
    let spirit_term = |stat: &str, unit: &str| {
        let per_point = hero.spirit_scaling.get(stat).copied().unwrap_or(0.0);
        term(format!("Spirit power: {} × {}", fmt(spirit_now), fmt(per_point)), per_point * spirit_now, unit)
    };
    let with_invest = |mut terms: Vec<Term>, label: &str, bonus: f64, unit: &str| {
        if bonus != 0.0 {
            terms.insert(0, term(label, bonus, unit));
        }
        terms
    };

    // --- Weapon ---
    let w = &hero.weapon;
    let inv_w = invest_bonus(&hero.invest.weapon, spent_w);
    let wd_terms = with_invest(
        bag.take(&[
            "MODIFIER_VALUE_WEAPON_DAMAGE_INCREASE",
            "MODIFIER_VALUE_WEAPON_POWER",
            "MODIFIER_VALUE_CLOSE_RANGE_WEAPON_DAMAGE_INCREASE",
            "MODIFIER_VALUE_LONG_RANGE_BULLET_DAMAGE_INCREASE",
        ]),
        &format!("Weapon investment ({spent_w} souls)"),
        inv_w,
        "%",
    );
    bag.take(&[
        "MODIFIER_VALUE_BONUS_WEAPON_DAMAGE_CLOSE_RANGE_MAX_RANGE",
        "MODIFIER_VALUE_BONUS_BULLET_DAMAGE_LONG_RANGE_MIN_RANGE",
    ]);
    let wd = total(&wd_terms);
    add("weapon_damage_pct", "Weapon damage bonus", "weapon", wd, "%", "Σ weapon damage % (additive)", wd_terms);

    let bullet_growth = per_boon("MODIFIER_VALUE_BASE_BULLET_DAMAGE_FROM_LEVEL");
    let bullet = (w.bullet_damage + boons * bullet_growth + from_spirit("EBulletDamage")) * (1.0 + wd / 100.0);
    add(
        "bullet_damage",
        "Bullet damage",
        "weapon",
        bullet,
        "",
        "(base + boons × per-level gain + spirit scaling) × (1 + weapon damage %)",
        {
            let mut t = vec![
                term("Base bullet damage", w.bullet_damage, ""),
                term(format!("Level: {boon_count} boons × {}", fmt(bullet_growth)), boons * bullet_growth, ""),
            ];
            if from_spirit("EBulletDamage") != 0.0 {
                t.push(spirit_term("EBulletDamage", ""));
            }
            t.push(term("Weapon damage bonus", wd, "%"));
            t
        },
    );

    let mut fr_terms = bag.take(&["MODIFIER_VALUE_FIRE_RATE"]);
    if from_spirit("EFireRate") != 0.0 {
        fr_terms.insert(0, spirit_term("EFireRate", "%"));
    }
    // Penalties (an item's own, or an enemy's slow) combine apart from the bonuses.
    let (mut slow_terms, fr_terms): (Vec<Term>, Vec<Term>) = fr_terms.into_iter().partition(|t| t.value < 0.0);
    for t in &mut slow_terms {
        t.value = t.value.abs();
    }
    if b.fire_rate_slow != 0.0 {
        slow_terms.push(term("Fire rate slow from an enemy", b.fire_rate_slow, "%"));
    }
    let fr = total(&fr_terms);
    add("fire_rate_pct", "Fire rate bonus", "weapon", fr, "%", "Σ fire rate % (additive)", fr_terms);
    let fr_slow = combine_resist(&slow_terms);
    if fr_slow != 0.0 {
        add("fire_rate_slow", "Fire rate slow", "weapon", fr_slow, "%", "1 − Π(1 − slowᵢ)  (slows combine multiplicatively)", slow_terms);
    }
    let sps = fire_rate(w.shots_per_second, fr, fr_slow);
    add(
        "shots_per_second",
        "Shots per second",
        "weapon",
        sps,
        "/s",
        "base shots/s × (1 + bonus − slow); below zero, base ÷ (1 + |bonus − slow|), at most −50%",
        vec![term("Base shots/s", w.shots_per_second, ""), term("Fire rate bonus", fr, "%"), term("Fire rate slow", -fr_slow, "%")],
    );

    let clip_pct_terms = bag.take(&["MODIFIER_VALUE_AMMO_CLIP_SIZE_PERCENT"]);
    let mut clip_flat_terms = bag.take(&["MODIFIER_VALUE_AMMO_CLIP_SIZE"]);
    if from_spirit("EClipSize") != 0.0 {
        // Only whole bullets count.
        let mut extra = spirit_term("EClipSize", "");
        extra.value = extra.value.floor();
        clip_flat_terms.insert(0, extra);
    }
    // Flat bonuses first, then the percentage bonuses, rounded up (deadlock.wiki, Ammo).
    let clip = ((w.clip_size + total(&clip_flat_terms)) * (1.0 + total(&clip_pct_terms) / 100.0) - 1e-9).ceil();
    let mut clip_terms = vec![term("Base magazine", w.clip_size, "")];
    clip_terms.extend(clip_pct_terms);
    clip_terms.extend(clip_flat_terms);
    add("clip_size", "Magazine", "weapon", clip, "", "(base + flat) × (1 + ammo %), rounded up", clip_terms);

    let reload_terms = bag.take(&["MODIFIER_VALUE_RELOAD_SPEED"]);
    let reload = w.reload_time / (1.0 + total(&reload_terms) / 100.0);
    let mut rt = vec![term("Base reload time", w.reload_time, "s")];
    rt.extend(reload_terms);
    add("reload_time", "Reload time", "weapon", reload, "s", "base time ÷ (1 + reload speed %)", rt);

    let velocity_terms = bag.take(&["MODIFIER_VALUE_BONUS_BULLET_SPEED_PERCENT"]);
    if w.bullet_speed > 0.0 {
        let velocity = w.bullet_speed * (1.0 + total(&velocity_terms) / 100.0);
        let mut t = vec![term("Base", w.bullet_speed, "m/s")];
        t.extend(velocity_terms);
        add("bullet_velocity", "Bullet velocity", "weapon", velocity, "m/s", "base × (1 + bullet velocity %)", t);
    }
    let range_pct_terms = bag.take(&["MODIFIER_VALUE_BONUS_ATTACK_RANGE_PERCENT"]);
    // Flat range (from levels) is stored in game units.
    let range_flat_terms: Vec<Term> = bag
        .take(&["MODIFIER_VALUE_BONUS_ATTACK_RANGE"])
        .into_iter()
        .map(|t| Term { value: t.value / crate::raw::UNITS_PER_METER, unit: "m".into(), ..t })
        .collect();
    for (key, label, start) in [
        ("falloff_start", "Damage falloff starts at", w.falloff_start),
        ("falloff_end", "Damage falloff ends at", w.falloff_end),
    ] {
        if start <= 0.0 {
            continue;
        }
        let mut t = vec![term("Base", start, "m")];
        t.extend(range_flat_terms.clone());
        t.extend(range_pct_terms.clone());
        let value = (start + total(&range_flat_terms)) * (1.0 + total(&range_pct_terms) / 100.0);
        add(key, label, "weapon", value, "m", "(base + flat range) × (1 + falloff range %)", t);
    }

    let per_shot = bullet * w.pellets;
    let dps_burst = per_shot * sps;
    add(
        "dps_burst",
        "DPS (burst)",
        "weapon",
        dps_burst,
        "",
        "bullet damage × pellets × shots/s",
        vec![term("Bullet damage", bullet, ""), term("Pellets", w.pellets, ""), term("Shots/s", sps, "")],
    );
    let mag_time = if sps > 0.0 { clip / sps } else { 0.0 };
    let dps_sustained = if mag_time + reload > 0.0 { per_shot * clip / (mag_time + reload) } else { 0.0 };
    add(
        "dps_sustained",
        "DPS (with reload)",
        "weapon",
        dps_sustained,
        "",
        "magazine damage ÷ (time to empty magazine + reload time)",
        vec![
            term("Magazine damage", per_shot * clip, ""),
            term("Time to empty magazine", mag_time, "s"),
            term("Reload time", reload, "s"),
        ],
    );
    let crit = crit_multiplier(&hero.class_name, w.crit_multiplier);
    add(
        "crit_multiplier",
        "Headshot multiplier",
        "weapon",
        crit,
        "×",
        "1 + 0.65 + the hero's crit bonus scale (some heroes hit heads for less, some cannot at all)",
        vec![],
    );

    let melee_growth = per_boon("MODIFIER_VALUE_BASE_MELEE_DAMAGE_FROM_LEVEL");
    let melee_terms = bag.take(&["MODIFIER_VALUE_MELEE_DAMAGE_INCREASE"]);
    // Melee damage gains half of every weapon damage bonus (deadlock.wiki, Melee Damage).
    let melee_pct = total(&melee_terms) + MELEE_WEAPON_SHARE * wd;
    for (key, label, base_key) in [
        ("light_melee", "Light melee", "light_melee_damage"),
        ("heavy_melee", "Heavy melee", "heavy_melee_damage"),
    ] {
        let mut t = vec![
            term("Base", base(base_key), ""),
            term(format!("Level: {boon_count} boons × {}", fmt(melee_growth)), boons * melee_growth, ""),
        ];
        // Only heavy melee has a spirit scaling so far (Paige).
        let extra = if key == "heavy_melee" { from_spirit("EHeavyMeleeDamage") } else { 0.0 };
        if extra != 0.0 {
            t.push(spirit_term("EHeavyMeleeDamage", ""));
        }
        t.extend(melee_terms.clone());
        if wd != 0.0 {
            t.push(term(format!("Half of the weapon damage bonus ({}%)", fmt(wd)), MELEE_WEAPON_SHARE * wd, "%"));
        }
        add(
            key,
            label,
            "weapon",
            (base(base_key) + boons * melee_growth + extra) * (1.0 + melee_pct / 100.0),
            "",
            "(base + boons × per-level gain + spirit scaling) × (1 + melee % + ½ weapon damage %)",
            t,
        );
    }

    // --- Vitality ---
    let inv_v = invest_bonus(&hero.invest.vitality, spent_v);
    let hp_growth = per_boon("MODIFIER_VALUE_BASE_HEALTH_FROM_LEVEL");
    let base_hp_pct_terms = with_invest(
        bag.take(&["MODIFIER_VALUE_BASE_HEALTH_PERCENT"]),
        &format!("Vitality investment ({spent_v} souls)"),
        inv_v,
        "%",
    );
    let flat_hp_terms = bag.take(&["MODIFIER_VALUE_HEALTH_MAX"]);
    let hp_pct_terms = bag.take(&["MODIFIER_VALUE_HEALTH_MAX_PERCENT"]);
    let base_hp = base("max_health") + boons * hp_growth;
    let max_health = (base_hp * (1.0 + total(&base_hp_pct_terms) / 100.0) + total(&flat_hp_terms))
        * (1.0 + total(&hp_pct_terms) / 100.0);
    let mut hp_terms = vec![
        term("Base health", base("max_health"), ""),
        term(format!("Level: {boon_count} boons × {}", fmt(hp_growth)), boons * hp_growth, ""),
    ];
    hp_terms.extend(base_hp_pct_terms);
    hp_terms.extend(flat_hp_terms);
    hp_terms.extend(hp_pct_terms);
    add(
        "max_health",
        "Max health",
        "vitality",
        max_health,
        "",
        "((base + boons × per-level gain) × (1 + base health %) + flat health) × (1 + total health %)",
        hp_terms,
    );

    let regen_terms = bag.take(&["MODIFIER_VALUE_HEALTH_REGEN_PER_SECOND"]);
    let mut t = vec![term("Base", base("base_health_regen"), "")];
    if from_spirit("EBaseHealthRegen") != 0.0 {
        t.push(spirit_term("EBaseHealthRegen", ""));
    }
    let regen = base("base_health_regen") + from_spirit("EBaseHealthRegen") + total(&regen_terms);
    t.extend(regen_terms);
    add("health_regen", "Health regen", "vitality", regen, "/s", "base + Σ items", t);
    let ooc_terms = bag.take(&["MODIFIER_VALUE_OUT_OF_COMBAT_HEALTH_REGEN"]);
    let mut t = vec![term("Base", base("ooc_health_regen"), "")];
    let ooc = base("ooc_health_regen") + total(&ooc_terms);
    t.extend(ooc_terms);
    add("ooc_health_regen", "Out of combat regen", "vitality", ooc, "/s", "base + Σ items (added to health regen while out of combat)", t);

    let mut resists = [0.0; 3];
    for (i, (key, label, modifier, base_key)) in [
        ("bullet_resist", "Bullet resist", "MODIFIER_VALUE_BULLET_ARMOR_DAMAGE_RESIST", "bullet_armor_damage_reduction"),
        ("spirit_resist", "Spirit resist", "MODIFIER_VALUE_TECH_RESIST", "tech_armor_damage_reduction"),
        ("melee_resist", "Melee resist", "MODIFIER_VALUE_MELEE_RESIST", ""),
    ]
    .into_iter()
    .enumerate()
    {
        let mut terms = bag.take(&[modifier]);
        // Some heroes start with a resist of their own (it can be negative).
        if base(base_key) != 0.0 {
            terms.insert(0, term("Hero base", base(base_key), "%"));
        }
        // And some gain resist from spirit power; it combines like any other source.
        let scaling = match key {
            "bullet_resist" => "EBulletArmorDamageReduction",
            "spirit_resist" => "ETechArmorDamageReduction",
            _ => "",
        };
        if from_spirit(scaling) != 0.0 {
            terms.push(spirit_term(scaling, "%"));
        }
        resists[i] = combine_resist(&terms);
        add(key, label, "vitality", resists[i], "%", "1 − Π(1 − rᵢ)  (sources combine multiplicatively)", terms);
    }
    for (key, label, r) in [
        ("ehp_bullet", "Effective health (vs bullets)", resists[0]),
        ("ehp_spirit", "Effective health (vs spirit)", resists[1]),
    ] {
        let ehp = if r < 100.0 { max_health / (1.0 - r / 100.0) } else { f64::INFINITY };
        add(
            key,
            label,
            "vitality",
            ehp,
            "",
            "max health ÷ (1 − resist)",
            vec![term("Max health", max_health, ""), term("Resist", r, "%")],
        );
    }

    let mut move_speed = 0.0;
    for (key, label, base_key, modifier, unit) in [
        ("move_speed", "Move speed", "max_move_speed", "MODIFIER_VALUE_MOVEMENT_SPEED_MAX", "m/s"),
        ("sprint_speed", "Sprint speed bonus", "sprint_speed", "MODIFIER_VALUE_SPRINT_SPEED_BONUS", "m/s"),
        ("stamina", "Stamina", "stamina", "MODIFIER_VALUE_STAMINA", ""),
    ] {
        let terms = bag.take(&[modifier]);
        let scaling = match key {
            "move_speed" => "EMaxMoveSpeed",
            "sprint_speed" => "ESprintSpeed",
            _ => "",
        };
        let value = base(base_key) + from_spirit(scaling) + total(&terms);
        let mut t = vec![term("Base", base(base_key), unit)];
        if from_spirit(scaling) != 0.0 {
            t.push(spirit_term(scaling, unit));
        }
        t.extend(terms);
        add(key, label, "vitality", value, unit, "base + Σ items", t);
        if key == "move_speed" {
            move_speed = value;
        }
    }
    // Shooting and zooming slow the hero down by a factor of the weapon; some items remove part of that.
    for (key, label, kept, modifier) in [
        (
            "move_speed_shooting",
            "Move speed while shooting",
            w.shoot_move_speed,
            "MODIFIER_VALUE_MOVEMENT_SPEED_WHILE_SHOOTING_PENALTY_REDUCTION_PERCENT",
        ),
        (
            "move_speed_zoomed",
            "Move speed while zoomed",
            w.zoom_move_speed,
            "MODIFIER_VALUE_MOVEMENT_SPEED_WHILE_ZOOMED_PENALTY_REDUCTION_PERCENT",
        ),
    ] {
        let terms = bag.take(&[modifier]);
        if kept <= 0.0 || (kept >= 1.0 && terms.is_empty()) {
            continue;
        }
        let removed = total(&terms).clamp(0.0, 100.0) / 100.0;
        let mut t = vec![term("Move speed", move_speed, "m/s"), term("Weapon keeps", kept * 100.0, "%")];
        t.extend(terms);
        add(
            key,
            label,
            "vitality",
            move_speed * (kept + (1.0 - kept).max(0.0) * removed),
            "m/s",
            "move speed × (share the weapon keeps + removed share of the penalty)",
            t,
        );
    }
    let stamina_terms = bag.take(&["MODIFIER_VALUE_STAMINA_REGEN_PER_SECOND_PERCENTAGE"]);
    let stamina_rate = base("stamina_regen_per_second") * (1.0 + total(&stamina_terms) / 100.0);
    if stamina_rate > 0.0 {
        let mut t = vec![term("Base stamina per second", base("stamina_regen_per_second"), "")];
        t.extend(stamina_terms);
        add(
            "stamina_recovery",
            "Stamina recovery time",
            "vitality",
            1.0 / stamina_rate,
            "s",
            "1 ÷ (base stamina per second × (1 + stamina recovery %))",
            t,
        );
    }
    if base("ground_dash_distance_in_meters") > 0.0 {
        let dash = base("ground_dash_distance_in_meters");
        add("dash_distance", "Dash distance", "vitality", dash, "m", "the hero's base ground dash", vec![term("Base", dash, "m")]);
    }
    let air_terms = bag.take(&["MODIFIER_VALUE_AIR_MOVE_DISTANCE_INCREASE_PERCENT"]);
    if base("air_dash_distance_in_meters") > 0.0 {
        let air = base("air_dash_distance_in_meters");
        let mut t = vec![term("Base", air, "m")];
        let value = air * (1.0 + total(&air_terms) / 100.0);
        t.extend(air_terms);
        add("air_dash_distance", "Air dash distance", "vitality", value, "m", "base × (1 + air dash distance %)", t);
    }
    for (key, label, modifier) in [
        ("debuff_resist", "Debuff resist", "MODIFIER_VALUE_STATUS_RESISTANCE"),
        ("slow_resist", "Slow resist", "MODIFIER_VALUE_MOVEMENT_SLOW_RESISTANCE"),
    ] {
        let terms = bag.take(&[modifier]);
        add(key, label, "vitality", total(&terms), "%", "Σ items (assumed additive)", terms);
    }
    for (key, label, group, modifier) in [
        ("bullet_lifesteal", "Bullet lifesteal", "vitality", "MODIFIER_VALUE_BULLET_LIFESTEAL"),
        ("spirit_lifesteal", "Spirit lifesteal", "vitality", "MODIFIER_VALUE_TECH_LIFESTEAL"),
    ] {
        let terms = bag.take(&[modifier]);
        add(key, label, group, combine_resist(&terms), "%", "1 − Π(1 − Lᵢ)  (sources combine multiplicatively)", terms);
    }

    // --- Spirit ---
    let inv_s = invest_bonus(&hero.invest.spirit, spent_s);
    let sp_growth = per_boon("MODIFIER_VALUE_TECH_POWER");
    let sp_flat_terms = with_invest(
        bag.take(&["MODIFIER_VALUE_TECH_POWER"]),
        &format!("Spirit investment ({spent_s} souls)"),
        inv_s,
        "",
    );
    let sp_pct_terms = bag.take(&["MODIFIER_VALUE_TECH_POWER_PERCENT"]);
    // Spirit power taken away by an enemy (Spirit Sap, Focus Lens) stops at zero.
    // Percentage bonuses to spirit power multiply each other (deadlock.wiki, Spirit Power).
    let sp_mult: f64 = sp_pct_terms.iter().map(|t| 1.0 + t.value / 100.0).product();
    let spirit_power = ((boons * sp_growth + total(&sp_flat_terms)) * sp_mult).max(0.0);
    let mut sp_terms = vec![term(format!("Level: {boon_count} boons × {}", fmt(sp_growth)), boons * sp_growth, "")];
    sp_terms.extend(sp_flat_terms);
    sp_terms.extend(sp_pct_terms);
    add(
        "spirit_power",
        "Spirit power",
        "spirit",
        spirit_power,
        "",
        "(boons × per-level gain + investment + Σ items) × Π(1 + spirit power %ᵢ), not below 0",
        sp_terms,
    );
    let mut spirit_mods = [0.0; 4];
    for (i, (key, label, modifier)) in [
        ("cooldown_reduction", "Cooldown reduction", "MODIFIER_VALUE_COOLDOWN_REDUCTION_PERCENTAGE"),
        ("ability_duration", "Ability duration bonus", "MODIFIER_VALUE_BONUS_ABILITY_DURATION_PERCENTAGE"),
        ("ability_range", "Ability range bonus", "MODIFIER_VALUE_TECH_RANGE_PERCENT"),
        ("ability_radius", "Ability radius bonus", "MODIFIER_VALUE_TECH_RADIUS_PERCENT"),
    ]
    .into_iter()
    .enumerate()
    {
        let terms = bag.take(&[modifier]);
        if key == "cooldown_reduction" {
            spirit_mods[i] = combine_resist(&terms);
            add(key, label, "spirit", spirit_mods[i], "%", "1 − Π(1 − CRᵢ)  (sources combine multiplicatively)", terms);
        } else {
            spirit_mods[i] = total(&terms);
            add(key, label, "spirit", spirit_mods[i], "%", "Σ items (additive)", terms);
        }
    }
    let ult_cdr_terms = bag.take(&["MODIFIER_VALUE_ULTIMATE_COOLDOWN_REDUCTION_PERCENTAGE"]);
    let ult_cdr = combine_resist(&ult_cdr_terms);
    add(
        "ultimate_cooldown_reduction",
        "Ultimate cooldown reduction",
        "spirit",
        ult_cdr,
        "%",
        "applies to the ultimate, multiplicatively with cooldown reduction",
        ult_cdr_terms,
    );
    let item_cdr_terms = bag.take(&["MODIFIER_VALUE_ITEM_COOLDOWN_REDUCTION_PERCENTAGE"]);
    let item_cdr = combine_resist(&item_cdr_terms);
    add(
        "item_cooldown_reduction",
        "Item cooldown reduction",
        "spirit",
        item_cdr,
        "%",
        "applies to item cooldowns only; ability cooldown reduction does not affect items",
        item_cdr_terms,
    );
    let charge_terms = bag.take(&[BETWEEN_CHARGES]);
    let charge_reduction = total(&charge_terms).min(100.0);
    add("charge_reduction", "Faster time between charges", "spirit", charge_reduction, "%", "Σ items (assumed additive)", charge_terms);
    let bonus_charge_terms = bag.take(&["MODIFIER_VALUE_BONUS_ABILITY_CHARGES"]);
    let bonus_charges = total(&bonus_charge_terms);
    add(
        "bonus_charges",
        "Bonus ability charges",
        "spirit",
        bonus_charges,
        "",
        "Σ items; only for abilities that already have charges",
        bonus_charge_terms,
    );

    // --- Target ---
    let bullet_shred_terms = bag.take(&["MODIFIER_VALUE_BULLET_AND_MELEE_RESIST_REDUCTION"]);
    let spirit_shred_terms = bag.take(&["MODIFIER_VALUE_TECH_RESIST_REDUCTION"]);
    // Resist reductions combine by what each leaves, then come off the target's resist (deadlock.wiki, Damage Resistance).
    let shred = |terms: &[Term]| 100.0 * (1.0 - terms.iter().map(|t| 1.0 - t.value.abs() / 100.0).product::<f64>());
    let bullet_shred = shred(&bullet_shred_terms);
    let spirit_shred = shred(&spirit_shred_terms);
    let target_br = b.target.bullet_resist - bullet_shred;
    let target_sr = b.target.spirit_resist - spirit_shred;
    // A bonus or penalty to all damage the hero deals (e.g. Golden Goose Egg).
    let outgoing_terms = bag.take(&["MODIFIER_VALUE_ALL_DAMAGE_MULTIPLIER"]);
    let outgoing = total(&outgoing_terms);
    add("outgoing_damage_pct", "Outgoing damage", "target", outgoing, "%", "Σ items (additive); multiplies all damage dealt", outgoing_terms);
    let bullet_taken = (1.0 - target_br / 100.0).max(0.0) * (1.0 + outgoing / 100.0);
    let spirit_taken = (1.0 - target_sr / 100.0).max(0.0) * (1.0 + outgoing / 100.0);
    let mut t = vec![term("Target bullet resist", b.target.bullet_resist, "%")];
    t.extend(bullet_shred_terms);
    add("target_bullet_resist", "Target net bullet resist", "target", target_br, "%", "target resist − (1 − Π(1 − shredᵢ))", t);
    let mut t = vec![term("Target spirit resist", b.target.spirit_resist, "%")];
    t.extend(spirit_shred_terms);
    add("target_spirit_resist", "Target net spirit resist", "target", target_sr, "%", "target resist − (1 − Π(1 − shredᵢ))", t);

    let target_dps = dps_sustained * bullet_taken;
    add(
        "target_dps_burst",
        "DPS vs target (burst)",
        "target",
        dps_burst * bullet_taken,
        "",
        "DPS × (1 − target net bullet resist) × (1 + outgoing damage %)",
        vec![term("DPS (burst)", dps_burst, ""), term("Net bullet resist", target_br, "%"), term("Outgoing damage", outgoing, "%")],
    );
    add(
        "target_dps",
        "DPS vs target (with reload)",
        "target",
        target_dps,
        "",
        "DPS × (1 − target net bullet resist) × (1 + outgoing damage %)",
        vec![term("DPS (with reload)", dps_sustained, ""), term("Net bullet resist", target_br, "%"), term("Outgoing damage", outgoing, "%")],
    );
    let shot_vs_target = per_shot * bullet_taken;
    let shots_to_kill = if shot_vs_target > 0.0 { (b.target.health / shot_vs_target).ceil() } else { f64::INFINITY };
    // Time until the killing shot: first shot at t=0, a reload after each magazine.
    let ttk = if shots_to_kill.is_finite() && sps > 0.0 && clip > 0.0 {
        let reloads = ((shots_to_kill - 1.0) / clip).floor();
        (shots_to_kill - 1.0 - reloads) / sps + reloads * (reload + 1.0 / sps)
    } else {
        f64::INFINITY
    };
    add(
        "target_shots",
        "Shots to kill (body)",
        "target",
        shots_to_kill,
        "",
        "ceil(target health ÷ damage per shot)",
        vec![term("Target health", b.target.health, ""), term("Damage per shot", shot_vs_target, "")],
    );
    add(
        "target_ttk",
        "Time to kill (weapon only)",
        "target",
        ttk,
        "s",
        "time between shots + required reloads",
        vec![term("Shots needed", shots_to_kill, ""), term("Shots/s", sps, ""), term("Magazine", clip, ""), term("Reload time", reload, "s")],
    );

    // --- Against troopers and other non-heroes ---
    let npc_terms = bag.take(&["MODIFIER_VALUE_WEAPON_DAMAGE_TO_NPC_INCREASE"]);
    if !npc_terms.is_empty() {
        let scale = 1.0 + total(&npc_terms) / 100.0;
        let mut t = vec![term("Bullet damage", bullet, "")];
        t.extend(npc_terms.clone());
        add("bullet_damage_npc", "Bullet damage vs non-heroes", "weapon", bullet * scale, "", "bullet damage × (1 + weapon damage vs non-heroes %)", t);
        let mut t = vec![term("DPS (burst)", dps_burst, "")];
        t.extend(npc_terms);
        add("dps_npc", "DPS vs non-heroes (burst)", "weapon", dps_burst * scale, "", "DPS × (1 + weapon damage vs non-heroes %)", t);
    }
    let npc_resist_terms = bag.take(&["MODIFIER_VALUE_BULLET_RESIST_NON_HERO"]);
    if !npc_resist_terms.is_empty() {
        let mut t = vec![term("Bullet resist", resists[0], "%")];
        t.extend(npc_resist_terms);
        let value = combine_resist(&t);
        add("bullet_resist_npc", "Bullet resist vs non-heroes", "vitality", value, "%", "1 − Π(1 − rᵢ)  (combined with bullet resist)", t);
    }

    // --- Remaining modifiers that have a known meaning ---
    enum Mode {
        Sum,
        /// The largest single source: slows are assumed not to stack.
        Largest,
        /// 1 − Π(1 − xᵢ), like resists.
        Stack,
    }
    const SUM: &str = "Σ items (additive)";
    const LARGEST: &str = "largest single source (assumed not to stack)";
    for (modifier, key, label, group, mode) in [
        ("MODIFIER_VALUE_HEAL_AMP_CAST_PERCENT", "healing_amp", "Healing effectiveness", "vitality", Mode::Sum),
        ("MODIFIER_VALUE_TECH_DAMAGE_TAKEN_HEALS_ATTACKER", "target_spirit_lifesteal", "Spirit lifesteal vs debuffed target", "vitality", Mode::Sum),
        ("MODIFIER_VALUE_PARRY_COOLDOWN_REDUCTION_FIXED", "parry_cooldown", "Parry cooldown reduction", "vitality", Mode::Sum),
        ("MODIFIER_VALUE_RESPAWN_TIME_PERCENTAGE", "respawn_time", "Respawn time", "vitality", Mode::Sum),
        ("MODIFIER_VALUE_MOVEMENT_SLIDE_DISTANCE_SCALE", "slide_distance", "Slide distance bonus", "vitality", Mode::Sum),
        ("MODIFIER_VALUE_MOVEMENT_SLIDE_TURN_SCALE", "slide_turn", "Slide turning bonus", "vitality", Mode::Sum),
        ("MODIFIER_VALUE_AIR_CONTROL_PERCENT", "air_control", "Air control bonus", "vitality", Mode::Sum),
        ("MODIFIER_VALUE_AIR_CONTROL_ACCEL_PERCENT", "air_acceleration", "Air acceleration bonus", "vitality", Mode::Sum),
        ("MODIFIER_VALUE_MOVE_SPEED_LIMIT", "float_speed", "Float speed while the item is active", "vitality", Mode::Largest),
        ("MODIFIER_VALUE_MELEE_TRAVEL_DISTANCE_PERCENTAGE", "melee_distance", "Heavy melee lunge distance bonus", "weapon", Mode::Sum),
        ("MODIFIER_VALUE_ZOOM_INCREASE_PERCENT", "zoom", "Weapon zoom bonus", "weapon", Mode::Sum),
        ("MODIFIER_VALUE_TECH_DAMAGE_MULTIPLIER", "echo_damage", "Echo damage (share of the imbued ability)", "spirit", Mode::Sum),
        ("MODIFIER_VALUE_MOVEMENT_SPEED_SLOW_PERCENT", "target_slow", "Movement slow on target", "target", Mode::Largest),
        ("MODIFIER_VALUE_FIRE_RATE_SLOW", "target_fire_rate_slow", "Fire rate slow on target", "target", Mode::Largest),
        ("ENEMY:MODIFIER_VALUE_MOVEMENT_GROUND_DASH_REDUCTION_PERCENT", "target_dash", "Dash distance reduction on target", "target", Mode::Largest),
        ("ENEMY:MODIFIER_VALUE_HEAL_AMP_RECEIVE_PERCENT", "target_heal_reduction", "Healing reduction on target", "target", Mode::Stack),
        ("ENEMY:MODIFIER_VALUE_TECH_DAMAGE_MULTIPLIER", "target_spirit_damage", "Spirit damage penalty on target", "target", Mode::Largest),
        ("ENEMY:MODIFIER_VALUE_ALL_DAMAGE_MULTIPLIER", "target_damage", "Damage penalty on target", "target", Mode::Largest),
    ] {
        let terms = bag.take(&[modifier]);
        if terms.is_empty() {
            continue;
        }
        let unit = terms.first().map(|t| t.unit.clone()).unwrap_or_default();
        let (value, formula) = match mode {
            Mode::Sum => (total(&terms), SUM),
            Mode::Largest => (terms.iter().map(|t| t.value.abs()).fold(0.0, f64::max), LARGEST),
            Mode::Stack => (
                100.0 * (1.0 - terms.iter().map(|t| 1.0 - t.value.abs() / 100.0).product::<f64>()),
                "1 − Π(1 − xᵢ)  (sources combine multiplicatively)",
            ),
        };
        add(key, label, group, value, &unit, formula, terms);
    }
    // Barriers are computed with their scaling further down; these only repeat other values or switch a mechanic on.
    bag.take(&[
        "MODIFIER_VALUE_BARRIER_HEALTH",
        "MODIFIER_VALUE_HEAL_AMP_REGEN_PERCENT",
        "ENEMY:MODIFIER_VALUE_HEAL_AMP_REGEN_PERCENT",
        "MODIFIER_VALUE_ENABLE_CHARGES",
        "MODIFIER_VALUE_ANIMATION_FRAME_SKIP_RATE",
        // Always equal to the ability range bonus, which is already shown.
        "MODIFIER_VALUE_TECH_RADIUS_PERCENT",
    ]);

    // --- Anything else ---
    let rest: Vec<(String, Vec<Term>)> = std::mem::take(&mut bag.map).into_iter().collect();
    for (modifier, terms) in rest {
        let label = bag.labels.get(&modifier).cloned().unwrap_or_else(|| {
            modifier.trim_start_matches("ENEMY:").trim_start_matches("MODIFIER_VALUE_").replace('_', " ").to_lowercase()
        });
        let unit = terms.first().map(|t| t.unit.clone()).unwrap_or_default();
        let group = if modifier.starts_with("ENEMY:") { "target" } else { "other" };
        add(&modifier, &label, group, total(&terms), &unit, "Σ items (assumed additive)", terms);
    }

    let (mut charged_cdr, mut charged_spirit) = (0.0, 0.0);
    for p in r.owned.iter().flat_map(|i| i.props.iter()).filter(|p| p.key.ends_with("ChargedAbilities")) {
        match p.modifier.as_deref() {
            Some("MODIFIER_VALUE_COOLDOWN_REDUCTION_PERCENTAGE") => charged_cdr = stack(charged_cdr, p.value),
            Some("MODIFIER_VALUE_TECH_POWER") => charged_spirit += p.value,
            _ => {}
        }
    }
    let ctx = Ctx {
        spirit_power,
        boons,
        cooldown_reduction: spirit_mods[0],
        ult_cooldown_reduction: ult_cdr,
        charge_reduction,
        bonus_charges,
        charged_cdr,
        charged_spirit,
        duration_bonus: spirit_mods[1],
        range_bonus: spirit_mods[2],
        radius_bonus: spirit_mods[3],
        spirit_taken,
        weapon_damage_pct: wd,
        light_melee: (base("light_melee_damage") + boons * melee_growth) * (1.0 + melee_pct / 100.0),
        heavy_melee: (base("heavy_melee_damage") + boons * melee_growth + from_spirit("EHeavyMeleeDamage")) * (1.0 + melee_pct / 100.0),
    };
    let mut imbues = vec![Imbue::default(); hero.abilities.len()];
    for item in r.owned.iter().filter(|i| !i.imbue.is_empty()) {
        let Some(slot) = b.imbues.get(&item.class_name).and_then(|i| imbues.get_mut(*i)) else { continue };
        slot.items.push(item.name.clone());
        if item.props.iter().any(|p| p.modifier.as_deref() == Some("MODIFIER_VALUE_ENABLE_CHARGES")) {
            let value = |key: &str| item.props.iter().find(|p| p.key == key).map(|p| p.value).unwrap_or(0.0);
            slot.enable = Some((value("BonusAbilityCharges"), value("BonusAbilityChargesNonCharge")));
        }
        for p in item.props.iter().filter(|p| p.section != "innate") {
            match p.modifier.as_deref() {
                Some("MODIFIER_VALUE_TECH_POWER") => slot.spirit += p.value,
                Some("MODIFIER_VALUE_COOLDOWN_REDUCTION_PERCENTAGE") => slot.cooldown += p.value,
                Some("MODIFIER_VALUE_BONUS_ABILITY_DURATION_PERCENTAGE") => slot.duration += p.value,
                Some("MODIFIER_VALUE_TECH_RANGE_PERCENT") => slot.range += p.value,
                Some("MODIFIER_VALUE_TECH_RADIUS_PERCENT") => slot.radius += p.value,
                _ => {}
            }
        }
    }
    let copied = b
        .copied_ult
        .as_ref()
        .filter(|class| **class != hero.class_name)
        .and_then(|class| data.hero(class))
        .and_then(|h| h.abilities.get(3));
    let (abilities, ability_points_used) = abilities(hero, b, &ctx, &imbues, copied);
    // Ability cooldown reduction and duration bonuses don't apply to items; item cooldown reduction does.
    let item_ctx = Ctx { cooldown_reduction: item_cdr, duration_bonus: 0.0, ..ctx };
    // Barriers scale with spirit power or boons. Each item raises its own barrier on its own trigger.
    let barrier_terms: Vec<Term> = r
        .owned
        .iter()
        .flat_map(|item| item.props.iter().map(move |p| (item, p)))
        .filter(|(_, p)| p.modifier.as_deref() == Some("MODIFIER_VALUE_BARRIER_HEALTH") && p.value > 0.0)
        .map(|(item, p)| term(item.name.clone(), scaled(p, &item_ctx).value, ""))
        .collect();
    if !barrier_terms.is_empty() {
        let largest = barrier_terms.iter().map(|t| t.value).fold(0.0, f64::max);
        add(
            "barrier",
            "Largest barrier",
            "vitality",
            largest,
            "",
            "each item's barrier with its scaling; they trigger separately, so the largest one is shown",
            barrier_terms.clone(),
        );
        add(
            "barrier_total",
            "Barriers if all are up",
            "vitality",
            total(&barrier_terms),
            "",
            "Σ of every owned barrier",
            barrier_terms,
        );
    }
    let item_effects = r
        .owned
        .iter()
        .filter_map(|item| {
            let props: Vec<ScaledProp> = item
                .props
                .iter()
                .filter(|p| p.modifier.is_none() && !p.label.is_empty())
                .map(|p| {
                    let mut s = scaled(p, &item_ctx);
                    // Build-up is normalized by fire rate so every weapon applies it in about the same time.
                    if p.key == "BuildUpPerShot" && p.value > 0.0 {
                        let rps = if w.buildup_rps > 0.0 { w.buildup_rps } else { w.shots_per_second };
                        s.value = 100.0 / (p.value * (rps + 1.0));
                        s.formula = format!("100 ÷ ({} × ({} shots/s + 1))", fmt(p.value), fmt(rps));
                        if w.pellets > 1.0 {
                            s.note = Some(format!("{}% per pellet", fmt(s.value / w.pellets)));
                        }
                    }
                    s
                })
                .collect();
            (!props.is_empty()).then(|| ItemEffects { item: item.class_name.clone(), name: item.name.clone(), props })
        })
        .collect();

    Ok(Sheet {
        hero: hero.name.clone(),
        level,
        boons: boon_count,
        souls,
        spent: r.spent,
        spent_weapon: spent_w,
        spent_vitality: spent_v,
        spent_spirit: spent_s,
        ability_points,
        ability_points_used,
        slots_used: r.owned.len(),
        slot_limit: SLOT_LIMIT,
        owned: r.owned.iter().map(|i| i.class_name.clone()).collect(),
        steps: r.steps,
        errors: r.errors,
        stats,
        abilities,
        item_effects,
    })
}

#[derive(Debug, Clone, Serialize)]
pub struct Ranked {
    pub item: String,
    pub name: String,
    pub category: String,
    pub cost: u32,
    /// Paid after the component discount.
    pub paid: u32,
    pub delta: f64,
    pub delta_pct: f64,
    pub per_1k: f64,
}

/// Ranks every unowned item by how much it adds to the chosen stat per soul.
/// The level is held fixed so only the item's own effect is measured.
pub fn rank_next(data: &GameData, b: &BuildInput, metric: &str, conditional: bool) -> Result<Vec<Ranked>, String> {
    let base = compute(data, b)?;
    let base_value = base.stat(metric).ok_or_else(|| format!("Unknown stat: {metric}"))?;
    let mut trial = b.clone();
    trial.purchases.truncate(b.upto.unwrap_or(b.purchases.len()).min(b.purchases.len()));
    trial.upto = None;

    let mut out: Vec<Ranked> = data
        .items
        .iter()
        .filter(|i| !base.owned.contains(&i.class_name))
        .filter_map(|item| {
            let mut t = trial.clone();
            t.purchases.push(item.class_name.clone());
            if conditional {
                t.active.push(item.class_name.clone());
            }
            let sheet = compute_at(data, &t, Some(base.level)).ok()?;
            let value = sheet.stat(metric)?;
            let paid = sheet.spent.saturating_sub(base.spent);
            let delta = value - base_value;
            if !delta.is_finite() || delta.abs() < 1e-9 || paid == 0 {
                return None;
            }
            Some(Ranked {
                item: item.class_name.clone(),
                name: item.name.clone(),
                category: item.category.clone(),
                cost: item.cost,
                paid,
                delta,
                delta_pct: if base_value != 0.0 { 100.0 * delta / base_value } else { 0.0 },
                per_1k: delta / paid as f64 * 1000.0,
            })
        })
        .collect();
    out.sort_by(|a, b| b.per_1k.total_cmp(&a.per_1k));
    Ok(out)
}
