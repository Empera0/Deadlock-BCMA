use engine::{
    build::BuildInput,
    calc,
    combo::{self, Action, ComboInput, Kind},
    data::GameData,
};

fn data() -> GameData {
    serde_json::from_str(include_str!("../../src-tauri/data/snapshot.json")).unwrap()
}

fn build(items: &[&str]) -> BuildInput {
    build_for("hero_inferno", items)
}

fn build_for(hero: &str, items: &[&str]) -> BuildInput {
    BuildInput {
        hero: hero.into(),
        purchases: items.iter().map(|s| s.to_string()).collect(),
        ..Default::default()
    }
}

fn class_of(d: &GameData, name: &str) -> String {
    d.items.iter().find(|i| i.name == name).unwrap_or_else(|| panic!("{name} missing")).class_name.clone()
}

fn close(a: f64, b: f64) -> bool {
    (a - b).abs() < 1e-6 * b.abs().max(1.0)
}

fn source(r: &combo::ComboResult, name: &str) -> f64 {
    r.sources.iter().filter(|s| s.source == name).map(|s| s.damage).sum()
}

#[test]
fn shooting_matches_weapon_dps_before_the_first_reload() {
    let d = data();
    let b = build(&[]);
    let sheet = calc::compute(&d, &b).unwrap();
    let per_shot = sheet.stat("bullet_damage").unwrap();
    let combo = ComboInput { actions: vec![Action::Shoot { seconds: 1.0 }], headshot_rate: 0.0, ..Default::default() };
    let r = combo::simulate(&d, &b, &combo).unwrap();
    // First shot at t=0, then one every 1/sps seconds.
    let shots = (sheet.stat("shots_per_second").unwrap() * 1.0).ceil();
    assert!(close(r.total, per_shot * shots), "{} vs {}", r.total, per_shot * shots);
    assert!(close(r.duration, 1.0));
}

#[test]
fn shooting_reloads_when_the_magazine_is_empty() {
    let d = data();
    let b = build(&[]);
    let sheet = calc::compute(&d, &b).unwrap();
    let clip = sheet.stat("clip_size").unwrap();
    let empty_time = clip / sheet.stat("shots_per_second").unwrap();
    // Long enough to empty the magazine, too short to finish the reload.
    let seconds = empty_time + sheet.stat("reload_time").unwrap() * 0.5;
    let combo = ComboInput { actions: vec![Action::Shoot { seconds }], headshot_rate: 0.0, ..Default::default() };
    let r = combo::simulate(&d, &b, &combo).unwrap();
    assert!(close(source(&r, "Weapon"), clip * sheet.stat("bullet_damage").unwrap()));
}

#[test]
fn headshots_add_crit_damage_and_trigger_headshot_booster() {
    let d = data();
    let b = build(&[&class_of(&d, "Headshot Booster")]);
    let sheet = calc::compute(&d, &b).unwrap();
    let bonus = sheet.item_effects[0].props.iter().find(|p| p.key == "HeadShotBonusDamage").unwrap().value;
    let combo = ComboInput { actions: vec![Action::Shoot { seconds: 1.0 }], headshot_rate: 100.0, ..Default::default() };
    let r = combo::simulate(&d, &b, &combo).unwrap();
    // One proc: the cooldown is longer than the action.
    assert!(close(source(&r, "Headshot Booster"), bonus));
    let shots = sheet.stat("shots_per_second").unwrap().ceil();
    let weapon = sheet.stat("bullet_damage").unwrap() * sheet.stat("crit_multiplier").unwrap() * shots;
    assert!(close(source(&r, "Weapon"), weapon));
    assert!(combo::simulate(&d, &b, &ComboInput { headshot_rate: 0.0, ..combo.clone() })
        .unwrap()
        .sources
        .iter()
        .all(|s| s.source != "Headshot Booster"));
}

