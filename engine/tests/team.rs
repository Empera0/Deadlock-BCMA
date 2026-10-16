//! What heroes' abilities and items do to enemies, checked against deadlock.wiki (see the audit notes).

use engine::data::GameData;
use engine::team::{self, Effect};

fn data() -> GameData {
    serde_json::from_str(include_str!("../../src-tauri/data/snapshot.json")).unwrap()
}

fn ability<'a>(d: &'a GameData, hero: &str, name: &str) -> &'a engine::data::Ability {
    d.heroes.iter().find(|h| h.name == hero).unwrap().abilities.iter().find(|a| a.name == name).unwrap()
}

fn item<'a>(d: &'a GameData, name: &str) -> &'a engine::data::Item {
    d.items.iter().find(|i| i.name == name).unwrap()
}

fn uptime(d: &GameData, hero: &str, name: &str, effect: Effect) -> f64 {
    team::ability_effects_timed(ability(d, hero, name)).into_iter().find(|e| e.0 == effect).map(|e| e.1).unwrap_or(0.0)
}

#[test]
fn ability_effects_last_their_own_duration_over_the_upgraded_cooldown() {
    let d = data();
    // Flog, fully upgraded: -30% fire rate for 3s every 26 - 16 = 10s.
    assert!((uptime(&d, "Lash", "Flog", Effect::FireRateCut) - 0.3).abs() < 0.01);
    // Baba's Brew: the third upgrade raises the 3s silence by a third (a percentage, not 33 seconds).
    assert!((uptime(&d, "Baba", "Baba's Brew", Effect::Silence) - 4.0 / 13.0).abs() < 0.01);
    // A charged ability with no cooldown of its own uses the time between charges.
    assert!(uptime(&d, "Graves", "Jar of Dead", Effect::Slow) < 0.2);
}

#[test]
fn effects_the_property_names_miss_come_from_the_wiki_fixes() {
    let d = data();
    let has = |hero: &str, name: &str, e: Effect| team::ability_effects(ability(&d, hero, name)).contains(&e);
    assert!(has("Bebop", "Exploding Uppercut", Effect::FireRateCut));
    assert!(has("Billy", "Chain Gang", Effect::Slow), "a slow named ...Pct");
    assert!(has("Grey Talon", "Spirit Snare", Effect::Disarm));
    assert!(has("Wraith", "Card Trick", Effect::SpiritShred));
    assert!(has("Mirage", "Fire Scarabs", Effect::DamageCut));
    assert!(!has("Mirage", "Djinn's Mark", Effect::Lockdown), "the T3 stun does not trigger");
    assert!(!has("Deadman Danny", "Chimera Mk1", Effect::Slow), "its slow numbers are Danny's own heat");
    // Upgrades carry no modifier: healing reduction is read by name (Stalker's Mark T3).
    assert!(has("Drifter", "Stalker's Mark", Effect::HealingCut));
}

#[test]
fn items_are_read_by_what_their_own_text_says() {
    let d = data();
    let effects = |name: &str| team::item_effects(item(&d, name));
    // The owner's own damage penalty is not a cut on enemies.
    assert!(!effects("Golden Goose Egg").contains(&Effect::DamageCut));
    assert!(!effects("Cursed Relic").contains(&Effect::DamageCut));
    assert!(effects("Inhibitor").contains(&Effect::DamageCut));
    // Silences and disarms written only in the text.
    assert!(effects("Cursed Relic").contains(&Effect::Silence) && effects("Cursed Relic").contains(&Effect::Disarm));
    assert!(effects("Silence Wave").contains(&Effect::Silence));
    assert!(effects("Phantom Strike").contains(&Effect::Disarm));
    assert!(!effects("Unstoppable").contains(&Effect::Disarm));
    assert!(!effects("Slowing Hex").contains(&Effect::Silence), "only movement items are silenced");
    // Unused slow numbers, and a stun item that does not help allies.
    assert!(!effects("Glass Cannon").contains(&Effect::Slow));
    assert!(effects("Capacitor").contains(&Effect::Slow));
    assert!(!effects("Knockdown").contains(&Effect::TeamHeal));
    assert!(effects("Arctic Blast").contains(&Effect::Lockdown), "it freezes");
}

#[test]
fn stuns_are_timed_for_items_and_abilities() {
    let d = data();
    assert!((team::item_lockdown_seconds(item(&d, "Crushing Fists")) - 0.75).abs() < 0.01);
    assert_eq!(team::item_lockdown_seconds(item(&d, "Extra Health")), 0.0);
    // Ivy's Stone Form stuns 0.75s, 1.5s with its third upgrade.
    assert!((team::ability_lockdown_seconds(ability(&d, "Ivy", "Stone Form")) - 1.5).abs() < 0.01);
}
