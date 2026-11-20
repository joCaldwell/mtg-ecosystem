//! CR 406.3, 607.2a/3, 608.2g–h and 702.75a: the same-instance exile link.
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;
use crate::bindings::Bindings;
use mtg_contracts::linked_hideaway;
use serde_json::{Value, json};

const KEYWORD: &str = "/faces/0/abilities/0/keywords/0";
const TAPPED: &str = "/faces/0/abilities/1";
const MANA: &str = "/faces/0/abilities/2";
const PLAY: &str = "/faces/0/abilities/3";
const MANA_STEP: &str = "/faces/0/abilities/2/effects/0/steps/0";
const PLAY_STEP: &str = "/faces/0/abilities/3/effects/0/steps/0";
const CONDITION: &str = "/faces/0/abilities/3/effects/0/steps/0/instruction/condition";
const AMONG: &str = "/faces/0/abilities/3/effects/0/steps/0/instruction/condition/value/among";

pub(super) fn validate(
    card: &Source,
    shape: &Value,
    parsed: &ParseResult,
    binding: &Value,
) -> Result<bool, Failure> {
    if !card.oracle_text.starts_with("Hideaway ")
        && !parsed.abilities.get().contains("\"hideaway\"")
    {
        return Ok(false);
    }
    if !linked_hideaway::source(&card.oracle_text)
        || card.type_line != "Land"
        || !card.mana_cost.is_empty()
        || decode::<Value>(parsed.abilities.get(), "abilities")? != linked_hideaway::abilities()
    {
        return Err(unsupported(
            "abilities.hideaway",
            "complete linked hideaway source required",
        ));
    }

    let mut graph = Bindings::new(&card.oracle_id, shape);
    let object = graph.id("/source", "object");
    let base = "/faces/0/abilities/0";
    graph.ability(base, "keywords", "static-source-controller");
    let keyword_controller = graph.node(
        KEYWORD,
        "you",
        "runtime-player",
        json!({
            "scope":graph.id(base,"ability"),"role":"trigger-controller","rules":["109.5","702.75"]
        }),
    );
    let keyword = graph.node(
        KEYWORD,
        "keyword",
        "keyword-ability",
        json!({
            "scope":graph.id(base,"ability"),"keyword":"hideaway","abilities":["triggered"],
            "you":keyword_controller,"rules":["702.75"]
        }),
    );
    let pool = graph.node(KEYWORD, "pool", "hideaway-exiled-pool", json!({
        "keyword":keyword,"source":object,"key":"source-incarnation","libraryOwner":keyword_controller,
        "lookCount":4,"exile":"one-face-down","remainder":"bottom-random",
        "lookPermission":"controller-of-exiling-permanent","membership":"still-in-exile",
        "cardinality":"many","rules":["400.7","406.3","607.2a","607.3","702.75a"]
    }));

    graph.ability(TAPPED, "static", "static-source-controller");
    graph.node(&format!("{TAPPED}/effect"), "entering-controller", "runtime-player", json!({
        "scope":graph.id(TAPPED,"ability"),"role":"entering-controller","rules":["110.2a","614.12"]
    }));

    graph.ability(MANA, "activated", "activator");
    let mana_payer = graph.id(MANA, "you");
    graph.event(MANA, MANA_STEP, "add-mana", &mana_payer, false);
    graph.ability(PLAY, "activated", "activator");
    let player = graph.id(PLAY, "you");
    let scope = graph.id(PLAY, "ability");
    let creatures = json!({"ref":"each","filter":{"types":["creature"],"control":"you"}});
    let selected = graph.node(
        AMONG,
        "query",
        "selection",
        json!({
            "scope":scope,"expression":creatures,"cardinality":"many","timing":"resolution",
            "selection":"evaluate-reference","rules":["608.2h"]
        }),
    );
    let condition = graph.node(CONDITION, "condition", "total-power-condition", json!({
        "scope":scope,"creatures":selected,"comparison":{"op":"ge","value":{"amount":"fixed","value":10}},
        "timing":"resolution","rules":["608.2h"]
    }));
    graph.node(PLAY_STEP, "event", "event", json!({
        "scope":scope,"operation":"play-card","inputs":[pool,player],"availability":"when-performed",
        "optional":true,"decider":player,"conditional":true,"condition":condition,"rules":["608.2c"]
    }));

    let you = json!({"player":"you"});
    let itself = json!({"ref":"self"});
    graph.reference(
        &format!("{TAPPED}/effect/what"),
        &json!({"ref":"this","noun":"land"}),
        &object,
        "201.5",
    );
    graph.reference(&format!("{MANA}/costs/0/source"), &itself, &object, "201.5");
    graph.reference(&format!("{MANA}/costs/0/payer"), &you, &mana_payer, "109.5");
    graph.reference(&format!("{MANA_STEP}/who"), &you, &mana_payer, "109.5");
    graph.reference(&format!("{PLAY}/costs/0/payer"), &you, &player, "109.5");
    graph.reference(&format!("{PLAY}/costs/1/source"), &itself, &object, "201.5");
    graph.reference(&format!("{PLAY}/costs/1/payer"), &you, &player, "109.5");
    graph.reference(
        &format!("{PLAY_STEP}/instruction/who"),
        &you,
        &player,
        "109.5",
    );
    graph.reference_rules(
        &format!("{PLAY_STEP}/instruction/what"),
        &linked_hideaway::abilities()[3]["effects"][0]["steps"][0]["instruction"]["what"],
        &pool,
        &["400.7", "607.2a", "607.3"],
    );
    graph.reference(
        &format!("{PLAY_STEP}/instruction/alternativeCost/payer"),
        &you,
        &player,
        "109.5",
    );
    graph.reference(&format!("{AMONG}/filter/control"), &you, &player, "109.5");
    graph.reference(AMONG, &creatures, &selected, "608.2h");

    graph.validate_ordered_with_features(binding, &[
        json!({"feature":"hideaway-resolution","path":KEYWORD,
            "message":"On entry, the trigger controller looks at the top N cards of their library, exiles one face down if possible and puts the rest on the bottom in random order. Track the cards exiled by this keyword on this incarnation and the granted and retained permissions to look.",
            "rules":["400.7","406.3","607.2a","609.3","702.75a"]}),
        json!({"feature":"linked-exile-play","path":PLAY_STEP,
            "message":"During resolution, if the activator's creatures have total power at least ten, they may play each card still in this incarnation's linked exile pool. Apply land-play limits or cast without paying its mana cost, subject to legality.",
            "rules":["113.7a","305.2","305.3","406.3a","607.2a","607.3","608.2g","608.2h","701.18b"]})
    ], "linked hideaway requires the complete same-incarnation binding graph")?;
    Ok(true)
}

#[test]
fn linked_hideaway() {
    let Some(path) = std::env::var_os("MTG_LINKED_HIDEAWAY_ARCHIVE_PROBE") else {
        return;
    };
    let sheet = std::fs::read_to_string(path).unwrap();
    assert_eq!(
        sha256(sheet.as_bytes()),
        "114f814949a6c0ddf680ac569113b15452e9a9efb025cbd79e04f58220d9fe4f"
    );
    let rows = sheet.lines().collect::<Vec<_>>();
    let [manifest, record] = rows.as_slice() else {
        panic!("one exact source required")
    };
    let result = compile(manifest, record);
    let CompileResult::Lowered { definition } = result else {
        panic!("{result:?}")
    };
    assert_eq!(definition.triggered, [linked_hideaway::entry()]);
    assert_eq!(definition.activated, [linked_hideaway::play()]);
    let altered = tests::changed_source(record, |r| {
        r["binding"]["nodes"][6]["key"] = json!("physical-card");
    });
    assert!(
        matches!(compile(manifest, &altered), CompileResult::Unsupported { path, .. } if path == "binding")
    );
}