#[test]
fn ability_damage_is_reduced_by_spirit_resist_and_triggers_mystic_burst() {
    let d = data();
    let mut b = build(&[&class_of(&d, "Mystic Burst")]);
    b.target.spirit_resist = 25.0;
    let sheet = calc::compute(&d, &b).unwrap();
    let napalm = sheet.abilities[0].props.iter().find(|p| p.key == "Damage").unwrap().value;
    let burst = sheet.item_effects[0].props.iter().find(|p| p.key == "Damage").unwrap().value;
    let combo = ComboInput { actions: vec![Action::Ability { index: 0, duration: None, hits: None }], headshot_rate: 0.0, ..Default::default() };
    let r = combo::simulate(&d, &b, &combo).unwrap();
    assert!(close(source(&r, &sheet.abilities[0].name), napalm * 0.75));
    // Napalm's damage amp lands with the hit, so the proc that follows already benefits from it.
    let amp = sheet.abilities[0].props.iter().find(|p| p.key == "IncomingDamagePercentFromCaster").unwrap().value;
    assert!(close(source(&r, "Mystic Burst"), burst * 0.75 * (1.0 + amp / 100.0)));
    assert!(r.sources.iter().all(|s| s.kind == Kind::Spirit));
}

#[test]
fn damage_over_time_runs_for_its_full_duration() {
    let d = data();
    let b = build(&[]);
    let sheet = calc::compute(&d, &b).unwrap();
    // Flame Dash: DPS over AbilityDuration.
    let dash = &sheet.abilities[1];
    let dps = dash.props.iter().find(|p| p.key == "DPS").unwrap().value;
    let seconds = dash.props.iter().find(|p| p.key == "AbilityDuration").unwrap().value;
    let combo = ComboInput { actions: vec![Action::Ability { index: 1, duration: None, hits: None }], headshot_rate: 0.0, ..Default::default() };
    let r = combo::simulate(&d, &b, &combo).unwrap();
    assert!(close(r.total, dps * seconds), "{} vs {}", r.total, dps * seconds);
    assert!(r.full_duration > r.duration);

    let half = ComboInput { actions: vec![Action::Ability { index: 1, duration: Some(seconds / 2.0), hits: None }], headshot_rate: 0.0, ..Default::default() };
    assert!(close(combo::simulate(&d, &b, &half).unwrap().total, dps * seconds / 2.0));
}

#[test]
fn recasting_an_ability_waits_for_its_cooldown() {
    let d = data();
    let b = build(&[]);
    let sheet = calc::compute(&d, &b).unwrap();
    let cooldown = sheet.abilities[3].props.iter().find(|p| p.key == "AbilityCooldown").unwrap().value;
    let cast = Action::Ability { index: 3, duration: None, hits: None };
    let combo = ComboInput { actions: vec![cast.clone(), cast], headshot_rate: 0.0, ..Default::default() };
    let r = combo::simulate(&d, &b, &combo).unwrap();
    assert!(r.duration >= cooldown);
}

#[test]
fn kill_time_is_reported_when_the_target_dies() {
    let d = data();
    let mut b = build(&[]);
    b.target.health = 100.0;
    let combo = ComboInput { actions: vec![Action::Shoot { seconds: 5.0 }], headshot_rate: 0.0, ..Default::default() };
    let r = combo::simulate(&d, &b, &combo).unwrap();
    let kill = r.kill_time.expect("target should die");
    assert!(kill > 0.0 && kill < 5.0);
    b.target.health = 1e9;
    assert!(combo::simulate(&d, &b, &combo).unwrap().kill_time.is_none());
}

#[test]
fn active_items_are_listed_and_deal_their_damage() {
    let d = data();
    let item = class_of(&d, "Cold Front");
    let b = build(&[&item]);
    let sheet = calc::compute(&d, &b).unwrap();
    let damage = sheet.item_effects[0].props.iter().find(|p| p.key == "Damage").unwrap().value;
    let combo = ComboInput { actions: vec![Action::Item { item: item.clone() }], headshot_rate: 0.0, ..Default::default() };
    let r = combo::simulate(&d, &b, &combo).unwrap();
    assert_eq!(r.usable_items.len(), 1);
    assert!(close(r.total, damage));
}

