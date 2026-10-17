//! What to do with the next souls when the build is already put together: buy, upgrade an owned item, or sell
//! one item for a better one. Every move is measured with the playstyle's fight, as the build suggestions are.

use serde::Serialize;

use crate::build::BuildInput;
use crate::calc;
use crate::data::GameData;
use crate::suggest;

/// Selling returns half of what an item cost.
const SELL_BACK: f64 = 0.5;
/// Items that only come from the Unstable Rift cost this; they are not in the shop.
const NOT_IN_SHOP: u32 = 9_999;
/// A move has to improve the playstyle's value by at least this share to be listed.
const WORTH: f64 = 0.005;
/// Moves listed of each kind.
const LISTED: usize = 5;

#[derive(Debug, Clone, Serialize)]
pub struct Move {
    /// `buy` | `upgrade` | `swap`.
    pub kind: String,
    /// The item sold first, for a swap.
    pub sell: Option<String>,
    pub sell_name: Option<String>,
    pub sell_icon: Option<String>,
    pub refund: u32,
    /// Purchases to take off the build for the sale: the item and the components bought into it.
    pub removes: Vec<String>,
    pub buy: String,
    pub buy_name: String,
    pub buy_icon: String,
    /// Souls the purchase takes: less than its price when it upgrades an owned item.
    pub paid: u32,
    /// Souls it takes on top of the refund.
    pub net_cost: u32,
    /// How much the playstyle's value grows over the build as it is (0.05 = 5% more).
    pub gain: f64,
    /// The ability an imbued item goes into.
    pub imbue: Option<usize>,
}

/// What an owned item does for the build: the share of the playstyle's value lost without it.
#[derive(Debug, Clone, Serialize)]
pub struct Worth {
    pub item: String,
    pub name: String,
    pub icon: String,
    pub cost: u32,
    pub loss: f64,
}

#[derive(Debug, Clone, Serialize)]
pub struct Advice {
    pub style: String,
    pub free_souls: u32,
    pub slots_used: usize,
    pub slot_limit: usize,
    pub buys: Vec<Move>,
    pub upgrades: Vec<Move>,
    pub swaps: Vec<Move>,
    /// Owned items, the least useful first.
    pub worth: Vec<Worth>,
}

/// An item's components, theirs too.
fn parts(data: &GameData, class: &str) -> Vec<String> {
    let mut out = vec![];
    let mut todo: Vec<String> = data.item(class).map(|i| i.components.clone()).unwrap_or_default();
    while let Some(c) = todo.pop() {
        if !out.contains(&c) {
            todo.extend(data.item(&c).map(|i| i.components.clone()).unwrap_or_default());
            out.push(c);
        }
    }
    out
}

/// The build with an item sold: the item and the components bought into it are gone.
fn without(data: &GameData, b: &BuildInput, class: &str) -> BuildInput {
    let gone = parts(data, class);
    let mut out = b.clone();
    out.purchases.retain(|c| c != class && !gone.contains(c));
    out.active.retain(|c| c != class);
    out.imbues.remove(class);
    out
}

fn a_move(data: &GameData, kind: &str, sold: Option<(&str, u32)>, g: &suggest::Gain, gain: f64) -> Option<Move> {
    let item = data.item(&g.item)?;
    let sell = sold.and_then(|(class, _)| data.item(class));
    let refund = sold.map(|s| s.1).unwrap_or(0);
    Some(Move {
        kind: kind.into(),
        sell: sell.map(|i| i.class_name.clone()),
        sell_name: sell.map(|i| i.name.clone()),
        sell_icon: sell.map(|i| i.icon.clone()),
        refund,
        removes: sell.map(|i| [vec![i.class_name.clone()], parts(data, &i.class_name)].concat()).unwrap_or_default(),
        buy: item.class_name.clone(),
        buy_name: item.name.clone(),
        buy_icon: item.icon.clone(),
        paid: g.paid,
        net_cost: g.paid.saturating_sub(refund),
        gain,
        imbue: g.imbue,
    })
}

