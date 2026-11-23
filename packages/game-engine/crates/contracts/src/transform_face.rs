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