#[test]
fn toxic_bullets_bleed_starts_after_enough_shots() {
    let d = data();
    let b = build(&[&class_of(&d, "Toxic Bullets")]);
    let sheet = calc::compute(&d, &b).unwrap();
    let toxic = &sheet.item_effects[0].props;
    let get = |key: &str| toxic.iter().find(|p| p.key == key).unwrap().value;
    let shots_needed = (100.0 / get("BuildUpPerShot")).ceil();
    let bleed_dps = get("DotHealthPercent") / 100.0 * b.target.health;

    let short = ComboInput { actions: vec![Action::Shoot { seconds: 1.0 }], headshot_rate: 0.0, ..Default::default() };
    assert!(sheet.stat("shots_per_second").unwrap() < shots_needed);
    assert_eq!(source(&combo::simulate(&d, &b, &short).unwrap(), "Toxic Bullets"), 0.0);

    // Applied once at the end of a long burst, the bleed then runs for its full duration.
    let long = ComboInput { actions: vec![Action::Shoot { seconds: 60.0 }], headshot_rate: 0.0, ..Default::default() };
    let r = combo::simulate(&d, &b, &long).unwrap();
    let bled = source(&r, "Toxic Bullets");
    assert!(bled >= bleed_dps * get("DotDuration") - 1e-6, "{bled}");
    assert!(bled <= bleed_dps * r.full_duration);
}

#[test]
fn spirit_damage_applies_bullet_resist_shredder_before_later_shots() {
    let d = data();
    let item = d.item(&class_of(&d, "Bullet Resist Shredder")).unwrap();
    let shred = item.props.iter().find(|p| p.key == "BulletArmorReduction").unwrap().value.abs();
    let mut b = build(&[&item.class_name]);
    b.target.bullet_resist = 20.0;
    b.target.health = 1e9;
    let sheet = calc::compute(&d, &b).unwrap();
    let amp = sheet.abilities[0].props.iter().find(|p| p.key == "IncomingDamagePercentFromCaster").unwrap().value;

    let shoot = Action::Shoot { seconds: 1.0 };
    let plain = combo::simulate(&d, &b, &ComboInput { actions: vec![shoot.clone()], headshot_rate: 0.0, ..Default::default() }).unwrap();
    let napalm = Action::Ability { index: 0, duration: None, hits: None };
    let shredded = combo::simulate(&d, &b, &ComboInput { actions: vec![napalm, shoot], headshot_rate: 0.0, ..Default::default() }).unwrap();

    // Napalm is spirit damage, so it applies the shred; it also adds its own damage amp.
    let expected = source(&plain, "Weapon") / 0.8 * (1.0 - (20.0 - shred) / 100.0) * (1.0 + amp / 100.0);
    assert!(close(source(&shredded, "Weapon"), expected), "{} vs {expected}", source(&shredded, "Weapon"));
    // Shooting first gets no benefit: the order of actions matters.
    assert!(close(source(&plain, "Weapon"), sheet.stat("bullet_damage").unwrap() * 0.8 * 10.0));
}

#[test]
fn rat_swarm_hits_repeatedly_and_each_bite_shreds_bullet_resist() {
    let d = data();
    let hero = d.heroes.iter().find(|h| h.name == "Rat King").unwrap();
    let mut b = build_for(&hero.class_name, &[]);
    b.target.health = 1e9;
    let sheet = calc::compute(&d, &b).unwrap();
    let index = sheet.abilities.iter().position(|a| a.name == "Rat Swarm").unwrap();
    let get = |key: &str| sheet.abilities[index].props.iter().find(|p| p.key == key).unwrap().value;
    let bites = (get("AbilityDuration") / get("BiteInterval") * get("RatsAttached")).floor();

    let swarm = Action::Ability { index, duration: None, hits: None };
    let r = combo::simulate(&d, &b, &ComboInput { actions: vec![swarm.clone()], headshot_rate: 0.0, ..Default::default() }).unwrap();
    assert!(close(r.total, bites * get("DamagePerBite")), "{} vs {}", r.total, bites * get("DamagePerBite"));

    // The hit count can be overridden, e.g. when only one rat reaches the target.
    let one_rat = Action::Ability { index, duration: None, hits: Some(bites / get("RatsAttached")) };
    let fewer = combo::simulate(&d, &b, &ComboInput { actions: vec![one_rat], headshot_rate: 0.0, ..Default::default() }).unwrap();
    assert!(close(fewer.total, bites / get("RatsAttached") * get("DamagePerBite")));

    // Bites lower bullet resist, so shots fired while the rats are biting hit harder.
    let shoot = Action::Shoot { seconds: 3.0 };
    let plain = combo::simulate(&d, &b, &ComboInput { actions: vec![shoot.clone()], headshot_rate: 0.0, ..Default::default() }).unwrap();
    let after = combo::simulate(&d, &b, &ComboInput { actions: vec![swarm, shoot], headshot_rate: 0.0, ..Default::default() }).unwrap();
    assert!(source(&after, "Weapon") > source(&plain, "Weapon") * 1.01);
}

