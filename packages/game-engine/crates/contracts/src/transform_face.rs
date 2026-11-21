//! The fixed self-transform activation and both complete creature faces.
//! CR 701.27a/f, 712.8d/e, 712.9 and 202.3b.
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;
use crate::bulk_catalog::{is_oracle_id, is_sha256};
use serde_json::{Value, json};
use std::sync::OnceLock;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct CreatureFace {
    name: String,
    subtypes: Vec<String>,
    power: Quantity,
    toughness: Quantity,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct TransformFaces {
    front_name: String,
    back: CreatureFace,
    #[serde(skip)]
    #[schemars(skip)]
    view: OnceLock<Box<Definition>>,
}

impl Clone for TransformFaces {
    fn clone(&self) -> Self {
        Self {
            front_name: self.front_name.clone(),
            back: self.back.clone(),
            view: OnceLock::new(),
        }
    }
}
impl PartialEq for TransformFaces {
    fn eq(&self, other: &Self) -> bool {
        self.front_name == other.front_name && self.back == other.back
    }
}
impl Eq for TransformFaces {}

impl CreatureFace {
    fn read(source: &Value) -> Option<Self> {
        let name = source["name"].as_str()?.to_owned();
        let subtypes: Vec<String> = source["type_line"]
            .as_str()?
            .strip_prefix("Artifact Creature — ")?
            .split(' ')
            .map(str::to_owned)
            .collect();
        let quantity = |key| {
            let word = source[key].as_str()?;
            let number: u16 = word.parse().ok()?;
            (word == number.to_string()).then(|| Quantity::from(i64::from(number)))
        };
        let power = quantity("power")?;
        let toughness = quantity("toughness")?;
        if name.is_empty()
            || source["colors"] != json!([])
            || toughness == Quantity::zero()
            || subtypes.iter().any(|s| {
                s.is_empty()
                    || !s.bytes().all(|c| c.is_ascii_alphabetic())
                    || unsupported_rules_subtype(s)
            })
            || ["loyalty", "defense", "color_indicator"]
                .iter()
                .any(|k| source.get(*k).is_some())
        {
            return None;
        }
        Some(Self {
            name,
            subtypes,
            power,
            toughness,
        })
    }

    fn definition(&self, root: String, provenance: SourceProvenance) -> Definition {
        let mut result = modal_land::plain_face(self.name.clone(), root, vec![], provenance);
        result.characteristics.kind = CardKind::Creature;
        result.characteristics.additional_types = vec![CardKind::Artifact];
        result.characteristics.subtypes = self.subtypes.clone();
        result.characteristics.power = Some(self.power.clone());
        result.characteristics.toughness = Some(self.toughness.clone());
        result
    }
}

impl Definition {
    pub fn transform_back(&self) -> Option<&Definition> {
        let face = self.transform_faces.as_ref()?;
        Some(face.view.get_or_init(|| {
            let mut projected = face
                .back
                .definition(self.oracle_id.clone(), self.provenance.clone());
            projected.keywords = vec![KeywordAbility::Flying];
            projected.source_labels = Some(SourceLabels {
                text: "Flying".into(),
                blocks: vec!["Flying".into()],
                ..SourceLabels::default()
            });
            projected.capabilities = projected.requirements();
            Box::new(projected)
        }))
    }
}

impl TransformFaces {
    pub(super) fn front_name(&self) -> &str {
        &self.front_name
    }
}

pub fn from_archive(provenance: SourceProvenance) -> Result<Definition, String> {
    let fail = || "requires two complete supported self-transform creature faces".to_owned();
    if !accepted_provenance(
        &provenance.archive_manifest_sha256,
        &provenance.parser_sha256,
    ) || sha256(provenance.source_json.as_bytes()) != provenance.source_sha256
        || sha256(format!("[{}]", provenance.abilities_json).as_bytes()) != provenance.ast_sha256
        || !is_sha256(&provenance.binding_sha256)
    {
        return Err(fail());
    }
    let source: Value = strict_read(&provenance.source_json)?;
    let ast: Value = strict_read(&format!("[{}]", provenance.abilities_json))?;
    let [a, b] = source["card_faces"].as_array().ok_or_else(fail)?.as_slice() else {
        return Err(fail());
    };
    let front = CreatureFace::read(a).ok_or_else(fail)?;
    let back = CreatureFace::read(b).ok_or_else(fail)?;
    let oracle_id = source["oracle_id"]
        .as_str()
        .filter(|id| is_oracle_id(id))
        .ok_or_else(fail)?;
    let name = format!("{} // {}", front.name, back.name);
    let card_cost = generic_cost(a["mana_cost"].as_str().ok_or_else(fail)?).ok_or_else(fail)?;
    let text = a["oracle_text"].as_str().ok_or_else(fail)?;
    let activation_text = text.strip_prefix("Defender\n").ok_or_else(fail)?;
    let payment = activation_text
        .strip_suffix(": Transform this creature.")
        .ok_or_else(fail)?;
    let activate = generic_cost(payment).filter(|n| *n != 0).ok_or_else(fail)?;
    let expected = json!([
        [{"kind":"keywords","keywords":[{"keyword":"defender"}]},
         {"kind":"activated","costs":[{"cost":"mana","symbols":[activate.to_string()]}],
          "effects":[{"steps":[{"effect":"transform","what":{"ref":"this","noun":"creature"},
                               "who":{"player":"you"}}]}]}],
        [{"kind":"keywords","keywords":[{"keyword":"flying"}]}]
    ]);
    let type_line = format!(
        "{} // {}",
        a["type_line"].as_str().unwrap(),
        b["type_line"].as_str().unwrap()
    );
    let keywords = source["keywords"].as_array().ok_or_else(fail)?;
    if source["layout"] != "transform"
        || source["name"] != name
        || source["type_line"] != type_line
        || source["id"] != provenance.printing_id
        || !is_oracle_id(&provenance.printing_id)
        || source["color_identity"] != json!([])
        || b["mana_cost"] != ""
        || b["oracle_text"] != "Flying"
        || front.name == back.name
        || keywords.len() != 3
        || !["Transform", "Flying", "Defender"]
            .iter()
            .all(|k| keywords.contains(&json!(k)))
        || ast != expected
    {
        return Err(fail());
    }
    let mut definition = front.definition(oracle_id.into(), provenance);
    definition.name = name;
    definition.characteristics.mana_cost.generic = i64::from(card_cost).into();
    definition.spell = Some(program(
        SpellTiming::MainPhase,
        SpellResolution::Permanent,
        vec![],
    ));
    definition.keywords.push(KeywordAbility::Defender);
    definition.activated = vec![ActivatedAbility {
        functioning_zone: FunctioningZone::Battlefield,
        cost: ActivationCost::Mana {
            cost: ManaCost {
                generic: i64::from(activate).into(),
                colored: vec![],
            },
        },
        program: program(
            SpellTiming::Instant,
            SpellResolution::Instructions,
            vec![Instruction::TransformSource { source_block: 1 }],
        ),
    }];
    definition.source_labels = Some(SourceLabels {
        text: text.into(),
        blocks: text.lines().map(str::to_owned).collect(),
        activated: vec![1],
        ..SourceLabels::default()
    });
    definition.transform_faces = Some(Box::new(TransformFaces {
        front_name: front.name,
        back,
        view: OnceLock::new(),
    }));
    definition.capabilities = definition.requirements();
    Ok(definition)
}

fn generic_cost(text: &str) -> Option<u16> {
    let number = text.strip_prefix('{')?.strip_suffix('}')?;
    let count = number.parse::<u16>().ok()?;
    (number == count.to_string()).then_some(count)
}

fn program(
    timing: SpellTiming,
    resolution: SpellResolution,
    instructions: Vec<Instruction>,
) -> SpellProgram {
    SpellProgram {
        timing,
        resolution,
        functioning_zone: FunctioningZone::Stack,
        optional_additional_costs: vec![],
        additional_sacrifice: None,
        targets: vec![],
        modes: vec![],
        instructions,
    }
}

pub(super) fn validate(d: &Definition) -> Result<bool, String> {
    let typed = d.transform_faces.is_some() || d.capabilities.contains(&Capability::TransformV1);
    let raw = &d.provenance.source_json;
    if !typed && !raw.contains("transform") && !raw.contains(r"\u") {
        return Ok(false);
    }
    if !typed && strict_read::<Value>(raw)?["layout"] != "transform" {
        return Ok(false);
    }
    let expected = from_archive(d.provenance.clone())?;
    if *d != expected || d.transform_back() != expected.transform_back() {
        return Err("transform face payload differs from complete retained source".into());
    }
    Ok(true)
}

pub(super) fn extend_schema(schema: &mut Value) {
    let capability = json!({"contains":{"const":Capability::TransformV1}});
    let source = json!({"properties":{"provenance":{"properties":{
        "source_json":{"pattern":r#""layout"\s*:\s*"transform""#}
    }}}});
    let op = json!({"required":["op"],"properties":{"op":{"const":"transform_source"}}});
    let free = json!({"$ref":"#/$defs/TransformFreeProgram"});
    schema["$defs"]["TransformFreeProgram"] = json!({"properties":{
        "instructions":{"items":{"not":op}},
        "modes":{"items":{"properties":{"program":free}}}
    }});
    schema["$defs"]["Definition"]["allOf"].as_array_mut().unwrap().push(json!({
        "properties":{
            "spell":free,"triggered":{"items":{"properties":{"program":free}}},
            "static_abilities":{"items":{"properties":{"effect":{"properties":{
                "bestow":{"properties":{"program":free}},"grant_spell_cascade":{"properties":{"program":free}}}
            }}}}
        },
        "if":{"anyOf":[{"required":["transform_faces"]},{"properties":{"capabilities":capability}},source]},
        "then":{"required":["transform_faces","activated"],"allOf":[source],"properties":{
            "modal_land":false,
            "characteristics":{"properties":{"kind":{"const":"creature"},"additional_types":{"const":["artifact"]}}},
            "capabilities":capability,"activated":{"minItems":1,"maxItems":1,"items":{"properties":{
                "cost":{"required":["mana"]},"program":{"properties":{
                    "instructions":{"minItems":1,"maxItems":1,"items":{"allOf":[op,
                        {"properties":{"source_block":{"const":1}}}]}},
                    "targets":{"maxItems":0},"modes":{"maxItems":0},"optional_additional_costs":{"maxItems":0}
                }}
            }}},
            "triggered":{"maxItems":0},"static_abilities":{"maxItems":0},"mana_abilities":{"maxItems":0}
        }},
        "else":{"properties":{"activated":{"items":{"properties":{"program":free}}}}}
    }));
}

#[cfg(test)]
pub(super) fn witness() -> Definition {
    from_archive(modal_land::witness_provenance(include_str!(
        "../../../tests/fixtures/phase1-r3/thraben-gargoyle.json"
    )))
    .unwrap()
}
