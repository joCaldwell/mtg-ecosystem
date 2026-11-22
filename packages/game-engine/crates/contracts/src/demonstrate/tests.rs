use super::*;

pub(crate) fn witness() -> Definition {
    // A synthetic contract witness using the exact historical printed source.
    // The compiler probe separately proves the private AST archive and graph.
    let record: Value = strict_read(include_str!(
        "../../../../tests/fixtures/ramos/c336b735-3e6b-471e-be7a-84d7402ae2f5.json"
    ))
    .unwrap();
    let source = &record["source"];
    let mut d = each_player_draw::witness();
    d.name = source["name"].as_str().unwrap().into();
    d.oracle_id = source["oracle_id"].as_str().unwrap().into();
    d.characteristics.kind = CardKind::Sorcery;
    d.characteristics.colors = vec![Color::R];
    d.characteristics.color_identity = vec![Color::R];
    d.characteristics.mana_cost = ManaCost {
        generic: 4.into(),
        colored: vec![Color::R],
    };
    let p = d.spell.as_mut().unwrap();
    p.timing = SpellTiming::MainPhase;
    p.instructions = vec![reveal()];
    d.triggered = vec![TriggeredAbility {
        functioning_zone: FunctioningZone::Stack,
        condition: TriggerCondition::SelfSpellCast,
        intervening_if: None,
        death_filter: None,
        program: SpellProgram {
            timing: SpellTiming::Instant,
            instructions: vec![demonstrate()],
            ..p.clone()
        },
    }];
    d.source_labels = Some(SourceLabels {
        text: text(),
        blocks: vec!["Demonstrate".into(), REVEAL_AND_CAST.into()],
        triggered: vec![0],
        ..SourceLabels::default()
    });
    d.provenance.printing_id = source["id"].as_str().unwrap().into();
    d.provenance.source_json = source.to_string();
    d.provenance.source_sha256 = sha256(d.provenance.source_json.as_bytes());
    d.provenance.abilities_json = abilities().to_string();
    d.provenance.ast_sha256 = sha256(format!("[{}]", d.provenance.abilities_json).as_bytes());
    d.capabilities = d.requirements();
    d
}

#[test]
fn creative_technique() {
    let d = witness();
    d.validate().unwrap();
    assert_eq!(d.capabilities, BTreeSet::from(CAPS));
    let decoded: Definition = strict_read(&canonical(&d).unwrap()).unwrap();
    assert_eq!(d, decoded);
    decoded.validate().unwrap();
    for (instruction, op, block) in [
        (demonstrate(), "demonstrate", 0),
        (reveal(), "reveal_and_cast", 1),
    ] {
        assert_eq!(instruction.source_block(), block);
        assert_eq!(json!(instruction), json!({"op":op,"source_block":block}));
        assert!(
            strict_read::<Instruction>(
                &json!({
                    "op":op,"source_block":block,"target_slot":0
                })
                .to_string()
            )
            .is_err()
        );
    }

    let changes: &[fn(&mut Definition)] = &[
        |d| d.characteristics.kind = CardKind::Instant,
        |d| d.triggered.clear(),
        |d| d.triggered.push(d.triggered[0].clone()),
        |d| d.triggered[0].functioning_zone = FunctioningZone::Battlefield,
        |d| d.triggered[0].condition = TriggerCondition::SelfEntersBattlefield,
        |d| d.triggered[0].intervening_if = Some(InterveningCondition::SourceCastByController),
        |d| {
            d.triggered[0].program.instructions = vec![Instruction::Demonstrate { source_block: 1 }]
        },
        |d| {
            d.spell.as_mut().unwrap().instructions =
                vec![Instruction::RevealAndCast { source_block: 0 }]
        },
        |d| d.spell.as_mut().unwrap().instructions.push(demonstrate()),
        |d| d.triggered[0].program.instructions.push(reveal()),
        |d| {
            d.spell.as_mut().unwrap().targets =
                targeted_counted_draw::witness().spell.unwrap().targets;
        },
        |d| {
            d.triggered[0].program.targets =
                targeted_counted_draw::witness().spell.unwrap().targets;
        },
        |d| {
            let spell = d.spell.as_mut().unwrap();
            let mode = spell.clone();
            spell.instructions.clear();
            spell.modes.push(SpellMode { program: mode });
        },
        |d| {
            d.activated.push(ActivatedAbility {
                functioning_zone: FunctioningZone::Battlefield,
                cost: ActivationCost::TapSource,
                program: d.triggered.remove(0).program,
            })
        },
        |d| {
            d.static_abilities.push(StaticAbility {
                functioning_zone: FunctioningZone::Battlefield,
                effect: StaticEffect::GrantSpellCascade {
                    eligibility: SpellCascadeEligibility::FirstCastEachTurn {},
                    program: d.triggered.remove(0).program,
                },
            })
        },
        |d| {
            d.provenance.abilities_json = "[]".into();
            d.provenance.ast_sha256 = sha256(b"[[]]");
        },
        |d| {
            d.triggered.clear();
            d.spell.as_mut().unwrap().instructions.clear();
            let mut source: Value = strict_read(&d.provenance.source_json).unwrap();
            source["oracle_text"] = json!("Draw a card.");
            source["keywords"] = json!([]);
            d.provenance.source_json = source.to_string();
            d.provenance.source_sha256 = sha256(d.provenance.source_json.as_bytes());
            d.source_labels = None;
        },
    ];
    for (index, change) in changes.iter().enumerate() {
        let mut bad = d.clone();
        change(&mut bad);
        bad.capabilities = bad.requirements();
        assert!(bad.validate().is_err(), "mutation {index}");
    }
    for cap in CAPS {
        let mut missing = d.clone();
        missing.capabilities.remove(&cap);
        assert!(missing.validate().is_err(), "missing {cap:?}");
    }
    let mut stray = d;
    stray.capabilities.insert(Capability::CascadeV1);
    assert!(stray.validate().is_err());
}
