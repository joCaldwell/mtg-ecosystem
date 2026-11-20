//! CR 119.3, 608.2c/h: "life lost this way" measures the loss from the preceding
//! instruction. CR 107.3a/601.2b bind the X of the loss to this cast's announced X.
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;
use crate::bindings::Bindings;
use serde_json::{Value, json};

const BASE: &str = "/faces/0/abilities/0";
const LOSS: &str = "/faces/0/abilities/0/effects/0/steps/0";
const GAIN: &str = "/faces/0/abilities/0/effects/1/steps/0";

fn graph<'a>(source: &Value, identity: &'a str, announced_x: bool, typed: bool) -> Bindings<'a> {
    const RULES: &[&str] = &["119.3", "608.2c", "608.2f", "608.2h"];
    let mut graph = Bindings::new(identity, source);
    graph.ability(BASE, "spell", "spell-controller");
    let scope = graph.id(BASE, "ability");
    let controller = graph.id(BASE, "you");
    let x = announced_x.then(|| graph.node(BASE, "announced-x", "announced-number",
        json!({"scope":scope,"chooser":controller,"announcement":"cast","rules":["107.3a","601.2b"]})));
    let loss_who = format!("{LOSS}/who");
    let expression = json!({"player":"each-opponent"});
    let opponents = graph.node(
        &loss_who,
        "query",
        "selection",
        json!({
            "scope":scope,"expression":expression,"cardinality":"many","timing":"resolution",
            "selection":"evaluate-reference","rules":["608.2h"]
        }),
    );
    graph.reference(&loss_who, &expression, &opponents, "608.2h");
    if let Some(x) = x {
        graph.reference(
            &format!("{LOSS}/amount"),
            &json!({"amount":"x"}),
            &x,
            "107.3a",
        );
    }
    graph.event(BASE, LOSS, "lose-life", &opponents, false);
    graph.node(
        LOSS,
        "quantity",
        "event-result",
        json!({
            "event":graph.id(LOSS,"event"),"result":"quantity","cardinality":"unknown",
            "availability":"if-produced","identity":"measured-quantity","rules":["608.2h"]
        }),
    );
    graph.reference(
        &format!("{GAIN}/who"),
        &json!({"player":"you"}),
        &controller,
        "109.5",
    );
    if typed {
        graph.node(
            &format!("{GAIN}/amount"),
            "event-aggregate",
            "event-aggregate",
            json!({"scope":scope,"event":graph.id(LOSS,"event"),"measure":"life-lost","rules":RULES}),
        );
    }
    graph.event(BASE, GAIN, "gain-life", &controller, false);
    graph.result(GAIN, "quantity", "measured-quantity", None);
    graph
}
fn expected_graph(source: &Value, identity: &str, announced_x: bool) -> Value {
    graph(source, identity, announced_x, false).with_scope_diagnostic(json!({
        "code":"unsupported-scope","path":format!("{GAIN}/amount"),
        "message":"An event or history quantity needs its exact measured event(s), participants and period bound; no nearby number substitutes for it.",
        "rules":["119.3","120.3","608.2h"]
    }))
}

