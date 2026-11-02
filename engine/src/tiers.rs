//! The heroes ranked by how they do in recent matches at a rank range: win rate, how often they are picked and
//! banned, and what their players average. The win rate a hero is ranked by is pulled toward even by its matches,
//! so a hero played rarely is not put at the top on a lucky run.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::data::GameData;

/// Matches of an even record pulled into each hero's win rate before it is ranked.
const PRIOR: f64 = 1_000.0;
/// Heroes play this many to a match.
const PLAYERS: f64 = 12.0;
/// Tiers by how far a hero's ranked win rate is from the average hero's, in win rate points.
const TIERS: [(&str, f64); 4] = [("S", 2.5), ("A", 1.0), ("B", -1.0), ("C", -2.5)];

/// A row of the API's hero-stats.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct HeroRow {
    pub hero_id: u64,
    pub matches: u64,
    pub wins: u64,
    pub total_kills: u64,
    pub total_deaths: u64,
    pub total_assists: u64,
    pub total_net_worth: u64,
    pub total_player_damage: u64,
    pub total_last_hits: u64,
    pub total_shots_hit: u64,
    pub total_shots_missed: u64,
}

#[derive(Debug, Clone, Serialize)]
pub struct HeroTier {
    pub hero_id: u64,
    pub hero: String,
    pub name: String,
    pub icon: String,
    pub hero_type: String,
    /// S, A, B, C or D.
    pub tier: String,
    pub matches: u64,
    /// Percent of its matches won, and the same pulled toward even by its matches (what it is ranked by).
    pub win_rate: f64,
    pub ranked_rate: f64,
    /// Percent of matches it is in, and its share of all bans.
    pub pick_rate: f64,
    pub ban_share: f64,
    /// Its players' averages per match.
    pub kills: f64,
    pub deaths: f64,
    pub assists: f64,
    pub net_worth: f64,
}

/// Every hero of the game with matches, the best first.
pub fn tiers(data: &GameData, rows: &[HeroRow], bans: &[(u64, u64)]) -> Vec<HeroTier> {
    let mut by_hero: BTreeMap<u64, HeroRow> = BTreeMap::new();
    // The API gives a row per bucket; a hero's rows are added up.
    for r in rows {
        let e = by_hero.entry(r.hero_id).or_default();
        e.hero_id = r.hero_id;
        e.matches += r.matches;
        e.wins += r.wins;
        e.total_kills += r.total_kills;
        e.total_deaths += r.total_deaths;
        e.total_assists += r.total_assists;
        e.total_net_worth += r.total_net_worth;
        e.total_player_damage += r.total_player_damage;
        e.total_last_hits += r.total_last_hits;
        e.total_shots_hit += r.total_shots_hit;
        e.total_shots_missed += r.total_shots_missed;
    }
    let slots: u64 = by_hero.values().map(|r| r.matches).sum();
    let all_matches = slots as f64 / PLAYERS;
    let all_bans: u64 = bans.iter().map(|b| b.1).sum();
    let ranked = |r: &HeroRow| (r.wins as f64 + PRIOR * 0.5) / (r.matches as f64 + PRIOR) * 100.0;
    // The average hero, weighted by matches: about 50, but not quite when the range cuts matches unevenly.
    let average = if slots > 0 { by_hero.values().map(|r| ranked(r) * r.matches as f64).sum::<f64>() / slots as f64 } else { 50.0 };
    let mut out: Vec<HeroTier> = data
        .heroes
        .iter()
        .filter_map(|h| {
            let r = by_hero.get(&h.id).filter(|r| r.matches > 0)?;
            let n = r.matches as f64;
            let rate = ranked(r);
            let tier = TIERS.iter().find(|(_, above)| rate - average >= *above).map(|(t, _)| *t).unwrap_or("D");
            Some(HeroTier {
                hero_id: h.id,
                hero: h.class_name.clone(),
                name: h.name.clone(),
                icon: h.icon.clone(),
                hero_type: h.hero_type.clone(),
                tier: tier.to_string(),
                matches: r.matches,
                win_rate: r.wins as f64 / n * 100.0,
                ranked_rate: rate,
                pick_rate: if all_matches > 0.0 { n / all_matches * 100.0 } else { 0.0 },
                ban_share: if all_bans > 0 { bans.iter().find(|b| b.0 == h.id).map(|b| b.1).unwrap_or(0) as f64 / all_bans as f64 * 100.0 } else { 0.0 },
                kills: r.total_kills as f64 / n,
                deaths: r.total_deaths as f64 / n,
                assists: r.total_assists as f64 / n,
                net_worth: r.total_net_worth as f64 / n,
            })
        })
        .collect();
    out.sort_by(|a, b| b.ranked_rate.total_cmp(&a.ranked_rate));
    out
}

