//! The order to spend ability points in: each step the upgrade that adds the most to a sustained fight per point
//! it costs, with the level it can be bought at; and the orders top players of the hero go with.

use serde::Serialize;
use serde_json::Value;

use crate::area;
use crate::build::BuildInput;
use crate::calc;
use crate::data::GameData;
use crate::suggest;

/// Ability points each upgrade tier costs.
const AP_COST: [u32; 3] = [1, 2, 5];
/// The fight the order is measured over, seconds: long enough that an ability is cast again as its cooldown allows,
/// so a cooldown upgrade counts for the casts it adds.
const SUSTAINED_S: f64 = 30.0;
/// The shooting in the measured fight, seconds.
const SHOOTING_S: f64 = 8.0;
/// A step that adds less than this share to the fight is said to change nothing in it.
const NO_CHANGE: f64 = 0.001;
/// Orders of top players listed, and the matches an order needs to be listed.
const POPULAR_ORDERS: usize = 3;
const MIN_ORDER_MATCHES: u64 = 30;

#[derive(Debug, Clone, Serialize)]
pub struct OrderStep {
    /// The ability's slot (0 to 3), its name and icon, and the tier the step reaches (1 to 3).
    pub index: usize,
    pub ability: String,
    pub icon: String,
    pub tier: u8,
    /// Points the step costs and all points spent with it.
    pub ap: u32,
    pub total_ap: u32,
    /// The level and souls at which that many points are earned.
    pub level: u32,
    pub souls: u32,
    /// What the step adds to the sustained fight, percent.
    pub gain_pct: f64,
    /// What the upgrade does, in words.
    pub upgrade: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct PopularOrder {
    /// The upgrades in order: (ability slot, tier reached). Unlocks are left out.
    pub steps: Vec<(usize, u8)>,
    pub matches: u64,
    pub win_pct: f64,
}

#[derive(Debug, Clone, Serialize)]
pub struct AbilityOrder {
    pub steps: Vec<OrderStep>,
    /// How the order was measured.
    pub note: String,
}

/// Damage of a sustained fight: every ability as often as its cooldown allows in it, then the rest as it is.
fn sustained(data: &GameData, b: &BuildInput) -> Option<f64> {
    let sheet = calc::compute(data, b).ok()?;
    let fight = suggest::fight_for(data, b, SHOOTING_S)?;
    let mut total = 0.0;
    // The sustained fight's lockdowns: every cast the cooldown allows.
    let casts: Vec<(&calc::AbilitySheet, f64)> = sheet
        .abilities
        .iter()
        .map(|a| {
            let cooldown = a.props.iter().find(|p| p.key == "AbilityCooldown").map(|p| p.value).unwrap_or(0.0);
            (a, if cooldown > 0.0 { (SUSTAINED_S / cooldown).floor().max(1.0) } else { 1.0 })
        })
        .collect();
    let control = area::control_share(&casts, SUSTAINED_S);
    for s in &fight.sources {
        let ability = sheet.abilities.iter().find(|a| s.source == a.name || s.source.starts_with(&format!("{} (", a.name)));
        let casts = ability
            .and_then(|a| a.props.iter().find(|p| p.key == "AbilityCooldown").map(|p| p.value))
            .filter(|cd| *cd > 0.0)
            .map(|cd| (SUSTAINED_S / cd).max(1.0))
            .unwrap_or(1.0);
        // On every enemy it hits in a team fight: an upgrade that widens an area counts for the enemies it adds.
        total += s.damage * casts * ability.map(area::targets).unwrap_or(1.0);
    }
    Some(area::with_control(total, control))
}

/// The best order for the build's items at its souls: each step the upgrade that adds the most to the sustained
/// fight for the points it costs. Upgrades that change no damage (a slow, a range) come when nothing else is left,
/// cheapest first.
pub fn ability_order(data: &GameData, b: &BuildInput) -> Result<AbilityOrder, String> {
    let hero = data.hero(&b.hero).ok_or("Unknown hero")?;
    let slots = hero.abilities.len().min(4);
    let mut tiers = vec![0u8; slots];
    let mut trial = b.clone();
    trial.souls = Some(b.souls.unwrap_or(30_000).max(1));
    trial.ability_tiers = tiers.clone();
    let mut now = sustained(data, &trial).ok_or("The build could not be measured")?.max(1.0);
    let sheet = calc::compute(data, &trial).map_err(|e| e.to_string())?;
    let mut steps = vec![];
    let mut spent = 0;
    // The level each point total is reached at, and the level the fourth ability (the ultimate) is unlocked at:
    // it cannot be upgraded before.
    let level_for = |points: u32| {
        let mut earned = 0;
        hero.levels.iter().find(|l| {
            earned += l.ability_points;
            earned >= points
        })
    };
    let mut unlocked = 0;
    let ult_level = hero.levels.iter().find(|l| {
        unlocked += l.ability_unlocks;
        unlocked >= 4
    });
    loop {
        let ult_ready = |i: usize| {
            i != 3 || ult_level.is_none_or(|u| level_for(spent + AP_COST[tiers[i] as usize]).is_none_or(|l| l.level >= u.level))
        };
        let mut open: Vec<usize> = (0..slots).filter(|i| (tiers[*i] as usize) < hero.abilities[*i].upgrades.len().min(AP_COST.len())).collect();
        if open.iter().any(|i| ult_ready(*i)) {
            open.retain(|i| ult_ready(*i));
        }
        let options: Vec<(usize, f64, f64)> = open
            .into_iter()
            .filter_map(|i| {
                let mut t = trial.clone();
                t.ability_tiers = tiers.clone();
                t.ability_tiers[i] += 1;
                let then = sustained(data, &t)?;
                let gain = (then - now) / now;
                let ap = AP_COST[tiers[i] as usize] as f64;
                // A tier that adds nothing can open one that adds a lot (Storm Cloud's second): the worth of a step
                // is the better of it alone and it with the next tier, per point.
                let next = (tiers[i] as usize + 1) < hero.abilities[i].upgrades.len().min(AP_COST.len());
                let two = if next {
                    t.ability_tiers[i] += 1;
                    sustained(data, &t).map(|v| (v - now) / now / (ap + AP_COST[tiers[i] as usize + 1] as f64)).unwrap_or(0.0)
                } else {
                    0.0
                };
                Some((i, gain, (gain / ap).max(two)))
            })
            .collect();
        let Some(&(i, gain, _)) = options.iter().max_by(|a, b| {
            // What adds damage first, by damage per point; then the cheapest upgrade.
            let worth = |x: &(usize, f64, f64)| if x.2 > NO_CHANGE { x.2 } else { -(AP_COST[tiers[x.0] as usize] as f64) - 10.0 };
            worth(a).total_cmp(&worth(b))
        }) else {
            break;
        };
        let ap = AP_COST[tiers[i] as usize];
        tiers[i] += 1;
        spent += ap;
        now *= 1.0 + gain.max(0.0);
        let reached = level_for(spent);
        steps.push(OrderStep {
            index: i,
            ability: hero.abilities[i].name.clone(),
            icon: hero.abilities[i].icon.clone(),
            tier: tiers[i],
            ap,
            total_ap: spent,
            level: reached.map(|l| l.level).unwrap_or(0),
            souls: reached.map(|l| l.required_souls).unwrap_or(0),
            gain_pct: gain.max(0.0) * 100.0,
            upgrade: sheet.abilities.get(i).and_then(|a| a.upgrade_notes.get(tiers[i] as usize - 1)).cloned().unwrap_or_default(),
        });
    }
    Ok(AbilityOrder {
        steps,
        note: format!(
            "Measured on a {SUSTAINED_S:.0}-second fight with this build: every ability as often as its cooldown allows and {SHOOTING_S:.0} seconds of shooting. Upgrades that change no damage (stuns, slows, range) come last, cheapest first: weigh them by hand."
        ),
    })
}

/// The ability orders top players of the hero go with, from the analytics service's ability-order-stats: the most
/// played ones, each with its matches and win rate.
pub fn popular_orders(data: &GameData, hero_id: u64, json: &str) -> Result<Vec<PopularOrder>, String> {
    let hero = data.heroes.iter().find(|h| h.id == hero_id).ok_or("Unknown hero")?;
    let rows: Vec<Value> = serde_json::from_str(json).map_err(|e| format!("The statistics service answered in an unexpected form: {e}"))?;
    let mut out: Vec<PopularOrder> = rows
        .iter()
        .filter_map(|r| {
            let matches = r.get("matches").and_then(Value::as_u64)?;
            let wins = r.get("wins").and_then(Value::as_u64).unwrap_or(0);
            let mut seen = vec![0u8; hero.abilities.len()];
            let steps = r
                .get("abilities")?
                .as_array()?
                .iter()
                .filter_map(|id| {
                    let i = hero.abilities.iter().position(|a| Some(a.id) == id.as_u64())?;
                    // The first time an ability shows up it is unlocked; each time after, upgraded.
                    let tier = seen[i];
                    seen[i] += 1;
                    (tier > 0 && i < 4).then_some((i, tier.min(3)))
                })
                .collect();
            (matches >= MIN_ORDER_MATCHES).then(|| PopularOrder { steps, matches, win_pct: wins as f64 / matches as f64 * 100.0 })
        })
        .collect();
    out.sort_by(|a, b| b.matches.cmp(&a.matches));
    out.truncate(POPULAR_ORDERS);
    Ok(out)
}
