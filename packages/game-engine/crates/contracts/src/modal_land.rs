//! Complete modal land back faces and graveyard-return fronts, CR 712.8d/f, 712.12 and 712.14.
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;
use crate::bulk_catalog::{is_oracle_id, is_sha256};
use serde_json::{Value, json};
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
impl Eq for ModalLand {}

impl LandFace {
    fn text(&self) -> String {
        let prefix = if self.tapped {
            "This land enters tapped.\n"
        } else {
            ""
        };
        format!("{prefix}{{T}}: Add {{{:?}}}.", self.color)
    }

    fn read(face: &Value) -> Option<Self> {
        if face["type_line"] != "Land"
            || face["mana_cost"] != ""
            || face["colors"] != json!([])
            || [
                "power",
                "toughness",
                "defense",
                "loyalty",
                "color_indicator",
            ]
            .iter()
            .any(|field| face.get(*field).is_some())
        {
            return None;
        }
        let name = face["name"].as_str()?.to_owned();
        if name.is_empty() {
            return None;
        }
        let text = face["oracle_text"].as_str()?;
        let (text, tapped) = text
            .strip_prefix("This land enters tapped.\n")
            .map_or((text, false), |s| (s, true));
        let color = match text.strip_prefix("{T}: Add {")?.strip_suffix("}.")? {
            "W" => Color::W,
            "U" => Color::U,
            "B" => Color::B,
            "R" => Color::R,
            "G" => Color::G,
            "C" => Color::C,
            _ => return None,
        };
        Some(Self {
            name,
            color,
            tapped,
        })
    }

