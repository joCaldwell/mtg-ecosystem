//! Bind the graveyard and destination owners to the same announced player.
use super::*;
use crate::bindings::Bindings;
use mtg_contracts::graveyard_shuffle::GraveyardShuffle;
use serde_json::{Value, json};

pub(super) fn bind_step(
    graph: &mut Bindings,
    root: &str,
    path: &str,
    step: &Value,
    shape: &GraveyardShuffle,
) {
    let scope = graph.id(root, "ability");
    let who = format!("{path}/who");
    let cards = format!("{path}/what/0");
    let target = graph.target(&scope, &who, &step["who"], &shape.player_selection);
    let selected =
        graph.bounded_card_targets(&scope, &cards, &step["what"][0], &shape.cards_selection);
    for owner in [
        format!("{cards}/filter/zone/owner"),
        format!("{path}/library/owner"),
    ] {
        graph.reference(&owner, &json!({"player":"they"}), &target, "608.2k");
    }
    graph.node(path,"event","event",json!({"scope":scope,"operation":"shuffle-into-library","inputs":[selected,target],"availability":"when-performed","rules":["608.2c"]}));
    graph.result(path, "quantity", "measured-quantity", None);
}

#[test]
fn target_graveyard_shuffle_requires_both_owner_edges_and_distinct_occurrences() {
    let row: Value = serde_json::from_str(include_str!(
        "../../../tests/fixtures/ramos/9c0f7cdc-9095-40ae-bf83-75a5ffda72b9.json"
    ))
    .unwrap();
    let root = "/faces/0/abilities/0";
    let path = format!("{root}/effects/0/steps/0/options/3/effects/0/steps/0");
    let raw = &row["faces"][0]["result"]["abilities"][0]["effects"][0]["steps"][0]["options"][3]["effects"]
        [0]["steps"][0];
    let text = row["source"]["oracle_text"]
        .as_str()
        .unwrap()
        .lines()
        .last()
        .unwrap()
        .strip_prefix("• ")
        .unwrap();
    let shape = GraveyardShuffle::from_clause(text, raw).unwrap();
    let mut graph = Bindings::new(row["oracleIdentity"].as_str().unwrap(), &row["source"]);
    graph.ability(root, "spell", "spell-controller");
    bind_step(&mut graph, root, &path, raw, &shape);
    let mut binding = row["binding"].clone();
    binding["nodes"].as_array_mut().unwrap().retain(|n| {
        let p = n["path"].as_str().unwrap();
        p == "/source" || p == "/faces/0" || p == root || p.starts_with(&path)
    });
    binding["references"]
        .as_array_mut()
        .unwrap()
        .retain(|r| r["path"].as_str().unwrap().starts_with(&path));
    graph.validate(&binding, "shuffle graph").unwrap();
    let owner = binding["references"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|r| r["path"].as_str().unwrap().ends_with("/library/owner"))
        .unwrap();
    owner["target"] = json!("unrelated-seat");
    assert!(graph.validate(&binding, "shuffle graph").is_err());
}
