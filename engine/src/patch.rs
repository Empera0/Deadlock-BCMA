//! What a patch changed: the game data before and after, compared number by number. Heroes (their base stats,
//! gun and growth per boon), their abilities (every labelled value and every upgrade) and items (cost and every
//! labelled value), added and removed ones too.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::data::{Ability, GameData, Hero, Item, Prop, PropUpgrade};

/// A hero's base stats worth showing, by their key, with the label they are shown under.
const BASE: [(&str, &str); 13] = [
    ("max_health", "Max Health"),
    ("base_health_regen", "Health Regen"),
    ("max_move_speed", "Move Speed"),
    ("sprint_speed", "Sprint Speed"),
    ("stamina", "Stamina"),
    ("stamina_regen_per_second", "Stamina Recovery"),
    ("light_melee_damage", "Light Melee Damage"),
    ("heavy_melee_damage", "Heavy Melee Damage"),
    ("reload_speed", "Reload Speed"),
    ("bullet_armor_damage_reduction", "Bullet Resist"),
    ("tech_armor_damage_reduction", "Spirit Resist"),
    ("ground_dash_distance_in_meters", "Dash Distance"),
    ("air_dash_distance_in_meters", "Air Dash Distance"),
];
/// What a hero gains with each boon.
const PER_BOON: [(&str, &str); 4] = [
    ("MODIFIER_VALUE_BASE_BULLET_DAMAGE_FROM_LEVEL", "Bullet Damage per Boon"),
    ("MODIFIER_VALUE_BASE_HEALTH_FROM_LEVEL", "Health per Boon"),
    ("MODIFIER_VALUE_BASE_MELEE_DAMAGE_FROM_LEVEL", "Melee Damage per Boon"),
    ("MODIFIER_VALUE_TECH_POWER", "Spirit Power per Boon"),
];

