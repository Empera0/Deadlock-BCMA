//! What a team already does to the enemies, from its heroes' kits and their usual builds: slows, stuns, silences,
//! disarms, healing reduction, resist reduction and the like. An item whose job the team already does is worth
//! less: why buy a slow when two teammates' abilities slow already?

use crate::data::{Ability, GameData, Item};
use crate::matches::{heal_cut_of, reduces_healing, shreds_bullet, shreds_spirit, TEAM_ITEMS};
use crate::counters::{self, Threat};

/// Something done to enemies (or for allies) that a team only needs so much of.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Effect {
    Slow,
    Lockdown,
    Silence,
    Disarm,
    HealingCut,
    BulletShred,
    SpiritShred,
    FireRateCut,
    SpiritPowerCut,
    Grounding,
    TeamHeal,
}

impl Effect {
    /// What it is, as said of a team: "Wraith and Bebop already slow".
    pub fn verb(self) -> &'static str {
        match self {
            Effect::Slow => "slow",
            Effect::Lockdown => "stun or lock down",
            Effect::Silence => "silence",
            Effect::Disarm => "disarm",
            Effect::HealingCut => "cut healing",
            Effect::BulletShred => "reduce bullet resist",
            Effect::SpiritShred => "reduce spirit resist",
            Effect::FireRateCut => "cut fire rate",
            Effect::SpiritPowerCut => "cut spirit power",
            Effect::Grounding => "bring flyers down",
            Effect::TeamHeal => "heal the team",
        }
    }

    /// One source is enough for these: healing reduction does not stack, nor does a second disarm or way to
    /// ground flyers. The others add up, but less with every source.
    fn single(self) -> bool {
        matches!(self, Effect::HealingCut | Effect::Disarm | Effect::Grounding | Effect::SpiritPowerCut | Effect::FireRateCut)
    }
}

const LOCKDOWN_KEYS: [&str; 9] = [
    "StunDuration",
    "StompStunDuration",
    "LateCheckoutStun",
    "SleepDuration",
    "PetrifyDuration",
    "HexDuration",
    "LiftDuration",
    "ImmobilizeDuration",
    "TetherDuration",
];
/// A slow on the owner (Metal Skin's, a channel's) is not a slow on enemies.
const OWN_SLOW_KEYS: [&str; 2] = ["ActiveMoveSpeedPenalty", "FireRateSlow"];

fn is_slow(key: &str) -> bool {
    key.contains("Slow")
        && (key.contains("Percent") || key.ends_with("Slow"))
        && !OWN_SLOW_KEYS.contains(&key)
        && !key.contains("FireRate")
        // Slow resistance and slow reduction are for the owner.
        && !key.contains("Resist")
        && !key.contains("Reduction")
}

fn effects_of_props<'a>(props: impl Iterator<Item = (&'a str, f64, Option<&'a str>)>) -> Vec<Effect> {
    let mut out = vec![];
    let mut add = |e: Effect| {
        if !out.contains(&e) {
            out.push(e);
        }
    };
    for (key, value, modifier) in props {
        if is_slow(key) && value > 0.0 {
            add(Effect::Slow);
        }
        if LOCKDOWN_KEYS.contains(&key) && value > 0.0 {
            add(Effect::Lockdown);
        }
        if key.contains("Silence") && key.contains("Duration") && value > 0.0 {
            add(Effect::Silence);
        }
        if key == "FireRateSlow" && value > 0.0 {
            add(Effect::FireRateCut);
        }
        if key == "TechPowerReduction" && value != 0.0 {
            add(Effect::SpiritPowerCut);
        }
        if let Some(m) = modifier {
            if m.contains("HEAL_AMP_RECEIVE") && value < 0.0 {
                add(Effect::HealingCut);
            }
            if m.contains("TECH_RESIST_REDUCTION") {
                add(Effect::SpiritShred);
            }
            if m.contains("BULLET_AND_MELEE_RESIST_REDUCTION") {
                add(Effect::BulletShred);
            }
        }
        if matches!(key, "BulletArmorReduction" | "BulletResistReduction") && value != 0.0 {
            add(Effect::BulletShred);
        }
        if matches!(key, "TechArmorDamageReduction" | "MagicResistReduction") && value != 0.0 {
            add(Effect::SpiritShred);
        }
    }
    out
}

