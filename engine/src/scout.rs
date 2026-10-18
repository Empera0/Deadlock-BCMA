//! The players of a match being played: their rank, how much they play their hero and how they do on it, the
//! heroes they play most, their streak, and who is queued with whom.

use std::collections::BTreeMap;

use serde::Serialize;
use serde_json::Value;

use crate::data::GameData;
use crate::matches::LivePlayer;

/// The ranks by badge tier (badge = tier × 10 + subrank).
const RANKS: [&str; 12] = ["Obscurus", "Initiate", "Seeker", "Alchemist", "Arcanist", "Ritualist", "Emissary", "Archon", "Oracle", "Phantom", "Ascendant", "Eternus"];
/// A hero is a player's main when this share of their matches is on it, and new to them under this many matches.
const MAIN_SHARE: f64 = 0.4;
const NEW_ON_HERO: u64 = 5;
/// A streak this long is worth saying.
const STREAK: i64 = 3;
/// Heroes listed as a player's most played.
const MOST_PLAYED: usize = 3;
/// A win rate on few matches says little: it is pulled toward 50% by this many matches.
const PRIOR: f64 = 10.0;

#[derive(Debug, Clone, Serialize)]
pub struct Scout {
    pub account_id: u64,
    pub name: String,
    pub avatar: String,
    pub ally: bool,
    pub lane: i64,
    pub hero: u64,
    pub hero_name: String,
    pub rank: String,
    pub badge: u32,
    /// On the hero of this match: matches, wins, kills, deaths and assists per match.
    pub hero_matches: u64,
    pub hero_win_pct: f64,
    pub hero_kda: (f64, f64, f64),
    /// All heroes together.
    pub matches: u64,
    pub win_pct: f64,
    /// Most played heroes: (name, matches, win %).
    pub most_played: Vec<(String, u64, f64)>,
    /// Wins in a row (negative: losses) going into the match, when the rank service knows.
    pub streak: i64,
    /// Teammates on Steam friends lists with this player: likely queued together.
    pub party: Vec<String>,
    /// Short notes: main, new on the hero, a streak, a party.
    pub tags: Vec<String>,
}

fn rank_name(badge: u32) -> String {
    let (tier, sub) = ((badge / 10) as usize, badge % 10);
    match RANKS.get(tier) {
        Some(name) if tier > 0 && sub > 0 => format!("{name} {sub}"),
        Some(name) if tier > 0 => name.to_string(),
        _ => "Unranked".into(),
    }
}

fn rows(json: &str) -> Vec<Value> {
    serde_json::from_str(json).unwrap_or_default()
}

fn u(v: &Value, key: &str) -> u64 {
    v.get(key).and_then(Value::as_u64).unwrap_or(0)
}

/// Puts the scouting report together from the service's answers for the match's accounts: per-hero stats,
/// ranks and Steam profiles (with friends lists).
pub fn scout(data: &GameData, players: &[LivePlayer], hero_stats: &str, ranks: &str, profiles: &str) -> Vec<Scout> {
    let stats = rows(hero_stats);
    let ranks: BTreeMap<u64, Value> = rows(ranks).into_iter().map(|r| (u(&r, "account_id"), r)).collect();
    let profiles: BTreeMap<u64, Value> = rows(profiles).into_iter().map(|r| (u(&r, "account_id"), r)).collect();
    let hero_name = |id: u64| data.heroes.iter().find(|h| h.id == id).map(|h| h.name.clone()).unwrap_or_else(|| format!("Hero {id}"));
    let name_of = |account: u64| profiles.get(&account).and_then(|p| p.get("personaname")).and_then(Value::as_str).map(str::to_string).unwrap_or_else(|| account.to_string());
    let friends_of = |account: u64| -> Vec<u64> {
        profiles
            .get(&account)
            .and_then(|p| p.get("friends"))
            .and_then(Value::as_array)
            .map(|f| f.iter().map(|x| u(x, "account_id")).collect())
            .unwrap_or_default()
    };
    players
        .iter()
        .map(|p| {
            let mine: Vec<&Value> = stats.iter().filter(|r| u(r, "account_id") == p.account_id).collect();
            let matches: u64 = mine.iter().map(|r| u(r, "matches_played")).sum();
            let wins: u64 = mine.iter().map(|r| u(r, "wins")).sum();
            let on = mine.iter().find(|r| u(r, "hero_id") == p.hero);
            let hero_matches = on.map(|r| u(r, "matches_played")).unwrap_or(0);
            let hero_wins = on.map(|r| u(r, "wins")).unwrap_or(0);
            let per = |key: &str| on.map(|r| u(r, key) as f64 / hero_matches.max(1) as f64).unwrap_or(0.0);
            let mut most: Vec<&&Value> = mine.iter().collect();
            most.sort_by_key(|r| std::cmp::Reverse(u(r, "matches_played")));
            let most_played = most
                .iter()
                .take(MOST_PLAYED)
                .map(|r| (hero_name(u(r, "hero_id")), u(r, "matches_played"), u(r, "wins") as f64 / u(r, "matches_played").max(1) as f64 * 100.0))
                .collect();
            let rank = ranks.get(&p.account_id);
            let badge = rank.map(|r| u(r, "badge") as u32).unwrap_or(0);
            let streak = rank.and_then(|r| r.pointer("/last_match/player_rank_initial_win_streak")).and_then(Value::as_i64).unwrap_or(0);
            // Friends on the same team: either one's list names the other.
            let mine_friends = friends_of(p.account_id);
            let party: Vec<String> = players
                .iter()
                .filter(|o| o.account_id != p.account_id && o.ally == p.ally)
                .filter(|o| mine_friends.contains(&o.account_id) || friends_of(o.account_id).contains(&p.account_id))
                .map(|o| name_of(o.account_id))
                .collect();
            let hero_win_pct = (hero_wins as f64 + PRIOR * 0.5) / (hero_matches as f64 + PRIOR) * 100.0;
            let mut tags = vec![];
            if matches > 0 && hero_matches as f64 >= MAIN_SHARE * matches as f64 && hero_matches >= 20 {
                tags.push(format!("{} main: {:.0}% of their matches", hero_name(p.hero), hero_matches as f64 / matches as f64 * 100.0));
            }
            if hero_matches < NEW_ON_HERO {
                tags.push(if hero_matches == 0 { "First match on this hero".to_string() } else { format!("New on this hero: {hero_matches} matches") });
            }
            if streak >= STREAK {
                tags.push(format!("{streak} wins in a row"));
            } else if streak <= -STREAK {
                tags.push(format!("{} losses in a row", -streak));
            }
            if !party.is_empty() {
                tags.push(format!("Likely queued with {}", party.join(", ")));
            }
            Scout {
                account_id: p.account_id,
                name: name_of(p.account_id),
                avatar: profiles.get(&p.account_id).and_then(|x| x.get("avatarmedium")).and_then(Value::as_str).unwrap_or("").to_string(),
                ally: p.ally,
                lane: p.lane,
                hero: p.hero,
                hero_name: hero_name(p.hero),
                rank: rank_name(badge),
                badge,
                hero_matches,
                hero_win_pct,
                hero_kda: (per("kills"), per("deaths"), per("assists")),
                matches,
                win_pct: if matches > 0 { wins as f64 / matches as f64 * 100.0 } else { 0.0 },
                most_played,
                streak,
                party,
                tags,
            }
        })
        .collect()
}
