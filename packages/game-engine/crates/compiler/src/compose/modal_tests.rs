//! Modal spell lowering from frozen R5 source: each legal selection is an
//! ordinary program in printed order, and every departure from the printed
//! declaration rejects by name (CR 601.2b, 700.2a-d, 608.2c).
use super::*;
use serde_json::json;

const R5: &str = include_str!("../../../../tests/fixtures/archive-manifest-phase1-r5.json");
const GRIXIS_CHARM: &str =
    include_str!("../../../../tests/fixtures/phase1-r5/387ae7be-1cb5-4ae8-bb63-edf4941945a9.json");
const AUSTERE_COMMAND: &str =
    include_str!("../../../../tests/fixtures/phase1-r5/09cc8709-fe10-472a-b05c-e89f3523018d.json");
const FAREWELL: &str =
    include_str!("../../../../tests/fixtures/phase1-r5/4eb813fd-2d5a-4b02-8193-662681ef4e7d.json");

fn lowered(raw: &str) -> Definition {
    match compile(R5, raw) {
        CompileResult::Lowered { definition } => *definition,
        other => panic!("frozen modal source must lower: {other:?}"),
    }
}

fn reason(raw: &str) -> String {
    match compile(R5, raw) {
        CompileResult::Unsupported { reason, .. } | CompileResult::Invalid { reason, .. } => reason,
        CompileResult::Lowered { .. } => panic!("changed modal source lowered"),
    }
}

/// Re-hash a changed archive row, keeping each line's AST equal to the aggregate.
fn changed(raw: &str, change: impl FnOnce(&mut serde_json::Value)) -> String {
    crate::tests::changed_source(raw, |row| {
        change(row);
        let result = &mut row["faces"][0]["result"];
        let abilities = result["abilities"].as_array().unwrap().clone();
        for (i, ability) in abilities.into_iter().enumerate() {
            result["lines"][i]["ability"] = ability;
        }
    })
}

#[test]
fn modal_spells_lower_each_legal_selection_in_printed_order() {
    let grixis = lowered(GRIXIS_CHARM);
    grixis.validate().unwrap();
    let modal = grixis.modal_spell.as_ref().unwrap();
    let base = grixis.spell.as_ref().unwrap();
    assert!(base.targets.is_empty() && base.instructions.is_empty());
    assert_eq!((modal.min, modal.max), (1, 1));
    assert_eq!(
        modal.labels,
        [
            "Return target permanent to its owner's hand.",
            "Target creature gets -4/-4 until end of turn.",
            "Creatures you control get +2/+0 until end of turn.",
        ]
    );
    assert_eq!(
        modal.selections.keys().copied().collect::<Vec<_>>(),
        [1, 2, 4]
    );
    // Each selection announces only its own target, in slot 0 (CR 700.2c).
    let targets = |mask| modal.selections[&mask].targets.len();
    assert_eq!([targets(1), targets(2), targets(4)], [1, 1, 0]);
    assert!(grixis.requirements().contains(&Capability::ModalChoiceV1));

    // "Choose two": each pair of distinct modes once (CR 700.2d), its
    // instructions the two modes' in printed order (CR 608.2c).
    let austere = lowered(AUSTERE_COMMAND);
    let modal = austere.modal_spell.as_ref().unwrap();
    assert_eq!(
        modal.selections.keys().copied().collect::<Vec<_>>(),
        [3, 5, 6, 9, 10, 12]
    );
    let ordered = |mask: u8| {
        modal.selections[&mask]
            .instructions
            .iter()
            .map(|i| serde_json::to_string(i).unwrap())
            .collect::<Vec<_>>()
    };
    let farewell = lowered(FAREWELL);
    let all = farewell.modal_spell.as_ref().unwrap();
    assert_eq!((all.min, all.max), (1, 4));
    assert_eq!(all.selections.len(), 15);
    // Mode 0 then mode 2 is mode 0's text followed by mode 2's.
    let (first, third) = (ordered(3)[..1].to_vec(), ordered(12)[..1].to_vec());
    assert_eq!(ordered(5), [first, third].concat());
}

