//! Full frozen source and deliberately changed-source negative controls.
use super::*;
use serde_json::json;
const RAW: &str = include_str!("../../../tests/fixtures/b5b4cf54-ed5e-42d0-9d98-5fec76b0b0b8.json");
#[test]
fn equipped_indestructible_full_plate_source() {
    assert_eq!(
        sha256(RAW.trim_end().as_bytes()),
        "faac785342cbfc3a67bce8cb491d039a42583e5e63c59caca44ab65c2d5c391f"
    );
    let CompileResult::Lowered { definition: d } = compile(fixtures::MANIFEST, RAW) else {
        panic!("complete Plate")
    };
    assert_eq!(d.name, "Darksteel Plate");
    assert_eq!(d.characteristics.kind, CardKind::Artifact);
    assert_eq!(d.characteristics.subtypes, ["Equipment"]);
    assert!(d.characteristics.additional_types.is_empty());
    assert!(d.characteristics.colors.is_empty() && d.characteristics.color_identity.is_empty());
    assert_eq!(d.characteristics.power, None);
    assert_eq!(d.characteristics.toughness, None);
    assert_eq!(
        d.characteristics.mana_cost,
        ManaCost {
            generic: 3.into(),
            colored: vec![]
        }
    );
    assert_eq!(d.keywords, [KeywordAbility::Indestructible]);
    assert_eq!(d.static_abilities.len(), 1);
    assert_eq!(
        d.static_abilities[0].effect,
        StaticEffect::ModifyCreatures {
            scope: CreatureScope::Equipped,
            power: 0.into(),
            toughness: 0.into(),
            keywords: vec![KeywordAbility::Indestructible],
            restrictions: vec![],
        }
    );
    assert_eq!(d.activated.len(), 1);
    let a = &d.activated[0];
    assert_eq!(
        a.cost,
        ActivationCost::Mana {
            cost: ManaCost {
                generic: 2.into(),
                colored: vec![]
            }
        }
    );
    assert!(matches!(
        a.program.instructions.as_slice(),
        [Instruction::AttachEquipment {
            target_slot: 0,
            source_block: 2
        }]
    ));
    assert_eq!(a.program.timing, SpellTiming::MainPhase);
    assert_eq!(
        a.program.targets[0].domain,
        TargetDomain::ControlledBattlefieldCreature
    );
    assert_eq!(d.provenance.source_index, 27380);
    assert_eq!(
        d.provenance.source_sha256,
        "fc06556d0b996b9945ed1033039647f03d1ff218314e1dc7a54c693255f422ef"
    );
    assert_eq!(
        d.provenance.ast_sha256,
        "61d09a3610f08900a49e6e95881b9092dee1c44077c307405dc9608b630f652a"
    );
    assert_eq!(
        d.provenance.binding_sha256,
        "52350dddab27232efc7274aae8daa09d54006e49b38ed9cd6c7986ebd6a3fba2"
    );
    assert_eq!(d.capabilities, d.requirements());
    let mut grant_only = *d;
    grant_only.keywords.clear();
    grant_only.capabilities = grant_only.requirements();
    grant_only.validate().unwrap();
    assert!(
        grant_only
            .capabilities
            .remove(&Capability::IndestructibleV1)
    );
    assert!(grant_only.validate().is_err());
}
#[test]
fn equipped_indestructible_full_source_rejects_mixed_and_other_grants() {
    for variant in 0..4 {
        let raw = tests::changed_source(RAW, |r| {
            let a = &mut r["faces"][0]["result"];
            let effect = &mut a["abilities"][1]["effect"];
            match variant {
                0 => effect["what"] = json!({"ref":"enchanted","noun":"creature"}),
                1 => effect["modifiers"][0]["abilities"]
                    .as_array_mut()
                    .unwrap()
                    .push(json!({"gained":"keyword","keyword":{"keyword":"flying"}})),
                2 => effect["modifiers"]
                    .as_array_mut()
                    .unwrap()
                    .push(json!({"modifier":"pt","power":{"sign":1,"amount":{"amount":"fixed","value":1}},"toughness":{"sign":1,"amount":{"amount":"fixed","value":1}}})),
                _ => effect["what"] = json!({"ref":"self"}),
            }
            a["lines"][1]["ability"] = a["abilities"][1].clone();
        });
        assert!(
            !matches!(
                compile(fixtures::MANIFEST, &raw),
                CompileResult::Lowered { .. }
            ),
            "variant {variant}"
        );
    }
}
