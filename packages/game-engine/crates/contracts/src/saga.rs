//! CR 714.2b-e. Each printed chapter symbol is a separate ability.
use super::*;
use serde_json::{Value, json};
use std::num::NonZeroU32;

pub struct LifeChapter {
    pub numbers: Vec<NonZeroU32>,
    pub amount: u32,
    pub source_block: u32,
}
impl LifeChapter {
    pub fn abilities(&self) -> impl Iterator<Item = TriggeredAbility> + '_ {
        self.numbers.iter().map(|&number| TriggeredAbility {
            condition: TriggerCondition::SagaChapter { number },
            functioning_zone: FunctioningZone::Battlefield,
            intervening_if: None,
            death_filter: None,
            program: SpellProgram {
                additional_sacrifice: None,
                optional_additional_costs: vec![],
                timing: SpellTiming::Instant,
                resolution: SpellResolution::Instructions,
                functioning_zone: FunctioningZone::Stack,
                instructions: vec![Instruction::GainLife {
                    recipient: LifeRecipient::Controller,
                    amount: Quantity::from(i64::from(self.amount)),
                    source_block: self.source_block,
                }],
                targets: vec![],
                modes: vec![],
            },
        })
    }
    pub fn ast(&self) -> Value {
        json!({"kind":"chapter","chapters":self.numbers,"effects":[{"steps":[{"effect":"gain-life",
            "who":{"player":"you"},"amount":{"amount":"fixed","value":self.amount}}]}]})
    }
}
pub fn life_chapters(text: &str) -> Result<Vec<LifeChapter>, String> {
    let mut chapters = vec![];
    let mut next = 1;
    for (block, line) in text.split('\n').enumerate() {
        let (symbols, effect) = line.split_once(" — ").ok_or("invalid chapter line")?;
        let digits = effect
            .strip_prefix("You gain ")
            .and_then(|s| s.strip_suffix(" life."))
            .ok_or("unsupported chapter effect")?;
        let amount = digits
            .parse::<u32>()
            .ok()
            .filter(|n| *n > 0 && n.to_string() == digits)
            .ok_or("invalid chapter life amount")?;
        let mut numbers = vec![];
        for symbol in symbols.split(", ") {
            let n = match symbol {
                "I" => 1,
                "II" => 2,
                "III" => 3,
                _ => return Err("unsupported chapter symbol".into()),
            };
            if n != next {
                return Err("expected consecutive printed chapter symbols".into());
            }
            next += 1;
            numbers.push(NonZeroU32::new(n).unwrap());
        }
        chapters.push(LifeChapter {
            numbers,
            amount,
            source_block: u32::try_from(block).map_err(|_| "chapter block overflow")?,
        });
    }
    Ok(chapters)
}
pub(super) fn validate(d: &Definition) -> Result<(), String> {
    let raw = &d.provenance.source_json;
    let typed = d.characteristics.subtypes.iter().any(|s| s == "Saga")
        || d.capabilities.contains(&Capability::SagaChaptersV1)
        || d.triggered
            .iter()
            .any(|t| matches!(t.condition, TriggerCondition::SagaChapter { .. }));
    if !typed && !raw.contains("saga") && !raw.contains(r"\u") {
        return Ok(());
    }
    let src: Value = strict_read(raw)?;
    if !typed && src["layout"] != "saga" {
        return Ok(());
    }
    let text = src["oracle_text"]
        .as_str()
        .ok_or("Saga source missing text")?;
    let chapters = life_chapters(text)?;
    let c = &d.characteristics;
    let mut mana = String::new();
    if c.mana_cost.generic != Quantity::zero() || c.mana_cost.colored.is_empty() {
        mana.push_str(&format!("{{{}}}", c.mana_cost.generic));
    }
    for color in &c.mana_cost.colored {
        mana.push_str(&format!("{{{color:?}}}"));
    }
    if src["layout"] != "saga"
        || src["type_line"] != "Enchantment — Saga"
        || src["id"] != d.provenance.printing_id
        || src["oracle_id"] != d.oracle_id
        || src["name"] != d.name
        || src["mana_cost"] != mana
        || src["colors"] != json!(c.colors)
        || src["color_identity"] != json!(c.color_identity)
        || src["keywords"] != json!([])
        || ["card_faces", "power", "toughness", "loyalty", "defense"]
            .iter()
            .any(|key| src.get(*key).is_some())
        || c.kind != CardKind::Enchantment
        || c.subtypes != ["Saga"]
        || !c.additional_types.is_empty()
        || c.legendary
        || c.snow
        || c.basic
        || d.x_mana_cost
        || !d.keywords.is_empty()
        || !d.activated.is_empty()
        || !d.static_abilities.is_empty()
        || !d.mana_abilities.is_empty()
        || !d.capabilities.contains(&Capability::SagaChaptersV1)
        || d.spell.as_ref().is_none_or(|p| {
            p.resolution != SpellResolution::Permanent
                || p.timing != SpellTiming::MainPhase
                || p.functioning_zone != FunctioningZone::Stack
                || !p.instructions.is_empty()
                || !p.targets.is_empty()
                || !p.modes.is_empty()
                || !p.optional_additional_costs.is_empty()
                || p.additional_sacrifice.is_some()
        })
        || !d
            .triggered
            .iter()
            .cloned()
            .eq(chapters.iter().flat_map(|c| c.abilities()))
        || strict_read::<Value>(&d.provenance.abilities_json)?
            != Value::Array(chapters.iter().map(|c| c.ast()).collect())
        || d.source_labels.as_ref().is_some_and(|l| {
            l.text != text
                || !l.blocks.iter().map(String::as_str).eq(text.lines())
                || !l.triggered.iter().copied().eq(chapters
                    .iter()
                    .flat_map(|c| c.numbers.iter().map(|_| c.source_block)))
                || !l.activated.is_empty()
                || !l.mana.is_empty()
                || !l.statics.is_empty()
                || !l.spell_modes.is_empty()
        })
    {
        return Err("invalid complete ordinary Saga source".into());
    }
    Ok(())
}

