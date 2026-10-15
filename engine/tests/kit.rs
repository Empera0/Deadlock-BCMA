//! Hero-specific ability behavior in the combo simulation.

use engine::{
    build::BuildInput,
    calc,
    combo::{self, Action, ComboInput, ComboResult},
    data::GameData,
};

fn data() -> GameData {
    serde_json::from_str(include_str!("../../src-tauri/data/snapshot.json")).unwrap()
}

fn hero(d: &GameData, name: &str) -> BuildInput {
    let class = d.heroes.iter().find(|h| h.name == name).unwrap_or_else(|| panic!("{name} missing")).class_name.clone();
    let mut b = BuildInput { hero: class, ..Default::default() };
    b.target.health = 1e9;
    b
}

fn ability(d: &GameData, b: &BuildInput, name: &str) -> (usize, calc::AbilitySheet) {
    let sheet = calc::compute(d, b).unwrap();
    let index = sheet.abilities.iter().position(|a| a.name == name).unwrap_or_else(|| panic!("{name} missing"));
    (index, sheet.abilities[index].clone())
}

fn value(a: &calc::AbilitySheet, key: &str) -> f64 {
    a.props.iter().find(|p| p.key == key).unwrap_or_else(|| panic!("{key} missing")).value
}

fn run(d: &GameData, b: &BuildInput, actions: Vec<Action>, headshot_rate: f64) -> ComboResult {
    combo::simulate(d, b, &ComboInput { actions, headshot_rate, ..Default::default() }).unwrap()
}

fn cast(index: usize) -> Action {
    Action::Ability { index, duration: None, hits: None }
}

fn source(r: &ComboResult, name: &str) -> f64 {
    r.sources.iter().filter(|s| s.source == name).map(|s| s.damage).sum()
}

fn close(a: f64, b: f64) -> bool {
    (a - b).abs() < 1e-6 * b.abs().max(1.0)
}

#[test]
fn every_hero_and_ability_simulates_without_errors() {
    let d = data();
    for h in &d.heroes {
        for tier in [0u8, 3] {
            let mut b = BuildInput { hero: h.class_name.clone(), ability_tiers: vec![tier; 4], ..Default::default() };
            b.target.health = 5000.0;
            let mut actions: Vec<Action> = (0..4).map(cast).collect();
            actions.push(Action::Shoot { seconds: 3.0 });
            actions.push(Action::Melee { heavy: true });
            let r = run(&d, &b, actions, 25.0);
            assert!(r.total.is_finite() && r.total > 0.0, "{} tier {tier}: {}", h.name, r.total);
            assert_eq!(r.abilities.len(), 4, "{}", h.name);
            assert!(r.abilities.iter().all(|a| !a.lines.is_empty()), "{}", h.name);
        }
    }
}

#[test]
fn haze_fixation_adds_bullet_damage_with_every_shot() {
    let d = data();
    let b = hero(&d, "Haze");
    let sheet = calc::compute(&d, &b).unwrap();
    let (_, fixation) = ability(&d, &b, "Fixation");
    let per_stack = value(&fixation, "DamageBonusFixedPerStack");
    let r = run(&d, &b, vec![Action::Shoot { seconds: 1.0 }], 0.0);
    // Shot k has k-1 stacks: the stack from a hit counts from the next shot.
    let shots = sheet.stat("shots_per_second").unwrap().ceil();
    let bonus = per_stack * shots * (shots - 1.0) / 2.0;
    assert!(close(source(&r, "Weapon"), sheet.stat("bullet_damage").unwrap() * shots + bonus));
}

#[test]
fn holliday_crackshot_procs_on_headshots_only() {
    let d = data();
    let b = hero(&d, "Holliday");
    let (_, crackshot) = ability(&d, &b, "Crackshot");
    let shoot = vec![Action::Shoot { seconds: 1.0 }];
    assert_eq!(source(&run(&d, &b, shoot.clone(), 0.0), "Crackshot"), 0.0);
    // One proc: the cooldown is longer than the action.
    assert!(close(source(&run(&d, &b, shoot, 100.0), "Crackshot"), value(&crackshot, "Damage")));
}

#[test]
fn infernus_afterburn_starts_once_the_buildup_is_full() {
    let d = data();
    let b = hero(&d, "Infernus");
    let sheet = calc::compute(&d, &b).unwrap();
    let (_, afterburn) = ability(&d, &b, "Afterburn");
    let shots_needed = (100.0 / value(&afterburn, "BuildUpBulletPercentPerHit")).ceil();
    let rate = sheet.stat("shots_per_second").unwrap();
    // Just short of the last needed shot: no burn yet.
    let short = run(&d, &b, vec![Action::Shoot { seconds: (shots_needed - 1.5) / rate }], 0.0);
    assert_eq!(source(&short, "Afterburn"), 0.0);
    let long = run(&d, &b, vec![Action::Shoot { seconds: (shots_needed + 5.0) / rate }], 0.0);
    assert!(source(&long, "Afterburn") > 0.0);
}

