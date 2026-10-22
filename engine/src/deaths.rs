//! Why a player died: who killed them with what build, what hit them around the death, and the duel between the
//! two: how fast the killer's full fight takes the player's health and how fast the player's would have taken the
//! killer's. Items are tried on both sides of it, defensive and offensive alike: an item that keeps the player
//! alive a second longer is worth little when the fight was lost by ten, and one that lets the player kill first
//! is worth more than any amount of health.

use std::collections::BTreeMap;

use serde::Serialize;
use serde_json::Value;

use crate::build::BuildInput;
use crate::calc;
use crate::combo::{ComboResult, Kind};
use crate::data::GameData;
use crate::matches::{build_at, events, int, list, num, source_label, AGGREGATES};
use crate::suggest;

/// The killer's fight: every ability and active item once and this many seconds of shooting.
const SHOOTING: f64 = 8.0;
/// The dearest item worth trying, by the minute of the death: the lane, the mid game, the late game.
const ITEM_CAPS: [(u32, u32); 3] = [(9 * 60, 1_600), (20 * 60, 3_200), (u32::MAX, 6_400)];
/// Items that only come from the Unstable Rift cost this; they are not in the shop.
const NOT_IN_SHOP: u32 = 9_999;
/// Fixes listed per death, and over the whole match.
const FIXES: usize = 3;
const MATCH_FIXES: usize = 4;
/// An item that only keeps the player alive is worth listing from this many more seconds alive.
const MIN_SECONDS_BOUGHT: f64 = 2.0;
/// An item that changes the duel by less than this, seconds, is not worth listing.
const MIN_MARGIN: f64 = 1.0;
/// Time-to-kill estimates are held to this many seconds: past it the fight is simply lost or won.
const LONG_FIGHT_S: f64 = 30.0;

#[derive(Debug, Clone, Serialize)]
pub struct Hit {
    /// The dealer's hero and the source, e.g. "Haze: Weapon".
    pub name: String,
    pub amount: f64,
}

