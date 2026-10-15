use engine::{build::BuildInput, calc, combo, data::GameData, matches, popular, raw};
use std::{
    fs,
    io::Read,
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
/// Answers of the analytics endpoints are kept this long; the API itself refreshes them every few hours.
const ANALYTICS_CACHE_S: u64 = 6 * 3600;

/// An analytics endpoint with the shared filters, read from the cache when it is fresh enough.
fn analytics(cache: &PathBuf, endpoint: &str, query: &str, badge: u32) -> Result<String, String> {
    // The window starts at a whole day, so the same question is the same request all day long.
    let day = SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0) / 86_400;
    let from = (day - ANALYTICS_DAYS) * 86_400;
    let filters = format!("min_unix_timestamp={from}&min_average_badge={badge}");
    let query = if query.is_empty() { filters } else { format!("{query}&{filters}") };
    let name: String = format!("{endpoint}-{query}").chars().map(|c| if c.is_ascii_alphanumeric() { c } else { '_' }).collect();
    let path = cache.join(format!("{name}.json"));
    let fresh = fs::metadata(&path)
        .and_then(|m| m.modified())
        .ok()
        .and_then(|t| t.elapsed().ok())
        .is_some_and(|age| age.as_secs() < ANALYTICS_CACHE_S);
    if fresh {
        if let Ok(json) = fs::read_to_string(&path) {
            return Ok(json);
        }
    }
    let json = get(&format!("{MATCH_API}/analytics/{endpoint}?{query}"), "The statistics service has no data for this.")?;
    let _ = fs::create_dir_all(cache);
    let _ = fs::write(&path, &json);
    Ok(json)
}

fn parse<T: serde::de::DeserializeOwned>(json: &str) -> Result<T, String> {
    serde_json::from_str(json).map_err(|e| format!("The statistics service answered in an unexpected form: {e}"))
}

/// A hero's popular build: what top players bought and kept over the last two weeks, and the rank it was read at.
fn popular_for(data: &GameData, cache: &PathBuf, hero_id: u64) -> Result<(popular::PopularBuild, u32), String> {
    for badge in [ETERNUS, ASCENDANT] {
        let heroes: Vec<popular::HeroStat> = parse(&analytics(cache, "hero-stats", "", badge)?)?;
        let matches = heroes.iter().find(|h| h.hero_id == hero_id).map(|h| h.matches).unwrap_or(0);
        if matches < ENOUGH_HERO_MATCHES && badge == ETERNUS {
            continue;
        }
        if matches == 0 {
            return Err("No recent matches of this hero at the top ranks.".into());
        }
        let stats: Vec<popular::ItemStat> = parse(&analytics(cache, "item-stats", &format!("hero_id={hero_id}"), badge)?)?;
        return popular::popular_build(data, hero_id, matches, &stats).map(|b| (b, badge)).ok_or_else(|| "Unknown hero.".to_string());
    }
    Err("No recent matches of this hero at the top ranks.".into())
}

fn rank_name(badge: u32) -> &'static str {
    if badge >= ETERNUS { "Eternus" } else { "Ascendant and above" }
}

