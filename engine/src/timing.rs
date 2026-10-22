//! When a player bought each item, against when top players of the hero buy it: the analytics service's item
//! statistics by the minute of purchase give, for every item, the spread of the minutes top players buy it at and
//! how often those who bought it at a minute won. Items most top players buy that the player never did are listed.

use serde::Serialize;

use crate::data::GameData;
use crate::matches::Purchase;
use crate::popular::{shrunk, ItemStat};

/// An item is judged when at least this many top players' purchases stand behind it.
const MIN_BUYERS: u64 = 30;
/// The win rate at a player's minute counts the purchases within this many minutes of it.
const WIN_WINDOW_MIN: u64 = 2;
/// Past the late quartile by this much (or the spread of the middle half, if more), an item is very late.
const VERY_LATE_S: u32 = 180;
/// An item missing from the build is listed when top players buy it at least this often against the most bought
/// item, and their median is before the match ended.
const MISSING_SHARE: f64 = 0.4;
const MISSING_SHOWN: usize = 5;
/// Items from this price up are the big ones the summary speaks of.
const BIG_ITEM: u32 = 3_200;
/// An item sold again within this many seconds was a misclick, not a purchase.
const MISCLICK_S: u32 = 30;

#[derive(Debug, Clone, Serialize)]
pub struct ItemTiming {
    pub item: String,
    pub name: String,
    pub icon: String,
    pub cost: u32,
    pub bought_at: u32,
    /// The minute top players buy it at: the median, and the middle half between the quartiles, seconds.
    pub median_s: u32,
    pub early_s: u32,
    pub late_s: u32,
    pub buyers: u64,
    /// `early`, `on time`, `late` or `very late`.
    pub tone: String,
    /// Minutes after the median (negative: before).
    pub minutes_off: f64,
    /// How much more often top players who bought it around the player's minute won than all who bought it,
    /// points, pulled toward zero for few purchases.
    pub win_pp_then: Option<f64>,
}

#[derive(Debug, Clone, Serialize)]
pub struct MissingItem {
    pub item: String,
    pub name: String,
    pub icon: String,
    pub cost: u32,
    pub median_s: u32,
    /// Its purchases against the most bought item's.
    pub share: f64,
}

#[derive(Debug, Clone, Serialize)]
pub struct TimingReport {
    pub items: Vec<ItemTiming>,
    pub missing: Vec<MissingItem>,
    pub summary: String,
}

fn clock(s: u32) -> String {
    format!("{}:{:02}", s / 60, s % 60)
}

/// The minute below which a share of the purchases falls, in seconds: the middle of that minute.
fn quantile(minutes: &[(u64, u64)], q: f64) -> u32 {
    let total: u64 = minutes.iter().map(|m| m.1).sum();
    let mut seen = 0;
    for (minute, n) in minutes {
        seen += n;
        if seen as f64 >= q * total as f64 {
            return (*minute * 60 + 30) as u32;
        }
    }
    minutes.last().map(|m| (m.0 * 60 + 30) as u32).unwrap_or(0)
}

