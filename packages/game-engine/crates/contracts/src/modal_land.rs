//! Complete modal land back faces and graveyard-return fronts, CR 712.8d/f, 712.12 and 712.14.
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;
use std::sync::OnceLock;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct LandFace {
    name: String,
    color: Color,
    tapped: bool,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ModalLand {
    front_name: String,
    back: LandFace,
    #[serde(skip)]
    #[schemars(skip)]
    projected: OnceLock<Box<Definition>>,
}

impl Clone for ModalLand {
    fn clone(&self) -> Self {
        Self {
            front_name: self.front_name.clone(),
            back: self.back.clone(),
            projected: OnceLock::new(),
        }
    }
}

impl PartialEq for ModalLand {
    fn eq(&self, other: &Self) -> bool {
        self.front_name == other.front_name && self.back == other.back
    }
}

impl LandFace {
    fn text(&self) -> String {
        let prefix = if self.tapped {
            "This land enters tapped.\n"
        } else {
            ""
        };
        format!("{prefix}{{T}}: Add {{{:?}}}.", self.color)
    }

    fn definition(
        &self,
        oracle_id: String,
        color_identity: Vec<Color>,
        provenance: SourceProvenance,
    ) -> Definition {
        let text = self.text();
        let mut definition = plain_face(self.name.clone(), oracle_id, color_identity, provenance);
        definition.source_labels = Some(SourceLabels {
            blocks: text.lines().map(str::to_owned).collect(),
            text,
            mana: vec![u32::from(self.tapped)],
            statics: self.tapped.then_some(0).into_iter().collect(),
            ..Default::default()
        });
        definition.mana_abilities.push(ManaAbility {
            self_damage: None,
            restriction: None,
            costs: PermanentActivationCosts {
                remove_source_plus_one_counters: None,
                tap: true,
                mana: None,
                sacrifice: false,
            },
            production: ManaProduction::Fixed {
                symbols: vec![self.color],
            },
            source_block: u32::from(self.tapped),
        });
        if self.tapped {
            definition.static_abilities.push(StaticAbility {
                functioning_zone: FunctioningZone::Battlefield,
                effect: StaticEffect::EntersTapped,
            });
        }
        definition.capabilities = definition.requirements();
        definition
    }
}

pub(super) fn plain_face(
    name: String,
    oracle_id: String,
    color_identity: Vec<Color>,
    provenance: SourceProvenance,
) -> Definition {
    Definition {
        modal_land: None,
        transform_faces: None,
        source_labels: None,
        x_mana_cost: false,
        oracle_id,
        name,
        characteristics: Characteristics {
            kind: CardKind::Land,
            additional_types: vec![],
            subtypes: vec![],
            basic: false,
            legendary: false,
            snow: false,
            hybrid_mana: vec![],
            mana_cost: ManaCost {
                generic: Quantity::zero(),
                colored: vec![],
            },
            colors: vec![],
            color_identity,
            power: None,
            toughness: None,
        },
        composed: false,
        intrinsic_zone: None,
        spell: None,
        activated: vec![],
        mana_abilities: vec![],
        triggered: vec![],
        static_abilities: vec![],
        keywords: vec![],
        capabilities: BTreeSet::new(),
        provenance,
    }
}

impl Definition {
    /// Cached, derived executable values for the chosen back side. The root
    /// still owns archive identity and remains the physical card definition.
    pub fn modal_land_back(&self) -> Option<&Definition> {
        let face = self.modal_land.as_ref()?;
        Some(
            face.projected
                .get_or_init(|| {
                    Box::new(face.back.definition(
                        self.oracle_id.clone(),
                        self.characteristics.color_identity.clone(),
                        self.provenance.clone(),
                    ))
                })
                .as_ref(),
        )
    }

    pub fn source_face_name(&self) -> &str {
        self.modal_land
            .as_ref()
            .map(|face| face.front_name.as_str())
            .or_else(|| self.transform_faces.as_ref().map(|face| face.front_name()))
            .unwrap_or(self.name.as_str())
    }
}

impl Eq for ModalLand {}
