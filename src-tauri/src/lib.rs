use engine::{build::BuildInput, calc, combo, data::GameData, master, matches, popular, raw};
use std::{
    fs,
    io::{BufRead, Read},
    path::PathBuf,
    sync::RwLock,
    time::{Duration, SystemTime},
};
use tauri::{AppHandle, Manager, State};

const SNAPSHOT: &str = include_str!("../data/snapshot.json");
const API: &str = "https://api.deadlock-api.com/v1/assets";
const MATCH_API: &str = "https://api.deadlock-api.com/v1";
/// SteamID64 minus this is the account ID the API uses.
const STEAM_ID64_BASE: u64 = 76_561_197_960_265_728;

struct AppState {
    data: RwLock<GameData>,
    /// The matches found in Steam's HTTP cache, read on the first need and again when one is missing from it.
    cache: std::sync::Mutex<Option<std::collections::BTreeMap<u64, CachedMatch>>>,
}

/// What Steam's HTTP cache holds of a match: its metadata file, and where Valve serves it.
#[derive(Clone, Default)]
struct CachedMatch {
    meta_path: Option<PathBuf>,
    cluster: u32,
    meta_salt: Option<u64>,
    replay_salt: Option<u64>,
}

fn data_dir(app: &AppHandle) -> Result<PathBuf, String> {
    let dir = app.path().app_data_dir().map_err(|e| e.to_string())?;
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    Ok(dir)
}

fn builds_dir(app: &AppHandle) -> Result<PathBuf, String> {
    let dir = data_dir(app)?.join("builds");
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    Ok(dir)
}

/// Notes what changed when the game data in use is not the one used last: on a refresh after a patch, or with a
/// newer snapshot shipped in an app update. The data used last is kept to compare against; the first run has none.
fn note_patch(app: &AppHandle, data: &GameData) {
    let Ok(dir) = data_dir(app) else { return };
    let seen_path = dir.join("seen-data.json");
    let seen: Option<GameData> = fs::read_to_string(&seen_path).ok().and_then(|s| serde_json::from_str(&s).ok());
    if seen.as_ref().is_some_and(|old| old.fetched_at == data.fetched_at) {
        return;
    }
    if let Some(old) = seen {
        let notes = engine::patch::diff(&old, data);
        if !notes.changes.is_empty() {
            if let Ok(json) = serde_json::to_string(&notes) {
                let _ = fs::write(dir.join("patch-notes.json"), json);
            }
        }
    }
    if let Ok(json) = serde_json::to_string(data) {
        let _ = fs::write(seen_path, json);
    }
}

/// What the last patch changed, when one was noted.
#[tauri::command]
fn patch_notes(app: AppHandle) -> Option<engine::patch::PatchNotes> {
    let json = fs::read_to_string(data_dir(&app).ok()?.join("patch-notes.json")).ok()?;
    serde_json::from_str(&json).ok()
}

/// Whichever is newer: the downloaded data or the snapshot embedded in the app.
fn load_initial(app: &AppHandle) -> GameData {
    let embedded: GameData = serde_json::from_str(SNAPSHOT).expect("embedded snapshot is corrupt");
    let downloaded: Option<GameData> = data_dir(app)
        .ok()
        .and_then(|d| fs::read_to_string(d.join("gamedata.json")).ok())
        .and_then(|s| serde_json::from_str(&s).ok());
    // A newer app build ships a newer snapshot; an older download must not shadow it.
    match downloaded {
        Some(d) if d.fetched_at >= embedded.fetched_at => d,
        _ => embedded,
    }
}

fn fetch(path: &str) -> Result<String, String> {
    let resp = ureq::get(&format!("{API}/{path}"))
        .timeout(Duration::from_secs(90))
        .call()
        .map_err(|e| format!("failed to download {path}: {e}"))?;
    let mut body = String::new();
    resp.into_reader()
        .read_to_string(&mut body)
        .map_err(|e| format!("failed to read {path}: {e}"))?;
    Ok(body)
}

fn download() -> Result<GameData, String> {
    let version = fetch("client-versions")
        .ok()
        .and_then(|s| serde_json::from_str::<Vec<u64>>(&s).ok())
        .and_then(|v| v.into_iter().max());
    let now = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    raw::normalize(&fetch("heroes")?, &fetch("items")?, version, now)
}

#[tauri::command]
fn get_data(state: State<AppState>) -> GameData {
    state.data.read().unwrap().clone()
}

#[tauri::command]
fn compute(state: State<AppState>, build: BuildInput) -> Result<calc::Sheet, String> {
    calc::compute(&state.data.read().unwrap(), &build)
}

/// Buys, upgrades and one-for-one swaps for the build, measured for a playstyle. Off the main thread: it runs
/// a fight for every item with every owned item sold in turn.
#[tauri::command]
async fn advise_build(state: State<'_, AppState>, build: BuildInput, style: String) -> Result<engine::advisor::Advice, String> {
    let data = state.data.read().unwrap().clone();
    tauri::async_runtime::spawn_blocking(move || engine::advisor::advise(&data, &build, &style)).await.map_err(|e| e.to_string())?
}

#[tauri::command]
fn rank_next(
    state: State<AppState>,
    build: BuildInput,
    metric: String,
    conditional: bool,
) -> Result<Vec<calc::Ranked>, String> {
    calc::rank_next(&state.data.read().unwrap(), &build, &metric, conditional)
}

/// Puts builds together for the hero from the calculator's own numbers. Takes a moment, so it runs off the
/// main thread.
#[tauri::command]
async fn suggest_builds(app: AppHandle, state: State<'_, AppState>, build: BuildInput, styles: Vec<String>) -> Result<engine::suggest::Suggestions, String> {
    let data = state.data.read().unwrap().clone();
    // The souls at the end of each stage come from the matches looked at in the app, when there are any.
    let dir = data_dir(&app)?.join("matches");
    tauri::async_runtime::spawn_blocking(move || {
        let matches: Vec<String> = fs::read_dir(&dir)
            .map(|entries| {
                entries
                    .filter_map(Result::ok)
                    .map(|e| e.path())
                    .filter(|p| p.extension().is_some_and(|x| x == "json") && !p.to_string_lossy().ends_with(".names.json"))
                    .filter_map(|p| fs::read_to_string(p).ok())
                    .collect()
            })
            .unwrap_or_default();
        engine::suggest::suggest(&data, &build, &styles, engine::suggest::stage_souls(&matches))
    })
    .await
    .map_err(|e| e.to_string())?
}

/// What players build is read from this many days of matches at the top of the ladder: Eternus lobbies (average
/// badge 111 and up) when there are enough of them, Ascendant and up (101) otherwise.
const ANALYTICS_DAYS: u64 = 14;
const ETERNUS: u32 = 111;
const ASCENDANT: u32 = 101;
/// A hero needs this many matches at a rank for its build, and an enemy this many for what is bought against it.
const ENOUGH_HERO_MATCHES: u64 = 400;
const ENOUGH_ITEM_MATCHES: u64 = 1_500;
/// Answers of the analytics endpoints are kept on disk this long, or until the game is patched, so the service
/// is asked again about once a month. A player's own record changes with every match and is kept a day.
const ANALYTICS_KEEP_S: u64 = 30 * 86_400;
const OWN_RECORD_KEEP_S: u64 = 86_400;

/// Where the analytics answers are kept, and the game version they belong to.
struct Cache {
    dir: PathBuf,
    version: u64,
}

impl Cache {
    fn new(app: &AppHandle, data: &GameData) -> Result<Cache, String> {
        let dir = data_dir(app)?.join("analytics");
        // Files named the old way (by the day of their window) are never read again.
        if let Ok(entries) = fs::read_dir(&dir) {
            for e in entries.filter_map(Result::ok) {
                if !e.file_name().to_string_lossy().starts_with('v') {
                    let _ = fs::remove_file(e.path());
                }
            }
        }
        Ok(Cache { dir, version: data.client_version.unwrap_or(0) })
    }
}

/// An analytics endpoint with the shared filters, read from the cache when it is fresh enough.
fn analytics(cache: &Cache, endpoint: &str, query: &str, badge: u32) -> Result<String, String> {
    analytics_over(cache, endpoint, query, badge, ANALYTICS_DAYS)
}

/// The same over a window of the given days. A kept answer is used while it is fresh and from the current game
/// version; when the service cannot be reached, any kept answer to the same question is used instead.
fn analytics_over(cache: &Cache, endpoint: &str, query: &str, badge: u32, days: u64) -> Result<String, String> {
    let key: String = format!("{endpoint}-{query}-b{badge}-d{days}").chars().map(|c| if c.is_ascii_alphanumeric() { c } else { '_' }).collect();
    let path = cache.dir.join(format!("v{}-{key}.json", cache.version));
    let keep = if query.contains("account_id=") { OWN_RECORD_KEEP_S } else { ANALYTICS_KEEP_S };
    let fresh = fs::metadata(&path).and_then(|m| m.modified()).ok().and_then(|t| t.elapsed().ok()).is_some_and(|age| age.as_secs() < keep);
    if fresh {
        if let Ok(json) = fs::read_to_string(&path) {
            return Ok(json);
        }
    }
    // Answers to this question kept from earlier game versions.
    let older: Vec<PathBuf> = fs::read_dir(&cache.dir)
        .map(|entries| {
            entries
                .filter_map(Result::ok)
                .map(|e| e.path())
                .filter(|p| p != &path && p.file_name().is_some_and(|n| n.to_string_lossy().ends_with(&format!("-{key}.json"))))
                .collect()
        })
        .unwrap_or_default();
    let day = SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0) / 86_400;
    let from = (day - days) * 86_400;
    let filters = format!("min_unix_timestamp={from}&min_average_badge={badge}");
    let full = if query.is_empty() { filters } else { format!("{query}&{filters}") };
    match get(&format!("{MATCH_API}/analytics/{endpoint}?{full}"), "The statistics service has no data for this.") {
        Ok(json) => {
            let _ = fs::create_dir_all(&cache.dir);
            let _ = fs::write(&path, &json);
            for p in older {
                let _ = fs::remove_file(p);
            }
            Ok(json)
        }
        // The service is down or out of reach: what was kept last time is better than nothing.
        Err(e) => [path].into_iter().chain(older).find_map(|p| fs::read_to_string(p).ok()).ok_or(e),
    }
}

/// Requests to the analytics service sent at once when filling the cache ahead of time.
const PARALLEL_REQUESTS: usize = 4;

