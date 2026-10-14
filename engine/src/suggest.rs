//! Builds the calculator puts together itself. Nothing comes from build lists made elsewhere: every item is
//! picked because the hero's own numbers gain the most from it per soul.
//!
//! A build is put together the way a match goes: first with the souls a player has at the end of the laning
//! phase and only the cheap items affordable then, then with the souls of the mid game, then of the late game,
//! each stage keeping what the one before bought. The souls of each stage come from the matches looked at in the
//! app when there are any. An item is judged by what it does as it is played: an active item is used in the
//! rotation, and an effect that needs a condition (a stack, a low-health target, a hit) counts for half.
//!
//! What "best" means depends on the playstyle: the gun, the abilities, melee, all of it together, or staying
//! alive at the front.

use crate::build::BuildInput;
use crate::calc;
use crate::combo::{self, Action, ComboInput};
use crate::data::{GameData, Item};
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Serialize)]
pub struct SuggestedItem {
    pub item: String,
    pub name: String,
    pub icon: String,
    pub category: String,
    pub cost: u32,
    /// Has to be used to do its main thing.
    pub active: bool,
    /// Part of what it gives depends on a condition; that part is counted for half.
    pub conditional: bool,
    /// The game's own text for the item.
    pub description: String,
    /// The stage it was bought in, as an index into the build's stages.
    pub stage: usize,
    /// How much of the build's gain came with this item, percent of the total gain.
    pub share: f64,
    /// Sold later to make room.
    pub sold: bool,
    /// Went into an upgrade bought later.
    pub upgraded: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct Stage {
    pub name: String,
    /// The souls the stage is put together with, and the cheapest and dearest items bought in it.
    pub souls: u32,
    pub max_cost: u32,
    pub min_cost: u32,
    pub spent: u32,
    /// Items sold in this stage to make room, by name.
    pub sold: Vec<String>,
    /// The build's numbers at the end of the stage, in the order of [`WINDOWS`].
    pub damage: Vec<f64>,
    pub effective_health: f64,
}

#[derive(Debug, Clone, Serialize)]
pub struct Suggested {
    /// The playstyle's key and name.
    pub key: String,
    pub name: String,
    /// What the build was put together for.
    pub goal: String,
    pub stages: Vec<Stage>,
    /// Every item in the order it was bought, sold ones included.
    pub items: Vec<SuggestedItem>,
    /// What the build ends with, in the order it was bought.
    pub purchases: Vec<String>,
    pub spent: u32,
    pub budget: u32,
    /// The damage of every window ([`WINDOWS`]) and the effective health, without items and with the build.
    pub damage_before: Vec<f64>,
    pub damage_after: Vec<f64>,
    /// Health divided by what gets through the resists (the average of bullet and spirit), without anything
    /// an active or a conditional passive adds.
    pub effective_health_before: f64,
    pub effective_health_after: f64,
}

/// The souls players have at the end of each stage of a match, read from matches.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct StageSouls {
    pub lane: u32,
    pub mid: u32,
    pub late: u32,
    /// How many matches and players the numbers come from; 0 when they are the built-in defaults.
    pub matches: u32,
    pub players: u32,
}

