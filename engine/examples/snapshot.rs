//! Generates the embedded data snapshot from raw API files:
//! cargo run -p engine --example snapshot -- data-raw src-tauri/data/snapshot.json

use std::{env, fs, time::SystemTime};

fn main() -> Result<(), String> {
    let args: Vec<String> = env::args().collect();
    let (dir, out) = match args.as_slice() {
        [_, dir, out] => (dir, out),
        _ => return Err("usage: snapshot <raw-data-dir> <output.json>".into()),
    };
    let read = |name: &str| fs::read_to_string(format!("{dir}/{name}")).map_err(|e| format!("{name}: {e}"));
    let version = read("client-versions.json")
        .ok()
        .and_then(|s| serde_json::from_str::<Vec<u64>>(&s).ok())
        .and_then(|v| v.into_iter().max());
    let now = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let data = engine::raw::normalize(&read("heroes.json")?, &read("items.json")?, version, now)?;
    let json = serde_json::to_string(&data).map_err(|e| e.to_string())?;
    fs::write(out, &json).map_err(|e| e.to_string())?;
    println!(
        "{} heroes, {} items, version {:?}, {} KB -> {out}",
        data.heroes.len(),
        data.items.len(),
        data.client_version,
        json.len() / 1024
    );
    Ok(())
}
