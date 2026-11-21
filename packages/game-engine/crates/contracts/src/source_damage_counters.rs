//! Source anaphora on a combat-damage trigger. CR 201.5, 400.7, 603.3a, 608.2h/k.
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;
use serde_json::{Value, json};

const PREFIX: &str = "Whenever this creature deals combat damage to a player, put ";

pub fn candidate(line: &str) -> bool {
    amount(line).is_some()
}

fn amount(line: &str) -> Option<u32> {
    let rest = line.strip_prefix(PREFIX)?;
    if rest == "a +1/+1 counter on it." {
        return Some(1);
    }
    let n = equipped_trigger::count(rest.strip_suffix(" +1/+1 counters on it.")?)?;
    (n > 1).then_some(n)
}

fn condition() -> TriggerCondition {
    TriggerCondition::DamageDealt {
        source: DamageTriggerSource::Source {},
        kind: TriggerDamageKind::Combat,
        recipient: DamageTriggerRecipient::Player,
    }
}

pub fn printed(line: &str, ast: &Value, source_block: u32) -> Option<TriggeredAbility> {
    let n = amount(line)?;
    let expected = json!({"kind":"triggered",
        "trigger":{"trigger":"deals-damage","what":{"ref":"this","noun":"creature"},"combat":true,"to":"any"},
        "effects":[{"steps":[{"effect":"put-counters","on":{"ref":"it"},
            "counter":{"type":"+1/+1"},"count":{"amount":"fixed","value":n}}]}]
    });
    (*ast == expected).then_some(TriggeredAbility {
        condition: condition(),
        functioning_zone: FunctioningZone::Battlefield,
        intervening_if: None,
        death_filter: None,
        program: SpellProgram {
            functioning_zone: FunctioningZone::Stack,
            timing: SpellTiming::Instant,
            resolution: SpellResolution::Instructions,
            optional_additional_costs: vec![],
            additional_sacrifice: None,
            targets: vec![],
            modes: vec![],
            instructions: vec![Instruction::PutPlusOneCounters {
                on: CounterRecipient::Source,
                amount: i64::from(n).into(),
                source_block,
            }],
        },
    })
}

pub(super) fn validate(d: &Definition) -> Result<(), String> {
    let actual = d
        .triggered
        .iter()
        .filter(|a| {
            a.condition == condition()
                && a.program.instructions.iter().any(|i| {
                    matches!(
                        i,
                        Instruction::PutPlusOneCounters {
                            on: CounterRecipient::Source,
                            ..
                        }
                    )
                })
        })
        .count();
    let raw = &d.provenance.source_json;
    if actual == 0 && !raw.contains(PREFIX) && !raw.contains(r"\u") {
        return Ok(());
    }
    let source: Value = strict_read(raw)?;
    let text = source["oracle_text"].as_str().unwrap_or_default();
    let mut expected = text
        .lines()
        .enumerate()
        .filter(|(_, line)| candidate(line))
        .peekable();
    if actual == 0 && expected.peek().is_none() {
        return Ok(());
    }
    let fail = "requires the printed combat damage and exact source counter recipient";
    let ast: Vec<Value> = strict_read(&d.provenance.abilities_json)?;
    if d.characteristics.kind != CardKind::Creature
        || ast.len() != text.lines().count()
        || source["name"] != d.name
        || source["oracle_id"] != d.oracle_id
        || source["id"] != d.provenance.printing_id
        || source["layout"] != "normal"
    {
        return Err(fail.into());
    }
    // This admission envelope composes with these previously supported
    // intrinsic keywords. Reject other printed siblings until independently bound.
    let mut keywords = Vec::new();
    for (line, ability) in text.lines().zip(&ast) {
        if candidate(line) {
            continue;
        }
        let base = basic_landwalk::without_reminder(line).ok_or(fail)?;
        let (keyword, parser_token) = match base.trim() {
            "Flying" => (KeywordAbility::Flying, "flying"),
            "Haste" => (KeywordAbility::Haste, "haste"),
            "Double strike" => (KeywordAbility::DoubleStrike, "double-strike"),
            "Trample" => (KeywordAbility::Trample, "trample"),
            _ => return Err(fail.into()),
        };
        if *ability != json!({"kind":"keywords","keywords":[{"keyword":parser_token}]}) {
            return Err(fail.into());
        }
        keywords.push(keyword);
    }
    if keywords != d.keywords
        || !d.activated.is_empty()
        || !d.static_abilities.is_empty()
        || !d.mana_abilities.is_empty()
        || d.triggered.len() != actual
    {
        return Err(fail.into());
    }
    let mut count = 0;
    for (block, line) in expected {
        let source_block = u32::try_from(block).map_err(|_| fail)?;
        let ordinal = ast[..block]
            .iter()
            .filter(|a| a["kind"] == "triggered")
            .count();
        if !printed(line, &ast[block], source_block)
            .is_some_and(|a| d.triggered.get(ordinal) == Some(&a))
            || d.source_labels
                .as_ref()
                .is_some_and(|l| l.triggered.get(ordinal) != Some(&source_block))
        {
            return Err(fail.into());
        }
        count += 1;
    }
    if actual != count {
        return Err(fail.into());
    }
    Ok(())
}

pub(super) fn extend_schema(s: &mut Value) {
    let counters = json!({"required":["op","on"],"properties":{
        "op":{"const":"put_plus_one_counters"},"on":{"const":CounterRecipient::Source},
    }});
    let trigger = json!({"required":["condition","program"],"properties":{
        "condition":{"const":condition()}, "functioning_zone":{"const":"battlefield"},
        "death_filter":{"type":"null"}, "intervening_if":{"type":"null"},
        "program":{"required":["instructions","targets"],"properties":{
            "targets":{"maxItems":0}, "modes":{"maxItems":0},
            "timing":{"const":"instant"},"functioning_zone":{"const":"stack"},
            "resolution":{"const":"instructions"},"additional_sacrifice":{"type":"null"},
            "optional_additional_costs":{"maxItems":0},
            "instructions":{"minItems":1,"maxItems":1,"items":counters},
        }}
    }});
    let printed = json!({"required":["provenance"],"properties":{"provenance":{"required":["source_json"],
        "properties":{"source_json":{"pattern":format!("{PREFIX}[^.]+ [+]1/[+]1 counter[s]? on it[.]")}}}}});
    s["$defs"]["Definition"]["allOf"].as_array_mut().unwrap().push(json!({
        "if":printed,
        "then":{"required":["triggered"],"properties":{"triggered":{"minItems":1,"items":trigger},
            "activated":{"maxItems":0},"static_abilities":{"maxItems":0},"mana_abilities":{"maxItems":0}}},
    }));
}
