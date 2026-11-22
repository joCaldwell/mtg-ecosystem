//! CR 109.5/603.3a bind "your" to the trigger controller; CR 400.7e/603.6c
//! let the effect find the public successor created by that exact move.
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;
use serde_json::{Value, json};
use std::num::NonZeroU32;

fn unsupported_shape() -> Failure {
    unsupported(
        "trigger.successor",
        "requires the complete controller graveyard-to-library successor program",
    )
}

pub(crate) fn lower(
    effects: &[Sentence],
    intervening: Option<&SourceCondition>,
    from: &str,
    to: &Value,
    block: u32,
) -> Result<TriggeredAbility, Failure> {
    let [
        Sentence {
            bindings,
            steps,
            condition: None,
            condition_kind: None,
        },
    ] = effects
    else {
        return Err(unsupported_shape());
    };
    let [
        Step::MoveZone {
            what,
            to: library,
            order: None,
            tapped: None,
        },
    ] = steps.as_slice()
    else {
        return Err(unsupported_shape());
    };
    let depth = library["depth"]
        .as_u64()
        .and_then(|n| u32::try_from(n).ok())
        .and_then(NonZeroU32::new)
        .ok_or_else(unsupported_shape)?;
    if intervening.is_some()
        || !bindings.is_empty()
        || from != "battlefield"
        || *to != graveyard()
        || *what != json!({"ref":"it"})
        || *library != destination(depth)
    {
        return Err(unsupported_shape());
    }
    Ok(TriggeredAbility {
        death_filter: None,
        functioning_zone: FunctioningZone::Battlefield,
        condition: TriggerCondition::SelfMoved {
            from: SourceMoveOrigin::Battlefield,
            to: TriggerSuccessorZone::TriggerControllerGraveyard,
        },
        intervening_if: None,
        program: SpellProgram {
            optional_additional_costs: vec![],
            additional_sacrifice: None,
            timing: SpellTiming::Instant,
            resolution: SpellResolution::Instructions,
            functioning_zone: FunctioningZone::Stack,
            targets: vec![],
            instructions: vec![Instruction::MoveTriggeredSuccessor {
                expected_zone: TriggerSuccessorZone::TriggerControllerGraveyard,
                destination: TriggerSuccessorZone::TriggerControllerLibrary,
                position: LibraryPosition::NthFromTop(depth),
                source_block: block,
            }],
            modes: vec![],
        },
    })
}

fn graveyard() -> Value {
    json!({"zone":"graveyard","owner":"your"})
}

fn destination(depth: NonZeroU32) -> Value {
    json!({"zone":"library","owner":"your","position":"top","depth":depth})
}