/// Items are compared within their own cost: an expensive item is bought by players already ahead, so its raw win
/// rate is high whatever it does.
#[derive(Debug, Clone, Serialize)]
pub struct ItemTier {
    pub item: String,
    pub name: String,
    pub icon: String,
    pub category: String,
    pub cost: u32,
    /// S, A, B, C or D against the other items of the same cost.
    pub tier: String,
    pub matches: u64,
    /// Percent of the matches it was bought in that were won, and the same pulled toward its cost's average.
    pub win_rate: f64,
    pub ranked_rate: f64,
    /// Percent of players who bought it.
    pub pick_rate: f64,
    /// When it is bought on average, in minutes.
    pub buy_minute: f64,
}

/// Every item of the shop with purchases, the best of each cost first; `players` is the number of player matches
/// the purchases come from.
pub fn item_tiers(data: &GameData, rows: &[crate::popular::ItemStat], players: u64) -> Vec<ItemTier> {
    let mut by_item: BTreeMap<u64, (u64, u64, f64)> = BTreeMap::new();
    for r in rows {
        let e = by_item.entry(r.item_id).or_insert((0, 0, 0.0));
        e.0 += r.wins;
        e.1 += r.matches;
        e.2 += r.avg_buy_time_s * r.matches as f64;
    }
    let shop: Vec<(&crate::data::Item, (u64, u64, f64))> =
        data.items.iter().filter_map(|i| by_item.get(&i.id).filter(|r| r.1 > 0).map(|r| (i, *r))).collect();
    // Each cost's average win rate, weighted by purchases.
    let mut per_cost: BTreeMap<u32, (u64, u64)> = BTreeMap::new();
    for (i, (wins, matches, _)) in &shop {
        let e = per_cost.entry(i.cost).or_default();
        e.0 += wins;
        e.1 += matches;
    }
    let mut out: Vec<ItemTier> = shop
        .iter()
        .map(|(i, (wins, matches, time))| {
            let avg = per_cost.get(&i.cost).map(|(w, m)| *w as f64 / (*m).max(1) as f64).unwrap_or(0.5);
            let ranked = (*wins as f64 + PRIOR * avg) / (*matches as f64 + PRIOR) * 100.0;
            let tier = TIERS.iter().find(|(_, above)| ranked - avg * 100.0 >= *above).map(|(t, _)| *t).unwrap_or("D");
            ItemTier {
                item: i.class_name.clone(),
                name: i.name.clone(),
                icon: i.icon.clone(),
                category: i.category.clone(),
                cost: i.cost,
                tier: tier.to_string(),
                matches: *matches,
                win_rate: *wins as f64 / *matches as f64 * 100.0,
                ranked_rate: ranked,
                pick_rate: if players > 0 { *matches as f64 / players as f64 * 100.0 } else { 0.0 },
                buy_minute: time / *matches as f64 / 60.0,
            }
        })
        .collect();
    out.sort_by(|a, b| a.cost.cmp(&b.cost).then(b.ranked_rate.total_cmp(&a.ranked_rate)));
    out
}

