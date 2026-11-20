//! CR 201.5, 400.7, 602.2, 611.2c: a fixed paid self grant on one incarnation.
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;
use serde_json::{Value, json};

const INVALID: &str = "requires complete printed mana-only self keyword grants";
const SUFFIX: &str = " until end of turn.";
const SUBJECT: &str = "This creature gains ";

pub struct PrintedGrant {
    pub cost: ManaCost,
    pub keywords: Vec<KeywordAbility>,
    pub ast: Value,
}

pub fn printed_grant(line: &str) -> Option<PrintedGrant> {
    let (printed_cost, effect) = line.split_once(": ")?;
    let effect = basic_landwalk::without_reminder(effect)?;
    let effect = effect.trim_end();
    let words = effect.strip_prefix(SUBJECT)?.strip_suffix(SUFFIX)?;
    let words = words.replace(", and ", " and ").replace(", ", " and ");
    let keywords = words
        .split(" and ")
        .map(|word| serde_json::from_value(json!(word)).ok())
        .collect::<Option<Vec<KeywordAbility>>>()?;
    if !supported_temporary_keywords(&keywords) || printed_cost.is_empty() {
        return None;
    }
    let mut rest = printed_cost;
    let mut cost = ManaCost {
        generic: 0.into(),
        colored: vec![],
    };
    let mut symbols = Vec::new();
    while !rest.is_empty() {
        let (symbol, tail) = rest.strip_prefix('{')?.split_once('}')?;
        match symbol {
            "W" => cost.colored.push(Color::W),
            "U" => cost.colored.push(Color::U),
            "B" => cost.colored.push(Color::B),
            "R" => cost.colored.push(Color::R),
            "G" => cost.colored.push(Color::G),
            "C" => cost.colored.push(Color::C),
            s if !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit()) => {
                cost.generic.add(&s.parse().ok()?);
            }
            _ => return None,
        }
        symbols.push(symbol);
        rest = tail;
    }
    let abilities: Vec<_> = keywords
        .iter()
        .map(|keyword| json!({"gained":"keyword","keyword":{"keyword":keyword}}))
        .collect();
    let ast = json!({
        "kind":"activated","costs":[{"cost":"mana","symbols":symbols}],
        "effects":[{"steps":[{"effect":"gain-abilities","what":{"ref":"this","noun":"creature"},
            "abilities":abilities,"duration":{"duration":"end-of-turn"}}]}]
    });
    Some(PrintedGrant {
        cost,
        keywords,
        ast,
    })
}

impl PrintedGrant {
    pub fn activation(self, source_block: u32) -> ActivatedAbility {
        ActivatedAbility {
            functioning_zone: FunctioningZone::Battlefield,
            cost: ActivationCost::Mana { cost: self.cost },
            program: SpellProgram {
                timing: SpellTiming::Instant,
                resolution: SpellResolution::Instructions,
                functioning_zone: FunctioningZone::Stack,
                targets: vec![],
                instructions: vec![Instruction::GrantSourceKeywords {
                    keywords: self.keywords,
                    source_block,
                }],
                modes: vec![],
                optional_additional_costs: vec![],
                additional_sacrifice: None,
            },
        }
    }
}

pub(super) fn has_grant(p: &SpellProgram) -> bool {
    p.instructions
        .iter()
        .any(|i| matches!(i, Instruction::GrantSourceKeywords { .. }))
        || p.modes.iter().any(|m| has_grant(&m.program))
}

pub fn keywords(a: &ActivatedAbility) -> Option<&[KeywordAbility]> {
    let p = &a.program;
    if a.functioning_zone != FunctioningZone::Battlefield
        || !matches!(&a.cost, ActivationCost::Mana { cost } if !cost.generic.is_negative())
        || p.functioning_zone != FunctioningZone::Stack
        || p.timing != SpellTiming::Instant
        || p.resolution != SpellResolution::Instructions
        || !p.targets.is_empty()
        || !p.modes.is_empty()
        || p.additional_sacrifice.is_some()
        || !p.optional_additional_costs.is_empty()
    {
        return None;
    }
    match p.instructions.as_slice() {
        [Instruction::GrantSourceKeywords { keywords, .. }]
            if supported_temporary_keywords(keywords) =>
        {
            Some(keywords)
        }
        _ => None,
    }
}

