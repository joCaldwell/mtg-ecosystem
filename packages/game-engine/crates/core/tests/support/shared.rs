use mtg_contracts::Catalog;
use std::sync::OnceLock;

pub fn catalog() -> &'static Catalog {
    static MATCH: OnceLock<Catalog> = OnceLock::new();
    MATCH.get_or_init(|| {
        Catalog::read(include_str!(
            "../../../../tests/fixtures/expected-match-ir.json"
        ))
        .unwrap()
    })
}

pub fn oracle(name: &str) -> String {
    catalog()
        .definitions
        .iter()
        .find(|d| d.name == name)
        .unwrap()
        .oracle_id
        .clone()
}

// The view's public turn and an engine registration's private turn share a
// field name. Bind the public DTO type explicitly at this boundary.
pub fn turn(view: &mtg_core::PermittedView) -> &mtg_core::TurnState {
    let mtg_core::PermittedView { turn, .. } = view;
    turn
}