pub(super) fn lower(
    source: &Value,
    result: &ParseResult,
    binding: &Value,
) -> Result<Option<OpponentDrainAmount>, Failure> {
    let text = source["oracle_text"].as_str().unwrap_or_default();
    let Some(rest) = text.strip_prefix("Each opponent loses ") else {
        return Ok(None);
    };
    let fail = || {
        unsupported(
            "opponent_life_drain",
            "requires a complete opponent loss and measured controller gain sorcery",
        )
    };
    let literal = rest
        .strip_suffix(" life. You gain life equal to the life lost this way.")
        .ok_or_else(fail)?;
    let x = literal == "X";
    let (amount, raw_amount) = if x {
        (OpponentDrainAmount::AnnouncedX, json!({"amount":"x"}))
    } else {
        let n = literal
            .parse::<u32>()
            .ok()
            .filter(|n| *n > 0 && n.to_string() == literal)
            .ok_or_else(fail)?;
        (
            OpponentDrainAmount::Fixed {
                value: i64::from(n).into(),
            },
            json!({"amount":"fixed","value":n}),
        )
    };
    let raw: Value = decode(result.abilities.get(), "abilities")?;
    // This complete two-sentence block proves the operand's event and participant
    // set: exactly one prior event loses life, exclusively for those opponents.
    // The fused instruction carries the sum of their actual losses into the gain.
    if source["type_line"] != "Sorcery"
        || !matches!(result.lines.as_slice(), [line] if line.text == text)
        || source["mana_cost"]
            .as_str()
            .ok_or_else(fail)?
            .matches("{X}")
            .count()
            != usize::from(x)
        || raw
            != json!([{"kind":"spell","effects":[
                {"steps":[{"effect":"lose-life","who":{"player":"each-opponent"},"amount":raw_amount}]},
                {"steps":[{"effect":"gain-life","who":{"player":"you"},"amount":{
                    "amount":"event-quantity","event":"life-change","change":"lost","scope":"this-way"
                }}]}
            ]}])
    {
        return Err(fail());
    }
    let identity = source["oracle_id"].as_str().ok_or_else(fail)?;
    if binding["referencesBound"] == true {
        graph(source, identity, x, true).validate_ordered_with_features(
            binding,
            &[json!({"feature":"event-aggregate","path":format!("{GAIN}/amount"),
                "message":"Sum the actual outcomes of the bound instruction across its participants.",
                "rules":["119.3","608.2c","608.2f","608.2h"]})],
            "requires the exact opponent loss and controller gain aggregate graph",
        )?;
    } else if *binding != expected_graph(source, identity, x) {
        return Err(unsupported(
            "binding",
            "requires the exact opponent loss and controller gain diagnostic graph",
        ));
    }
    Ok(Some(amount))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::changed_source;

    const MANIFEST: &str = include_str!("../../../tests/fixtures/archive-manifest-current.json");

    fn synthetic(x: bool) -> String {
        changed_source(
            include_str!("../../../tests/fixtures/lightning-bolt.json"),
            |r| {
                let source = &mut r["source"];
                let (literal, amount, cost) = if x {
                    ("X", json!({"amount":"x"}), "{X}{R}")
                } else {
                    ("1", json!({"amount":"fixed","value":1}), "{R}")
                };
                let text = format!(
                    "Each opponent loses {literal} life. You gain life equal to the life lost this way."
                );
                source["oracle_text"] = json!(text);
                source["mana_cost"] = json!(cost);
                source["type_line"] = json!("Sorcery");
                let binding = expected_graph(source, source["oracle_id"].as_str().unwrap(), x);
                r["binding"] = binding;
                let result = &mut r["faces"][0]["result"];
                result["abilities"] = json!([{"kind":"spell","effects":[
                    {"steps":[{"effect":"lose-life","who":{"player":"each-opponent"},"amount":amount}]},
                    {"steps":[{"effect":"gain-life","who":{"player":"you"},"amount":{"amount":"event-quantity",
                        "event":"life-change","change":"lost","scope":"this-way"}}]}
                ]}]);
                result["lines"][0]["ability"] = result["abilities"][0].clone();
                result["lines"][0]["text"] = json!(text);
            },
        )
    }

    fn witness(raw: &str, amount: OpponentDrainAmount) {
        use Capability::*;
        let compiled = compile(MANIFEST, raw);
        let CompileResult::Lowered { definition: d } = compiled else {
            panic!("{compiled:?}")
        };
        assert_eq!(d.x_mana_cost, amount == OpponentDrainAmount::AnnouncedX);
        assert_eq!(
            d.capabilities,
            BTreeSet::from([
                NormalLayoutV1,
                CastSorceryV1,
                OpponentLifeDrainV1,
                LifeLossV1,
                LifeGainV1
            ])
        );
        let p = d.spell.as_ref().unwrap();
        assert!(
            p.targets.is_empty()
                && p.modes.is_empty()
                && p.optional_additional_costs.is_empty()
                && p.additional_sacrifice.is_none()
        );
        assert_eq!(
            p.instructions,
            [Instruction::DrainOpponents {
                amount,
                source_block: 0
            }]
        );
        for case in [
            "loss actor",
            "gain actor",
            "quantity",
            "scope",
            "event",
            "direction",
            "extra step",
            "extra ability",
            "optional",
            "loss amount",
            "missing loss result",
            "false binding",
            "diagnostic",
            "count",
            "reference",
            "type",
            "mana",
            "text",
        ] {
            let bad = changed_source(raw, |r| {
                let result = &mut r["faces"][0]["result"];
                let effects = &mut result["abilities"][0]["effects"];
                let gain = &mut effects[1]["steps"][0];
                match case {
                    "loss actor" => effects[0]["steps"][0]["who"]["player"] = json!("each-player"),
                    "gain actor" => gain["who"]["of"] = Value::Null,
                    "quantity" => gain["amount"] = json!({"amount":"x"}),
                    "scope" => gain["amount"]["scope"] = json!("this-turn"),
                    "event" => gain["amount"]["event"] = json!("damage"),
                    "direction" => gain["amount"]["change"] = json!("gained"),
                    "optional" => gain["optional"] = json!(true),
                    "loss amount" => {
                        effects[0]["steps"][0]["amount"] = json!({"amount":"fixed","value":0})
                    }
                    "extra step" => {
                        let first = effects[0]["steps"][0].clone();
                        effects[0]["steps"].as_array_mut().unwrap().push(first);
                    }
                    "extra ability" => {
                        let a = result["abilities"][0].clone();
                        result["abilities"].as_array_mut().unwrap().push(a);
                    }
                    "missing loss result" => r["binding"]["nodes"]
                        .as_array_mut()
                        .unwrap()
                        .retain(|n| !(n["path"] == LOSS && n["node"] == "event-result")),
                    "false binding" => r["binding"]["referencesBound"] = json!(true),
                    "diagnostic" => {
                        r["binding"]["diagnostics"][0]["path"] = json!(format!("{LOSS}/amount"))
                    }
                    "count" => r["binding"]["counts"]["unresolved"] = json!(1),
                    "reference" => r["binding"]["references"][0]["target"] = json!("foreign query"),
                    "type" => {
                        r["source"]["type_line"] = json!("Instant");
                        r["binding"]["nodes"][1]["typeLine"] = json!("Instant");
                    }
                    "mana" => {
                        let cost = if d.x_mana_cost { "{X}{X}{B}" } else { "{X}{B}" };
                        r["source"]["mana_cost"] = json!(cost);
                        r["binding"]["nodes"][1]["manaCost"] = json!(cost);
                    }
                    "text" => {
                        let text = "Each opponent loses 2 life. You gain 2 life.";
                        result["lines"][0]["text"] = json!(text);
                        r["source"]["oracle_text"] = json!(text);
                    }
                    _ => unreachable!(),
                }
                let result = &mut r["faces"][0]["result"];
                result["lines"][0]["ability"] = result["abilities"][0].clone();
            });
            assert!(
                !matches!(compile(MANIFEST, &bad), CompileResult::Lowered { .. }),
                "{case}"
            );
        }
    }

    #[test]
    fn opponent_life_drain() {
        witness(
            &synthetic(false),
            OpponentDrainAmount::Fixed { value: 1.into() },
        );
        witness(&synthetic(true), OpponentDrainAmount::AnnouncedX);
        if let Some(path) = std::env::var_os("MTG_OPPONENT_LIFE_DRAIN_ARCHIVE_PROBE") {
            // Two unmodified JSONL records, in physical archive order (19469, 30162).
            let rows = std::fs::read_to_string(path).unwrap();
            let pins = [
                (
                    19469,
                    "a77306b60e81ef512604cd681331a9cd37ea6d86d89f1ed45915f4aa4e9b5e5c",
                    OpponentDrainAmount::AnnouncedX,
                ),
                (
                    30162,
                    "e2482338d88940a4b9094194fc4649922ee90917dd69109ac831526435b047f0",
                    OpponentDrainAmount::Fixed { value: 3.into() },
                ),
            ];
            assert_eq!(rows.lines().count(), pins.len());
            for (raw, (line, hash, amount)) in rows.lines().zip(pins) {
                assert_eq!(sha256(raw.as_bytes()), hash);
                assert_eq!(
                    serde_json::from_str::<Value>(raw).unwrap()["sourceIndex"],
                    line - 2
                );
                witness(raw, amount);
            }
        }
    }
}
