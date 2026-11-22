//! CR 119.3, 608.2c/h: "life lost this way" measures the loss from the preceding
//! instruction. CR 107.3a/601.2b bind the X of the loss to this cast's announced X.
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;
use crate::bindings::Bindings;
use serde_json::{Value, json};

const BASE: &str = "/faces/0/abilities/0";
const LOSS: &str = "/faces/0/abilities/0/effects/0/steps/0";
const GAIN: &str = "/faces/0/abilities/0/effects/1/steps/0";

fn graph<'a>(source: &Value, identity: &'a str, announced_x: bool) -> Bindings<'a> {
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
    graph.node(
        &format!("{GAIN}/amount"),
        "event-aggregate",
        "event-aggregate",
        json!({"scope":scope,"event":graph.id(LOSS,"event"),"measure":"life-lost","rules":RULES}),
    );
    graph.event(BASE, GAIN, "gain-life", &controller, false);
    graph.result(GAIN, "quantity", "measured-quantity", None);
    graph
}
pub(crate) fn lower(
    source: &Value,
    result: &ParseResult,
    binding: &Value,
) -> Result<Option<kir::Amount>, Failure> {
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
        (kir::Amount::announced_x(), json!({"amount":"x"}))
    } else {
        let n = literal
            .parse::<u32>()
            .ok()
            .filter(|n| *n > 0 && n.to_string() == literal)
            .ok_or_else(fail)?;
        (
            kir::Amount::fixed(i64::from(n)),
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
    graph(source, identity, x).validate_ordered_with_features(
        binding,
        &[
            json!({"feature":"event-aggregate","path":format!("{GAIN}/amount"),
            "message":"Sum the actual outcomes of the bound instruction across its participants.",
            "rules":["119.3","608.2c","608.2f","608.2h"]}),
        ],
        "requires the exact opponent loss and controller gain aggregate graph",
    )?;
    Ok(Some(amount))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::changed_source;

    const MANIFEST: &str =
        include_str!("../../../../tests/fixtures/archive-manifest-phase1-r4.json");
    const FIXED: &str =
        include_str!("../../../../tests/fixtures/phase1-r4/fixed-opponent-drain.json");
    const ANNOUNCED: &str =
        include_str!("../../../../tests/fixtures/phase1-r4/announced-x-opponent-drain.json");

    fn witness(raw: &str, amount: kir::Amount) {
        use Capability::*;
        let compiled = compile(MANIFEST, raw);
        let CompileResult::Lowered { definition: d } = compiled else {
            panic!("{compiled:?}")
        };
        assert_eq!(d.x_mana_cost, amount.is_announced_x());
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
            [Instruction::drain(kir::Context::Spell, amount, 0)]
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
            "feature",
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
                    "false binding" => r["binding"]["referencesBound"] = json!(false),
                    "diagnostic" => {
                        r["binding"]["diagnostics"] = json!([{"code":"unsupported-scope",
                        "path":format!("{LOSS}/amount"),"message":"unbound","rules":["608.2h"]}])
                    }
                    "feature" => r["binding"]["engineFeatures"] = json!([]),
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
        witness(FIXED, kir::Amount::fixed(3));
        witness(ANNOUNCED, kir::Amount::announced_x());
        // The pre-Pass-2 diagnostic graph (an unsupported-scope hole where the
        // gain measures the loss) is not an alternative to the aggregate.
        let legacy = changed_source(FIXED, |r| {
            let b = &mut r["binding"];
            b["nodes"]
                .as_array_mut()
                .unwrap()
                .retain(|n| n["node"] != "event-aggregate");
            b["engineFeatures"] = json!([]);
            b["diagnostics"] = json!([{"code":"unsupported-scope","path":format!("{GAIN}/amount"),
                "message":"An event or history quantity needs its exact measured event(s), participants and period bound; no nearby number substitutes for it.",
                "rules":["119.3","120.3","608.2h"]}]);
            b["referencesBound"] = json!(false);
            b["fullyBound"] = json!(false);
        });
        assert!(matches!(compile(MANIFEST, &legacy),
            CompileResult::Unsupported { path, .. } if path.starts_with("binding")));
        for (raw, line) in [(ANNOUNCED, 19469), (FIXED, 30162)] {
            assert_eq!(
                serde_json::from_str::<Value>(raw).unwrap()["sourceIndex"],
                line - 2
            );
        }
    }
}
