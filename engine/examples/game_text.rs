//! The game's own Turkish for its texts, read from the Turkish assets: item and ability descriptions and the labels
//! of their values, by their English text. Hero, item and ability names are left out: the app keeps them as in the
//! English data. `cargo run -p engine --example game_text -- <english assets dir> <turkish assets dir> <out json>`
//! with heroes.json and items.json in each directory.

use std::collections::{BTreeMap, HashMap, HashSet};

use engine::{data::GameData, raw};

fn load(dir: &str) -> GameData {
    let read = |f: &str| std::fs::read_to_string(format!("{dir}/{f}")).unwrap_or_else(|e| panic!("{dir}/{f}: {e}"));
    raw::normalize(&read("heroes.json"), &read("items.json"), None, 0).unwrap()
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let (en, tr) = (load(&args[1]), load(&args[2]));
    let names: HashSet<&str> = en
        .heroes
        .iter()
        .flat_map(|h| std::iter::once(h.name.as_str()).chain(h.abilities.iter().map(|a| a.name.as_str())))
        .chain(en.items.iter().map(|i| i.name.as_str()))
        .collect();
    // Every Turkish text seen for an English one; the most common wins.
    let mut seen: HashMap<String, HashMap<String, u32>> = HashMap::new();
    let mut pair = |a: &str, b: &str| {
        let (a, b) = (a.trim(), b.trim());
        if a.is_empty() || b.is_empty() || a == b || names.contains(a) || !a.chars().any(|c| c.is_ascii_alphabetic()) {
            return;
        }
        *seen.entry(a.to_string()).or_default().entry(b.to_string()).or_default() += 1;
    };
    let props = |a: &[engine::data::Prop], b: &[engine::data::Prop], pair: &mut dyn FnMut(&str, &str)| {
        for p in a {
            if let Some(q) = b.iter().find(|q| q.key == p.key) {
                pair(&p.label, &q.label);
            }
        }
    };
    for item in &en.items {
        let Some(other) = tr.items.iter().find(|i| i.class_name == item.class_name) else { continue };
        pair(&item.description, &other.description);
        props(&item.props, &other.props, &mut pair);
    }
    for hero in &en.heroes {
        let Some(other) = tr.heroes.iter().find(|h| h.class_name == hero.class_name) else { continue };
        for a in &hero.abilities {
            let Some(b) = other.abilities.iter().find(|b| b.class_name == a.class_name) else { continue };
            pair(&a.description, &b.description);
            pair(&a.quip, &b.quip);
            props(&a.props, &b.props, &mut pair);
        }
    }
    let out: BTreeMap<String, String> = seen
        .into_iter()
        .map(|(en, counts)| {
            let best = counts.into_iter().max_by(|a, b| a.1.cmp(&b.1).then(b.0.cmp(&a.0))).unwrap().0;
            (en, best)
        })
        .collect();
    std::fs::write(&args[3], serde_json::to_string_pretty(&out).unwrap() + "\n").unwrap();
    println!("{} texts -> {}", out.len(), args[3]);
}