/// What an item does for the team.
pub fn item_effects(item: &Item) -> Vec<Effect> {
    let mut out: Vec<Effect> = effects_of_props(item.props.iter().map(|p| (p.key.as_str(), p.value, p.modifier.as_deref())))
        .into_iter()
        // Healing reduction and resist reduction are read with the same care as everywhere else.
        .filter(|e| match e {
            Effect::HealingCut => reduces_healing(item) && heal_cut_of(item) > 0.0,
            Effect::SpiritShred => shreds_spirit(item),
            Effect::BulletShred => shreds_bullet(item),
            _ => true,
        })
        .collect();
    let mut add = |e: Effect| {
        if !out.contains(&e) {
            out.push(e);
        }
    };
    if item.description.contains("Disarms") {
        add(Effect::Disarm);
    }
    if counters::answers(item).contains(&Threat::Flight) {
        add(Effect::Grounding);
    }
    if TEAM_ITEMS.contains(&item.name.as_str()) {
        add(Effect::TeamHeal);
    }
    out
}

/// What an ability does to enemies, with the upgrades counted.
pub fn ability_effects(a: &Ability) -> Vec<Effect> {
    let base = a.props.iter().map(|p| (p.key.as_str(), p.value, p.modifier.as_deref()));
    let upgrades = a.upgrades.iter().flatten().map(|u| (u.name.as_str(), u.bonus, None));
    effects_of_props(base.chain(upgrades))
}

/// Who on a team does an effect: each hero's name, and the abilities or items it does it with.
pub struct TeamEffects {
    pub providers: Vec<(Effect, String, Vec<String>)>,
}

impl TeamEffects {
    /// The heroes (by id, with their usual builds as item class names) and what each does.
    pub fn of(data: &GameData, team: &[(u64, Vec<String>)]) -> TeamEffects {
        let mut providers: Vec<(Effect, String, Vec<String>)> = vec![];
        for (id, build) in team {
            let Some(hero) = data.heroes.iter().find(|h| h.id == *id) else { continue };
            let mut by: Vec<(Effect, String)> = vec![];
            for a in &hero.abilities {
                by.extend(ability_effects(a).into_iter().map(|e| (e, a.name.clone())));
            }
            for item in build.iter().filter_map(|c| data.item(c)) {
                by.extend(item_effects(item).into_iter().map(|e| (e, item.name.clone())));
            }
            for (effect, what) in by {
                match providers.iter_mut().find(|p| p.0 == effect && p.1 == hero.name) {
                    Some(p) if !p.2.contains(&what) => p.2.push(what),
                    Some(_) => {}
                    None => providers.push((effect, hero.name.clone(), vec![what])),
                }
            }
        }
        TeamEffects { providers }
    }

    /// How much an item is still worth when the team already does what it does (1 = all of it), and why.
    pub fn redundancy(&self, item: &Item) -> (f64, Option<String>) {
        let mut factor: f64 = 1.0;
        let mut why: Vec<String> = vec![];
        for effect in item_effects(item) {
            let by: Vec<&(Effect, String, Vec<String>)> = self.providers.iter().filter(|p| p.0 == effect).collect();
            if by.is_empty() {
                continue;
            }
            let f = match (effect.single(), by.len()) {
                (true, 1) => 0.45,
                (true, _) => 0.3,
                (false, n) => 1.0 / (1.0 + 0.35 * n as f64),
            };
            factor = factor.min(f);
            let names: Vec<String> = by.iter().map(|p| format!("{} ({})", p.1, p.2.join(", "))).collect();
            why.push(format!("{} already {}", names.join(" and "), effect.verb()));
        }
        (factor.max(0.3), (!why.is_empty()).then(|| format!("Your team has it: {}", why.join("; "))))
    }
}