#[test]
fn modal_declarations_that_disagree_with_print_reject_by_name() {
    let options = |row: &mut serde_json::Value| -> serde_json::Value {
        row["faces"][0]["result"]["abilities"][0]["effects"][0]["steps"][0].clone()
    };
    // The AST count must be the printed "Choose one".
    let recount = changed(GRIXIS_CHARM, |row| {
        let mut step = options(row);
        step["count"] = json!({"exactly": 2});
        row["faces"][0]["result"]["abilities"][0]["effects"][0]["steps"][0] = step;
    });
    assert_eq!(reason(&recount), "verb:modal:count_disagrees_with_print");
    // Every printed bullet is one AST option; the resolution already binds
    // each bullet's references, so a dropped option never reaches lowering.
    let dropped = changed(GRIXIS_CHARM, |row| {
        let mut step = options(row);
        step["options"].as_array_mut().unwrap().pop();
        row["faces"][0]["result"]["abilities"][0]["effects"][0]["steps"][0] = step;
    });
    assert!(reason(&dropped).contains("does not cover the AST"));
    // A declaration term the lowering does not consume: repeated modes
    // (CR 700.2d) are a different announcement.
    let extra = changed(GRIXIS_CHARM, |row| {
        let mut step = options(row);
        step["allowRepeat"] = json!(true);
        row["faces"][0]["result"]["abilities"][0]["effects"][0]["steps"][0] = step;
    });
    assert_eq!(reason(&extra), "verb:modal:declaration");
}

#[test]
fn modal_executable_shape_is_closed() {
    let grixis = lowered(GRIXIS_CHARM);
    let rejects = |change: &dyn Fn(&mut Definition)| {
        let mut d = grixis.clone();
        change(&mut d);
        assert!(d.validate().is_err());
    };
    // Only instants and sorceries announce modes here.
    rejects(&|d| d.characteristics.kind = CardKind::Creature);
    // The base program carries no effects of its own.
    rejects(&|d| {
        let first = d.modal_spell.as_ref().unwrap().selections[&1].clone();
        d.spell = Some(first);
    });
    // The selections are exactly the legal counts.
    rejects(&|d| {
        d.modal_spell.as_mut().unwrap().selections.remove(&4);
    });
    rejects(&|d| d.modal_spell.as_mut().unwrap().max = 2);
    // One selection announces at most one target.
    rejects(&|d| {
        let modal = d.modal_spell.as_mut().unwrap();
        let extra = modal.selections[&1].targets[0].clone();
        modal.selections.get_mut(&2).unwrap().targets.push(extra);
    });
}

const TRUFFLESNOUT: &str =
    include_str!("../../../../tests/fixtures/phase1-r5/8a15841a-dcb6-454a-b865-43a067f42b4e.json");
const COLISEUM_BEHEMOTH: &str =
    include_str!("../../../../tests/fixtures/phase1-r5/076cf4f1-b3d4-4175-b976-938ad851cc0e.json");
const KNIGHT_OF_AUTUMN: &str =
    include_str!("../../../../tests/fixtures/phase1-r5/c4212ba4-8c43-40ca-aa36-b3b535659fa8.json");
const GOBLIN_CRATERMAKER: &str =
    include_str!("../../../../tests/fixtures/phase1-r5/1962d9b3-f057-4d2c-9c5b-386d9a66148a.json");

/// CR 603.3c, 700.2: a modal trigger keeps its event and timing on an empty
/// base program; each selection is its modes' complete programs in order.
#[test]
fn modal_triggers_lower_each_selection_beside_their_event() {
    let behemoth = lowered(COLISEUM_BEHEMOTH);
    behemoth.validate().unwrap();
    assert!(behemoth.modal_spell.is_none());
    let [trigger] = behemoth.triggered.as_slice() else {
        panic!("one trigger");
    };
    assert_eq!(
        trigger.condition,
        TriggerCondition::on(kir::EventPattern::self_enters())
    );
    assert!(trigger.program.targets.is_empty() && trigger.program.instructions.is_empty());
    let modal = trigger.modal.as_ref().unwrap();
    assert_eq!(
        modal.labels,
        ["Destroy target artifact or enchantment.", "Draw a card."]
    );
    let targets = |mask| modal.selections[&mask].targets.len();
    assert_eq!(modal.selections.keys().copied().collect::<Vec<_>>(), [1, 2]);
    assert_eq!([targets(1), targets(2)], [1, 0]);
    assert!(behemoth.requirements().contains(&Capability::ModalChoiceV1));
    assert_eq!(
        lowered(KNIGHT_OF_AUTUMN).triggered[0]
            .modal
            .as_ref()
            .unwrap()
            .selections
            .keys()
            .copied()
            .collect::<Vec<_>>(),
        [1, 2, 4]
    );
    // The printed count binds the AST for an ability as for a spell.
    let recount = changed(TRUFFLESNOUT, |row| {
        row["faces"][0]["result"]["abilities"][0]["effects"][0]["steps"][0]["count"] =
            json!({"exactly": 2});
    });
    assert_eq!(reason(&recount), "verb:modal:count_disagrees_with_print");
}

