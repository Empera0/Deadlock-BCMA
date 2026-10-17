//! Damage against non-player units (troopers and haunts) and how fast a build farms the jungle.
//!
//! Haunt values from deadlock.wiki (Haunt, City Never Sleeps update, 29 September 2026): each camp tier has a
//! baseline health, soul bounty, bullet and spirit resist; health grows 2.1% and the bounty 1.08% of its base
//! per minute of the match.

use crate::build::BuildInput;
use crate::calc::{self, Sheet};
use crate::data::GameData;
use crate::kit;
use serde::Serialize;

/// The ultimate's slot among the hero's abilities: kept for fights, not spent on units.
const ULTIMATE_SLOT: usize = 3;

/// What a build does to non-player units.
pub struct NpcDamage {
    /// Sustained gun damage per second against units, before their resists.
    pub gun_dps: f64,
    /// Bullet resist against units' attacks (Monster Rounds, Cultist Sacrifice).
    pub npc_bullet_resist: f64,
    /// The three basic abilities: (damage per cast before resists, hits an area, cooldown).
    pub abilities: Vec<(f64, bool, f64)>,
}

/// The build's damage against non-player units: weapon damage that only works on them is added to the gun.
pub fn npc_damage(data: &GameData, b: &BuildInput, sheet: &Sheet) -> NpcDamage {
    let stat = |key: &str| sheet.stat(key).unwrap_or(0.0);
    let owned = || sheet.owned.iter().filter_map(|c| data.item(c)).flat_map(|i| i.props.iter());
    let npc_power: f64 = owned().filter(|p| p.key == "NonPlayerBonusWeaponPower").map(|p| p.value).sum();
    let npc_resist: f64 = owned().filter(|p| p.key == "NonPlayerBulletResist").map(|p| p.value).fold(0.0, f64::max);
    let weapon = stat("weapon_damage_pct");
    let gun_dps = stat("dps_sustained") * (1.0 + (weapon + npc_power) / 100.0) / (1.0 + weapon / 100.0).max(0.01);
    let pellets = data.hero(&b.hero).map(|h| h.weapon.pellets).unwrap_or(1.0);
    let ctx = kit::Context { per_shot: stat("bullet_damage") * pellets, shots_per_second: stat("shots_per_second"), abilities: &sheet.abilities, flags: &[] };
    let abilities = sheet
        .abilities
        .iter()
        .take(ULTIMATE_SLOT)
        .filter_map(|a| {
            let cooldown = a.props.iter().find(|p| p.key == "AbilityCooldown").map(|p| p.value).unwrap_or(0.0);
            let plan = kit::plan(a, &ctx, None, None);
            if !plan.castable || cooldown <= 0.0 {
                return None;
            }
            let damage: f64 = plan
                .parts
                .iter()
                .map(|part| match part {
                    kit::Part::Instant { raw, .. } => *raw,
                    kit::Part::Dot { dps, seconds, .. } => dps * seconds,
                    kit::Part::Hits { raw, count, .. } => raw * *count as f64,
                })
                .sum();
            let area = a.props.iter().any(|p| (p.key.contains("Radius") || p.key.contains("Width")) && p.value > 0.0);
            (damage > 0.0).then_some((damage, area, cooldown))
        })
        .collect();
    NpcDamage { gun_dps, npc_bullet_resist: npc_resist, abilities }
}

/// A camp tier: (name, haunts of each tier in the camp, respawn seconds).
struct Camp {
    name: &'static str,
    /// Haunts in the camp as (tier index, count).
    haunts: &'static [(usize, f64)],
    respawn: f64,
    spawns_at: f64,
}

/// Haunt tiers: (health, souls, soul gain per minute, bullet resist, spirit resist).
const TIERS: [(f64, f64, f64, f64, f64); 3] = [(129.0, 41.0, 0.44, 50.0, 45.0), (322.0, 68.0, 0.73, 50.0, 45.0), (1133.0, 181.0, 1.95, 60.0, 55.0)];
const HEALTH_GROWTH_PER_MINUTE: f64 = 0.021;
/// Small camps hold three small haunts; medium camps mix small and medium (taken as two medium and a small);
/// large camps hold a large haunt with two medium or two large (taken as two medium).
const CAMPS: [Camp; 3] = [
    Camp { name: "Small camp", haunts: &[(0, 3.0)], respawn: 85.0, spawns_at: 2.0 },
    Camp { name: "Medium camp", haunts: &[(1, 2.0), (0, 1.0)], respawn: 290.0, spawns_at: 5.0 },
    Camp { name: "Large camp", haunts: &[(2, 1.0), (1, 2.0)], respawn: 335.0, spawns_at: 8.0 },
];
/// Seconds walking from one camp to the next (an assumption).
pub const TRAVEL_SECONDS: f64 = 15.0;

