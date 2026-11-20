//! CR 109.5, 113.7a, 601.2h, 602.2b, 608.2c, 701.23b and 701.24a/e:
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;
use serde_json::{Value, json};

const SEARCH: &str = "Search your library for a basic land card, put ";
const BAD: &str = "requires a complete basic-land search, entry, and shuffle";

pub(super) fn activation(
    parsed: &ParseResult,
    source: &Value,
    binding: &Value,
    card: &Source,
) -> Result<Option<ActivatedAbility>, Failure> {
    let raw: Value = decode(parsed.abilities.get(), "abilities")?;
    if !raw.as_array().is_some_and(|abilities| {
        abilities.iter().any(|a| {
            a["kind"] == "activated"
                && a["effects"][0]["steps"][0]["effect"] == "search"
                && a["effects"][0]["steps"][0]["for"]["filter"]["supertypes"] == json!(["basic"])
                && a["costs"].as_array().is_some_and(|costs| {
                    costs.iter().any(|c| c["cost"] == "sacrifice")
                        && costs.iter().any(|c| c["cost"] == "tap-self")
                })
        })
    }) {
        return Ok(None);
    }
    let bad = || unsupported("abilities.effects.search", BAD);
    let Some([ability]) = raw.as_array().map(Vec::as_slice) else {
        return Err(bad());
    };
    let Some((printed_cost, printed_effect)) = card.oracle_text.split_once(": ") else {
        return Err(bad());
    };
    let (moved, force_tapped, effects) = search_effect(printed_effect).ok_or_else(bad)?;
    let costs = ability["costs"].as_array().ok_or_else(bad)?;
    let noun = match card.type_line.as_str() {
        "Land" => "land",
        "Artifact" => "artifact",
        _ => return Err(bad()),
    };
    let tap = json!({"cost":"tap-self"});
    let object = json!({"ref":"this","noun":noun});
    let sacrifice = json!({"cost":"sacrifice","what":object});
    let (tap_index, pay_life, cost, verified_cost) = match costs.as_slice() {
        [t, s] if noun == "land" && *t == tap && *s == sacrifice => (
            0,
            false,
            composite(None),
            "{T}, Sacrifice this land".to_owned(),
        ),
        [t, life, s] if noun == "land" && *t == tap && *s == sacrifice => {
            if *life != json!({"cost":"pay-life","amount":life["amount"]}) {
                return Err(bad());
            }
            let life = typed_fetchland::life(&life["amount"])?;
            let text = format!("{{T}}, Pay {life} life, Sacrifice this land");
            (
                0,
                true,
                ActivationCost::TapPayLifeSacrificeSource { life },
                text,
            )
        }
        [mana, t, s] if noun == "artifact" && *t == tap && *s == sacrifice => {
            let Some((mana_text, _)) = printed_cost.split_once(", ") else {
                return Err(bad());
            };
            let symbols: Vec<_> = mana_text
                .strip_prefix('{')
                .and_then(|text| text.strip_suffix('}'))
                .ok_or_else(bad)?
                .split("}{")
                .collect();
            if symbols
                .iter()
                .any(|symbol| symbol.is_empty() || symbol.contains(['{', '}']))
                || *mana != json!({"cost":"mana","symbols":symbols})
            {
                return Err(bad());
            }
            (
                1,
                false,
                composite(Some(mana_cost(mana_text)?)),
                format!("{mana_text}, {{T}}, Sacrifice this artifact"),
            )
        }
        _ => return Err(bad()),
    };
    if printed_cost != verified_cost
        || *ability != json!({"kind":"activated","costs":costs,"effects":effects})
    {
        return Err(bad());
    }
    let mut graph = bindings::Bindings::new(&card.oracle_id, source);
    let root = "/faces/0/abilities/0";
    graph.ability(root, "activated", "activator");
    bind_costs(&mut graph, root, tap_index, pay_life, &object);
    bind_search(&mut graph, root, &moved);
    graph.validate(binding, BAD)?;
    Ok(Some(ActivatedAbility {
        functioning_zone: FunctioningZone::Battlefield,
        cost,
        program: basic_land_fetch_program(force_tapped),
    }))
}