#[test]
fn calico_gloom_bombs_after_the_first_deal_reduced_damage() {
    let d = data();
    let b = hero(&d, "Calico");
    let (index, bombs) = ability(&d, &b, "Gloom Bombs");
    let (damage, count) = (value(&bombs, "Damage"), value(&bombs, "GrenadeCount"));
    let r = run(&d, &b, vec![cast(index)], 0.0);
    assert!(close(r.total, damage * (1.0 + 0.65 * (count - 1.0))));
    let one = run(&d, &b, vec![Action::Ability { index, duration: None, hits: Some(1.0) }], 0.0);
    assert!(close(one.total, damage));
}

#[test]
fn billy_blasted_makes_melee_hits_amplify_later_bullets() {
    let d = data();
    let b = hero(&d, "Billy");
    let (_, blasted) = ability(&d, &b, "Blasted");
    let amp = value(&blasted, "BulletDamageAmp");
    let shoot = Action::Shoot { seconds: 1.0 };
    let plain = run(&d, &b, vec![shoot.clone()], 0.0);
    let after_melee = run(&d, &b, vec![Action::Melee { heavy: false }, shoot], 0.0);
    assert!(close(source(&after_melee, "Weapon"), source(&plain, "Weapon") * (1.0 + amp / 100.0)));
}

#[test]
fn vyper_venom_grows_with_missing_health() {
    let d = data();
    let mut b = hero(&d, "Vyper");
    let (index, venom) = ability(&d, &b, "Lethal Venom");
    let (min, max) = (value(&venom, "VenomMinDamage"), value(&venom, "VenomMaxDamage"));
    assert!(close(run(&d, &b, vec![cast(index)], 0.0).total, min));

    // Shoot the target down to about 20% health first: the venom then deals its maximum.
    let sheet = calc::compute(&d, &b).unwrap();
    let per_second = sheet.stat("bullet_damage").unwrap() * sheet.stat("shots_per_second").unwrap();
    b.target.health = per_second * 1.0 / 0.8;
    let r = run(&d, &b, vec![Action::Shoot { seconds: 1.0 }, cast(index)], 0.0);
    assert!(close(source(&r, "Lethal Venom"), max), "{} vs {max}", source(&r, "Lethal Venom"));
}

#[test]
fn mirage_marks_double_the_damage_per_stack() {
    let d = data();
    let b = hero(&d, "Mirage");
    let (index, mark) = ability(&d, &b, "Djinn's Mark");
    let base = value(&mark, "ProcDamageBase");
    let with = |marks: f64| run(&d, &b, vec![Action::Ability { index, duration: None, hits: Some(marks) }], 0.0).total;
    assert!(close(with(1.0), base));
    assert!(close(with(3.0), base * 4.0));
    assert!(close(run(&d, &b, vec![cast(index)], 0.0).total, base * 8.0));
}

#[test]
fn drifter_mark_bleeds_a_share_of_current_health() {
    let d = data();
    let mut b = hero(&d, "Drifter");
    b.target.health = 1000.0;
    let (index, mark) = ability(&d, &b, "Stalker's Mark");
    let (pct, seconds) = (value(&mark, "DotHealthPercent"), value(&mark, "AbilityDuration"));
    let r = run(&d, &b, vec![cast(index)], 0.0);
    // Current-health damage decays exponentially: health × (1 − e^(−rate × time)).
    let expected = 1000.0 * (1.0 - (-pct / 100.0 * seconds).exp());
    assert!((r.total - expected).abs() < 0.01 * expected, "{} vs {expected}", r.total);
}

#[test]
fn vindicta_flight_adds_spirit_damage_to_shots() {
    let d = data();
    let b = hero(&d, "Vindicta");
    let sheet = calc::compute(&d, &b).unwrap();
    let (index, flight) = ability(&d, &b, "Flight");
    let per_bullet = value(&flight, "MagicDamagePerBullet");
    let r = run(&d, &b, vec![cast(index), Action::Shoot { seconds: 1.0 }], 0.0);
    let shots = sheet.stat("shots_per_second").unwrap().ceil();
    assert!(close(source(&r, "Weapon (bonus spirit damage)"), per_bullet * shots));
}

#[test]
fn baba_heavy_melee_gains_spirit_damage_while_granny_long_legs_is_active() {
    let d = data();
    let b = hero(&d, "Baba");
    let (index, legs) = ability(&d, &b, "Granny Long Legs");
    let bonus = value(&legs, "HeavyMeleeSpiritDamage");
    let heavy = Action::Melee { heavy: true };
    let plain = run(&d, &b, vec![heavy.clone()], 0.0);
    let buffed = run(&d, &b, vec![cast(index), heavy, Action::Melee { heavy: false }], 0.0);
    assert_eq!(source(&plain, "Heavy melee (bonus spirit damage)"), 0.0);
    // Only the heavy hit carries the bonus.
    assert!(close(source(&buffed, "Heavy melee (bonus spirit damage)"), bonus));
}

#[test]
fn percentage_upgrades_multiply_the_base_value() {
    let d = data();
    let lash = d.heroes.iter().find(|h| h.name == "Lash").unwrap();
    let base = BuildInput { hero: lash.class_name.clone(), ..Default::default() };
    let maxed = BuildInput { ability_tiers: vec![3, 0, 0, 0], ..base.clone() };
    let per_meter = |b: &BuildInput| value(&ability(&d, b, "Ground Strike").1, "StompDamagePerMeterPrimary");
    // Ground Strike's last upgrade is +120% damage per meter (spirit power is 0 at level 1).
    assert!(close(per_meter(&maxed), per_meter(&base) * 2.2), "{} vs {}", per_meter(&maxed), per_meter(&base) * 2.2);
}

