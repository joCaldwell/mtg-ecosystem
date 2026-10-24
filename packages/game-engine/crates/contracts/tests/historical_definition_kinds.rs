use mtg_contracts::*;
fn load(name: &str) -> Catalog {
    let p = format!("{}/../../tests/fixtures/{name}", env!("CARGO_MANIFEST_DIR"));
    Catalog::read(&std::fs::read_to_string(p).unwrap()).unwrap()
}
#[test]
fn v18_rejects_noncreature_enchantment_trigger() {
    let mut c = load("expected-removal-ir.json");
    let s = load("expected-sacrifice-ir.json");
    let bastion = s
        .definitions
        .iter()
        .find(|d| d.name == "Bastion of Remembrance")
        .unwrap();
    let furnace = c
        .definitions
        .iter_mut()
        .find(|d| d.name == "Furnace of Rath")
        .unwrap();
    furnace.triggered = vec![bastion.triggered[0].clone()];
    furnace.capabilities = furnace.requirements();
    assert!(c.validate().is_err());
}
#[test]
fn v18_rejects_creature_anthem_static() {
    let mut c = load("expected-removal-ir.json");
    let anthem = c
        .definitions
        .iter()
        .find(|d| d.name == "Glorious Anthem")
        .unwrap()
        .static_abilities
        .clone();
    let crow = c
        .definitions
        .iter_mut()
        .find(|d| {
            d.characteristics.kind == CardKind::Creature
                && d.static_abilities.is_empty()
                && d.triggered.is_empty()
                && d.activated.is_empty()
        })
        .unwrap();
    crow.static_abilities = anthem;
    crow.capabilities = crow.requirements();
    assert!(c.validate().is_err());
}
#[test]
fn v18_rejects_enchantment_activation() {
    let mut c = load("expected-removal-ir.json");
    let looter = c
        .definitions
        .iter()
        .find(|d| d.name == "Merfolk Looter")
        .unwrap()
        .activated
        .clone();
    let furnace = c
        .definitions
        .iter_mut()
        .find(|d| d.name == "Furnace of Rath")
        .unwrap();
    furnace.activated = looter;
    furnace.capabilities = furnace.requirements();
    assert!(c.validate().is_err());
}
#[test]
fn v19_accepts_its_reviewed_definition_kinds() {
    load("expected-sacrifice-ir.json").validate().unwrap();
}
