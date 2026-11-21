//! CR 109.5, 119.3, 603.6a: the land's own entry triggers mandatory life gain
//! for the controller of that trigger.
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;
use serde_json::{Value, json};

pub(super) fn lower(
    trigger: &SourceTrigger,
    effects: &[Sentence],
    intervening: Option<&SourceCondition>,
    block: usize,
) -> Result<Option<TriggeredAbility>, Failure> {
    if !matches!(trigger, SourceTrigger::Enters {what: TriggerObject::This {noun}} if noun == "land")
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
    let [
        Step::GainLife {
            who,
            amount,
            for_each: None,
        },
    ] = steps.as_slice()
    else {
        return Ok(None);
    };
    if intervening.is_some()
        || !bindings.is_empty()
        || who.player != "you"
        || who.of.is_some()
        || who.selection_id.is_some()
    {
        return Err(unsupported(
            "abilities.trigger",
            "entry life gain must be mandatory for the trigger controller",
        ));
    }
    let amount = fixed_positive(
        &serde_json::from_value(amount.clone())
            .map_err(|_| unsupported("amount", "entry life gain requires a fixed amount"))?,
    )?;
    Ok(Some(TriggeredAbility {
        functioning_zone: FunctioningZone::Battlefield,
        condition: TriggerCondition::SelfEntersBattlefield,
        intervening_if: None,
        death_filter: None,
        program: SpellProgram {
            functioning_zone: FunctioningZone::Stack,
            timing: SpellTiming::Instant,
            resolution: SpellResolution::Instructions,
            optional_additional_costs: vec![],
            additional_sacrifice: None,
            targets: vec![],
            instructions: vec![Instruction::act(
                Verb::GainLife {
                    player: PlayerExpr::you(Context::Triggered),
                    amount: Amount::Fixed { value: amount },
                },
                source_block(block)?,
            )],
            modes: vec![],
        },
    }))
}

pub(super) fn validate_binding(
    definition: &Definition,
    source: &Value,
    binding: &Value,
) -> Result<(), Failure> {
    if !definition
        .capabilities
        .contains(&Capability::LandEntryGainLifeV1)
    {
        return Ok(());
    }
    // Definition::validate already checked the complete static/trigger/mana shape.
    let tapped = !definition.static_abilities.is_empty();
    let block = usize::from(tapped);
    let mut graph = bindings::Bindings::new(&definition.oracle_id, source);
    let object = graph.id("/source", "object");
    let subject = json!({"ref":"this","noun":"land"});
    if tapped {
        let root = "/faces/0/abilities/0";
        let effect = format!("{root}/effect");
        graph.ability(root, "static", "static-source-controller");
        graph.node(&effect, "entering-controller", "runtime-player", json!({
            "scope":graph.id(root,"ability"),"role":"entering-controller","rules":["110.2a","614.12"]
        }));
        graph.reference(&format!("{effect}/what"), &subject, &object, "201.5");
    }
    let root = format!("/faces/0/abilities/{block}");
    let trigger = format!("{root}/trigger");
    let step = format!("{root}/effects/0/steps/0");
    graph.ability(&root, "triggered", "trigger-controller");
    graph.reference(&format!("{trigger}/what"), &subject, &object, "201.5");
    graph.event(&root, &trigger, "trigger:enters", &object, false);
    graph.result(
        &trigger,
        "objects",
        "same-objects",
        Some(&json!({"types":["land"]})),
    );
    let controller = graph.id(&root, "you");
    graph.reference(
        &format!("{step}/who"),
        &json!({"player":"you"}),
        &controller,
        "109.5",
    );
    graph.event(&root, &step, "gain-life", &controller, false);
    graph.result(&step, "quantity", "measured-quantity", None);
    listed_mana::bind_tap_mana(&mut graph, &format!("/faces/0/abilities/{}", block + 1));
    graph.validate(
        binding,
        "requires a complete land entry life gain binding graph",
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    const MANIFEST: &str = include_str!("../../../tests/fixtures/archive-manifest-current.json");
    const SOURCE: &str =
        include_str!("../../../tests/fixtures/5d641bf6-0f93-4189-8dc1-ec7ea446dade.json");

    #[test]
    fn land_entry_life_gain_authenticates_amount_and_trigger_controller() {
        assert_eq!(
            sha256(SOURCE.trim_end().as_bytes()),
            "fefe55af922069abf5e106082a3cef837885f74c4b3d64283d6707e66ce0e4b7"
        );
        let result = compile(MANIFEST, SOURCE);
        let CompileResult::Lowered { definition } = result else {
            panic!("expected Tranquil Cove to lower: {result:?}");
        };
        assert!(
            definition
                .capabilities
                .contains(&Capability::LandEntryGainLifeV1)
        );
        assert_eq!(
            definition.triggered[0].condition,
            TriggerCondition::SelfEntersBattlefield
        );
        assert_eq!(
            definition.triggered[0].program.instructions,
            [Instruction::act(
                Verb::GainLife {
                    player: PlayerExpr::you(Context::Triggered),
                    amount: Amount::fixed(1)
                },
                1
            )]
        );

        let wrong_amount = crate::tests::changed_source(SOURCE, |record| {
            let result = &mut record["faces"][0]["result"];
            result["abilities"][1]["effects"][0]["steps"][0]["amount"]["value"] = json!(2);
            result["lines"][1]["ability"] = result["abilities"][1].clone();
        });
        assert!(matches!(compile(MANIFEST, &wrong_amount),
            CompileResult::Unsupported {path, ..} if path == "definition"));

        let wrong_recipient = crate::tests::changed_source(SOURCE, |record| {
            let refs = record["binding"]["references"].as_array_mut().unwrap();
            let recipient = refs
                .iter_mut()
                .find(|r| r["path"] == "/faces/0/abilities/1/effects/0/steps/0/who")
                .unwrap();
            recipient["target"] = json!(recipient["target"].as_str().unwrap().replace(
                "/abilities/1#you",
                "/abilities/0/effect#entering-controller"
            ));
        });
        assert!(matches!(compile(MANIFEST, &wrong_recipient),
            CompileResult::Unsupported {path, ..} if path == "binding"));
    }
}
