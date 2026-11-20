// Both crates exercise the same original paid artifact using their local catalog envelope.
fn set_up_synthetic_artifact_loss(d: &mut Definition) {
    let subject = "this artifact";
    let text = format!(
        "This artifact enters tapped.\n{{U}}, {{T}}, Sacrifice {subject}: Target player loses the game."
    );
    let bound = ArtifactLossSource::parse(&text).unwrap();
    d.oracle_id = "synthetic-silent-chime".into();
    d.characteristics.subtypes.clear();
    d.name = "Synthetic Silent Chime".into();
    d.characteristics.kind = CardKind::Artifact;
    d.characteristics.colors.clear();
    d.characteristics.color_identity = vec![Color::U];
    d.characteristics.mana_cost = ManaCost {
        generic: 1.into(),
        colored: vec![],
    };
    d.source_labels = Some(SourceLabels {
        text: text.clone(),
        blocks: text.lines().map(str::to_owned).collect(),
        statics: vec![0],
        activated: vec![1],
        ..SourceLabels::default()
    });
    d.provenance.printing_id = "synthetic-silent-chime".into();
    d.provenance.source_json = canonical(&serde_json::json!({"id":d.provenance.printing_id,
            "oracle_id":d.oracle_id,"name":d.name,"layout":"normal","type_line":"Artifact","colors":[],
            "color_identity":["U"],"mana_cost":"{1}","oracle_text":text})).unwrap();
    d.provenance.source_sha256 = sha256(d.provenance.source_json.as_bytes());
    d.provenance.abilities_json = canonical(&bound.abilities).unwrap();
    d.provenance.ast_sha256 = sha256(format!("[{}]", d.provenance.abilities_json).as_bytes());
    let spell = d.spell.as_mut().unwrap();
    let mut program = spell.clone();
    program.timing = SpellTiming::Instant;
    program.targets = vec![TargetSpec {
        slot: 0,
        selection_id: bound.selection,
        domain: TargetDomain::AnyPlayer,
        filter: None,
        chooser: Chooser::Controller,
        stage: EvaluationStage::Announcement,
    }];
    program.instructions = vec![Instruction::LoseGameTargetPlayer {
        target_slot: 0,
        source_block: 1,
    }];
    spell.timing = SpellTiming::MainPhase;
    spell.resolution = SpellResolution::Permanent;
    spell.targets.clear();
    spell.instructions.clear();
    d.static_abilities = vec![StaticAbility {
        functioning_zone: FunctioningZone::Battlefield,
        effect: StaticEffect::EntersTapped,
    }];
    d.activated = vec![ActivatedAbility {
        functioning_zone: FunctioningZone::Battlefield,
        cost: ActivationCost::Composite {
            costs: PermanentActivationCosts {
                tap: true,
                mana: Some(bound.mana),
                sacrifice: true,
                remove_source_plus_one_counters: None,
            },
        },
        program,
    }];
    d.capabilities = d.requirements();
}
