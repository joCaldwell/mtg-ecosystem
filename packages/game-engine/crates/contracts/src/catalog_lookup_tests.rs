//! Catalog lookup must preserve the reviewed linear lookup's identity and bytes.
use super::*;

fn catalogs() -> impl Iterator<Item = Catalog> {
    [
        include_str!("../../../tests/fixtures/expected-ir.json"),
        include_str!("../../../tests/fixtures/expected-wipes-ir.json"),
        include_str!("../../../tests/fixtures/cards-data-d2cd/expected-ir.json"),
    ]
    .into_iter()
    .map(|raw| Catalog::read(raw).unwrap())
}

#[test]
fn catalog_lookup_matches_linear_reference_without_mutating_catalog() {
    for catalog in catalogs() {
        let before = canonical(&catalog).unwrap();
        let mut queries = vec![String::new(), "~after-all-oracle-ids".into()];
        for definition in &catalog.definitions {
            queries.push(definition.oracle_id.clone());
            // Probe the insertion position just after each existing identity.
            queries.push(format!("{}\0", definition.oracle_id));
        }
        for query in queries {
            let expected = catalog
                .definitions
                .iter()
                .find(|definition| definition.oracle_id == query);
            let actual = catalog.definition_by_oracle_id(&query);
            assert_eq!(actual, expected, "Oracle identity {query:?}");
            if let (Some(actual), Some(expected)) = (actual, expected) {
                assert!(std::ptr::eq(actual, expected));
            }
        }
        assert_eq!(canonical(&catalog).unwrap(), before);
    }
}

#[test]
fn catalog_lookup_handles_empty_singleton_and_replaced_catalogs() {
    let mut catalog = catalogs().next().unwrap();
    let first = catalog.definitions[0].clone();
    let last = catalog.definitions.last().unwrap().clone();
    assert_ne!(first.oracle_id, last.oracle_id);
    catalog.definitions = vec![first.clone()];
    catalog.validate().unwrap();
    assert_eq!(
        catalog.definition_by_oracle_id(&first.oracle_id),
        Some(&first)
    );
    assert_eq!(catalog.definition_by_oracle_id(&last.oracle_id), None);

    catalog.definitions = vec![last.clone()];
    catalog.validate().unwrap();
    assert_eq!(catalog.definition_by_oracle_id(&first.oracle_id), None);
    assert_eq!(
        catalog.definition_by_oracle_id(&last.oracle_id),
        Some(&last)
    );

    catalog.definitions.clear();
    catalog.validate().unwrap();
    assert_eq!(catalog.definition_by_oracle_id(&first.oracle_id), None);
    assert_eq!(catalog.definition_by_oracle_id(""), None);
}

#[test]
fn catalog_lookup_ordering_precondition_is_enforced_at_read_boundary() {
    let catalog = catalogs().next().unwrap();
    let mut reversed = catalog.clone();
    reversed.definitions.reverse();
    assert!(reversed.validate().is_err());
    assert!(Catalog::read(&canonical(&reversed).unwrap()).is_err());

    let mut duplicate = catalog;
    duplicate
        .definitions
        .insert(0, duplicate.definitions[0].clone());
    assert!(duplicate.validate().is_err());
    assert!(Catalog::read(&canonical(&duplicate).unwrap()).is_err());
}
