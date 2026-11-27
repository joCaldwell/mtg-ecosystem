//! Pronoun antecedents on exact frozen R5 rows, lowered through the
//! composition entry points without the archive's reference resolution, so
//! the Rust rule must be right on its own. Each case has a neighbour with
//! the opposite outcome. CR 110.1, 201.5, 400.7, 608.2b-c, 608.2h, 700.2,
//! 700.7.
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;
use crate::reference_trace::{Bound, Resolved, traced};

macro_rules! record {
    ($id:literal) => {
        include_str!(concat!(
            "../../../../tests/fixtures/phase1-r5/",
            $id,
            ".json"
        ))
    };
}

const FABLED_HERO: &str = record!("f9fd9b1c-4c00-4db7-bbfe-7fd5d8bb9258");
const BLOODCRAZED_HOPLITE: &str = record!("b1f441ce-7619-4ab5-ab36-724ed0a76728");
const LAGONNA_BAND_TRAILBLAZER: &str = record!("09cc0faa-71d7-428b-9fc4-5ba09e244e89");
const INFERNAL_PET: &str = record!("831b7dad-ab5a-4980-a52d-65c5633d474a");
const FEARLESS_FLEDGLING: &str = record!("75ae9b87-7137-47a0-b3e1-8915ccd0edf1");
const ADAPTIVE_TRAINING_POST: &str = record!("a6657fcf-f08c-4b03-8ec8-cb0b194eb553");
const KAERVEK: &str = record!("c7b72d38-0aa0-4e17-9dd5-9276d7cb21ec");
const RIVETEERS_OVERLOOK: &str = record!("5548ff43-e5f6-4a63-8562-a2b1de06d6f5");
const BOGARDAN_PHOENIX: &str = record!("72586a7a-3c73-4012-8664-1db0f268eb60");
const GILDED_SCUTTLER: &str = record!("093f6123-6b1d-456f-a526-8740bc969b4e");
const SYGGS_COMMAND: &str = record!("7fe5baab-b182-4cd9-bea5-912ce9b1464a");
const BRIGHTMARE: &str = record!("32025108-0dba-40bd-88db-5fcc48e3d054");
const R5: &str = include_str!("../../../../tests/fixtures/archive-manifest-phase1-r5.json");

fn printed(kind: CardKind) -> Card<'static> {
    Card {
        kind,
        additional_types: &[],
        subtypes: &[],
    }
}

fn ability(raw: &str, index: usize) -> serde_json::Value {
    let row: serde_json::Value = serde_json::from_str(raw).unwrap();
    row["faces"][0]["result"]["abilities"][index].clone()
}

fn code(failure: &Failure) -> String {
    crate::coverage::reason_code(&failure.path, &failure.reason, &serde_json::Value::Null)
}

/// Lower one frozen triggered block of a creature card, recording every
/// pronoun binding.
fn lower_trigger_block(
    raw: &str,
    index: usize,
    effects: Option<Vec<serde_json::Value>>,
) -> (Result<TriggeredAbility, Failure>, Vec<Bound>) {
    lower_printed_block(CardKind::Creature, raw, index, effects)
}

/// Lower one frozen triggered block with `effects` (its own sentences unless
/// given), recording every pronoun binding.
fn lower_printed_block(
    kind: CardKind,
    raw: &str,
    index: usize,
    effects: Option<Vec<serde_json::Value>>,
) -> (Result<TriggeredAbility, Failure>, Vec<Bound>) {
    let block = ability(raw, index);
    assert_eq!(block["kind"], "triggered");
    let effects = effects.unwrap_or_else(|| block["effects"].as_array().unwrap().clone());
    traced(|| {
        trigger(
            &printed(kind),
            &block["trigger"],
            parts(&effects)?,
            block.get("condition"),
            index,
        )
    })
}

fn bound(index: u32, resolved: Resolved) -> Bound {
    Bound {
        block: index,
        word: "it",
        resolved,
    }
}

/// "Whenever you cast a spell that targets this creature, put a +1/+1
/// counter on it": "it" is this creature (CR 201.5), never the spell on the
/// stack, which is not a permanent (CR 110.1). The trigger condition names
/// the source and the counter step acts on a permanent.
#[test]
fn heroic_it_names_the_targeted_source_not_the_cast_spell() {
    for (raw, index) in [
        (FABLED_HERO, 1),
        (BLOODCRAZED_HOPLITE, 0),
        (LAGONNA_BAND_TRAILBLAZER, 0),
    ] {
        let (lowered, trace) = lower_trigger_block(raw, index, None);
        let ability = lowered.unwrap();
        assert!(matches!(
            ability.condition.pattern(),
            EventPattern::Cast {
                targets_source: true,
                ..
            }
        ));
        assert_eq!(trace, [bound(index as u32, Resolved::Source)]);
        assert_eq!(
            ability.program.instructions[0].verb(),
            Some(&Verb::PutCounters {
                on: SetExpr::source(),
                counter: kir::CounterKind::PlusOnePlusOne,
                amount: Amount::fixed(1),
            })
        );
    }
}

