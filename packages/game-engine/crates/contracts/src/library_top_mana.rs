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
    let source: serde_json::Value = strict_read(&provenance.source_json)?;
    let field = |key: &str| source[key].as_str().ok_or("missing printed field");
    if field("mana_cost")?.is_empty() {
        return Err("missing printed cast cost".into());
    }
    if field("id")? != provenance.printing_id {
        return Err("incorrect frozen printing".into());
    }
    let (mana_cost, hybrid_mana) =
        color_hybrid_cost::ordinary_cost(field("mana_cost")?).ok_or("unsupported printed cost")?;
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
    let minimum_symbols = amount(&look["revealIf"]["value"])?;
    let mana = amount(&look["mana"])?;
    let mut expected: serde_json::Value = strict_read(include_str!(
        "../../../tests/fixtures/ramos/library-top-mana-abilities.json"
    ))?;
    let expected_step = &mut expected[1]["effects"][0]["steps"][0];
    expected_step["revealIf"]["value"]["value"] = look["revealIf"]["value"]["value"].clone();
    expected_step["mana"]["value"] = look["mana"]["value"].clone();
    let text = field("oracle_text")?;
    let (static_text, trigger_text) = text.split_once('\n').ok_or("expected two source blocks")?;
    let (threshold, rest) = trigger_text.strip_prefix("At the beginning of your first main phase, look at the top card of your library. You may reveal that card if it has ")
        .and_then(|s| s.split_once(" or more colored mana symbols in its mana cost. If you do, add ")).ok_or("invalid look/reveal condition")?;
    let produced = rest.strip_suffix(" mana in any combination of its colors and put it into your hand. If you don't reveal it, put it into your hand.").ok_or("invalid reveal/hand placement")?;
    if abilities != expected
        || static_text != unspent_mana::text()
        || printed_amount(threshold) != Some(minimum_symbols.clone())
        || printed_amount(produced) != Some(mana.clone())
    {
        return Err("library-top quantities, complete Oracle text and AST must agree".into());
    }
    let type_line = field("type_line")?;
    let (head, tail) = type_line.split_once(" — ").unwrap_or((type_line, ""));
    let legendary = match head {
        "Legendary Creature" => true,
        "Creature" => false,
        _ => return Err("library-top composition requires an ordinary creature".into()),
    };
    let subtypes: Vec<String> = tail.split_whitespace().map(str::to_owned).collect();
    if subtypes.iter().any(|s| unsupported_rules_subtype(s))
        || field("layout")? != "normal"
        || ["card_faces", "color_indicator", "defense", "loyalty"]
            .iter()
            .any(|key| !source[*key].is_null())
    {
        return Err("unsupported library-top source characteristics".into());
    }
    let colors: Vec<Color> = strict_read(&source["colors"].to_string())?;
    let color_identity: Vec<Color> = strict_read(&source["color_identity"].to_string())?;
    let printed_colors: BTreeSet<_> = mana_cost
        .colored
        .iter()
        .copied()
        .chain(
            hybrid_mana
                .iter()
                .flat_map(|s| s.colors)
                .filter_map(HybridPayment::color),
        )
        .filter(|c| *c != Color::C)
        .collect();
    if [&colors, &color_identity].iter().any(|list| {
        list.len() != printed_colors.len()
            || list.iter().copied().collect::<BTreeSet<_>>() != printed_colors
    }) {
        return Err("ordinary creature colors must agree with its printed cost".into());
    }
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
                minimum_symbols,
                mana,
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
            legendary,
            snow: false,
            subtypes,
            hybrid_mana,
            mana_cost,
            colors,
            color_identity,
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

fn printed_amount(word: &str) -> Option<Quantity> {
    let words = [
        "one", "two", "three", "four", "five", "six", "seven", "eight", "nine", "ten",
    ];
    let value = words
        .iter()
        .position(|w| *w == word)
        .map(|n| Quantity::from(n as i64 + 1))
        .or_else(|| word.parse().ok())?;
    (value > Quantity::zero()).then_some(value)
}