fn healed(r: &combo::ComboResult, name: &str) -> f64 {
    r.heal_sources.iter().filter(|h| h.source == name).map(|h| h.amount).sum()
}

#[test]
fn lifesteal_returns_a_share_of_the_damage_and_sources_combine_multiplicatively() {
    let d = data();
    let b = build(&[&class_of(&d, "Bullet Lifesteal"), &class_of(&d, "Frenzy")]);
    let sheet = calc::compute(&d, &b).unwrap();
    let value = |name: &str| {
        let item = d.items.iter().find(|i| i.name == name).unwrap();
        item.props.iter().find(|p| p.key == "BulletLifestealPercent").unwrap().value / 100.0
    };
    let expected = 1.0 - (1.0 - value("Bullet Lifesteal")) * (1.0 - value("Frenzy"));
    assert!(close(sheet.stat("bullet_lifesteal").unwrap(), expected * 100.0));

    let combo = ComboInput { actions: vec![Action::Shoot { seconds: 1.0 }], ..Default::default() };
    let r = combo::simulate(&d, &b, &combo).unwrap();
    assert!(close(healed(&r, "Bullet lifesteal"), source(&r, "Weapon") * expected));
    // Spirit damage (Afterburn) is not covered by bullet lifesteal.
    assert_eq!(healed(&r, "Spirit lifesteal"), 0.0);
    assert!(close(r.healing, r.heal_sources.iter().map(|h| h.amount).sum()));
}

#[test]
fn a_used_heal_item_is_credited_in_full_and_boosted_by_healing_effectiveness() {
    let d = data();
    let rite = class_of(&d, "Healing Rite");
    let b = build(&[&rite]);
    let sheet = calc::compute(&d, &b).unwrap();
    let total = sheet.item_effects[0].props.iter().find(|p| p.key == "TotalHealthRegen").unwrap().value;
    let combo = ComboInput { actions: vec![Action::Item { item: rite.clone() }], ..Default::default() };
    let r = combo::simulate(&d, &b, &combo).unwrap();
    assert!(close(healed(&r, "Healing Rite"), total), "{} vs {total}", healed(&r, "Healing Rite"));
    // Health never goes above the maximum.
    assert!(r.own_health_end <= r.own_max_health + 1e-9);
    assert!(close(r.own_health_start, r.own_max_health * 0.5));

    let boosted = build(&[&rite, &class_of(&d, "Healing Booster")]);
    let amp = d.items.iter().find(|i| i.name == "Healing Booster").unwrap().props.iter().find(|p| p.key == "HealAmpCastPercent").unwrap().value;
    // More souls spent means more spirit power, so the base amount is read from the new build.
    let total = calc::compute(&d, &boosted).unwrap().item_effects[0].props.iter().find(|p| p.key == "TotalHealthRegen").unwrap().value;
    let r = combo::simulate(&d, &boosted, &combo).unwrap();
    assert!(close(healed(&r, "Healing Rite"), total * (1.0 + amp / 100.0)));
}

#[test]
fn abrams_siphon_life_heals_for_a_share_of_its_damage() {
    let d = data();
    let hero = d.heroes.iter().find(|h| h.name == "Abrams").unwrap();
    let mut b = build_for(&hero.class_name, &[]);
    b.target.health = 1e9;
    let sheet = calc::compute(&d, &b).unwrap();
    let index = sheet.abilities.iter().position(|a| a.name == "Siphon Life").unwrap();
    let factor = sheet.abilities[index].props.iter().find(|p| p.key == "HealingFactor").unwrap().value;
    let combo = ComboInput { actions: vec![Action::Ability { index, duration: None, hits: None }], ..Default::default() };
    let r = combo::simulate(&d, &b, &combo).unwrap();
    let dealt = source(&r, "Siphon Life");
    assert!(dealt > 0.0);
    assert!(close(healed(&r, "Siphon Life (damage to health)"), dealt * factor / 100.0));
}