/// Fills the cache with every (endpoint, query, badge, days) asked for, several requests at a time. The service
/// can take seconds to answer a question it has not been asked lately, so one after another a master build's
/// forty requests took a minute. Failures are left for the caller's own request to report.
fn prefetch(cache: &Cache, jobs: Vec<(&'static str, String, u32, u64)>) {
    let queue = std::sync::Mutex::new(jobs);
    std::thread::scope(|scope| {
        for _ in 0..PARALLEL_REQUESTS {
            scope.spawn(|| loop {
                let Some((endpoint, query, badge, days)) = queue.lock().unwrap().pop() else { break };
                let _ = analytics_over(cache, endpoint, &query, badge, days);
            });
        }
    });
}

fn parse<T: serde::de::DeserializeOwned>(json: &str) -> Result<T, String> {
    serde_json::from_str(json).map_err(|e| format!("The statistics service answered in an unexpected form: {e}"))
}

/// A hero's popular build: what top players bought and kept over the last two weeks, and the rank it was read at.
/// The ranks a hero's build is read at, in turn: the next is tried when a hero has too few matches in one. At
/// the top of the ladder that is Eternus, then Ascendant and up; a range the player chose is read as it is.
fn popular_for(data: &GameData, cache: &Cache, hero_id: u64, ladder: &[Range]) -> Result<(popular::PopularBuild, usize), String> {
    for (at, range) in ladder.iter().enumerate() {
        let heroes: Vec<popular::HeroStat> = parse(&ranked(cache, "hero-stats", "", *range)?)?;
        let matches = heroes.iter().find(|h| h.hero_id == hero_id).map(|h| h.matches).unwrap_or(0);
        if matches < ENOUGH_HERO_MATCHES && at + 1 < ladder.len() {
            continue;
        }
        if matches == 0 {
            return Err("No recent matches of this hero in these ranks.".into());
        }
        let stats: Vec<popular::ItemStat> = parse(&ranked(cache, "item-stats", &format!("hero_id={hero_id}"), *range)?)?;
        return popular::popular_build(data, hero_id, matches, &stats).map(|b| (b, at)).ok_or_else(|| "Unknown hero.".to_string());
    }
    Err("No recent matches of this hero in these ranks.".into())
}

/// A rank range: the lowest average badge of a match, and the highest when there is one.
type Range = (u32, Option<u32>);

/// The ranks builds are read at: the player's range, or the top of the ladder when none is set.
fn ladder(min_badge: Option<u32>, max_badge: Option<u32>) -> Vec<Range> {
    match min_badge {
        Some(min) => vec![(min, max_badge)],
        None => vec![(ETERNUS, None), (ASCENDANT, None)],
    }
}

/// A query with the highest rank added, when there is one.
fn with_max(query: &str, max: Option<u32>) -> String {
    match max {
        Some(max) if query.is_empty() => format!("max_average_badge={max}"),
        Some(max) => format!("{query}&max_average_badge={max}"),
        None => query.to_string(),
    }
}

/// An analytics endpoint over a rank range, from the shared window.
fn ranked(cache: &Cache, endpoint: &str, query: &str, range: Range) -> Result<String, String> {
    analytics(cache, endpoint, &with_max(query, range.1), range.0)
}

/// The game's rank names, by badge tier (badge = tier × 10 + subrank).
const TIER_NAMES: [&str; 11] = ["Initiate", "Seeker", "Acolyte", "Sentinel", "Mystic", "Ritualist", "Emissary", "Oracle", "Phantom", "Ascendant", "Eternus"];

fn tier_name(badge: u32) -> Option<&'static str> {
    TIER_NAMES.get((badge / 10).checked_sub(1)? as usize).copied()
}

fn rank_name((min, max): Range) -> String {
    match (tier_name(min), max.and_then(tier_name)) {
        (Some("Eternus"), None) => "Eternus".into(),
        (Some(a), Some(b)) if a == b => a.into(),
        (Some(a), Some(b)) => format!("{a}–{b}"),
        (Some(a), None) => format!("{a} and above"),
        (None, Some(b)) => format!("up to {b}"),
        (None, None) => "every rank".into(),
    }
}

#[derive(serde::Serialize)]
struct MasterReport {
    master: master::MasterBuild,
    /// What each enemy usually builds.
    builds: Vec<popular::PopularBuild>,
    plan: popular::CounterPlan,
    days: u64,
    /// The rank the enemy builds and what is bought against them were read at.
    rank: String,
}

#[derive(serde::Serialize)]
struct PopularReport {
    build: popular::PopularBuild,
    days: u64,
    rank: String,
}

#[tauri::command]
async fn popular_build(
    app: AppHandle,
    state: State<'_, AppState>,
    hero_id: u64,
    min_badge: Option<u32>,
    max_badge: Option<u32>,
) -> Result<PopularReport, String> {
    let data = state.data.read().unwrap().clone();
    let cache = Cache::new(&app, &data)?;
    tauri::async_runtime::spawn_blocking(move || {
        let ladder = ladder(min_badge, max_badge);
        let (build, at) = popular_for(&data, &cache, hero_id, &ladder)?;
        Ok(PopularReport { build, days: ANALYTICS_DAYS, rank: rank_name(ladder[at]) })
    })
    .await
    .map_err(|e| e.to_string())?
}

/// The draft helper reads matchups from this many days: the window its weights were fitted with.
const DRAFT_DAYS: u64 = 30;
/// A player's own record on each hero comes from this many days of their matches, at any rank.
const OWN_RECORD_DAYS: u64 = 90;

/// Picks, bans and the draft's chance for the heroes picked so far, at a rank (lowest average badge).
#[tauri::command]
#[allow(clippy::too_many_arguments)]
async fn draft_picks(
    app: AppHandle,
    state: State<'_, AppState>,
    allies: Vec<u64>,
    enemies: Vec<u64>,
    lane: Vec<u64>,
    banned: Vec<u64>,
    badge: u32,
    max_badge: Option<u32>,
    account_id: Option<String>,
) -> Result<engine::draft::DraftReport, String> {
    let data = state.data.read().unwrap().clone();
    let cache = Cache::new(&app, &data)?;
    // A rank range: from `badge` up, and up to `max_badge` when one is given.
    let upto = move |query: &str| with_max(query, max_badge);
    tauri::async_runtime::spawn_blocking(move || {
        prefetch(
            &cache,
            vec![
                ("hero-stats", upto(""), badge, DRAFT_DAYS),
                ("hero-counter-stats", upto("same_lane_filter=false"), badge, DRAFT_DAYS),
                ("hero-counter-stats", upto("same_lane_filter=true"), badge, DRAFT_DAYS),
                ("hero-ban-stats", upto(""), badge, DRAFT_DAYS),
                ("hero-synergy-stats", upto("same_lane_filter=false"), badge, DRAFT_DAYS),
            ],
        );
        let read = |endpoint: &str, query: &str| analytics_over(&cache, endpoint, &upto(query), badge, DRAFT_DAYS);
        let account = match account_id.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
            Some(text) => {
                let id: u64 = text.parse().map_err(|_| "Enter a Steam account ID (numbers only)".to_string())?;
                Some(if id > STEAM_ID64_BASE { id - STEAM_ID64_BASE } else { id })
            }
            None => None,
        };
        let mine = match account {
            Some(id) => parse(&analytics_over(&cache, "hero-stats", &format!("account_id={id}"), 0, OWN_RECORD_DAYS)?)?,
            None => vec![],
        };
        let stats = engine::draft::DraftData {
            heroes: parse(&read("hero-stats", "")?)?,
            counters: engine::draft::parse_counters(&read("hero-counter-stats", "same_lane_filter=false")?)?,
            lane_counters: engine::draft::parse_counters(&read("hero-counter-stats", "same_lane_filter=true")?)?,
            synergies: engine::draft::parse_synergies(&read("hero-synergy-stats", "same_lane_filter=false")?)?,
            mine,
            bans: engine::draft::parse_bans(&read("hero-ban-stats", "")?)?,
        };
        let input = engine::draft::DraftInput { allies, enemies, lane, banned };
        Ok(engine::draft::draft(&data, &stats, &input))
    })
    .await
    .map_err(|e| e.to_string())?
}

/// The heroes ranked at a rank range, from the same statistics as the draft (so neither asks the service twice).
#[tauri::command]
async fn hero_tiers(app: AppHandle, state: State<'_, AppState>, badge: u32, max_badge: Option<u32>) -> Result<Vec<engine::tiers::HeroTier>, String> {
    let data = state.data.read().unwrap().clone();
    let cache = Cache::new(&app, &data)?;
    tauri::async_runtime::spawn_blocking(move || {
        prefetch(&cache, vec![("hero-stats", with_max("", max_badge), badge, DRAFT_DAYS), ("hero-ban-stats", with_max("", max_badge), badge, DRAFT_DAYS)]);
        let rows: Vec<engine::tiers::HeroRow> = parse(&analytics_over(&cache, "hero-stats", &with_max("", max_badge), badge, DRAFT_DAYS)?)?;
        let bans = engine::draft::parse_bans(&analytics_over(&cache, "hero-ban-stats", &with_max("", max_badge), badge, DRAFT_DAYS)?)?;
        Ok(engine::tiers::tiers(&data, &rows, &bans))
    })
    .await
    .map_err(|e| e.to_string())?
}

#[derive(serde::Serialize)]
struct HeroPage {
    tier: Option<engine::tiers::HeroTier>,
    /// How the hero fares against each other hero, and beside it, the best first.
    against: Vec<engine::draft::Fit>,
    with: Vec<engine::draft::Fit>,
}

/// One hero at a rank range: its line of the tier list, its matchups and its partners. Read from the draft's
/// statistics, so it shares their cache.
#[tauri::command]
async fn hero_page(app: AppHandle, state: State<'_, AppState>, hero_id: u64, badge: u32, max_badge: Option<u32>) -> Result<HeroPage, String> {
    let data = state.data.read().unwrap().clone();
    let cache = Cache::new(&app, &data)?;
    tauri::async_runtime::spawn_blocking(move || {
        let upto = |query: &str| with_max(query, max_badge);
        prefetch(
            &cache,
            vec![
                ("hero-stats", upto(""), badge, DRAFT_DAYS),
                ("hero-ban-stats", upto(""), badge, DRAFT_DAYS),
                ("hero-counter-stats", upto("same_lane_filter=false"), badge, DRAFT_DAYS),
                ("hero-synergy-stats", upto("same_lane_filter=false"), badge, DRAFT_DAYS),
            ],
        );
        let read = |endpoint: &str, query: &str| analytics_over(&cache, endpoint, &upto(query), badge, DRAFT_DAYS);
        let hero_json = read("hero-stats", "")?;
        let rows: Vec<engine::tiers::HeroRow> = parse(&hero_json)?;
        let bans = engine::draft::parse_bans(&read("hero-ban-stats", "")?)?;
        let stats = engine::draft::DraftData {
            heroes: parse(&hero_json)?,
            counters: engine::draft::parse_counters(&read("hero-counter-stats", "same_lane_filter=false")?)?,
            synergies: engine::draft::parse_synergies(&read("hero-synergy-stats", "same_lane_filter=false")?)?,
            ..Default::default()
        };
        let tier = engine::tiers::tiers(&data, &rows, &bans).into_iter().find(|h| h.hero_id == hero_id);
        let (against, with) = engine::draft::matchups(&data, &stats, hero_id);
        Ok(HeroPage { tier, against, with })
    })
    .await
    .map_err(|e| e.to_string())?
}

/// The shop's items ranked at a rank range, for every hero or for one.
#[tauri::command]
async fn item_tiers(app: AppHandle, state: State<'_, AppState>, badge: u32, max_badge: Option<u32>, hero_id: Option<u64>) -> Result<Vec<engine::tiers::ItemTier>, String> {
    let data = state.data.read().unwrap().clone();
    let cache = Cache::new(&app, &data)?;
    tauri::async_runtime::spawn_blocking(move || {
        let items_query = with_max(&hero_id.map(|h| format!("hero_id={h}")).unwrap_or_default(), max_badge);
        prefetch(&cache, vec![("hero-stats", with_max("", max_badge), badge, DRAFT_DAYS), ("item-stats", items_query.clone(), badge, DRAFT_DAYS)]);
        let heroes: Vec<popular::HeroStat> = parse(&analytics_over(&cache, "hero-stats", &with_max("", max_badge), badge, DRAFT_DAYS)?)?;
        let players = match hero_id {
            Some(h) => heroes.iter().filter(|r| r.hero_id == h).map(|r| r.matches).sum(),
            None => heroes.iter().map(|r| r.matches).sum(),
        };
        let rows: Vec<popular::ItemStat> = parse(&analytics_over(&cache, "item-stats", &items_query, badge, DRAFT_DAYS)?)?;
        Ok(engine::tiers::item_tiers(&data, &rows, players))
    })
    .await
    .map_err(|e| e.to_string())?
}

