//! Exact archived source and complete semantic near misses; no fixture rewrites.
use super::*;
use serde_json::{Value, json};
const MANIFEST: &str = include_str!("../../../tests/fixtures/ramos/archive-manifest-current.json");
const SOURCE: &str =
    include_str!("../../../tests/fixtures/ramos/3ed41d2d-211b-4013-8562-8c64d54cc43a.json");
fn lowered() -> Box<Definition> {
    let result = compile(MANIFEST, SOURCE);
    let CompileResult::Lowered { definition } = result else {
        panic!("{result:?}")
    };
    definition
}
#[test]
fn ramos_m1_full_frozen_source_and_capability_fences() {
    let d = lowered();
    assert_eq!(d.oracle_id, "3ed41d2d-211b-4013-8562-8c64d54cc43a");
    assert_eq!(
        d.provenance.source_sha256,
        "0062c9639e97d8e719819c156c845f7d534b849095cf96ff8bb6c360b712999f"
    );
    assert_eq!(
        d.provenance.ast_sha256,
        "b61d955c581e9e821e0b7e493a5af98506532574f2869700bda5484916494fc8"
    );
    assert_eq!(d.keywords, [KeywordAbility::Flying]);
    assert_eq!(d.triggered.len(), 1);
    assert_eq!(d.mana_abilities.len(), 1);
    assert!(d.activated.is_empty());
    assert!(matches!(
        d.triggered[0].program.instructions.as_slice(),
        [Instruction::PutSourcePlusOneCountersForCastColors { source_block: 1 }]
    ));
    let a = &d.mana_abilities[0];
    assert!(!a.costs.tap && !a.costs.sacrifice && a.costs.mana.is_none());
    assert_eq!(a.costs.remove_source_plus_one_counters, Some(5.into()));
    assert_eq!(a.restriction, Some(ManaActivationRestriction::OnceEachTurn));
    assert_eq!(
        a.production,
        ManaProduction::Fixed {
            symbols: vec![
                Color::W,
                Color::W,
                Color::U,
                Color::U,
                Color::B,
                Color::B,
                Color::R,
                Color::R,
                Color::G,
                Color::G
            ]
        }
    );
    d.validate().unwrap();
    for c in [
        Capability::CastSpellColorCountersV1,
        Capability::CounterRemovalManaV1,
        Capability::PlusOneCountersV1,
        Capability::PermanentManaV1,
    ] {
        assert!(d.requirements().contains(&c));
        let mut bad = d.clone();
        bad.capabilities.remove(&c);
        assert!(bad.validate().is_err());
    }
    for mutate in [
        (|d: &mut Definition| d.triggered[0].condition = TriggerCondition::SelfEntersBattlefield)
            as fn(&mut Definition),
        |d| {
            d.triggered[0].condition = TriggerCondition::SpellCast {
                caster: CasterRelation::Opponent,
                spell: TypePredicate::Any {},
            }
        },
        |d| d.mana_abilities[0].restriction = None,
        |d| d.mana_abilities[0].costs.tap = true,
        |d| d.mana_abilities[0].costs.remove_source_plus_one_counters = Some(0.into()),
        |d| {
            d.mana_abilities[0].costs.mana = Some(ManaCost {
                generic: 1.into(),
                colored: vec![],
            })
        },
        |d| d.mana_abilities[0].costs.sacrifice = true,
        |d| d.mana_abilities[0].production = ManaProduction::AnyColor,
        |d| {
            let i = d.triggered[0].program.instructions[0].clone();
            d.spell.as_mut().unwrap().instructions.push(i);
        },
    ] {
        let mut bad = d.clone();
        mutate(&mut bad);
        bad.capabilities = bad.requirements();
        assert!(bad.validate().is_err());
    }
    assert_eq!(CURRENT_IR_VERSION, 1);
}
#[test]
fn ramos_m1_complete_near_miss_ast_rejections() {
    for mutate in [
        (|a: &mut Value| a[1]["effects"][0]["steps"][0]["count"]["factors"][0]["value"] = 2.into())
            as fn(&mut Value),
        |a| a[1]["effects"][0]["steps"][0]["count"]["factors"][1]["among"]["ref"] = "it".into(),
        |a| {
            a[1]["effects"][0]["steps"][0]["count"]["factors"][1]["characteristic"] =
                "color-identity".into()
        },
        |a| a[1]["effects"][0]["steps"][0]["counter"]["type"] = "charge".into(),
        |a| a[2]["costs"][0]["from"] = json!({"ref":"this","noun":"artifact"}),
        |a| a[2]["costs"][0]["counter"]["type"] = "charge".into(),
        |a| a[2]["costs"][0]["count"]["value"] = 0.into(),
        |a| a[2]["restriction"]["restriction"] = "once-your-turn".into(),
        |a| {
            a[2].as_object_mut().unwrap().remove("restriction");
        },
        |a| a[2]["restriction"]["extra"] = true.into(),
        |a| a[2]["unconsumed"] = true.into(),
        |a| a[1]["effects"][0]["steps"][0]["on"] = json!({"ref":"this","noun":"artifact"}),
    ] {
        let raw = tests::changed_source(SOURCE, |r| {
            mutate(&mut r["faces"][0]["result"]["abilities"]);
            let result = &mut r["faces"][0]["result"];
            for i in 0..3 {
                result["lines"][i]["ability"] = result["abilities"][i].clone();
            }
        });
        assert!(
            !matches!(compile(MANIFEST, &raw), CompileResult::Lowered { .. }),
            "near miss admitted"
        );
    }
    // Actual copies and spell-color changes remain unavailable as complete sources.
    for raw in [include_str!(
        "../../../tests/fixtures/ramos/ece44a82-dcf0-4439-bdd9-a09c99a6f159.json"
    )] {
        assert!(!matches!(
            compile(MANIFEST, raw),
            CompileResult::Lowered { .. }
        ));
    }
}
