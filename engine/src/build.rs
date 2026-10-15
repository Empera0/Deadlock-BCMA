//! Build definition and shop rules (ordered purchases, component upgrades, slot limit).

use crate::data::{GameData, Item};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Total item slots in game (9 + 3 flex). An assumption until verified.
pub const SLOT_LIMIT: usize = 12;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Target {
    pub health: f64,
    pub bullet_resist: f64,
    pub spirit_resist: f64,
}

impl Default for Target {
    fn default() -> Self {
        Target { health: 2000.0, bullet_resist: 0.0, spirit_resist: 0.0 }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct BuildInput {
    pub hero: String,
    /// Item `class_name`s in purchase order.
    pub purchases: Vec<String>,
    /// Only count the first N purchases (timeline).
    pub upto: Option<usize>,
    /// Total souls earned. Falls back to souls spent when empty.
    pub souls: Option<u32>,
    /// Items whose conditional (passive/active) effects are switched on.
    pub active: Vec<String>,
    /// Tier (0-3) for each ability.
    pub ability_tiers: Vec<u8>,
    pub target: Target,
    /// Hero `class_name` whose ultimate Sinclair's Audience Participation has copied.
    pub copied_ult: Option<String>,
    /// Imbue items: item `class_name` -> index of the ability it is imbued into.
    pub imbues: BTreeMap<String, usize>,
    /// Temporary spirit power from a buff. Set by the combo simulation, never saved with a build.
    #[serde(skip)]
    pub bonus_spirit: f64,
    #[serde(skip)]
    pub bonus_spirit_pct: f64,
    /// Fire rate taken away by an enemy's item, percent points. Set when an item's effect on this hero is
    /// measured, never saved with a build.
    #[serde(skip)]
    pub fire_rate_slow: f64,
}

#[derive(Debug, Clone, Serialize)]
pub struct Step {
    pub item: String,
    pub name: String,
    /// Paid after the component discount.
    pub paid: u32,
    pub total_spent: u32,
}

pub struct Resolved<'a> {
    pub owned: Vec<&'a Item>,
    pub steps: Vec<Step>,
    pub errors: Vec<String>,
    pub spent: u32,
}

pub fn resolve<'a>(data: &'a GameData, b: &BuildInput) -> Resolved<'a> {
    let n = b.upto.unwrap_or(b.purchases.len()).min(b.purchases.len());
    let mut r = Resolved { owned: vec![], steps: vec![], errors: vec![], spent: 0 };
    for class in &b.purchases[..n] {
        let Some(item) = data.item(class) else {
            r.errors.push(format!("Unknown item: {class}"));
            continue;
        };
        if r.owned.iter().any(|o| o.class_name == item.class_name) {
            r.errors.push(format!("{} is already purchased", item.name));
            continue;
        }
        let mut paid = item.cost;
        for c in &item.components {
            if let Some(pos) = r.owned.iter().position(|o| &o.class_name == c) {
                paid = paid.saturating_sub(r.owned[pos].cost);
                r.owned.remove(pos);
            }
        }
        r.owned.push(item);
        r.spent += paid;
        r.steps.push(Step {
            item: item.class_name.clone(),
            name: item.name.clone(),
            paid,
            total_spent: r.spent,
        });
        if r.owned.len() > SLOT_LIMIT {
            r.errors.push(format!("No free slot for {} (all {SLOT_LIMIT} slots are full)", item.name));
        }
    }
    r
}
