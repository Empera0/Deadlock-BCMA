use engine::{build::BuildInput, calc, data::GameData};

fn data() -> GameData {
    serde_json::from_str(include_str!("../../src-tauri/data/snapshot.json")).unwrap()
}

fn build(hero: &str, items: &[&str]) -> BuildInput {
    BuildInput {
        hero: hero.into(),
        purchases: items.iter().map(|s| s.to_string()).collect(),
        ..Default::default()
    }
}

fn close(a: f64, b: f64) -> bool {
    (a - b).abs() < 1e-6 * b.abs().max(1.0)
}

const INFERNUS: &str = "hero_inferno";

#[test]
fn base_stats_match_game_data() {
    let d = data();
    let hero = d.hero(INFERNUS).unwrap();
    let s = calc::compute(&d, &build(INFERNUS, &[])).unwrap();
    assert_eq!((s.level, s.boons, s.spent), (1, 0, 0));
    assert!(close(s.stat("bullet_damage").unwrap(), hero.weapon.bullet_damage));
    assert!(close(s.stat("max_health").unwrap(), hero.base["max_health"]));
    assert!(close(
        s.stat("dps_burst").unwrap(),
        hero.weapon.bullet_damage * hero.weapon.pellets * hero.weapon.shots_per_second
    ));
    assert_eq!(s.stat("spirit_power").unwrap(), 0.0);
}

#[test]
fn souls_set_level_and_boons() {
    let d = data();
    let hero = d.hero(INFERNUS).unwrap();
    let last = hero.levels.last().unwrap();
    let mut b = build(INFERNUS, &[]);
    b.souls = Some(last.required_souls);
    let s = calc::compute(&d, &b).unwrap();
    assert_eq!(s.level, last.level);
    let boons = hero.levels.iter().filter(|l| l.boon).count() as f64;
    let hp = hero.base["max_health"] + boons * hero.level_up["MODIFIER_VALUE_BASE_HEALTH_FROM_LEVEL"];
    assert!(close(s.stat("max_health").unwrap(), hp));
}

#[test]
fn component_upgrade_refunds_component_cost() {
    let d = data();
    let upgrade = d.items.iter().find(|i| !i.components.is_empty()).unwrap();
    let comp = d.item(&upgrade.components[0]).unwrap();
    let s = calc::compute(&d, &build(INFERNUS, &[&comp.class_name, &upgrade.class_name])).unwrap();
    assert_eq!(s.spent, upgrade.cost);
    assert_eq!(s.steps[1].paid, upgrade.cost - comp.cost);
    assert_eq!(s.owned, vec![upgrade.class_name.clone()]);
}

#[test]
fn resists_stack_multiplicatively() {
    let d = data();
    let innate = |i: &engine::data::Item| {
        i.props
            .iter()
            .filter(|p| p.section == "innate" && p.modifier.as_deref() == Some("MODIFIER_VALUE_BULLET_ARMOR_DAMAGE_RESIST"))
            .map(|p| p.value)
            .sum::<f64>()
    };
    let picks: Vec<_> = d.items.iter().filter(|i| innate(i) > 0.0 && i.components.is_empty()).take(2).collect();
    assert_eq!(picks.len(), 2);
    let s = calc::compute(&d, &build(INFERNUS, &[&picks[0].class_name, &picks[1].class_name])).unwrap();
    let expected = 100.0 * (1.0 - (1.0 - innate(picks[0]) / 100.0) * (1.0 - innate(picks[1]) / 100.0));
    assert!(close(s.stat("bullet_resist").unwrap(), expected));
    assert!(expected < innate(picks[0]) + innate(picks[1]));
}

