//! Synthetic contract witnesses. The compiler separately verifies complete frozen sources.
use super::*;
pub(super) fn split_search() -> Definition {
    let catalog = Catalog::read(include_str!(
        "../../../tests/fixtures/expected-staples-ir.json"
    ))
    .unwrap();
    let mut d = catalog
        .definitions
        .into_iter()
        .find(|d| d.name == "Rampant Growth")
        .unwrap();
    d.spell.as_mut().unwrap().instructions =
        vec![Instruction::SearchBasicLandsSplit { source_block: 0 }];
    d.capabilities = d.requirements();
    d
}
#[test]
fn basic_land_split_contract_requires_capability_and_complete_program() {
    let d = split_search();
    d.validate().unwrap();
    assert!(d.capabilities.contains(&Capability::BasicLandSplitSearchV1));
    assert!(!d.capabilities.contains(&Capability::BasicLandSearchV1));
    let mut missing = d.clone();
    missing
        .capabilities
        .remove(&Capability::BasicLandSplitSearchV1);
    assert!(missing.validate().is_err());
    let mut extra = d.clone();
    extra
        .spell
        .as_mut()
        .unwrap()
        .instructions
        .push(Instruction::SearchBasicLands {
            maximum: 1,
            source_block: 0,
        });
    extra.capabilities = extra.requirements();
    assert!(extra.validate().is_err());
    let mut target = d.clone();
    let growth = Catalog::read(include_str!(
        "../../../tests/fixtures/expected-staples-ir.json"
    ))
    .unwrap()
    .definitions
    .into_iter()
    .find(|d| d.name == "Giant Growth")
    .unwrap();
    target.spell.as_mut().unwrap().targets = growth.spell.unwrap().targets;
    target.capabilities = target.requirements();
    assert!(target.validate().is_err());
    assert!(
        strict_read::<Instruction>(
            r#"{"op":"search_basic_lands_split","source_block":0,"maximum":1}"#
        )
        .is_err()
    );
    assert_eq!(CURRENT_IR_VERSION, 1);
}
