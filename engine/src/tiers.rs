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