#[test]
fn missing_health_heals_depend_on_the_starting_health() {
    let d = data();
    let blessing = class_of(&d, "Celestial Blessing");
    let mut b = build(&[&blessing]);
    b.souls = Some(50_000);
    let sheet = calc::compute(&d, &b).unwrap();
    assert!(sheet.errors.is_empty(), "{:?}", sheet.errors);
    let get = |key: &str| sheet.item_effects[0].props.iter().find(|p| p.key == key).unwrap().value;
    let run = |own_health: f64| {
        let combo = ComboInput { actions: vec![Action::Item { item: blessing.clone() }], own_health: Some(own_health), ..Default::default() };
        combo::simulate(&d, &b, &combo).unwrap()
    };
    // At full health only the minimum heal is left.
    assert!(close(healed(&run(100.0), "Celestial Blessing"), get("MinHeal")));
    let max = sheet.stat("max_health").unwrap();
    let low = (max * 0.9 * get("HealPercentAmount") / 100.0).max(get("MinHeal"));
    assert!(close(healed(&run(10.0), "Celestial Blessing"), low));
}

fn item_value(d: &GameData, item: &str, key: &str) -> f64 {
    let item = d.items.iter().find(|i| i.name == item).unwrap_or_else(|| panic!("{item} missing"));
    item.props.iter().find(|p| p.key == key).unwrap_or_else(|| panic!("{key} missing")).value
}

fn ability_value(sheet: &calc::Sheet, index: usize, key: &str) -> f64 {
    sheet.abilities[index].props.iter().find(|p| p.key == key).map(|p| p.value).unwrap_or(0.0)
}

#[test]
fn heroes_keep_their_own_base_spirit_resist() {
    let d = data();
    for (hero, expected) in [("hero_lash", 10.0), ("hero_synth", -15.0), ("hero_inferno", 0.0)] {
        let sheet = calc::compute(&d, &build_for(hero, &[])).unwrap();
        assert!(close(sheet.stat("spirit_resist").unwrap(), expected), "{hero}");
    }
}

#[test]
fn cooldown_reduction_is_always_on_and_combines_multiplicatively() {
    let d = data();
    let superior = item_value(&d, "Superior Cooldown", "CooldownReduction");
    let slinger = item_value(&d, "Spellslinger", "CooldownReduction");
    let plain = calc::compute(&d, &build(&[])).unwrap();
    // No switch needed: the item's cooldown reduction counts as soon as it is owned.
    let one = calc::compute(&d, &build(&[&class_of(&d, "Superior Cooldown")])).unwrap();
    assert!(close(one.stat("cooldown_reduction").unwrap(), superior));
    assert!(close(ability_value(&one, 0, "AbilityCooldown"), ability_value(&plain, 0, "AbilityCooldown") * (1.0 - superior / 100.0)));

    let two = calc::compute(&d, &build(&[&class_of(&d, "Superior Cooldown"), &class_of(&d, "Spellslinger")])).unwrap();
    let combined = 100.0 * (1.0 - (1.0 - superior / 100.0) * (1.0 - slinger / 100.0));
    assert!(close(two.stat("cooldown_reduction").unwrap(), combined));
}

#[test]
fn ultimate_and_item_cooldown_reductions_apply_to_their_own_cooldowns() {
    let d = data();
    let plain = calc::compute(&d, &build(&[])).unwrap();
    let kevlar = calc::compute(&d, &build(&[&class_of(&d, "Diviner's Kevlar")])).unwrap();
    let ult = item_value(&d, "Diviner's Kevlar", "UltimateCooldownReduction");
    assert!(close(ability_value(&kevlar, 3, "AbilityCooldown"), ability_value(&plain, 3, "AbilityCooldown") * (1.0 - ult / 100.0)));
    assert!(close(ability_value(&kevlar, 0, "AbilityCooldown"), ability_value(&plain, 0, "AbilityCooldown")));

    let rite = class_of(&d, "Healing Rite");
    let cooldown = |b: &BuildInput| {
        let sheet = calc::compute(&d, b).unwrap();
        let effects = sheet.item_effects.iter().find(|e| e.name == "Healing Rite").unwrap();
        effects.props.iter().find(|p| p.key == "AbilityCooldown").unwrap().value
    };
    let item_cdr = item_value(&d, "Transcendent Cooldown", "ItemCooldownReduction");
    let with = build(&[&rite, &class_of(&d, "Transcendent Cooldown")]);
    assert!(close(cooldown(&with), cooldown(&build(&[&rite])) * (1.0 - item_cdr / 100.0)));
}

