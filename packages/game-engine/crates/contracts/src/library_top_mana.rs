//! Source closure for the reviewed two-block c38aef79 reparse.
//! CR 106.4, 500.5, 505.1, 608.2c-d and 701.20.
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;

pub const ARCHIVED_ROW: &str = "13234a94184005a029e34f9b084bfa8df1faa36ce041f8996c999369b74b0775";
pub const ARCHIVED_BINDING: &str =
    "5f0cc7147ffbaddb4f4c15ec6ed1a8fa9fe5e11661e7d76b0efc11df0890d254";
pub const SOURCE: &str = "ef30e9c890e3c92f31f307688bef9808a94381b2f1fc10eccb62a3225e64932d";
pub const AST: &str = "47fbe83b9eb8cf9ad82b9a1d2e20a1222d81840df0cbdc13f0cd230017554197";
pub const BINDING: &str = "8c50edda9acb04773896234c7038ee1eadb4c78365a788d0700130e5b505851f";
pub const PARSER: &str = "f2c18417c3f3d0acb40880c0beede8d0a600a87beeec5c0ed52d070f2eb56d21";
pub const ARCHIVE: &str = "994bc48e8d8791696330f63338bc32ee0bd17c8c7e9901956ebd35c171f9315a";

fn definition(provenance: SourceProvenance) -> Result<Definition, String> {
    if provenance.source_sha256 != SOURCE
        || sha256(provenance.source_json.as_bytes()) != SOURCE
        || provenance.ast_sha256 != AST
        || sha256(format!("[{}]", provenance.abilities_json).as_bytes()) != AST
        || provenance.binding_sha256 != BINDING
        || provenance.parser_sha256 != PARSER
        || provenance.archive_manifest_sha256 != ARCHIVE
        || provenance.source_index != 5753
    {
        return Err("invalid frozen library-top source or reparse identity".into());
    }
    let source: serde_json::Value = strict_read(&provenance.source_json)?;
    let field = |key: &str| source[key].as_str().ok_or("missing printed field");
    if field("id")? != provenance.printing_id {
        return Err("incorrect frozen printing".into());
    }
    let (mana_cost, hybrid_mana) =
        color_hybrid_cost(field("mana_cost")?).ok_or("unsupported printed cost")?;
    let spell = SpellProgram {
        timing: SpellTiming::MainPhase,
        resolution: SpellResolution::Permanent,
        functioning_zone: FunctioningZone::Stack,
        targets: vec![],
        instructions: vec![],
        modes: vec![],
        additional_sacrifice: None,
        optional_additional_costs: vec![],
    };
    let abilities: serde_json::Value = strict_read(&provenance.abilities_json)?;
    let look = &abilities[1]["effects"][0]["steps"][0];
    let amount = |node: &serde_json::Value| {
        node["value"]
            .as_i64()
            .map(Quantity::from)
            .ok_or("expected a fixed quantity")
    };
    let trigger = TriggeredAbility {
        functioning_zone: FunctioningZone::Battlefield,
        death_filter: None,
        intervening_if: None,
        condition: TriggerCondition::StepBegan {
            step: BeginningStep::PrecombatMain,
            turn_relation: TurnRelation::Controller,
        },
        program: SpellProgram {
            timing: SpellTiming::Instant,
            resolution: SpellResolution::Instructions,
            instructions: vec![Instruction::LookTopMayRevealForMana {
                minimum_symbols: amount(&look["revealIf"]["value"])?,
                mana: amount(&look["mana"])?,
                source_block: 1,
            }],
            ..spell.clone()
        },
    };
    let mut d = Definition {
        oracle_id: field("oracle_id")?.into(),
        name: field("name")?.into(),
        source_labels: Some(SourceLabels {
            text: field("oracle_text")?.into(),
            blocks: field("oracle_text")?.lines().map(str::to_owned).collect(),
            activated: vec![],
            triggered: vec![1],
            mana: vec![],
            statics: vec![0],
            spell_modes: vec![],
        }),
        x_mana_cost: false,
        characteristics: Characteristics {
            kind: CardKind::Creature,
            additional_types: vec![],
            basic: false,
            legendary: true,
            snow: false,
            subtypes: vec!["Phyrexian".into(), "Elemental".into()],
            hybrid_mana,
            mana_cost,
            colors: strict_read(&source["colors"].to_string())?,
            color_identity: strict_read(&source["color_identity"].to_string())?,
            power: Some(field("power")?.parse()?),
            toughness: Some(field("toughness")?.parse()?),
        },
        provenance,
        spell: Some(spell),
        intrinsic_zone: None,
        triggered: vec![trigger],
        activated: vec![],
        mana_abilities: vec![],
        keywords: vec![],
        static_abilities: vec![StaticAbility {
            functioning_zone: FunctioningZone::Battlefield,
            effect: StaticEffect::KeepUnspentManaAsBlack,
        }],
        capabilities: BTreeSet::new(),
    };
    d.capabilities = d.requirements();
    Ok(d)
}

// The archive identifies the unchanged failed row. The AST and binding are the
// independent compact JSON subvalues from the complete c38aef79 reparse.
pub fn frozen_definition() -> Result<Definition, String> {
    #[derive(Deserialize)]
    struct Row {
        source: Box<serde_json::value::RawValue>,
    }
    let row: Row = strict_read(include_str!(
        "../../../tests/fixtures/ramos/26445dc7-8363-4205-aee8-1cafeb4ba4c0.json"
    ))?;
    let abilities =
        include_str!("../../../tests/fixtures/ramos/library-top-mana-abilities.json").trim_end();
    let graph =
        include_str!("../../../tests/fixtures/ramos/library-top-mana-binding.json").trim_end();
    definition(SourceProvenance {
        printing_id: "33d94ecf-758b-4f68-a7be-6bf3ff1047f4".into(),
        source_index: 5753,
        source_sha256: SOURCE.into(),
        ast_sha256: AST.into(),
        binding_sha256: sha256(graph.as_bytes()),
        archive_manifest_sha256: ARCHIVE.into(),
        parser_sha256: PARSER.into(),
        source_json: row.source.get().into(),
        abilities_json: abilities.into(),
    })
}

pub(super) fn extend_schema(schema: &mut serde_json::Value) {
    use serde_json::json;
    let constraints = &mut schema["$defs"]["Definition"]["allOf"];
    let previous = constraints.take();
    *constraints = json!([{
        "if":{"anyOf":[
            {"properties":{"provenance":{"properties":{"source_sha256":{"const":SOURCE}}}}},
            {"properties":{"capabilities":{"contains":{"const":Capability::LookTopMayRevealForManaV1}}}}
        ]},
        "then":{"const":frozen_definition().expect("authenticated frozen composition")},
        "else":{"allOf":previous}
    }]);
}