#[test]
fn viscous_splatter_repeat_hits_fall_off() {
    let d = data();
    let b = hero(&d, "Viscous");
    let (index, splatter) = ability(&d, &b, "Splatter");
    let damage = value(&splatter, "Damage");
    let with = |hits: f64| run(&d, &b, vec![Action::Ability { index, duration: None, hits: Some(hits) }], 0.0).total;
    assert!(close(run(&d, &b, vec![cast(index)], 0.0).total, damage));
    assert!(close(with(3.0), damage * (1.0 + 0.5 + 0.38)));
}

#[test]
fn lash_ground_strike_adds_damage_per_meter_of_height() {
    let d = data();
    let b = hero(&d, "Lash");
    let (index, strike) = ability(&d, &b, "Ground Strike");
    let (stomp, primary, secondary) =
        (value(&strike, "StompDamage"), value(&strike, "StompDamagePerMeterPrimary"), value(&strike, "StompDamagePerMeterSecondary"));
    let from = |meters: f64| run(&d, &b, vec![Action::Ability { index, duration: None, hits: Some(meters) }], 0.0).total;
    assert!(close(from(10.0), stomp + 10.0 * primary));
    assert!(close(from(30.0), stomp + 25.0 * primary + 5.0 * secondary));
}

#[test]
fn graves_ghouls_explode_one_by_one_and_trigger_item_effects() {
    let d = data();
    let mut b = hero(&d, "Graves");
    let (index, decree) = ability(&d, &b, "Borrowed Decree");
    let damage = value(&decree, "Damage");
    let spawned = ((value(&decree, "AbilityDuration") - value(&decree, "SummonInitialDelay")) / value(&decree, "SummonFrequency")).floor() + 1.0;
    let spawned = spawned * value(&decree, "SummonBurstCount");
    let all = run(&d, &b, vec![cast(index)], 0.0);
    assert!(close(source(&all, "Borrowed Decree"), damage * spawned), "{} vs {}", all.total, damage * spawned);

    // Only some ghouls reach the target.
    let three = Action::Ability { index, duration: None, hits: Some(3.0) };
    assert!(close(run(&d, &b, vec![three.clone()], 0.0).total, damage * 3.0));

    // Each explosion is spirit damage, so it applies Bullet Resist Shredder for the shots that follow.
    let shredder = d.items.iter().find(|i| i.name == "Bullet Resist Shredder").unwrap();
    b.purchases = vec![shredder.class_name.clone()];
    b.target.bullet_resist = 30.0;
    let shoot = Action::Shoot { seconds: 2.0 };
    let plain = run(&d, &b, vec![shoot.clone()], 0.0);
    let after = run(&d, &b, vec![three, Action::Wait { seconds: 6.0 }, shoot], 0.0);
    assert!(source(&after, "Weapon") > source(&plain, "Weapon") * 1.05);
    assert!(after.events.iter().any(|e| e.source == "Bullet Resist Shredder"));
}

#[test]
fn graves_essence_theft_builds_weapon_damage_while_shooting() {
    let d = data();
    let b = hero(&d, "Graves");
    let sheet = calc::compute(&d, &b).unwrap();
    let (_, theft) = ability(&d, &b, "Essence Theft");
    let (rate, bullet) = (sheet.stat("shots_per_second").unwrap(), sheet.stat("bullet_damage").unwrap());
    let shots_to_max = (value(&theft, "ShootDurationForMax") * rate).round();
    let r = run(&d, &b, vec![Action::Shoot { seconds: 1.0 }], 0.0);
    // Shot k has k-1 stacks of the stolen weapon damage.
    let shots = rate.ceil();
    let per_stack = value(&theft, "MaxStolenAttackDamage") / shots_to_max / 100.0;
    let expected = bullet * (shots + per_stack * shots * (shots - 1.0) / 2.0);
    assert!(close(source(&r, "Weapon"), expected), "{} vs {expected}", source(&r, "Weapon"));
}

#[test]
fn wraith_cards_come_from_weapon_hits_and_use_the_average_of_the_four_suits() {
    let d = data();
    let b = hero(&d, "Wraith");
    let (_, card) = ability(&d, &b, "Card Trick");
    // A card every ResourcePerCard of resource, CardResourcePerBulletHit a hit: shoot just long enough for one.
    let hits_per_card = value(&card, "ResourcePerCard") / value(&card, "CardResourcePerBulletHit");
    let rate = calc::compute(&d, &b).unwrap().stat("shots_per_second").unwrap();
    let one_card = Action::Shoot { seconds: (hits_per_card + 0.5) / rate };
    let r = run(&d, &b, vec![one_card.clone()], 0.0);
    assert!(close(source(&r, "Card Trick"), value(&card, "Damage") * (1.0 + value(&card, "SpadeDamageBonus") / 100.0 / 4.0)));
    // Slower shooting gives fewer cards in the same time.
    let mut slowed = b.clone();
    slowed.fire_rate_slow = 30.0;
    let r = run(&d, &slowed, vec![one_card], 0.0);
    assert_eq!(source(&r, "Card Trick"), 0.0);
}

