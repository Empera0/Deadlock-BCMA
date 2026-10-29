use engine::data::GameData;
use engine::patch::diff;

fn data() -> GameData {
    serde_json::from_str(include_str!("../../src-tauri/data/snapshot.json")).unwrap()
}

#[test]
fn the_same_data_has_no_changes() {
    assert!(diff(&data(), &data()).changes.is_empty());
}

#[test]
fn changed_added_and_removed_values_are_listed() {
    let old = data();
    let mut new = data();
    new.client_version = Some(old.client_version.unwrap_or(0) + 1);
    new.heroes[0].base.insert("max_health".into(), 900.0);
    let cooldown = new.heroes[0].abilities[0].props.iter().position(|p| p.label == "Cooldown").unwrap();
    new.heroes[0].abilities[0].props[cooldown].value -= 5.0;
    new.items[0].cost += 250;
    let gone = new.items.pop().unwrap();
    let notes = diff(&old, &new);
    let hero = notes.changes.iter().find(|c| c.kind == "hero" && c.class_name == new.heroes[0].class_name).unwrap();
    let health = hero.values.iter().find(|v| v.label == "Max Health").unwrap();
    assert_eq!(health.to, Some(900.0));
    assert_ne!(health.from, health.to);
    let ability = notes.changes.iter().find(|c| c.kind == "ability" && c.class_name == new.heroes[0].abilities[0].class_name).unwrap();
    assert_eq!(ability.owner.as_deref(), Some(new.heroes[0].name.as_str()));
    assert!(ability.values.iter().any(|v| v.label == "Cooldown" && v.to.unwrap() + 5.0 == v.from.unwrap()));
    let item = notes.changes.iter().find(|c| c.class_name == new.items[0].class_name).unwrap();
    assert_eq!(item.values[0].label, "Cost");
    assert!(notes.changes.iter().any(|c| c.class_name == gone.class_name && c.status == "removed"));
    // Nothing else changed.
    assert_eq!(notes.changes.len(), 4);
}