/// "Put a +1/+1 counter on this creature and it gains flying" (Infernal
/// Pet, on a cast) and "put a +1/+1 counter on this creature. It gains
/// flying" (Fearless Fledgling, on a land entering): the earlier step names
/// the source after the trigger's event subject, so "it" is this creature,
/// not the spell or the land. The gain-abilities verb does not act on a
/// non-target pronoun yet, so both still reject by name.
#[test]
fn it_after_a_step_naming_the_source_binds_the_source() {
    for (raw, index) in [(INFERNAL_PET, 0), (FEARLESS_FLEDGLING, 0)] {
        let (lowered, trace) = lower_trigger_block(raw, index, None);
        assert_eq!(code(&lowered.unwrap_err()), "operand:target:ref_it");
        assert_eq!(trace, [bound(index as u32, Resolved::Source)]);
    }
}

/// Neighbours: "if this artifact has fewer than three charge counters on
/// it, put a charge counter on it" names the artifact only in its
/// intervening condition, so the counter step has no permanent antecedent
/// and rejects; it never acts on the cast spell. A read of "that spell"
/// still means the cast spell (Kaervek, "damage equal to that spell's mana
/// value").
#[test]
fn a_cast_spell_is_read_but_never_acted_on() {
    let (lowered, trace) = lower_trigger_block(ADAPTIVE_TRAINING_POST, 0, None);
    assert_eq!(
        code(&lowered.unwrap_err()),
        "operand:object:unbound_antecedent"
    );
    assert!(trace.is_empty(), "{trace:?}");
    let (lowered, trace) = lower_trigger_block(KAERVEK, 0, None);
    lowered.unwrap();
    assert_eq!(
        trace,
        [Bound {
            block: 0,
            word: "that",
            resolved: Resolved::EventObject {
                role: "CastSpell".into(),
                is_source: false,
            },
        }]
    );
}

/// CR 400.7, 701.21a: Riveteers Overlook's "when this land enters,
/// sacrifice it" moves the entering land, the trigger's event object, to the
/// graveyard. A later step that acts on "it" would mean the card there, a
/// new object the event no longer names, so it rejects. No R5 card prints
/// that sequence, so the case joins the Overlook's frozen sacrifice sentence
/// with Bogardan Phoenix's frozen "put a death counter on it"; the
/// Phoenix's own "return it to the battlefield" is not lowered at all.
#[test]
fn acting_on_an_event_object_after_moving_it_rejects() {
    let overlook = ability(RIVETEERS_OVERLOOK, 0);
    let phoenix = ability(BOGARDAN_PHOENIX, 1);
    let otherwise = &phoenix["effects"][0]["otherwise"]["steps"];
    assert_eq!(otherwise[1]["effect"], "put-counters");
    let counter = serde_json::json!({"steps": [otherwise[1]]});
    let sacrifice = overlook["effects"][0].clone();
    let lower = |effects| lower_printed_block(CardKind::Land, RIVETEERS_OVERLOOK, 0, Some(effects));
    let (lowered, trace) = lower(vec![sacrifice.clone(), counter]);
    assert_eq!(
        code(&lowered.unwrap_err()),
        "operand:object:pronoun:moved_object"
    );
    let event = Resolved::EventObject {
        role: "Entered".into(),
        is_source: true,
    };
    assert_eq!(trace, [bound(0, event.clone())]);
    // Neighbour: the sacrifice alone acts on the event object.
    let (lowered, trace) = lower(vec![sacrifice]);
    lowered.unwrap();
    assert_eq!(trace, [bound(0, event)]);
    // The Phoenix's frozen sentence stops at its unsupported return.
    let (lowered, _) = lower_trigger_block(
        BOGARDAN_PHOENIX,
        1,
        Some(vec![serde_json::json!({"steps": otherwise})]),
    );
    assert_eq!(code(&lowered.unwrap_err()), "operand:target:ref_it");
}