#[derive(serde::Serialize)]
struct CounterReport {
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
async fn popular_build(app: AppHandle, state: State<'_, AppState>, hero_id: u64) -> Result<PopularReport, String> {
    let data = state.data.read().unwrap().clone();
    let cache = data_dir(&app)?.join("analytics");
    tauri::async_runtime::spawn_blocking(move || {
        let (build, badge) = popular_for(&data, &cache, hero_id)?;
        Ok(PopularReport { build, days: ANALYTICS_DAYS, rank: rank_name(badge).into() })
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Items to buy against a team of enemies, for a hero of the player's choice or for anyone.
#[tauri::command]
async fn counter_advice(
    app: AppHandle,
    state: State<'_, AppState>,
    my_hero: Option<String>,
    allies: Vec<u64>,
    enemies: Vec<u64>,
    lane: Vec<u64>,
) -> Result<CounterReport, String> {
    let data = state.data.read().unwrap().clone();
    let cache = data_dir(&app)?.join("analytics");
    tauri::async_runtime::spawn_blocking(move || {
        let my_id = my_hero.as_deref().and_then(|h| data.hero(h)).map(|h| h.id);
        // The same hero cannot be on both teams.
        let enemies: Vec<u64> = enemies.into_iter().filter(|id| Some(*id) != my_id && !allies.contains(id)).collect();
        let allies: Vec<u64> = allies.into_iter().filter(|id| Some(*id) != my_id).collect();
        let ally_builds: Vec<popular::PopularBuild> = allies.iter().map(|id| popular_for(&data, &cache, *id).map(|b| b.0)).collect::<Result<_, _>>()?;
        let ally_team: Vec<popular::Ally> = allies.iter().zip(&ally_builds).map(|(id, build)| popular::Ally { hero_id: *id, build }).collect();
        // What everybody buys against an enemy: one hero's own sample at the top ranks is too small to read.
        let mut lowest = ETERNUS;
        let mut builds = vec![];
        let mut lifts = vec![];
        for id in &enemies {
            let (build, badge) = popular_for(&data, &cache, *id)?;
            lowest = lowest.min(badge);
            builds.push(build);
            let mut found = vec![];
            for badge in [ETERNUS, ASCENDANT] {
                let general: Vec<popular::ItemStat> = parse(&analytics(&cache, "item-stats", "", badge)?)?;
                let against: Vec<popular::ItemStat> = parse(&analytics(&cache, "item-stats", &format!("enemy_hero_ids={id}"), badge)?)?;
                let sample = against.iter().map(|r| r.matches).max().unwrap_or(0);
                found = popular::lifts(&general, &against);
                if sample >= ENOUGH_ITEM_MATCHES || badge == ASCENDANT {
                    lowest = lowest.min(badge);
                    break;
                }
            }
            lifts.push(found);
        }
        let team: Vec<popular::Enemy> =
            enemies.iter().zip(&builds).zip(&lifts).map(|((id, build), lifts)| popular::Enemy { hero_id: *id, build, lifts }).collect();
        // Which resist reduction is worth it depends on the damage the player's own hero deals.
        let mine = match my_id {
            Some(id) => Some(popular_for(&data, &cache, id)?.0),
            None => None,
        };
        let my_spirit = mine.as_ref().map(|b| popular::deals_spirit(&data, b));
        let my_core: Vec<String> = mine.as_ref().map(|b| b.core.iter().map(|c| c.item.clone()).collect()).unwrap_or_default();
        let plan = popular::counter_plan(
            &data,
            &popular::Matchup { my_hero: my_hero.as_deref(), my_spirit, my_core: &my_core, allies: &ally_team, enemies: &team, lane: &lane },
        );
        Ok(CounterReport { builds: builds.clone(), plan, days: ANALYTICS_DAYS, rank: rank_name(lowest).into() })
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

#[tauri::command]
async fn refresh_data(app: AppHandle, state: State<'_, AppState>) -> Result<GameData, String> {
    let data = tauri::async_runtime::spawn_blocking(download)
        .await
        .map_err(|e| e.to_string())??;
    let json = serde_json::to_string(&data).map_err(|e| e.to_string())?;
    fs::write(data_dir(&app)?.join("gamedata.json"), json).map_err(|e| e.to_string())?;
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
    Ok(report)
}

/// A match found in Steam's cache on this computer.
#[derive(serde::Serialize)]
struct LocalMatch {
    match_id: u64,
    /// Unix seconds of the newest cache file for the match.
    seen_at: u64,
    has_replay: bool,
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

fn scan_cache(folder: &std::path::Path) -> (usize, Vec<LocalMatch>) {
    let mut found: std::collections::BTreeMap<u64, LocalMatch> = Default::default();
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
        let row = found.entry(entry.match_id).or_insert(LocalMatch { match_id: entry.match_id, seen_at: 0, has_replay: false });
        row.seen_at = row.seen_at.max(seen_at);
        row.has_replay |= entry.demo;
    }
    let mut matches: Vec<LocalMatch> = found.into_values().collect();
    matches.sort_by(|a, b| b.seen_at.cmp(&a.seen_at).then(b.match_id.cmp(&a.match_id)));
    (files, matches)
}

/// Finds the matches the game has opened on this computer by reading Steam's HTTP cache. Nothing is sent anywhere.
#[tauri::command]
async fn find_local_matches() -> Result<LocalScan, String> {
    tauri::async_runtime::spawn_blocking(|| {
        let folders: Vec<PathBuf> = steam_folders().into_iter().map(|p| p.join("appcache").join("httpcache")).collect();
        match folders.iter().find(|f| f.is_dir()) {
            Some(folder) => {
                let (files, matches) = scan_cache(folder);
                LocalScan { folder: folder.display().to_string(), found: true, files, matches }
            }
            None => LocalScan {
                folder: folders.first().map(|f| f.display().to_string()).unwrap_or_default(),
                found: false,
                files: 0,
                matches: vec![],
            },
        }
    })
    .await
    .map_err(|e| e.to_string())
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

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        // Updates come from the GitHub releases; the process plugin restarts the app after one is installed.
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_process::init())
        .setup(|app| {
            let data = load_initial(app.handle());
            app.manage(AppState { data: RwLock::new(data) });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_data,
            compute,
            rank_next,
            suggest_builds,
            rift_comeback,
            popular_build,
            counter_advice,
            simulate_combo,
            refresh_data,
            save_build,
            list_builds,
            load_build,
            delete_build,
            fetch_match,
            match_history,
            find_local_matches,
            analyze_history
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
