//! CR 121.2c: a simultaneous instruction to draw is carried out in turn order.
//! CR 107.3a and 601.2b: printed mana X is announced for this cast.
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;
use crate::bindings::Bindings;
use serde_json::{Value, json};

pub(super) fn lower(
    source: &Value,
    result: &ParseResult,
    binding: &Value,
) -> Result<Option<kir::Amount>, Failure> {
    let Some(printed) = source["oracle_text"]
        .as_str()
        .and_then(|text| text.strip_prefix("Each player draws "))
    else {
        return Ok(None);
    };
    let fail = || {
        unsupported(
            "each_player_draw",
            "requires a complete bound each-player draw spell",
        )
    };
    if !matches!(source["type_line"].as_str(), Some("Instant" | "Sorcery")) {
        return Err(fail());
    }
    let abilities: Value = decode(result.abilities.get(), "abilities")?;
    let (count, amount) = if printed == "X cards." {
        (kir::Amount::announced_x(), json!({"amount":"x"}))
    } else {
        let n = abilities[0]["effects"][0]["steps"][0]["amount"]["value"]
            .as_u64()
            .and_then(|n| u32::try_from(n).ok())
            .filter(|n| *n > 0)
            .ok_or_else(fail)?;
        let literal = printed
            .strip_suffix(if n == 1 { " card." } else { " cards." })
            .ok_or_else(fail)?;
        if !(n == 1 && literal == "a") && !matches_fixed_amount(literal, i64::from(n)) {
            return Err(fail());
        }
        (
            kir::Amount::fixed(i64::from(n)),
            json!({"amount":"fixed","value":n}),
        )
    };
    let x = count.is_announced_x();
    let who = json!({"player":"each-player"});
    let step = json!({"effect":"draw","who":who,"amount":amount});
    if abilities != json!([{"kind":"spell","effects":[{"steps":[step]}]}])
        || source["mana_cost"]
            .as_str()
            .ok_or_else(fail)?
            .matches("{X}")
            .count()
            != usize::from(x)
    {
        return Err(fail());
    }

    let base = "/faces/0/abilities/0";
    let path = "/faces/0/abilities/0/effects/0/steps/0";
    let mut graph = Bindings::new(source["oracle_id"].as_str().ok_or_else(fail)?, source);
    graph.ability(base, "spell", "spell-controller");
    let scope = graph.id(base, "ability");
    let field = format!("{path}/who");
    let players = graph.node(&field, "query", "selection", json!({"scope":scope,"expression":who,
        "cardinality":"many","timing":"resolution","selection":"evaluate-reference","rules":["608.2h"]}));
    graph.reference(&field, &who, &players, "608.2h");
    if x {
        let announced = graph.node(base, "announced-x", "announced-number",
            json!({"scope":scope,"chooser":graph.id(base,"you"),"announcement":"cast","rules":["107.3a","601.2b"]}));
        graph.reference(&format!("{path}/amount"), &amount, &announced, "107.3a");
    }
    let event = graph.node(
        path,
        "event",
        "event",
        json!({"scope":scope,"operation":"draw",
        "inputs":[players],"availability":"when-performed","rules":["608.2c"]}),
    );
    // The number of resulting objects is unknown even for a fixed count per player.
    graph.node(path, "objects", "event-result", json!({"event":event,"result":"objects","cardinality":"unknown",
        "availability":"if-produced","identity":"new-objects","description":{"cls":"card"},"rules":["608.2h"]}));
    graph.node(
        path,
        "quantity",
        "event-result",
        json!({"event":event,"result":"quantity","cardinality":"one",
        "availability":"if-produced","identity":"measured-quantity","rules":["608.2h"]}),
    );
    graph.validate(binding, "requires complete each-player draw bindings")?;
    Ok(Some(count))
}

