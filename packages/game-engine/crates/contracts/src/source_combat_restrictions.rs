//! Source-only blocker declaration restrictions, CR 509.1a–b and 509.1g.
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;
use serde_json::{Value, json};

const UNBLOCKABLE: &str = "This creature can't be blocked.";
const FLYERS_ONLY: &str = "This creature can block only creatures with flying.";
const INVALID: &str = "requires exact printed source combat restrictions";
const RESTRICTIONS: [CreatureRestriction; 2] = [
    CreatureRestriction::CantBeBlocked,
    CreatureRestriction::CanBlockOnlyFlying,
];

pub fn printed_restriction(line: &str) -> Option<CreatureRestriction> {
    match line {
        UNBLOCKABLE => Some(CreatureRestriction::CantBeBlocked),
        FLYERS_ONLY => Some(CreatureRestriction::CanBlockOnlyFlying),
        _ => None,
    }
}

pub fn source_operand() -> Value {
    json!({"ref":"this","noun":"creature"})
}

pub fn flying_attackers() -> Value {
    json!({"ref":"each","filter":{"types":["creature"],"withAbility":["flying"]}})
}

pub fn restriction_ast(restriction: CreatureRestriction) -> Option<Value> {
    let what = source_operand();
    let effect = match restriction {
        CreatureRestriction::CantBeBlocked => {
            json!({"static":"cant","what":what,"action":"be-blocked"})
        }
        CreatureRestriction::CanBlockOnlyFlying => json!({
            "static":"combat-rule","what":what,
            "rule":{"rule":"blocking-filter","mode":"only","attackers":flying_attackers()}
        }),
        _ => return None,
    };
    Some(json!({"kind":"static","effect":effect}))
}

pub fn static_ability(restriction: CreatureRestriction) -> StaticAbility {
    StaticAbility {
        functioning_zone: FunctioningZone::Battlefield,
        effect: StaticEffect::ModifyCreatures {
            scope: CreatureScope::Source,
            power: Quantity::zero(),
            toughness: Quantity::zero(),
            keywords: vec![],
            restrictions: vec![restriction],
        },
    }
}

pub(super) fn restriction(effect: &StaticEffect) -> Option<CreatureRestriction> {
    match effect {
        StaticEffect::ModifyCreatures {
            scope: CreatureScope::Source,
            restrictions,
            ..
        } => match restrictions.as_slice() {
            [restriction] if RESTRICTIONS.contains(restriction) => Some(*restriction),
            _ => None,
        },
        _ => None,
    }
}

pub(super) fn validate_definition(d: &Definition) -> Result<(), String> {
    let executable: Vec<_> = d
        .static_abilities
        .iter()
        .filter_map(|a| restriction(&a.effect))
        .collect();
    let raw = &d.provenance.source_json;
    let raw_ast = &d.provenance.abilities_json;
    if executable.is_empty()
        && !raw.contains(UNBLOCKABLE)
        && !raw.contains(FLYERS_ONLY)
        && !raw_ast.contains("blocking-filter")
        && !raw_ast.contains("\"be-blocked\"")
        && !raw_ast.contains(r"\u")
    {
        return Ok(());
    }
    let source: Value = strict_read(raw)?;
    let printed: Vec<_> = source["oracle_text"]
        .as_str()
        .unwrap_or_default()
        .lines()
        .filter_map(printed_restriction)
        .collect();
    let ast: Vec<Value> = strict_read(raw_ast)?;
    let blocks: Vec<_> = ast
        .iter()
        .enumerate()
        .filter_map(|(index, node)| {
            RESTRICTIONS
                .iter()
                .copied()
                .find(|&r| restriction_ast(r).as_ref() == Some(node))
                .map(|r| (index, r))
        })
        .collect();
    if executable.is_empty() && printed.is_empty() && blocks.is_empty() {
        return Ok(());
    }
    if d.characteristics.kind != CardKind::Creature
        || source["layout"] != "normal"
        || source["oracle_id"] != d.oracle_id
        || source["name"] != d.name
        || source["id"] != d.provenance.printing_id
        || executable != printed
        || !executable
            .iter()
            .copied()
            .eq(blocks.iter().map(|(_, r)| *r))
        || d.source_labels.as_ref().is_some_and(|labels| {
            blocks.iter().any(|&(i, r)| {
                labels.blocks.get(i).and_then(|b| printed_restriction(b)) != Some(r)
                    || !labels.statics.iter().any(|&block| block as usize == i)
            })
        })
    {
        return Err(INVALID.into());
    }
    Ok(())
}

pub(super) fn extend_schema(schema: &mut Value) {
    let effects = RESTRICTIONS.map(|r| static_ability(r).effect);
    let exact_effect = json!({"enum":effects});
    for kind in ["StaticEffect", "StaticAbility"] {
        schema["$defs"][kind]
            .as_object_mut()
            .unwrap()
            .entry("allOf")
            .or_insert_with(|| json!([]))
            .as_array_mut()
            .unwrap();
    }
    schema["$defs"]["StaticEffect"]["allOf"]
        .as_array_mut()
        .unwrap()
        .push(
            json!({"if":{"type":"object","required":["modify_creatures"],"properties":{
            "modify_creatures":{"required":["restrictions"],"properties":{
                "restrictions":{"contains":{"const":CreatureRestriction::CanBlockOnlyFlying}}
            }}
        }},"then":{"const":static_ability(CreatureRestriction::CanBlockOnlyFlying).effect}}),
        );
    schema["$defs"]["StaticAbility"]["allOf"]
        .as_array_mut()
        .unwrap()
        .push(
            json!({"if":{"required":["effect"],"properties":{"effect":exact_effect}},
            "then":{"properties":{"functioning_zone":{"const":"battlefield"}}}}),
        );
    let cap = json!({"contains":{"const":Capability::SourceCombatRestrictionsV1}});
    schema["$defs"]["Definition"]["allOf"]
        .as_array_mut()
        .unwrap()
        .push(json!({
            "if":{"required":["static_abilities"],"properties":{"static_abilities":{"contains":{
                "required":["effect"],"properties":{"effect":exact_effect}
            }}}},
            "then":{"properties":{"characteristics":{"properties":{"kind":{"const":"creature"}}},
                "capabilities":cap}},
            "else":{"properties":{"capabilities":{"not":cap}}}
        }));
}