pub(super) fn validate(d: &Definition) -> Result<(), String> {
    if d.spell.as_ref().is_some_and(has_grant) || d.triggered.iter().any(|a| has_grant(&a.program))
    {
        return Err(INVALID.into());
    }
    let grants: Vec<_> = d
        .activated
        .iter()
        .filter(|a| has_grant(&a.program))
        .collect();
    let raw = &d.provenance.source_json;
    let raw_ast = &d.provenance.abilities_json;
    if grants.is_empty() && !raw.contains(SUBJECT) && !raw.contains(r"\u") {
        return Ok(());
    }
    let source: Value = strict_read(raw)?;
    let text = source["oracle_text"].as_str().unwrap_or_default();
    let mut prints = text.lines().filter_map(printed_grant).peekable();
    if grants.is_empty() && prints.peek().is_none() {
        return Ok(());
    }
    if d.characteristics.kind != CardKind::Creature
        || source["layout"] != "normal"
        || source["name"] != d.name
        || source["oracle_id"] != d.oracle_id
        || source["id"] != d.provenance.printing_id
    {
        return Err(INVALID.into());
    }
    let raw_blocks: Vec<Value> = strict_read(raw_ast)?;
    let mut expected = Vec::new();
    for (index, node) in raw_blocks.iter().enumerate() {
        if prints.peek().is_some_and(|spec| &spec.ast == node) {
            let spec = prints.next().unwrap();
            let block = u32::try_from(index).map_err(|_| INVALID)?;
            if d.source_labels.as_ref().is_some_and(|labels| {
                !labels.activated.contains(&block)
                    || labels
                        .blocks
                        .get(index)
                        .and_then(|line| printed_grant(line))
                        .is_none_or(|line| line.ast != spec.ast)
            }) {
                return Err(INVALID.into());
            }
            expected.push(spec.activation(block));
        }
    }
    if prints.next().is_some() || !expected.iter().eq(grants) {
        return Err(INVALID.into());
    }
    Ok(())
}

pub(super) fn schema_presence(keyword: Option<KeywordAbility>) -> Value {
    let mut fields = json!({"op":{"const":"grant_source_keywords"}});
    if let Some(keyword) = keyword {
        fields["keywords"] = json!({"contains":{"const":keyword}});
    }
    json!({"required":["activated"],"properties":{"activated":{"contains":{"properties":{"program":{"properties":{"instructions":{"contains":{"properties":fields}}}}}}}}})
}

pub(super) fn extend_schema(schema: &mut Value) {
    let vocabulary: Vec<_> = schema["$defs"]["KeywordAbility"]["enum"]
        .as_array()
        .unwrap()
        .iter()
        .cloned()
        .filter_map(|word| serde_json::from_value::<KeywordAbility>(word).ok())
        .filter(|&keyword| supported_temporary_keywords(&[keyword]))
        .collect();
    let free = json!({"$ref":"#/$defs/SourceKeywordGrantFreeProgram"});
    let opcode = json!({"required":["op"],"properties":{"op":{"const":"grant_source_keywords"}}});
    let no_grant = json!({"items":{"not":opcode}});
    schema["$defs"]["SourceKeywordGrantFreeProgram"] = json!({"properties":{
        "instructions":no_grant,
        "modes":{"items":{"properties":{"program":free}}}
    }});
    let instruction = json!({"allOf":[opcode,{"properties":{
        "keywords":{"type":"array","minItems":1,"uniqueItems":true,"items":{"enum":vocabulary}}
    }}]});
    let shape = json!({"properties":{
        "functioning_zone":{"const":"battlefield"},
        "cost":{"type":"object","required":["mana"],"properties":{"mana":{"properties":{"cost":{"properties":{"generic":{"pattern":"^(0|[1-9][0-9]*)$"}}}}}}},
        "program":{"properties":{
            "functioning_zone":{"const":"stack"},"timing":{"const":"instant"},
            "resolution":{"const":"instructions"},"targets":{"maxItems":0},
            "modes":{"maxItems":0},"optional_additional_costs":{"maxItems":0},
            "additional_sacrifice":{"type":"null"},
            "instructions":{"minItems":1,"maxItems":1,"items":instruction}
        }}
    }});
    let cap = json!({"contains":{"const":Capability::ActivatedSourceKeywordGrantV1}});
    let closure = schema["$defs"]["Definition"]["allOf"]
        .as_array_mut()
        .unwrap();
    closure.extend([
        json!({"properties":{
            "spell":{"anyOf":[{"type":"null"},free]},
            "triggered":{"items":{"properties":{"program":free}}},
            "activated":{"items":{"anyOf":[{"properties":{"program":free}},shape]}}
        }}),
        json!({"if":schema_presence(None),"then":{"properties":{
            "characteristics":{"properties":{"kind":{"const":"creature"}}},
            "capabilities":cap
        }},"else":{"properties":{"capabilities":{"not":cap}}}}),
    ]);
    for keyword in vocabulary {
        let cap = temporary_keyword_primitive_capability(keyword).unwrap();
        closure.push(json!({"if":schema_presence(Some(keyword)),
            "then":{"properties":{"capabilities":{"contains":{"const":cap}}}}}));
    }
}