#[test]
fn each_player_draw() {
    use crate::tests::changed_source;
    const MANIFEST: &str = include_str!("../../../tests/fixtures/archive-manifest-current.json");
    // Even without the private rows, prove a controller binding cannot authorize all players.
    let controller =
        include_str!("../../../tests/fixtures/273b339c-964b-4a18-8eb5-ceb8abcdfd9e.json");
    let stale = changed_source(controller, |r| {
        let text = "Each player draws two cards.";
        r["source"]["oracle_text"] = json!(text);
        let result = &mut r["faces"][0]["result"];
        result["lines"][0]["text"] = json!(text);
        result["abilities"][0]["effects"][0]["steps"][0]["who"] = json!({"player":"each-player"});
        result["lines"][0]["ability"] = result["abilities"][0].clone();
    });
    assert!(
        matches!(compile(MANIFEST, &stale), CompileResult::Unsupported { path, .. } if path == "binding")
    );
    let Some(path) = std::env::var_os("MTG_EACH_PLAYER_DRAW_ARCHIVE_PROBE") else {
        return;
    };
    let rows = std::fs::read_to_string(path).unwrap();
    let pins = [
        (
            16434,
            "6396d0c1b0870998b12b5e755702eae14f6ca027021b51c4ff8d47252cf06ee7",
            kir::Amount::fixed(2),
            Capability::CastInstantV1,
        ),
        (
            29766,
            "ceb103d282b6979a50401010eab88d81a960950d42ac8fd17b8e10358c0a1658",
            kir::Amount::announced_x(),
            Capability::CastSorceryV1,
        ),
    ];
    assert_eq!(rows.lines().count(), pins.len());
    for (raw, (row, hash, count, cast)) in rows.lines().zip(pins) {
        assert_eq!(sha256(raw.as_bytes()), hash);
        let compiled = compile(MANIFEST, raw);
        let CompileResult::Lowered { definition: d } = compiled else {
            panic!("{compiled:?}")
        };
        assert_eq!(d.provenance.source_index, row - 2);
        assert_eq!(d.x_mana_cost, count.is_announced_x());
        assert_eq!(
            d.capabilities,
            BTreeSet::from([
                Capability::NormalLayoutV1,
                cast,
                Capability::EachPlayerDrawV1
            ])
        );
        let p = d.spell.as_ref().unwrap();
        assert!(p.targets.is_empty() && p.modes.is_empty() && p.additional_sacrifice.is_none());
        assert_eq!(
            p.instructions,
            [Instruction::act(
                Verb::Draw {
                    player: PlayerExpr::EachPlayer,
                    amount: count
                },
                0
            )]
        );

        for case in [
            "who",
            "amount",
            "optional",
            "extra step",
            "reference",
            "event",
            "text",
            "cost",
        ] {
            let bad = changed_source(raw, |r| {
                let result = &mut r["faces"][0]["result"];
                let steps = &mut result["abilities"][0]["effects"][0]["steps"];
                match case {
                    "who" => steps[0]["who"]["player"] = json!("each-opponent"),
                    "amount" => steps[0]["amount"] = json!({"amount":"fixed","value":3}),
                    "optional" => steps[0]["optional"] = json!(true),
                    "extra step" => {
                        let second = steps[0].clone();
                        steps.as_array_mut().unwrap().push(second);
                    }
                    "reference" => {
                        let refs = r["binding"]["references"].as_array_mut().unwrap();
                        refs.last_mut().unwrap()["target"] = json!("foreign scope");
                    }
                    "event" => {
                        let event = r["binding"]["nodes"]
                            .as_array_mut()
                            .unwrap()
                            .iter_mut()
                            .find(|node| node["node"] == "event")
                            .unwrap();
                        event["inputs"] = json!([]);
                    }
                    "text" => {
                        let text = "Each opponent draws two cards.";
                        result["lines"][0]["text"] = json!(text);
                        r["source"]["oracle_text"] = json!(text);
                    }
                    "cost" => {
                        let cost = if d.x_mana_cost { "{U}" } else { "{X}{U}" };
                        r["source"]["mana_cost"] = json!(cost);
                        r["binding"]["nodes"][1]["manaCost"] = json!(cost);
                    }
                    _ => unreachable!(),
                }
                let result = &mut r["faces"][0]["result"];
                result["lines"][0]["ability"] = result["abilities"][0].clone();
            });
            assert!(
                !matches!(compile(MANIFEST, &bad), CompileResult::Lowered { .. }),
                "row {row}, {case}"
            );
        }
    }
}
