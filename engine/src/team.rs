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
    /// Less damage dealt by the enemy (Fire Scarabs, Inhibitor).
    DamageCut,
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
            Effect::SpiritPowerCut => "cut spirit power or spirit damage",
            Effect::Grounding => "bring flyers down",
            Effect::TeamHeal => "help the team (heals, barriers, team auras)",
            Effect::DamageCut => "cut their damage",
        }
    }

    /// One source is enough for these: healing reduction does not stack, nor does a second disarm or way to
    /// ground flyers. The others add up, but less with every source.
    fn single(self) -> bool {
        matches!(self, Effect::HealingCut | Effect::Disarm | Effect::Grounding | Effect::SpiritPowerCut | Effect::FireRateCut)
    }
}

const LOCKDOWN_KEYS: [&str; 10] = [
    "StunDuration",
    "FreezeDuration",
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
        && (key.contains("Percent") || key.contains("Pct") || key.ends_with("Slow"))
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
        if key == "OutgoingDamagePenaltyPercent" && value < 0.0 {
            add(Effect::DamageCut);
        }
        // Upgrades carry no modifier, so healing reduction is also read by its name (Stalker's Mark's third upgrade).
        if key.starts_with("HealAmpReceive") && value < 0.0 {
            add(Effect::HealingCut);
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
/// Whether an item's own text says it slows enemies. Some items carry slow numbers they no longer use (Glass
/// Cannon), or ones that are not on enemies.
pub fn slows_enemies(item: &Item) -> bool {
    let t = item.description.to_lowercase();
    t.contains("slow") && !t.contains("fire rate slowed") || t.contains("move speed reduc") || t.contains("move speed reduced")
}

/// Whether an item silences enemies (not only their movement items, and not the owner).
pub fn silences_enemies(item: &Item) -> bool {
    ["Silences enemies", "Silence them", "Silencing", "build up to a Silence"].iter().any(|p| item.description.contains(p))
}

/// Whether an item disarms enemies ("immune to ... Disarm" is Unstoppable, the opposite).
pub fn disarms_enemies(item: &Item) -> bool {
    item.description.contains("Disarm") && !item.description.contains("immune to")
}

/// How much an item cuts the outgoing damage of the enemies it hits, percent (Inhibitor). Golden Goose Egg and
/// Cursed Relic cut their owner's own damage with the same property.
pub fn enemy_damage_cut(item: &Item) -> f64 {
    if !item.description.contains("target's outgoing damage") {
        return 0.0;
    }
    item.props.iter().filter(|p| p.key == "OutgoingDamagePenaltyPercent" && p.section != "innate").map(|p| p.value.abs()).fold(0.0, f64::max)
}

pub fn item_effects(item: &Item) -> Vec<Effect> {
    let mut out: Vec<Effect> = effects_of_props(item.props.iter().map(|p| (p.key.as_str(), p.value, p.modifier.as_deref())))
        .into_iter()
        // Healing reduction and resist reduction are read with the same care as everywhere else, and the rest
        // only when the item's own text says it does that to enemies.
        .filter(|e| match e {
            Effect::HealingCut => reduces_healing(item) && heal_cut_of(item) > 0.0,
            Effect::SpiritShred => shreds_spirit(item),
            Effect::BulletShred => shreds_bullet(item),
            Effect::Slow => slows_enemies(item),
            Effect::DamageCut => enemy_damage_cut(item) > 0.0,
            Effect::Silence => silences_enemies(item),
            _ => true,
        })
        .collect();
    if silences_enemies(item) && !out.contains(&Effect::Silence) {
        out.push(Effect::Silence);
    }
    // Capacitor's slow is a "maximum" slow that recovers over time.
    if item.props.iter().any(|p| p.key == "MaxSlowPercent" && p.value > 0.0) && slows_enemies(item) && !out.contains(&Effect::Slow) {
        out.push(Effect::Slow);
    }
    let mut add = |e: Effect| {
        if !out.contains(&e) {
            out.push(e);
        }
    };
    if disarms_enemies(item) {
        add(Effect::Disarm);
    }
    if counters::answers(item).contains(&Threat::Flight) {
        add(Effect::Grounding);
    }
    // Knockdown is on the list of team items for its stun, not for helping allies.
    if TEAM_ITEMS.contains(&item.name.as_str()) && item.name != "Knockdown" {
        add(Effect::TeamHeal);
    }
    out
}

/// What an ability does to enemies, with the upgrades counted.
pub fn ability_effects(a: &Ability) -> Vec<Effect> {
    let base = a.props.iter().map(|p| (p.key.as_str(), p.value, p.modifier.as_deref()));
    let upgrades = a.upgrades.iter().flatten().map(|u| (u.name.as_str(), u.bonus, None));
    let mut out = effects_of_props(base.chain(upgrades));
    if let Some((_, add, drop)) = ABILITY_FIXES.iter().find(|f| f.0 == a.class_name) {
        out.retain(|e| !drop.contains(e));
        for (e, _) in add.iter() {
            if !out.contains(e) {
                out.push(*e);
            }
        }
    }
    out
}

/// What abilities do that their property names do not show, checked one by one against deadlock.wiki: the ability,
/// effects to add or to time by a given property (the property that holds how long they last), and effects to drop.
/// Pulls, knock-ups and launches count as lockdown for as long as the target is displaced.
#[allow(clippy::type_complexity)]
const ABILITY_FIXES: &[(&str, &[(Effect, &str)], &[Effect])] = &[
    // Apollo, Itani Lo Sahn: struck enemies cannot act (or heal) while time-slowed, 1.8s.
    ("ability_fencer_ultimate", &[(Effect::Lockdown, "DebuffDuration")], &[]),
    // Baba, Threadsap: steals spirit power from enemies hit, for 22s (30s upgraded).
    ("ability_baba_bubbling_brew", &[(Effect::SpiritPowerCut, "DebuffDuration")], &[]),
    // Bebop, Exploding Uppercut: launches enemies (0.5s) and gives them -14% fire rate for 5s.
    ("citadel_ability_uppercut", &[(Effect::FireRateCut, "ExplodeDebuffDuration"), (Effect::Lockdown, "TossDuration")], &[]),
    // Bebop, Grapple Arm: pulls the target in; -72% move speed for 0.5s after the pull.
    ("citadel_ability_hook", &[(Effect::Lockdown, "RestrictionDuration"), (Effect::Slow, "RestrictionDuration")], &[]),
    // Bebop, Hyper Beam: -20% move speed all through the 11s beam.
    ("citadel_ability_bebop_laser_beam", &[(Effect::Slow, "AbilityChannelTime")], &[]),
    // Billy, Bashdown: pulls enemies down (0.4s) and knocks them up.
    ("ability_punkgoat_ult", &[(Effect::Lockdown, "TossDuration")], &[]),
    // Billy, Rising Ram: launches the target into the air, then -70% dash and air control for 1.5s.
    ("ability_punkgoat_goatflip", &[(Effect::Lockdown, "AbilityDuration"), (Effect::Slow, "AirControlDebuffDuration")], &[]),
    // Billy, Blasted: Wrecked enemies take +10% bullet damage (+17% upgraded) for 7s.
    ("ability_punkgoat_blasted", &[(Effect::BulletShred, "BulletDamageAmpDuration")], &[]),
    // Billy, Chain Gang: tethered enemies are slowed (up to 35%) for 2.8s, cannot use movement abilities, then pulled in.
    ("ability_punkgoat_tether", &[(Effect::Slow, "AbilityDuration"), (Effect::Lockdown, "PullDuration")], &[]),
    // Celeste, Dazzling Trick: when the barrier breaks, nearby enemies are silenced for 1.75s (3s upgraded).
    ("ability_unicorn_prismaticguard", &[(Effect::Silence, "DebuffDuration")], &[]),
    // Deadman Danny, Chimera Mk1: the landing launches nearby enemies (0.9s). Its "slow" numbers are Danny's own heat.
    ("ability_deadpack_rocketeer", &[(Effect::Lockdown, "TossDuration")], &[Effect::Slow]),
    // Drifter, Rend: the third upgrade silences enemies at close range for 2s.
    ("drifter_blood_blast", &[(Effect::Silence, "DebuffDuration")], &[]),
    // Dynamo, Kinetic Pulse: knocks enemies up for 1s.
    ("citadel_ability_stomp", &[(Effect::Lockdown, "TossDuration")], &[]),
    // Dynamo, Singularity: stuns and pulls enemies in for the whole channel (2.75s, 3.5s upgraded).
    ("citadel_ability_self_vacuum", &[(Effect::Lockdown, "AbilityChannelTime")], &[]),
    // Graves, Essence Theft: steals spirit resist (and weapon damage) from up to 3 enemies he hits.
    ("ability_necro_fear", &[(Effect::SpiritShred, "AbilityDuration")], &[]),
    // Grey Talon, Spirit Snare: the curse (2.25s, 3.25s upgraded) also silences and disarms.
    ("ability_immobilize_trap", &[(Effect::Silence, "TetherDuration"), (Effect::Disarm, "TetherDuration")], &[]),
    // Holliday, Powder Keg: knocks enemies up for 0.4s.
    ("ability_explosive_barrel", &[(Effect::Lockdown, "TossDuration")], &[]),
    // Holliday, Spirit Lasso: lifts and stuns the target for 2.25s (3s upgraded).
    ("ability_gravity_lasso", &[(Effect::Lockdown, "AbilityDuration")], &[]),
    // Infernus, Afterburn: the second upgrade makes burning enemies deal -35% spirit damage.
    ("ability_afterburn", &[(Effect::SpiritPowerCut, "BurnDuration")], &[]),
    // Ivy, Entangling Thorns: the third upgrade immobilizes enemies caught for 2s, for 1.6s.
    ("citadel_ability_tengu_urn", &[(Effect::Lockdown, "EntangleDuration")], &[]),
    // Ivy, Air Drop: the second upgrade slows enemies hit for 3s (not the 21s flight).
    ("citadel_ability_tengu_airlift", &[(Effect::Slow, "DebuffDuration")], &[]),
    // Kelvin, Arctic Beam: ramps up to -20% fire rate (-45% upgraded) and a move slow on enemies in the 5s beam.
    ("ability_icebeam", &[(Effect::FireRateCut, "AbilityDuration"), (Effect::Slow, "AbilityDuration")], &[]),
    // Lady Geist, Malice: each stack slows enemies by 12% for 4s.
    ("ability_blood_shards", &[(Effect::Slow, "SlowDuration")], &[]),
    // Lash, Ground Strike: the second upgrade knocks enemies up (and slows them 40% for 3s).
    ("citadel_ability_lash_down_strike", &[(Effect::Lockdown, "TossDuration")], &[]),
    // Lash, Death Slam: grabbed enemies are lifted and stunned through the 2.3s channel, then slammed.
    ("citadel_ability_lash_ultimate", &[(Effect::Lockdown, "AbilityChannelTime")], &[]),
    // McGinnis, Heavy Barrage: the first upgrade slows enemies hit by the rockets all through the barrage.
    ("citadel_ability_rocket_barrage", &[(Effect::Slow, "AbilityDuration")], &[]),
    // Mina, Love Bites: the first upgrade slows 24% for 3s on a proc, once per target every 10s (5s upgraded).
    ("ability_vampirebat_lovebites", &[(Effect::Slow, "SlowDuration")], &[]),
    // Mina, Nox Nostra: every bat hit silences for 1.25s.
    ("ability_vampirebat_batswarm", &[(Effect::Silence, "DebuffDuration")], &[]),
    // Mirage, Fire Scarabs: -20% outgoing damage (-35% upgraded) for 5s (7s upgraded).
    ("mirage_fire_beetles", &[(Effect::DamageCut, "StealDuration")], &[]),
    // Mirage, Dust Devil: lifts the enemies it passes through for a moment.
    ("mirage_tornado", &[(Effect::Lockdown, "EnemyLiftDuration")], &[]),
    // Mirage, Djinn's Mark: the third upgrade's stun at max marks does not trigger (a 5th mark cannot be added).
    ("mirage_sand_phantom", &[], &[Effect::Lockdown]),
    // Mo & Krill, Burrow: the spin out knocks enemies up for 1s.
    ("ability_burrow", &[(Effect::Lockdown, "TossDuration")], &[]),
    // Mo & Krill, Sand Blast: disarms for 2.5s (4s upgraded).
    ("ability_throw_sand", &[(Effect::Disarm, "AbilityDuration")], &[]),
    // Mo & Krill, Combo: holds and stuns the target through the 2.4s channel (3.1s upgraded).
    ("ability_ult_combo", &[(Effect::Lockdown, "AbilityChannelTime")], &[]),
    // Paradox, Pulse Grenade: slows on every pulse for the whole 3.2s (4.8s upgraded).
    ("citadel_ability_chrono_pulse_grenade", &[(Effect::Slow, "AbilityDuration")], &[]),
    // Paradox, Time Wall: touching it slows 64% for 2s; the second upgrade also silences for 2.3s.
    ("citadel_ability_chrono_time_wall", &[(Effect::Slow, "AuraEffectDuration"), (Effect::Silence, "DebuffDuration")], &[]),
    // Paradox, Kinetic Carbine: time-stops the target for 0.25-0.4s (0.8s upgraded).
    ("citadel_ability_chrono_kinetic_carbine", &[(Effect::Lockdown, "MaxSlowDuration")], &[]),
    // Paradox, Paradoxical Swap: neither side can act during the swap, about 1s.
    ("citadel_ability_chrono_swap", &[(Effect::Lockdown, "SwapTime")], &[]),
    // Pocket, Affliction: the third upgrade prevents all healing for the 10s (13s) debuff.
    ("synth_affliction", &[(Effect::HealingCut, "DebuffDuration")], &[]),
    // Rat King, Rule, Ratannia!: the banner slam knocks enemies up (and slows them 40%).
    ("ability_ratking_standard_bearer", &[(Effect::Lockdown, "TossDuration")], &[]),
    // Rem, Pillow Toss: the fading slow and the second upgrade's -35% fire rate last the 3s debuff.
    ("ability_familiar_ability02", &[(Effect::Slow, "EffectDuration"), (Effect::FireRateCut, "EffectDuration")], &[]),
    // Seven, Lightning Ball: the second upgrade's slow lasts as long as the ball (6s upgraded).
    ("citadel_ability_lightning_ball", &[(Effect::Slow, "MaxLifetime")], &[]),
    // Silver, Boot Kick: briefly immobilizes and kicks the target away; the third upgrade makes it deal -35% damage for 5s.
    ("ability_werewolf_kickflip", &[(Effect::Lockdown, "SlowDuration"), (Effect::DamageCut, "DebuffDuration")], &[]),
    // Silver, Lycan Curse: the werewolf form's own abilities are not in the data: Tail Whack disarms for 2s and Mauling
    // Leap takes -8% bullet resist (-20% upgraded) for 6s, both used through the 15s curse.
    ("ability_werewolf_transformation", &[(Effect::Disarm, "TailWhackDisarm"), (Effect::BulletShred, "AbilityDuration")], &[]),
    // Solomon, Setup Bishop: the third upgrade's squares take -2.5% spirit resist a stack, up to 10.
    ("ability_chessmaster_throwbishop", &[(Effect::SpiritShred, "StackDuration")], &[]),
    // The Doorman, Call Bell: -40% weapon accuracy (and -28% move speed) for 4s on enemies in the blast.
    ("ability_doorman_bomb", &[(Effect::DamageCut, "AbilityDuration")], &[]),
    // The Doorman, Luggage Cart: drags enemy heroes along its path (forced movement).
    ("ability_doorman_luggage_cart", &[(Effect::Lockdown, "CartDragDuration")], &[]),
    // The Doorman, Hotel Guest: sends the target away for 6.5s, out of the fight.
    ("ability_doorman_hotel", &[(Effect::Lockdown, "AbilityDuration")], &[]),
    // Vindicta, Stake: tethers enemies (and silences their movement items) for 1.75s (2.5s upgraded), slowing them.
    ("citadel_ability_hornet_chain", &[(Effect::Lockdown, "ChainDuration"), (Effect::Slow, "ChainDuration")], &[]),
    // Viscous, Splatter: the goo puddles slow 28% for 10s.
    ("viscous_goo_grenade", &[(Effect::Slow, "PuddleDuration")], &[]),
    // Viscous, Puddle Punch: knocks enemies up for 0.6s and slows them 24% for 3s.
    ("viscous_telepunch", &[(Effect::Lockdown, "TossDuration"), (Effect::Slow, "ImpactDuration")], &[]),
    // Warden, Alchemical Flask: -25% weapon damage (-50% upgraded) for 7s; the third upgrade's -30% fire rate lasts as long.
    ("ability_warden_crowd_control", &[(Effect::DamageCut, "DebuffDuration"), (Effect::FireRateCut, "DebuffDuration")], &[]),
    // Warden, Binding Word: silences the target until it escapes or is immobilized (2.8s escape time). The first
    // upgrade is Warden's own bullet damage against the target, not a resist reduction on it.
    ("ability_warden_lock_down", &[(Effect::Silence, "EscapeTime")], &[Effect::BulletShred]),
    // Wraith, Card Trick: diamonds take bullet and spirit resist for 5s (and clubs slow).
    ("citadel_ability_card_toss", &[(Effect::BulletShred, "DiamondResistShredDuration"), (Effect::SpiritShred, "DiamondResistShredDuration")], &[]),
];

/// Abilities whose effect comes back on a timer other than the ability cooldown: (ability, property, seconds when
/// no property holds it).
const COOLDOWN_FIXES: &[(&str, &str, f64)] = &[
    ("ability_vampirebat_lovebites", "PerTargetCooldown", 10.0),
    // Victor, Pain Battery: usable once 40% of his health has been lost, about once a fight rather than every 2s.
    ("ability_frank_shocktarget2", "", 15.0),
];

/// Whether an ability stuns, locks down or silences enemies (fixes included): for an enemy's list of crowd control.
pub fn locks_down(a: &Ability) -> bool {
    ability_effects(a).iter().any(|e| matches!(e, Effect::Lockdown | Effect::Silence))
}

/// Who on a team does an effect: each hero's name, and the abilities or items it does it with.
pub struct TeamEffects {
    /// Effect, hero, the abilities or items it does it with, and how much of the time it does it (1 for an item of
    /// its usual build, the effect's share of the cooldown for an ability).
    pub providers: Vec<(Effect, String, Vec<String>, f64)>,
}

/// An ability's numbers with every upgrade added to the base value of the same property.
/// An upgrade of kind `EMultiplyBase` raises the value by a percentage, `EMultiplyScale` changes its spirit
/// scaling (left out here), and any other kind adds to it.
fn upgraded(a: &Ability) -> Vec<(&str, f64)> {
    let mut by_key: Vec<(&str, f64)> = a.props.iter().map(|p| (p.key.as_str(), p.value)).collect();
    for u in a.upgrades.iter().flatten() {
        let k = u.name.as_str();
        match (u.kind.as_str(), by_key.iter_mut().find(|x| x.0 == k)) {
            ("EMultiplyScale", _) => {}
            ("EMultiplyBase", Some(x)) => x.1 *= 1.0 + u.bonus / 100.0,
            ("EMultiplyBase", None) => {}
            (_, Some(x)) => x.1 += u.bonus,
            (_, None) => by_key.push((k, u.bonus)),
        }
    }
    by_key
}

/// How long one of an ability's effects stays on the enemy, seconds, read from the duration that belongs to it.
fn effect_seconds(effect: Effect, all: &[(&str, f64)]) -> f64 {
    let longest = |test: &dyn Fn(&str) -> bool| all.iter().filter(|(k, v)| *v > 0.0 && test(k)).map(|(_, v)| *v).fold(0.0, f64::max);
    let ability = longest(&|k| k == "AbilityDuration");
    let debuff = longest(&|k| k.contains("Duration") && (k.contains("Debuff") || k.contains("Reduction") || k.contains("Shred")));
    let or = |a: f64, b: f64| if a > 0.0 { a } else { b };
    match effect {
        Effect::Lockdown => longest(&|k| LOCKDOWN_KEYS.contains(&k)),
        Effect::Silence => longest(&|k| k.contains("Silence") && k.contains("Duration")),
        Effect::Slow => or(longest(&|k| k.contains("Slow") && k.contains("Duration")), or(ability, 2.0)),
        Effect::FireRateCut => or(longest(&|k| k.contains("FireRate") && k.contains("Duration")), or(longest(&|k| k.contains("Slow") && k.contains("Duration")), or(debuff, or(ability, 3.0)))),
        _ => or(debuff, or(ability, 3.0)),
    }
}

/// Each effect an ability has on enemies (upgrades counted), with how much of the time it is on: its own duration
/// over the ability's cooldown (with cooldown upgrades). Lash's Flog, fully upgraded, slows fire rate for 3s
/// every 10s. An ability without a cooldown (a passive) does it all the time.
pub fn ability_effects_timed(a: &Ability) -> Vec<(Effect, f64)> {
    let all = upgraded(a);
    // A charged ability without a cooldown of its own (Graves' Jar of Dead) gets a charge back every so often.
    let value = |key: &str| all.iter().find(|x| x.0 == key).map(|x| x.1).unwrap_or(0.0);
    let cooldown = match COOLDOWN_FIXES.iter().find(|c| c.0 == a.class_name) {
        Some((_, key, seconds)) => if value(key) > 0.0 { value(key) } else { *seconds },
        None if value("AbilityCooldown") > 0.0 => value("AbilityCooldown"),
        None => value("AbilityCooldownBetweenCharge").max(0.0),
    };
    ability_effects(a)
        .into_iter()
        .map(|e| {
            let seconds = ability_effect_seconds(a, e, &all);
            (e, if cooldown > 0.0 { (seconds / cooldown).clamp(0.02, 1.0) } else { 1.0 })
        })
        .collect()
}

fn ability_effect_seconds(a: &Ability, e: Effect, all: &[(&str, f64)]) -> f64 {
    let fixed = ABILITY_FIXES.iter().find(|f| f.0 == a.class_name).and_then(|f| f.1.iter().find(|x| x.0 == e)).map(|x| x.1);
    match fixed {
        Some(key) => all.iter().find(|x| x.0 == key).map(|x| x.1).unwrap_or(3.0),
        None => effect_seconds(e, all),
    }
}

/// How long one cast of an ability stuns or locks enemies down, seconds (0 when it does not), upgrades counted.
pub fn ability_lockdown_seconds(a: &Ability) -> f64 {
    if !ability_effects(a).contains(&Effect::Lockdown) {
        return 0.0;
    }
    ability_effect_seconds(a, Effect::Lockdown, &upgraded(a))
}

/// How long an item stuns, freezes or immobilizes an enemy, seconds (0 when it does not).
pub fn item_lockdown_seconds(item: &Item) -> f64 {
    if !item_effects(item).contains(&Effect::Lockdown) {
        return 0.0;
    }
    item.props.iter().filter(|p| LOCKDOWN_KEYS.contains(&p.key.as_str())).map(|p| p.value).fold(0.0, f64::max)
}

/// An ability does its effect to the one or few enemies it hits, an item's passive to everyone its owner hits:
/// a teammate's ability covers this share of what an item would.
const ABILITY_REACH: f64 = 0.5;

impl TeamEffects {
    /// The heroes (by id, with their usual builds as item class names) and what each does.
    pub fn of(data: &GameData, team: &[(u64, Vec<String>)]) -> TeamEffects {
        let mut providers: Vec<(Effect, String, Vec<String>, f64)> = vec![];
        for (id, build) in team {
            let Some(hero) = data.heroes.iter().find(|h| h.id == *id) else { continue };
            let mut by: Vec<(Effect, String, f64)> = vec![];
            for a in &hero.abilities {
                by.extend(ability_effects_timed(a).into_iter().map(|(e, uptime)| (e, a.name.clone(), uptime * ABILITY_REACH)));
            }
            for item in build.iter().filter_map(|c| data.item(c)) {
                by.extend(item_effects(item).into_iter().map(|e| (e, item.name.clone(), 1.0)));
            }
            for (effect, what, uptime) in by {
                match providers.iter_mut().find(|p| p.0 == effect && p.1 == hero.name) {
                    Some(p) => {
                        if !p.2.contains(&what) {
                            p.2.push(what);
                        }
                        p.3 = p.3.max(uptime);
                    }
                    None => providers.push((effect, hero.name.clone(), vec![what], uptime)),
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
            let by: Vec<&(Effect, String, Vec<String>, f64)> = self.providers.iter().filter(|p| p.0 == effect).collect();
            // Each teammate counts for the share of the time it does the effect: an item all the time, an ability
            // for its duration out of its cooldown.
            let sources: f64 = by.iter().map(|p| p.3).sum();
            let f = if effect.single() {
                // Does not stack: the share of the time the team already covers is what the item adds nothing to.
                (1.0 - 0.55 * sources.min(1.0)).max(0.3)
            } else {
                1.0 / (1.0 + 0.35 * sources)
            };
            // Something the team does only now and then is not worth bringing up.
            if f > 0.9 {
                continue;
            }
            factor = factor.min(f);
            let names: Vec<String> = by
                .iter()
                .map(|p| if p.3 < 1.0 { format!("{} ({}, {:.0}% of the time)", p.1, p.2.join(", "), p.3 * 100.0) } else { format!("{} ({})", p.1, p.2.join(", ")) })
                .collect();
            why.push(format!("{} already {}", names.join(" and "), effect.verb()));
        }
        (factor.max(0.3), (!why.is_empty()).then(|| format!("Your team has it: {}", why.join("; "))))
    }
}
