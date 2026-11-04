//! What each item a player bought gave back in the match: the hero damage it dealt itself, the healing it gave,
//! the enemy healing it prevented, and the teammates' damage its resist reductions added, against what it cost.
//! Items that only raise stats (health, fire rate, spirit power…) leave no trace of their own in the match's
//! records: their worth shows in the player's other numbers, so they are listed but not measured.

use std::collections::BTreeMap;

use serde::Serialize;
use serde_json::Value;

use crate::data::GameData;
use crate::matches::{int, list, MatchReport};

/// An item held for less than this many seconds is not judged.
const MIN_HELD_S: u32 = 120;
/// Of the measured items, one that gave back less than this share of the measured items' typical return per
/// soul is called weak.
const WEAK_SHARE: f64 = 0.33;

#[derive(Debug, Clone, Serialize)]
pub struct ItemPayoff {
    pub item: String,
    pub name: String,
    pub icon: String,
    pub category: String,
    pub cost: u32,
    pub bought_at: u32,
    /// When it left the inventory; `None` when kept to the end.
    pub sold_at: Option<u32>,
    pub held_s: u32,
    pub damage: f64,
    pub healing: f64,
    pub heal_prevented: f64,
    /// Teammates' damage that exists only because of its resist reduction (an estimate).
    pub enabled: f64,
    /// All of the above.
    pub total: f64,
    /// Per 1,000 souls it cost; `None` when the item leaves no record of its own.
    pub per_1k: Option<f64>,
    /// Measured, held long enough, and far below the other measured items.
    pub weak: bool,
}

/// Every item the player bought, in the order bought, with what it gave back.
pub fn payoff(data: &GameData, report: &MatchReport, json: &str, slot: u32) -> Result<Vec<ItemPayoff>, String> {
    let root: Value = serde_json::from_str(json).map_err(|e| format!("Unreadable match data: {e}"))?;
    let info = root.get("match_info").ok_or("The response has no match information")?;
    let p = report.players.iter().find(|p| p.slot == slot).ok_or("No such player in the match")?;
    let enemies: Vec<u32> = report.players.iter().filter(|o| o.team != p.team).map(|o| o.slot).collect();
    let matrix = info.get("damage_matrix").cloned().unwrap_or(Value::Null);
    let details = matrix.get("source_details").cloned().unwrap_or(Value::Null);
    let (types, names) = (list(&details, "stat_type"), list(&details, "source_name"));
    // Totals by (raw source name, stat type): damage and prevented healing on enemies, healing on anyone.
    let mut by_source: BTreeMap<(String, u64), f64> = BTreeMap::new();
    for dealer in list(&matrix, "damage_dealers").iter().filter(|d| int(d, "dealer_player_slot") == slot) {
        for source in list(dealer, "damage_sources") {
            let index = int(source, "source_details_index") as usize;
            let raw = names.get(index).and_then(Value::as_str).unwrap_or("").to_string();
            let kind = types.get(index).and_then(Value::as_u64).unwrap_or(u64::MAX);
            let amount: f64 = list(source, "damage_to_players")
                .iter()
                .filter(|t| kind == 1 || enemies.contains(&int(t, "target_player_slot")))
                .filter_map(|t| list(t, "damage").last().and_then(Value::as_f64))
                .sum();
            *by_source.entry((raw, kind)).or_default() += amount;
        }
    }
    let of = |class: &str, kind: u64| -> f64 {
        by_source.iter().filter(|((raw, k), _)| *k == kind && (raw == class || raw.starts_with(&format!("{class}_")))).map(|(_, v)| v).sum()
    };
    let mut out: Vec<ItemPayoff> = p
        .purchases
        .iter()
        .filter(|x| x.cost > 0)
        .map(|x| {
            let item = data.items.iter().find(|i| i.class_name == x.item);
            let (damage, healing, prevented) = (of(&x.item, 0).max(0.0), of(&x.item, 1).max(0.0), of(&x.item, 2).max(0.0));
            let enabled: f64 = p.enabled.iter().filter(|e| e.source == x.name || e.source == x.item).map(|e| e.bonus).sum();
            let total = damage + healing + prevented + enabled;
            // An item the game credits with nothing is one whose worth is in stats (or whose effect the game
            // records under another source): it is not judged.
            let measured = total > 0.0;
            let held = x.sold_at.unwrap_or(report.duration_s).saturating_sub(x.time);
            ItemPayoff {
                item: x.item.clone(),
                name: x.name.clone(),
                icon: x.icon.clone(),
                category: x.category.clone(),
                cost: x.cost,
                bought_at: x.time,
                sold_at: x.sold_at,
                held_s: held,
                damage,
                healing,
                heal_prevented: prevented,
                enabled,
                total,
                per_1k: measured.then(|| total / x.cost as f64 * 1000.0),
                weak: false,
            }
            .with_category(item.map(|i| i.category.clone()))
        })
        .collect();
    // An item judged against the others measured in the same match, per soul and per minute held.
    let rate = |x: &ItemPayoff| x.per_1k.map(|v| v / (x.held_s.max(1) as f64 / 60.0));
    let mut rates: Vec<f64> = out.iter().filter(|x| x.held_s >= MIN_HELD_S).filter_map(rate).collect();
    rates.sort_by(f64::total_cmp);
    if rates.len() >= 3 {
        let typical = rates[rates.len() / 2];
        for x in &mut out {
            x.weak = x.held_s >= MIN_HELD_S && rate(x).is_some_and(|r| r < typical * WEAK_SHARE);
        }
    }
    Ok(out)
}

impl ItemPayoff {
    fn with_category(mut self, category: Option<String>) -> Self {
        if let Some(c) = category {
            self.category = c;
        }
        self
    }
}
