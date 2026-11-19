//! CR 109.5, 121.2, 305.2, 514.2, 608.2c and 611.2a:
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;
use serde_json::{Value, json};

pub(super) fn lower(
    abilities: &[Ability],
    parsed: &ParseResult,
    binding: &Value,
    source: &Source,
) -> Result<Option<[Instruction; 2]>, Failure> {
    let has_step = abilities.iter().any(|ability| {
        let Ability::Spell { effects, .. } = ability else {
            return false;
        };
        effects
            .iter()
            .flat_map(|sentence| &sentence.steps)
            .any(|step| matches!(step, Step::AdditionalLandPlays { .. }))
    });
    let has_binding = binding["nodes"].as_array().is_some_and(|nodes| {
        nodes
            .iter()
            .any(|node| node["operation"] == "additional-land-plays")
    });
    let prints_allowance =
        source.oracle_text.contains("may play ") && source.oracle_text.contains(" additional land");
    if !has_step && !has_binding && !prints_allowance {
        return Ok(None);
    }
    let fail = || {
        unsupported(
            "land allowance",
            "requires complete fixed controller draw and land allowance",
        )
    };
    let [
        Ability::Spell {
            ability_word: None,
            effects,
        },
    ] = abilities
    else {
        return Err(fail());
    };
    let [
        Sentence {
            bindings,
            steps: draw,
            condition: None,
            condition_kind: None,
        },
        Sentence {
            bindings: allowance_bindings,
            steps: grant,
            condition: None,
            condition_kind: None,
        },
    ] = effects.as_slice()
    else {
        return Err(fail());
    };
    let [
        Step::Draw {
            amount: DrawAmount::Fixed { value },
            optional: false,
            ..
        },
    ] = draw.as_slice()
    else {
        return Err(fail());
    };
    let [
        Step::AdditionalLandPlays {
            allowance,
            duration,
        },
    ] = grant.as_slice()
    else {
        return Err(fail());
    };
    let additional = &allowance["additional"]["value"];
    let count = additional
        .as_number()
        .and_then(|n| n.to_string().parse::<Quantity>().ok())
        .filter(|n| !n.is_negative())
        .ok_or_else(fail)?;
    let draw = Quantity::from(i64::from(*value));
    let you = json!({"player":"you"});
    let raw: Value = decode(parsed.abilities.get(), "abilities")?;
    if !bindings.is_empty()
        || !allowance_bindings.is_empty()
        || duration.duration != "this-turn"
        || raw
            != json!([{"kind":"spell","effects":[
                {"steps":[{"effect":"draw","who":you,"amount":{"amount":"fixed","value":value}}]},
                {"steps":[{"effect":"additional-land-plays",
                    "allowance":{"who":you,"additional":{"amount":"fixed","value":additional}},
                    "duration":{"duration":"this-turn"}}]}
            ]}])
        || !printed_counts(&source.oracle_text, &draw, &count)
    {
        return Err(fail());
    }
    validate_binding(binding, source, if *value == 1 { "one" } else { "many" })?;
    Ok(Some([
        Instruction::DrawControllerCards {
            count: draw,
            source_block: 0,
        },
        Instruction::GrantLandPlaysThisTurn {
            count,
            source_block: 0,
        },
    ]))
}

fn printed_counts(text: &str, draw: &Quantity, grant: &Quantity) -> bool {
    let Some((draw_text, grant_text)) = text
        .strip_prefix("Draw ")
        .and_then(|text| text.split_once(". You may play "))
    else {
        return false;
    };
    let Some((draw_count, cards)) = draw_text.split_once(' ') else {
        return false;
    };
    let Some((grant_count, lands)) = grant_text.split_once(" additional ") else {
        return false;
    };
    let one = Quantity::from(1);
    let draw_noun = if *draw == one { "card" } else { "cards" };
    let grant_tail = if *grant == one {
        "land this turn."
    } else {
        "lands this turn."
    };
    cards == draw_noun
        && lands == grant_tail
        && printed_count(draw_count).as_ref() == Some(draw)
        && printed_count(grant_count).as_ref() == Some(grant)
}