/// A change to the player's build and what the killer's fight does to it then.
#[derive(Debug, Clone, Serialize)]
pub struct Fix {
    pub item: String,
    pub name: String,
    pub icon: String,
    /// 0 for an active item the player already had, used at the start of the fight.
    pub cost: u32,
    pub owned: bool,
    /// Share of the player's health the killer's fight takes with the item (1 = exactly dead).
    pub health_taken: f64,
    pub kill_time: Option<f64>,
    /// With every slot taken, the item held that is sold to make room for it: the cheapest one.
    pub replaces: Option<String>,
    /// `defense` for a vitality item or an owned defensive active, `offense` for a weapon or spirit item.
    pub kind: String,
    /// Seconds more the player lives through the killer's fight (estimated past the fight's end).
    pub seconds_gained: f64,
    /// Seconds the player's own fight takes to kill the killer with the item (estimated past the fight's end).
    pub my_time_to_kill: f64,
    /// The player kills the killer first with it, where without it the killer won.
    pub wins_duel: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct DeathStudy {
    pub time: u32,
    pub killer_slot: u32,
    pub killer_hero: String,
    /// Seconds from the first damage of the fight to the death, as the game recorded it.
    pub time_to_kill: f64,
    pub my_items: Vec<String>,
    pub killer_items: Vec<String>,
    /// The damage-matrix interval the death falls in, and what hit the player in it.
    pub window_from: u32,
    pub window_to: u32,
    pub taken: Vec<Hit>,
    /// Share of the interval's damage from bullets and melee (the rest is spirit).
    pub bullet_share: f64,
    /// The killer's full fight against the player's build at the time.
    pub fight_damage: f64,
    pub my_health: f64,
    pub health_taken: f64,
    pub kill_time: Option<f64>,
    pub fight_bullet_share: f64,
    /// The duel at the time, seconds (estimated past the fights' end): the killer's fight to kill the player, and
    /// the player's to kill the killer.
    pub their_time_to_kill: f64,
    pub my_time_to_kill: f64,
    pub fixes: Vec<Fix>,
    pub verdict: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct DeathAnalysis {
    pub slot: u32,
    pub hero: String,
    pub deaths: Vec<DeathStudy>,
    /// Share of all the damage taken in the death intervals that came from bullets and melee.
    pub bullet_share: f64,
    /// The killers, most kills first: (hero, kills).
    pub killers: Vec<(String, u32)>,
    /// Fixes that came up most often, with the number of deaths they helped in.
    pub fixes: Vec<(Fix, u32)>,
    pub summary: String,
}

/// The player's level at a point in the match: the last snapshot before it.
fn level_at(player: &Value, time: u32) -> u32 {
    list(player, "stats").iter().filter(|s| int(s, "time_stamp_s") <= time).last().map(|s| int(s, "level")).unwrap_or(1).max(1)
}

/// A player's build at a point in the match, with the hero's name.
fn build_of(data: &GameData, player: &Value, time: u32) -> Option<(BuildInput, String)> {
    let id = player.get("hero_id").and_then(Value::as_u64)?;
    let hero = data.heroes.iter().find(|h| h.id == id)?;
    let ev = events(data, hero, player);
    Some((build_at(hero, &ev, time, level_at(player, time)), hero.name.clone()))
}

/// The build of the player in a slot at a point of a match (the parsed raw match metadata), as the match data has it.
pub fn build_in_match(data: &GameData, root: &Value, slot: u32, time: u32) -> Option<BuildInput> {
    let player = list(root.get("match_info")?, "players").iter().find(|p| int(p, "player_slot") == slot)?.clone();
    build_of(data, &player, time).map(|b| b.0)
}

fn item_names(data: &GameData, b: &BuildInput) -> Vec<String> {
    b.purchases.iter().filter_map(|c| data.item(c)).map(|i| i.name.clone()).collect()
}

/// What hit the player in the damage-matrix interval a death falls in: (from, to, hits, bullet share).
fn taken_around(data: &GameData, matrix: &Value, heroes: &BTreeMap<u32, String>, slot: u32, time: u32) -> (u32, u32, Vec<Hit>, f64) {
    let times: Vec<u32> = list(matrix, "sample_time_s").iter().map(|t| t.as_f64().unwrap_or(0.0) as u32).collect();
    let Some(end) = times.iter().position(|t| *t >= time).or(times.len().checked_sub(1)) else { return (0, time, vec![], 0.0) };
    let from = if end == 0 { 0 } else { times[end - 1] };
    let details = matrix.get("source_details").cloned().unwrap_or(Value::Null);
    let (types, names) = (list(&details, "stat_type"), list(&details, "source_name"));
    let mut totals: BTreeMap<String, f64> = BTreeMap::new();
    let (mut bullet, mut all) = (0.0, 0.0);
    for dealer in list(matrix, "damage_dealers") {
        let Some(hero) = heroes.get(&int(dealer, "dealer_player_slot")) else { continue };
        for source in list(dealer, "damage_sources") {
            let index = int(source, "source_details_index") as usize;
            let raw = names.get(index).and_then(Value::as_str).unwrap_or("");
            if types.get(index).and_then(Value::as_u64) != Some(0) || AGGREGATES.contains(&raw) {
                continue;
            }
            for target in list(source, "damage_to_players").iter().filter(|t| int(t, "target_player_slot") == slot) {
                let values = list(target, "damage");
                // A row starts when the first damage was dealt, so it lines up with the end of the time list.
                let skip = times.len().saturating_sub(values.len());
                let at = |i: usize| if i < skip { 0.0 } else { values.get(i - skip).and_then(Value::as_f64).unwrap_or(0.0) };
                let amount = at(end) - if end == 0 { 0.0 } else { at(end - 1) };
                if amount <= 0.0 {
                    continue;
                }
                *totals.entry(format!("{hero}: {}", source_label(data, raw))).or_default() += amount;
                all += amount;
                if raw.starts_with("citadel_weapon_") || raw.starts_with("ability_melee_") {
                    bullet += amount;
                }
            }
        }
    }
    let mut hits: Vec<Hit> = totals.into_iter().map(|(name, amount)| Hit { name, amount }).collect();
    hits.sort_by(|a, b| b.amount.total_cmp(&a.amount));
    (from, times[end], hits, if all > 0.0 { bullet / all } else { 0.0 })
}

/// Share of the target's health a fight takes: what reached health over the health it had.
fn health_taken(r: &ComboResult) -> f64 {
    ((r.total - r.absorbed) / r.target.health.max(1.0)).max(0.0)
}

fn bullet_share(r: &ComboResult) -> f64 {
    let by = |kinds: &[Kind]| r.sources.iter().filter(|s| kinds.contains(&s.kind)).map(|s| s.damage).sum::<f64>();
    let (gun, spirit) = (by(&[Kind::Bullet, Kind::Melee]), by(&[Kind::Spirit]));
    if gun + spirit > 0.0 { gun / (gun + spirit) } else { 0.0 }
}

fn pct(v: f64) -> String {
    format!("{:.0}%", v * 100.0)
}

/// The seconds a fight takes to kill its target: when it does within the fight, else the fight's pace carried on.
fn time_to_kill(r: &ComboResult) -> f64 {
    if let Some(t) = r.kill_time {
        return t.min(LONG_FIGHT_S);
    }
    let taken = health_taken(r);
    if taken <= 0.0 {
        return LONG_FIGHT_S;
    }
    (r.full_duration.max(r.duration).max(1.0) / taken).min(LONG_FIGHT_S)
}

/// The fixes for one death: the player's own defensive actives used at the start, then each shop item tried in
/// both fights, the killer's against the player and the player's against the killer.
fn fixes(data: &GameData, killer: &BuildInput, me: &BuildInput, time: u32, theirs: f64, mine: f64) -> Vec<Fix> {
    let cap = ITEM_CAPS.iter().find(|(until, _)| time < *until).map(|c| c.1).unwrap_or(6_400);
    let usable = |class: &str| data.item(class).is_some_and(|i| i.active && suggest::DEFENSIVE_ACTIVES.contains(&i.name.as_str()));
    let mut trials: Vec<(String, bool)> = me.purchases.iter().filter(|c| usable(c)).map(|c| (c.clone(), true)).collect();
    trials.extend(
        data.items
            .iter()
            .filter(|i| ["vitality", "weapon", "spirit"].contains(&i.category.as_str()))
            .filter(|i| i.cost <= cap && i.cost < NOT_IN_SHOP && !me.purchases.contains(&i.class_name))
            .map(|i| (i.class_name.clone(), false)),
    );
    let margin = theirs - mine;
    let mut out: Vec<Fix> = trials
        .into_iter()
        .filter_map(|(class, owned)| {
            let item = data.item(&class)?;
            let mut trial = me.clone();
            let mut replaces = None;
            if !owned {
                trial.purchases.push(class.clone());
                // No free slot: the cheapest item held is sold for it, as a player would.
                let sheet = calc::compute(data, &trial).ok()?;
                if sheet.slots_used > sheet.slot_limit {
                    let (sold, name) = sheet
                        .owned
                        .iter()
                        .filter(|c| **c != class)
                        .filter_map(|c| data.item(c).map(|i| (c.clone(), i.name.clone(), i.cost)))
                        .min_by_key(|x| x.2)
                        .map(|x| (x.0, x.1))?;
                    trial.purchases.retain(|c| *c != sold);
                    trial.active.retain(|c| *c != sold);
                    replaces = Some(name);
                }
            }
            let actives: Vec<String> = if usable(&class) { vec![class.clone()] } else { vec![] };
            let defense = owned || item.category == "vitality" || usable(&class);
            // The killer's fight against the player with the item, and the player's against the killer. The side the
            // item is for is tried first; when it changes nothing there, the other fight is not run at all.
            let (against, back) = if defense {
                let against = suggest::fight_against(data, killer, &trial, &actives, SHOOTING)?;
                if time_to_kill(&against) - theirs < MIN_MARGIN {
                    return None;
                }
                (against, suggest::fight_against(data, &trial, killer, &[], SHOOTING)?)
            } else {
                let back = suggest::fight_against(data, &trial, killer, &[], SHOOTING)?;
                if mine - time_to_kill(&back) < MIN_MARGIN {
                    return None;
                }
                (suggest::fight_against(data, killer, &trial, &actives, SHOOTING)?, back)
            };
            let (their, my) = (time_to_kill(&against), time_to_kill(&back));
            let gained = their - theirs;
            let wins = margin <= 0.0 && my < their;
            let better = (their - my) - margin;
            // Worth listing: it turns the duel, or changes it by a second or more, and a defensive item that only
            // keeps the player alive has to keep them alive a while.
            let worth = wins || (better >= MIN_MARGIN && (!defense || gained >= MIN_SECONDS_BOUGHT || my < mine - MIN_MARGIN));
            worth.then(|| Fix {
                item: class.clone(),
                name: item.name.clone(),
                icon: item.icon.clone(),
                cost: if owned { 0 } else { item.cost },
                owned,
                health_taken: health_taken(&against),
                kill_time: against.kill_time,
                replaces,
                kind: if defense { "defense" } else { "offense" }.into(),
                seconds_gained: gained,
                my_time_to_kill: my,
                wins_duel: wins,
            })
        })
        .collect();
    // Owned actives first: they cost nothing. Then whatever turns the duel, then the most it changes.
    let change = |f: &Fix| (theirs + f.seconds_gained - f.my_time_to_kill) - margin;
    out.sort_by(|a, b| b.owned.cmp(&a.owned).then(b.wins_duel.cmp(&a.wins_duel)).then(change(b).total_cmp(&change(a))).then(a.cost.cmp(&b.cost)));
    out.truncate(FIXES + out.iter().filter(|f| f.owned).count());
    out
}

/// ", in place of Extra Charge" when the slots were full.
fn instead(f: &Fix) -> String {
    f.replaces.as_ref().map(|r| format!(", in place of {r}")).unwrap_or_default()
}

/// Seconds as said in a verdict: "4.1s", or "over 30s" for a fight that never ends.
fn secs(t: f64) -> String {
    if t >= LONG_FIGHT_S { format!("over {LONG_FIGHT_S:.0}s") } else { format!("{t:.1}s") }
}

fn verdict(s: &DeathStudy) -> String {
    let kind = if s.fight_bullet_share >= 0.65 {
        "mostly gun damage"
    } else if s.fight_bullet_share <= 0.35 {
        "mostly spirit damage"
    } else {
        "a mix of gun and spirit damage"
    };
    let fight = match s.kill_time {
        Some(t) => format!("{}'s full fight kills you on its own in {:.1}s ({kind})", s.killer_hero, t),
        None => format!("{}'s full fight takes {} of your health ({kind}): someone else helped", s.killer_hero, pct(s.health_taken)),
    };
    let duel = if s.my_time_to_kill < s.their_time_to_kill {
        format!(" Your own fight kills {} in {}, first: the duel was yours to win", s.killer_hero, secs(s.my_time_to_kill))
    } else {
        format!(" Your own fight needs {} to kill {}, against {} for theirs: the duel was lost", secs(s.my_time_to_kill), s.killer_hero, secs(s.their_time_to_kill))
    };
    let fix = match s.fixes.first() {
        Some(f) if f.owned => format!(" Using your {} at the start keeps you alive {:.1}s longer.", f.name, f.seconds_gained),
        Some(f) if f.wins_duel => format!(
            " {} ({} souls{}) turns it: you kill {} in {} and live {}.",
            f.name,
            f.cost,
            instead(f),
            s.killer_hero,
            secs(f.my_time_to_kill),
            secs(s.their_time_to_kill + f.seconds_gained)
        ),
        Some(f) if f.kind == "offense" => format!(" {} ({} souls{}) kills {} faster, in {}.", f.name, f.cost, instead(f), s.killer_hero, secs(f.my_time_to_kill)),
        Some(f) => format!(" {} ({} souls{}) keeps you alive {:.1}s longer.", f.name, f.cost, instead(f), f.seconds_gained),
        None => " No item within reach changes it by much: the fight was lost before it started.".into(),
    };
    format!("{fight}.{duel}.{fix}")
}

/// Studies every death of the player in the given slot.
pub fn analyze(data: &GameData, json: &str, slot: u32) -> Result<DeathAnalysis, String> {
    let root: Value = serde_json::from_str(json).map_err(|e| format!("Unreadable match data: {e}"))?;
    let info = root.get("match_info").ok_or("The response has no match information")?;
    let matrix = info.get("damage_matrix").cloned().unwrap_or(Value::Null);
    let players = list(info, "players");
    let player = players.iter().find(|p| int(p, "player_slot") == slot).ok_or("No such player in the match")?;
    let heroes: BTreeMap<u32, String> = players
        .iter()
        .filter_map(|p| {
            let id = p.get("hero_id").and_then(Value::as_u64)?;
            Some((int(p, "player_slot"), data.heroes.iter().find(|h| h.id == id)?.name.clone()))
        })
        .collect();
    let hero = heroes.get(&slot).cloned().ok_or("The player's hero is not in the data")?;

    let records = list(player, "death_details");
    let studies: Vec<Option<DeathStudy>> = std::thread::scope(|scope| {
        let handles: Vec<_> = records
            .iter()
            .map(|d| {
                let (matrix, heroes) = (&matrix, &heroes);
                scope.spawn(move || {
                    let time = int(d, "game_time_s");
                    let killer_slot = int(d, "killer_player_slot");
                    let killer = players.iter().find(|p| int(p, "player_slot") == killer_slot)?;
                    let (killer_build, killer_hero) = build_of(data, killer, time)?;
                    let (me, _) = build_of(data, player, time)?;
                    let (window_from, window_to, taken, share) = taken_around(data, matrix, heroes, slot, time);
                    let r = suggest::fight_against(data, &killer_build, &me, &[], SHOOTING)?;
                    let back = suggest::fight_against(data, &me, &killer_build, &[], SHOOTING)?;
                    let before = health_taken(&r);
                    let (theirs, mine) = (time_to_kill(&r), time_to_kill(&back));
                    let mut s = DeathStudy {
                        time,
                        killer_slot,
                        killer_hero,
                        time_to_kill: num(d, "time_to_kill_s"),
                        my_items: item_names(data, &me),
                        killer_items: item_names(data, &killer_build),
                        window_from,
                        window_to,
                        taken,
                        bullet_share: share,
                        fight_damage: r.total,
                        my_health: r.target.health,
                        health_taken: before,
                        kill_time: r.kill_time,
                        fight_bullet_share: bullet_share(&r),
                        their_time_to_kill: theirs,
                        my_time_to_kill: mine,
                        fixes: fixes(data, &killer_build, &me, time, theirs, mine),
                        verdict: String::new(),
                    };
                    s.verdict = verdict(&s);
                    Some(s)
                })
            })
            .collect();
        handles.into_iter().map(|h| h.join().ok().flatten()).collect()
    });
    let deaths: Vec<DeathStudy> = studies.into_iter().flatten().collect();

    let (mut bullet, mut all) = (0.0, 0.0);
    for s in &deaths {
        let sum: f64 = s.taken.iter().map(|h| h.amount).sum();
        all += sum;
        bullet += sum * s.bullet_share;
    }
    let share = if all > 0.0 { bullet / all } else { 0.0 };
    let mut killers: BTreeMap<String, u32> = BTreeMap::new();
    for s in &deaths {
        *killers.entry(s.killer_hero.clone()).or_default() += 1;
    }
    let mut killers: Vec<(String, u32)> = killers.into_iter().collect();
    killers.sort_by(|a, b| b.1.cmp(&a.1));
    let mut counted: Vec<(Fix, u32)> = vec![];
    for f in deaths.iter().flat_map(|s| s.fixes.iter().filter(|f| !f.owned)) {
        match counted.iter_mut().find(|(c, _)| c.item == f.item) {
            Some((_, n)) => *n += 1,
            None => counted.push((f.clone(), 1)),
        }
    }
    counted.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cost.cmp(&b.0.cost)));
    counted.truncate(MATCH_FIXES);