#[test]
fn mina_rake_and_shots_build_up_the_bite_together() {
    let d = data();
    let b = hero(&d, "Mina");
    let (rake, _) = ability(&d, &b, "Rake");
    let (_, bites) = ability(&d, &b, "Love Bites");
    // Rake gives 30%; shots give 18.4% each. Four shots alone are not enough, with Rake they are.
    assert!(value(&bites, "BuildUpPerShot") * 4.0 < 100.0);
    assert!(value(&bites, "BuildUpPerDagger") + value(&bites, "BuildUpPerShot") * 4.0 >= 100.0);
    let four_shots = Action::Shoot { seconds: 3.5 / calc::compute(&d, &b).unwrap().stat("shots_per_second").unwrap() };
    let without = run(&d, &b, vec![four_shots.clone()], 0.0);
    let with = run(&d, &b, vec![cast(rake), four_shots], 0.0);
    assert_eq!(source(&without, "Love Bites"), 0.0);
    assert!(close(source(&with, "Love Bites"), value(&bites, "BonusDamage")));
}

#[test]
fn baba_ground_pound_works_with_melee_items() {
    let d = data();
    let mut b = hero(&d, "Baba");
    let class = |name: &str| d.items.iter().find(|i| i.name == name).unwrap().class_name.clone();
    let (index, legs) = ability(&d, &b, "Granny Long Legs");
    let pound = vec![cast(index), Action::Melee { heavy: true }];
    let plain_heavy = source(&run(&d, &b, pound.clone(), 0.0), "Heavy melee");

    b.purchases = vec![class("Melee Charge"), class("Spirit Strike")];
    let sheet = calc::compute(&d, &b).unwrap();
    let effect = |item: &str, key: &str| {
        sheet.item_effects.iter().find(|e| e.name == item).unwrap().props.iter().find(|p| p.key == key).unwrap().value
    };
    let r = run(&d, &b, pound, 0.0);
    // Melee Charge: +10% melee damage always, +25% on the charged heavy melee.
    let charged = sheet.stat("heavy_melee").unwrap() * (1.0 + effect("Melee Charge", "BonusHeavyMeleeDamage") / 100.0);
    assert!(close(source(&r, "Heavy melee"), charged));
    assert!(source(&r, "Heavy melee") > plain_heavy * 1.3);
    // Spirit Strike procs on the ground pound, and its shred lands before the ground pound's own spirit damage.
    assert!(close(source(&r, "Spirit Strike"), effect("Spirit Strike", "SpiritDamage")));
    assert!(source(&r, "Heavy melee (bonus spirit damage)") >= value(&legs, "HeavyMeleeSpiritDamage"));
}

#[test]
fn spirit_power_buffs_raise_the_damage_of_later_abilities() {
    let d = data();
    let mut b = hero(&d, "Yamato");
    let (slash, _) = ability(&d, &b, "Power Slash");
    let (flying, _) = ability(&d, &b, "Flying Slash");
    // Flying Slash's second upgrade grants spirit power for a few seconds.
    b.ability_tiers = vec![0; 4];
    b.ability_tiers[flying] = 2;
    let sheet = calc::compute(&d, &b).unwrap();
    let bonus = value(&sheet.abilities[flying], "SpiritBonus");
    let coef = sheet.abilities[slash].props.iter().find(|p| p.key == "FullChargeDamage").unwrap().coef;
    assert!(bonus > 0.0 && coef > 0.0);

    let alone = run(&d, &b, vec![cast(slash)], 0.0);
    let buffed = run(&d, &b, vec![cast(flying), cast(slash)], 0.0);
    assert!(close(source(&buffed, "Power Slash"), source(&alone, "Power Slash") + bonus * coef));
    // Once the buff has run out, the damage is back to normal.
    let expired = run(&d, &b, vec![cast(flying), Action::Wait { seconds: 10.0 }, cast(slash)], 0.0);
    assert!(close(source(&expired, "Power Slash"), source(&alone, "Power Slash")));
}

#[test]
fn baba_threadsap_spirit_steal_boosts_her_other_abilities() {
    let d = data();
    let b = hero(&d, "Baba");
    let (sap, thread) = ability(&d, &b, "Threadsap");
    let (birds, feed) = ability(&d, &b, "Feed The Birds");
    let gained = value(&thread, "TechPowerGain") * value(&thread, "StolenSpiritEffectiveness") / 100.0;
    let coef = feed.props.iter().find(|p| p.key == "BarrageMaxDamage").unwrap().coef;
    let alone = run(&d, &b, vec![cast(birds)], 0.0);
    let after = run(&d, &b, vec![cast(sap), cast(birds)], 0.0);
    assert!(close(source(&after, "Feed The Birds"), source(&alone, "Feed The Birds") + gained * coef));
}

