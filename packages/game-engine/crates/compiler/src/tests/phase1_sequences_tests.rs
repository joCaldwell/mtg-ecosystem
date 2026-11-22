//! Frozen ordinary spell, target-fact, and continuous scope witnesses.
use super::*;
use serde_json::{Value, json};

const HEADER: &str = include_str!("../../../../tests/fixtures/archive-manifest-current.json");
const EXPLORE: &str =
    include_str!("../../../../tests/fixtures/b8f566ea-8283-4afa-9ac8-737e26419283.json");
const DEADLY: &str =
    include_str!("../../../../tests/fixtures/04a3bb42-0464-4638-a6ac-d5f412c222cc.json");
const RAPID: &str =
    include_str!("../../../../tests/fixtures/06692cd9-ac2f-4a32-8fd1-043ba3c0fe71.json");
const WING: &str =
    include_str!("../../../../tests/fixtures/7fbf6afc-8a16-447b-bdef-b7d0b640f76c.json");

fn definition(raw: &str) -> Definition {
    match compile(HEADER, raw) {
        CompileResult::Lowered { definition } => *definition,
        other => {
            let record: Value = serde_json::from_str(raw).unwrap();
            panic!("{}: {other:?}", record["source"]["name"])
        }
    }
}

#[test]
fn phase1_sequences_scalar_steps_keep_source_order_and_primitive_capabilities() {
    use Capability::*;
    let explore = definition(EXPLORE);
    let p = explore.spell.unwrap();
    assert_eq!(
        p.instructions,
        [
            Instruction::GrantLandPlaysThisTurn {
                count: 1.into(),
                source_block: 0
            },
            Instruction::draw(PlayerExpr::you(Context::Spell), 1.into(), 1),
        ]
    );
    for (raw, capabilities) in [
        (DEADLY, &[ControllerSurveilV1][..]),
        (
            include_str!("../../../../tests/fixtures/115fd5ac-460b-49a3-83ed-9d56a7aecb14.json"),
            &[ControllerScryV1][..],
        ),
        (
            include_str!("../../../../tests/fixtures/441de890-7b9f-42f7-859a-a154b8ed74ac.json"),
            &[ControllerScryV1, ControllerCountedDrawV1][..],
        ),
        (
            include_str!("../../../../tests/fixtures/5bf4d8d9-a2b2-4dba-ac05-9d4470a89db2.json"),
            &[ControllerScryV1, ControllerCountedDrawV1, LifeLossV1][..],
        ),
        (
            include_str!("../../../../tests/fixtures/47d52cd7-d72f-455f-aad3-70f08d54691f.json"),
            &[ControllerSurveilV1, ControllerCountedDrawV1, LifeLossV1][..],
        ),
        (
            include_str!("../../../../tests/fixtures/b734cfd9-afef-41f1-8377-68c885fb3a69.json"),
            &[FixedMillProgramV1, ControllerCountedDrawV1][..],
        ),
        (
            include_str!("../../../../tests/fixtures/8ca585b5-25c4-41cc-a0b9-dc66ae803d04.json"),
            &[ControllerCountedDrawV1][..],
        ),
    ] {
        let d = definition(raw);
        assert!(d.capabilities.contains(&ComposedOrdinarySpellV1));
        assert!(
            capabilities.iter().all(|c| d.capabilities.contains(c)),
            "{}",
            d.name
        );
        d.validate().unwrap();
    }
    let catalog = definition(include_str!(
        "../../../../tests/fixtures/965139fb-2e58-4ef2-b639-3ceaa2995c00.json"
    ));
    assert!(catalog.capabilities.contains(&ControllerDrawDiscardV1));
    let mut dropped = definition(DEADLY);
    dropped.spell.as_mut().unwrap().instructions.pop();
    dropped.capabilities = dropped.requirements();
    assert!(dropped.validate().is_err());
}

#[test]
fn phase1_sequences_splicers_keep_typed_etb_and_the_exact_live_subtype_scope() {
    use mtg_contracts::scoped_static::Subtype;
    for (raw, creatures_only) in [
        (WING, true),
        (
            include_str!("../../../../tests/fixtures/194166ad-0179-42a3-86b9-ba7f322ec576.json"),
            false,
        ),
        (
            include_str!("../../../../tests/fixtures/9676da93-1e17-4f6c-b610-a242b78c71ab.json"),
            true,
        ),
        (
            include_str!("../../../../tests/fixtures/9e0c5919-aed6-4d98-85c8-2658be78bdf0.json"),
            false,
        ),
        (
            include_str!("../../../../tests/fixtures/b0d0c534-8bd2-4801-aadd-dcc9c92d6ec3.json"),
            true,
        ),
    ] {
        let mut d = definition(raw);
        assert!(
            d.capabilities
                .contains(&Capability::FilteredStaticModifiersV1)
        );
        assert_eq!(d.triggered.len(), 1);
        let StaticEffect::ModifyCreatures { scope, .. } = &mut d.static_abilities[0].effect else {
            panic!("live scope")
        };
        assert_eq!(
            *scope,
            CreatureScope::ControlledSubtype {
                subtype: Subtype::Golem,
                creatures_only,
                other: false
            }
        );
        *scope = CreatureScope::ControlledSubtype {
            subtype: Subtype::Elf,
            creatures_only,
            other: false,
        };
        d.capabilities = d.requirements();
        assert!(d.validate().is_err());
    }
}

#[test]
fn phase1_sequences_target_facts_reject_wrong_antecedent_and_unconsumed_source() {
    for raw in [
        RAPID,
        include_str!("../../../../tests/fixtures/1657be64-b38f-4074-a743-bf558dc57901.json"),
    ] {
        let p = definition(raw).spell.unwrap();
        assert!(matches!(
            p.instructions.as_slice(),
            [
                kernel!(Destroy {
                    what: target_object!(0),
                    regeneration_forbidden: _
                }),
                Instruction::CreateTokensFor {
                    recipient: EffectRecipient::TargetController { target_slot: 0 },
                    ..
                },
            ]
        ));
    }
    let wrong_edge = super::tests::changed_source(RAPID, |r| {
        let edge = r["binding"]["references"]
            .as_array_mut()
            .unwrap()
            .iter_mut()
            .find(|edge| edge["path"].as_str().unwrap().ends_with("/who/of"))
            .unwrap();
        edge["target"] = json!("another-target");
    });
    let wrong_scope = super::tests::changed_source(WING, |r| {
        let scope = r["binding"]["nodes"]
            .as_array_mut()
            .unwrap()
            .iter_mut()
            .find(|node| node["timing"] == "continuous")
            .unwrap();
        scope["timing"] = json!("resolution");
    });
    let extra = super::tests::changed_source(EXPLORE, |r| {
        r["source"]["oracle_text"] = json!(
            "You may play an additional land this turn.\nDraw a card, then you may discard a card."
        );
    });
    let dropped = super::tests::changed_source(EXPLORE, |r| {
        let parsed = &mut r["faces"][0]["result"];
        parsed["abilities"].as_array_mut().unwrap().pop();
        parsed["lines"].as_array_mut().unwrap().pop();
    });
    for record in [
        wrong_edge.as_str(),
        wrong_scope.as_str(),
        extra.as_str(),
        dropped.as_str(),
        include_str!("../../../../tests/fixtures/d683d985-9888-4d21-8b5f-69e69ce4a03b.json"),
        include_str!("../../../../tests/fixtures/a1408bae-ed5f-4837-a1a7-438dc30a2786.json"),
    ] {
        assert!(!matches!(
            compile(HEADER, record),
            CompileResult::Lowered { .. }
        ));
    }
}
