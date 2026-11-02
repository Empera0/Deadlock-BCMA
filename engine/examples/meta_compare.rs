//! Compares a match file decoded here with the statistics service's JSON of the same match:
//! `cargo run -p engine --example meta_compare -- <file> <service json>`.
use serde_json::Value;

fn walk(path: &str, a: &Value, b: &Value, out: &mut Vec<String>) {
    match (a, b) {
        (Value::Object(x), Value::Object(y)) => {
            for k in x.keys().chain(y.keys()).collect::<std::collections::BTreeSet<_>>() {
                match (x.get(k), y.get(k)) {
                    (Some(p), Some(q)) => walk(&format!("{path}.{k}"), p, q, out),
                    (Some(p), None) => out.push(format!("only ours {path}.{k} = {}", short(p))),
                    (None, Some(q)) => out.push(format!("only theirs {path}.{k} = {}", short(q))),
                    _ => {}
                }
            }
        }
        (Value::Array(x), Value::Array(y)) => {
            if x.len() != y.len() {
                out.push(format!("length {path}: {} vs {}", x.len(), y.len()));
            }
            for (i, (p, q)) in x.iter().zip(y).enumerate() {
                walk(&format!("{path}[{i}]"), p, q, out);
            }
        }
        (Value::Number(x), Value::Number(y)) if (x.as_f64().unwrap_or(0.0) - y.as_f64().unwrap_or(0.0)).abs() < 1e-3 => {}
        _ if a == b => {}
        _ => out.push(format!("differs {path}: {} vs {}", short(a), short(b))),
    }
}

fn short(v: &Value) -> String {
    let s = v.to_string();
    if s.len() > 80 { format!("{}…", &s[..80]) } else { s }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let ours: Value = serde_json::from_str(&engine::cached_meta::metadata_json(&std::fs::read(&args[1]).unwrap()).unwrap()).unwrap();
    let theirs: Value = serde_json::from_str(&std::fs::read_to_string(&args[2]).unwrap()).unwrap();
    let mut out = vec![];
    walk("", &ours, &theirs, &mut out);
    // Same kinds of difference once: paths without indices.
    let mut kinds: std::collections::BTreeMap<String, (usize, String)> = Default::default();
    for d in &out {
        let key: String = d.split(':').next().unwrap().chars().filter(|c| !c.is_ascii_digit()).collect::<String>().split(" = ").next().unwrap().to_string();
        let e = kinds.entry(key).or_insert((0, d.clone()));
        e.0 += 1;
    }
    for (k, (n, ex)) in &kinds {
        println!("{n:6}  {k}\n        e.g. {ex}");
    }
    println!("{} differences", out.len());
}