// The same fixed count words as the archive parser's vocab.ts. Decimal counts
// stay exact and do not depend on the small printed-word vocabulary.
fn printed_count(text: &str) -> Option<Quantity> {
    const WORDS: [&str; 21] = [
        "zero",
        "one",
        "two",
        "three",
        "four",
        "five",
        "six",
        "seven",
        "eight",
        "nine",
        "ten",
        "eleven",
        "twelve",
        "thirteen",
        "fourteen",
        "fifteen",
        "sixteen",
        "seventeen",
        "eighteen",
        "nineteen",
        "twenty",
    ];
    const TENS: [&str; 8] = [
        "twenty", "thirty", "forty", "fifty", "sixty", "seventy", "eighty", "ninety",
    ];
    if let Ok(value) = text.parse() {
        return Some(value);
    }
    let value = match text {
        "a" | "an" => 1,
        "fifty" => 50,
        _ => match text.split_once('-') {
            Some((tens, units)) => {
                (TENS.iter().position(|word| *word == tens)? + 2) * 10
                    + WORDS[1..10].iter().position(|word| *word == units)?
                    + 1
            }
            None => WORDS[1..].iter().position(|word| *word == text)? + 1,
        },
    };
    Some(Quantity::from(value as i64))
}

fn validate_binding(binding: &Value, card: &Source, draw_cardinality: &str) -> Result<(), Failure> {
    let root = format!("card:{}", card.oracle_id);
    let source = format!("{root}/source#object");
    let face = format!("{root}/faces/0#face");
    let path = "/faces/0/abilities/0";
    let scope = format!("{root}{path}#ability");
    let you = format!("{root}{path}#you");
    let draw = format!("{path}/effects/0/steps/0");
    let grant = format!("{path}/effects/1/steps/0");
    let draw_event = format!("{root}{draw}#event");
    let expected = json!({
        "artifactKind":"oracle-reference-binding","schemaVersion":1,"executable":false,
        "identity":card.oracle_id,"rules":{
            "edition":RULES_EDITION,"url":"https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt"
        },
        "nodes":[
            {"id":source,"path":"/source","node":"source","identity":card.oracle_id,"rules":["201.5"]},
            {"id":face,"path":"/faces/0","node":"face","source":source,"sourceFaceIndex":0,
             "name":card.name,"layout":card.layout,"typeLine":card.type_line,"manaCost":card.mana_cost,
             "rules":["712"]},
            {"id":scope,"path":path,"node":"ability","source":source,"face":face,
             "abilityKind":"spell","you":you,"rules":["113","109.5"]},
            {"id":you,"path":path,"node":"runtime-player","scope":scope,"role":"spell-controller",
             "rules":["109.5"]},
            {"id":draw_event,"path":draw,"node":"event","scope":scope,"operation":"draw",
             "inputs":[you],"availability":"when-performed","rules":["608.2c"]},
            {"id":format!("{root}{draw}#objects"),"path":draw,"node":"event-result","event":draw_event,
             "result":"objects","cardinality":draw_cardinality,"availability":"if-produced","identity":"new-objects",
             "description":{"cls":"card"},"rules":["608.2h"]},
            {"id":format!("{root}{draw}#quantity"),"path":draw,"node":"event-result","event":draw_event,
             "result":"quantity","cardinality":"one","availability":"if-produced","identity":"measured-quantity",
             "rules":["608.2h"]},
            {"id":format!("{root}{grant}#event"),"path":grant,"node":"event","scope":scope,
             "operation":"additional-land-plays","inputs":[],"availability":"when-performed","rules":["608.2c"]}
        ],
        "references":[
            {"path":format!("{draw}/who"),"expression":{"player":"you"},"binding":"bound","target":you,"rules":["109.5"]},
            {"path":format!("{grant}/allowance/who"),"expression":{"player":"you"},"binding":"bound","target":you,"rules":["109.5"]}
        ],
        "diagnostics":[],"counts":{"obligations":2,"bound":2,"unresolved":0},
        "referencesBound":true,"engineFeatures":[],"fullyBound":true
    });
    if *binding != expected {
        return Err(unsupported(
            "binding",
            "requires exact draw and land allowance controller bindings",
        ));
    }
    Ok(())
}
