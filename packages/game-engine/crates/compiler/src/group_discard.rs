//! CR 101.4, 109.5, 608.2c–d and 701.9. Each affected player chooses from
//! their own hand. The spell or trigger controller determines who is an opponent.
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;
use crate::bindings::Bindings;
use serde_json::{Value, json};

const BASE: &str = "/faces/0/abilities/0";
const STEP: &str = "/faces/0/abilities/0/effects/0/steps/0";

fn fail() -> Failure {
    unsupported(
        "group discard",
        "requires a complete bound fixed group discard",
    )
}

fn printed_discard(text: &str) -> Option<(GroupDiscardScope, bool, &str)> {
    use GroupDiscardScope::*;
    [
        ("Each player discards ", EachPlayer, false),
        ("Each opponent discards ", EachOpponent, false),
        (
            "When this creature enters, each player discards ",
            EachPlayer,
            true,
        ),
        (
            "When this creature enters, each opponent discards ",
            EachOpponent,
            true,
        ),
    ]
    .into_iter()
    .find_map(|(prefix, scope, entry)| text.strip_prefix(prefix).map(|rest| (scope, entry, rest)))
}

pub(super) fn lower(
    result: &ParseResult,
    source: &Value,
    binding: &Value,
    kind: CardKind,
) -> Result<Option<Instruction>, Failure> {
    let Some((scope, entry, amount)) = result
        .lines
        .iter()
        .find_map(|line| printed_discard(&line.text))
    else {
        return Ok(None);
    };
    let [line] = result.lines.as_slice() else {
        return Err(fail());
    };
    if source["oracle_text"].as_str() != Some(line.text.as_str())
        || source["keywords"] != json!([])
        || !(entry && kind == CardKind::Creature
            || !entry && matches!(kind, CardKind::Instant | CardKind::Sorcery))
    {
        return Err(fail());
    }
    let raw: Value = decode(result.abilities.get(), "abilities")?;
    let count = raw[0]["effects"][0]["steps"][0]["what"]["count"]["value"]
        .as_u64()
        .and_then(|n| u32::try_from(n).ok())
        .filter(|&n| n > 0)
        .ok_or_else(fail)?;
    let word = amount
        .strip_suffix(if count == 1 { " card." } else { " cards." })
        .ok_or_else(fail)?;
    if !(count == 1 && word == "a" || matches_fixed_amount(word, i64::from(count))) {
        return Err(fail());
    }
    let who = json!({"player":match scope {
        GroupDiscardScope::EachPlayer => "each-player",
        GroupDiscardScope::EachOpponent => "each-opponent",
    }});
    let what =
        json!({"ref":"filter","filter":{"cls":"card"},"count":{"amount":"fixed","value":count}});
    let effects = json!([{"steps":[{"effect":"discard","who":who,"what":what}]}]);
    let mut graph = Bindings::new(source["oracle_id"].as_str().ok_or_else(fail)?, source);
    let ability = if entry {
        graph.ability(BASE, "triggered", "trigger-controller");
        let trigger = format!("{BASE}/trigger");
        let subject = json!({"ref":"this","noun":"creature"});
        let object = graph.id("/source", "object");
        graph.reference(&format!("{trigger}/what"), &subject, &object, "201.5");
        graph.event(BASE, &trigger, "trigger:enters", &object, false);
        graph.result(
            &trigger,
            "objects",
            "same-objects",
            Some(&json!({"types":["creature"]})),
        );
        json!({"kind":"triggered","trigger":{"trigger":"enters","what":subject},"effects":effects})
    } else {
        graph.ability(BASE, "spell", "spell-controller");
        json!({"kind":"spell","effects":effects})
    };
    if raw != json!([ability]) {
        return Err(fail());
    }

    let ability = graph.id(BASE, "ability");
    let mut query = |field: &str, expression: &Value, cardinality: &str| {
        let path = format!("{STEP}/{field}");
        let id = graph.node(&path, "query", "selection", json!({
            "scope":ability,"expression":expression,"cardinality":cardinality,"timing":"resolution",
            "selection":"evaluate-reference","rules":["608.2h"]
        }));
        graph.reference(&path, expression, &id, "608.2h");
        id
    };
    let actors = query("who", &who, "many");
    let cards = query("what", &what, if count == 1 { "one" } else { "many" });
    let event = graph.node(STEP, "event", "event", json!({
        "scope":ability,"operation":"discard","inputs":[cards,actors],"availability":"when-performed","rules":["608.2c"]
    }));
    graph.node(
        STEP,
        "objects",
        "event-result",
        json!({
            "event":event,"result":"objects","cardinality":"unknown","availability":"if-produced",
            "identity":"zone-successors","description":{"cls":"card"},"rules":["400.7j"]
        }),
    );
    if count > 1 {
        graph.result(STEP, "quantity", "measured-quantity", None);
    }
    graph.validate(binding, "requires complete group discard bindings")?;
    Ok(Some(Instruction::discard(
        scope.player(),
        Amount::fixed(i64::from(count)),
        false,
        0,
    )))
}