#[derive(Debug, Clone, Serialize)]
pub struct Playstyle {
    pub key: String,
    pub name: String,
    pub goal: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct Suggestions {
    /// The damage windows, by name, in the order every damage list uses.
    pub windows: Vec<String>,
    /// What the hero does in each window without items.
    pub bare: Vec<f64>,
    /// Share of the hybrid rotation's damage that comes from abilities rather than the gun, percent, without items.
    pub ability_share: f64,
    pub stage_souls: StageSouls,
    pub playstyles: Vec<Playstyle>,
    pub builds: Vec<Suggested>,
}

/// The ways damage is dealt that a build can be judged by.
#[derive(Clone, Copy, PartialEq)]
enum Window {
    /// Every ability and active item once, then shooting.
    Hybrid,
    /// Active items, then shooting.
    Gun,
    /// Every ability and active item once, and nothing else.
    Spirit,
    /// Every ability and active item once, then a string of heavy and light melee hits.
    Melee,
}

const WINDOWS: [(Window, &str); 4] = [(Window::Hybrid, "Full kit"), (Window::Gun, "Gun"), (Window::Spirit, "Abilities"), (Window::Melee, "Melee")];

fn window_index(w: Window) -> usize {
    WINDOWS.iter().position(|x| x.0 == w).unwrap_or(0)
}

/// Seconds of shooting in the hybrid rotation and in the gun window, and the melee string.
const SHOOT_SECONDS: f64 = 6.0;
const GUN_SECONDS: f64 = 8.0;
const MELEE_STRING: [bool; 6] = [true, false, true, false, true, false];

/// The playstyles: (key, name, goal, the windows it needs, whether effective health counts).
const PLAYSTYLES: [(&str, &str, &str, &[Window], bool); 5] = [
    ("hybrid", "Full kit", "The most damage from every ability and active item once and six seconds of shooting", &[Window::Hybrid], false),
    ("gun", "Gun carry", "The most damage from eight seconds of shooting", &[Window::Gun], false),
    ("spirit", "Spirit burst", "The most damage from every ability and active item once, without the gun", &[Window::Spirit], false),
    ("melee", "Melee brawler", "The most damage from every ability and active item once followed by three heavy and three light melee hits", &[Window::Melee], false),
    (
        "frontline",
        "Frontline",
        "Kit damage and effective health in balance: the product of the two is what is maximized",
        &[Window::Hybrid],
        true,
    ),
];

/// Souls assumed when the build has none set.
const DEFAULT_BUDGET: u32 = 30_000;
/// Souls at the end of each stage when there are no matches to read them from.
const DEFAULT_STAGES: StageSouls = StageSouls { lane: 4_800, mid: 15_000, late: 30_000, matches: 0, players: 0 };
/// The match times the stages end at: the laning phase, 20 and 30 minutes.
pub const LANE_END_S: u32 = 9 * 60;
pub const MID_END_S: u32 = 20 * 60;
pub const LATE_END_S: u32 = 30 * 60;
/// Each stage: the dearest item worth buying in it, and the cheapest one still worth a slot this late. Rushing
/// an expensive item in the lane leaves a player behind in souls; late in a match a cheap item wastes a slot.
const STAGE_ITEMS: [(&str, u32, u32); 4] = [("Lane", 1_600, 0), ("Mid game", 3_200, 0), ("Late game", 6_400, 1_600), ("Full build", 6_400, 3_200)];
/// While slots are free, an item has to cost at least this share of the souls left per open slot, so that the
/// slots do not fill up with cheap items while most of the souls stay unspent. Not in the lane.
const SLOT_FLOOR: f64 = 0.5;
/// A pick has to improve the goal by at least this much, percent.
const WORTH_PCT: f64 = 0.1;
/// Selling returns half of what an item cost.
const SELL_BACK: f64 = 0.5;
/// When the slots are full, these many of the cheapest items are tried for selling, at most this many times.
const SELL_CANDIDATES: usize = 3;
const MAX_SALES: usize = 6;

/// The median souls at the end of each stage over the players of the given matches (raw match metadata).
/// A stage only counts matches that lasted that long. `None` without a single usable match.
pub fn stage_souls(match_jsons: &[String]) -> Option<StageSouls> {
    let mut at: [Vec<u32>; 3] = [vec![], vec![], vec![]];
    let mut matches = 0;
    let mut players = 0;
    for json in match_jsons {
        let Ok(root) = serde_json::from_str::<Value>(json) else { continue };
        let info = root.get("match_info").unwrap_or(&root);
        let Some(list) = info.get("players").and_then(Value::as_array) else { continue };
        let duration = info.get("duration_s").and_then(Value::as_u64).unwrap_or(0) as u32;
        matches += 1;
        for p in list {
            let Some(stats) = p.get("stats").and_then(Value::as_array) else { continue };
            players += 1;
            for (i, end) in [LANE_END_S, MID_END_S, LATE_END_S].into_iter().enumerate() {
                if duration < end {
                    continue;
                }
                let souls = stats
                    .iter()
                    .filter(|s| s.get("time_stamp_s").and_then(Value::as_u64).is_some_and(|t| t as u32 <= end))
                    .last()
                    .and_then(|s| s.get("net_worth").and_then(Value::as_u64));
                if let Some(souls) = souls {
                    at[i].push(souls as u32);
                }
            }
        }
    }
    let median = |v: &mut Vec<u32>, fallback: u32| {
        v.sort();
        if v.is_empty() { fallback } else { v[v.len() / 2] }
    };
    if matches == 0 {
        return None;
    }
    Some(StageSouls {
        lane: median(&mut at[0], DEFAULT_STAGES.lane),
        mid: median(&mut at[1], DEFAULT_STAGES.mid),
        late: median(&mut at[2], DEFAULT_STAGES.late),
        matches,
        players,
    })
}

fn actives(data: &GameData, b: &BuildInput) -> Vec<Action> {
    b.purchases
        .iter()
        .filter(|class| data.item(class).is_some_and(|i| i.active))
        .map(|class| Action::Item { item: class.clone() })
        .collect()
}

fn window_input(data: &GameData, b: &BuildInput, abilities: usize, window: Window) -> ComboInput {
    let mut actions: Vec<Action> = vec![];
    if window != Window::Gun {
        actions.extend((0..abilities).map(|index| Action::Ability { index, duration: None, hits: None }));
    }
    actions.extend(actives(data, b));
    match window {
        Window::Hybrid => actions.push(Action::Shoot { seconds: SHOOT_SECONDS }),
        Window::Gun => actions.push(Action::Shoot { seconds: GUN_SECONDS }),
        Window::Spirit => {}
        Window::Melee => actions.extend(MELEE_STRING.iter().map(|heavy| Action::Melee { heavy: *heavy })),
    }
    ComboInput { actions, ..Default::default() }
}

#[derive(Clone)]
struct Measured {
    damage: Vec<f64>,
    effective_health: f64,
    spent: u32,
    slots_used: usize,
    slot_limit: usize,
    fits: bool,
}

/// The numbers of a build with its conditional effects switched on or off, for the windows asked for.
fn measure_with(data: &GameData, b: &BuildInput, conditions: bool, windows: &[Window]) -> Option<Measured> {
    let mut b = b.clone();
    b.active = if conditions { b.purchases.clone() } else { vec![] };
    let sheet = calc::compute(data, &b).ok()?;
    let stat = |key: &str| sheet.stat(key).unwrap_or(0.0);
    let through = (1.0 - (stat("bullet_resist") + stat("spirit_resist")) / 200.0).max(0.05);
    let damage = WINDOWS
        .iter()
        .map(|(w, _)| {
            if windows.contains(w) {
                combo::simulate(data, &b, &window_input(data, &b, sheet.abilities.len(), *w)).map(|r| r.total).unwrap_or(0.0)
            } else {
                0.0
            }
        })
        .collect();
    Some(Measured {
        damage,
        effective_health: stat("max_health") / through,
        spent: sheet.spent,
        slots_used: sheet.slots_used,
        slot_limit: sheet.slot_limit,
        fits: sheet.errors.is_empty(),
    })
}

/// A build's numbers as it is played: conditional effects counted for half. Effective health never counts
/// them, or a single big active would look like permanent armor.
fn measure(data: &GameData, b: &BuildInput, windows: &[Window]) -> Option<Measured> {
    let conditional = b.purchases.iter().any(|class| data.item(class).is_some_and(Item::has_conditional));
    let off = measure_with(data, b, false, windows)?;
    if !conditional || windows.is_empty() {
        return Some(off);
    }
    let on = measure_with(data, b, true, windows)?;
    let damage = off.damage.iter().zip(&on.damage).map(|(a, b)| (a + b) / 2.0).collect();
    Some(Measured { damage, ..off })
}

/// What a playstyle maximizes.
fn value(m: &Measured, windows: &[Window], durable: bool) -> f64 {
    let damage: f64 = windows.iter().map(|w| m.damage[window_index(*w)]).sum();
    if durable { (damage.max(1.0) * m.effective_health.max(1.0)).sqrt() } else { damage }
}

/// One build, stage by stage.
fn put_together(data: &GameData, start: &BuildInput, budget: u32, stages_souls: &StageSouls, style: &(&str, &str, &str, &[Window], bool), bare: &Measured) -> Option<Suggested> {
    let (key, name, goal, windows, durable) = *style;
    let mut current = start.clone();
    // (item, gain, stage, sold)
    let mut picked: Vec<(String, f64, usize, bool)> = vec![];
    let mut stages: Vec<Stage> = vec![];
    // What has really been paid: selling gives only part of an item's price back.
    let mut paid_total: u32 = 0;
    let ends = [stages_souls.lane, stages_souls.mid, stages_souls.late, u32::MAX];
    let stage_list: Vec<(&str, u32, u32, u32)> = STAGE_ITEMS
        .iter()
        .zip(ends)
        .map(|((name, max_cost, min_cost), end)| (*name, end, *max_cost, *min_cost))
        .filter(|s| s.1 < budget)
        .chain(std::iter::once(("Full build", budget, STAGE_ITEMS[3].1, STAGE_ITEMS[3].2)))
        .collect();
    let all: Vec<Window> = WINDOWS.iter().map(|w| w.0).collect();
    for (stage, (stage_name, souls, max_cost, min_cost)) in stage_list.iter().copied().enumerate() {
        // The level, the boons and the spirit power that come with the souls of the stage.
        current.souls = Some(souls);
        let mut sold: Vec<String> = vec![];
        let mut sales = 0;
        loop {
            let Some(now) = measure(data, &current, windows) else { break };
            if paid_total >= souls {
                break;
            }
            let left = souls - paid_total;
            let before = value(&now, windows, durable);
            let full = now.slots_used >= now.slot_limit;
            let floor = if stage == 0 || full { 0 } else { (left as f64 / (now.slot_limit - now.slots_used) as f64 * SLOT_FLOOR) as u32 };
            // Nothing already bought, and nothing sold earlier comes back.
            let candidates = data
                .items
                .iter()
                .filter(|i| i.cost > 0 && i.tier <= 4 && i.cost <= max_cost && i.cost >= min_cost)
                .filter(|i| !current.purchases.contains(&i.class_name) && !picked.iter().any(|p| p.0 == i.class_name));
            // (score, gain, item, paid, item sold for it, worth its slot)
            let mut best: Option<(f64, f64, String, u32, Option<String>, bool)> = None;
            let mut consider = |score: f64, gain: f64, class: &str, paid: u32, sell: Option<String>, dear: bool| {
                if best.as_ref().is_none_or(|b| (dear, score) > (b.5, b.0)) {
                    best = Some((score, gain, class.to_string(), paid, sell, dear));
                }
            };
            if !full {
                for item in candidates {
                    let mut trial = current.clone();
                    trial.purchases.push(item.class_name.clone());
                    let Some(then) = measure(data, &trial, windows) else { continue };
                    let paid = then.spent.saturating_sub(now.spent);
                    if !then.fits || paid == 0 || paid > left || then.slots_used > then.slot_limit {
                        continue;
                    }
                    let gain = value(&then, windows, durable) - before;
                    if gain > before * WORTH_PCT / 100.0 {
                        consider(gain / paid as f64, gain, &item.class_name, paid, None, item.cost >= floor);
                    }
                }
            } else if sales < MAX_SALES {
                // Slots full: sell one of the cheapest items to make room for something better.
                let owned = calc::compute(data, &current).ok()?.owned;
                let mut cheap: Vec<&Item> = owned.iter().filter_map(|class| data.item(class)).collect();
                cheap.sort_by_key(|i| i.cost);
                let candidates: Vec<&Item> = candidates.filter(|i| i.cost > cheap.first().map(|c| c.cost).unwrap_or(0)).collect();
                for sell in cheap.iter().take(SELL_CANDIDATES) {
                    let mut without = current.clone();
                    without.purchases.retain(|c| c != &sell.class_name);
                    let Some(rest) = measure_with(data, &without, false, &[]) else { continue };
                    let refund = (sell.cost as f64 * SELL_BACK) as u32;
                    for item in &candidates {
                        let mut trial = without.clone();
                        trial.purchases.push(item.class_name.clone());
                        let Some(then) = measure(data, &trial, windows) else { continue };
                        let paid = then.spent.saturating_sub(rest.spent);
                        if !then.fits || paid == 0 || paid.saturating_sub(refund) > left || then.slots_used > then.slot_limit {
                            continue;
                        }
                        let gain = value(&then, windows, durable) - before;
                        if gain > before * WORTH_PCT / 100.0 {
                            let net = paid.saturating_sub(refund).max(1);
                            consider(gain / net as f64, gain, &item.class_name, net, Some(sell.class_name.clone()), true);
                        }
                    }
                }
            }
            let Some((_, gain, class, paid, sell, _)) = best else { break };
            if let Some(sell) = sell {
                current.purchases.retain(|c| c != &sell);
                if let Some(p) = picked.iter_mut().find(|p| p.0 == sell) {
                    p.3 = true;
                }
                sold.push(data.item(&sell).map(|i| i.name.clone()).unwrap_or(sell));
                sales += 1;
            }
            current.purchases.push(class.clone());
            paid_total += paid;
            picked.push((class, gain, stage, false));
        }
        let end = measure(data, &current, &all)?;
        stages.push(Stage {
            name: stage_name.to_string(),
            souls,
            max_cost,
            min_cost,
            spent: paid_total,
            sold,
            damage: end.damage,
            effective_health: end.effective_health,
        });
    }

    current.souls = Some(budget);
    let sheet = calc::compute(data, &current).ok()?;
    let total_gain: f64 = picked.iter().map(|p| p.1.max(0.0)).sum::<f64>().max(1e-9);
    let items: Vec<SuggestedItem> = picked
        .iter()
        .filter_map(|(class, gain, stage, sold)| {
            let item = data.item(class)?;
            Some(SuggestedItem {
                item: class.clone(),
                name: item.name.clone(),
                icon: item.icon.clone(),
                category: item.category.clone(),
                cost: item.cost,
                active: item.active,
                conditional: item.has_conditional(),
                description: item.description.clone(),
                stage: *stage,
                share: gain.max(0.0) / total_gain * 100.0,
                sold: *sold,
                upgraded: !*sold && !sheet.owned.contains(class),
            })
        })
        .collect();
    let purchases: Vec<String> = picked.iter().filter(|p| sheet.owned.contains(&p.0)).map(|p| p.0.clone()).collect();
    let after = stages.last()?;
    Some(Suggested {
        key: key.into(),
        name: name.into(),
        goal: goal.into(),
        items,
        purchases,
        spent: paid_total,
        budget,
        damage_before: bare.damage.clone(),
        damage_after: after.damage.clone(),
        effective_health_before: bare.effective_health,
        effective_health_after: after.effective_health,
        stages,
    })
}

/// The playstyles builds can be put together for.
pub fn playstyles() -> Vec<Playstyle> {
    PLAYSTYLES.iter().map(|p| Playstyle { key: p.0.into(), name: p.1.into(), goal: p.2.into() }).collect()
}

/// Puts together a build for each playstyle asked for (all of them when the list is empty), at the build's souls,
/// with the stages ending at the given souls (or the built-in defaults).
pub fn suggest(data: &GameData, b: &BuildInput, styles: &[String], stage_souls: Option<StageSouls>) -> Result<Suggestions, String> {
    let mut start = b.clone();
    start.purchases.clear();
    start.active.clear();
    start.imbues.clear();
    start.upto = None;
    let budget = b.souls.filter(|s| *s > 0).unwrap_or(DEFAULT_BUDGET);
    start.souls = Some(budget);
    let all: Vec<Window> = WINDOWS.iter().map(|w| w.0).collect();
    let bare = measure(data, &start, &all).ok_or("The hero could not be computed")?;
    let stages = stage_souls.unwrap_or(DEFAULT_STAGES);

    let chosen: Vec<&(&str, &str, &str, &[Window], bool)> = PLAYSTYLES.iter().filter(|p| styles.is_empty() || styles.iter().any(|s| s == p.0)).collect();
    // The builds do not depend on each other, so they are put together side by side.
    let builds: Vec<Suggested> = std::thread::scope(|scope| {
        let handles: Vec<_> = chosen
            .iter()
            .map(|style| {
                let (start, bare, stages) = (&start, &bare, &stages);
                scope.spawn(move || put_together(data, start, budget, stages, style, bare))
            })
            .collect();
        handles.into_iter().filter_map(|h| h.join().ok().flatten()).collect()
    });
    if builds.len() != chosen.len() {
        return Err("A build could not be computed".into());
    }

    // How much of the hybrid rotation is abilities: what is left once the shooting in it is taken out.
    let (hybrid, gun) = (bare.damage[window_index(Window::Hybrid)], bare.damage[window_index(Window::Gun)]);
    let shooting = gun * SHOOT_SECONDS / GUN_SECONDS;
    Ok(Suggestions {
        windows: WINDOWS.iter().map(|w| w.1.to_string()).collect(),
        bare: bare.damage.clone(),
        ability_share: if hybrid > 0.0 { ((hybrid - shooting) / hybrid * 100.0).clamp(0.0, 100.0) } else { 0.0 },
        stage_souls: stages,
        playstyles: playstyles(),
        builds,
    })
}