/// The moves for a build, measured for a playstyle (`hybrid`, `gun`, `spirit`, `melee`, `frontline`).
pub fn advise(data: &GameData, b: &BuildInput, style: &str) -> Result<Advice, String> {
    let mut b = b.clone();
    b.purchases.truncate(b.upto.unwrap_or(b.purchases.len()).min(b.purchases.len()));
    b.upto = None;
    let sheet = calc::compute(data, &b).map_err(|e| e.to_string())?;
    let souls = b.souls.unwrap_or(0).max(sheet.spent);
    let free = souls - sheet.spent;
    let owned = sheet.owned.clone();
    let inside: Vec<String> = owned.iter().flat_map(|c| parts(data, c)).collect();
    let candidates: Vec<String> = data
        .items
        .iter()
        .filter(|i| i.cost < NOT_IN_SHOP && !owned.contains(&i.class_name) && !inside.contains(&i.class_name) && !b.purchases.contains(&i.class_name))
        .map(|i| i.class_name.clone())
        .collect();
    let upgrades_owned = |class: &str| parts(data, class).iter().any(|p| owned.contains(p));

    let mut buys = vec![];
    let mut upgrades = vec![];
    for g in suggest::gains(data, &b, style, &candidates).iter().filter(|g| g.paid <= free && g.gain >= WORTH) {
        let upgrade = upgrades_owned(&g.item);
        if let Some(m) = a_move(data, if upgrade { "upgrade" } else { "buy" }, None, g, g.gain) {
            if upgrade { upgrades.push(m) } else { buys.push(m) }
        }
    }

    // Each owned item sold in turn: what it was worth, and the best item the refund and the free souls buy instead.
    let studies: Vec<(Worth, Vec<Move>)> = owned
        .iter()
        .filter_map(|class| {
            let item = data.item(class)?;
            let rest = without(data, &b, class);
            let back = suggest::gains(data, &rest, style, std::slice::from_ref(class));
            // Without a gain the item adds nothing the measure sees; it is worth nothing to the playstyle.
            let kept = back.first().map(|g| g.gain).unwrap_or(0.0).max(0.0);
            let worth = Worth { item: class.clone(), name: item.name.clone(), icon: item.icon.clone(), cost: item.cost, loss: kept / (1.0 + kept) };
            let refund = (item.cost as f64 * SELL_BACK) as u32;
            let others: Vec<String> = candidates.iter().filter(|c| !parts(data, c).contains(class)).cloned().collect();
            let mut best: Vec<Move> = suggest::gains(data, &rest, style, &others)
                .into_iter()
                .filter(|g| g.paid <= free + refund)
                .filter_map(|g| {
                    let net = (1.0 + g.gain) / (1.0 + kept) - 1.0;
                    (net >= WORTH).then(|| a_move(data, "swap", Some((class, refund)), &g, net)).flatten()
                })
                .collect();
            best.sort_by(|a, b| b.gain.total_cmp(&a.gain));
            best.truncate(LISTED);
            Some((worth, best))
        })
        .collect();
    let mut worth: Vec<Worth> = studies.iter().map(|s| s.0.clone()).collect();
    let mut swaps: Vec<Move> = studies.into_iter().flat_map(|s| s.1).collect();

    let by_gain = |a: &Move, b: &Move| b.gain.total_cmp(&a.gain);
    buys.sort_by(by_gain);
    upgrades.sort_by(by_gain);
    // One swap per item bought: the one that sells the least useful item, the cheapest of those.
    let loss_of = |m: &Move| worth.iter().find(|w| Some(&w.item) == m.sell.as_ref()).map(|w| w.loss).unwrap_or(0.0);
    swaps.sort_by(|a, b| by_gain(a, b).then(loss_of(a).total_cmp(&loss_of(b))).then(a.refund.cmp(&b.refund)));
    let mut seen = vec![];
    swaps.retain(|m| if seen.contains(&m.buy) { false } else { seen.push(m.buy.clone()); true });
    for list in [&mut buys, &mut upgrades, &mut swaps] {
        list.truncate(LISTED);
    }
    worth.sort_by(|a, b| a.loss.total_cmp(&b.loss));
    Ok(Advice { style: style.into(), free_souls: free, slots_used: sheet.slots_used, slot_limit: sheet.slot_limit, buys, upgrades, swaps, worth })
}