/// Neighbour: "tap target creature an opponent controls and put a stun
/// counter on it" acts on the target nothing moved.
#[test]
fn acting_on_an_unmoved_target_binds_it() {
    let (lowered, trace) = lower_trigger_block(GILDED_SCUTTLER, 1, None);
    let ability = lowered.unwrap();
    assert_eq!(
        trace,
        [bound(
            1,
            Resolved::Target {
                selection: "line:1:6".into()
            }
        )]
    );
    assert_eq!(
        ability.program.instructions[1].verb(),
        Some(&Verb::PutCounters {
            on: SetExpr::target(0),
            counter: kir::CounterKind::Stun,
            amount: Amount::fixed(1),
        })
    );
}

/// CR 700.2: Sygg's Command's "Tap target creature. Put a stun counter on
/// it." is its own mode. Chosen with "create a token that's a copy of target
/// Merfolk you control", the shared program already holds the Merfolk, but
/// "it" still means the creature this mode tapped.
#[test]
fn a_later_modes_pronoun_sees_only_its_own_targets() {
    let block = ability(SYGGS_COMMAND, 0);
    let options = block["effects"][0]["steps"][0]["options"]
        .as_array()
        .unwrap()
        .clone();
    let option = |i: usize| options[i]["effects"].as_array().unwrap().clone();
    let mut selection = SpellProgram::instructions(SpellTiming::Instant);
    // The first mode's announced Merfolk, as the combined selection holds
    // it (its copy-token step has no lowering of its own).
    bind_target(
        &mut selection,
        &mut BTreeMap::new(),
        "line:1:8",
        TargetDomain::BattlefieldPermanent,
    )
    .unwrap();
    let (lowered, trace) = traced(|| modal::append_mode(&mut selection, &option(3), 0));
    lowered.unwrap();
    assert_eq!(selection.targets.len(), 2);
    assert_eq!(
        trace,
        [bound(
            0,
            Resolved::Target {
                selection: selection.targets[1].selection_id.clone()
            }
        )]
    );
    assert_eq!(
        selection.instructions.last().and_then(Instruction::verb),
        Some(&Verb::PutCounters {
            on: SetExpr::target(1),
            counter: kir::CounterKind::Stun,
            amount: Amount::fixed(1),
        })
    );
    // Neighbour: a mode with no target of its own cannot reach the earlier
    // mode's creature, so its pronoun is unbound.
    let mut selection = SpellProgram::instructions(SpellTiming::Instant);
    bind_target(
        &mut selection,
        &mut BTreeMap::new(),
        "line:1:8",
        TargetDomain::BattlefieldPermanent,
    )
    .unwrap();
    let stun = vec![option(3)[1].clone()];
    assert_eq!(
        code(&modal::append_mode(&mut selection, &stun, 0).unwrap_err()),
        "operand:object:unbound_antecedent"
    );
}

/// CR 608.2b, 608.2h: Brightmare's "you gain life equal to that creature's
/// power" reads its up-to-one target directly, so the verb skips when no
/// target was chosen. Inside a sum or a product the other terms still have
/// values ("gain 2 life plus that creature's power"), so the tier rejects
/// rather than gain nothing.
#[test]
fn up_to_one_target_life_reads_admit_only_the_target_read_itself() {
    use mtg_contracts::kir::{ExecutionTier, definition_tiers};
    let CompileResult::Lowered { definition } = compile(R5, BRIGHTMARE) else {
        panic!("Brightmare must lower");
    };
    assert!(
        definition_tiers(&definition)
            .unwrap()
            .admits(ExecutionTier::Executes)
    );
    let read = Amount::StatOf {
        object: Ref::Target { slot: 0 },
        stat: kir::Stat::Power,
    };
    for amount in [
        Amount::Sum {
            terms: vec![Amount::fixed(2), read.clone()],
        },
        Amount::Times {
            factor: 2.into(),
            of: Box::new(read.clone()),
        },
    ] {
        let mut changed = definition.clone();
        let program = &mut changed.triggered[0].program;
        let Some(Instruction::Kernel {
            node: kir::Node::Do(Verb::GainLife { amount: slot, .. }),
            ..
        }) = program.instructions.get_mut(1)
        else {
            panic!("unexpected Brightmare program {program:?}");
        };
        assert_eq!(*slot, read);
        *slot = amount;
        assert_eq!(
            definition_tiers(&changed).unwrap_err(),
            kir::KirError::Unaudited(
                "cell:target:up_to_one_requires_independent_object_leaves".into()
            )
        );
    }
}

