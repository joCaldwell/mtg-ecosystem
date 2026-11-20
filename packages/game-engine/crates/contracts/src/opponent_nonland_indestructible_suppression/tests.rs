use super::*;
use serde_json::value::RawValue;

pub(crate) fn witness() -> Definition {
    const ROW: &str =
        include_str!("../../../../tests/fixtures/ramos/8d90ffb5-63da-48d5-a5a2-7b61062556f6.json");
    const HEADER: &str =
        include_str!("../../../../tests/fixtures/ramos/archive-manifest-current.json");
    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct Row {
        source: Box<RawValue>,
        faces: Vec<Face>,
        source_index: u32,
        source_sha256: String,
        ast_sha256: String,
        binding_sha256: String,
    }
    #[derive(Deserialize)]
    struct Face {
        result: Parsed,
    }
    #[derive(Deserialize)]
    struct Parsed {
        abilities: Box<RawValue>,
    }

    let row: Row = strict_read(ROW).unwrap();
    let source: Value = strict_read(row.source.get()).unwrap();
    let archive = accepted_manifest(HEADER).unwrap();
    Definition {
        oracle_id: source["oracle_id"].as_str().unwrap().into(),
        name: source["name"].as_str().unwrap().into(),
        source_labels: Some(labels()),
        characteristics: characteristics(),
        spell: Some(program()),
        activated: vec![],
        mana_abilities: vec![],
        keywords: vec![],
        static_abilities: vec![],
        triggered: vec![],
        intrinsic_zone: None,
        x_mana_cost: false,
        capabilities: BTreeSet::from(CAPS),
        provenance: SourceProvenance {
            printing_id: source["id"].as_str().unwrap().into(),
            source_index: row.source_index,
            source_sha256: row.source_sha256,
            ast_sha256: row.ast_sha256,
            binding_sha256: row.binding_sha256,
            archive_manifest_sha256: archive.manifest_sha256.clone(),
            parser_sha256: archive.parser_source_sha256.clone(),
            source_json: row.source.get().into(),
            abilities_json: row.faces[0].result.abilities.get().into(),
        },
    }
}

#[test]
fn opponent_nonland_indestructible_suppression() {
    let d = witness();
    d.validate().unwrap();
    assert_eq!(d.requirements(), BTreeSet::from(CAPS));
    let p = d.spell.as_ref().unwrap();
    let [suppress, destroy] = instructions();
    assert_eq!(
        json!(suppress),
        json!({"op":"suppress_opponent_nonland_indestructible","source_block":0})
    );
    assert!(serde_json::from_value::<Instruction>(json!({"op":"suppress_opponent_nonland_indestructible","source_block":0,"set":"creatures"})).is_err());
    for ops in [
        vec![suppress.clone()],
        vec![destroy.clone()],
        vec![destroy.clone(), suppress.clone()],
        vec![suppress.clone(), suppress.clone(), destroy.clone()],
        vec![
            Instruction::SuppressOpponentNonlandIndestructible { source_block: 1 },
            destroy.clone(),
        ],
        vec![
            suppress.clone(),
            Instruction::DestroySet {
                set: ObjectSet::NonlandPermanents,
                regeneration_forbidden: true,
                source_block: 1,
            },
        ],
        vec![
            suppress,
            Instruction::DestroySet {
                set: ObjectSet::OpponentNonlandPermanents,
                regeneration_forbidden: false,
                source_block: 1,
            },
        ],
        vec![Instruction::DestroyAllCreatures { source_block: 0 }],
    ] {
        let mut bad = d.clone();
        bad.spell.as_mut().unwrap().instructions = ops;
        bad.capabilities = bad.requirements();
        assert!(bad.validate().is_err());
    }
    let mut bad = d.clone();
    bad.capabilities
        .remove(&Capability::OpponentNonlandIndestructibleSuppressionV1);
    assert!(bad.validate().is_err());
    let mut nested = d.clone();
    nested
        .spell
        .as_mut()
        .unwrap()
        .modes
        .push(SpellMode { program: p.clone() });
    nested.capabilities = nested.requirements();
    assert!(nested.validate().is_err());
    let mut cost = d.clone();
    cost.spell
        .as_mut()
        .unwrap()
        .optional_additional_costs
        .push(OptionalAdditionalManaCost {
            identity: "extra".into(),
            source_block: 0,
            mana: d.characteristics.mana_cost.clone(),
        });
    cost.capabilities = cost.requirements();
    assert!(cost.validate().is_err());

    // An unrelated printed spell cannot use the unrestricted nonland destroy by itself.
    let mut foreign = crate::counter_unless_generic::witness();
    foreign.spell.as_mut().unwrap().instructions = vec![destroy];
    assert!(validate_definition(&foreign).is_err());
}