#[test]
fn item_spirit_buffs_raise_the_damage_of_later_abilities() {
    let d = data();
    let mut b = hero(&d, "Yamato");
    let class = |name: &str| d.items.iter().find(|i| i.name == name).unwrap().class_name.clone();
    let prop = |item: &str, key: &str| d.items.iter().find(|i| i.name == item).unwrap().props.iter().find(|p| p.key == key).unwrap().value;
    b.purchases = vec![class("Infuser"), class("Spirit Snatch")];
    let (slash, sheet_slash) = ability(&d, &b, "Power Slash");
    let coef = sheet_slash.props.iter().find(|p| p.key == "FullChargeDamage").unwrap().coef;
    let alone = source(&run(&d, &b, vec![cast(slash)], 0.0), "Power Slash");

    // Infuser: used as an action, then the ability.
    let infused = run(&d, &b, vec![Action::Item { item: class("Infuser") }, cast(slash)], 0.0);
    assert!(close(source(&infused, "Power Slash"), alone + prop("Infuser", "BonusSpirit") * coef));

    // Spirit Snatch: a heavy melee steals the full amount, a light one 30% less.
    // The same hit also lowers the target's spirit resist, which the later ability benefits from too.
    let gain = prop("Spirit Snatch", "TechPowerGain");
    let shred = 1.0 + prop("Spirit Snatch", "TechArmorDamageReduction").abs() / 100.0;
    let heavy = run(&d, &b, vec![Action::Melee { heavy: true }, cast(slash)], 0.0);
    assert!(close(source(&heavy, "Power Slash"), (alone + gain * coef) * shred));
    let light = run(&d, &b, vec![Action::Melee { heavy: false }, cast(slash)], 0.0);
    assert!(close(source(&light, "Power Slash"), (alone + gain * 0.7 * coef) * shred));

    // Switching the item's conditional stats on in the build does not count the steal twice.
    b.active = vec![class("Spirit Snatch")];
    let switched = run(&d, &b, vec![Action::Melee { heavy: true }, cast(slash)], 0.0);
    assert!(close(source(&switched, "Power Slash"), (alone + gain * coef) * shred));
}

#[test]
fn diviners_kevlar_grants_spirit_after_the_ultimate() {
    let d = data();
    let mut b = hero(&d, "Yamato");
    let kevlar = d.items.iter().find(|i| i.name == "Diviner's Kevlar").unwrap();
    b.purchases = vec![kevlar.class_name.clone()];
    let (slash, sheet_slash) = ability(&d, &b, "Power Slash");
    let coef = sheet_slash.props.iter().find(|p| p.key == "FullChargeDamage").unwrap().coef;
    let bonus = kevlar.props.iter().find(|p| p.key == "TechPower").unwrap().value;
    let alone = source(&run(&d, &b, vec![cast(slash)], 0.0), "Power Slash");
    let after_ult = run(&d, &b, vec![cast(3), cast(slash)], 0.0);
    assert!(close(source(&after_ult, "Power Slash"), alone + bonus * coef));
}

#[test]
fn imbue_items_only_affect_the_chosen_ability() {
    let d = data();
    let mut b = hero(&d, "Yamato");
    let class = |name: &str| d.items.iter().find(|i| i.name == name).unwrap().class_name.clone();
    let prop = |item: &str, key: &str| d.items.iter().find(|i| i.name == item).unwrap().props.iter().find(|p| p.key == key).unwrap().value;
    let base = calc::compute(&d, &b).unwrap();
    let cooldown = |s: &calc::Sheet, i: usize| value(&s.abilities[i], "AbilityCooldown");
    let (slash, crimson) = (0, 2);

    // Not imbued yet: the items do nothing for the abilities, even with their switch on.
    b.purchases = vec![class("Surge of Power"), class("Compress Cooldown")];
    b.active = b.purchases.clone();
    let idle = calc::compute(&d, &b).unwrap();
    assert!(close(cooldown(&idle, slash), cooldown(&base, slash)));

    b.imbues.insert(class("Surge of Power"), slash);
    b.imbues.insert(class("Compress Cooldown"), crimson);
    let s = calc::compute(&d, &b).unwrap();
    let coef = s.abilities[slash].props.iter().find(|p| p.key == "FullChargeDamage").unwrap().coef;
    let spirit = s.stat("spirit_power").unwrap();
    // Power Slash gets the imbued spirit power; the hero's own spirit power does not.
    let damage = |s: &calc::Sheet| value(&s.abilities[slash], "FullChargeDamage");
    assert!(close(damage(&s), damage(&idle) + prop("Surge of Power", "ImbuedTechPower") * coef));
    assert!(close(spirit, idle.stat("spirit_power").unwrap()));
    // Crimson Slash gets the cooldown reduction; Power Slash keeps its cooldown.
    assert!(close(cooldown(&s, crimson), cooldown(&idle, crimson) * (1.0 - prop("Compress Cooldown", "CooldownReduction") / 100.0)));
    assert!(close(cooldown(&s, slash), cooldown(&idle, slash)));
    assert_eq!(s.abilities[slash].imbued, vec!["Surge of Power".to_string()]);
}

#[test]
fn quicksilver_reload_only_procs_on_its_imbued_ability() {
    let d = data();
    let mut b = hero(&d, "Yamato");
    let item = d.items.iter().find(|i| i.name == "Quicksilver Reload").unwrap();
    b.purchases = vec![item.class_name.clone()];
    assert_eq!(source(&run(&d, &b, vec![cast(0)], 0.0), "Quicksilver Reload"), 0.0);
    b.imbues.insert(item.class_name.clone(), 2);
    assert_eq!(source(&run(&d, &b, vec![cast(0)], 0.0), "Quicksilver Reload"), 0.0);
    assert!(source(&run(&d, &b, vec![cast(2)], 0.0), "Quicksilver Reload") > 0.0);
}