#[test]
fn spirit_power_scales_ability_damage() {
    let d = data();
    let hero = d.hero(INFERNUS).unwrap();
    let s = calc::compute(&d, &build(INFERNUS, &["upgrade_improved_spirit"])).unwrap();
    let sp = s.stat("spirit_power").unwrap();
    // Level boons + Extra Spirit + the 800-soul spirit investment bonus
    let item_sp: f64 = d.item("upgrade_improved_spirit").unwrap().props.iter()
        .filter(|p| p.modifier.as_deref() == Some("MODIFIER_VALUE_TECH_POWER")).map(|p| p.value).sum();
    let level_sp = s.boons as f64 * hero.level_up["MODIFIER_VALUE_TECH_POWER"];
    assert!(s.boons > 0);
    assert!(close(sp, level_sp + item_sp + hero.invest.spirit[0].bonus));
    let raw = hero.abilities[0].props.iter().find(|p| p.key == "Damage").unwrap();
    let got = s.abilities[0].props.iter().find(|p| p.key == "Damage").unwrap().value;
    assert!(close(got, raw.value + raw.scale.as_ref().unwrap().coef * sp));
}

#[test]
fn ranking_is_sorted_and_skips_owned() {
    let d = data();
    let b = build(INFERNUS, &["upgrade_headshot_booster"]);
    let ranked = calc::rank_next(&d, &b, "dps_sustained", false).unwrap();
    assert!(!ranked.is_empty());
    assert!(ranked.windows(2).all(|w| w[0].per_1k >= w[1].per_1k));
    assert!(ranked.iter().all(|r| r.item != "upgrade_headshot_booster"));
}

#[test]
fn buildup_per_shot_matches_published_per_hero_values() {
    // Toxic Bullets build-up per shot (percent), from the deadlock.wiki per-hero table.
    let expected = [
        ("Infernus", 7.4),
        ("Haze", 7.4),
        ("Vyper", 5.1),
        ("Grey Talon", 29.3),
        ("Abrams", 3.4 * 9.0),
        ("McGinnis", 6.1),
        ("Paige", 26.0),
        ("The Doorman", 30.2),
        ("Rat King", 6.5 * 2.0),
    ];
    let d = data();
    let toxic = d.items.iter().find(|i| i.name == "Toxic Bullets").unwrap().class_name.clone();
    for (hero, per_shot) in expected {
        let class = &d.heroes.iter().find(|h| h.name == hero).unwrap().class_name;
        let s = calc::compute(&d, &build(class, &[&toxic])).unwrap();
        let got = s.item_effects[0].props.iter().find(|p| p.key == "BuildUpPerShot").unwrap().value;
        assert!((got - per_shot).abs() < 0.03 * per_shot, "{hero}: {got} vs {per_shot}");
    }
}

#[test]
fn base_stats_that_scale_with_spirit_power_grow_with_it() {
    let d = data();
    let hero = |name: &str| d.heroes.iter().find(|h| h.name == name).unwrap_or_else(|| panic!("{name} missing"));
    let sheet = |name: &str, souls: u32, items: &[&str]| {
        let b = BuildInput {
            hero: hero(name).class_name.clone(),
            souls: Some(souls),
            purchases: items.iter().map(|i| d.items.iter().find(|x| x.name == *i).unwrap().class_name.clone()).collect(),
            ..Default::default()
        };
        calc::compute(&d, &b).unwrap()
    };
    // Solomon's bullets gain 0.14 damage per point of spirit power, before weapon damage %.
    let solomon = hero("Solomon");
    let s = sheet("Solomon", 20_000, &["Extra Spirit"]);
    let spirit = s.stat("spirit_power").unwrap();
    assert!(spirit > 10.0);
    let boons = s.boons as f64;
    let expected = (solomon.weapon.bullet_damage + boons * solomon.level_up["MODIFIER_VALUE_BASE_BULLET_DAMAGE_FROM_LEVEL"] + 0.14 * spirit)
        * (1.0 + s.stat("weapon_damage_pct").unwrap() / 100.0);
    assert!(close(s.stat("bullet_damage").unwrap(), expected), "{} vs {expected}", s.stat("bullet_damage").unwrap());
    assert!(close(solomon.spirit_scaling["EBulletDamage"], 0.14));

    // Haze gains half a bullet of magazine per point (whole bullets only); Warden gains fire rate.
    let haze = sheet("Haze", 20_000, &["Extra Spirit"]);
    let extra = (0.5 * haze.stat("spirit_power").unwrap()).floor();
    assert!(extra >= 1.0);
    assert!(close(haze.stat("clip_size").unwrap(), hero("Haze").weapon.clip_size + extra));
    let warden = sheet("Warden", 20_000, &["Extra Spirit"]);
    assert!(close(warden.stat("fire_rate_pct").unwrap(), 0.21 * warden.stat("spirit_power").unwrap()));
    // A hero without any scaling is untouched.
    assert!(hero("Infernus").spirit_scaling.is_empty());
}