pub(super) fn validate(d: &Definition) -> Result<bool, String> {
    let marked = d
        .capabilities
        .contains(&Capability::LookTopMayRevealForManaV1)
        || d.triggered.iter().any(|a| {
            matches!(
                a.condition,
                TriggerCondition::StepBegan {
                    step: BeginningStep::PrecombatMain,
                    ..
                }
            )
        });
    if !marked
        && !d
            .provenance
            .abilities_json
            .contains("look-top-may-reveal-for-mana")
        && !d
            .provenance
            .source_json
            .contains("or more colored mana symbols")
        && !d.provenance.source_json.contains(r"\u")
    {
        return Ok(false);
    }
    let source: serde_json::Value = strict_read(&d.provenance.source_json)?;
    if !marked
        && !source["oracle_text"]
            .as_str()
            .unwrap_or("")
            .contains("or more colored mana symbols")
        && !d
            .provenance
            .abilities_json
            .contains("look-top-may-reveal-for-mana")
    {
        return Ok(false);
    }
    if definition(d.provenance.clone())? != *d {
        return Err("incomplete library-top mana composition".into());
    }
    Ok(true)
}

pub(super) fn authenticated_reparse(p: &SourceProvenance) -> bool {
    p.source_sha256 == SOURCE
        && p.ast_sha256 == AST
        && p.binding_sha256 == BINDING
        && p.archive_manifest_sha256 == ARCHIVE
        && p.parser_sha256 == PARSER
        && p.source_index == 5753
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
    let provenance = SourceProvenance {
        printing_id: "33d94ecf-758b-4f68-a7be-6bf3ff1047f4".into(),
        source_index: 5753,
        source_sha256: sha256(row.source.get().as_bytes()),
        ast_sha256: sha256(format!("[{abilities}]").as_bytes()),
        binding_sha256: sha256(graph.as_bytes()),
        archive_manifest_sha256: ARCHIVE.into(),
        parser_sha256: PARSER.into(),
        source_json: row.source.get().into(),
        abilities_json: abilities.into(),
    };
    if !authenticated_reparse(&provenance) {
        return Err("frozen reparse identity mismatch".into());
    }
    let d = definition(provenance)?;
    d.validate()?;
    Ok(d)
}

pub(super) fn marker_schema() -> serde_json::Value {
    serde_json::json!({"required":["capabilities"],"properties":{"capabilities":{"contains":{"const":Capability::LookTopMayRevealForManaV1}}}})
}

pub(super) fn extend_schema(schema: &mut serde_json::Value) {
    use serde_json::json;
    let op =
        json!({"required":["op"],"properties":{"op":{"const":"look_top_may_reveal_for_mana"}}});
    let step = json!({"step_began":{"step":"precombat_main","turn_relation":"controller"}});
    let free = json!({"properties":{"instructions":{"items":{"not":op}},"modes":{"items":{"properties":{"program":{"$ref":"#/$defs/LibraryTopManaFreeProgram"}}}}}});
    schema["$defs"]["LibraryTopManaFreeProgram"] = free;
    let positive = json!({"type":"string","pattern":"^[1-9][0-9]*$"});
    let program = json!({"properties":{"timing":{"const":"instant"},"resolution":{"const":"instructions"},
        "functioning_zone":{"const":"stack"},"targets":{"maxItems":0},"modes":{"maxItems":0},
        "additional_sacrifice":{"type":"null"},"optional_additional_costs":{"maxItems":0},
        "instructions":{"minItems":1,"maxItems":1,"items":{"allOf":[op,{"properties":{
            "minimum_symbols":positive,"mana":positive,"source_block":{"const":1}}}]}}}});
    let trigger = json!({"properties":{"condition":{"const":step},"functioning_zone":{"const":"battlefield"},
        "death_filter":{"type":"null"},"intervening_if":{"type":"null"},"program":program}});
    let no_program = json!({"$ref":"#/$defs/LibraryTopManaFreeProgram"});
    let precombat = json!({"type":"object","required":["step_began"],"properties":{
        "step_began":{"properties":{"step":{"const":"precombat_main"}}}}});
    schema["$defs"]["Definition"]["allOf"].as_array_mut().unwrap().push(json!({
        "if":marker_schema(),
        "then":{"required":["triggered","static_abilities","spell"],"properties":{
            "triggered":{"minItems":1,"maxItems":1,"items":trigger},
            "static_abilities":{"const":[{"functioning_zone":"battlefield","effect":"keep_unspent_mana_as_black"}]},
            "spell":{"type":"object","properties":{"resolution":{"const":"permanent"},"timing":{"const":"main_phase"},
                "functioning_zone":{"const":"stack"},"targets":{"maxItems":0},"instructions":{"maxItems":0},
                "modes":{"maxItems":0},"additional_sacrifice":{"type":"null"},"optional_additional_costs":{"maxItems":0}}},
            "activated":{"maxItems":0},"mana_abilities":{"maxItems":0},"keywords":{"maxItems":0},
            "characteristics":{"properties":{"kind":{"const":"creature"},"additional_types":{"maxItems":0},
                "basic":{"const":false},"snow":{"const":false}}}}},
        "else":{"properties":{"spell":no_program,"activated":{"items":{"properties":{"program":no_program}}},
            "triggered":{"items":{"properties":{"condition":{"not":precombat},"program":no_program}}}}}
    }));
}

