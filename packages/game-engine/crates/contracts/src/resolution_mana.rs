//! Spell resolution produces unrestricted mana in the normal event batch.
//! CR 106.4, 601.2h, 605.5b, 608.2c and 608.2h.
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;
use serde_json::{Value, json};

const COST: &str = "As an additional cost to cast this spell, sacrifice a creature.";
const TAPPED: &str = " for each tapped land your opponents control";

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum ManaResolutionQuantity {
    Fixed {
        #[schemars(with = "String", regex(pattern = r"^[1-9][0-9]*$"))]
        amount: Quantity,
    },
    PerMatchingPermanent {
        #[schemars(with = "String", regex(pattern = r"^[1-9][0-9]*$"))]
        amount: Quantity,
        filter: PermanentFilter,
    },
}

pub fn candidate(text: &str) -> bool {
    text.lines().any(|line| line.starts_with("Add {"))
}

fn spec(text: &str, kind: CardKind) -> Option<(Value, SpellProgram)> {
    let timing = match kind {
        CardKind::Instant => SpellTiming::Instant,
        CardKind::Sorcery => SpellTiming::MainPhase,
        _ => return None,
    };
    let mut lines = text.lines();
    let mut line = lines.next()?;
    let mut expected = Vec::new();
    let sacrifice = line == COST;
    if sacrifice {
        expected.push(
            json!({"kind":"additional-cost","costs":[{"cost":"sacrifice","what":{
                "ref":"filter","filter":{"types":["creature"]},"count":{"amount":"fixed","value":1}
            }}]}),
        );
        line = lines.next()?;
    }
    if lines.next().is_some() {
        return None;
    }
    let symbols = line.strip_prefix("Add ")?.strip_suffix('.')?;
    let (mut symbols, per_match) = symbols
        .strip_suffix(TAPPED)
        .map_or((symbols, false), |s| (s, true));
    let mut colors = Vec::new();
    while !symbols.is_empty() {
        let (symbol, tail) = symbols.strip_prefix('{')?.split_once('}')?;
        let color = match symbol {
            "W" => Color::W,
            "U" => Color::U,
            "B" => Color::B,
            "R" => Color::R,
            "G" => Color::G,
            "C" => Color::C,
            _ => return None,
        };
        colors.push(color);
        symbols = tail;
    }
    let color = *colors.first()?;
    if colors.iter().any(|c| *c != color) {
        return None;
    }
    let amount = Quantity::from(i64::try_from(colors.len()).ok()?);
    let mut step = json!({"effect":"add-mana","who":{"player":"you"},"mana":{"mana":"fixed","symbols":colors}});
    let quantity = if per_match {
        step["forEach"] = json!({"status":["tapped"],"types":["land"],"control":"opponent"});
        ManaResolutionQuantity::PerMatchingPermanent {
            amount,
            filter: PermanentFilter {
                types: vec![CardKind::Land],
                non_types: vec![],
                non_colors: vec![],
                opponent: true,
                tapped: true,
                power: None,
            },
        }
    } else {
        ManaResolutionQuantity::Fixed { amount }
    };
    expected.push(json!({"kind":"spell","effects":[{"steps":[step]}]}));
    Some((
        json!(expected),
        SpellProgram {
            timing,
            functioning_zone: FunctioningZone::Stack,
            resolution: SpellResolution::Instructions,
            optional_additional_costs: vec![],
            additional_sacrifice: sacrifice.then(|| SacrificeFilter {
                types: vec![CardKind::Creature],
                subtypes: vec![],
            }),
            targets: vec![],
            instructions: vec![Instruction::AddResolutionMana {
                color,
                quantity,
                source_block: u32::from(sacrifice),
            }],
            modes: vec![],
        },
    ))
}

pub fn printed(text: &str, ast: &Value, kind: CardKind) -> Option<SpellProgram> {
    let (expected, program) = spec(text, kind)?;
    (*ast == expected).then_some(program)
}

fn uses(p: &SpellProgram) -> bool {
    p.instructions
        .iter()
        .any(|i| matches!(i, Instruction::AddResolutionMana { .. }))
        || p.modes.iter().any(|m| uses(&m.program))
}

