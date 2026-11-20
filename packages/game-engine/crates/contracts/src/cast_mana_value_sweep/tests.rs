use super::*;

pub(crate) fn witness() -> Definition {
    // Same raw source on the historical unsupported row. This is a constructed
    // contract witness; the compiler probe proves lowering of the new archive.
    let historical: Value = strict_read(include_str!(
        "../../../../tests/fixtures/ramos/0623c51a-e829-413f-a3a0-817c0902821e.json"
    ))
    .unwrap();
    let source = &historical["source"];
    let mut d = each_player_draw::witness();
    d.name = source["name"].as_str().unwrap().into();
    d.oracle_id = source["oracle_id"].as_str().unwrap().into();
    d.source_labels = None;
    d.characteristics.kind = CardKind::Sorcery;
    d.characteristics.colors = vec![Color::R];
    d.characteristics.color_identity = vec![Color::R];
    d.characteristics.mana_cost = ManaCost {
        generic: 5.into(),
        colored: vec![Color::R; 3],
    };
    let p = d.spell.as_mut().unwrap();
    p.timing = SpellTiming::MainPhase;
    p.instructions = vec![instruction()];
    d.triggered = vec![cascade(), cascade()];
    d.provenance.printing_id = source["id"].as_str().unwrap().into();
    d.provenance.source_json = source.to_string();
    d.provenance.source_sha256 = sha256(d.provenance.source_json.as_bytes());
    d.provenance.abilities_json = abilities().to_string();
    d.provenance.ast_sha256 = sha256(format!("[{}]", d.provenance.abilities_json).as_bytes());
    d.capabilities = d.requirements();
    d
}

#[test]
fn cast_mana_value_sweep() {
    use Capability::*;
    let d = witness();
    d.validate().unwrap();
    assert_eq!(
        d.capabilities,
        BTreeSet::from([
            NormalLayoutV1,
            CastSorceryV1,
            TriggeredAbilityV1,
            CascadeV1,
            ResolutionCastV1,
            RandomBottomV1,
            BoardWipesV1,
            CastHistoryV1,
            CastManaValueSweepV1
        ])
    );
    assert_eq!(
        strict_read::<WipeAmount>(r#"{"amount":"other_casts_mana_value"}"#).unwrap(),
        WipeAmount::OtherCastsManaValue
    );
    assert!(
        strict_read::<WipeAmount>(r#"{"amount":"other_casts_mana_value","value":"2"}"#).is_err()
    );

    // These source-backed damage programs cover fixed, announced X and creature count.
    let legacy = Catalog::read(include_str!(
        "../../../../tests/fixtures/expected-wipes-ir.json"
    ))
    .unwrap();
    let mut old_amounts = BTreeSet::new();
    for mut old in legacy.definitions {
        let Some(p) = &mut old.spell else { continue };
        let Some(amount) = p.instructions.iter_mut().find_map(|i| match i {
            Instruction::DamageSet { amount, .. } => Some(amount),
            _ => None,
        }) else {
            continue;
        };
        old_amounts.insert(match amount {
            WipeAmount::Fixed { .. } => "fixed",
            WipeAmount::CreatureCount => "creature count",
            WipeAmount::AnnouncedX => "announced X",
            WipeAmount::OtherCastsManaValue => panic!("historical amount changed"),
        });
        assert!(!old.capabilities.contains(&CastManaValueSweepV1));
        *amount = WipeAmount::OtherCastsManaValue;
        old.capabilities = old.requirements();
        assert!(old.validate().is_err());
    }
    assert_eq!(
        old_amounts,
        BTreeSet::from(["fixed", "creature count", "announced X"])
    );

    let changes: &[fn(&mut Definition)] = &[
        |d| {
            d.triggered.pop();
        },
        |d| d.triggered[0].program = canonical_cascade_program(1),
        |d| d.triggered[1].program = d.spell.clone().unwrap(),
        |d| {
            d.activated.push(ActivatedAbility {
                functioning_zone: FunctioningZone::Battlefield,
                cost: ActivationCost::TapSource,
                program: d.spell.clone().unwrap(),
            })
        },
        |d| {
            let p = d.spell.as_mut().unwrap();
            p.modes.push(SpellMode { program: p.clone() });
            p.instructions.clear();
        },
        |d| {
            let Instruction::DamageSet { sets, .. } =
                &mut d.spell.as_mut().unwrap().instructions[0]
            else {
                unreachable!()
            };
            *sets = vec![ObjectSet::Creatures];
        },
        |d| {
            let Instruction::DamageSet { amount, .. } =
                &mut d.spell.as_mut().unwrap().instructions[0]
            else {
                unreachable!()
            };
            *amount = WipeAmount::Fixed { value: 8.into() };
        },
        |d| {
            d.provenance.abilities_json = "[]".into();
            d.provenance.ast_sha256 = sha256(b"[[]]");
        },
        |d| {
            let mut source: Value = strict_read(&d.provenance.source_json).unwrap();
            source["oracle_text"] = json!("Cascade, cascade");
            d.provenance.source_json = source.to_string();
            d.provenance.source_sha256 = sha256(d.provenance.source_json.as_bytes());
        },
    ];
    for change in changes {
        let mut wrong = d.clone();
        change(&mut wrong);
        wrong.capabilities = wrong.requirements();
        assert!(wrong.validate().is_err());
    }
    let mut missing = d;
    missing.capabilities.remove(&CastManaValueSweepV1);
    assert!(missing.validate().is_err());
}