#[test]
fn conditions_are_switched_on_as_combo_actions() {
    let d = data();
    let mut b = hero(&d, "Infernus");
    let item = d.items.iter().find(|i| i.name == "Close Quarters").unwrap();
    let bonus = item.props.iter().find(|p| p.key == "CloseRangeBonusWeaponPower").unwrap().value;
    b.purchases = vec![item.class_name.clone()];
    b.souls = Some(0);
    let sheet = calc::compute(&d, &b).unwrap();
    let innate = sheet.stat("weapon_damage_pct").unwrap();
    let shoot = Action::Shoot { seconds: 1.0 };
    let plain = run(&d, &b, vec![shoot.clone()], 0.0);
    let on = run(&d, &b, vec![Action::Condition { item: item.class_name.clone(), count: None }, shoot.clone()], 0.0);
    assert!(close(source(&on, "Weapon"), source(&plain, "Weapon") * (100.0 + innate + bonus) / (100.0 + innate)));
    assert!(on.usable_items.iter().any(|u| u.condition && u.name == "Close Quarters"));

    // The switch on the build itself is ignored inside the combo: only the action counts.
    b.active = vec![item.class_name.clone()];
    let switched = run(&d, &b, vec![shoot], 0.0);
    assert!(close(source(&switched, "Weapon"), source(&plain, "Weapon")));
}

fn condition(id: &str) -> Action {
    Action::Condition { item: id.to_string(), count: None }
}

fn counted(id: &str, count: f64) -> Action {
    Action::Condition { item: id.to_string(), count: Some(count) }
}

#[test]
fn shiv_full_rage_is_a_condition() {
    let d = data();
    let mut b = hero(&d, "Shiv");
    b.target.health = 1000.0;
    let (dash, slice) = ability(&d, &b, "Slice and Dice");
    let (blow, killing) = ability(&d, &b, "Killing Blow");
    let (knives, serrated) = ability(&d, &b, "Serrated Knives");
    let amp = 1.0 + value(&killing, "BuffDamage") / 100.0;
    let impact = value(&slice, "ImpactDamage");

    let plain = run(&d, &b, vec![cast(dash)], 0.0);
    assert!(close(plain.total, impact));
    assert!(plain.usable_items.iter().any(|u| u.condition && u.name == "Full rage"));

    // Slice and Dice hits twice, and everything deals the rage bonus.
    let raged = run(&d, &b, vec![condition("hero:shiv_rage"), cast(dash)], 0.0);
    let shred = 1.0 + value(&slice, "TechArmorDamageReduction").abs() / 100.0;
    assert!(close(raged.total, impact * amp + impact * amp * shred), "{} vs {}", raged.total, impact * amp * (1.0 + shred));

    // Knives add a share of the target's current health on impact.
    let pct = value(&serrated, "FullRageCurrentHealthDamagePct") / 100.0;
    let knife = run(&d, &b, vec![condition("hero:shiv_rage"), Action::Ability { index: knives, duration: Some(0.0), hits: None }], 0.0);
    assert!(close(knife.total, 1000.0 * pct * amp), "{} vs {}", knife.total, 1000.0 * pct * amp);
    let _ = blow;
}

#[test]
fn drifter_isolation_comes_from_the_condition_or_eternal_night() {
    let d = data();
    let b = hero(&d, "Drifter");
    let (rend, _) = ability(&d, &b, "Rend");
    let (night, _) = ability(&d, &b, "Eternal Night");
    let (_, scent) = ability(&d, &b, "Bloodscent");
    let amp = 1.0 + value(&scent, "AmpDamagePercent") / 100.0;
    let plain = run(&d, &b, vec![cast(rend)], 0.0).total;
    assert!(close(run(&d, &b, vec![condition("hero:drifter_isolated"), cast(rend)], 0.0).total, plain * amp));
    assert!(close(run(&d, &b, vec![cast(night), cast(rend)], 0.0).total, plain * amp));
}

#[test]
fn hero_conditions_change_single_abilities() {
    let d = data();
    // Venator: Gutshot near a wall.
    let b = hero(&d, "Venator");
    let (gut, shot) = ability(&d, &b, "Gutshot");
    let plain = run(&d, &b, vec![cast(gut)], 0.0).total;
    let wall = run(&d, &b, vec![condition("hero:venator_wall"), cast(gut)], 0.0).total;
    assert!(close(wall, plain + value(&shot, "BonusDamage")));

    // Sinclair: the Assistant repeats Vexing Bolt at reduced damage.
    let b = hero(&d, "Sinclair");
    let (bolt, vexing) = ability(&d, &b, "Vexing Bolt");
    let plain = run(&d, &b, vec![cast(bolt)], 0.0).total;
    let both = run(&d, &b, vec![condition("hero:sinclair_assistant"), cast(bolt)], 0.0).total;
    assert!(close(both, plain * (1.0 + value(&vexing, "CloneDamagePercentage") / 100.0)));

    // Paradox: shots through the wall.
    let b = hero(&d, "Paradox");
    let (_, wall_ability) = ability(&d, &b, "Time Wall");
    let shoot = Action::Shoot { seconds: 1.0 };
    let plain = run(&d, &b, vec![shoot.clone()], 0.0).total;
    let through = run(&d, &b, vec![condition("hero:paradox_time_wall"), shoot], 0.0).total;
    assert!(close(through, plain * (1.0 + value(&wall_ability, "FriendlyBulletDamageBonus") / 100.0)));
}