#[test]
fn golden_goose_egg_lowers_all_damage_dealt() {
    let d = data();
    let penalty = item_value(&d, "Golden Goose Egg", "OutgoingDamagePenaltyPercent");
    assert!(penalty < 0.0);
    let mut egg = build(&[&class_of(&d, "Golden Goose Egg")]);
    egg.target.health = 1e9;
    let sheet = calc::compute(&d, &egg).unwrap();
    assert!(close(sheet.stat("outgoing_damage_pct").unwrap(), penalty));
    let combo = ComboInput { actions: vec![Action::Shoot { seconds: 1.0 }], ..Default::default() };
    let r = combo::simulate(&d, &egg, &combo).unwrap();
    let shots = sheet.stat("shots_per_second").unwrap().ceil();
    let expected = sheet.stat("bullet_damage").unwrap() * shots * (1.0 + penalty / 100.0);
    assert!(close(source(&r, "Weapon"), expected), "{} vs {expected}", source(&r, "Weapon"));
}

#[test]
fn charged_abilities_spend_charges_before_waiting_for_the_cooldown() {
    let d = data();
    let mut b = build_for("hero_astro", &[]);
    b.target.health = 1e9;
    let sheet = calc::compute(&d, &b).unwrap();
    let index = sheet.abilities.iter().position(|a| a.name == "Powder Keg").unwrap();
    let (charges, between, cooldown) = (
        ability_value(&sheet, index, "AbilityCharges"),
        ability_value(&sheet, index, "AbilityCooldownBetweenCharge"),
        ability_value(&sheet, index, "AbilityCooldown"),
    );
    assert_eq!(charges, 2.0);
    let casts = |b: &BuildInput, count: usize| {
        let actions = vec![Action::Ability { index, duration: None, hits: None }; count];
        combo::simulate(&d, b, &ComboInput { actions, ..Default::default() }).unwrap().duration
    };
    // The second charge only waits for the time between charges; the third cast needs the first charge back.
    assert!(casts(&b, 2) < between + 1.0);
    assert!(casts(&b, 3) >= cooldown);

    // Extra Charge adds a third charge, so three casts no longer wait for the cooldown.
    let mut extra = build_for("hero_astro", &[&class_of(&d, "Extra Charge")]);
    extra.target.health = 1e9;
    let sheet = calc::compute(&d, &extra).unwrap();
    assert_eq!(ability_value(&sheet, index, "AbilityCharges"), 3.0);
    assert!(casts(&extra, 3) < cooldown);
}

#[test]
fn omnicharge_signet_gives_its_ability_charges_and_boosts_only_charged_abilities() {
    let d = data();
    let signet = class_of(&d, "Omnicharge Signet");
    let (more, fresh) = (item_value(&d, "Omnicharge Signet", "BonusAbilityCharges"), item_value(&d, "Omnicharge Signet", "BonusAbilityChargesNonCharge"));
    let spirit = item_value(&d, "Omnicharge Signet", "BonusSpiritForChargedAbilities");

    // Holliday's Powder Keg already has two charges; Bounce Pad has none to speak of.
    let mut b = build_for("hero_astro", &[&signet]);
    b.souls = Some(60_000);
    let plain = calc::compute(&d, &b).unwrap();
    assert!(plain.errors.is_empty(), "{:?}", plain.errors);
    let keg = plain.abilities.iter().position(|a| a.name == "Powder Keg").unwrap();
    let other = (0..3).find(|i| *i != keg && ability_value(&plain, *i, "AbilityCharges") < 2.0).unwrap();
    // Not imbued yet: nothing changes.
    assert_eq!(ability_value(&plain, keg, "AbilityCharges"), 2.0);

    b.imbues.insert(signet.clone(), keg);
    let charged = calc::compute(&d, &b).unwrap();
    assert_eq!(ability_value(&charged, keg, "AbilityCharges"), 2.0 + more);

    b.imbues.insert(signet.clone(), other);
    let fresh_sheet = calc::compute(&d, &b).unwrap();
    assert_eq!(ability_value(&fresh_sheet, other, "AbilityCharges"), 1.0 + fresh);
    assert!(ability_value(&fresh_sheet, other, "AbilityCooldownBetweenCharge") > 0.0);
    // The charged-ability bonuses shorten its cooldown too.
    assert!(ability_value(&fresh_sheet, other, "AbilityCooldown") < ability_value(&plain, other, "AbilityCooldown"));
    assert!(spirit > 0.0);
}