    fn abilities(&self) -> Value {
        let mut abilities = vec![];
        if self.tapped {
            abilities.push(json!({"kind":"static","effect":{"static":"enters-tapped","what":{"ref":"this","noun":"land"}}}));
        }
        abilities.push(json!({"kind":"activated","costs":[{"cost":"tap-self"}],
            "effects":[{"steps":[{"effect":"add-mana","who":{"player":"you"},
                "mana":{"mana":"fixed","symbols":[self.color]}}]}]}));
        Value::Array(abilities)
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

fn parse_return_spell(face: &Value) -> Option<(ManaCost, Vec<Color>)> {
    if face["type_line"] != "Sorcery"
        || face["oracle_text"] != own_graveyard_card_return::SPELL_TEXT
        || [
            "power",
            "toughness",
            "loyalty",
            "defense",
            "color_indicator",
        ]
        .iter()
        .any(|key| face.get(*key).is_some())
    {
        return None;
    }
    let printed = face["mana_cost"].as_str()?;
    let mut rest = printed;
    let mut cost = ManaCost {
        generic: Quantity::zero(),
        colored: vec![],
    };
    while let Some((symbol, remaining)) = rest.strip_prefix('{').and_then(|s| s.split_once('}')) {
        match symbol {
            "W" => cost.colored.push(Color::W),
            "U" => cost.colored.push(Color::U),
            "B" => cost.colored.push(Color::B),
            "R" => cost.colored.push(Color::R),
            "G" => cost.colored.push(Color::G),
            digits if cost.generic == Quantity::zero() && cost.colored.is_empty() => {
                cost.generic = i64::from(digits.parse::<u16>().ok()?).into();
            }
            _ => return None,
        }
        rest = remaining;
    }
    let colors: Vec<Color> = serde_json::from_value(face["colors"].clone()).ok()?;
    let printed_colors: BTreeSet<_> = cost.colored.iter().copied().collect();
    if !rest.is_empty()
        || printed != cost.printed_fixed_cost()
        || colors.iter().copied().collect::<BTreeSet<_>>() != printed_colors
        || colors.len() != printed_colors.len()
    {
        return None;
    }
    Some((cost, colors))
}

fn return_program() -> SpellProgram {
    SpellProgram {
        additional_sacrifice: None,
        optional_additional_costs: vec![],
        timing: SpellTiming::MainPhase,
        resolution: SpellResolution::Instructions,
        functioning_zone: FunctioningZone::Stack,
        modes: vec![],
        targets: vec![TargetSpec {
            slot: 0,
            selection_id: "line:0:1".into(),
            domain: TargetDomain::OwnGraveyardCard,
            chooser: Chooser::Controller,
            stage: EvaluationStage::Announcement,
            filter: None,
        }],
        instructions: vec![Instruction::ReturnOwnGraveyardCardToHand {
            target_slot: 0,
            source_block: 0,
            optional: false,
        }],
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

pub fn from_archive(provenance: SourceProvenance) -> Result<Definition, String> {
    let invalid = || "requires complete fixed-mana modal land faces".to_owned();
    if !accepted_provenance(
        &provenance.archive_manifest_sha256,
        &provenance.parser_sha256,
    ) || sha256(provenance.source_json.as_bytes()) != provenance.source_sha256
        || sha256(format!("[{}]", provenance.abilities_json).as_bytes()) != provenance.ast_sha256
        || !is_sha256(&provenance.binding_sha256)
    {
        return Err(invalid());
    }
    let source: Value = strict_read(&provenance.source_json)?;
    let ast: Value = strict_read(&format!("[{}]", provenance.abilities_json))?;
    let [front, back] = source["card_faces"]
        .as_array()
        .ok_or_else(invalid)?
        .as_slice()
    else {
        return Err(invalid());
    };
    let back = LandFace::read(back).ok_or_else(invalid)?;
    let front_name = front["name"]
        .as_str()
        .filter(|name| !name.is_empty())
        .ok_or_else(invalid)?;
    let oracle_id = source["oracle_id"]
        .as_str()
        .filter(|id| is_oracle_id(id))
        .ok_or_else(invalid)?;
    let identity: Vec<Color> =
        serde_json::from_value(source["color_identity"].clone()).map_err(|_| invalid())?;
    let name = format!("{front_name} // {}", back.name);
    if source["layout"] != "modal_dfc"
        || source["name"] != name
        || source["id"] != provenance.printing_id
        || !is_oracle_id(&provenance.printing_id)
        || source["keywords"] != json!([])
        || front_name == back.name
    {
        return Err(invalid());
    }
    let (mut definition, front_ast, front_colors) = if let Some(land) = LandFace::read(front) {
        if source["type_line"] != "Land // Land" {
            return Err(invalid());
        }
        let definition = land.definition(oracle_id.into(), identity.clone(), provenance);
        (definition, land.abilities(), vec![land.color])
    } else {
        let (cost, colors) = parse_return_spell(front).ok_or_else(invalid)?;
        if source["type_line"] != "Sorcery // Land" {
            return Err(invalid());
        }
        let mut definition = plain_face(
            front_name.into(),
            oracle_id.into(),
            identity.clone(),
            provenance,
        );
        definition.characteristics.kind = CardKind::Sorcery;
        definition.characteristics.mana_cost = cost;
        definition.characteristics.colors.clone_from(&colors);
        definition.spell = Some(return_program());
        let text = own_graveyard_card_return::SPELL_TEXT;
        definition.source_labels = Some(SourceLabels {
            text: text.into(),
            blocks: vec![text.into()],
            ..Default::default()
        });
        (
            definition,
            own_graveyard_card_return::abilities(false),
            colors,
        )
    };
    let printed_identity: BTreeSet<_> = front_colors
        .into_iter()
        .chain([back.color])
        .filter(|c| *c != Color::C)
        .collect();
    if identity.iter().copied().collect::<BTreeSet<_>>() != printed_identity
        || identity.len() != printed_identity.len()
        || ast != json!([front_ast, back.abilities()])
    {
        return Err(invalid());
    }
    definition.name = name;
    definition.modal_land = Some(Box::new(ModalLand {
        front_name: front_name.into(),
        back,
        projected: OnceLock::new(),
    }));
    definition.capabilities = definition.requirements();
    Ok(definition)
}

pub(super) fn validate(definition: &Definition) -> Result<bool, String> {
    let raw = &definition.provenance.source_json;
    if definition.modal_land.is_none()
        && !definition
            .capabilities
            .contains(&Capability::ModalLandFacesV1)
        && !raw.contains("modal_dfc")
        && !raw.contains(r"\u")
    {
        return Ok(false);
    }
    if definition.modal_land.is_none()
        && !definition
            .capabilities
            .contains(&Capability::ModalLandFacesV1)
        && strict_read::<Value>(raw)?["layout"] != "modal_dfc"
    {
        return Ok(false);
    }
    let expected = from_archive(definition.provenance.clone())?;
    if *definition != expected || definition.modal_land_back() != expected.modal_land_back() {
        return Err("modal face payload differs from complete retained source".into());
    }
    Ok(true)
}

pub(super) fn extend_schema(schema: &mut Value) {
    let capability = json!({"contains":{"const":Capability::ModalLandFacesV1}});
    let source = json!({"properties":{"provenance":{"properties":{
        "source_json":{"pattern":r#""layout"\s*:\s*"modal_dfc""#}
    }}}});
    schema["$defs"]["Definition"]["allOf"].as_array_mut().unwrap().push(json!({
        "if":{"anyOf":[{"required":["modal_land"]},{"properties":{"capabilities":capability}},source]},
        "then":{"required":["modal_land"],"properties":{
            "characteristics":{"properties":{"kind":{"enum":["land","sorcery"]},"subtypes":{"maxItems":0},"basic":{"const":false}}},
            "capabilities":{"allOf":[capability,{"contains":{"const":Capability::LandPlayV1}}]},
            "intrinsic_zone":{"type":"null"},"mana_abilities":{"maxItems":1},"triggered":{"maxItems":0},
            "activated":{"maxItems":0},"keywords":{"maxItems":0},"static_abilities":{"maxItems":1},
            "x_mana_cost":{"const":false}
        },"allOf":[
            source,
            {"if":{"properties":{"characteristics":{"properties":{"kind":{"const":"land"}}}}},
                "then":{"required":["mana_abilities"],"properties":{"spell":{"type":"null"},"mana_abilities":{"minItems":1}}}},
            {"if":{"properties":{"characteristics":{"properties":{"kind":{"const":"sorcery"}}}}},
                "then":{"required":["spell"],"properties":{"spell":{"type":"object"},"mana_abilities":{"maxItems":0},
                    "capabilities":{"contains":{"const":Capability::OwnGraveyardCardReturnV1}}}}}
        ]}
    }));
}

#[cfg(test)]
pub(super) fn witness() -> Definition {
    from_archive(witness_provenance(include_str!(
        "../../../tests/fixtures/phase1-r3/branchloft-pathway.json"
    )))
    .unwrap()
}

#[cfg(test)]
pub(super) fn witness_provenance(raw: &str) -> SourceProvenance {
    use serde_json::value::RawValue;
    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct Record {
        source_index: u32,
        source_sha256: String,
        ast_sha256: String,
        binding_sha256: String,
        source: Box<RawValue>,
        faces: Vec<Face>,
    }
    #[derive(Deserialize)]
    struct Face {
        result: Parsed,
    }
    #[derive(Deserialize)]
    struct Parsed {
        abilities: Box<RawValue>,
    }
    let row: Record = strict_read(raw).unwrap();
    let source: Value = strict_read(row.source.get()).unwrap();
    let pin = accepted_manifest(include_str!(
        "../../../tests/fixtures/archive-manifest-phase1-r3.json"
    ))
    .unwrap();
    SourceProvenance {
        printing_id: source["id"].as_str().unwrap().into(),
        source_index: row.source_index,
        source_sha256: row.source_sha256,
        ast_sha256: row.ast_sha256,
        binding_sha256: row.binding_sha256,
        archive_manifest_sha256: pin.manifest_sha256.clone(),
        parser_sha256: pin.parser_source_sha256.clone(),
        source_json: row.source.get().into(),
        abilities_json: format!(
            "{},{}",
            row.faces[0].result.abilities.get(),
            row.faces[1].result.abilities.get()
        ),
    }
}
