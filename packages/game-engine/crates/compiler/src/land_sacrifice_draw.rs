//! CR 109.5, 201.5, 601.2h, 602.2b and 121.2: the activator pays mana,
//! taps and sacrifices this land, then draws one card when the ability resolves.
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;
use serde_json::{Value, json};

pub(super) fn lower(
    costs: &[SourceCost],
    effects: &[Sentence],
    block: usize,
    kind: CardKind,
) -> Result<Option<ActivatedAbility>, Failure> {
    if kind != CardKind::Land
        || !matches!(costs, [SourceCost::Mana { .. }, SourceCost::TapSelf {}, SourceCost::Sacrifice { what }]
            if *what == json!({"ref":"this","noun":"land"}))
    {
        return Ok(None);
    }
    let [
        Sentence {
            bindings,
            steps,
            condition: None,
            condition_kind: None,
        },
    ] = effects
    else {
        return Ok(None);
    };
    if !bindings.is_empty()
        || !matches!(steps.as_slice(), [Step::Draw { who, amount: DrawAmount::Fixed { value: 1 }, optional: false }]
            if who.player == "you" && who.selection_id.is_none() && who.of.is_none())
    {
        return Ok(None);
    }
    Ok(Some(ActivatedAbility {
        functioning_zone: FunctioningZone::Battlefield,
        cost: ActivationCost::Composite {
            costs: lower_permanent_costs(costs, kind)?,
        },
        program: SpellProgram {
            additional_sacrifice: None,
            optional_additional_costs: vec![],
            functioning_zone: FunctioningZone::Stack,
            timing: SpellTiming::Instant,
            resolution: SpellResolution::Instructions,
            targets: vec![],
            instructions: vec![Instruction::draw(
                PlayerExpr::you(Context::Activated),
                1.into(),
                source_block(block)?,
            )],
            modes: vec![],
        },
    }))
}

pub(super) fn validate_binding(
    d: &Definition,
    source: &Value,
    binding: &Value,
) -> Result<(), Failure> {
    if !d.capabilities.contains(&Capability::LandSacrificeDrawV1) {
        return Ok(());
    }
    let mut graph = bindings::Bindings::new(&d.oracle_id, source);
    let object = graph.id("/source", "object");
    let land = json!({"ref":"this","noun":"land"});
    let player = json!({"player":"you"});

    let entry = "/faces/0/abilities/0";
    let effect = format!("{entry}/effect");
    graph.ability(entry, "static", "static-source-controller");
    graph.node(&effect, "entering-controller", "runtime-player", json!({
        "scope":graph.id(entry,"ability"),"role":"entering-controller","rules":["110.2a","614.12"]
    }));
    graph.reference(&format!("{effect}/what"), &land, &object, "201.5");
    listed_mana::bind_tap_mana(&mut graph, "/faces/0/abilities/1");

    let draw = "/faces/0/abilities/2";
    let step = format!("{draw}/effects/0/steps/0");
    let sacrifice = format!("{draw}/costs/2");
    graph.ability(draw, "activated", "activator");
    let activator = graph.id(draw, "you");
    for index in 0..3 {
        graph.reference(
            &format!("{draw}/costs/{index}/payer"),
            &player,
            &activator,
            "109.5",
        );
    }
    graph.reference(
        &format!("{draw}/costs/1/source"),
        &json!({"ref":"self"}),
        &object,
        "201.5",
    );
    graph.reference(&format!("{sacrifice}/what"), &land, &object, "201.5");
    graph.node(
        &sacrifice,
        "event",
        "event",
        json!({
            "scope":graph.id(draw,"ability"),"operation":"cost:sacrifice","inputs":[],
            "availability":"when-performed","rules":["608.2c"]
        }),
    );
    graph.result(&sacrifice, "quantity", "measured-quantity", None);
    graph.reference(&format!("{step}/who"), &player, &activator, "109.5");
    graph.event(draw, &step, "draw", &activator, false);
    graph.result(
        &step,
        "objects",
        "new-objects",
        Some(&json!({"cls":"card"})),
    );
    graph.result(&step, "quantity", "measured-quantity", None);
    graph.validate(
        binding,
        "requires the complete land sacrifice and draw binding graph",
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    const MANIFEST: &str = include_str!("../../../tests/fixtures/archive-manifest-current.json");
    const SOURCE: &str =
        include_str!("../../../tests/fixtures/01ed9139-293d-4477-a145-3c6fe59266c7.json");

    #[test]
    fn land_sacrifice_draw_binds_same_source_and_activator() {
        assert_eq!(
            sha256(SOURCE.trim_end().as_bytes()),
            "1613aad04c9bf25e92eade63a81969eeea4b690e420bf6712c8a4c5e3b96567a"
        );
        let result = compile(MANIFEST, SOURCE);
        let CompileResult::Lowered { definition } = result else {
            panic!("expected Airship Engine Room to lower: {result:?}");
        };
        assert!(
            definition
                .capabilities
                .contains(&Capability::LandSacrificeDrawV1)
        );
        let [ability] = definition.activated.as_slice() else {
            panic!("expected exactly one nonmana activation");
        };
        assert_eq!(
            ability.cost,
            ActivationCost::Composite {
                costs: PermanentActivationCosts {
                    tap: true,
                    sacrifice: true,
                    mana: Some(ManaCost {
                        generic: 4.into(),
                        colored: vec![]
                    }),
                    remove_source_plus_one_counters: None,
                }
            }
        );
        assert_eq!(
            ability.program.instructions,
            [Instruction::draw(
                PlayerExpr::you(Context::Activated),
                1.into(),
                2
            )]
        );

        let wrong_payer = crate::tests::changed_source(SOURCE, |r| {
            let payer = r["binding"]["references"]
                .as_array_mut()
                .unwrap()
                .iter_mut()
                .find(|reference| reference["path"] == "/faces/0/abilities/2/costs/2/payer")
                .unwrap();
            payer["target"] = json!(payer["target"].as_str().unwrap().replace(
                "/abilities/2#you",
                "/abilities/0/effect#entering-controller"
            ));
        });
        assert!(matches!(compile(MANIFEST, &wrong_payer),
            CompileResult::Unsupported {path,..} if path=="binding"));

        let wrong_payment = crate::tests::changed_source(SOURCE, |r| {
            let result = &mut r["faces"][0]["result"];
            result["abilities"][2]["costs"][0]["symbols"][0] = json!("3");
            result["lines"][2]["ability"] = result["abilities"][2].clone();
        });
        assert!(matches!(compile(MANIFEST, &wrong_payment),
            CompileResult::Unsupported {path,..} if path=="definition"));
    }
}