#[cfg(test)]
pub(super) fn witness() -> Definition {
    let record: Value = serde_json::from_str(include_str!(
        "../../../tests/fixtures/synthetic-saga-life.json"
    ))
    .unwrap();
    let source = &record["source"];
    let mut d = entry_graveyard_exile::witness(CardKind::Creature);
    d.name = source["name"].as_str().unwrap().into();
    d.oracle_id = source["oracle_id"].as_str().unwrap().into();
    d.provenance.printing_id = source["id"].as_str().unwrap().into();
    d.provenance.source_json = canonical(source).unwrap();
    d.provenance.source_sha256 = record["sourceSha256"].as_str().unwrap().into();
    d.provenance.abilities_json = canonical(&record["faces"][0]["result"]["abilities"]).unwrap();
    d.provenance.ast_sha256 = record["astSha256"].as_str().unwrap().into();
    d.provenance.binding_sha256 = record["bindingSha256"].as_str().unwrap().into();
    d.characteristics.kind = CardKind::Enchantment;
    d.characteristics.subtypes = vec!["Saga".into()];
    d.characteristics.power = None;
    d.characteristics.toughness = None;
    d.characteristics.colors.clear();
    d.characteristics.color_identity.clear();
    d.characteristics.mana_cost = ManaCost {
        generic: 1.into(),
        colored: vec![],
    };
    d.keywords.clear();
    let text = source["oracle_text"].as_str().unwrap();
    let chapters = life_chapters(text).unwrap();
    d.triggered = chapters.iter().flat_map(|c| c.abilities()).collect();
    d.source_labels = Some(SourceLabels {
        text: text.into(),
        blocks: text.lines().map(str::to_string).collect(),
        triggered: vec![0, 1, 1],
        ..Default::default()
    });
    d.capabilities = d.requirements();
    d
}

pub(super) fn extend_schema(schema: &mut Value) {
    use Capability::*;
    let chapter = json!({"required":["condition"],"properties":{"condition":{"type":"object","required":["saga_chapter"]}}});
    let allowed = [
        SagaChaptersV1,
        CastEnchantmentV1,
        TriggeredAbilityV1,
        LifeGainV1,
    ];
    schema["$defs"]["Definition"]["allOf"].as_array_mut().unwrap().push(json!({
        "if":{"anyOf":[{"properties":{"capabilities":{"contains":{"const":SagaChaptersV1}}}},
            {"required":["triggered"],"properties":{"triggered":{"contains":chapter}}},
            {"properties":{"characteristics":{"properties":{"subtypes":{"contains":{"const":"Saga"}}}}}},
            {"properties":{"provenance":{"properties":{"source_json":{"pattern":r#""layout"\s*:\s*"saga""#}}}}}]},
        "then":{"required":["triggered","spell"],"properties":{
            "capabilities":{"minItems":allowed.len(),"maxItems":allowed.len(),"uniqueItems":true,"items":{"enum":allowed}},
            "characteristics":{"properties":{"kind":{"const":"enchantment"},"subtypes":{"const":["Saga"]},"basic":{"const":false},
                "legendary":{"const":false},"snow":{"const":false},"additional_types":{"maxItems":0}}},
            "x_mana_cost":{"const":false},"activated":{"maxItems":0},"mana_abilities":{"maxItems":0},"static_abilities":{"maxItems":0},"keywords":{"maxItems":0},
            "spell":{"type":"object","properties":{"resolution":{"const":"permanent"},"timing":{"const":"main_phase"},"functioning_zone":{"const":"stack"},
                "instructions":{"maxItems":0},"targets":{"maxItems":0},"modes":{"maxItems":0},"additional_sacrifice":{"type":"null"},"optional_additional_costs":{"maxItems":0}}},
            "triggered":{"minItems":1,"maxItems":3,"items":{"allOf":[chapter],"properties":{"functioning_zone":{"const":"battlefield"},
                "intervening_if":{"type":"null"},"death_filter":{"type":"null"},"program":{"properties":{"functioning_zone":{"const":"stack"},
                "timing":{"const":"instant"},"resolution":{"const":"instructions"},"targets":{"maxItems":0},"modes":{"maxItems":0},
                "additional_sacrifice":{"type":"null"},"optional_additional_costs":{"maxItems":0},"instructions":{"minItems":1,"maxItems":1,
                "items":{"properties":{"op":{"const":"gain_life"},"recipient":{"const":"controller"},"amount":{"pattern":"^[1-9][0-9]*$"}}}}}}}}},
            "provenance":{"properties":{"source_json":{"pattern":r#""layout"\s*:\s*"saga""#}}}
        }}
    }));
}