pub(crate) fn bind(
    abilities: &[Ability],
    binding: &Value,
    card: &Source,
    parsed: &ParseResult,
) -> Result<(), Failure> {
    let self_move = |a: &Ability| {
        matches!(a, Ability::Triggered { trigger, .. }
            if matches!(trigger.as_ref(), SourceTrigger::PutIntoGraveyard {
                what: TriggerObject::This { noun }, ..
            } if noun == "creature"))
    };
    if !abilities.iter().any(self_move) {
        return Ok(());
    }
    let bad = || {
        unsupported(
            "trigger.successor.binding",
            "incomplete successor tuck source or reference closure",
        )
    };
    let [
        _,
        Ability::Triggered {
            trigger,
            effects,
            condition,
            ..
        },
        _,
    ] = abilities
    else {
        return Err(bad());
    };
    let SourceTrigger::PutIntoGraveyard { from, to, .. } = trigger.as_ref() else {
        return Err(bad());
    };
    let ability = lower(effects, condition.as_ref(), from, to, 1)?;
    let Instruction::MoveTriggeredSuccessor {
        position: LibraryPosition::NthFromTop(depth),
        ..
    } = ability.program.instructions[0]
    else {
        return Err(bad());
    };
    let raw: Value = decode(parsed.abilities.get(), "abilities")?;
    let subject = json!({"ref":"this","noun":"creature"});
    let pronoun = json!({"ref":"it"});
    if raw
        != json!([
            {"kind":"keywords","keywords":[{"keyword":"flying"}]},
            {"kind":"triggered","trigger":{"trigger":"put-into-graveyard","what":subject,"from":"battlefield","to":graveyard()},
                "effects":[{"steps":[{"effect":"move-zone","what":pronoun,"to":destination(depth)}]}]},
            {"kind":"keywords","keywords":[{"keyword":"cascade"}]}
        ])
    {
        return Err(bad());
    }
    // Match the archived parser's ordinal words for this grammar. The executable
    // instruction accepts every positive u32 depth, including beyond library size.
    const ORDINALS: [&str; 9] = [
        "second", "third", "fourth", "fifth", "sixth", "seventh", "eighth", "ninth", "tenth",
    ];
    let ordinal = depth
        .get()
        .checked_sub(2)
        .and_then(|i| ORDINALS.get(i as usize))
        .ok_or_else(bad)?;
    let sentence = format!(
        "When this creature is put into your graveyard from the battlefield, put it into your library {ordinal} from the top."
    );
    if parsed
        .lines
        .iter()
        .map(|line| normalization::literal_spelling(&line.text))
        .ne(["Flying", sentence.as_str(), "Cascade"].map(str::to_owned))
    {
        return Err(bad());
    }

    let identity = &card.oracle_id;
    let root = format!("card:{identity}");
    let source = format!("{root}/source#object");
    let face = format!("{root}/faces/0#face");
    let mut nodes = vec![
        json!({"id":source,"path":"/source","node":"source","identity":identity,"rules":["201.5"]}),
        json!({"id":face,"path":"/faces/0","node":"face","source":source,"sourceFaceIndex":0,
            "name":card.name,"layout":card.layout,"typeLine":card.type_line,"manaCost":card.mana_cost,"rules":["712"]}),
    ];
    let mut references = vec![];
    for (block, kind, role) in [
        (0, "keywords", "static-source-controller"),
        (1, "triggered", "trigger-controller"),
        (2, "keywords", "static-source-controller"),
    ] {
        let path = format!("/faces/0/abilities/{block}");
        let scope = format!("{root}{path}#ability");
        let you = format!("{root}{path}#you");
        nodes.push(
            json!({"id":scope,"path":path,"node":"ability","source":source,"face":face,
            "abilityKind":kind,"you":you,"rules":["113","109.5"]}),
        );
        nodes.push(json!({"id":you,"path":path,"node":"runtime-player","scope":scope,"role":role,"rules":["109.5"]}));
        if kind == "keywords" {
            let (keyword, role, ability, rule) = if block == 0 {
                ("flying", "static-source-controller", "static", "702.9")
            } else {
                ("cascade", "trigger-controller", "triggered", "702.85")
            };
            let path = format!("{path}/keywords/0");
            let you = format!("{root}{path}#you");
            nodes.push(json!({"id":you,"path":path,"node":"runtime-player","scope":scope,"role":role,"rules":["109.5",rule]}));
            nodes.push(
                json!({"id":format!("{root}{path}#keyword"),"path":path,"node":"keyword-ability",
                "scope":scope,"keyword":keyword,"abilities":[ability],"you":you,"rules":[rule]}),
            );
            continue;
        }
        let trigger = format!("{path}/trigger");
        let event = format!("{root}{trigger}#event");
        let successor = format!("{root}{trigger}#successor");
        let effect = format!("{path}/effects/0/steps/0");
        nodes.extend([
            json!({"id":event,"path":trigger,"node":"event","scope":scope,"operation":"trigger:put-into-graveyard",
                "inputs":[source],"availability":"when-performed","rules":["608.2c"]}),
            json!({"id":format!("{root}{trigger}#objects"),"path":trigger,"node":"event-result","event":event,
                "result":"objects","cardinality":"one","availability":"if-produced","identity":"same-objects",
                "description":{"types":["creature"]},"rules":["608.2h"]}),
            json!({"id":successor,"path":trigger,"node":"event-result","event":event,
                "result":"objects","cardinality":"one","availability":"if-produced","identity":"zone-successors",
                "description":{"types":["creature"],"cls":"card"},"rules":["400.7e","603.6c"]}),
            json!({"id":format!("{root}{effect}#event"),"path":effect,"node":"event","scope":scope,"operation":"move-zone",
                "inputs":[successor],"availability":"when-performed","rules":["608.2c"]}),
        ]);
        references.extend([
            json!({"path":format!("{trigger}/what"),"expression":subject,"binding":"bound","target":source,"rules":["201.5"]}),
            json!({"path":format!("{trigger}/to/owner"),"expression":{"player":"you"},"binding":"bound","target":you,"rules":["109.5"]}),
            json!({"path":format!("{effect}/who"),"expression":{"player":"you"},"binding":"bound","target":you,"rules":["109.5"]}),
            json!({"path":format!("{effect}/what"),"expression":pronoun,"binding":"bound","target":successor,"rules":["608.2k","400.7j","608.2h"]}),
            json!({"path":format!("{effect}/to/owner"),"expression":{"player":"you"},"binding":"bound","target":you,"rules":["109.5"]}),
        ]);
    }
    if *binding
        != json!({
            "artifactKind":"oracle-reference-binding","schemaVersion":1,"executable":false,"identity":identity,
            "rules":{"edition":RULES_EDITION,"url":"https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt"},
            "nodes":nodes,"references":references,"diagnostics":[],
            "counts":{"obligations":5,"bound":5,"unresolved":0},"referencesBound":true,
            "engineFeatures":[],"fullyBound":true
        })
    {
        return Err(bad());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    const HEADER: &str =
        include_str!("../../../../tests/fixtures/ramos/archive-manifest-current.json");
    const SOURCE: &str =
        include_str!("../../../../tests/fixtures/ramos/6fb84ecd-ab60-4c2c-987d-8a0749db5777.json");

    #[test]
    fn successor_tuck_complete_frozen_source_and_binding() {
        assert_eq!(
            sha256(SOURCE.as_bytes()),
            "f1cdfe32dade7027face42850fb574fcb9d5c3fbcb825825345697a2a18bed05"
        );
        let CompileResult::Lowered { definition } = compile(HEADER, SOURCE) else {
            panic!("{:?}", compile(HEADER, SOURCE));
        };
        assert_eq!(definition.keywords, [KeywordAbility::Flying]);
        assert_eq!(definition.triggered.len(), 2);
        assert_eq!(
            definition.triggered[1].program,
            canonical_cascade_program(2)
        );
        assert_eq!(
            definition.triggered[0].program.instructions,
            [Instruction::MoveTriggeredSuccessor {
                expected_zone: TriggerSuccessorZone::TriggerControllerGraveyard,
                destination: TriggerSuccessorZone::TriggerControllerLibrary,
                position: LibraryPosition::NthFromTop(NonZeroU32::new(3).unwrap()),
                source_block: 1,
            }]
        );
        assert!(
            definition
                .capabilities
                .contains(&Capability::TriggeredSuccessorLibraryTuckV1)
        );
        assert!(definition.characteristics.has_type(CardKind::Artifact));
        assert_eq!(
            definition.characteristics.mana_cost,
            ManaCost {
                generic: 4.into(),
                colored: vec![Color::W, Color::U, Color::B],
            }
        );

        let bad = crate::tests::changed_source(SOURCE, |record| {
            let reference = &mut record["binding"]["references"][3];
            reference["target"] = reference["target"]
                .as_str()
                .unwrap()
                .replace("#successor", "#objects")
                .into();
        });
        assert!(matches!(
            compile(HEADER, &bad),
            CompileResult::Unsupported { .. }
        ));

        let second = crate::tests::changed_source(SOURCE, |record| {
            let text = &mut record["source"]["oracle_text"];
            *text = text.as_str().unwrap().replace("third", "second").into();
            let result = &mut record["faces"][0]["result"];
            result["abilities"][1]["effects"][0]["steps"][0]["to"]["depth"] = 2.into();
            result["lines"][1]["ability"] = result["abilities"][1].clone();
            let text = &mut result["lines"][1]["text"];
            *text = text.as_str().unwrap().replace("third", "second").into();
        });
        let CompileResult::Lowered { definition } = compile(HEADER, &second) else {
            panic!("{:?}", compile(HEADER, &second));
        };
        assert!(matches!(
            definition.triggered[0].program.instructions.as_slice(),
            [Instruction::MoveTriggeredSuccessor { position: LibraryPosition::NthFromTop(depth), .. }]
                if depth.get() == 2
        ));
    }
}