/// Each purchase of the player against top players' minutes for the item (rows of item statistics bucketed by
/// the minute of purchase, for the hero), and the items most of them buy that the player did not.
pub fn compare(data: &GameData, stats: &[ItemStat], purchases: &[Purchase], duration: u32) -> TimingReport {
    let rows_of = |id: u64| {
        let mut rows: Vec<&ItemStat> = stats.iter().filter(|r| r.item_id == id && r.bucket.is_some()).collect();
        rows.sort_by_key(|r| r.bucket);
        rows
    };
    let mut items = vec![];
    let mut seen: Vec<&str> = vec![];
    let mut bought: Vec<&Purchase> = purchases.iter().filter(|p| p.sold_at.is_none_or(|s| s >= p.time + MISCLICK_S)).collect();
    bought.sort_by_key(|p| p.time);
    for p in bought {
        // The first purchase of an item is the one that counts.
        if seen.contains(&p.item.as_str()) {
            continue;
        }
        seen.push(&p.item);
        let Some(item) = data.item(&p.item) else { continue };
        let rows = rows_of(item.id);
        let minutes: Vec<(u64, u64)> = rows.iter().map(|r| (r.bucket.unwrap_or(0), r.matches)).collect();
        let buyers: u64 = minutes.iter().map(|m| m.1).sum();
        if buyers < MIN_BUYERS {
            continue;
        }
        let (early, median, late) = (quantile(&minutes, 0.25), quantile(&minutes, 0.5), quantile(&minutes, 0.75));
        let tone = if p.time <= early {
            "early"
        } else if p.time <= late {
            "on time"
        } else if p.time <= late + VERY_LATE_S.max(late - early) {
            "late"
        } else {
            "very late"
        };
        let (w, n) = rows.iter().fold((0u64, 0u64), |a, r| (a.0 + r.wins, a.1 + r.wins + r.losses));
        let minute = (p.time / 60) as u64;
        let near: Vec<&&ItemStat> = rows.iter().filter(|r| r.bucket.unwrap_or(0).abs_diff(minute) <= WIN_WINDOW_MIN).collect();
        let (nw, nn) = near.iter().fold((0u64, 0u64), |a, r| (a.0 + r.wins, a.1 + r.wins + r.losses));
        let win_pp_then = (n > 0 && nn > 0).then(|| shrunk((nw as f64 / nn as f64 - w as f64 / n as f64) * 100.0, nn as f64));
        items.push(ItemTiming {
            item: p.item.clone(),
            name: item.name.clone(),
            icon: item.icon.clone(),
            cost: item.cost,
            bought_at: p.time,
            median_s: median,
            early_s: early,
            late_s: late,
            buyers,
            tone: tone.into(),
            minutes_off: (p.time as f64 - median as f64) / 60.0,
            win_pp_then,
        });
    }

    // Items most top players buy that the player never did, nor anything built on them.
    let built_on = |class: &str| purchases.iter().any(|p| p.item == class || data.item(&p.item).is_some_and(|i| i.components.iter().any(|c| c == class)));
    let mut totals: Vec<(u64, u64)> = vec![];
    for r in stats.iter().filter(|r| r.bucket.is_some()) {
        match totals.iter_mut().find(|t| t.0 == r.item_id) {
            Some(t) => t.1 += r.matches,
            None => totals.push((r.item_id, r.matches)),
        }
    }
    let most = totals.iter().map(|t| t.1).max().unwrap_or(1).max(1) as f64;
    let mut missing: Vec<MissingItem> = totals
        .iter()
        .filter(|t| t.1 as f64 >= MISSING_SHARE * most)
        .filter_map(|t| {
            let item = data.items.iter().find(|i| i.id == t.0)?;
            if built_on(&item.class_name) {
                return None;
            }
            let minutes: Vec<(u64, u64)> = rows_of(t.0).iter().map(|r| (r.bucket.unwrap_or(0), r.matches)).collect();
            let median = quantile(&minutes, 0.5);
            (median < duration).then(|| MissingItem {
                item: item.class_name.clone(),
                name: item.name.clone(),
                icon: item.icon.clone(),
                cost: item.cost,
                median_s: median,
                share: t.1 as f64 / most,
            })
        })
        .collect();
    missing.sort_by(|a, b| b.share.total_cmp(&a.share));
    missing.truncate(MISSING_SHOWN);

    let big: Vec<&ItemTiming> = items.iter().filter(|i| i.cost >= BIG_ITEM).collect();
    let mut summary = if big.is_empty() {
        "No big item to compare yet.".to_string()
    } else {
        let off = big.iter().map(|i| i.minutes_off).sum::<f64>() / big.len() as f64;
        let when = if off.abs() < 1.0 {
            "about when top players do".to_string()
        } else if off > 0.0 {
            format!("{off:.0} min later than top players")
        } else {
            format!("{:.0} min earlier than top players", -off)
        };
        let latest = big.iter().filter(|i| i.minutes_off >= 3.0).max_by(|a, b| a.minutes_off.total_cmp(&b.minutes_off));
        let worst = latest.map(|i| format!(" The latest was {} at {}, against {} for them.", i.name, clock(i.bought_at), clock(i.median_s))).unwrap_or_default();
        format!("Your items of {BIG_ITEM} souls and up came {when} on average.{worst}")
    };
    if let Some(m) = missing.first() {
        summary += &format!(" Most top players buy {} by {}; you never did.", m.name, clock(m.median_s));
    }
    TimingReport { items, missing, summary }
}