#[test]
fn solomon_s_pieces_deal_their_damage() {
    let d = data();
    let mut b = build_for("hero_chessmaster", &[]);
    b.target.health = 1e9;
    b.ability_tiers = vec![3, 3, 3, 3];
    b.souls = Some(40_000);
    let sheet = calc::compute(&d, &b).unwrap();
    let index = |name: &str| sheet.abilities.iter().position(|a| a.name == name).unwrap();
    let cast = |name: &str, duration: Option<f64>, hits: Option<f64>| {
        let actions = vec![Action::Ability { index: index(name), duration, hits }];
        combo::simulate(&d, &b, &ComboInput { actions, ..Default::default() }).unwrap()
    };

    // Bishop: the landing, one pass, and a second on the squares (four ticks, each stripping spirit resist).
    let bishop = index("Setup Bishop");
    let (impact, pass, dps) = (ability_value(&sheet, bishop, "ImpactDamage"), ability_value(&sheet, bishop, "Damage"), ability_value(&sheet, bishop, "DPS"));
    let r = cast("Setup Bishop", Some(1.0), None);
    let flat = impact + pass + dps;
    assert!(r.total > flat && r.total < flat * 1.1, "{} vs {flat}", r.total);

    // Rooks: damage plus 5% of current health per hit with the upgrade; two hits deal more than one.
    let rook = index("Rook Battery");
    assert!(ability_value(&sheet, rook, "CurrentHealthPercent") > 0.0);
    let one = cast("Rook Battery", None, None).total;
    assert!(close(one, ability_value(&sheet, rook, "Damage") + 0.05 * 1e9));
    assert!(cast("Rook Battery", None, Some(2.0)).total > one * 1.9);

    // Queen: one landing plus her squares; more moves hit again.
    let queen = index("Develop Queen");
    let landing = ability_value(&sheet, queen, "Damage");
    let r = cast("Develop Queen", Some(2.0), None);
    assert!(close(r.total, landing + 2.0 * ability_value(&sheet, queen, "DPS")), "{}", r.total);
    assert!(cast("Develop Queen", Some(2.0), Some(3.0)).total > r.total + landing * 1.9);

    // The knight is a buff for allies only.
    assert_eq!(cast("Kingside Knight", None, None).total, 0.0);
}

#[test]
fn deadman_danny_s_kit_deals_its_damage() {
    let d = data();
    let mut b = build_for("hero_deadpack", &[]);
    b.target.health = 1e9;
    b.ability_tiers = vec![3, 3, 3, 3];
    b.souls = Some(30_000);
    let sheet = calc::compute(&d, &b).unwrap();
    let index = |name: &str| sheet.abilities.iter().position(|a| a.name == name).unwrap();
    let cast = |name: &str, hits: Option<f64>| {
        let actions = vec![Action::Ability { index: index(name), duration: None, hits }];
        combo::simulate(&d, &b, &ComboInput { actions, ..Default::default() }).unwrap()
    };

    // Strix Salvo: a missile and its mark per hit; more missiles landing deal more.
    let salvo = index("Strix Salvo");
    let one = ability_value(&sheet, salvo, "Damage") + ability_value(&sheet, salvo, "CoatedDamage");
    let r = cast("Strix Salvo", None);
    assert!(r.total > 0.0 && r.total <= one * 1.05, "{} vs {one}", r.total);
    assert!(cast("Strix Salvo", Some(3.0)).total > r.total * 2.5);

    // Close The Deal: the fall is worth far more than a single meter of it.
    let ult = index("Close The Deal");
    let per_meter = ability_value(&sheet, ult, "DamagePerMeter");
    let r = cast("Close The Deal", None);
    assert!(r.total > ability_value(&sheet, ult, "Damage") + per_meter * 30.0, "{}", r.total);

    // Chimera Mk1: both parts of the explosion, and the upgrade heals on a hero hit.
    let r = cast("Chimera Mk1", None);
    assert!(r.total > ability_value(&sheet, index("Chimera Mk1"), "ExplosionDamage"));
    assert!(r.healing > 0.0);
}