#[test]
fn every_hero_condition_can_be_switched_on() {
    let d = data();
    for h in &d.heroes {
        let b = BuildInput { hero: h.class_name.clone(), ability_tiers: vec![3; 4], ..Default::default() };
        let listed = run(&d, &b, vec![], 0.0).usable_items;
        for c in listed.iter().filter(|u| u.condition) {
            let mut actions = vec![condition(&c.item)];
            actions.extend((0..4).map(cast));
            actions.push(Action::Shoot { seconds: 2.0 });
            let r = run(&d, &b, actions, 0.0);
            assert!(r.total.is_finite() && r.total > 0.0, "{} / {}", h.name, c.name);
            assert!(r.events.iter().any(|e| e.source == c.name), "{} / {} left no trace in the timeline", h.name, c.name);
        }
    }
}

#[test]
fn counted_hero_conditions_scale_with_the_count() {
    let d = data();
    // Bebop: bomb stacks, at half rate beyond the soft cap.
    let b = hero(&d, "Bebop");
    let (bomb, sticky) = ability(&d, &b, "Sticky Bomb");
    let (hit, kill) = (value(&sticky, "BonusDamagePctPerPlayerHit"), value(&sticky, "BonusDamagePctPerPlayerKilled"));
    let plain = run(&d, &b, vec![cast(bomb)], 0.0).total;
    let stacked = run(&d, &b, vec![counted("hero:bebop_bomb_hits", 20.0), counted("hero:bebop_bomb_kills", 3.0), cast(bomb)], 0.0).total;
    assert!(close(stacked, plain * (1.0 + (20.0 * hit + 3.0 * kill) / 100.0)));
    let beyond = run(&d, &b, vec![counted("hero:bebop_bomb_hits", 80.0), cast(bomb)], 0.0).total;
    assert!(close(beyond, plain * (1.0 + (60.0 + 10.0) * hit / 100.0)));

    // Grey Talon: spirit power per owl kill raises his abilities.
    let b = hero(&d, "Grey Talon");
    let (shot, charged) = ability(&d, &b, "Charged Shot");
    let (_, owl) = ability(&d, &b, "Guided Owl");
    let coef = charged.props.iter().find(|p| p.key == "Damage").unwrap().coef;
    let plain = run(&d, &b, vec![cast(shot)], 0.0).total;
    let fed = run(&d, &b, vec![counted("hero:talon_owl_kills", 5.0), cast(shot)], 0.0).total;
    assert!(close(fed, plain + 5.0 * value(&owl, "BonusTechPowerPerKill") * coef));

    // Vindicta: weapon damage per Assassinate kill; setting the count again replaces it.
    let b = hero(&d, "Vindicta");
    let (_, snipe) = ability(&d, &b, "Assassinate");
    let per = value(&snipe, "WeaponDamageBonusPerKill") / 100.0;
    let shoot = Action::Shoot { seconds: 1.0 };
    let plain = run(&d, &b, vec![shoot.clone()], 0.0).total;
    let four = run(&d, &b, vec![counted("hero:vindicta_kills", 4.0), shoot.clone()], 0.0).total;
    assert!(close(four, plain * (1.0 + 4.0 * per)));
    let replaced = run(&d, &b, vec![counted("hero:vindicta_kills", 4.0), counted("hero:vindicta_kills", 2.0), shoot], 0.0).total;
    assert!(close(replaced, plain * (1.0 + 2.0 * per)));

    // Mina: earlier bites add bats.
    let b = hero(&d, "Mina");
    let (nox, nostra) = ability(&d, &b, "Nox Nostra");
    let plain = source(&run(&d, &b, vec![cast(nox)], 0.0), "Nox Nostra");
    let more = source(&run(&d, &b, vec![counted("hero:mina_bites", 10.0), cast(nox)], 0.0), "Nox Nostra");
    let bats = value(&nostra, "BatCount");
    assert!(close(more, plain * (bats + 10.0 * value(&nostra, "BonusBatsPerProc")) / bats));
}