    let summary = if deaths.is_empty() {
        "No deaths to study.".to_string()
    } else {
        let kind = if share >= 0.65 {
            "Most of the damage around your deaths was gun damage: bullet resist and fire-rate slows pay off most."
        } else if share <= 0.35 {
            "Most of the damage around your deaths was spirit damage: spirit resist and cleanses pay off most."
        } else {
            "The damage around your deaths was a mix of gun and spirit."
        };
        let top = killers.first().map(|(h, n)| format!(" {h} killed you the most ({n})."));
        let fix = counted.first().map(|(f, n)| format!(" {} would have helped in {n} of {} deaths.", f.name, deaths.len()));
        format!("{kind}{}{}", top.unwrap_or_default(), fix.unwrap_or_default())
    };
    Ok(DeathAnalysis { slot, hero, deaths, bullet_share: share, killers, fixes: counted, summary })
}

/// The player's slot in a match, by their account.
pub fn slot_of(json: &str, account_id: u64) -> Option<u32> {
    let root: Value = serde_json::from_str(json).ok()?;
    list(root.get("match_info")?, "players").iter().find(|p| p.get("account_id").and_then(Value::as_u64) == Some(account_id)).map(|p| int(p, "player_slot"))
}

/// Deaths over many matches: who kills the player, with what kind of damage, and what would have helped most often.
#[derive(Debug, Clone, Serialize)]
pub struct DeathTrend {
    pub matches: u32,
    pub deaths: u32,
    /// Share of the damage taken around the deaths that was gun damage.
    pub bullet_share: f64,
    /// Killers, most kills first: (hero, kills, matches they killed the player in).
    pub killers: Vec<(String, u32, u32)>,
    /// Deaths in which the killer's fight alone was enough, against the ones others helped in.
    pub solo_kills: u32,
    /// Fixes that helped in the most deaths, with the deaths and the matches they helped in.
    pub fixes: Vec<(Fix, u32, u32)>,
    /// Each match: id, the player's hero, deaths.
    pub per_match: Vec<(u64, String, u32)>,
    pub summary: String,
}

