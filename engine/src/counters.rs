//! What a hero's kit threatens the other team with (crowd control, silences, flight, channelled ultimates)
//! and which items answer each of those. Read from the ability and item data wherever the data says it.

use crate::calc::AbilitySheet;
use crate::data::Item;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Threat {
    /// Stuns, sleeps, roots, lifts, hexes and petrifies: the target cannot act or cannot move.
    Lockdown,
    Silence,
    /// Abilities that put their hero in the air, out of reach of most things.
    Flight,
    /// An ultimate that is channelled and so can be interrupted.
    Channel,
}

#[derive(Debug, Clone)]
pub struct Found {
    pub kind: Threat,
    pub ability: String,
    /// Seconds of the effect, or of the channel.
    pub seconds: f64,
}

/// Property names that mean the target is locked down, with the shortest duration worth counting.
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
const SHORTEST_LOCKDOWN: f64 = 0.5;
/// Lockdown the property names do not show: the ability and the property that holds how long it lasts.
const LOCKDOWN_FIXES: [(&str, &str); 1] = [
    // Nurse Harrow, Lurking Terrors: full terror makes the target flee in fear.
    ("ability_nurse_fear", "DebuffDuration"),
];
const SILENCE_KEYS: [&str; 2] = ["SilenceDuration", "SilenceDebuff"];
/// Abilities that lift their hero into the air; the data has no flag for it.
const FLIGHT_ABILITIES: [&str; 4] =
    ["citadel_ability_hornet_leap", "ability_power_jump", "citadel_ability_storm_cloud", "citadel_ability_tengu_airlift"];
/// A channel of at least this long on an ultimate is worth interrupting. Channels listed with a huge number have no real length.
const SHORTEST_CHANNEL: f64 = 1.5;
const LONGEST_CHANNEL: f64 = 100.0;
/// Victor's revive has a channel time in the data but is not something to interrupt.
const NOT_INTERRUPTIBLE: [&str; 1] = ["ability_frank_revive"];

/// The threats in a hero's kit at its current upgrade tiers.
pub fn threats(abilities: &[AbilitySheet]) -> Vec<Found> {
    let mut out = vec![];
    for (index, a) in abilities.iter().enumerate() {
        let longest = |keys: &[&str]| a.props.iter().filter(|p| keys.contains(&p.key.as_str())).map(|p| p.value).fold(0.0, f64::max);
        let mut add = |kind: Threat, seconds: f64| out.push(Found { kind, ability: a.name.clone(), seconds });
        let fixed = LOCKDOWN_FIXES.iter().find(|f| f.0 == a.class_name).map(|f| longest(&[f.1])).unwrap_or(0.0);
        let lockdown = longest(&LOCKDOWN_KEYS).max(fixed);
        if lockdown >= SHORTEST_LOCKDOWN {
            add(Threat::Lockdown, lockdown);
        }
        let silence = longest(&SILENCE_KEYS);
        if silence > 0.0 {
            add(Threat::Silence, silence);
        }
        if FLIGHT_ABILITIES.contains(&a.class_name.as_str()) {
            add(Threat::Flight, longest(&["AbilityDuration"]));
        }
        let channel = longest(&["AbilityChannelTime"]);
        if index == 3 && (SHORTEST_CHANNEL..LONGEST_CHANNEL).contains(&channel) && !NOT_INTERRUPTIBLE.contains(&a.class_name.as_str()) {
            add(Threat::Channel, channel);
        }
    }
    out
}

/// Items that get their owner out of, or shorten, being locked down.
const LOCKDOWN_ITEMS: [&str; 6] = ["Dispel Magic", "Unstoppable", "Indomitable", "Cloak of Opportunity", "Unstable Concoction", "Reactive Barrier"];
const SILENCE_ITEMS: [&str; 4] = ["Dispel Magic", "Unstoppable", "Indomitable", "Divine Barrier"];
/// Items that bring a flying hero down or stop the ability that lifts them.
const FLIGHT_ITEMS: [&str; 4] = ["Knockdown", "Phantom Strike", "Slowing Hex", "Vortex Web"];
/// Items that interrupt a channel.
const CHANNEL_ITEMS: [&str; 4] = ["Knockdown", "Cursed Relic", "Mystical Piano", "Lightning Scroll"];
/// Debuff resist from this much up shortens crowd control enough to count as an answer.
const MEANINGFUL_DEBUFF_RESIST: f64 = 20.0;

/// The threats an item answers.
pub fn answers(item: &Item) -> Vec<Threat> {
    let name = item.name.as_str();
    let resist = item
        .props
        .iter()
        .filter(|p| p.section == "innate" && p.modifier.as_deref() == Some("MODIFIER_VALUE_STATUS_RESISTANCE"))
        .map(|p| p.value)
        .fold(0.0, f64::max);
    let mut out = vec![];
    if LOCKDOWN_ITEMS.contains(&name) || resist >= MEANINGFUL_DEBUFF_RESIST {
        out.push(Threat::Lockdown);
    }
    if SILENCE_ITEMS.contains(&name) || resist >= MEANINGFUL_DEBUFF_RESIST {
        out.push(Threat::Silence);
    }
    if FLIGHT_ITEMS.contains(&name) {
        out.push(Threat::Flight);
    }
    if CHANNEL_ITEMS.contains(&name) {
        out.push(Threat::Channel);
    }
    out
}
