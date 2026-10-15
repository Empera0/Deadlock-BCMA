use engine::build::{BuildInput, Target};
use engine::data::GameData;
use engine::measure::{self, Fight};
use engine::popular::{self, HeroStat, ItemStat};

fn data() -> GameData {
    serde_json::from_str(include_str!("../../src-tauri/data/snapshot.json")).unwrap()
}

fn stats(json: &str) -> Vec<ItemStat> {
    serde_json::from_str(json).unwrap()
}

fn hero_matches(id: u64) -> u64 {
    let heroes: Vec<HeroStat> = serde_json::from_str(include_str!("popular/hero-stats.json")).unwrap();
    heroes.iter().find(|h| h.hero_id == id).unwrap().matches
}

fn item<'a>(d: &'a GameData, name: &str) -> &'a engine::data::Item {
    d.items.iter().find(|i| i.name == name).unwrap()
}

#[test]
fn a_fire_rate_slow_takes_more_off_heroes_whose_damage_comes_from_shooting() {
    let d = data();
    let me = Target { health: 3000.0, bullet_resist: 20.0, spirit_resist: 20.0 };
    let haze_build = popular::popular_build(&d, 13, hero_matches(13), &stats(include_str!("popular/items-haze.json"))).unwrap();
    let infernus_build = popular::popular_build(&d, 1, hero_matches(1), &stats(include_str!("popular/items-infernus.json"))).unwrap();
    let haze = Fight::new(&d, &measure::build_at(&haze_build, 30_000, &[2, 2, 2, 2]), &me).unwrap();
    let infernus = Fight::new(&d, &measure::build_at(&infernus_build, 30_000, &[2, 2, 2, 2]), &me).unwrap();
    let wraith_b = BuildInput { hero: "hero_wraith".into(), souls: Some(30_000), ability_tiers: vec![2, 2, 2, 2], ..Default::default() };
    let wraith = Fight::new(&d, &wraith_b, &me).unwrap();
    for name in ["Suppressor", "Rusted Barrel", "Metal Skin", "Bullet Resilience", "Spirit Sap", "Silencer", "Spirit Resilience", "Inhibitor", "Spellbreaker"] {
        let it = item(&d, name);
        let show = |f: &Fight| f.cut(&d, it).map(|c| format!("{:>4.0}%", c * 100.0)).unwrap_or("   -".into());
        println!("{name:<18} Haze {} Wraith {} Infernus {}", show(&haze), show(&wraith), show(&infernus));
    }
    let suppressor = item(&d, "Suppressor");
    let (h, i) = (haze.cut(&d, suppressor).unwrap(), infernus.cut(&d, suppressor).unwrap());
    assert!(h > i, "Suppressor should hurt Haze more than Infernus: {h} vs {i}");
    assert!(wraith.cut(&d, suppressor).unwrap() > 0.05);
    // An item that does nothing to damage is not measured.
    assert!(haze.cut(&d, item(&d, "Extra Health")).is_none());
}