/// Values a change is measured in closer than this are the same (float noise in the game files).
const SAME: f64 = 1e-6;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Value {
    pub label: String,
    /// `None` when the value is new, or gone.
    pub from: Option<f64>,
    pub to: Option<f64>,
    pub postfix: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Change {
    /// hero, ability or item.
    pub kind: String,
    pub class_name: String,
    pub name: String,
    pub icon: String,
    /// The hero an ability belongs to: its name, and its class name.
    pub owner: Option<String>,
    pub hero: Option<String>,
    /// changed, added or removed.
    pub status: String,
    pub values: Vec<Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PatchNotes {
    pub from_version: Option<u64>,
    pub to_version: Option<u64>,
    /// When each side was read, Unix seconds.
    pub from_fetched: u64,
    pub to_fetched: u64,
    pub changes: Vec<Change>,
}

fn differ(a: f64, b: f64) -> bool {
    (a - b).abs() > SAME * a.abs().max(b.abs()).max(1.0)
}

/// Values compared by key: changed, new and gone ones, in the order of the newer side.
fn values(old: &[(String, String, f64, String)], new: &[(String, String, f64, String)]) -> Vec<Value> {
    let before: BTreeMap<&str, f64> = old.iter().map(|(k, _, v, _)| (k.as_str(), *v)).collect();
    let after: BTreeMap<&str, f64> = new.iter().map(|(k, _, v, _)| (k.as_str(), *v)).collect();
    let mut out: Vec<Value> = new
        .iter()
        .filter_map(|(key, label, to, postfix)| match before.get(key.as_str()) {
            Some(from) if !differ(*from, *to) => None,
            from => Some(Value { label: label.clone(), from: from.copied(), to: Some(*to), postfix: postfix.clone() }),
        })
        .collect();
    out.extend(old.iter().filter(|(k, ..)| !after.contains_key(k.as_str())).map(|(_, label, from, postfix)| Value {
        label: label.clone(),
        from: Some(*from),
        to: None,
        postfix: postfix.clone(),
    }));
    out
}

/// The labelled values of props: the ones without a label are the game's own bookkeeping.
fn props(props: &[Prop]) -> Vec<(String, String, f64, String)> {
    props.iter().filter(|p| !p.label.trim().is_empty()).map(|p| (p.key.clone(), p.label.clone(), p.value, p.postfix.clone())).collect()
}

/// An ability's upgrades, as values keyed by tier and the value they change.
fn upgrades(ability: &Ability) -> Vec<(String, String, f64, String)> {
    let label = |up: &PropUpgrade| ability.props.iter().find(|p| p.key == up.name).map(|p| (p.label.clone(), p.postfix.clone()));
    ability
        .upgrades
        .iter()
        .enumerate()
        .flat_map(|(tier, ups)| {
            ups.iter().filter_map(move |up| {
                let (name, postfix) = label(up).filter(|(l, _)| !l.trim().is_empty())?;
                // A bonus that adds to a scaling is a different value from a flat one of the same name.
                let scaling = if up.kind.is_empty() { "" } else { " (scaling)" };
                Some((
                    format!("{tier}:{}:{}", up.name, up.kind),
                    format!("Upgrade {}: {name}{scaling}", tier + 1),
                    up.bonus,
                    if scaling.is_empty() { postfix } else { String::new() },
                ))
            })
        })
        .collect()
}

fn hero_values(h: &Hero) -> Vec<(String, String, f64, String)> {
    let mut out: Vec<(String, String, f64, String)> = vec![];
    for (key, label) in BASE {
        if let Some(v) = h.base.get(key) {
            out.push((key.into(), label.into(), *v, String::new()));
        }
    }
    for (key, label) in PER_BOON {
        if let Some(v) = h.level_up.get(key) {
            out.push((key.into(), label.into(), *v, String::new()));
        }
    }
    let w = &h.weapon;
    for (key, label, v) in [
        ("bullet_damage", "Bullet Damage", w.bullet_damage),
        ("pellets", "Bullets per Shot", w.pellets),
        ("shots_per_second", "Shots per Second", w.shots_per_second),
        ("clip_size", "Ammo", w.clip_size),
        ("reload_time", "Reload Time", w.reload_time),
        ("crit_multiplier", "Headshot Multiplier", w.crit_multiplier),
        ("falloff_start", "Falloff Start", w.falloff_start),
        ("falloff_end", "Falloff End", w.falloff_end),
    ] {
        out.push((key.into(), label.into(), v, String::new()));
    }
    out
}

fn item_values(i: &Item) -> Vec<(String, String, f64, String)> {
    let mut out = vec![("cost".to_string(), "Cost".to_string(), i.cost as f64, String::new())];
    out.extend(props(&i.props));
    out
}

fn change(kind: &str, class_name: &str, name: &str, icon: &str, owner: Option<&Hero>, status: &str, values: Vec<Value>) -> Change {
    Change {
        kind: kind.into(),
        class_name: class_name.into(),
        name: name.into(),
        icon: icon.into(),
        owner: owner.map(|h| h.name.clone()),
        hero: owner.map(|h| h.class_name.clone()),
        status: status.into(),
        values,
    }
}

/// Everything that changed from `old` to `new`: heroes with their abilities, then items by tier.
pub fn diff(old: &GameData, new: &GameData) -> PatchNotes {
    let mut changes = vec![];
    let mut heroes: Vec<&Hero> = new.heroes.iter().collect();
    heroes.sort_by(|a, b| a.name.cmp(&b.name));
    for h in heroes {
        let Some(before) = old.hero(&h.class_name) else {
            changes.push(change("hero", &h.class_name, &h.name, &h.icon, None, "added", vec![]));
            continue;
        };
        let v = values(&hero_values(before), &hero_values(h));
        if !v.is_empty() {
            changes.push(change("hero", &h.class_name, &h.name, &h.icon, None, "changed", v));
        }
        for a in &h.abilities {
            let Some(was) = before.abilities.iter().find(|x| x.class_name == a.class_name) else {
                changes.push(change("ability", &a.class_name, &a.name, &a.icon, Some(h), "added", vec![]));
                continue;
            };
            let mut v = values(&props(&was.props), &props(&a.props));
            v.extend(values(&upgrades(was), &upgrades(a)));
            if !v.is_empty() {
                changes.push(change("ability", &a.class_name, &a.name, &a.icon, Some(h), "changed", v));
            }
        }
        for a in before.abilities.iter().filter(|a| !h.abilities.iter().any(|x| x.class_name == a.class_name)) {
            changes.push(change("ability", &a.class_name, &a.name, &a.icon, Some(h), "removed", vec![]));
        }
    }
    for h in old.heroes.iter().filter(|h| new.hero(&h.class_name).is_none()) {
        changes.push(change("hero", &h.class_name, &h.name, &h.icon, None, "removed", vec![]));
    }

    let mut items: Vec<&Item> = new.items.iter().collect();
    items.sort_by(|a, b| a.tier.cmp(&b.tier).then(a.name.cmp(&b.name)));
    for i in items {
        match old.item(&i.class_name) {
            None => changes.push(change("item", &i.class_name, &i.name, &i.icon, None, "added", vec![])),
            Some(before) => {
                let v = values(&item_values(before), &item_values(i));
                if !v.is_empty() {
                    changes.push(change("item", &i.class_name, &i.name, &i.icon, None, "changed", v));
                }
            }
        }
    }
    for i in old.items.iter().filter(|i| new.item(&i.class_name).is_none()) {
        changes.push(change("item", &i.class_name, &i.name, &i.icon, None, "removed", vec![]));
    }

    PatchNotes { from_version: old.client_version, to_version: new.client_version, from_fetched: old.fetched_at, to_fetched: new.fetched_at, changes }
}