/// CR 602.2b: a modal activation lowers each mode as a complete activation
/// of the same cost; Goblin Cratermaker reaches its second mode's colorless
/// filter, which is outside the operand tables, instead of an unlowered
/// modal step.
#[test]
fn modal_activations_lower_each_mode_through_the_activation_lowerer() {
    assert_eq!(
        reason(GOBLIN_CRATERMAKER),
        "operand:target_filter:colorless"
    );
}

#[test]
fn modal_ability_executable_shape_is_closed() {
    let behemoth = lowered(COLISEUM_BEHEMOTH);
    let rejects = |change: &dyn Fn(&mut Definition)| {
        let mut d = behemoth.clone();
        change(&mut d);
        assert!(d.validate().is_err());
    };
    // The base program carries no effects of its own.
    rejects(&|d| {
        let first = d.triggered[0].modal.as_ref().unwrap().selections[&2].clone();
        d.triggered[0].program = first;
    });
    // The selections are exactly the legal counts.
    rejects(&|d| {
        d.triggered[0].modal.as_mut().unwrap().selections.remove(&2);
    });
    // A selection may not leave the declaring block.
    rejects(&|d| d.triggered[0].modal.as_mut().unwrap().source_block += 1);
    // As an activation, only a battlefield ability whose cost the targeted
    // announcement route pays.
    let activated = |cost: ActivationCost, zone: FunctioningZone| {
        move |d: &mut Definition| {
            let trigger = d.triggered.remove(0);
            d.activated.push(ActivatedAbility {
                functioning_zone: zone,
                x_mana_cost: false,
                hybrid_mana: vec![],
                restriction: None,
                cost: cost.clone(),
                program: trigger.program,
                modal: trigger.modal,
            });
            if let Some(labels) = &mut d.source_labels {
                labels.activated = std::mem::take(&mut labels.triggered);
            }
            d.capabilities = d.requirements();
        }
    };
    let mana = ActivationCost::Mana {
        cost: ManaCost {
            generic: Quantity::from(1),
            colored: vec![],
        },
    };
    let mut admitted = behemoth.clone();
    activated(mana.clone(), FunctioningZone::Battlefield)(&mut admitted);
    admitted.validate().unwrap();
    rejects(&activated(mana, FunctioningZone::Hand));
    rejects(&activated(
        ActivationCost::TapPayLifeSacrificeSource {
            life: Quantity::from(1),
        },
        FunctioningZone::Battlefield,
    ));
}

/// CR 605.1a: a modal activation with a mode that adds mana would be a mana
/// ability, which never uses the stack; it rejects by name instead of
/// lowering as a modal stack activation. No frozen R5 record has one, and a
/// changed record fails the archive's span coverage first, so the guard the
/// activated block runs on its effects is checked on Goblin Cratermaker's
/// frozen effects with one mode replaced.
#[test]
fn a_modal_activation_that_adds_mana_rejects_by_name() {
    let row: serde_json::Value = serde_json::from_str(GOBLIN_CRATERMAKER).unwrap();
    let ability = row["faces"][0]["result"]["abilities"]
        .as_array()
        .unwrap()
        .iter()
        .find(|a| a["kind"] == "activated")
        .unwrap()
        .clone();
    let effects = ability["effects"].as_array().unwrap().clone();
    assert!(modal::declared(&effects));
    modal::reject_mana_modes(&effects).unwrap();
    let mut mana = effects;
    mana[0]["steps"][0]["options"][1]["effects"] = json!([{"steps": [{
        "effect": "add-mana",
        "who": {"player": "you"},
        "mana": {"mana": "fixed", "symbols": ["C", "C"]}
    }]}]);
    assert_eq!(
        modal::reject_mana_modes(&mana).unwrap_err().reason,
        "verb:modal:mana_ability"
    );
}