pub(super) fn spell(
    parsed: &ParseResult,
    source: &Value,
    binding: &Value,
    card: &Source,
) -> Result<Option<SpellProgram>, Failure> {
    // The existing basic ramp lowering already handles "that card". The
    // pronoun form has an independently checked antecedent from this search.
    if card.oracle_text != format!("{SEARCH}it onto the battlefield tapped, then shuffle.") {
        return Ok(None);
    }
    let bad = || unsupported("effects.search", BAD);
    let (moved, _, effects) = search_effect(&card.oracle_text).ok_or_else(bad)?;
    let raw: Value = decode(parsed.abilities.get(), "abilities")?;
    if raw != json!([{"kind":"spell","effects":effects}]) {
        return Err(bad());
    }
    let timing = match card.type_line.split(" — ").next() {
        Some("Instant") => SpellTiming::Instant,
        Some("Sorcery") => SpellTiming::MainPhase,
        _ => return Err(bad()),
    };
    let mut graph = bindings::Bindings::new(&card.oracle_id, source);
    let root = "/faces/0/abilities/0";
    graph.ability(root, "spell", "spell-controller");
    bind_search(&mut graph, root, &moved);
    graph.validate(binding, BAD)?;
    Ok(Some(SpellProgram {
        timing,
        instructions: vec![Instruction::SearchBasicLands {
            maximum: 1,
            source_block: 0,
        }],
        ..basic_land_fetch_program(true)
    }))
}

fn composite(mana: Option<ManaCost>) -> ActivationCost {
    ActivationCost::Composite {
        costs: PermanentActivationCosts {
            tap: true,
            sacrifice: true,
            mana,
            remove_source_plus_one_counters: None,
        },
    }
}

fn find() -> Value {
    json!({"ref":"filter","filter":{"supertypes":["basic"],"types":["land"],"cls":"card"},
        "count":{"amount":"fixed","value":1}})
}

fn search_effect(printed: &str) -> Option<(Value, bool, Value)> {
    let (pronoun, remainder) = printed
        .strip_prefix(SEARCH)?
        .split_once(" onto the battlefield")?;
    let moved = match pronoun {
        "it" => json!({"ref":"it"}),
        "that card" => json!({"ref":"that","filter":{"cls":"card"}}),
        _ => return None,
    };
    let you = json!({"player":"you"});
    let mut movement = json!({"effect":"move-zone","what":moved,"to":{"zone":"battlefield"}});
    let force_tapped = match remainder {
        " tapped, then shuffle." => {
            movement["tapped"] = json!(true);
            true
        }
        ", then shuffle." => false,
        _ => return None,
    };
    let effects = json!([{"steps":[
        {"effect":"search","who":you,"zones":{"selection":"all","of":[{"zone":"library","owner":"your"}]},"for":find()},
        movement,
        {"effect":"shuffle","who":you,"library":{"zone":"library","owner":you}}
    ]}]);
    Some((moved, force_tapped, effects))
}

fn bind_costs(
    graph: &mut bindings::Bindings<'_>,
    root: &str,
    tap_index: usize,
    pay_life: bool,
    object: &Value,
) {
    let you = json!({"player":"you"});
    let controller = graph.id(root, "you");
    let source = graph.id("/source", "object");
    let sacrifice = tap_index + 1 + usize::from(pay_life);
    for index in 0..=sacrifice {
        graph.reference(
            &format!("{root}/costs/{index}/payer"),
            &you,
            &controller,
            "109.5",
        );
    }
    graph.reference(
        &format!("{root}/costs/{tap_index}/source"),
        &json!({"ref":"self"}),
        &source,
        "201.5",
    );
    graph.reference(
        &format!("{root}/costs/{sacrifice}/what"),
        object,
        &source,
        "201.5",
    );

    if pay_life {
        bind_measured_cost(graph, root, 1, "cost:pay-life");
    }
    bind_measured_cost(graph, root, sacrifice, "cost:sacrifice");
}

fn bind_measured_cost(graph: &mut bindings::Bindings<'_>, root: &str, index: usize, op: &str) {
    let path = format!("{root}/costs/{index}");
    graph.node(
        &path,
        "event",
        "event",
        json!({"scope":graph.id(root,"ability"),
        "operation":op,"inputs":[],"availability":"when-performed","rules":["608.2c"]}),
    );
    graph.result(&path, "quantity", "measured-quantity", None);
}