fn class_of(d: &GameData, name: &str) -> String {
    d.items.iter().find(|i| i.name == name).unwrap_or_else(|| panic!("{name} missing")).class_name.clone()
}

#[test]
fn fire_rate_slows_lengthen_the_time_between_shots() {
    // A bonus raises the shots per second; a penalty lengthens the time between shots, at most by half.
    assert!(close(calc::fire_rate(2.0, 25.0, 0.0), 2.5));
    assert!(close(calc::fire_rate(2.0, 0.0, 25.0), 2.0 / 1.25));
    assert!(close(calc::fire_rate(2.0, 10.0, 80.0), 2.0 / 1.5));
    assert!(close(calc::fire_rate(2.0, 30.0, 10.0), 2.0 * 1.2));
}

#[test]
fn the_magazine_rounds_up_after_the_percentage_bonus() {
    let d = data();
    let hero = d.hero(INFERNUS).unwrap();
    let magazine = class_of(&d, "Extended Magazine");
    let s = calc::compute(&d, &build(INFERNUS, &[&magazine])).unwrap();
    let pct = d.item(&magazine).unwrap().props.iter().find(|p| p.key == "BonusClipSizePercent").unwrap().value;
    assert!(close(s.stat("clip_size").unwrap(), (hero.weapon.clip_size * (1.0 + pct / 100.0) - 1e-9).ceil()));
}

#[test]
fn melee_gets_half_the_weapon_damage_bonus() {
    let d = data();
    // The same souls for both, so the level is the same.
    let plain = calc::compute(&d, &BuildInput { souls: Some(800), ..build(INFERNUS, &[]) }).unwrap();
    let rounds = class_of(&d, "High-Velocity Rounds");
    let s = calc::compute(&d, &BuildInput { souls: Some(800), ..build(INFERNUS, &[&rounds]) }).unwrap();
    let wd = s.stat("weapon_damage_pct").unwrap();
    assert!(wd > 0.0);
    assert!(close(s.stat("light_melee").unwrap(), plain.stat("light_melee").unwrap() * (1.0 + 0.5 * wd / 100.0)));
}

#[test]
fn map_buffs_raise_the_stats() {
    let d = data();
    let plain = calc::compute(&d, &build(INFERNUS, &[])).unwrap();
    let mut b = build(INFERNUS, &[]);
    b.map = engine::build::MapBuffs { minute: 40.0, casting: true, gun: true, statues: engine::build::Statues { health: 2, ..Default::default() } };
    let s = calc::compute(&d, &b).unwrap();
    // At minute 40 the powerups are at full strength; statue buffs from minute 30 are at their third level.
    assert!(close(s.stat("spirit_power").unwrap() - plain.stat("spirit_power").unwrap(), 65.0));
    assert!(close(s.stat("fire_rate_pct").unwrap() - plain.stat("fire_rate_pct").unwrap(), 35.0));
    assert!(close(s.stat("max_health").unwrap() - plain.stat("max_health").unwrap(), 60.0));
}