pub(super) fn entry_trigger(discard: Instruction) -> TriggeredAbility {
    TriggeredAbility {
        death_filter: None,
        intervening_if: None,
        functioning_zone: FunctioningZone::Battlefield,
        condition: TriggerCondition::SelfEntersBattlefield,
        program: SpellProgram {
            additional_sacrifice: None,
            optional_additional_costs: vec![],
            timing: SpellTiming::Instant,
            resolution: SpellResolution::Instructions,
            functioning_zone: FunctioningZone::Stack,
            targets: vec![],
            instructions: vec![discard],
            modes: vec![],
        },
    }
}

#[cfg(test)]
#[test]
fn group_discard() {
    use GroupDiscardScope::*;
    // Exercise the source vs. binding boundary even without a local archive.
    let text = "Each opponent discards two cards.";
    let source = json!({"oracle_id":"example","oracle_text":text,"keywords":[]});
    let step = json!({"effect":"discard","who":{"player":"each-opponent"},"what":{"ref":"filter","filter":{"cls":"card"},"count":{"amount":"fixed","value":2}}});
    for (qualifier, value, expected_path) in [
        (None, Value::Null, "binding"),
        (Some("random"), json!(false), "group discard"),
        (Some("optional"), json!(true), "group discard"),
    ] {
        let mut step = step.clone();
        if let Some(key) = qualifier {
            step[key] = value;
        }
        let ability = json!({"kind":"spell","effects":[{"steps":[step]}]});
        let result: ParseResult = serde_json::from_str(&json!({
            "name":"example","ok":true,"abilities":[ability],"lines":[{"text":text,"ok":true,"ability":ability}]
        }).to_string()).unwrap();
        assert_eq!(
            lower(&result, &source, &json!({}), CardKind::Instant)
                .unwrap_err()
                .path,
            expected_path
        );
    }

    let Some(path) = std::env::var_os("MTG_GROUP_DISCARD_ARCHIVE_PROBE") else {
        return;
    };
    const MANIFEST: &str = include_str!("../../../tests/fixtures/archive-manifest-current.json");
    let rows = std::fs::read_to_string(path).unwrap();
    // Physical lines in the frozen archive, complete record digests, scope and count.
    let expected = [
        (
            4871,
            "518337f90e5be7c64e57f197fde6bdd85b018308d06dee6d39a3e3b694565493",
            EachOpponent,
            1,
        ),
        (
            7059,
            "3571053356afee1478988943f0416d40f60698e79ec15857b7b8b1276f242bd6",
            EachOpponent,
            1,
        ),
        (
            10873,
            "4e8a14cf78dc75e4dbaf4b986d2e972e14686bee7d6e9c95bc2f47a193342750",
            EachPlayer,
            1,
        ),
        (
            15154,
            "974166b9c719735f19464482cbe38df9f8a3ba40b4215e9d5cf8decc02f80387",
            EachOpponent,
            1,
        ),
        (
            17399,
            "f9558c510e495aa0cdab92be865a58d40501eb74d9ebd0819d6cc0be4ee26255",
            EachPlayer,
            3,
        ),
        (
            18402,
            "f09c3be40a31078266326dd8176ef0fdf3258f0a41c0d5bba672317b4d12bdf4",
            EachOpponent,
            1,
        ),
        (
            19790,
            "576b9e5363d4fdbf05d7f4f1ddc3c793df835f76e948c7cb1353ea9d299e7513",
            EachOpponent,
            2,
        ),
        (
            21356,
            "4ec138d5f9cdf065585209fb597edba12b865d20a292350e997e16ac9ed2b6fb",
            EachOpponent,
            1,
        ),
        (
            25173,
            "09133a2d626cb7d7e5a282866b7ede01971fd4f3206564b43a5ab571d18994c9",
            EachOpponent,
            1,
        ),
        (
            27376,
            "3f3f7739dd92cb2f280307cad7b62530af3fc07d95d66dbfdbf27b303fa76c82",
            EachPlayer,
            1,
        ),
        (
            31660,
            "4f986d7d605fb978bf81b6ffca0598b2e0be2e191f53b14551309e094c9f4386",
            EachOpponent,
            1,
        ),
        (
            34094,
            "ac6a25b07eca90c7eb5a79c3a44c055ddc39ee79dad3efdb538eabc4439832c4",
            EachOpponent,
            1,
        ),
    ];
    assert_eq!(rows.lines().count(), expected.len());
    for (row, (line, digest, scope, count)) in rows.lines().zip(expected) {
        assert_eq!(sha256(row.as_bytes()), digest);
        let d = match compile(MANIFEST, row) {
            CompileResult::Lowered { definition } => definition,
            rejected => panic!("line {line}: {rejected:?}"),
        };
        assert_eq!(d.provenance.source_index + 2, line);
        assert!(d.capabilities.contains(&Capability::GroupHandDiscardV1));
        let p = if matches!(line, 17399 | 19790) {
            assert_eq!(d.characteristics.kind, CardKind::Sorcery);
            d.spell.as_ref().unwrap()
        } else {
            assert_eq!(d.characteristics.kind, CardKind::Creature);
            assert_eq!(
                d.characteristics.has_type(CardKind::Artifact),
                line == 34094
            );
            assert_eq!(
                d.triggered[0].condition,
                TriggerCondition::SelfEntersBattlefield
            );
            &d.triggered[0].program
        };
        assert!(p.targets.is_empty());
        assert_eq!(
            p.instructions,
            [Instruction::discard(
                scope.player(),
                Amount::fixed(count),
                false,
                0
            )]
        );

        // The first creature and first spell exercise different actor and result graphs.
        if matches!(line, 4871 | 17399) {
            assert!(matches!(
                compile(MANIFEST, &tests::changed_source(row, |_| {})),
                CompileResult::Lowered { .. }
            ));
            for mutation in 0..8 {
                let bad = tests::changed_source(row, |r| {
                    let result = &mut r["faces"][0]["result"];
                    let step = &mut result["abilities"][0]["effects"][0]["steps"][0];
                    match mutation {
                        0 => step["what"]["filter"]["types"] = json!(["creature"]),
                        1 => step["random"] = json!(false),
                        2 => step["who"]["selectionId"] = json!("line:0:0"),
                        3 => step["what"]["count"]["value"] = json!(0),
                        4 => {
                            *step = json!({"effect":"draw","who":{"player":"you"},"amount":{"amount":"fixed","value":1}})
                        }
                        _ => {}
                    }
                    result["lines"][0]["ability"] = result["abilities"][0].clone();
                    if mutation == 5 {
                        r["binding"]["references"][0]["target"] = json!("unrelated");
                    } else if mutation == 6 {
                        let nodes = r["binding"]["nodes"].as_array_mut().unwrap();
                        let event = nodes
                            .iter_mut()
                            .find(|n| n["operation"] == "discard")
                            .unwrap();
                        event["inputs"].as_array_mut().unwrap().reverse();
                    } else if mutation == 7 {
                        let nodes = r["binding"]["nodes"].as_array_mut().unwrap();
                        let actors = nodes
                            .iter_mut()
                            .find(|n| {
                                n["node"] == "selection"
                                    && n["path"].as_str().unwrap().ends_with("/who")
                            })
                            .unwrap();
                        actors["timing"] = json!("stack-announcement");
                    }
                });
                assert!(
                    !matches!(compile(MANIFEST, &bad), CompileResult::Lowered { .. }),
                    "line {line} mutation {mutation}"
                );
            }
        }
    }
}