pub(super) fn validate(d: &Definition) -> Result<(), String> {
    let error = "mana production requires its complete exact ordinary spell and cost";
    if d.activated.iter().any(|a| uses(&a.program)) || d.triggered.iter().any(|a| uses(&a.program) && !matches!(
            a.condition, TriggerCondition::SagaChapter { .. }
        ))
        || d.static_abilities.iter().any(|a| matches!(&a.effect,StaticEffect::Bestow{program,..}|StaticEffect::GrantSpellCascade{program,..} if uses(program))) {
        return Err(error.into());
    }
    let use_mana = d.spell.as_ref().is_some_and(uses);
    let raw = &d.provenance.source_json;
    if !use_mana && !raw.contains("Add {") && !raw.contains(r"\u") {
        return Ok(());
    }
    let row: Value = strict_read(raw)?;
    let Some((ast, expected)) = spec(
        row["oracle_text"].as_str().unwrap_or_default(),
        d.characteristics.kind,
    ) else {
        return if use_mana { Err(error.into()) } else { Ok(()) };
    };
    if d.spell.as_ref() != Some(&expected)
        || strict_read::<Value>(&d.provenance.abilities_json)? != ast
        || row["colors"] != json!(d.characteristics.colors)
        || row["color_identity"] != json!(d.characteristics.color_identity)
        || row["id"] != d.provenance.printing_id
        || row["oracle_id"] != d.oracle_id
        || row["name"] != d.name
        || row["layout"] != "normal"
        || row["type_line"]
            != if d.characteristics.kind == CardKind::Instant {
                "Instant"
            } else {
                "Sorcery"
            }
        || row["mana_cost"] != d.characteristics.mana_cost.printed_fixed_cost()
        || !d.keywords.is_empty()
        || !d.activated.is_empty()
        || !d.triggered.is_empty()
        || !d.mana_abilities.is_empty()
        || !d.static_abilities.is_empty()
        || d.x_mana_cost
        || d.intrinsic_zone.is_some()
        || !d.characteristics.hybrid_mana.is_empty()
    {
        return Err(error.into());
    }
    Ok(())
}

pub(super) fn extend_schema(s: &mut Value) {
    let op = json!({"required":["op"],"properties":{"op":{"const":"add_resolution_mana"}}});
    let free = json!({"$ref":"#/$defs/ResolutionManaFreeProgram"});
    s["$defs"]["ResolutionManaFreeProgram"] = json!({"properties":{"instructions":{"items":{"not":op}},
        "modes":{"items":{"properties":{"program":free}}}}});
    // The Saga gate separately ties the fixed program and chapter condition to
    // the complete printed chapter family. Other trigger contexts remain fenced.
    let chapter = json!({"$ref":"#/$defs/SagaFixedManaChapter"});
    let cap = json!({"contains":{"const":Capability::ResolutionManaV1}});
    s["$defs"]["Definition"]["allOf"].as_array_mut().unwrap().push(json!({
        "properties":{
            "activated":{"items":{"properties":{"program":free}}},
            "triggered":{"items":{"anyOf":[{"properties":{"program":free}},chapter]}},
            "static_abilities":{"items":{"properties":{"effect":{"properties":{
                "bestow":{"properties":{"program":free}}},"grant_spell_cascade":{"properties":{"program":free}}
            }}}}},
        "if":{"required":["spell"],"properties":{"spell":{"type":"object","not":free}}},
        "then":{"properties":{
            "characteristics":{"properties":{"kind":{"enum":["instant","sorcery"]}}},
            "capabilities":cap,"spell":{"properties":{
                "instructions":{"minItems":1,"maxItems":1,"items":op},
                "targets":{"maxItems":0},"modes":{"maxItems":0},"optional_additional_costs":{"maxItems":0}
            }}}},
        "else":{
            "if":{"required":["triggered"],"properties":{"triggered":{"contains":chapter}}},
            "then":{"properties":{"capabilities":cap}},
            "else":{"properties":{"capabilities":{"not":cap}}}
        }
    }));
}

#[cfg(test)]
pub(super) fn witness() -> Definition {
    let row: Value = strict_read(include_str!(
        "../../../tests/fixtures/53f7c868-b03e-4fc2-8dcf-a75bbfa3272b.json"
    ))
    .unwrap();
    let source = &row["source"];
    let ast = &row["faces"][0]["result"]["abilities"];
    let mut d = Catalog::read(include_str!(
        "../../../tests/fixtures/expected-effects-ir.json"
    ))
    .unwrap()
    .definitions
    .into_iter()
    .find(|d| d.characteristics.kind == CardKind::Instant)
    .unwrap();
    d.oracle_id = source["oracle_id"].as_str().unwrap().into();
    d.name = source["name"].as_str().unwrap().into();
    d.provenance.printing_id = source["id"].as_str().unwrap().into();
    d.provenance.source_index = row["sourceIndex"].as_u64().unwrap().try_into().unwrap();
    d.provenance.source_json = canonical(source).unwrap();
    d.provenance.source_sha256 = sha256(d.provenance.source_json.as_bytes());
    d.provenance.abilities_json = canonical(ast).unwrap();
    d.provenance.ast_sha256 = sha256(format!("[{}]", d.provenance.abilities_json).as_bytes());
    d.provenance.binding_sha256 = row["bindingSha256"].as_str().unwrap().into();
    d.source_labels = None;
    d.characteristics.colors = vec![Color::B];
    d.characteristics.color_identity = vec![Color::B];
    d.characteristics.mana_cost = ManaCost {
        generic: Quantity::zero(),
        colored: vec![Color::B],
    };
    d.spell = printed(
        source["oracle_text"].as_str().unwrap(),
        ast,
        CardKind::Instant,
    );
    d.capabilities = d.requirements();
    d
}
