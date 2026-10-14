//! Normalized game data, produced from the API's raw JSON by `raw::normalize`.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GameData {
    pub client_version: Option<u64>,
    /// Unix seconds.
    pub fetched_at: u64,
    pub heroes: Vec<Hero>,
    pub items: Vec<Item>,
}

impl GameData {
    pub fn hero(&self, class_name: &str) -> Option<&Hero> {
        self.heroes.iter().find(|h| h.class_name == class_name)
    }

    pub fn item(&self, class_name: &str) -> Option<&Item> {
        self.items.iter().find(|i| i.class_name == class_name)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Hero {
    pub id: u64,
    pub class_name: String,
    pub name: String,
    pub icon: String,
    pub card: String,
    pub hero_type: String,
    /// `starting_stats` (e.g. `max_health`, `sprint_speed`).
    pub base: BTreeMap<String, f64>,
    /// Values gained per boon (`MODIFIER_VALUE_*`).
    pub level_up: BTreeMap<String, f64>,
    /// Base stats that grow with spirit power: the game's stat name (e.g. `EBulletDamage`) -> gain per point.
    #[serde(default)]
    pub spirit_scaling: BTreeMap<String, f64>,
    pub levels: Vec<Level>,
    pub invest: Invest,
    pub weapon: Weapon,
    pub abilities: Vec<Ability>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Level {
    pub level: u32,
    pub required_souls: u32,
    pub ability_points: u32,
    pub ability_unlocks: u32,
    pub boon: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InvestTier {
    pub threshold: u32,
    pub bonus: f64,
}

/// Bonuses granted by souls spent in a category.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Invest {
    pub weapon: Vec<InvestTier>,
    pub vitality: Vec<InvestTier>,
    pub spirit: Vec<InvestTier>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Weapon {
    pub name: String,
    pub bullet_damage: f64,
    pub pellets: f64,
    pub shots_per_second: f64,
    /// Fire rate the game uses for build-up: max spin for spin-up weapons, no intra-burst delay.
    #[serde(default)]
    pub buildup_rps: f64,
    pub clip_size: f64,
    /// Total time from an empty magazine until firing again.
    pub reload_time: f64,
    pub crit_multiplier: f64,
    /// Meters per second.
    #[serde(default)]
    pub bullet_speed: f64,
    /// Distance in meters where bullet damage starts to drop and where it reaches its minimum.
    #[serde(default)]
    pub falloff_start: f64,
    #[serde(default)]
    pub falloff_end: f64,
    /// Share of move speed kept while shooting and while zoomed, 0-1.
    #[serde(default)]
    pub shoot_move_speed: f64,
    #[serde(default)]
    pub zoom_move_speed: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Scale {
    pub class: String,
    pub stat: String,
    pub coef: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Prop {
    pub key: String,
    pub label: String,
    pub value: f64,
    pub postfix: String,
    /// `innate` | `passive` | `active` | `` (tooltip section).
    pub section: String,
    /// `MODIFIER_VALUE_*` for stat-providing properties.
    pub modifier: Option<String>,
    pub scale: Option<Scale>,
    /// The game's display class, e.g. `tech_damage`, `bullet_damage`, `melee_damage`.
    #[serde(default)]
    pub css: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PropUpgrade {
    pub name: String,
    pub bonus: f64,
    /// `` | `EAddToBase` | `EMultiplyBase` | `EAddToScale` | `EMultiplyScale`.
    pub kind: String,
    pub stat_filter: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Ability {
    /// The game's numeric id, as used in match data.
    #[serde(default)]
    pub id: u64,
    pub class_name: String,
    pub name: String,
    pub icon: String,
    pub props: Vec<Prop>,
    /// Ability point tiers (1 / 2 / 5 AP).
    pub upgrades: Vec<Vec<PropUpgrade>>,
    /// The game's own description of the ability, as plain text, and its one-line summary.
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub quip: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Item {
    pub id: u64,
    pub class_name: String,
    pub name: String,
    pub icon: String,
    /// `weapon` | `vitality` | `spirit`.
    pub category: String,
    pub tier: u32,
    pub cost: u32,
    pub active: bool,
    pub components: Vec<String>,
    pub props: Vec<Prop>,
    /// How the item is imbued into an ability: `` | `imbue_modifier_value` | `imbue_active` | `imbue_active_non_ult`.
    #[serde(default)]
    pub imbue: String,
    /// The item's tooltip text without markup, for searching and for the tooltip.
    #[serde(default)]
    pub description: String,
}

impl Item {
    /// Does it grant conditional stats (in the passive/active section)?
    pub fn has_conditional(&self) -> bool {
        self.props
            .iter()
            .any(|p| p.modifier.is_some() && p.section != "innate")
    }
}