#[derive(Debug, Clone, Serialize)]
pub struct CampFarm {
    pub name: String,
    pub health: f64,
    pub souls: f64,
    /// Seconds to clear the camp, and the souls per minute of clearing such camps one after another.
    pub clear_seconds: f64,
    pub souls_per_minute: f64,
    pub available: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct FarmReport {
    pub minute: f64,
    pub gun_dps: f64,
    pub camps: Vec<CampFarm>,
    /// Extra souls per minute from Cultist Sacrifice consuming a large haunt each time it is ready.
    pub cultist_per_minute: f64,
    pub notes: Vec<String>,
}

/// How fast the build clears each camp tier at a minute of the match: its abilities once on the whole camp
/// (area abilities hit every haunt), then the gun.
pub fn farm(data: &GameData, b: &BuildInput, minute: f64) -> Result<FarmReport, String> {
    let sheet = calc::compute(data, b)?;
    let npc = npc_damage(data, b, &sheet);
    let growth = 1.0 + HEALTH_GROWTH_PER_MINUTE * minute.max(0.0);
    let camps = CAMPS
        .iter()
        .map(|c| {
            let count: f64 = c.haunts.iter().map(|(_, n)| n).sum();
            let (mut health, mut souls) = (0.0, 0.0);
            for (tier, n) in c.haunts {
                let (hp, base, gain, _, _) = TIERS[*tier];
                health += hp * growth * n;
                souls += (base + gain * minute) * n;
            }
            // Abilities first: each counts against the camp's spirit resist, area ones on every haunt.
            let mut ability_damage = 0.0;
            for (damage, area, _) in &npc.abilities {
                let hits = if *area { count } else { 1.0 };
                let resist = c.haunts.iter().map(|(t, n)| TIERS[*t].4 * n).sum::<f64>() / count;
                ability_damage += damage * hits * (1.0 - resist / 100.0);
            }
            // The gun finishes what is left, against the camp's bullet resist.
            let left = (health - ability_damage).max(0.0);
            let bullet_resist = c.haunts.iter().map(|(t, n)| TIERS[*t].3 * n).sum::<f64>() / count;
            let gun = npc.gun_dps * (1.0 - bullet_resist / 100.0);
            let clear = if left <= 0.0 { 0.0 } else if gun > 0.0 { left / gun } else { f64::INFINITY };
            CampFarm {
                name: c.name.into(),
                health,
                souls,
                clear_seconds: clear,
                souls_per_minute: if clear.is_finite() { souls * 60.0 / (clear + TRAVEL_SECONDS) } else { 0.0 },
                available: minute >= c.spawns_at,
            }
        })
        .collect();
    let cultist = sheet.owned.iter().filter_map(|c| data.item(c)).find(|i| i.name == "Cultist Sacrifice").map(|i| {
        let v = |key: &str| i.props.iter().find(|p| p.key == key).map(|p| p.value).unwrap_or(0.0);
        let large = TIERS[2].1 + TIERS[2].2 * minute;
        large * v("BonusSoulsPct") / 100.0 * 60.0 / v("AbilityCooldown").max(1.0)
    });
    let mut notes = vec![format!(
        "Assumes {}s of walking between camps, medium camps of two medium haunts and a small one, and large camps of a large haunt with two medium ones.",
        TRAVEL_SECONDS
    )];
    notes.push(format!("Respawn times: small {}s, medium {}s, large {}s, so one camp alone cannot be farmed back to back.", CAMPS[0].respawn, CAMPS[1].respawn, CAMPS[2].respawn));
    Ok(FarmReport { minute, gun_dps: npc.gun_dps, camps, cultist_per_minute: cultist.unwrap_or(0.0), notes })
}