/// Puts the analyses of several matches together.
pub fn trend(analyses: &[(u64, DeathAnalysis)]) -> DeathTrend {
    let deaths: Vec<&DeathStudy> = analyses.iter().flat_map(|(_, a)| a.deaths.iter()).collect();
    let (mut bullet, mut all) = (0.0, 0.0);
    for s in &deaths {
        let sum: f64 = s.taken.iter().map(|h| h.amount).sum();
        all += sum;
        bullet += sum * s.bullet_share;
    }
    let share = if all > 0.0 { bullet / all } else { 0.0 };
    let mut killers: Vec<(String, u32, u32)> = vec![];
    let mut fixes: Vec<(Fix, u32, u32)> = vec![];
    for (_, a) in analyses {
        let mut seen_killers: Vec<&str> = vec![];
        let mut seen_fixes: Vec<&str> = vec![];
        for s in &a.deaths {
            match killers.iter_mut().find(|k| k.0 == s.killer_hero) {
                Some(k) => k.1 += 1,
                None => killers.push((s.killer_hero.clone(), 1, 0)),
            }
            if !seen_killers.contains(&s.killer_hero.as_str()) {
                seen_killers.push(&s.killer_hero);
                if let Some(k) = killers.iter_mut().find(|k| k.0 == s.killer_hero) {
                    k.2 += 1;
                }
            }
            for f in s.fixes.iter().filter(|f| !f.owned) {
                match fixes.iter_mut().find(|x| x.0.item == f.item) {
                    Some(x) => x.1 += 1,
                    None => fixes.push((f.clone(), 1, 0)),
                }
                if !seen_fixes.contains(&f.item.as_str()) {
                    seen_fixes.push(&f.item);
                    if let Some(x) = fixes.iter_mut().find(|x| x.0.item == f.item) {
                        x.2 += 1;
                    }
                }
            }
        }
    }
    killers.sort_by(|a, b| b.1.cmp(&a.1));
    killers.truncate(MATCH_FIXES + 2);
    fixes.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cost.cmp(&b.0.cost)));
    fixes.truncate(MATCH_FIXES);
    let solo_kills = deaths.iter().filter(|s| s.kill_time.is_some()).count() as u32;
    let matches = analyses.len() as u32;
    let summary = if deaths.is_empty() {
        "No deaths in these matches.".to_string()
    } else {
        let kind = if share >= 0.65 {
            "Most of what kills you is gun damage: bullet resist and fire-rate slows pay off most."
        } else if share <= 0.35 {
            "Most of what kills you is spirit damage: spirit resist and cleanses pay off most."
        } else {
            "What kills you is a mix of gun and spirit damage."
        };
        let killer = killers.first().map(|(h, n, m)| format!(" {h} killed you the most: {n} times in {m} matches."));
        let alone = format!(
            " In {solo_kills} of {} deaths the killer's full fight was enough on its own; in the rest someone else helped, so positioning matters as much as items.",
            deaths.len()
        );
        let fix = fixes.first().map(|(f, n, m)| format!(" {} ({} souls) would have helped in {n} deaths over {m} matches.", f.name, f.cost));
        format!("{kind}{}{alone}{}", killer.unwrap_or_default(), fix.unwrap_or_default())
    };
    DeathTrend {
        matches,
        deaths: deaths.len() as u32,
        bullet_share: share,
        killers,
        solo_kills,
        fixes,
        per_match: analyses.iter().map(|(id, a)| (*id, a.hero.clone(), a.deaths.len() as u32)).collect(),
        summary,
    }
}