/// The days compared around a patch: up to this many before it and after it.
const META_DAYS: u64 = 14;
/// The patch list is read again after this long.
const PATCHES_KEEP_S: u64 = 86_400;
/// An answer about days that are over never changes; one about days still running is read again after this long.
const META_OPEN_KEEP_S: u64 = 6 * 3600;

#[derive(serde::Serialize)]
struct MetaReport {
    patches: Vec<engine::meta::PatchDay>,
    /// The patch compared, and the days each window covers.
    patch: engine::meta::PatchDay,
    days_before: u64,
    days_after: u64,
    shift: engine::meta::MetaShift,
}

/// An analytics endpoint between two times at a rank range, kept on this computer.
fn analytics_between(app: &AppHandle, endpoint: &str, badge: u32, max_badge: Option<u32>, from: u64, to: u64, now: u64) -> Result<String, String> {
    let query = with_max(&format!("min_unix_timestamp={from}&max_unix_timestamp={to}&min_average_badge={badge}"), max_badge);
    let key: String = format!("{endpoint}-{query}").chars().map(|c| if c.is_ascii_alphanumeric() { c } else { '_' }).collect();
    let path = data_dir(app)?.join("meta").join(format!("{key}.json"));
    let keep = if to < now { u64::MAX } else { META_OPEN_KEEP_S };
    kept(&path, keep, || get(&format!("{MATCH_API}/analytics/{endpoint}?{query}"), "The statistics service has no data for these days."))
}

/// Every hero's and item's win and pick rate before a patch beside after it, at a rank range. `patch` picks the
/// patch by its day; the newest is taken otherwise. Five requests the first time; days that are over are kept.
#[tauri::command]
async fn meta_shift(app: AppHandle, state: State<'_, AppState>, patch: Option<u64>, badge: u32, max_badge: Option<u32>) -> Result<MetaReport, String> {
    let data = state.data.read().unwrap().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let now = SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
        let patches_path = data_dir(&app)?.join("meta").join("patches.json");
        let patches = engine::meta::patch_days(&kept(&patches_path, PATCHES_KEEP_S, || get(&format!("{MATCH_API}/patches"), "No patch notes found."))?);
        // The newest patch with at least a day after it, unless one is asked for.
        let chosen = match patch {
            Some(day) => patches.iter().find(|p| p.day == day).cloned(),
            None => patches.iter().find(|p| p.day + 2 * 86_400 <= now).cloned(),
        }
        .ok_or("No patch found to compare.")?;
        // The patch day itself is left out: the patch came out some time in it. The window before stops at the
        // patch before, so it holds one version of the game.
        let previous = patches.iter().find(|p| p.day < chosen.day).map(|p| p.day + 86_400).unwrap_or(0);
        let before_from = chosen.day.saturating_sub(META_DAYS * 86_400).max(previous);
        let next = patches.iter().filter(|p| p.day > chosen.day).map(|p| p.day).min().unwrap_or(u64::MAX);
        let after_from = chosen.day + 86_400;
        let after_to = (after_from + META_DAYS * 86_400).min(next).min(now);
        if after_to <= after_from {
            return Err("The patch is too recent: there is not a full day of matches after it yet.".into());
        }
        let read = |endpoint: &str, from: u64, to: u64| analytics_between(&app, endpoint, badge, max_badge, from, to, now);
        let heroes_before: Vec<engine::tiers::HeroRow> = parse(&read("hero-stats", before_from, chosen.day)?)?;
        let heroes_after: Vec<engine::tiers::HeroRow> = parse(&read("hero-stats", after_from, after_to)?)?;
        let items_before: Vec<popular::ItemStat> = parse(&read("item-stats", before_from, chosen.day)?)?;
        let items_after: Vec<popular::ItemStat> = parse(&read("item-stats", after_from, after_to)?)?;
        Ok(MetaReport {
            days_before: (chosen.day - before_from) / 86_400,
            days_after: (after_to - after_from).div_ceil(86_400),
            shift: engine::meta::shift(&data, &heroes_before, &heroes_after, &items_before, &items_after),
            patches,
            patch: chosen,
        })
    })
    .await
    .map_err(|e| e.to_string())?
}

#[derive(serde::Serialize)]
struct MyHeroes {
    /// The player's rank badge (0 when unranked), and the ranks the averages come from.
    badge: u32,
    min_badge: u32,
    max_badge: Option<u32>,
    heroes: Vec<engine::tiers::Mastery>,
}

/// The player's heroes beside the average player of their rank: the rank's own tier when the player has a rank,
/// the ranks given otherwise.
#[tauri::command]
async fn my_heroes(app: AppHandle, state: State<'_, AppState>, account_id: String, badge: u32, max_badge: Option<u32>) -> Result<MyHeroes, String> {
    let id = steam_account(&account_id)?;
    let data = state.data.read().unwrap().clone();
    let cache = Cache::new(&app, &data)?;
    tauri::async_runtime::spawn_blocking(move || {
        let rank: serde_json::Value = get(&format!("{MATCH_API}/players/{id}/rank"), "").ok().and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default();
        let own = rank.get("badge").and_then(serde_json::Value::as_u64).unwrap_or(0) as u32;
        let (min, max) = if own >= 10 { (own / 10 * 10 + 1, Some(own / 10 * 10 + 6)) } else { (badge, max_badge) };
        let mine: Vec<engine::tiers::MyHeroRow> = parse(&get(&format!("{MATCH_API}/players/hero-stats?account_ids={id}"), "No matches found for this account.")?)?;
        let average: Vec<engine::tiers::HeroRow> = parse(&analytics_over(&cache, "hero-stats", &with_max("", max), min, DRAFT_DAYS)?)?;
        Ok(MyHeroes { badge: own, min_badge: min, max_badge: max, heroes: engine::tiers::mastery(&data, &mine, &average) })
    })
    .await
    .map_err(|e| e.to_string())?
}

/// The note for the player's lane at the start of a match: how the player's hero does against each lane enemy, when
/// that enemy is strong, and what is bought against it. Read at the draft's ranks, from cached statistics.
#[tauri::command]
async fn lane_note(app: AppHandle, state: State<'_, AppState>, hero_id: u64, lane: Vec<u64>, badge: u32, max_badge: Option<u32>) -> Result<Vec<engine::tiers::LaneEnemy>, String> {
    let data = state.data.read().unwrap().clone();
    let cache = Cache::new(&app, &data)?;
    tauri::async_runtime::spawn_blocking(move || {
        let upto = |query: &str| with_max(query, max_badge);
        let mut jobs = vec![
            ("hero-stats", upto(""), badge, DRAFT_DAYS),
            ("hero-counter-stats", upto("same_lane_filter=false"), badge, DRAFT_DAYS),
            ("hero-counter-stats", upto("same_lane_filter=true"), badge, DRAFT_DAYS),
            ("hero-stats", upto(&format!("min_duration_s={LONG_MATCH_S}")), badge, DRAFT_DAYS),
            ("hero-stats", upto(&format!("max_duration_s={SHORT_MATCH_S}")), badge, DRAFT_DAYS),
            ("item-stats", upto(""), badge, DRAFT_DAYS),
        ];
        for id in &lane {
            jobs.push(("item-stats", upto(&format!("enemy_hero_ids={id}")), badge, DRAFT_DAYS));
        }
        prefetch(&cache, jobs);
        let read = |endpoint: &str, query: &str| analytics_over(&cache, endpoint, &upto(query), badge, DRAFT_DAYS);
        let stats = engine::draft::DraftData {
            heroes: parse(&read("hero-stats", "")?)?,
            counters: engine::draft::parse_counters(&read("hero-counter-stats", "same_lane_filter=false")?)?,
            lane_counters: engine::draft::parse_counters(&read("hero-counter-stats", "same_lane_filter=true")?)?,
            ..Default::default()
        };
        let long: Vec<popular::HeroStat> = parse(&read("hero-stats", &format!("min_duration_s={LONG_MATCH_S}"))?)?;
        let short: Vec<popular::HeroStat> = parse(&read("hero-stats", &format!("max_duration_s={SHORT_MATCH_S}"))?)?;
        let rate = |rows: &[popular::HeroStat], id: u64| {
            rows.iter().find(|h| h.hero_id == id).filter(|h| h.matches >= ENOUGH_SCALING_MATCHES).map(|h| h.wins as f64 / h.matches as f64 * 100.0)
        };
        let general: Vec<popular::ItemStat> = parse(&read("item-stats", "")?)?;
        lane.iter()
            .filter_map(|id| data.heroes.iter().find(|h| h.id == *id))
            .map(|h| {
                let (overall, lane_fit) = engine::draft::matchup(&stats, hero_id, h.id);
                let against: Vec<popular::ItemStat> = parse(&read("item-stats", &format!("enemy_hero_ids={}", h.id))?)?;
                Ok(engine::tiers::LaneEnemy {
                    hero_id: h.id,
                    name: h.name.clone(),
                    icon: h.icon.clone(),
                    lane: lane_fit,
                    overall,
                    late: rate(&long, h.id).zip(rate(&short, h.id)).map(|(l, s)| l - s),
                    counters: engine::tiers::counter_items(&data, &popular::lifts(&general, &against)),
                })
            })
            .collect()
    })
    .await
    .map_err(|e| e.to_string())?
}