/// A row of the API's per-player hero statistics (`/v1/players/hero-stats`): one player's matches on one hero.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct MyHeroRow {
    pub hero_id: u64,
    pub matches_played: u64,
    /// Seconds.
    pub time_played: u64,
    pub wins: u64,
    pub kills: u64,
    pub deaths: u64,
    pub assists: u64,
    pub total_player_damage: u64,
    pub networth_per_min: f64,
    pub last_hits_per_min: f64,
    /// Share of shots that hit, 0 to 1.
    pub accuracy: f64,
}

/// The player's value and the average player's, per match.
#[derive(Debug, Clone, Copy, Serialize)]
pub struct Versus {
    pub mine: f64,
    pub average: f64,
}

/// How a player does on a hero against the average player of their rank on it.
#[derive(Debug, Clone, Serialize)]
pub struct Mastery {
    pub hero_id: u64,
    pub hero: String,
    pub name: String,
    pub icon: String,
    pub matches: u64,
    /// Percent.
    pub win_rate: Versus,
    pub kills: Versus,
    pub deaths: Versus,
    pub assists: Versus,
    pub souls: Versus,
    pub damage: Versus,
    pub last_hits: Versus,
    /// Percent of shots that hit; `None` when the average has no shots counted.
    pub accuracy: Option<Versus>,
}

/// Every hero the player has matches on, the most played first, beside the average player of the given ranks
/// (`average` is the API's hero statistics at those ranks).
pub fn mastery(data: &GameData, mine: &[MyHeroRow], average: &[HeroRow]) -> Vec<Mastery> {
    let mut avg: BTreeMap<u64, HeroRow> = BTreeMap::new();
    for r in average {
        let e = avg.entry(r.hero_id).or_default();
        e.matches += r.matches;
        e.wins += r.wins;
        e.total_kills += r.total_kills;
        e.total_deaths += r.total_deaths;
        e.total_assists += r.total_assists;
        e.total_net_worth += r.total_net_worth;
        e.total_player_damage += r.total_player_damage;
        e.total_last_hits += r.total_last_hits;
        e.total_shots_hit += r.total_shots_hit;
        e.total_shots_missed += r.total_shots_missed;
    }
    let mut out: Vec<Mastery> = mine
        .iter()
        .filter(|m| m.matches_played > 0)
        .filter_map(|m| {
            let h = data.heroes.iter().find(|h| h.id == m.hero_id)?;
            let a = avg.get(&m.hero_id).filter(|a| a.matches > 0)?;
            let n = m.matches_played as f64;
            let an = a.matches as f64;
            // Per-minute values back to per match, over the minutes actually played.
            let minutes = m.time_played as f64 / 60.0;
            let vs = |mine: f64, average: f64| Versus { mine, average };
            let shots = a.total_shots_hit + a.total_shots_missed;
            Some(Mastery {
                hero_id: h.id,
                hero: h.class_name.clone(),
                name: h.name.clone(),
                icon: h.icon.clone(),
                matches: m.matches_played,
                win_rate: vs(m.wins as f64 / n * 100.0, a.wins as f64 / an * 100.0),
                kills: vs(m.kills as f64 / n, a.total_kills as f64 / an),
                deaths: vs(m.deaths as f64 / n, a.total_deaths as f64 / an),
                assists: vs(m.assists as f64 / n, a.total_assists as f64 / an),
                souls: vs(m.networth_per_min * minutes / n, a.total_net_worth as f64 / an),
                damage: vs(m.total_player_damage as f64 / n, a.total_player_damage as f64 / an),
                last_hits: vs(m.last_hits_per_min * minutes / n, a.total_last_hits as f64 / an),
                accuracy: (shots > 0).then(|| vs(m.accuracy * 100.0, a.total_shots_hit as f64 / shots as f64 * 100.0)),
            })
        })
        .collect();
    out.sort_by(|a, b| b.matches.cmp(&a.matches));
    out
}
