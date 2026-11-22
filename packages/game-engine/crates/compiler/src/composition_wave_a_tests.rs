use super::*;

const MANIFEST: &str = include_str!("../../../tests/fixtures/archive-manifest-current.json");
const SHADE: &str =
    include_str!("../../../tests/fixtures/0036d062-10dc-4267-98b4-1d2f6b190a61.json");
const YETI: &str =
    include_str!("../../../tests/fixtures/375ea10b-da68-4c66-b3eb-f5113c09ea4b.json");
const PRISM: &str =
    include_str!("../../../tests/fixtures/134a9877-5cfb-4a2a-a0f0-930dce45f58b.json");
const ARC: &str = include_str!("../../../tests/fixtures/72a8620e-1ba2-40a8-9b22-11cc57864adf.json");
const SWORD: &str =
    include_str!("../../../tests/fixtures/181965fb-e685-4b1f-9bd6-3a33bed933b2.json");
const LOSS: &str =
    include_str!("../../../tests/fixtures/710e6d6a-caaa-4b76-82c8-e363e9438bec.json");

const MARAUDER: &str =
    include_str!("../../../tests/fixtures/4b1bf05e-753e-4350-a913-894cf3cecc0c.json");

fn lowered(raw: &str) -> Definition {
    match compile(MANIFEST, raw) {
        CompileResult::Lowered { definition } => *definition,
        rejected => panic!(
            "{:?}: {rejected:?}",
            strict_read::<serde_json::Value>(raw).unwrap()["source"]["name"]
        ),
    }
}

#[test]
fn composition_wave_a_binds_intrinsic_keywords_and_exact_siblings() {
    let shade = lowered(SHADE);
    assert_eq!(shade.keywords, [KeywordAbility::Swampwalk]);
    assert_eq!(shade.activated.len(), 1);
    let yeti = lowered(YETI);
    assert_eq!(
        yeti.keywords,
        [
            KeywordAbility::Mountainwalk,
            KeywordAbility::ProtectionFromWhite
        ]
    );
    let loss = lowered(LOSS);
    let mut forged = loss.clone();
    let kernel!(LoseLife {
        amount: fixed!(amount),
        ..
    }) = &mut forged.triggered[0].program.instructions[0]
    else {
        panic!("missing life loss");
    };
    *amount = 1.into();
    assert!(forged.validate().is_err());
    let mut missing = shade.clone();
    missing.source_labels = None;
    missing.activated.clear();
    missing.capabilities = missing.requirements();
    assert!(missing.validate().is_err());
}

#[test]
fn composition_wave_a_independently_qualifies_permanent_entry() {
    let marauder = lowered(MARAUDER);
    assert!(matches!(
        marauder.triggered[0].program.instructions.as_slice(),
        [Instruction::EachPlayerSacrifice { .. }]
    ));
    let d = lowered(PRISM);
    assert_eq!(d.mana_abilities.len(), 1);
    assert!(matches!(
        d.triggered[0].program.instructions.as_slice(),
        [kernel!(Draw {
            player: kir::PlayerExpr::Is { .. },
            amount: fixed!(_)
        })]
    ));
    let d = lowered(ARC);
    assert_eq!(
        d.triggered[0].program.targets[0].domain,
        TargetDomain::AnyDamageTarget
    );
    assert!(matches!(
        d.triggered[0].program.instructions.as_slice(),
        [Instruction::DealDamage { target_slot: 0, .. }]
    ));
    let mut duplicated = d.clone();
    duplicated.triggered.push(d.triggered[0].clone());
    duplicated.source_labels = None;
    duplicated.capabilities = duplicated.requirements();
    assert!(duplicated.validate().is_err());
    let d = lowered(SWORD);
    assert_eq!(d.static_abilities.len(), 1);
    assert_eq!(d.activated.len(), 1);
    assert!(matches!(
        d.triggered[0].program.instructions.as_slice(),
        [Instruction::DestroyPermanent { target_slot: 0, .. }]
    ));
    let forged = tests::changed_source(ARC, |row| {
        let object = format!(
            "card:{}/source#object",
            row["oracleIdentity"].as_str().unwrap()
        );
        let source = row["binding"]["references"]
            .as_array_mut()
            .unwrap()
            .iter_mut()
            .find(|r| r["path"] == "/faces/0/abilities/1/effects/0/steps/0/source")
            .unwrap();
        source["target"] = serde_json::json!(object);
    });
    assert!(!matches!(
        compile(MANIFEST, &forged),
        CompileResult::Lowered { .. }
    ));
}