/// The heroes the player plays: the most played of those played in the last 60 days, by class name.
#[tauri::command]
async fn played_heroes(state: State<'_, AppState>, account_id: String) -> Result<Vec<String>, String> {
    const RECENT_S: u64 = 60 * 86_400;
    const AT_LEAST: u64 = 3;
    const MOST: usize = 8;
    let id = steam_account(&account_id)?;
    let data = state.data.read().unwrap().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let rows: Vec<serde_json::Value> = parse(&get(&format!("{MATCH_API}/players/hero-stats?account_ids={id}"), "No matches found for this account.")?)?;
        let now = SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
        let num = |r: &serde_json::Value, k: &str| r.get(k).and_then(serde_json::Value::as_u64).unwrap_or(0);
        let mut played: Vec<&serde_json::Value> =
            rows.iter().filter(|r| num(r, "matches_played") >= AT_LEAST && now.saturating_sub(num(r, "last_played")) <= RECENT_S).collect();
        played.sort_by_key(|r| std::cmp::Reverse(num(r, "matches_played")));
        Ok(played.iter().take(MOST).filter_map(|r| data.heroes.iter().find(|h| h.id == num(r, "hero_id")).map(|h| h.class_name.clone())).collect())
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Matches this long are long ones, and this short short ones, when telling which heroes grow strong late.
const LONG_MATCH_S: u32 = 35 * 60;
const SHORT_MATCH_S: u32 = 25 * 60;
/// A hero needs this many long and short matches for the comparison.
const ENOUGH_SCALING_MATCHES: u64 = 200;

/// A full build for the player's hero against an enemy team and alongside the player's own team.
#[tauri::command]
#[allow(clippy::too_many_arguments)]
async fn master_build(
    app: AppHandle,
    state: State<'_, AppState>,
    my_hero: String,
    allies: Vec<u64>,
    enemies: Vec<u64>,
    lane: Vec<u64>,
    focus: String,
    damage: String,
    bought: Option<Vec<(String, u32)>>,
    min_badge: Option<u32>,
    max_badge: Option<u32>,
) -> Result<MasterReport, String> {
    let data = state.data.read().unwrap().clone();
    let cache = Cache::new(&app, &data)?;
    tauri::async_runtime::spawn_blocking(move || {
        let my_id = data.hero(&my_hero).map(|h| h.id).ok_or("Unknown hero")?;
        // The same hero cannot be on both teams.
        let enemies: Vec<u64> = enemies.into_iter().filter(|id| *id != my_id && !allies.contains(id)).collect();
        let allies: Vec<u64> = allies.into_iter().filter(|id| *id != my_id).collect();
        let ladder = ladder(min_badge, max_badge);
        // The widest ranks of the ladder, for the statistics spread too thin at the top alone.
        let wide = *ladder.last().unwrap();
        // Everything the build below may read, fetched at once; it then reads it from the cache. Every rank of the
        // ladder is fetched, as which one a hero is read at is only known from the answers.
        let job = |endpoint: &'static str, query: String, (min, max): Range| (endpoint, with_max(&query, max), min, ANALYTICS_DAYS);
        let mut jobs = vec![
            job("item-stats", format!("hero_id={my_id}&bucket=net_worth_by_1000"), wide),
            job("hero-stats", format!("min_duration_s={LONG_MATCH_S}"), wide),
            job("hero-stats", format!("max_duration_s={SHORT_MATCH_S}"), wide),
        ];
        for range in &ladder {
            jobs.push(job("hero-stats", String::new(), *range));
            jobs.push(job("item-stats", String::new(), *range));
            for id in allies.iter().chain(&enemies).chain([&my_id]) {
                jobs.push(job("item-stats", format!("hero_id={id}"), *range));
            }
            for id in &enemies {
                jobs.push(job("item-stats", format!("enemy_hero_ids={id}"), *range));
            }
        }
        prefetch(&cache, jobs);
        let ally_builds: Vec<popular::PopularBuild> = allies.iter().map(|id| popular_for(&data, &cache, *id, &ladder).map(|b| b.0)).collect::<Result<_, _>>()?;
        let ally_team: Vec<popular::Ally> = allies.iter().zip(&ally_builds).map(|(id, build)| popular::Ally { hero_id: *id, build }).collect();
        // What everybody buys against an enemy: one hero's own sample at the top ranks is too small to read.
        // The widest step of the ladder anything was read at.
        let mut lowest = 0;
        let mut builds = vec![];
        let mut lifts = vec![];
        for id in &enemies {
            let (build, at) = popular_for(&data, &cache, *id, &ladder)?;
            lowest = lowest.max(at);
            builds.push(build);
            let mut found = vec![];
            for (at, range) in ladder.iter().enumerate() {
                let general: Vec<popular::ItemStat> = parse(&ranked(&cache, "item-stats", "", *range)?)?;
                let against: Vec<popular::ItemStat> = parse(&ranked(&cache, "item-stats", &format!("enemy_hero_ids={id}"), *range)?)?;
                let sample = against.iter().map(|r| r.matches).max().unwrap_or(0);
                found = popular::lifts(&general, &against);
                if sample >= ENOUGH_ITEM_MATCHES || at + 1 == ladder.len() {
                    lowest = lowest.max(at);
                    break;
                }
            }
            lifts.push(found);
        }
        let team: Vec<popular::Enemy> =
            enemies.iter().zip(&builds).zip(&lifts).map(|((id, build), lifts)| popular::Enemy { hero_id: *id, build, lifts }).collect();

        // The player's hero: its usual build, and what its top players buy by their souls at the time.
        let (mine, at) = popular_for(&data, &cache, my_id, &ladder)?;
        lowest = lowest.max(at);
        let my_spirit = Some(popular::deals_spirit(&data, &mine));
        let my_core: Vec<String> = mine.core.iter().map(|c| c.item.clone()).collect();
        // Purchases by souls are spread over many phases: Eternus alone has too few late ones, so these come from
        // the widest ranks.
        let heroes: Vec<popular::HeroStat> = parse(&ranked(&cache, "hero-stats", "", wide)?)?;
        let hero_matches = heroes.iter().find(|h| h.hero_id == my_id).map(|h| h.matches).unwrap_or(0);
        let phase_stats: Vec<popular::ItemStat> =
            parse(&ranked(&cache, "item-stats", &format!("hero_id={my_id}&bucket=net_worth_by_1000"), wide)?)?;

        // Which heroes grow strong late: their win rate in long matches against short ones.
        let long: Vec<popular::HeroStat> = parse(&ranked(&cache, "hero-stats", &format!("min_duration_s={LONG_MATCH_S}"), wide)?)?;
        let short: Vec<popular::HeroStat> = parse(&ranked(&cache, "hero-stats", &format!("max_duration_s={SHORT_MATCH_S}"), wide)?)?;
        let rate = |rows: &[popular::HeroStat], id: u64| {
            rows.iter().find(|h| h.hero_id == id).filter(|h| h.matches >= ENOUGH_SCALING_MATCHES).map(|h| h.wins as f64 / h.matches as f64 * 100.0)
        };
        let scaling: master::Scaling = data.heroes.iter().filter_map(|h| Some((h.id, rate(&long, h.id)? - rate(&short, h.id)?))).collect();

        let input = master::MasterInput {
            matchup: popular::Matchup {
                my_hero: Some(&my_hero),
                my_spirit,
                my_gun_pct: popular::gun_pct(&data, &mine),
                my_core: &my_core,
                allies: &ally_team,
                enemies: &team,
                lane: &lane,
                ranged: !(focus == "frontline" || damage == "melee"),
            },
            hero_id: my_id,
            phase_stats: &phase_stats,
            hero_matches,
            own_build: &mine,
            focus: &focus,
            damage: &damage,
            scaling: &scaling,
            bought: bought.as_deref().unwrap_or(&[]),
        };
        let master = master::master_build(&data, &input)?;
        let plan = master.counter.clone();
        Ok(MasterReport { master, plan, builds: builds.clone(), days: ANALYTICS_DAYS, rank: rank_name(ladder[lowest]) })
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
fn rift_comeback(minute: f64, own_souls: f64, enemy_souls: f64) -> engine::misc::RiftComeback {
    engine::misc::rift_comeback(minute, own_souls, enemy_souls)
}

#[tauri::command]
fn simulate_combo(
    state: State<AppState>,
    build: BuildInput,
    combo: combo::ComboInput,
) -> Result<combo::ComboResult, String> {
    combo::simulate(&state.data.read().unwrap(), &build, &combo)
}

/// The build's strength at each point of a master build, for the power curve.
#[tauri::command]
async fn power_curve(state: State<'_, AppState>, builds: Vec<BuildInput>) -> Result<Vec<Option<engine::suggest::PowerPoint>>, String> {
    let data = state.data.read().unwrap().clone();
    tauri::async_runtime::spawn_blocking(move || builds.iter().map(|b| engine::suggest::power(&data, b)).collect())
        .await
        .map_err(|e| e.to_string())
}

/// How fast the build clears each jungle camp at a minute of the match.
#[tauri::command]
fn farm_report(state: State<'_, AppState>, build: BuildInput, minute: f64) -> Result<engine::farm::FarmReport, String> {
    engine::farm::farm(&state.data.read().unwrap(), &build, minute)
}

#[tauri::command]
async fn refresh_data(app: AppHandle, state: State<'_, AppState>) -> Result<GameData, String> {
    let data = tauri::async_runtime::spawn_blocking(download)
        .await
        .map_err(|e| e.to_string())??;
    let json = serde_json::to_string(&data).map_err(|e| e.to_string())?;
    fs::write(data_dir(&app)?.join("gamedata.json"), json).map_err(|e| e.to_string())?;
    note_patch(&app, &data);
    *state.data.write().unwrap() = data.clone();
    Ok(data)
}

fn get(url: &str, missing: &str) -> Result<String, String> {
    let resp = match ureq::get(url).timeout(Duration::from_secs(90)).call() {
        Ok(resp) => resp,
        Err(ureq::Error::Status(404, _)) => return Err(missing.to_string()),
        Err(ureq::Error::Status(429, _)) => return Err("The match service is rate limiting requests. Try again in a moment.".into()),
        Err(e) => return Err(format!("Download failed: {e}")),
    };
    let mut body = String::new();
    resp.into_reader().read_to_string(&mut body).map_err(|e| format!("Download failed: {e}"))?;
    Ok(body)
}

/// A match's metadata: the copy saved earlier, or a fresh download that is then saved.
async fn match_json(app: &AppHandle, match_id: u64) -> Result<String, String> {
    let dir = data_dir(app)?.join("matches");
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let path = dir.join(format!("{match_id}.json"));
    if let Ok(json) = fs::read_to_string(&path) {
        return Ok(json);
    }
    // Opened in the game: the file is in Steam's cache, or at least where Valve serves it.
    let state = app.state::<AppState>();
    let known = state.cache.lock().unwrap().as_ref().and_then(|c| c.get(&match_id).cloned());
    let cached = match known {
        Some(c) => Some(c),
        None => {
            let scanned = tauri::async_runtime::spawn_blocking(|| cache_folder().ok().map(|f| scan_cache(&f).2)).await.map_err(|e| e.to_string())?;
            let found = scanned.as_ref().and_then(|c| c.get(&match_id).cloned());
            if scanned.is_some() {
                *state.cache.lock().unwrap() = scanned;
            }
            found
        }
    };
    if let Some(cached) = cached {
        let from_valve = tauri::async_runtime::spawn_blocking(move || -> Result<String, String> {
            if let Some(json) = cached.meta_path.as_ref().and_then(|p| fs::read(p).ok()).and_then(|f| engine::cached_meta::metadata_json(&f).ok()) {
                return Ok(json);
            }
            let (Some(salt), cluster) = (cached.meta_salt, cached.cluster) else { return Err(String::new()) };
            if cluster == 0 {
                return Err(String::new());
            }
            // Valve serves the file to anyone who knows its salt; no sign-in.
            let url = format!("http://replay{cluster}.valve.net/1422450/{match_id}_{salt}.meta.bz2");
            let resp = ureq::get(&url).timeout(Duration::from_secs(60)).call().map_err(|e| e.to_string())?;
            let mut bytes = vec![];
            resp.into_reader().read_to_end(&mut bytes).map_err(|e| e.to_string())?;
            engine::cached_meta::metadata_json(&bytes)
        })
        .await
        .map_err(|e| e.to_string())?;
        if let Ok(json) = from_valve {
            let _ = fs::write(&path, &json);
            return Ok(json);
        }
    }
    let url = format!("{MATCH_API}/matches/{match_id}/metadata");
    let json = tauri::async_runtime::spawn_blocking(move || {
        get(&url, "Match not found. Very recent matches can take a while to become available.")
    })
    .await
    .map_err(|e| e.to_string())??;
    // A finished match never changes, so the copy is kept for next time.
    let _ = fs::write(&path, &json);
    Ok(json)
}

/// Why a player in a match died, death by death, and what would have helped.
#[tauri::command]
async fn death_analysis(app: AppHandle, state: State<'_, AppState>, match_id: u64, slot: u32) -> Result<engine::deaths::DeathAnalysis, String> {
    let json = match_json(&app, match_id).await?;
    engine::deaths::analyze(&state.data.read().unwrap(), &json, slot)
}

#[derive(serde::Serialize)]
struct Trend {
    rows: Vec<matches::MatchSummary>,
    /// Matches of the history that could not be loaded or read.
    skipped: usize,
}

/// Analyzes a player's most recent matches and returns one row per match for that player.
#[tauri::command]
async fn analyze_history(app: AppHandle, state: State<'_, AppState>, account_id: String, count: usize) -> Result<Trend, String> {
    let id: u64 = account_id.trim().parse().map_err(|_| "Enter a Steam account ID (numbers only)".to_string())?;
    let id = if id > STEAM_ID64_BASE { id - STEAM_ID64_BASE } else { id };
    let url = format!("{MATCH_API}/players/{id}/match-history");
    let json = tauri::async_runtime::spawn_blocking(move || get(&url, "No match history found for this account."))
        .await
        .map_err(|e| e.to_string())??;
    let history = matches::parse_history(&state.data.read().unwrap(), &json, count.clamp(1, 25))?;
    let (mut rows, mut skipped) = (vec![], 0);
    for entry in history {
        // One at a time: the match service limits how fast it can be asked.
        let row = match match_json(&app, entry.match_id).await {
            Ok(json) => matches::parse(&state.data.read().unwrap(), &json).ok().and_then(|report| matches::summarize(&report, id)),
            Err(_) => None,
        };
        match row {
            Some(row) => rows.push(row),
            None => skipped += 1,
        }
    }
    Ok(Trend { rows, skipped })
}

/// A Steam account ID as typed: the long SteamID64 or the short account ID.
fn steam_account(text: &str) -> Result<u64, String> {
    let id: u64 = text.trim().parse().map_err(|_| "Enter a Steam account ID (numbers only)".to_string())?;
    Ok(if id > STEAM_ID64_BASE { id - STEAM_ID64_BASE } else { id })
}

/// Why a player keeps dying: the deaths of their last matches put together.
#[tauri::command]
async fn death_trend(app: AppHandle, state: State<'_, AppState>, account_id: String, count: usize) -> Result<engine::deaths::DeathTrend, String> {
    let id = steam_account(&account_id)?;
    let url = format!("{MATCH_API}/players/{id}/match-history");
    let json = tauri::async_runtime::spawn_blocking(move || get(&url, "No match history found for this account."))
        .await
        .map_err(|e| e.to_string())??;
    let data = std::sync::Arc::new(state.data.read().unwrap().clone());
    let history = matches::parse_history(&data, &json, count.clamp(1, 20))?;
    let mut analyses = vec![];
    for entry in history {
        // One at a time: the match service limits how fast it can be asked; matches seen before are on disk.
        let Ok(json) = match_json(&app, entry.match_id).await else { continue };
        let data = data.clone();
        let found = tauri::async_runtime::spawn_blocking(move || {
            let slot = engine::deaths::slot_of(&json, id)?;
            engine::deaths::analyze(&data, &json, slot).ok()
        })
        .await
        .map_err(|e| e.to_string())?;
        if let Some(a) = found {
            analyses.push((entry.match_id, a));
        }
    }
    if analyses.is_empty() {
        return Err("None of the recent matches could be read.".into());
    }
    Ok(engine::deaths::trend(&analyses))
}

/// Where Deadlock's console log is: every Steam library's Deadlock folder. The game writes it when it is started
/// with the -condebug launch option.
fn console_logs() -> Vec<PathBuf> {
    let mut libraries: Vec<PathBuf> = vec![];
    for steam in steam_folders() {
        libraries.push(steam.clone());
        if let Ok(text) = fs::read_to_string(steam.join("steamapps").join("libraryfolders.vdf")) {
            for line in text.lines() {
                let parts: Vec<&str> = line.split('"').collect();
                // A library line reads: "path"  "D:\\SteamLibrary"
                if parts.len() >= 4 && parts[1] == "path" {
                    libraries.push(PathBuf::from(parts[3].replace("\\\\", "\\")));
                }
            }
        }
    }
    libraries.dedup();
    libraries.into_iter().map(|l| l.join("steamapps").join("common").join("Deadlock").join("game").join("citadel").join("console.log")).collect()
}

/// The end of the console log is enough: a match's lines are the last ones written.
const CONSOLE_TAIL: u64 = 4 * 1024 * 1024;

#[derive(serde::Serialize)]
struct ConsoleState {
    /// The log that was read, or where it was looked for when none was found.
    path: String,
    found: bool,
    #[serde(flatten)]
    seen: matches::ConsoleMatch,
}

/// The match being played and the signed-in account, from the game's console log.
#[tauri::command]
async fn console_match() -> Result<ConsoleState, String> {
    tauri::async_runtime::spawn_blocking(|| {
        let logs = console_logs();
        // With Deadlock in more than one Steam library, the log the game last wrote is the one that counts.
        let modified = |p: &PathBuf| fs::metadata(p).and_then(|m| m.modified()).ok();
        let Some(path) = logs.iter().filter(|p| p.is_file()).max_by_key(|p| modified(p)) else {
            return ConsoleState { path: logs.first().map(|p| p.display().to_string()).unwrap_or_default(), found: false, seen: Default::default() };
        };
        // What was read so far is kept, and only what the game wrote since is read: a long match can push the
        // line that started it out of any fixed tail of the log.
        static READ: std::sync::Mutex<Option<(PathBuf, u64, matches::ConsoleMatch)>> = std::sync::Mutex::new(None);
        let mut read = READ.lock().unwrap();
        let len = fs::metadata(path).map(|m| m.len()).unwrap_or(0);
        let resume = read.as_ref().filter(|(p, offset, _)| p == path && *offset <= len).map(|(_, offset, seen)| (*offset, seen.clone()));
        // A new log (the game was started again, or another library's) is read from its last part.
        let (start, mut seen) = resume.unwrap_or_else(|| (len.saturating_sub(CONSOLE_TAIL), matches::ConsoleMatch::default()));
        let mut bytes = vec![];
        if let Ok(mut f) = fs::File::open(path) {
            use std::io::{Seek, SeekFrom};
            let _ = f.seek(SeekFrom::Start(start));
            let _ = f.take(len - start).read_to_end(&mut bytes);
        }
        // A line still being written is left for the next read.
        let complete = bytes.iter().rposition(|b| *b == b'\n').map(|i| i + 1).unwrap_or(0);
        seen.feed(&String::from_utf8_lossy(&bytes[..complete]));
        *read = Some((path.clone(), start + complete as u64, seen.clone()));
        ConsoleState { path: path.display().to_string(), found: true, seen }
    })
    .await
    .map_err(|e| e.to_string())
}

/// How long a replay query may take before giving up, and how often its state is asked for.
const DEMO_WAIT: Duration = Duration::from_secs(240);
const DEMO_POLL: Duration = Duration::from_secs(4);

/// The answer to a query against a match's replay, from the statistics service (it downloads and parses the
/// replay, about a minute), kept on this computer once read.
fn demo_answer(dir: &std::path::Path, match_id: u64, name: &str, query: &str) -> Result<String, String> {
    let path = dir.join(format!("{match_id}-{name}.ndjson"));
    if let Ok(text) = fs::read_to_string(&path) {
        return Ok(text);
    }
    let body = serde_json::json!({ "match_id": match_id, "query": query, "format": "ndjson" }).to_string();
    let submitted = match ureq::post(&format!("{MATCH_API}/matches/demo/query")).set("content-type", "application/json").timeout(Duration::from_secs(60)).send_string(&body) {
        Ok(resp) => resp.into_string().map_err(|e| format!("Download failed: {e}"))?,
        Err(ureq::Error::Status(404, _)) => return Err("This match has no replay: Valve keeps the replays of only some matches.".into()),
        Err(ureq::Error::Status(429, _)) => return Err("The replay service is rate limiting requests. Try again in a moment.".into()),
        Err(e) => return Err(format!("Download failed: {e}")),
    };
    let job: serde_json::Value = serde_json::from_str(&submitted).map_err(|e| format!("The statistics service answered in an unexpected form: {e}"))?;
    let id = job.get("job_id").and_then(serde_json::Value::as_str).ok_or("The statistics service answered in an unexpected form: no job")?.to_string();
    let started = std::time::Instant::now();
    let url = loop {
        let state: serde_json::Value = serde_json::from_str(&get(&format!("{MATCH_API}/matches/demo/query/{id}"), "The replay query was lost.")?)
            .map_err(|e| format!("The statistics service answered in an unexpected form: {e}"))?;
        match state.get("status").and_then(serde_json::Value::as_str) {
            Some("done") => break state.get("result_url").and_then(serde_json::Value::as_str).ok_or("The replay query finished without a result.")?.to_string(),
            Some("failed") | Some("error") => return Err(format!("The replay could not be read: {}", state.get("error").map(|e| e.to_string()).unwrap_or_default())),
            _ if started.elapsed() > DEMO_WAIT => return Err("The replay took too long to read. Try again in a few minutes.".into()),
            _ => std::thread::sleep(DEMO_POLL),
        }
    };
    let mut bytes = vec![];
    ureq::get(&url).timeout(Duration::from_secs(120)).call().map_err(|e| format!("Download failed: {e}"))?.into_reader().read_to_end(&mut bytes).map_err(|e| format!("Download failed: {e}"))?;
    let text = engine::replay::decompress(&bytes)?;
    let _ = fs::create_dir_all(dir);
    let _ = fs::write(&path, &text);
    Ok(text)
}

/// One player of a match studied from its replay: the last seconds before each death, and what each ability's
/// casts caught and held. The first look reads the replay (about a minute); it is kept for the next.
#[tauri::command]
async fn replay_study(app: AppHandle, state: State<'_, AppState>, match_id: u64, slot: u32) -> Result<engine::replay::ReplayStudy, String> {
    let data = state.data.read().unwrap().clone();
    let json = match_json(&app, match_id).await?;
    let dir = data_dir(&app)?.join("replays");
    tauri::async_runtime::spawn_blocking(move || {
        let (hits, notes, kills) = std::thread::scope(|scope| {
            let ask = |name: &'static str, query: &'static str| {
                let dir = &dir;
                scope.spawn(move || demo_answer(dir, match_id, name, query))
            };
            let (h, n, k) = (
                ask("hits", engine::replay::QUERY_HITS),
                ask("notes", engine::replay::QUERY_NOTES),
                ask("kills", engine::replay::QUERY_KILLS),
            );
            let join = |t: std::thread::ScopedJoinHandle<'_, Result<String, String>>| t.join().map_err(|_| "Reading the replay failed.".to_string()).and_then(|r| r);
            (join(h), join(n), join(k))
        });
        engine::replay::study(&data, &json, slot, &hits?, &notes?, &kills?)
    })
    .await
    .map_err(|e| e.to_string())?
}

/// When a player of a match bought each item, against when top players of the hero buy it, and the items most of
/// them buy that the player never did.
#[tauri::command]
async fn item_timing(app: AppHandle, state: State<'_, AppState>, match_id: u64, slot: u32) -> Result<engine::timing::TimingReport, String> {
    let data = state.data.read().unwrap().clone();
    let cache = Cache::new(&app, &data)?;
    let json = match_json(&app, match_id).await?;
    tauri::async_runtime::spawn_blocking(move || {
        let report = matches::parse(&data, &json)?;
        let player = report.players.iter().find(|p| p.slot == slot).ok_or("No such player in the match")?;
        let hero = data.hero(&player.hero).ok_or("Unknown hero")?;
        let rows = analytics(&cache, "item-stats", &format!("hero_id={}&bucket=game_time_min", hero.id), ASCENDANT)?;
        let stats: Vec<popular::ItemStat> = serde_json::from_str(&rows).map_err(|e| format!("The statistics service answered in an unexpected form: {e}"))?;
        let worth: Vec<(u32, u32)> = player.timeline.iter().map(|s| (s.time, s.net_worth)).collect();
        Ok(engine::timing::compare(&data, &stats, &player.purchases, report.duration_s, &worth))
    })
    .await
    .map_err(|e| e.to_string())?
}

/// What each item a player bought in a match gave back: its own damage, healing and prevented healing, and the
/// damage its resist reductions added for teammates. Read from the match alone.
#[tauri::command]
async fn item_payoff(app: AppHandle, state: State<'_, AppState>, match_id: u64, slot: u32) -> Result<Vec<engine::payoff::ItemPayoff>, String> {
    let data = state.data.read().unwrap().clone();
    let json = match_json(&app, match_id).await?;
    tauri::async_runtime::spawn_blocking(move || {
        let report = matches::parse(&data, &json)?;
        engine::payoff::payoff(&data, &report, &json, slot)
    })
    .await
    .map_err(|e| e.to_string())?
}

/// A player's ability upgrades in a match beside the order top players of the hero go with most.
#[tauri::command]
async fn match_order(app: AppHandle, state: State<'_, AppState>, match_id: u64, slot: u32) -> Result<Option<engine::order::OrderCompare>, String> {
    let data = state.data.read().unwrap().clone();
    let cache = Cache::new(&app, &data)?;
    let json = match_json(&app, match_id).await?;
    tauri::async_runtime::spawn_blocking(move || {
        let report = matches::parse(&data, &json)?;
        let p = report.players.iter().find(|p| p.slot == slot).ok_or("No such player in the match")?;
        let hero_id = data.hero(&p.hero).map(|h| h.id).ok_or("Unknown hero")?;
        let popular = analytics(&cache, "ability-order-stats", &format!("hero_id={hero_id}"), ASCENDANT)
            .ok()
            .and_then(|json| engine::order::popular_orders(&data, hero_id, &json).ok())
            .unwrap_or_default();
        let steps: Vec<(usize, u8, u32)> = p.abilities.iter().map(|a| (a.index, a.tier, a.time)).collect();
        Ok(engine::order::compare_order(&data, &p.hero, &steps, &popular))
    })
    .await
    .map_err(|e| e.to_string())?
}

/// What the statistics service holds of a finished match once it is shared: its salts, which let anyone read its
/// metadata and, when Valve kept one, its replay.
#[derive(serde::Serialize)]
struct Shared {
    match_id: u64,
    metadata: bool,
    replay: bool,
}

/// Shares a finished match with the statistics service: asking for its salts makes the service fetch them from
/// Steam and keep them, so the match and its replay can be read by anyone. The match's metadata is then downloaded
/// and kept, as the Match analysis screen would.
#[tauri::command]
async fn share_match(app: AppHandle, match_id: u64) -> Result<Shared, String> {
    let url = format!("{MATCH_API}/matches/{match_id}/salts");
    let salts = tauri::async_runtime::spawn_blocking(move || get(&url, "The match is not known yet.")).await.map_err(|e| e.to_string())??;
    let v: serde_json::Value = serde_json::from_str(&salts).map_err(|e| format!("The statistics service answered in an unexpected form: {e}"))?;
    let has = |key: &str| v.get(key).and_then(serde_json::Value::as_u64).is_some();
    let shared = Shared { match_id, metadata: has("metadata_salt"), replay: has("replay_salt") };
    if shared.metadata {
        let _ = match_json(&app, match_id).await;
    }
    Ok(shared)
}

/// The player's recent matches looked at for ones the statistics service does not have, and how many of those are
/// shared in one go.
const HISTORY_LOOKED_AT: usize = 30;
const HISTORY_SHARED_AT_ONCE: usize = 10;
/// Matches known to be on the service, remembered so they are not asked about again.
const HISTORY_REMEMBERED: usize = 2000;

#[derive(serde::Serialize)]
struct HistoryShare {
    /// Recent matches looked at, and how many of them the service did not have.
    checked: usize,
    missing: usize,
    /// Matches the service now has, and the ones it could not get from Steam (yet).
    shared: Vec<u64>,
    failed: Vec<u64>,
}

/// Finds the player's recent matches the statistics service does not have and shares them: asking for a match's
/// salts makes the service get them from Steam, and reading its metadata makes it download the match from Valve.
/// The match is then kept on this computer too. One request for the history, one to see which matches the service
/// has, and two per missing match.
#[tauri::command]
async fn share_history(app: AppHandle, account_id: String) -> Result<HistoryShare, String> {
    let account = steam_account(&account_id)?;
    let done_path = data_dir(&app)?.join("shared-history.json");
    let mut done: Vec<u64> = fs::read_to_string(&done_path).ok().and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default();
    let history = tauri::async_runtime::spawn_blocking(move || get(&format!("{MATCH_API}/players/{account}/match-history"), "No matches found for this account."))
        .await
        .map_err(|e| e.to_string())??;
    let rows: Vec<serde_json::Value> = serde_json::from_str(&history).map_err(|e| format!("The statistics service answered in an unexpected form: {e}"))?;
    let recent: Vec<u64> = rows.iter().filter_map(|r| r.get("match_id").and_then(serde_json::Value::as_u64)).take(HISTORY_LOOKED_AT).collect();
    let unknown: Vec<u64> = recent.iter().copied().filter(|id| !done.contains(id)).collect();
    let mut out = HistoryShare { checked: recent.len(), missing: 0, shared: vec![], failed: vec![] };
    if unknown.is_empty() {
        return Ok(out);
    }
    let list = unknown.iter().map(u64::to_string).collect::<Vec<_>>().join(",");
    let url = format!("{MATCH_API}/matches/metadata?match_ids={list}&include_info=true&limit={}", unknown.len());
    // The service answers "not found" when it has none of them.
    let have = tauri::async_runtime::spawn_blocking(move || get(&url, "[]").or_else(|e| if e == "[]" { Ok("[]".into()) } else { Err(e) }))
        .await
        .map_err(|e| e.to_string())??;
    let have: Vec<u64> = serde_json::from_str::<Vec<serde_json::Value>>(&have)
        .unwrap_or_default()
        .iter()
        .filter_map(|m| m.get("match_id").and_then(serde_json::Value::as_u64))
        .collect();
    done.extend(have.iter().copied());
    let missing: Vec<u64> = unknown.into_iter().filter(|id| !have.contains(id)).collect();
    out.missing = missing.len();
    for id in missing.into_iter().take(HISTORY_SHARED_AT_ONCE) {
        match share_match(app.clone(), id).await {
            Ok(s) if s.metadata => {
                out.shared.push(id);
                done.push(id);
            }
            Ok(_) => out.failed.push(id),
            Err(e) => {
                out.failed.push(id);
                // Rate limited: the rest wait for the next round.
                if e.contains("rate limiting") {
                    break;
                }
            }
        }
    }
    done.sort_unstable();
    done.dedup();
    let keep = done.len().saturating_sub(HISTORY_REMEMBERED);
    let _ = fs::write(&done_path, serde_json::to_string(&done[keep..]).unwrap_or_default());
    Ok(out)
}

/// Matches sent to the statistics service per request, and the name the shares are sent under.
const SALTS_PER_POST: usize = 100;
const SALTS_USERNAME: &str = "deadlock-bcma";

/// Sends the statistics service where Valve serves the matches in Steam's cache (server, metadata and replay salts),
/// so that anyone can read them; each match once. Only these numbers are sent. Returns how many were sent.
#[tauri::command]
async fn share_cached_salts(app: AppHandle, state: State<'_, AppState>) -> Result<usize, String> {
    let sent_path = data_dir(&app)?.join("shared-salts.json");
    let scanned = tauri::async_runtime::spawn_blocking(|| cache_folder().ok().map(|f| scan_cache(&f).2)).await.map_err(|e| e.to_string())?;
    let Some(cached) = scanned else { return Ok(0) };
    *state.cache.lock().unwrap() = Some(cached.clone());
    let mut sent: std::collections::BTreeSet<u64> =
        fs::read_to_string(&sent_path).ok().and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default();
    let rows: Vec<serde_json::Value> = cached
        .iter()
        .filter(|(id, c)| !sent.contains(id) && c.cluster > 0 && c.meta_salt.is_some())
        .map(|(id, c)| {
            serde_json::json!({
                "match_id": id,
                "cluster_id": c.cluster,
                "metadata_salt": c.meta_salt,
                "replay_salt": c.replay_salt,
                "username": SALTS_USERNAME,
            })
        })
        .collect();
    let mut count = 0;
    for chunk in rows.chunks(SALTS_PER_POST) {
        let body = serde_json::Value::Array(chunk.to_vec());
        let posted = tauri::async_runtime::spawn_blocking(move || {
            ureq::post(&format!("{MATCH_API}/matches/salts")).timeout(Duration::from_secs(60)).set("Content-Type", "application/json").send_string(&body.to_string()).map(|_| ()).map_err(|e| e.to_string())
        })
        .await
        .map_err(|e| e.to_string())?;
        if posted.is_err() {
            break;
        }
        for row in chunk {
            if let Some(id) = row["match_id"].as_u64() {
                sent.insert(id);
            }
        }
        count += chunk.len();
    }
    if count > 0 {
        let _ = fs::write(&sent_path, serde_json::to_string(&sent).unwrap_or_default());
    }
    Ok(count)
}

/// Players in a full match.
const FULL_MATCH: usize = 12;

/// Every player of a match being played, with team, hero and lane, from the statistics service's live query: it
/// spectates the match and reads its broadcast, which takes about a minute the first time.
#[tauri::command]
async fn match_teams(match_id: u64, account_id: String) -> Result<matches::LiveMatch, String> {
    let account = steam_account(&account_id)?;
    // A match's teams are read once: the build, the draft and the lane note all ask for them as it starts.
    static TEAMS: std::sync::Mutex<Option<(u64, u64, matches::LiveMatch)>> = std::sync::Mutex::new(None);
    if let Some((m, a, live)) = TEAMS.lock().unwrap().as_ref() {
        if *m == match_id && *a == account {
            return Ok(live.clone());
        }
    }
    let live = tauri::async_runtime::spawn_blocking(move || {
        // Valve's list of matches open to spectators has the teams at once, without anyone watching the match; it
        // only holds the matches the game's Watch tab lists, so the broadcast is read for the others.
        let active = get(&format!("{MATCH_API}/matches/active?account_id={account}"), "No live match found.");
        if let Ok(live) = active.and_then(|json| matches::parse_active(&json, match_id, account)) {
            return Ok(live);
        }
        let query = r#"SELECT "m_steamID", "m_iTeamNum", "m_PlayerDataGlobal__m_nHeroID" AS hero, "m_nAssignedLane" AS lane FROM CCitadelPlayerController WHERE "m_PlayerDataGlobal__m_nHeroID" > 0 LIMIT 24"#;
        let resp = match ureq::get(&format!("{MATCH_API}/matches/demo/live/query"))
            .query("match_id", &match_id.to_string())
            .query("query", query)
            .timeout(Duration::from_secs(180))
            .call()
        {
            Ok(resp) => resp,
            Err(ureq::Error::Status(429, _)) => return Err("The spectating service is busy (it takes a few matches an hour). Trying again shortly.".to_string()),
            Err(ureq::Error::Status(code, r)) => return Err(format!("The spectating service could not watch the match ({code}): {}", r.into_string().unwrap_or_default())),
            Err(e) => return Err(format!("Download failed: {e}")),
        };
        // Server-sent events: read until every player is in or the stream ends.
        let mut text = String::new();
        let mut players = 0;
        for line in std::io::BufReader::new(resp.into_reader()).lines() {
            let Ok(line) = line else { break };
            if line.contains("m_steamID") {
                players += 1;
            }
            let end = line.starts_with("event: end");
            text.push_str(&line);
            text.push('\n');
            if end || players >= FULL_MATCH {
                break;
            }
        }
        matches::parse_live_query(&text, match_id, account).map_err(|e| {
            // Valve's relay sometimes never broadcasts a match: say so plainly rather than pass on the relay's words.
            if e.to_lowercase().contains("broadcast") {
                format!("Match {match_id} was found, but Valve is not broadcasting it, so its teams cannot be read. Try again in a few minutes, or enter the teams by hand.")
            } else {
                e
            }
        })
    })
    .await
    .map_err(|e| e.to_string())??;
    // Teams read before every player was in are shown but asked for again.
    if live.players.len() >= FULL_MATCH {
        *TEAMS.lock().unwrap() = Some((match_id, account, live.clone()));
    }
    Ok(live)
}

/// The players of a match: rank, record on their hero, most played heroes, streak and party, from three batch
/// requests to the statistics service.
#[tauri::command]
async fn scout_lobby(state: State<'_, AppState>, players: Vec<matches::LivePlayer>) -> Result<Vec<engine::scout::Scout>, String> {
    let data = state.data.read().unwrap().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let ids = players.iter().map(|p| p.account_id.to_string()).collect::<Vec<_>>().join(",");
        let fetch = |path: &str| get(&format!("{MATCH_API}/players/{path}?account_ids={ids}"), "No player data found.");
        // Each part only adds to the report: without one, the players are still listed with the rest.
        let stats = fetch("hero-stats").unwrap_or_else(|_| "[]".into());
        let ranks = fetch("rank").unwrap_or_else(|_| "[]".into());
        let profiles = fetch("steam").unwrap_or_else(|_| "[]".into());
        Ok(engine::scout::scout(&data, &players, &stats, &ranks, &profiles))
    })
    .await
    .map_err(|e| e.to_string())?
}

#[derive(serde::Serialize)]
struct OrderReport {
    order: engine::order::AbilityOrder,
    /// What top players of the hero go with, when the statistics service can be reached.
    popular: Vec<engine::order::PopularOrder>,
    rank: String,
}

/// The order to spend ability points in for the build, and the orders top players go with.
#[tauri::command]
async fn ability_order(app: AppHandle, state: State<'_, AppState>, build: BuildInput) -> Result<OrderReport, String> {
    let data = state.data.read().unwrap().clone();
    let cache = Cache::new(&app, &data)?;
    tauri::async_runtime::spawn_blocking(move || {
        let order = engine::order::ability_order(&data, &build)?;
        let hero_id = data.hero(&build.hero).map(|h| h.id).ok_or("Unknown hero")?;
        let popular = analytics(&cache, "ability-order-stats", &format!("hero_id={hero_id}"), ASCENDANT)
            .ok()
            .and_then(|json| engine::order::popular_orders(&data, hero_id, &json).ok())
            .unwrap_or_default();
        Ok(OrderReport { order, popular, rank: rank_name((ASCENDANT, None)) })
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Downloads a match's metadata (or reads the copy saved earlier) and turns it into a report.
#[tauri::command]
async fn fetch_match(app: AppHandle, state: State<'_, AppState>, match_id: u64) -> Result<matches::MatchReport, String> {
    let dir = data_dir(&app)?.join("matches");
    let json = match_json(&app, match_id).await?;
    let mut report = matches::parse(&state.data.read().unwrap(), &json)?;
    // Player names are a separate lookup; without them the report is still complete.
    let names_path = dir.join(format!("{match_id}.names.json"));
    let names = match fs::read_to_string(&names_path) {
        Ok(names) => Some(names),
        Err(_) => {
            let ids: Vec<String> = report.players.iter().map(|p| p.account_id.to_string()).collect();
            let url = format!("{MATCH_API}/players/steam?account_ids={}", ids.join(","));
            let names = tauri::async_runtime::spawn_blocking(move || get(&url, "No profiles found.")).await.ok().and_then(Result::ok);
            if let Some(names) = &names {
                let _ = fs::write(&names_path, names);
            }
            names
        }
    };
    if let Some(names) = names {
        matches::apply_names(&mut report, &names);
    }
    // Ranked matches record each player's rank; for the others, the players' current ranks, read once per match.
    if report.players.iter().any(|p| p.badge_from.is_empty() && p.account_id > 0) {
        let ranks_path = dir.join(format!("{match_id}.ranks.json"));
        let ids: Vec<String> = report.players.iter().map(|p| p.account_id.to_string()).collect();
        let url = format!("{MATCH_API}/players/rank?account_ids={}", ids.join(","));
        let ranks = tauri::async_runtime::spawn_blocking(move || kept(&ranks_path, u64::MAX, || get(&url, "No ranks found."))).await.ok().and_then(Result::ok);
        if let Some(ranks) = ranks {
            matches::apply_ranks(&mut report, &ranks);
        }
    }
    Ok(report)
}

/// A file kept from an earlier answer while younger than `keep_s` seconds, else a fresh answer that is then kept.
/// When the service cannot be reached, an older kept answer is used.
fn kept(path: &std::path::Path, keep_s: u64, fetch: impl FnOnce() -> Result<String, String>) -> Result<String, String> {
    let fresh = fs::metadata(path).and_then(|m| m.modified()).ok().and_then(|t| t.elapsed().ok()).is_some_and(|age| age.as_secs() < keep_s);
    if fresh {
        if let Ok(text) = fs::read_to_string(path) {
            return Ok(text);
        }
    }
    match fetch() {
        Ok(text) => {
            if let Some(dir) = path.parent() {
                let _ = fs::create_dir_all(dir);
            }
            let _ = fs::write(path, &text);
            Ok(text)
        }
        Err(e) => fs::read_to_string(path).map_err(|_| e),
    }
}

/// How long the top players' habits on a hero are kept before they are read again.
const REFERENCE_KEEP_S: u64 = 7 * 86_400;
/// Top players' matches read for them, and the rank and days they come from.
const REFERENCE_MATCHES: usize = 3;
const REFERENCE_BADGE: u32 = 100;
const REFERENCE_DAYS: u64 = 14;
/// A player's own record on their heroes is kept this long.
const OWN_AVERAGE_KEEP_S: u64 = 12 * 3600;

#[derive(serde::Serialize)]
struct MatchReview {
    habits: engine::review::Habits,
    lane: engine::review::LaneFlow,
}

/// Where a player spent the match, what their time went to and how the laning phase went minute by minute. Read
/// from the match alone: nothing is downloaded.
#[tauri::command]
async fn match_review(app: AppHandle, state: State<'_, AppState>, match_id: u64, slot: u32) -> Result<MatchReview, String> {
    let data = state.data.read().unwrap().clone();
    let json = match_json(&app, match_id).await?;
    tauri::async_runtime::spawn_blocking(move || {
        let report = matches::parse(&data, &json)?;
        Ok(MatchReview { habits: engine::review::habits(&json, slot)?, lane: engine::review::lane_flow(&report, &json, slot)? })
    })
    .await
    .map_err(|e| e.to_string())?
}

/// The same habits of top players on the hero a player played in a match, in the same lane when possible: a few
/// recent high-rank matches of the hero, read once a week.
#[tauri::command]
async fn top_habits(app: AppHandle, state: State<'_, AppState>, match_id: u64, slot: u32) -> Result<engine::review::Habits, String> {
    let data = state.data.read().unwrap().clone();
    let json = match_json(&app, match_id).await?;
    let dir = data_dir(&app)?.join("references");
    tauri::async_runtime::spawn_blocking(move || {
        let report = matches::parse(&data, &json)?;
        let player = report.players.iter().find(|p| p.slot == slot).ok_or("No such player in the match")?;
        let hero_id = data.hero(&player.hero).ok_or("Unknown hero")?.id;
        let lane = engine::review::assigned_lane(&json, slot);
        let path = dir.join(format!("{hero_id}-{lane}.json"));
        let text = kept(&path, REFERENCE_KEEP_S, || {
            let day = SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0) / 86_400;
            let from = (day - REFERENCE_DAYS) * 86_400;
            let url = format!(
                "{MATCH_API}/matches/metadata?hero_ids={hero_id}&min_average_badge={REFERENCE_BADGE}&min_unix_timestamp={from}&min_duration_s=1500&game_mode=normal&include_player_info=true&order_by=match_id&order_direction=desc&limit=10"
            );
            let bulk = get(&url, "No recent top-rank matches of this hero.")?;
            let picked = engine::review::reference_matches(&bulk, hero_id, lane, REFERENCE_MATCHES);
            let all: Vec<engine::review::Habits> = picked
                .iter()
                .filter_map(|(id, slot)| {
                    let mut bytes = vec![];
                    let resp = ureq::get(&format!("{MATCH_API}/matches/{id}/metadata/raw")).timeout(Duration::from_secs(60)).call().ok()?;
                    resp.into_reader().read_to_end(&mut bytes).ok()?;
                    let json = engine::cached_meta::metadata_json(&bytes).ok()?;
                    engine::review::habits(&json, *slot).ok()
                })
                .collect();
            if all.is_empty() {
                return Err("No recent top-rank matches of this hero could be read.".into());
            }
            serde_json::to_string(&engine::review::average(&all)).map_err(|e| e.to_string())
        })?;
        serde_json::from_str(&text).map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| e.to_string())?
}

/// A player's match next to their own average on the hero.
#[tauri::command]
async fn hero_average(app: AppHandle, state: State<'_, AppState>, match_id: u64, slot: u32) -> Result<Option<engine::review::VersusAverage>, String> {
    let data = state.data.read().unwrap().clone();
    let json = match_json(&app, match_id).await?;
    let dir = data_dir(&app)?.join("players");
    tauri::async_runtime::spawn_blocking(move || {
        let report = matches::parse(&data, &json)?;
        let player = report.players.iter().find(|p| p.slot == slot).ok_or("No such player in the match")?;
        if player.account_id == 0 {
            return Ok(None);
        }
        let hero_id = data.hero(&player.hero).ok_or("Unknown hero")?.id;
        let id = player.account_id;
        let stats = kept(&dir.join(format!("{id}.hero-stats.json")), OWN_AVERAGE_KEEP_S, || {
            get(&format!("{MATCH_API}/players/hero-stats?account_ids={id}"), "No matches found for this account.")
        })?;
        Ok(engine::review::versus_average(&report, slot, hero_id, &stats))
    })
    .await
    .map_err(|e| e.to_string())?
}

/// A match found in Steam's cache on this computer.
#[derive(serde::Serialize)]
struct LocalMatch {
    match_id: u64,
    /// Unix seconds of the newest cache file for the match.
    seen_at: u64,
    has_replay: bool,
    /// The match's details are in the cache: it opens without asking anyone.
    has_details: bool,
}

#[derive(serde::Serialize)]
struct LocalScan {
    /// The cache folder that was read, or where it was looked for.
    folder: String,
    found: bool,
    files: usize,
    matches: Vec<LocalMatch>,
}

/// Where Steam is installed: the registry first, then the usual places.
fn steam_folders() -> Vec<PathBuf> {
    let mut out = vec![];
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        // CREATE_NO_WINDOW: no console flashes up for the query.
        let query = std::process::Command::new("reg")
            .args(["query", r"HKCU\Software\Valve\Steam", "/v", "SteamPath"])
            .creation_flags(0x0800_0000)
            .output();
        if let Ok(output) = query {
            let text = String::from_utf8_lossy(&output.stdout);
            if let Some(path) = text.lines().find_map(|l| l.split("REG_SZ").nth(1)) {
                out.push(PathBuf::from(path.trim()));
            }
        }
    }
    for path in [r"C:\Program Files (x86)\Steam", r"C:\Program Files\Steam"] {
        out.push(PathBuf::from(path));
    }
    out
}

fn scan_cache(folder: &std::path::Path) -> (usize, Vec<LocalMatch>, std::collections::BTreeMap<u64, CachedMatch>) {
    let mut found: std::collections::BTreeMap<u64, LocalMatch> = Default::default();
    let mut cached: std::collections::BTreeMap<u64, CachedMatch> = Default::default();
    let mut files = 0;
    let subfolders = fs::read_dir(folder).into_iter().flatten().flatten();
    for file in subfolders.flat_map(|d| fs::read_dir(d.path()).into_iter().flatten().flatten()) {
        files += 1;
        // The request is named at the very start of each file.
        let mut head = [0u8; 512];
        let Ok(read) = fs::File::open(file.path()).and_then(|mut f| f.read(&mut head)) else { continue };
        let Some(entry) = matches::cache_entry(&head[..read]) else { continue };
        let seen_at = file
            .metadata()
            .and_then(|m| m.modified())
            .ok()
            .and_then(|t| t.duration_since(SystemTime::UNIX_EPOCH).ok())
            .map(|d| d.as_secs())
            .unwrap_or(0);
        let row = found.entry(entry.match_id).or_insert(LocalMatch { match_id: entry.match_id, seen_at: 0, has_replay: false, has_details: false });
        row.seen_at = row.seen_at.max(seen_at);
        row.has_replay |= entry.demo;
        row.has_details |= !entry.demo;
        let c = cached.entry(entry.match_id).or_default();
        if entry.cluster > 0 {
            c.cluster = entry.cluster;
        }
        if entry.demo {
            c.replay_salt = Some(entry.salt);
        } else {
            c.meta_salt = Some(entry.salt);
            c.meta_path = Some(file.path());
        }
    }
    let mut matches: Vec<LocalMatch> = found.into_values().collect();
    matches.sort_by(|a, b| b.seen_at.cmp(&a.seen_at).then(b.match_id.cmp(&a.match_id)));
    (files, matches, cached)
}

/// Steam's HTTP cache folder, when there is one.
fn cache_folder() -> Result<PathBuf, PathBuf> {
    let folders: Vec<PathBuf> = steam_folders().into_iter().map(|p| p.join("appcache").join("httpcache")).collect();
    folders.iter().find(|f| f.is_dir()).cloned().ok_or_else(|| folders.first().cloned().unwrap_or_default())
}

/// Finds the matches the game has opened on this computer by reading Steam's HTTP cache. Nothing is sent anywhere.
#[tauri::command]
async fn find_local_matches(state: State<'_, AppState>) -> Result<LocalScan, String> {
    let (scan, cached) = tauri::async_runtime::spawn_blocking(|| match cache_folder() {
        Ok(folder) => {
            let (files, matches, cached) = scan_cache(&folder);
            (LocalScan { folder: folder.display().to_string(), found: true, files, matches }, Some(cached))
        }
        Err(folder) => (LocalScan { folder: folder.display().to_string(), found: false, files: 0, matches: vec![] }, None),
    })
    .await
    .map_err(|e| e.to_string())?;
    if let Some(cached) = cached {
        *state.cache.lock().unwrap() = Some(cached);
    }
    Ok(scan)
}

#[tauri::command]
async fn match_history(state: State<'_, AppState>, account_id: String) -> Result<Vec<matches::HistoryEntry>, String> {
    let id: u64 = account_id.trim().parse().map_err(|_| "Enter a Steam account ID (numbers only)".to_string())?;
    let id = if id > STEAM_ID64_BASE { id - STEAM_ID64_BASE } else { id };
    let url = format!("{MATCH_API}/players/{id}/match-history");
    let json = tauri::async_runtime::spawn_blocking(move || get(&url, "No match history found for this account."))
        .await
        .map_err(|e| e.to_string())??;
    matches::parse_history(&state.data.read().unwrap(), &json, 40)
}

fn build_path(app: &AppHandle, name: &str) -> Result<PathBuf, String> {
    let clean: String = name
        .trim()
        .chars()
        .filter(|c| c.is_alphanumeric() || matches!(c, ' ' | '-' | '_'))
        .collect();
    if clean.is_empty() {
        return Err("Enter a valid build name".into());
    }
    Ok(builds_dir(app)?.join(format!("{clean}.json")))
}

#[tauri::command]
fn save_build(app: AppHandle, name: String, build: BuildInput) -> Result<(), String> {
    let json = serde_json::to_string_pretty(&build).map_err(|e| e.to_string())?;
    fs::write(build_path(&app, &name)?, json).map_err(|e| e.to_string())
}

#[tauri::command]
fn list_builds(app: AppHandle) -> Result<Vec<String>, String> {
    let mut names: Vec<String> = fs::read_dir(builds_dir(&app)?)
        .map_err(|e| e.to_string())?
        .filter_map(|e| {
            let path = e.ok()?.path();
            (path.extension()? == "json").then(|| path.file_stem()?.to_str().map(String::from))?
        })
        .collect();
    names.sort();
    Ok(names)
}

#[tauri::command]
fn load_build(app: AppHandle, name: String) -> Result<BuildInput, String> {
    let json = fs::read_to_string(build_path(&app, &name)?).map_err(|e| e.to_string())?;
    serde_json::from_str(&json).map_err(|e| e.to_string())
}

#[tauri::command]
fn delete_build(app: AppHandle, name: String) -> Result<(), String> {
    fs::remove_file(build_path(&app, &name)?).map_err(|e| e.to_string())
}

/// What a backup file holds: the app's settings (as the window keeps them), the saved builds, and optionally the
/// matches kept on this computer.
#[derive(serde::Serialize, serde::Deserialize)]
struct Backup {
    app: String,
    version: u32,
    exported_at: u64,
    settings: serde_json::Value,
    builds: std::collections::BTreeMap<String, serde_json::Value>,
    #[serde(default)]
    matches: std::collections::BTreeMap<String, String>,
}

const BACKUP_APP: &str = "deadlock-bcma";
const BACKUP_VERSION: u32 = 1;

/// Writes a backup to the Downloads folder and says where.
#[tauri::command]
async fn export_backup(app: AppHandle, settings: serde_json::Value, include_matches: bool) -> Result<String, String> {
    let builds_dir = builds_dir(&app)?;
    let matches_dir = data_dir(&app)?.join("matches");
    let out_dir = app.path().download_dir().or_else(|_| data_dir(&app)).map_err(|e| e.to_string())?;
    tauri::async_runtime::spawn_blocking(move || {
        let read_dir = |dir: &PathBuf| fs::read_dir(dir).into_iter().flatten().flatten().map(|e| e.path()).filter(|p| p.extension().is_some_and(|x| x == "json")).collect::<Vec<_>>();
        let stem = |p: &PathBuf| p.file_stem().and_then(|s| s.to_str()).map(String::from);
        let builds = read_dir(&builds_dir)
            .iter()
            .filter_map(|p| Some((stem(p)?, serde_json::from_str(&fs::read_to_string(p).ok()?).ok()?)))
            .collect();
        let matches = if include_matches { read_dir(&matches_dir).iter().filter_map(|p| Some((stem(p)?, fs::read_to_string(p).ok()?))).collect() } else { Default::default() };
        let now = SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
        let backup = Backup { app: BACKUP_APP.into(), version: BACKUP_VERSION, exported_at: now, settings, builds, matches };
        let path = out_dir.join(format!("deadlock-bcma-backup-{}.json", now / 86_400 * 86_400));
        fs::write(&path, serde_json::to_string(&backup).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
        Ok(path.display().to_string())
    })
    .await
    .map_err(|e| e.to_string())?
}

#[derive(serde::Serialize)]
struct Restored {
    /// The settings to put back in the window.
    settings: serde_json::Value,
    builds: usize,
    matches: usize,
}

/// Restores a backup file's builds and matches, and hands back its settings. A build of the same name is replaced.
#[tauri::command]
async fn import_backup(app: AppHandle, content: String) -> Result<Restored, String> {
    let builds_dir = builds_dir(&app)?;
    let matches_dir = data_dir(&app)?.join("matches");
    tauri::async_runtime::spawn_blocking(move || {
        let backup: Backup = serde_json::from_str(&content).map_err(|_| "This is not a Deadlock BCMA backup file.".to_string())?;
        if backup.app != BACKUP_APP {
            return Err("This is not a Deadlock BCMA backup file.".to_string());
        }
        // Names and match ids become file names: only plain ones are written.
        let plain = |s: &str| !s.is_empty() && !s.contains(['/', '\\', ':']) && !s.starts_with('.');
        let mut builds = 0;
        for (name, build) in &backup.builds {
            if plain(name) && fs::write(builds_dir.join(format!("{name}.json")), serde_json::to_string_pretty(build).unwrap_or_default()).is_ok() {
                builds += 1;
            }
        }
        let _ = fs::create_dir_all(&matches_dir);
        let mut matches = 0;
        for (id, json) in &backup.matches {
            if plain(id) && fs::write(matches_dir.join(format!("{id}.json")), json).is_ok() {
                matches += 1;
            }
        }
        Ok(Restored { settings: backup.settings, builds, matches })
    })
    .await
    .map_err(|e| e.to_string())?
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        // Updates come from the GitHub releases; the process plugin restarts the app after one is installed.
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_process::init())
        .setup(|app| {
            let data = load_initial(app.handle());
            note_patch(app.handle(), &data);
            app.manage(AppState { data: RwLock::new(data), cache: Default::default() });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            item_payoff,
            match_order,
            meta_shift,
            share_history,
            match_review,
            top_habits,
            hero_average,
            get_data,
            compute,
            rank_next,
            advise_build,
            suggest_builds,
            rift_comeback,
            power_curve,
            farm_report,
            popular_build,
            master_build,
            draft_picks,
            hero_tiers,
            hero_page,
            my_heroes,
            lane_note,
            played_heroes,
            export_backup,
            import_backup,
            item_tiers,
            patch_notes,
            simulate_combo,
            refresh_data,
            save_build,
            list_builds,
            load_build,
            delete_build,
            fetch_match,
            death_analysis,
            death_trend,
            console_match,
            match_teams,
            share_match,
            item_timing,
            replay_study,
            scout_lobby,
            ability_order,
            match_history,
            find_local_matches,
            share_cached_salts,
            analyze_history
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
