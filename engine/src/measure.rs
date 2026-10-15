//! What an item does to an enemy's damage, worked out by the calculator instead of guessed from its numbers:
//! the enemy's fight against the player's hero (every ability and active item once, six seconds of shooting),
//! once as it is and once with the item's effect on.
//!
//! So a fire-rate slow counts for what it does to that hero: more against Haze, whose ultimate shoots for her,
//! or Wraith, whose cards come from her hits, than against a hero who mostly casts.

use crate::build::{BuildInput, Target};
use crate::calc;
use crate::combo::{ComboResult, Kind};
use crate::data::{GameData, Item};
use crate::matches::resist_of;
use crate::popular::PopularBuild;
use crate::suggest;

/// A hero's build at some souls: its usual items, bought in the usual order while the souls last.
pub fn build_at(build: &PopularBuild, souls: u32, tiers: &[u8]) -> BuildInput {
    let mut spent = 0;
    let purchases = build
        .core
        .iter()
        .take_while(|c| {
            spent += c.cost;
            spent <= souls
        })
        .map(|c| c.item.clone())
        .collect();
    BuildInput { hero: build.hero.clone(), purchases, souls: Some(souls), ability_tiers: tiers.to_vec(), ..Default::default() }
}

/// The player's hero as a target: health and resists of the build.
pub fn target_of(data: &GameData, me: &BuildInput) -> Target {
    match calc::compute(data, me) {
        Ok(sheet) => {
            let stat = |key: &str| sheet.stat(key).unwrap_or(0.0);
            Target { health: stat("max_health").max(1.0), bullet_resist: stat("bullet_resist"), spirit_resist: stat("spirit_resist") }
        }
        Err(_) => Target::default(),
    }
}

/// Resists stack multiplicatively: 1 − (1 − a)(1 − b).
fn stack(a: f64, b: f64) -> f64 {
    100.0 - (100.0 - a) * (100.0 - b) / 100.0
}

fn prop(item: &Item, key: &str) -> f64 {
    item.props.iter().filter(|p| p.key == key).map(|p| p.value.abs()).fold(0.0, f64::max)
}

/// The fights an enemy is measured in: every ability and active item once, then this many seconds of shooting.
/// One short fight is decided by where the reload falls (a slower gun empties the same magazine before the
/// reload either way), so a few lengths are added together.
const SHOOTING: [f64; 3] = [8.0, 12.0, 16.0];

/// The fights added together: total damage and damage by source.
fn fights(data: &GameData, b: &BuildInput) -> Option<ComboResult> {
    let mut all: Option<ComboResult> = None;
    for seconds in SHOOTING {
        let r = suggest::fight_for(data, b, seconds)?;
        match all.as_mut() {
            None => all = Some(r),
            Some(sum) => {
                sum.total += r.total;
                for s in r.sources {
                    match sum.sources.iter_mut().find(|x| x.source == s.source && x.kind == s.kind) {
                        Some(x) => x.damage += s.damage,
                        None => sum.sources.push(s),
                    }
                }
            }
        }
    }
    all
}

/// An enemy's fight against the player, ready to be measured with items.
pub struct Fight {
    build: BuildInput,
    base: ComboResult,
}

impl Fight {
    pub fn new(data: &GameData, enemy: &BuildInput, me: &Target) -> Option<Fight> {
        let mut build = enemy.clone();
        build.target = me.clone();
        let base = fights(data, &build)?;
        (base.total > 0.0).then_some(Fight { build, base })
    }

    /// The enemy's damage in the fight as it is.
    pub fn damage(&self) -> f64 {
        self.base.total
    }

    /// The share of the enemy's damage the item takes away while its effect is on (0 to 1), or `None` when the
    /// item does nothing to it. Effects on the enemy: fire-rate slow, spirit power taken, spirit or all outgoing
    /// damage cut, disarm. Effects on the player: bullet and spirit resist, bullet immunity, the next big spirit hit
    /// cut. How often the effect is on (a cast, a cooldown) is left to the caller.
    pub fn cut(&self, data: &GameData, item: &Item) -> Option<f64> {
        let mut b = self.build.clone();
        let mut rerun = false;
        let slow = prop(item, "FireRateSlow");
        if slow > 0.0 {
            b.fire_rate_slow = slow;
            rerun = true;
        }
        let sap = prop(item, "TechPowerReduction");
        if sap > 0.0 {
            b.bonus_spirit -= sap;
            rerun = true;
        }
        let (bullet, spirit) = (resist_of(item, false), resist_of(item, true));
        if bullet > 0.0 {
            b.target.bullet_resist = stack(b.target.bullet_resist, bullet);
            rerun = true;
        }
        if spirit > 0.0 {
            b.target.spirit_resist = stack(b.target.spirit_resist, spirit);
            rerun = true;
        }
        let spirit_cut = prop(item, "TechDamageReduction") / 100.0;
        let all_cut = crate::team::enemy_damage_cut(item) / 100.0;
        // Deflected bullets do no damage: a share of them works like bullet resist.
        let deflect = prop(item, "DeflectionPercent");
        if deflect > 0.0 {
            b.target.bullet_resist = stack(b.target.bullet_resist, deflect);
            rerun = true;
        }
        // A silenced enemy casts nothing: its spirit damage stops while the silence lasts.
        let silence = crate::team::silences_enemies(item);
        let big_hit_cut = prop(item, "SpiritDamageReductionProc") / 100.0;
        let text = item.description.to_lowercase();
        let no_bullets = crate::team::disarms_enemies(item) || text.contains("immune to bullets");
        if !rerun && spirit_cut == 0.0 && all_cut == 0.0 && big_hit_cut == 0.0 && !no_bullets && !silence {
            return None;
        }
        let with = if rerun { fights(data, &b)? } else { self.base.clone() };
        let biggest_spirit = with.sources.iter().filter(|s| s.kind == Kind::Spirit).map(|s| s.damage).fold(0.0, f64::max);
        let mut big_hit_done = false;
        let total: f64 = with
            .sources
            .iter()
            .map(|s| {
                let mut d = s.damage;
                if s.kind == Kind::Spirit {
                    d *= 1.0 - spirit_cut;
                    if big_hit_cut > 0.0 && !big_hit_done && s.damage >= biggest_spirit {
                        d *= 1.0 - big_hit_cut;
                        big_hit_done = true;
                    }
                }
                if no_bullets && s.kind == Kind::Bullet {
                    d = 0.0;
                }
                if silence && s.kind == Kind::Spirit {
                    d = 0.0;
                }
                d * (1.0 - all_cut)
            })
            .sum();
        Some(((self.base.total - total) / self.base.total).clamp(0.0, 1.0))
    }
}
