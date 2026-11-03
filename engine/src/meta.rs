//! What a patch did to the game: every hero's and item's win and pick rate in the weeks before a patch day beside
//! the days after it, from the statistics service's matches of both windows. A change is called clear only when it
//! is larger than chance would make it with that many matches.

use serde::Serialize;
use serde_json::Value;

use crate::data::GameData;
use crate::popular::ItemStat;
use crate::tiers::{item_tiers, tiers, HeroRow};

/// A change counts as clear past this many standard errors.
const CLEAR_SE: f64 = 2.0;
/// Too few matches in either window to say anything.
const MIN_MATCHES: u64 = 300;

/// A patch day from the game's changelog (deadlock-api `/v1/patches`): its title and the day it came out, as the
/// Unix time of that day's midnight (UTC).
#[derive(Debug, Clone, Serialize)]
pub struct PatchDay {
    pub title: String,
    pub day: u64,
}

/// Days since 1970-01-01 of a civil date (proleptic Gregorian).
fn days_from_civil(y: i64, m: i64, d: i64) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400;
    let mp = (m + 9) % 12;
    let doy = (153 * mp + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

/// The patch days, newest first. The day is read from the title ("09-29-2026" or "09-16-2026 Update"): the
/// changelog's own publishing date is often days later.
pub fn patch_days(json: &str) -> Vec<PatchDay> {
    let rows: Vec<Value> = serde_json::from_str(json).unwrap_or_default();
    let mut out: Vec<PatchDay> = rows
        .iter()
        .filter_map(|r| {
            let title = r.get("title")?.as_str()?.trim().to_string();
            let date = title.split_whitespace().next()?;
            let parts: Vec<i64> = date.split('-').filter_map(|p| p.parse().ok()).collect();
            let [m, d, y] = parts[..] else { return None };
            if !(1..=12).contains(&m) || !(1..=31).contains(&d) || y < 2024 {
                return None;
            }
            Some(PatchDay { title, day: days_from_civil(y, m, d) as u64 * 86_400 })
        })
        .collect();
    out.sort_by(|a, b| b.day.cmp(&a.day));
    out.dedup_by_key(|p| p.day);
    out
}

#[derive(Debug, Clone, Serialize)]
pub struct Shift {
    /// Class name, display name and icon; for items also the category and cost.
    pub key: String,
    pub name: String,
    pub icon: String,
    pub category: String,
    pub cost: u32,
    pub matches_before: u64,
    pub matches_after: u64,
    /// Percent before and after, and the change in points.
    pub win_before: f64,
    pub win_after: f64,
    pub win_change: f64,
    pub pick_before: f64,
    pub pick_after: f64,
    pub pick_change: f64,
    /// The win rate change is larger than chance would make it.
    pub clear: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct MetaShift {
    pub heroes: Vec<Shift>,
    pub items: Vec<Shift>,
}

/// Whether a change in win rate between two samples is clear: past `CLEAR_SE` standard errors of the difference.
fn clear(p1: f64, n1: u64, p2: f64, n2: u64) -> bool {
    if n1 < MIN_MATCHES || n2 < MIN_MATCHES {
        return false;
    }
    let p = (p1 * n1 as f64 + p2 * n2 as f64) / (n1 + n2) as f64 / 100.0;
    let se = (p * (1.0 - p) * (1.0 / n1 as f64 + 1.0 / n2 as f64)).sqrt() * 100.0;
    (p2 - p1).abs() > CLEAR_SE * se
}

/// Every hero and item before and after: the clear changes first, the largest first.
pub fn shift(data: &GameData, heroes_before: &[HeroRow], heroes_after: &[HeroRow], items_before: &[ItemStat], items_after: &[ItemStat]) -> MetaShift {
    let (hb, ha) = (tiers(data, heroes_before, &[]), tiers(data, heroes_after, &[]));
    let players = |rows: &[HeroRow]| rows.iter().map(|r| r.matches).sum::<u64>();
    let (ib, ia) = (item_tiers(data, items_before, players(heroes_before)), item_tiers(data, items_after, players(heroes_after)));
    let mut heroes: Vec<Shift> = ha
        .iter()
        .filter_map(|a| {
            let b = hb.iter().find(|b| b.hero_id == a.hero_id)?;
            Some(Shift {
                key: a.hero.clone(),
                name: a.name.clone(),
                icon: a.icon.clone(),
                category: a.hero_type.clone(),
                cost: 0,
                matches_before: b.matches,
                matches_after: a.matches,
                win_before: b.win_rate,
                win_after: a.win_rate,
                win_change: a.win_rate - b.win_rate,
                pick_before: b.pick_rate,
                pick_after: a.pick_rate,
                pick_change: a.pick_rate - b.pick_rate,
                clear: clear(b.win_rate, b.matches, a.win_rate, a.matches),
            })
        })
        .collect();
    let mut items: Vec<Shift> = ia
        .iter()
        .filter_map(|a| {
            let b = ib.iter().find(|b| b.item == a.item)?;
            Some(Shift {
                key: a.item.clone(),
                name: a.name.clone(),
                icon: a.icon.clone(),
                category: a.category.clone(),
                cost: a.cost,
                matches_before: b.matches,
                matches_after: a.matches,
                win_before: b.win_rate,
                win_after: a.win_rate,
                win_change: a.win_rate - b.win_rate,
                pick_before: b.pick_rate,
                pick_after: a.pick_rate,
                pick_change: a.pick_rate - b.pick_rate,
                clear: clear(b.win_rate, b.matches, a.win_rate, a.matches),
            })
        })
        .collect();
    let order = |a: &Shift, b: &Shift| b.clear.cmp(&a.clear).then(b.win_change.abs().total_cmp(&a.win_change.abs()));
    heroes.sort_by(order);
    items.sort_by(order);
    MetaShift { heroes, items }
}