#[test]
fn renamed_library_top_composition_binds_its_printed_quantities_and_characteristics() {
    let mut p = frozen_definition().unwrap().provenance;
    let mut source: serde_json::Value = strict_read(&p.source_json).unwrap();
    let graph = include_str!("../../../tests/fixtures/ramos/library-top-mana-binding.json")
        .replace("26445dc7-8363-4205-aee8-1cafeb4ba4c0", "synthetic-prism")
        .replace("Omnath, Locus of All", "Prism Witness")
        .replace(
            "Legendary Creature — Phyrexian Elemental",
            "Creature — Wizard",
        )
        .replace("{W}{U}{B/P}{R}{G}", "{2}{G}")
        .replace(
            r#""amount":"fixed","value":3"#,
            r#""amount":"fixed","value":2"#,
        );
    p.binding_sha256 = sha256(graph.trim_end().as_bytes());
    p.printing_id = "synthetic-prism-print".into();
    source["id"] = serde_json::json!(p.printing_id);
    source["name"] = serde_json::json!("Prism Witness");
    source["oracle_id"] = serde_json::json!("synthetic-prism");
    source["type_line"] = serde_json::json!("Creature — Wizard");
    source["power"] = serde_json::json!("2");
    source["mana_cost"] = serde_json::json!("{2}{G}");
    source["colors"] = serde_json::json!(["G"]);
    source["color_identity"] = serde_json::json!(["G"]);
    source["oracle_text"] = serde_json::json!(
        source["oracle_text"]
            .as_str()
            .unwrap()
            .replace("three or more", "two or more")
    );
    p.source_json = canonical(&source).unwrap();
    p.source_sha256 = sha256(p.source_json.as_bytes());
    let mut ast: serde_json::Value = strict_read(&p.abilities_json).unwrap();
    ast[1]["effects"][0]["steps"][0]["revealIf"]["value"]["value"] = serde_json::json!(2);
    p.abilities_json = canonical(&ast).unwrap();
    p.ast_sha256 = sha256(format!("[{}]", p.abilities_json).as_bytes());
    p.parser_sha256 = accepted_archives()
        .unwrap()
        .iter()
        .find(|a| a.manifest_sha256 == p.archive_manifest_sha256)
        .unwrap()
        .parser_source_sha256
        .clone();
    let mut d = definition(p).unwrap();
    d.validate().unwrap();
    assert_eq!(d.characteristics.subtypes, ["Wizard"]);
    assert!(!d.characteristics.legendary);
    let extra = d.triggered[0].program.instructions[0].clone();
    d.triggered[0].program.instructions.push(extra);
    assert!(d.validate().is_err());
}
