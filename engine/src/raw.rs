//! Converts deadlock-api `/v1/assets/heroes` and `/v1/assets/items` output into `GameData`.

use crate::data::*;
use serde_json::Value;
use std::collections::{BTreeMap, HashMap, HashSet};

/// "15m", "45", 9 style values: reads the leading number.
fn num(v: &Value) -> Option<f64> {
    match v {
        Value::Number(n) => n.as_f64(),
        Value::String(s) => {
            let s = s.trim();
            let end = s
                .char_indices()
                .find(|(i, c)| !(c.is_ascii_digit() || *c == '.' || (*i == 0 && (*c == '-' || *c == '+'))))
                .map(|(i, _)| i)
                .unwrap_or(s.len());
            s[..end].parse().ok()
        }
        _ => None,
    }
}

fn text(v: &Value, key: &str) -> String {
    v.get(key).and_then(Value::as_str).unwrap_or_default().to_string()
}

fn f(v: &Value, key: &str) -> f64 {
    v.get(key).and_then(num).unwrap_or(0.0)
}

/// Game units per meter.
pub const UNITS_PER_METER: f64 = 39.37;

/// Removes HTML tags and collapses whitespace.
fn plain(html: &str) -> String {
    let mut out = String::new();
    let mut depth = 0u32;
    for c in html.chars() {
        match c {
            '<' => {
                depth += 1;
                out.push(' ');
            }
            '>' => depth = depth.saturating_sub(1),
            _ if depth == 0 => out.push(c),
            _ => {}
        }
    }
    out.replace("&amp;", "&").split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Every piece of tooltip text of an item, each once.
fn description(item: &Value) -> String {
    let mut parts: Vec<String> = vec![];
    let mut add = |text: Option<&str>| {
        let text = plain(text.unwrap_or_default());
        if !text.is_empty() && !parts.contains(&text) {
            parts.push(text);
        }
    };
    if let Some(sections) = item.get("tooltip_sections").and_then(Value::as_array) {
        for attribute in sections.iter().filter_map(|s| s.get("section_attributes").and_then(Value::as_array)).flatten() {
            add(attribute.get("loc_string").and_then(Value::as_str));
        }
    }
    if let Some(texts) = item.get("description").and_then(Value::as_object) {
        for text in texts.values() {
            add(text.as_str());
        }
    }
    parts.join(" ")
}

fn first_text(v: &Value, keys: &[&str]) -> String {
    keys.iter()
        .map(|k| text(v, k))
        .find(|s| !s.is_empty())
        .unwrap_or_default()
}

fn prop(key: &str, p: &Value) -> (Prop, bool) {
    let value = p.get("value").and_then(num).unwrap_or(0.0);
    let disabled = p.get("disable_value").and_then(num) == Some(value);
    let scale = p.get("scale_function").and_then(|sf| {
        let coef = sf.get("stat_scale").and_then(num)?;
        Some(Scale {
            class: text(sf, "class_name"),
            stat: text(sf, "specific_stat_scale_type"),
            coef,
        })
    });
    // A size that grows with the ability range or ability radius stat, alone or among several.
    let grows = p
        .get("scale_function")
        .map(|sf| {
            let mut stats = vec![text(sf, "specific_stat_scale_type")];
            stats.extend(sf.get("scaling_stats").and_then(Value::as_array).into_iter().flatten().filter_map(Value::as_str).map(String::from));
            // The tech range and tech duration scale functions grow with their stat without naming it.
            let class = text(sf, "class_name");
            if stats.iter().any(|s| s == "ETechRadius") {
                "radius"
            } else if class.starts_with("scale_function_tech_range") || stats.iter().any(|s| s == "ETechRange") {
                "range"
            } else if class.starts_with("scale_function_tech_duration") || stats.iter().any(|s| s == "ETechDuration") {
                "duration"
            } else {
                ""
            }
        })
        .unwrap_or("")
        .to_string();
    let modifier = Some(text(p, "provided_property_type"))
        .filter(|m| !m.is_empty() && m != "MODIFIER_VALUE_INVALID");
    (
        Prop {
            key: key.to_string(),
            label: text(p, "label"),
            value,
            postfix: text(p, "postfix"),
            section: text(p, "tooltip_section"),
            modifier,
            scale,
            css: text(p, "css_class"),
            grows,
        },
        disabled,
    )
}

fn upgrades(item: &Value) -> Vec<Vec<PropUpgrade>> {
    let Some(list) = item.get("upgrades").and_then(Value::as_array) else {
        return vec![];
    };
    list.iter()
        .map(|u| {
            u.get("property_upgrades")
                .and_then(Value::as_array)
                .map(|ps| {
                    ps.iter()
                        .map(|p| PropUpgrade {
                            name: text(p, "name"),
                            bonus: f(p, "bonus"),
                            kind: text(p, "upgrade_type"),
                            stat_filter: Some(text(p, "scale_stat_filter")).filter(|s| !s.is_empty()),
                        })
                        .collect()
                })
                .unwrap_or_default()
        })
        .collect()
}

fn ability(item: &Value) -> Ability {
    let upgrades = upgrades(item);
    let referenced: HashSet<&str> = upgrades.iter().flatten().map(|u| u.name.as_str()).collect();
    let props = item
        .get("properties")
        .and_then(Value::as_object)
        .map(|m| {
            m.iter()
                .filter_map(|(k, p)| {
                    let (p, disabled) = prop(k, p);
                    let has_scale = p.scale.as_ref().is_some_and(|s| s.coef != 0.0);
                    let live = (p.value != 0.0 && !disabled) || has_scale || referenced.contains(k.as_str());
                    // Upgrades often target unlabelled properties such as a debuff's duration.
                    // Unlabelled properties stay too: summon counts, attack intervals and the like are
                    // never shown in game but decide how an ability plays out.
                    live.then_some(p)
                })
                .collect()
        })
        .unwrap_or_default();
    Ability {
        id: f(item, "id") as u64,
        class_name: text(item, "class_name"),
        name: text(item, "name"),
        icon: first_text(item, &["image_webp", "image"]),
        props,
        upgrades,
        description: plain(item.get("description").and_then(|d| d.get("desc")).and_then(Value::as_str).unwrap_or_default()),
        quip: plain(item.get("description").and_then(|d| d.get("quip")).and_then(Value::as_str).unwrap_or_default()),
    }
}

/// Items whose passive effect has no condition: it is simply always on. The game data files it
/// under the passive section like the conditional ones, so these are named here.
const ALWAYS_ON_ITEMS: [&str; 8] = [
    "Fleetfoot",
    "Superior Cooldown",
    "Transcendent Cooldown",
    "Healing Booster",
    "Enduring Speed",
    "Greater Expansion",
    "Debuff Reducer",
    "Mystic Conduit",
];

fn shop_item(item: &Value) -> Option<Item> {
    let shopable = item.get("shopable").and_then(Value::as_bool).unwrap_or(false);
    let disabled = item.get("disabled").and_then(Value::as_bool).unwrap_or(false);
    let cost = f(item, "cost") as u32;
    if text(item, "type") != "upgrade" || !shopable || disabled || cost == 0 {
        return None;
    }
    let always_on = ALWAYS_ON_ITEMS.contains(&text(item, "name").as_str());
    let props: Vec<Prop> = item
        .get("properties")
        .and_then(Value::as_object)
        .map(|m| {
            m.iter()
                .filter_map(|(k, p)| {
                    let (p, disabled) = prop(k, p);
                    let has_scale = p.scale.as_ref().is_some_and(|s| s.coef != 0.0);
                    let stat = p.modifier.is_some() && p.value != 0.0 && !disabled;
                    // Generic ability fields use negative values for "none" and only add noise.
                    let noise = (k.starts_with("Ability") && p.value < 0.0) || k == "ChannelMoveSpeed";
                    let in_section = !p.section.is_empty() && (p.value != 0.0 || has_scale);
                    let shown = !p.label.is_empty() && !noise && (in_section || (p.value != 0.0 && !disabled));
                    (stat || shown).then_some(p)
                })
                .map(|mut p| {
                    if always_on && p.modifier.is_some() && p.value > 0.0 && p.section != "active" {
                        p.section = "innate".into();
                    }
                    p
                })
                .collect()
        })
        .unwrap_or_default();
    let components = item
        .get("component_items")
        .and_then(Value::as_array)
        .map(|a| a.iter().filter_map(|c| c.as_str().map(String::from)).collect())
        .unwrap_or_default();
    Some(Item {
        id: f(item, "id") as u64,
        class_name: text(item, "class_name"),
        name: text(item, "name"),
        icon: first_text(item, &["shop_image_webp", "shop_image", "image_webp", "image"]),
        category: text(item, "item_slot_type"),
        tier: f(item, "item_tier") as u32,
        cost,
        active: item.get("is_active_item").and_then(Value::as_bool).unwrap_or(false),
        components,
        props,
        imbue: text(item, "imbue"),
        description: description(item),
    })
}

fn weapon(item: Option<&&Value>) -> Weapon {
    let Some(item) = item else { return Weapon::default() };
    let wi = item.get("weapon_info").cloned().unwrap_or(Value::Null);
    let sps = f(&wi, "shots_per_second");
    let sps_reload = f(&wi, "shots_per_second_with_reload");
    let clip = f(&wi, "clip_size");
    let reload_time = if sps > 0.0 && sps_reload > 0.0 && clip > 0.0 {
        clip / sps_reload - clip / sps
    } else {
        f(&wi, "reload_duration")
    };
    let cycle = if wi.get("spins_up").and_then(Value::as_bool).unwrap_or(false) {
        f(&wi, "max_spin_cycle_time")
    } else {
        f(&wi, "cycle_time")
    };
    let buildup_rps = if f(&wi, "burst_shot_count") <= 1.0 && cycle > 0.0 { 1.0 / cycle } else { sps };
    Weapon {
        name: text(item, "name"),
        bullet_damage: f(&wi, "bullet_damage"),
        pellets: f(&wi, "bullets").max(1.0),
        shots_per_second: sps,
        buildup_rps,
        clip_size: clip,
        reload_time,
        crit_multiplier: f(&wi, "crit_bonus_start"),
        bullet_speed: f(&wi, "bullet_speed") / UNITS_PER_METER,
        falloff_start: f(&wi, "damage_falloff_start_range") / UNITS_PER_METER,
        falloff_end: f(&wi, "damage_falloff_end_range") / UNITS_PER_METER,
        shoot_move_speed: f(&wi, "shoot_move_speed_percent"),
        zoom_move_speed: f(&wi, "zoom_move_speed_percent"),
    }
}

fn invest_tiers(v: Option<&Value>) -> Vec<InvestTier> {
    v.and_then(Value::as_array)
        .map(|a| {
            a.iter()
                .map(|t| InvestTier {
                    threshold: f(t, "gold_threshold") as u32,
                    bonus: f(t, "bonus"),
                })
                .collect()
        })
        .unwrap_or_default()
}

fn hero(h: &Value, by_class: &HashMap<&str, &Value>) -> Option<Hero> {
    let selectable = h.get("player_selectable").and_then(Value::as_bool).unwrap_or(false);
    let disabled = h.get("disabled").and_then(Value::as_bool).unwrap_or(false);
    if !selectable || disabled {
        return None;
    }
    let base: BTreeMap<String, f64> = h
        .get("starting_stats")
        .and_then(Value::as_object)
        .map(|m| {
            m.iter()
                .filter_map(|(k, v)| Some((k.clone(), v.get("value").and_then(num)?)))
                .collect()
        })
        .unwrap_or_default();
    let level_up = h
        .get("standard_level_up_upgrades")
        .and_then(Value::as_object)
        .map(|m| m.iter().filter_map(|(k, v)| Some((k.clone(), num(v)?))).collect())
        .unwrap_or_default();
    let mut levels: Vec<Level> = h
        .get("level_info")
        .and_then(Value::as_object)
        .map(|m| {
            m.iter()
                .filter_map(|(k, v)| {
                    let count = |name: &str| {
                        v.get("bonus_currencies")
                            .and_then(Value::as_array)
                            .map(|a| a.iter().filter(|c| c.as_str() == Some(name)).count() as u32)
                            .unwrap_or(0)
                    };
                    Some(Level {
                        level: k.parse().ok()?,
                        required_souls: f(v, "required_gold") as u32,
                        ability_points: count("EAbilityPoints"),
                        ability_unlocks: count("EAbilityUnlocks"),
                        boon: v.get("use_standard_upgrade").and_then(Value::as_bool).unwrap_or(false),
                    })
                })
                .collect()
        })
        .unwrap_or_default();
    levels.sort_by_key(|l| l.level);

    let slots = h.get("items").cloned().unwrap_or(Value::Null);
    let abilities = ["signature1", "signature2", "signature3", "signature4"]
        .iter()
        .filter_map(|s| by_class.get(slots.get(*s)?.as_str()?))
        .map(|a| ability(a))
        .collect();
    let cb = h.get("cost_bonuses");
    let images = h.get("images").cloned().unwrap_or(Value::Null);
    Some(Hero {
        id: f(h, "id") as u64,
        class_name: text(h, "class_name"),
        name: text(h, "name"),
        icon: first_text(&images, &["icon_image_small_webp", "icon_image_small"]),
        card: first_text(&images, &["icon_hero_card_webp", "icon_hero_card"]),
        hero_type: text(h, "hero_type"),
        base,
        level_up,
        spirit_scaling: h
            .get("scaling_stats")
            .and_then(Value::as_object)
            .map(|m| {
                m.iter()
                    .filter(|(_, v)| text(v, "scaling_stat") == "ETechPower")
                    .map(|(k, v)| (k.clone(), f(v, "scale")))
                    .filter(|(_, scale)| *scale != 0.0)
                    .collect()
            })
            .unwrap_or_default(),
        levels,
        invest: Invest {
            weapon: invest_tiers(cb.and_then(|c| c.get("weapon"))),
            vitality: invest_tiers(cb.and_then(|c| c.get("vitality"))),
            spirit: invest_tiers(cb.and_then(|c| c.get("spirit"))),
        },
        weapon: weapon(slots.get("weapon_primary").and_then(Value::as_str).and_then(|c| by_class.get(c))),
        abilities,
    })
}

pub fn normalize(
    heroes_json: &str,
    items_json: &str,
    client_version: Option<u64>,
    fetched_at: u64,
) -> Result<GameData, String> {
    let heroes: Vec<Value> = serde_json::from_str(heroes_json).map_err(|e| format!("failed to parse hero data: {e}"))?;
    let items: Vec<Value> = serde_json::from_str(items_json).map_err(|e| format!("failed to parse item data: {e}"))?;
    let by_class: HashMap<&str, &Value> = items
        .iter()
        .filter_map(|i| Some((i.get("class_name")?.as_str()?, i)))
        .collect();

    let mut out_heroes: Vec<Hero> = heroes.iter().filter_map(|h| hero(h, &by_class)).collect();
    out_heroes.sort_by(|a, b| a.name.cmp(&b.name));
    let mut out_items: Vec<Item> = items.iter().filter_map(shop_item).collect();
    out_items.sort_by(|a, b| (a.tier, &a.name).cmp(&(b.tier, &b.name)));

    if out_heroes.is_empty() || out_items.is_empty() {
        return Err("data came back empty (the API format may have changed)".into());
    }
    Ok(GameData {
        client_version,
        fetched_at,
        heroes: out_heroes,
        items: out_items,
    })
}