fn bind_search(graph: &mut bindings::Bindings<'_>, root: &str, moved: &Value) {
    let scope = graph.id(root, "ability");
    let controller = graph.id(root, "you");
    let you = json!({"player":"you"});
    let search = format!("{root}/effects/0/steps/0");
    let movement = format!("{root}/effects/0/steps/1");
    let shuffle = format!("{root}/effects/0/steps/2");
    let expression = find();
    let query = graph.node(
        &format!("{search}/for"),
        "query",
        "selection",
        json!({
            "scope":scope,"expression":expression,"cardinality":"one","timing":"resolution",
            "selection":"evaluate-reference","rules":["608.2h"]
        }),
    );
    graph.node(
        &search,
        "event",
        "event",
        json!({"scope":scope,"operation":"search",
        "inputs":[query,controller],"availability":"when-performed","rules":["608.2c"]}),
    );
    graph.event(root, &movement, "move-zone", &query, false);
    graph.node(&movement, "objects", "event-result", json!({"event":graph.id(&movement,"event"),
        "result":"objects","cardinality":"one","availability":"if-produced","identity":"zone-successors",
        "description":{"supertypes":["basic"],"types":["land"],"cls":"permanent"},"rules":["400.7j"]}));
    graph.event(root, &shuffle, "shuffle", &controller, false);
    for path in [
        format!("{search}/who"),
        format!("{search}/zones/of/0/owner"),
        format!("{movement}/who"),
        format!("{shuffle}/who"),
        format!("{shuffle}/library/owner"),
    ] {
        graph.reference(&path, &you, &controller, "109.5");
    }
    graph.reference(&format!("{search}/for"), &expression, &query, "608.2h");
    graph.reference_rules(
        &format!("{movement}/what"),
        moved,
        &query,
        &["608.2k", "400.7j", "608.2h"],
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    const MANIFEST: &str = include_str!("../../../tests/fixtures/archive-manifest-current.json");
    const WILDS: &str =
        include_str!("../../../tests/fixtures/a75445d3-1303-4bb5-89ad-26ea93fecd48.json");
    const VISTA: &str =
        include_str!("../../../tests/fixtures/032b8a0d-491a-4a12-ab9f-689010054d5b.json");
    const BAUBLE: &str =
        include_str!("../../../tests/fixtures/31f15274-301b-47c5-ba19-0ced04520878.json");
    const ROOTS: &str =
        include_str!("../../../tests/fixtures/9e0fd3bf-f47a-4f06-8ff1-73f6bf5d1e03.json");

    #[test]
    fn basic_land_fetch_checks_exact_sources_cost_and_antecedent() {
        for (raw, cost, tapped) in [
            (WILDS, composite(None), true),
            (
                VISTA,
                ActivationCost::TapPayLifeSacrificeSource { life: 1.into() },
                false,
            ),
            (
                BAUBLE,
                composite(Some(ManaCost {
                    generic: 2.into(),
                    colored: vec![],
                })),
                true,
            ),
        ] {
            let result = compile(MANIFEST, raw);
            let CompileResult::Lowered { definition } = result else {
                panic!("{result:?}");
            };
            let [ability] = definition.activated.as_slice() else {
                panic!()
            };
            assert_eq!(ability.cost, cost);
            assert_eq!(ability.program, basic_land_fetch_program(tapped));
            assert!(
                definition
                    .capabilities
                    .contains(&Capability::BasicLandFetchV1)
            );

            let mut flipped = (*definition).clone();
            flipped.activated[0].program = basic_land_fetch_program(!tapped);
            assert!(
                flipped.validate().is_err(),
                "entry mode must match the source"
            );

            let mut bad = *definition;
            bad.activated[0].program.modes.push(SpellMode {
                program: basic_land_fetch_program(tapped),
            });
            bad.capabilities = bad.requirements();
            assert!(bad.validate().is_err());
        }
        let result = compile(MANIFEST, ROOTS);
        let CompileResult::Lowered { definition } = result else {
            panic!("{result:?}");
        };
        assert_eq!(
            definition.spell.unwrap().instructions,
            [Instruction::SearchBasicLands {
                maximum: 1,
                source_block: 0
            }]
        );

        for raw in [WILDS, VISTA, ROOTS] {
            let bad = crate::tests::changed_source(raw, |value| {
                let reference = value["binding"]["references"]
                    .as_array_mut()
                    .unwrap()
                    .iter_mut()
                    .find(|reference| {
                        reference["path"] == "/faces/0/abilities/0/effects/0/steps/1/what"
                    })
                    .unwrap();
                let target = reference["target"].as_str().unwrap();
                reference["target"] = json!(target.replace("0/for#query", "1#objects"));
            });
            assert!(
                matches!(compile(MANIFEST, &bad), CompileResult::Unsupported { path, .. } if path == "binding")
            );
        }
        let bad = crate::tests::changed_source(VISTA, |value| {
            let result = &mut value["faces"][0]["result"];
            result["abilities"][0]["effects"][0]["steps"][1]["tapped"] = json!(null);
            result["lines"][0]["ability"] = result["abilities"][0].clone();
        });
        assert!(!matches!(
            compile(MANIFEST, &bad),
            CompileResult::Lowered { .. }
        ));
    }
}