#[test]
fn counted_item_conditions_scale_with_stacks() {
    let d = data();
    let mut b = hero(&d, "Infernus");
    let item = d.items.iter().find(|i| i.name == "Berserker").unwrap();
    let per = item.props.iter().find(|p| p.key == "WeaponPowerPerStack").unwrap().value;
    let max = item.props.iter().find(|p| p.key == "MaxStacks").unwrap().value;
    b.purchases = vec![item.class_name.clone()];
    b.souls = Some(0);
    let innate = calc::compute(&d, &b).unwrap().stat("weapon_damage_pct").unwrap();
    let shoot = Action::Shoot { seconds: 1.0 };
    let weapon = |actions: Vec<Action>| source(&run(&d, &b, actions, 0.0), "Weapon");
    let plain = weapon(vec![shoot.clone()]);
    let three = weapon(vec![counted(&item.class_name, 3.0), shoot.clone()]);
    assert!(close(three, plain * (100.0 + innate + 3.0 * per) / (100.0 + innate)));
    // The count is capped at the item's maximum stacks.
    let capped = weapon(vec![counted(&item.class_name, 999.0), shoot]);
    assert!(close(capped, plain * (100.0 + innate + max * per) / (100.0 + innate)));
    let listed = run(&d, &b, vec![], 0.0).usable_items;
    assert!(listed.iter().any(|u| u.name == "Berserker" && u.max_count == Some(max)));
}

#[test]
fn sinclair_copies_an_ultimate_with_his_own_stats_and_tier() {
    let d = data();
    let mut b = hero(&d, "Sinclair");
    let infernus = d.heroes.iter().find(|h| h.name == "Infernus").unwrap();
    let original = &infernus.abilities[3];
    b.copied_ult = Some(infernus.class_name.clone());
    b.ability_tiers = vec![0, 0, 0, 1];
    let sheet = calc::compute(&d, &b).unwrap();
    let copy = &sheet.abilities[3];
    assert_eq!(copy.class_name, original.class_name);
    assert_eq!(copy.slot_class, "ability_magician_copyult");
    assert!(copy.name.contains(&original.name));
    // Tier 1 of the copy is tier 1 of the original: +100 damage on Concussive Combustion.
    let base = original.props.iter().find(|p| p.key == "Damage").unwrap();
    let up = original.upgrades[0].iter().find(|u| u.name == "Damage").unwrap().bonus;
    let spirit = sheet.stat("spirit_power").unwrap();
    assert!(close(value(copy, "Damage"), base.value + up + base.scale.as_ref().unwrap().coef * spirit));
    // The copy has 60% of the original cooldown.
    let cooldown = original.props.iter().find(|p| p.key == "AbilityCooldown").unwrap().value;
    assert!(close(value(copy, "AbilityCooldown"), cooldown * 0.6));
    // And it plays out like the original in a combo: a delayed explosion.
    let r = run(&d, &b, vec![cast(3)], 0.0);
    assert!(close(r.total, value(copy, "Damage")));
    assert!(r.full_duration > 3.0);

    // Without a copied hero the slot stays Audience Participation.
    b.copied_ult = None;
    assert_eq!(calc::compute(&d, &b).unwrap().abilities[3].class_name, "ability_magician_copyult");
}

#[test]
fn silver_slam_fire_rewards_landing_every_shot() {
    let d = data();
    let mut b = hero(&d, "Silver");
    b.target.health = 1000.0;
    let (slam, _) = ability(&d, &b, "Slam Fire");
    b.ability_tiers = vec![0; 4];
    b.ability_tiers[slam] = 3;
    let fire = calc::compute(&d, &b).unwrap().abilities[slam].clone();
    let (per_shot, all) = (value(&fire, "CurrentHealthDamagePercentage") / 100.0, value(&fire, "BonusCurrentHealthDamagePercentage") / 100.0);
    // Each bonus takes a share of what is left after the previous one.
    let after_three = 1000.0 * (1.0 - per_shot).powi(3);
    let all_hit = run(&d, &b, vec![cast(slam)], 0.0);
    assert!(close(all_hit.total, 1000.0 - after_three * (1.0 - all)), "{} vs {}", all_hit.total, 1000.0 - after_three * (1.0 - all));
    let two = run(&d, &b, vec![Action::Ability { index: slam, duration: None, hits: Some(2.0) }], 0.0);
    assert!(close(two.total, 1000.0 * (1.0 - (1.0 - per_shot).powi(2))));
}

#[test]
fn express_shot_fires_a_boosted_double_shot_on_cooldown() {
    let d = data();
    let mut b = hero(&d, "Infernus");
    let item = d.items.iter().find(|i| i.name == "Express Shot").unwrap();
    b.purchases = vec![item.class_name.clone()];
    b.souls = Some(0);
    let sheet = calc::compute(&d, &b).unwrap();
    let proc = item.props.iter().find(|p| p.key == "ProcBaseAttackDamagePercent").unwrap();
    let bonus = proc.value + proc.scale.as_ref().unwrap().coef * sheet.boons as f64;
    let weapon_pct = sheet.stat("weapon_damage_pct").unwrap();
    let base = sheet.stat("bullet_damage").unwrap() / (1.0 + weapon_pct / 100.0);
    let r = run(&d, &b, vec![Action::Shoot { seconds: 1.0 }], 100.0);
    // One proc in a second: two shots, each with the bonus, and no headshot damage on them.
    assert!(close(source(&r, "Express Shot"), 2.0 * base * (1.0 + (weapon_pct + bonus) / 100.0)));
    // The other shots of that second are normal headshots.
    let shots = sheet.stat("shots_per_second").unwrap().ceil() - 1.0;
    assert!(close(source(&r, "Weapon"), shots * sheet.stat("bullet_damage").unwrap() * sheet.stat("crit_multiplier").unwrap()));
}