/// CR 113.10, 700.7: "this creature" inside a quoted granted ability names
/// the object that has the ability, not the granting block's source, so a
/// later bare "it" cannot mean the source through it. The same reference
/// outside the quotes still names the source.
#[test]
fn a_quoted_abilitys_this_does_not_name_the_source() {
    let this = serde_json::json!({"ref": "this", "noun": "creature"});
    let quoted = serde_json::json!({
        "effect": "gain-abilities",
        "what": {"ref": "target", "filter": {"types": ["creature"]}},
        "abilities": [{"gained": "quoted", "ability": {
            "kind": "activated",
            "effects": [{"steps": [{"effect": "sacrifice", "what": this}]}],
        }}],
        "duration": {"until": "end-of-turn"},
    });
    let introduced = crate::ingest::Introduced::of_step(&quoted);
    assert!(introduced.object_target);
    assert!(!introduced.source);
    // Neighbour: the same grant to this creature itself names the source.
    let mut own = quoted.clone();
    own["what"] = this;
    assert!(crate::ingest::Introduced::of_step(&own).source);
}

/// CR 700.2, 608.2h: "you gain life equal to the life lost this way" reads
/// the life loss of its own mode. In a selection where an earlier chosen
/// mode made opponents lose life, a later mode cannot read that loss, so it
/// rejects; the same two steps in one mode link.
#[test]
fn life_lost_this_way_reads_only_its_own_mode() {
    let lose = serde_json::json!({"steps": [{"effect": "lose-life",
        "who": {"player": "each-opponent"}, "amount": {"amount": "fixed", "value": 2}}]});
    let gain = serde_json::json!({"steps": [{"effect": "gain-life", "who": {"player": "you"},
        "amount": {"amount": "event-quantity", "event": "life-change", "scope": "this-way",
            "change": "lost"}}]});
    let mut selection = SpellProgram::instructions(SpellTiming::Instant);
    modal::append_mode(&mut selection, std::slice::from_ref(&lose), 0).unwrap();
    assert!(
        modal::append_mode(&mut selection, std::slice::from_ref(&gain), 0).is_err_and(|failure| {
            failure
                .reason
                .contains("no dominating life-change projection")
        })
    );
    // Neighbour: one mode prints both steps, so the gain reads the loss.
    let mut selection = SpellProgram::instructions(SpellTiming::Instant);
    modal::append_mode(&mut selection, &[lose, gain], 0).unwrap();
    let [
        Instruction::Kernel {
            node: kir::Node::Seq(nodes),
            ..
        },
    ] = selection.instructions.as_slice()
    else {
        panic!("one linked life change: {:?}", selection.instructions);
    };
    assert!(matches!(
        nodes.as_slice(),
        [
            kir::Node::Do(Verb::LoseLife { .. }),
            kir::Node::Do(Verb::GainLife { .. })
        ]
    ));
}

/// CR 700.2, 608.2c: "draw a card for each card discarded this way" reads
/// the discard of its own mode. An earlier chosen mode's discard in the
/// shared program is neither a competing producer nor a fallback one.
#[test]
fn discarded_this_way_reads_only_its_own_mode() {
    let discard = |who: &str| {
        serde_json::json!({"effect": "discard", "who": {"player": who},
            "what": {"ref": "filter", "filter": {"cls": "card"},
                "count": {"amount": "fixed", "value": 1}}})
    };
    let draw = serde_json::json!({"effect": "draw", "who": {"player": "you"},
        "amount": {"amount": "count", "of": {"cls": "card",
            "relation": {"relation": "event-result", "event": "discard", "scope": "this-way"}}}});
    let earlier = serde_json::json!({"steps": [discard("each-opponent")]});
    let own = serde_json::json!({"steps": [discard("you"), draw.clone()]});
    let mut selection = SpellProgram::instructions(SpellTiming::Instant);
    modal::append_mode(&mut selection, std::slice::from_ref(&earlier), 0).unwrap();
    modal::append_mode(&mut selection, std::slice::from_ref(&own), 0).unwrap();
    let [
        Instruction::Kernel {
            node: kir::Node::Do(opponents),
            ..
        },
        Instruction::Kernel {
            node: kir::Node::Seq(linked),
            ..
        },
    ] = selection.instructions.as_slice()
    else {
        panic!("the later mode links its own discard: {:?}", selection.instructions);
    };
    assert!(opponents.evaluated().is_some());
    assert_eq!(linked.len(), 2);
    // Neighbour: a mode with no discard of its own cannot read the earlier
    // mode's.
    let alone = serde_json::json!({"steps": [draw]});
    let mut selection = SpellProgram::instructions(SpellTiming::Instant);
    modal::append_mode(&mut selection, std::slice::from_ref(&earlier), 0).unwrap();
    assert_eq!(
        code(&modal::append_mode(&mut selection, std::slice::from_ref(&alone), 0).unwrap_err()),
        "operand:result:unbound_event_result"
    );
}
