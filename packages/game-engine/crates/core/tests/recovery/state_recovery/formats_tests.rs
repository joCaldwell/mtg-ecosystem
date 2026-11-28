//! Golden wire fingerprints, one per versioned format in `formats`. A changed
//! fingerprint with an unchanged version constant fails with the constant to
//! bump; after bumping, re-pin the `(version, fingerprint)` pair below.
//!
//! `Observation` derives `JsonSchema`, so its pin covers the complete schema.
//! The private `Snapshot` and `SavedWork` envelopes do not (deriving it across
//! all rules state would add schema code to every core build), so their pins
//! are a structural shape over representative instances (a Foundations pod,
//! and a game paused inside a resolving spell's choice): every object key and
//! JSON type reached, with ID-keyed maps folded. That catches added, removed,
//! renamed or retyped fields on the reached state; payloads of enum variants
//! a representative never reaches are not covered. A third representative
//! pays a spell with a pain land's colored mana ability, reaching a selected
//! printed mana ability, its rider's damage transaction and a kept receipt; a
//! fourth stops a granted mana ability at its color choice; a fifth puts a
//! loyalty ability and its source's prior-row facts on the stack; a sixth
//! fills a held mana receipt's cost event identities and ledger delta; a
//! seventh suspends a counted draw between attempts; an eighth suspends a
//! proliferate on a replacement order, crosses a departure and commits
//! its lineage; a ninth suspends an evaluated move on a commander owner's
//! choice.
use super::*;
use mtg_contracts::{canonical, sha256};
use mtg_core::testkit::formats::*;
use serde_json::Value;
use std::collections::BTreeSet;

const OBSERVATION_PIN: (u32, &str) = (
    116,
    "2176aff885305138cc86859c6e344673e8256004a3eb7ee8147105b4e17e72c0",
);
const SNAPSHOT_PIN: (u32, &str) = (
    150,
    "235978e40b6e00501b2c3d38d2997498f691af62bc1810eb9b440ae15211ed33",
);
const WORK_PIN: (u32, &str) = (
    71,
    "5f071e11dd1a310b17ad5138750df874ff9213dfe6ff6cd4a1efde04714ffd82",
);

fn check(format: &str, constant: &str, version: u32, pin: (u32, &str), actual: &str) {
    if pin == (version, actual) {
        return;
    }
    if version == pin.0 {
        panic!(
            "{format} wire format changed without a version bump: bump {constant} in \
             packages/game-engine/crates/core/src/formats.rs, then pin ({}, {actual:?}) in \
             formats_tests.rs",
            version + 1
        );
    }
    panic!(
        "{format}: {constant} is {version}; re-pin ({version}, {actual:?}) in formats_tests.rs \
         after reviewing the {format} change"
    );
}

/// Shape of one JSON value. Objects whose keys are all identifiers (decimal
/// handles, UUIDs, digests) are maps: their values fold into one shape.
fn shape(value: &Value) -> Value {
    fn identifier(key: &str) -> bool {
        !key.is_empty()
            && (key.bytes().all(|b| b.is_ascii_digit())
                || key.bytes().all(|b| b.is_ascii_hexdigit() || b == b'-') && key.len() >= 32)
    }
    fn union(shapes: impl IntoIterator<Item = Value>) -> Value {
        let set = shapes
            .into_iter()
            .map(|s| canonical(&s).unwrap())
            .collect::<BTreeSet<_>>();
        Value::Array(
            set.into_iter()
                .map(|s| serde_json::from_str(&s).unwrap())
                .collect(),
        )
    }
    match value {
        Value::Null => "null".into(),
        Value::Bool(_) => "bool".into(),
        Value::Number(_) => "number".into(),
        Value::String(_) => "string".into(),
        Value::Array(items) => serde_json::json!({ "array": union(items.iter().map(shape)) }),
        Value::Object(map) if !map.is_empty() && map.keys().all(|k| identifier(k)) => {
            serde_json::json!({ "map": union(map.values().map(shape)) })
        }
        Value::Object(map) => Value::Object(
            map.iter()
                .map(|(k, v)| (k.clone(), shape(v)))
                .collect::<serde_json::Map<_, _>>(),
        ),
    }
}

fn shape_digest(samples: &[String]) -> String {
    let shapes = samples
        .iter()
        .map(|raw| canonical(&shape(&serde_json::from_str(raw).unwrap())).unwrap())
        .collect::<BTreeSet<_>>();
    sha256(canonical(&shapes).unwrap().as_bytes())
}

/// Foundations pod at its opening decision and after one automatic step past it.
fn representatives() -> Vec<Game> {
    let catalog = FixtureCatalog::Match.get().clone();
    let config =
        MatchConfig::for_profile("commander-foundations-v1", [41; 32], PlayerId(2)).unwrap();
    let mut game = Game::new(Prepared::admit_pod(catalog, config).unwrap());
    let mut games = vec![game.fork()];
    for _ in 0..20_000 {
        if !matches!(game.advance(Budget::new(4096)), Advance::Suspended { .. }) {
            break;
        }
    }
    games.push(game);
    games
}

#[test]
fn observation_schema_fingerprint_is_pinned_to_observation_version() {
    let schema = serde_json::to_value(schemars::schema_for!(Observation)).unwrap();
    let actual = sha256(canonical(&schema).unwrap().as_bytes());
    check(
        "Observation",
        "OBSERVATION_VERSION",
        OBSERVATION_VERSION,
        OBSERVATION_PIN,
        &actual,
    );
}

/// A live resolving frame: the Actions scenario's edict has resolved to its
/// target player's sacrifice choice (CR 608.2), so the snapshot carries the
/// `ResolutionChoice` around a `Resolving` frame and its invocation's stack
/// entry. Fixed inputs make it deterministic; restoring it must reproduce it.
pub(super) fn actions_scenario() -> Prepared {
    FixtureCatalog::Action.admit(ScenarioConfig::actions())
}

pub(super) fn cast_edict(prepared: Prepared) -> Game {
    cast_edict_at(prepared, vec![PlayerId(1)])
}

pub(super) fn cast_edict_at(prepared: Prepared, targets: Vec<PlayerId>) -> Game {
    let mut game = Game::new(prepared);
    let answer = |game: &mut Game, response: Answer| {
        let Advance::AwaitingDecision(request) = game.advance(Budget::unlimited()) else {
            panic!("expected a decision before {response:?}")
        };
        game.respond(request.player, request.id, response).unwrap();
    };
    {
        let selected_answer = game.next_test_decision().cast(CardId(4));
        answer(&mut game, selected_answer)
    };
    {
        let selected_answer = game.next_test_decision().player_targets(targets);
        answer(&mut game, selected_answer)
    };
    for land in [2, 3] {
        let object = game.object_id(CardId(land)).unwrap();
        {
            let selected_answer = game.next_test_decision().activate_mana(object);
            answer(&mut game, selected_answer)
        };
    }
    {
        let selected_answer = Answer::Pay {
            pay: PayAnswer::Finish {
                undo_on_failure: true,
            },
        };
        answer(&mut game, selected_answer)
    };
    {
        let selected_answer = Answer::Pay {
            pay: PayAnswer::Spend {
                unrestricted: BTreeMap::from([
                    (Color::B, Quantity::from(1)),
                    (Color::G, Quantity::from(1)),
                ]),
            },
        };
        answer(&mut game, selected_answer)
    };
    for _ in 0..game.live_players().len() {
        {
            let selected_answer = Answer::Pick {
                picks: vec![vec![0]],
            };
            answer(&mut game, selected_answer)
        };
    }
    game
}

/// CR 113.10a: a granted mana ability waiting for its color, on a land
/// whose synthetic artifact grants lands their controller controls "{T}:
/// Add one mana of any color." Frozen witnesses live in `mtg-witness`.
fn granted_mana() -> Game {
    use mtg_contracts::kir::{Atom, Change, Pred, Scope, StaticEffect};
    let mut catalog = FixtureCatalog::Witness.get().clone();
    let relic = catalog
        .definitions
        .iter_mut()
        .find(|d| d.name == "Darksteel Relic")
        .unwrap();
    relic.source_labels = None;
    relic.static_abilities.push(mtg_contracts::StaticAbility {
        functioning_zone: mtg_contracts::FunctioningZone::Battlefield,
        effect: StaticEffect::continuous(
            Scope::Matching {
                filter: Pred(vec![
                    Atom::Type {
                        kind: CardKind::Land,
                    },
                    Atom::ControlledBy {
                        player: mtg_contracts::kir::PlayerExpr::static_controller(),
                    },
                ]),
            },
            vec![Change::AddManaAbilities {
                abilities: vec![mtg_contracts::ManaAbility {
                    spend_restriction: None,
                    restriction: None,
                    condition: None,
                    costs: mtg_contracts::PermanentActivationCosts {
                        remove_source_counters: None,
                        tap: true,
                        mana: None,
                        sacrifice: false,
                        pay_life: None,
                    },
                    hybrid_mana: vec![],
                    production: mtg_contracts::ManaProduction::AnyColor,
                    riders: vec![],
                    source_block: 0,
                }],
            }],
        ),
    });
    relic.capabilities = relic.requirements();
    relic.validate().unwrap();
    let prepared = Board::new(catalog, "granted-mana-format-witness")
        .step(TurnStep::PrecombatMain)
        .seats(2)
        .permanent_or_hand("Darksteel Relic", 0, 0, false)
        .permanent_or_hand("Forest", 0, 0, false)
        .libraries("Forest", 4)
        .prepare();
    let mut game = Game::new(prepared);
    let forest = Driver::object(&game, 1);
    let activate = game.next_test_decision().activate_mana(forest);
    Driver::answer(&mut game, u64::MAX, activate);
    let d = Driver::request(&mut game, u64::MAX);
    let answer = d
        .pick_one(&Candidate::ManaAbility {
            ability: ManaAbilityIdentity::Granted { index: 0 },
        })
        .unwrap();
    game.respond(d.player, d.id, answer).unwrap();
    assert!(matches!(
        Driver::request(&mut game, u64::MAX).reason,
        Reason::ManaColor { .. }
    ));
    game
}

/// CR 733.1: a held mana receipt whose cost published actuals, as a
/// sacrificed source's would (cards admit sacrifice mana costs only on
/// predefined tokens, which no fixture board here creates): its event
/// identities and every ledger delta field. Only its JSON shape is
/// fingerprinted.
fn sacrificed_mana_receipt() -> String {
    fn fill(value: &mut Value) -> bool {
        match value {
            Value::Object(map) if map.contains_key("cost_events") => {
                map.insert(
                    "cost_events".into(),
                    serde_json::to_value([EventId {
                        occurrence: OccurrenceId {
                            batch: 1,
                            path: vec![0],
                        },
                        member: 0,
                    }])
                    .unwrap(),
                );
                map.insert(
                    "ledger".into(),
                    serde_json::json!({ "seats": [{
                        "seat": PlayerId(0),
                        "permanent_left": true,
                        "creature_died": true,
                        "artifacts_entered": 1,
                        "creatures_entered": 1,
                        "nonlands_entered": 1,
                        "tokens_created": 1,
                    }] }),
                );
                true
            }
            Value::Object(map) => map.values_mut().any(fill),
            Value::Array(items) => items.iter_mut().any(fill),
            _ => false,
        }
    }
    let game = mana_riders().pop().unwrap();
    let mut sample: Value = serde_json::from_str(&game.snapshot().unwrap()).unwrap();
    assert!(fill(&mut sample["state"]), "a held mana receipt");
    // The shape must round-trip through the real receipt type.
    let mut receipt = None;
    fn find(value: &Value, out: &mut Option<PermanentManaUndo>) {
        match value {
            Value::Object(map) if map.contains_key("cost_events") => {
                *out = Some(serde_json::from_value(value.clone()).unwrap())
            }
            Value::Object(map) => map.values().for_each(|v| find(v, out)),
            Value::Array(items) => items.iter().for_each(|v| find(v, out)),
            _ => {}
        }
    }
    find(&sample["state"], &mut receipt);
    assert_eq!(receipt.unwrap().ledger.seats.len(), 1);
    serde_json::to_string(&sample).unwrap()
}

/// CR 605.3a-b, 733.1: a cast paid by Yavimaya Coast's second printed mana
/// ability, sampled at its color choice (a nonzero ability index), inside its
/// damage rider's suspended transaction, and with its kept receipt retained
/// by the cast.
fn mana_riders() -> Vec<Game> {
    let prepared = Board::new(
        FixtureCatalog::Witness.get().clone(),
        "w3-mana-format-witness",
    )
    .step(TurnStep::PrecombatMain)
    .seats(2)
    .permanent_or_hand("Colossal Dreadmaw", 0, 0, true)
    .permanent_or_hand("Yavimaya Coast", 0, 0, false)
    .libraries("Forest", 4)
    .prepare();
    let mut game = Game::new(prepared);
    Driver::cast(&mut game, CardId(0));
    let coast = Driver::object(&game, 1);
    let activate = game.next_test_decision().activate_mana(coast);
    Driver::answer(&mut game, u64::MAX, activate);
    let d = Driver::request(&mut game, u64::MAX);
    let answer = d
        .pick_one(&Candidate::ManaAbility {
            ability: ManaAbilityIdentity::Printed { index: 1 },
        })
        .unwrap();
    game.respond(d.player, d.id, answer).unwrap();
    let mut samples = vec![];
    let d = Driver::request(&mut game, u64::MAX);
    assert!(matches!(d.reason, Reason::ManaColor { .. }));
    samples.push(game.fork());
    let answer = d.pick_one(&Candidate::Color { color: Color::G }).unwrap();
    game.respond(d.player, d.id, answer).unwrap();
    while !matches!(&game.state.rules.frame, Frame::DamageEffects { .. }) {
        assert!(matches!(
            game.advance(Budget::new(1)),
            Advance::Suspended { .. }
        ));
    }
    samples.push(game.fork());
    let d = Driver::request(&mut game, u64::MAX);
    assert!(matches!(d.reason, Reason::Mana { .. }));
    samples.push(game);
    samples
}

fn mid_resolution() -> Game {
    let mut game = cast_edict(actions_scenario());
    let Advance::AwaitingDecision(Decision {
        player,
        reason: Reason::Sacrifice,
        ..
    }) = game.advance(Budget::unlimited())
    else {
        panic!("the edict resolves to its target player's sacrifice")
    };
    assert_eq!(player, PlayerId(1));
    assert!(matches!(game.state.rules.frame, Frame::Kernel { .. }));
    let restored =
        Game::restore(game.prepared.as_ref().clone(), &game.snapshot().unwrap()).unwrap();
    assert_eq!(
        restored.state_digest().unwrap(),
        game.state_digest().unwrap()
    );
    game
}

fn suspended_battlefield_entry() -> Game {
    let mut catalog = FixtureCatalog::Witness.get().clone();
    let land = catalog
        .definitions
        .iter_mut()
        .find(|d| d.name == "Forest")
        .unwrap();
    land.source_labels = None;
    land.static_abilities.push(StaticAbility {
        functioning_zone: FunctioningZone::Battlefield,
        effect: StaticEffect::PayLifeOrEnterTapped { amount: 2.into() },
    });
    land.capabilities = land.requirements();
    let mut game = Game::new(
        Board::new(catalog, "snapshot-entry-choice")
            .step(TurnStep::PrecombatMain)
            .permanent_or_hand("Forest", 0, 0, true)
            .libraries("Forest", 4)
            .prepare(),
    );
    let object = Driver::object(&game, 0);
    Driver::action(&mut game, Action::PlayLand { object });
    assert_eq!(
        Driver::request(&mut game, u64::MAX).reason,
        Reason::OptionalEffect
    );
    game
}

#[test]
fn suspended_land_play_rejects_rehashed_tampering() {
    let game = suspended_battlefield_entry();
    let raw = game.snapshot().unwrap();
    let prepared = game.prepared.as_ref();
    Game::restore(prepared.clone(), &raw).unwrap();
    for (tamper, expected) in [
        ("controller", "invalid suspended land play origin"),
        (
            "zone",
            "invalid suspended land play: invalid or unsupported land play",
        ),
        ("cause", "invalid suspended battlefield entry cause"),
        ("decision_id", "invalid suspended land play origin"),
        ("resume", "invalid suspended land play origin"),
        ("timing", "invalid suspended land play: land play requires"),
        (
            "allowance",
            "invalid suspended land play: invalid or unsupported land play",
        ),
    ] {
        let mut forged: Snapshot = strict_read(&raw).unwrap();
        let object = Driver::object(&game, 0);
        let mut frame = serde_json::to_value(&forged.state.rules.frame).unwrap();
        match tamper {
            "controller" => frame["entry"]["controller"] = serde_json::json!(PlayerId(1)),
            "zone" => {
                forged
                    .state
                    .rules
                    .objects
                    .get_mut(&object.card)
                    .unwrap()
                    .zone = Zone::Exile
            }
            "cause" => frame["entry"]["cause"] = serde_json::json!(BatchCause::Fixture),
            "decision_id" => frame["entry"]["cause"]["decision_id"] = u64::MAX.into(),
            "resume" => forged.state.rules.frames[0] = Frame::Transition,
            "timing" => forged.state.rules.turn.step = TurnStep::Upkeep,
            "allowance" => forged.state.rules.turn.lands_played = 1,
            _ => unreachable!(),
        }
        forged.state.rules.frame = serde_json::from_value(frame).unwrap();
        forged.state_sha256 = digest(&forged.state).unwrap();
        let error = Game::restore(prepared.clone(), &canonical(&forged).unwrap())
            .err()
            .expect("tampered entry must not restore");
        assert!(error.starts_with(expected), "{tamper}: {error}");
    }
}

/// Wire shapes no fixture board suspends in: a battlefield search's entry
/// choice with a chosen Aura host, a payment and reveals (CR 303.4f,
/// 614.1c, 701.20a), and a
/// forced tapped entry that still offers its payment, and a private
/// top-order choice and outcome. Only their JSON shape is fingerprinted.
fn library_entry_shapes() -> Vec<String> {
    let game = mid_resolution();
    let object = Driver::object(&game, 0);
    let Frame::Resolve { frame } = &game.state.rules.frames[0] else {
        unreachable!("fixture has a saved resolving frame")
    };
    let choice_frame = frame.clone();
    let mut search: Value = serde_json::from_str(&game.snapshot().unwrap()).unwrap();
    search["state"]["rules"]["frame"]["choice"] =
        serde_json::to_value(ResolutionChoice::SearchEntry {
            player: PlayerId(0),
            found: vec![object],
            command_choices: vec![(object, false)],
            hosts: vec![(object, object)],
            paid: vec![(object, true)],
            revealed: vec![(object, Some(object)), (object, None)],
            source_block: 0,
        })
        .unwrap();
    let mut entry: Value =
        serde_json::from_str(&suspended_battlefield_entry().snapshot().unwrap()).unwrap();
    entry["state"]["rules"]["frame"]["entry"]["force_tapped"] = true.into();
    let mut reorder = search.clone();
    reorder["state"]["rules"]["frame"]["choice"] =
        serde_json::to_value(ResolutionChoice::ReorderLibraryTop {
            player: PlayerId(0),
            looked: vec![object],
            source_block: 0,
        })
        .unwrap();
    let mut frame = frame.clone();
    frame.cursor = 1;
    frame.outcomes = vec![InstructionOutcome::Effect {
        actual: Some(EventOutcome::LibraryTopReordered {
            player: PlayerId(0),
            looked: vec![object],
            top: vec![object],
        }),
    }];
    let mut completed = search.clone();
    completed["state"]["rules"]["frame"] = serde_json::to_value(Frame::Resolve { frame }).unwrap();
    let mut split = search.clone();
    split["state"]["rules"]["frame"]["choice"] =
        serde_json::to_value(ResolutionChoice::SearchSplit {
            player: PlayerId(0),
            found: [object, object],
            source_block: 0,
        })
        .unwrap();
    let mut look = search.clone();
    look["state"]["rules"]["frame"]["choice"] =
        serde_json::to_value(ResolutionChoice::LookChooseHand {
            player: PlayerId(0),
            looked: vec![object],
            source_block: 0,
        })
        .unwrap();
    let mut order = search.clone();
    order["state"]["rules"]["frame"]["choice"] =
        serde_json::to_value(ResolutionChoice::LookOrderBottom {
            player: PlayerId(0),
            looked: vec![object],
            hand: object,
            source_block: 0,
        })
        .unwrap();
    let mut commander = search.clone();
    commander["state"]["rules"]["frame"] = serde_json::to_value(Frame::CommanderReturn {
        object,
        destination: Zone::Hand { owner: PlayerId(0) },
        continuation: CommanderContinuation::LookHand {
            frame: choice_frame,
            player: PlayerId(0),
            looked: vec![object],
            bottom: vec![object],
            source_block: 0,
        },
    })
    .unwrap();
    let mut bottomed = completed.clone();
    bottomed["state"]["rules"]["frame"]["frame"]["outcomes"][0] =
        serde_json::to_value(InstructionOutcome::Effect {
            actual: Some(EventOutcome::LibraryPrefixBottomed {
                player: PlayerId(0),
                looked: vec![object],
                bottom: vec![object],
            }),
        })
        .unwrap();
    [
        search, entry, reorder, completed, split, look, order, commander, bottomed,
    ]
    .iter()
    .map(|value| serde_json::to_string(value).unwrap())
    .collect()
}

/// CR 608.2d: the open object choice carries only the resolving frame and a tag.
fn object_choice_shape() -> String {
    let mut sample: Value = serde_json::from_str(&mid_resolution().snapshot().unwrap()).unwrap();
    let frame = sample["state"]["frames"][0]["frame"].clone();
    sample["state"]["frame"] = serde_json::json!({
        "stage": "resolution_choice",
        "frame": frame,
        "choice": ResolutionChoice::Object {},
    });
    serde_json::to_string(&sample).unwrap()
}

fn chosen_cost_shapes() -> Vec<Game> {
    let mut catalog = FixtureCatalog::Witness.get().clone();
    let forest = catalog
        .definitions
        .iter_mut()
        .find(|d| d.name == "Forest")
        .unwrap();
    forest.source_labels = None;
    for action in [ChosenCostAction::Discard, ChosenCostAction::Tap] {
        forest.activated.push(ActivatedAbility {
            x_mana_cost: false,
            hybrid_mana: vec![],
            modal: None,
            cost: ActivationCost::ChooseObjects(ChosenObjectCost {
                minimum_power: None,
                action,
                filter: ChosenCostFilter::Any,
                count: 1,
                another: false,
                tap_source: false,
                mana: None,
            }),
            functioning_zone: FunctioningZone::Battlefield,
            restriction: None,
            program: SpellProgram::instructions(SpellTiming::Instant).with_instructions([
                Instruction::act(
                    mtg_contracts::kir::Verb::Draw {
                        player: PlayerExpr::you(Context::Activated),
                        amount: mtg_contracts::kir::Amount::fixed(1),
                    },
                    0,
                ),
            ]),
        });
    }
    forest.capabilities = forest.requirements();
    let prepared = Board::new(catalog, "snapshot-chosen-cost")
        .step(TurnStep::PrecombatMain)
        .permanent_or_hand("Forest", 0, 0, false)
        .permanent_or_hand("Forest", 0, 0, true)
        .prepare();
    let mut samples = vec![];
    for (ability, card) in [(0, 1), (1, 0)] {
        let mut game = Game::new(prepared.clone());
        let source = Driver::object(&game, 0);
        let object = Driver::object(&game, card);
        Driver::action(
            &mut game,
            Action::ActivateAbility {
                object: source,
                ability,
            },
        );
        assert!(matches!(
            Driver::request(&mut game, u64::MAX).reason,
            Reason::ChosenObjectCost { .. }
        ));
        samples.push(game.fork());
        Driver::pick_object(&mut game, object);
        assert_eq!(
            Driver::request(&mut game, u64::MAX).reason,
            Reason::Priority
        );
        samples.push(game);
    }
    samples
}

/// CR 602.5: a conditioned mana ability's receipt in an open mana window,
/// then the conditioned stack activation it paid for, each with the counts
/// read as the activation began.
fn activation_condition_shapes() -> Vec<Game> {
    let you = PlayerExpr::you(Context::Activated);
    let controls = |kind| mtg_contracts::kir::Cond::Compare {
        left: Amount::Count {
            set: SetExpr::battlefield(mtg_contracts::kir::Pred(vec![
                mtg_contracts::kir::Atom::Type { kind },
                mtg_contracts::kir::Atom::ControlledBy {
                    player: you.clone(),
                },
            ])),
        },
        cmp: mtg_contracts::kir::Cmp::AtLeast,
        right: Amount::fixed(1),
    };
    let mut catalog = FixtureCatalog::Witness.get().clone();
    for d in &mut catalog.definitions {
        if d.name == "Forest" {
            d.activated.push(ActivatedAbility {
                x_mana_cost: false,
                hybrid_mana: vec![],
                cost: ActivationCost::Mana {
                    cost: ManaCost {
                        generic: 2.into(),
                        colored: vec![],
                    },
                },
                functioning_zone: FunctioningZone::Battlefield,
                restriction: Some(ActivationRestriction {
                    window: None,
                    once_each_turn: false,
                    times_each_turn: None,
                    condition: Some(controls(CardKind::Land)),
                }),
                modal: None,
                program: SpellProgram::instructions(SpellTiming::Instant).with_instructions([
                    Instruction::act(
                        mtg_contracts::kir::Verb::Draw {
                            player: you.clone(),
                            amount: mtg_contracts::kir::Amount::fixed(1),
                        },
                        0,
                    ),
                ]),
            });
        } else if d.name == "Sol Ring" {
            d.mana_abilities[0].condition = Some(controls(CardKind::Artifact));
        } else {
            continue;
        }
        d.source_labels = None;
        d.capabilities = d.requirements();
    }
    let prepared = Board::new(catalog, "snapshot-activation-condition")
        .step(TurnStep::PrecombatMain)
        .permanent_or_hand("Forest", 0, 0, false)
        .permanent_or_hand("Sol Ring", 0, 0, false)
        .libraries("Forest", 2)
        .prepare();
    let mut game = Game::new(prepared);
    let forest = Driver::object(&game, 0);
    let ring = Driver::object(&game, 1);
    Driver::action(
        &mut game,
        Action::ActivateAbility {
            object: forest,
            ability: 0,
        },
    );
    let d = Driver::request(&mut game, u64::MAX);
    let source = d
        .index(&Candidate::Action {
            action: Action::ActivateMana { object: ring },
        })
        .unwrap();
    Driver::answer(
        &mut game,
        u64::MAX,
        Answer::Pay {
            pay: PayAnswer::Activate { source },
        },
    );
    Driver::request(&mut game, u64::MAX);
    let window = game.fork();
    Driver::pay(&mut game, &[]);
    assert_eq!(
        Driver::request(&mut game, u64::MAX).reason,
        Reason::Priority
    );
    vec![window, game]
}

/// Substitute each measured variant in a saved resolution check to
/// fingerprint its wire shape.
fn intervening_comparison_shapes() -> impl Iterator<Item = String> {
    let mut sample: Value = serde_json::from_str(&mid_resolution().snapshot().unwrap()).unwrap();
    let controller = PlayerId(0);
    [
        ControllerMeasurement::Life(5.into()),
        ControllerMeasurement::HandCount(7),
        ControllerMeasurement::GraveyardCount(3),
        ControllerMeasurement::CreatureTotalPower(9.into()),
    ]
    .into_iter()
    .map(move |observed| InterveningCheck::ControllerComparison {
        controller,
        observed,
        passed: true,
    })
    .chain([
        InterveningCheck::ControlsOwnCommander {
            controller,
            commander: Some(ObjectId {
                card: CardId(7),
                face: mtg_contracts::CardFace::Front,
                incarnation: 2,
            }),
            passed: true,
        },
        InterveningCheck::OpponentControlsMore {
            controller,
            own: 2,
            opponents: vec![(PlayerId(1), 3), (PlayerId(2), 2)],
            passed: true,
        },
    ])
    .map(move |check| {
        sample["state"]["frames"][0]["frame"]["condition_check"] =
            serde_json::to_value(check).unwrap();
        serde_json::to_string(&sample).unwrap()
    })
}

/// A paused keyword choice with real player counters from its preceding leaf.
fn suspended_proliferate() -> Game {
    use mtg_contracts::kir::{Amount, Context, PlayerCounterKind, PlayerExpr, Verb};

    let mut catalog = FixtureCatalog::Base.get().clone();
    let spell = catalog
        .definitions
        .iter_mut()
        .find(|d| d.name == "Unsummon")
        .unwrap();
    spell.source_labels = None;
    let program = spell.spell.as_mut().unwrap();
    program.targets.clear();
    let player = PlayerExpr::you(Context::Spell);
    program.instructions = vec![
        Instruction::act(
            Verb::GiveCounters {
                player: player.clone(),
                counter: PlayerCounterKind::Energy,
                amount: Amount::fixed(2),
            },
            0,
        ),
        Instruction::act(Verb::Proliferate { player }, 0),
    ];
    spell.capabilities = spell.requirements();
    let mut game = Game::new(
        Board::new(catalog, "snapshot-proliferate")
            .step(TurnStep::PrecombatMain)
            .permanent_or_hand("Unsummon", 0, 0, true)
            .permanent_or_hand("Island", 0, 0, false)
            .prepare(),
    );
    Driver::cast(&mut game, CardId(0));
    Driver::pay(&mut game, &[1]);
    for _ in 0..game.live_players().len() {
        Driver::pass(&mut game);
    }
    assert_eq!(
        Driver::request(&mut game, u64::MAX).reason,
        Reason::Proliferate
    );
    game
}

/// CR 616.1: a proliferate whose +1/+1 placement meets an addition and a
/// multiplication suspends on the creature controller's order, keeping the
/// consumed proliferate input; answering it commits the placement with its
/// recorded lineage (traced in detail).
fn suspended_replacement_order() -> [Game; 3] {
    use mtg_contracts::QuantityModification;
    use mtg_contracts::kir::{
        Context, CounterKind, CounterRecipients, PlayerExpr, QuantityEvent, ReplacementOrigin,
        StaticEffect, StaticRule, Verb,
    };

    let mut catalog = FixtureCatalog::Witness.get().clone();
    for (name, change) in [
        (
            "Darksteel Relic",
            QuantityModification::Add { amount: 1.into() },
        ),
        (
            "Forest",
            QuantityModification::Multiply { factor: 2.into() },
        ),
    ] {
        let d = catalog
            .definitions
            .iter_mut()
            .find(|d| d.name == name)
            .unwrap();
        d.source_labels = None;
        d.static_abilities = vec![StaticAbility {
            functioning_zone: FunctioningZone::Battlefield,
            effect: StaticEffect::rule(StaticRule::ModifyQuantity {
                event: QuantityEvent::PutCounters {
                    counter: Some(CounterKind::PlusOnePlusOne),
                    recipients: CounterRecipients::Creatures,
                    origin: ReplacementOrigin::Any,
                },
                change,
            }),
        }];
        d.capabilities = d.requirements();
    }
    let spell = catalog
        .definitions
        .iter_mut()
        .find(|d| d.name == "Unsummon")
        .unwrap();
    spell.source_labels = None;
    let program = spell.spell.as_mut().unwrap();
    program.targets.clear();
    program.instructions = vec![Instruction::act(
        Verb::Proliferate {
            player: PlayerExpr::you(Context::Spell),
        },
        0,
    )];
    spell.capabilities = spell.requirements();
    let mut game = Game::with_trace(
        Board::new(catalog, "snapshot-replacement-order")
            .step(TurnStep::PrecombatMain)
            .seats(4)
            .permanent_or_hand("Unsummon", 0, 0, true)
            .permanent_or_hand("Island", 0, 0, false)
            .permanent_or_hand("Grizzly Bears", 0, 0, false)
            .permanent_or_hand("Darksteel Relic", 0, 0, false)
            .permanent_or_hand("Forest", 0, 0, false)
            .permanent_or_hand("Grizzly Bears", 2, 2, false)
            .permanent_or_hand("Forest", 2, 2, true)
            .card("Forest", 2, 2, Zone::Graveyard { owner: PlayerId(2) })
            .prepare(),
        TraceMode::Detailed,
    );
    let bears = Driver::object(&game, 2);
    game.state
        .rules
        .objects
        .get_mut(&bears.card)
        .unwrap()
        .plus_one_counters = 1.into();
    Driver::cast(&mut game, CardId(0));
    Driver::pay(&mut game, &[1]);
    for _ in 0..game.live_players().len() {
        Driver::pass(&mut game);
    }
    let d = Driver::request(&mut game, u64::MAX);
    assert_eq!(d.reason, Reason::Proliferate);
    let answer = d.pick_one(&Candidate::Object { object: bears }).unwrap();
    game.respond(d.player, d.id, answer).unwrap();
    let d = Driver::request(&mut game, u64::MAX);
    assert_eq!(
        d.reason,
        Reason::ReplacementOrder {
            subject: Some(bears)
        }
    );
    let pending = game.fork();
    let mut rebuilding = pending.fork();
    rebuilding.concede(PlayerId(2)).unwrap();
    assert!(matches!(
        rebuilding.advance(Budget::new(1)),
        Advance::Suspended { .. }
    ));
    let relic = Driver::object(&game, 3);
    let answer = d.pick_one(&Candidate::Object { object: relic }).unwrap();
    game.respond(d.player, d.id, answer).unwrap();
    Driver::request(&mut game, u64::MAX);
    // 1 + (1 + 1) × 2.
    assert_eq!(game.current(bears).unwrap().plus_one_counters, 5.into());
    [pending, rebuilding, game]
}

/// CR 903.9b: a spell returning a commander to its owner's hand suspends
/// the evaluated move on that owner's command-zone choice.
fn suspended_commander_move() -> Game {
    let mut game = Game::new(
        Prepared::admit(
            FixtureCatalog::Action.get().clone(),
            ScenarioConfig::command_casting(),
        )
        .unwrap(),
    );
    Driver::cast(&mut game, CardId(0));
    Driver::pay(&mut game, &[1]);
    for _ in 0..5 {
        Driver::pass(&mut game);
    }
    Driver::cast(&mut game, CardId(7));
    let commander = Driver::object(&game, 0);
    let answer = game.next_test_decision().targets(vec![commander]);
    Driver::answer(&mut game, u64::MAX, answer);
    Driver::pay(&mut game, &[6]);
    for _ in 0..4 {
        Driver::pass(&mut game);
    }
    assert!(matches!(
        Driver::request(&mut game, u64::MAX).reason,
        Reason::CommanderReturn { .. }
    ));
    let snapshot: Value = serde_json::from_str(&game.snapshot().unwrap()).unwrap();
    assert_eq!(
        snapshot["state"]["frame"]["task"]["stage"]["progress"],
        "moves"
    );
    game
}

/// A stack target offer with two public regeneration shields. The first
/// shield can be projected while the second still keeps work suspended.
fn stack_target_and_shields() -> Game {
    use mtg_contracts::kir::{Ref, SetExpr, Verb};

    let mut catalog = FixtureCatalog::Witness.get().clone();
    for (name, domain, verb) in [
        (
            "Unsummon",
            TargetDomain::BattlefieldCreature,
            Verb::Regenerate {
                what: SetExpr::target(0),
            },
        ),
        (
            "Giant Growth",
            TargetDomain::StackSpell,
            Verb::Counter {
                what: Ref::Target { slot: 0 },
            },
        ),
    ] {
        let spell = catalog
            .definitions
            .iter_mut()
            .find(|d| d.name == name)
            .unwrap();
        spell.source_labels = None;
        let program = spell.spell.as_mut().unwrap();
        program.targets[0].domain = domain;
        program.instructions = vec![Instruction::act(verb, 0)];
        spell.capabilities = spell.requirements();
    }
    let mut board =
        Board::new(catalog, "formats-stack-target-and-shields").step(TurnStep::PrecombatMain);
    for _ in 0..3 {
        board = board.permanent_or_hand("Unsummon", 0, 0, true);
    }
    board = board
        .permanent_or_hand("Giant Growth", 0, 0, true)
        .permanent_or_hand("Grizzly Bears", 0, 0, false)
        .permanent_or_hand("Forest", 0, 0, false);
    for _ in 0..3 {
        board = board.permanent_or_hand("Island", 0, 0, false);
    }
    let mut game = Game::new(board.prepare());
    let bear = Driver::object(&game, 4);
    for card in 0..3 {
        Driver::cast(&mut game, CardId(card));
        Driver::pick_object(&mut game, bear);
        Driver::pay(&mut game, &[6 + card]);
        if card < 2 {
            for _ in game.live_players() {
                Driver::pass(&mut game);
            }
        }
    }
    Driver::cast(&mut game, CardId(3));
    let offer = Driver::request(&mut game, u64::MAX);
    assert!(matches!(
        offer.candidates.as_slice(),
        [Candidate::Stack { .. }]
    ));
    assert_eq!(game.state.rules.effects.len(), 2);
    game
}

#[test]
fn simultaneous_effect_records_may_share_a_timestamp() {
    let mut game = stack_target_and_shields();
    let effects = &mut game.state.rules.effects;
    assert!(effects[0].timestamp < effects[1].timestamp);
    let later = effects[1].timestamp;
    effects[1].timestamp = effects[0].timestamp;
    game.validate_effect_state().unwrap();

    let effects = &mut game.state.rules.effects;
    effects[0].timestamp = later;
    assert_eq!(
        game.validate_effect_state().unwrap_err(),
        "effect creation order invariant"
    );
}

fn pending_stack_target() -> Game {
    let mut game = stack_target_and_shields();
    let offer = Driver::request(&mut game, u64::MAX);
    let answer = offer.pick_one(&offer.candidates[0]).unwrap();
    game.respond(offer.player, offer.id, answer).unwrap();
    game
}

fn suspended_delve() -> Game {
    let mut catalog = FixtureCatalog::Witness.get().clone();
    let ring = catalog
        .definitions
        .iter_mut()
        .find(|d| d.name == "Sol Ring")
        .unwrap();
    ring.delve = true;
    let mut game = Game::new(
        Board::new(catalog, "snapshot-delve")
            .step(TurnStep::PrecombatMain)
            .permanent_or_hand("Sol Ring", 0, 0, true)
            .card("Island", 0, 0, Zone::Graveyard { owner: PlayerId(0) })
            .libraries("Island", 2)
            .prepare(),
    );
    Driver::cast(&mut game, CardId(0));
    Driver::answer(
        &mut game,
        u64::MAX,
        Answer::Pay {
            pay: PayAnswer::Finish {
                undo_on_failure: true,
            },
        },
    );
    let card = Driver::object(&game, 1);
    Driver::pick_object(&mut game, card);
    Driver::request(&mut game, u64::MAX);
    game
}

fn suspended_convoke() -> Game {
    let mut catalog = FixtureCatalog::Witness.get().clone();
    let ring = catalog
        .definitions
        .iter_mut()
        .find(|d| d.name == "Sol Ring")
        .unwrap();
    ring.convoke = true;
    let mut game = Game::new(
        Board::new(catalog, "snapshot-convoke")
            .step(TurnStep::PrecombatMain)
            .permanent_or_hand("Sol Ring", 0, 0, true)
            .permanent_or_hand("Grizzly Bears", 0, 0, false)
            .libraries("Island", 2)
            .prepare(),
    );
    Driver::cast(&mut game, CardId(0));
    Driver::answer(
        &mut game,
        u64::MAX,
        Answer::Pay {
            pay: PayAnswer::Finish {
                undo_on_failure: true,
            },
        },
    );
    let card = Driver::object(&game, 1);
    Driver::pick_object(&mut game, card);
    Driver::request(&mut game, u64::MAX);
    game
}

/// A frozen Saga cast that entered with a lore counter and put its first
/// chapter on the stack (CR 714.2b, 714.3a).
fn saga_chapter() -> Game {
    let mut game = Game::new(
        Board::new(
            FixtureCatalog::Witness.get().clone(),
            "formats-saga-chapter",
        )
        .seats(2)
        .step(TurnStep::PrecombatMain)
        .permanent_or_hand("The Birth of Meletis", 0, 0, true)
        .permanent_or_hand("Plains", 0, 0, false)
        .permanent_or_hand("Plains", 0, 0, false)
        .prepare(),
    );
    Driver::cast(&mut game, CardId(0));
    Driver::pay(&mut game, &[1, 2]);
    for _ in 0..8 {
        let d = Driver::request(&mut game, u64::MAX);
        assert_eq!(d.reason, Reason::Priority);
        if game.state.rules.stack.last().is_some_and(|entry| {
            matches!(
                &entry.kind,
                StackKind::Triggered {
                    witness: TriggerEventWitness::LorePlaced { .. },
                    ..
                }
            )
        }) {
            return game;
        }
        Driver::pass(&mut game);
    }
    panic!("Saga chapter did not reach the stack");
}

/// CR 606.4, 733.1: a loyalty ability on the stack, whose source facts
/// carry the planeswalker's loyalty counters as captured at activation. The
/// planeswalker is a synthetic retype of Darksteel Relic with loyalty 3 and
/// "[-1]: Draw a card." The sample also sets the remaining optional prior-row
/// fields (a loyalty use stamp, attachment, phasing, a hideaway link and an
/// Adventure permission) on those facts so the fingerprint covers their shape.
fn loyalty_activation() -> String {
    let mut catalog = FixtureCatalog::Witness.get().clone();
    let walker = catalog
        .definitions
        .iter_mut()
        .find(|d| d.name == "Darksteel Relic")
        .unwrap();
    walker.source_labels = None;
    walker.static_abilities.clear();
    walker.keywords.clear();
    walker.characteristics.kind = CardKind::Planeswalker;
    walker.characteristics.loyalty = Some(3.into());
    walker.activated.push(ActivatedAbility {
        x_mana_cost: false,
        hybrid_mana: vec![],
        modal: None,
        cost: ActivationCost::Loyalty {
            change: (-1).into(),
        },
        functioning_zone: FunctioningZone::Battlefield,
        restriction: None,
        program: SpellProgram::instructions(SpellTiming::MainPhase).with_instructions([
            Instruction::act(
                mtg_contracts::kir::Verb::Draw {
                    player: PlayerExpr::you(Context::Activated),
                    amount: mtg_contracts::kir::Amount::fixed(1),
                },
                0,
            ),
        ]),
    });
    walker.capabilities = walker.requirements();
    let mut game = Game::new(
        Board::new(catalog, "formats-loyalty-activation")
            .step(TurnStep::PrecombatMain)
            .seats(2)
            .permanent_or_hand("Darksteel Relic", 0, 0, false)
            .libraries("Forest", 3)
            .prepare(),
    );
    let walker = Driver::object(&game, 0);
    Driver::action(
        &mut game,
        Action::ActivateAbility {
            object: walker,
            ability: 0,
        },
    );
    Driver::request(&mut game, u64::MAX);
    assert_eq!(game.state.rules.stack.len(), 1);
    assert_eq!(game.current(walker).unwrap().loyalty_counters, 2.into());
    let mut sample: Value = serde_json::from_str(&game.snapshot().unwrap()).unwrap();
    let facts = sample["state"]["stack"][0]["source_facts"]
        .as_object_mut()
        .expect("the loyalty ability records its source facts");
    // Captured before the cost is paid: three counters and no use yet.
    assert_eq!(facts["loyalty_counters"], "3");
    assert!(!facts.contains_key("loyalty_used_turn"));
    facts.insert("loyalty_used_turn".into(), 1.into());
    facts.insert("attached_to".into(), serde_json::to_value(walker).unwrap());
    facts.insert("attachment_timestamp".into(), 1.into());
    facts.insert(
        "phased_out".into(),
        serde_json::to_value(PhaseReturn::Indirect {
            attached_to: walker,
        })
        .unwrap(),
    );
    facts.insert(
        "hideaway".into(),
        serde_json::json!({
            "card": walker,
            "source": walker,
            "source_block": 0,
            "source_handles": ["v0-o1", "v1-o1", "v2-o1", "v3-o1"],
            "lookers": [true, false, false, false],
        }),
    );
    facts.insert(
        "adventure_player".into(),
        serde_json::to_value(PlayerId(0)).unwrap(),
    );
    serde_json::to_string(&sample).unwrap()
}

#[test]
fn snapshot_shape_fingerprint_is_pinned_to_snapshot_version() {
    let samples = representatives()
        .into_iter()
        .chain([
            mid_resolution(),
            suspended_battlefield_entry(),
            suspended_proliferate(),
            super::kernel_control_tests::suspended_control(),
            super::kernel_control_tests::suspended_counted_draw(),
            super::kernel_control_tests::suspended_creature_payment(),
            pending_stack_target(),
            suspended_delve(),
            suspended_convoke(),
            saga_chapter(),
            suspended_commander_move(),
        ])
        .chain(suspended_replacement_order())
        .chain(mana_riders())
        .chain([granted_mana()])
        .chain(chosen_cost_shapes())
        .chain(activation_condition_shapes())
        .map(|g| g.snapshot().unwrap())
        .chain(library_entry_shapes())
        .chain(intervening_comparison_shapes())
        .chain([{
            let mut sample: Value =
                serde_json::from_str(&mid_resolution().snapshot().unwrap()).unwrap();
            let event = Event::TriggeredAbilityNotPlaced {
                trigger_id: TriggerId(1),
                stack_id: StackId(1),
                reason: TriggerPlacementFailure::NoModesChosen,
            };
            sample["trace"]["events"] = serde_json::json!([event]);
            sample["trace"]["records"] = serde_json::json!([TraceRecord {
                sequence: 1,
                boundary: 1,
                decisions_seen: 1,
                concessions_seen: 0,
                record: EvidenceRecord::Event {
                    event: Box::new(event)
                },
            }]);
            let history = &mut sample["state"]["cast_history"][0];
            history["attacked"] = true.into();
            history["creature_died"] = true.into();
            history["permanent_left"] = true.into();
            history["hand_casts"] = 1.into();
            history["tokens_created"] = 1.into();
            for field in ["artifacts_entered", "creatures_entered", "nonlands_entered"] {
                history[field] = serde_json::to_value(Quantity::from(1)).unwrap();
            }
            history["life_gained"] = serde_json::to_value(Quantity::from(2)).unwrap();
            sample["state"]["frames"][0]["frame"]["condition_check"] =
                serde_json::to_value(InterveningCheck::TurnHistory {
                    controller: PlayerId(0),
                    observed: 2.into(),
                    passed: true,
                })
                .unwrap();
            serde_json::to_string(&sample).unwrap()
        }])
        .chain([
            object_choice_shape(),
            loyalty_activation(),
            sacrificed_mana_receipt(),
        ])
        .collect::<Vec<_>>();
    for raw in &samples {
        let value: Value = serde_json::from_str(raw).unwrap();
        assert_eq!(value["snapshot_version"], SNAPSHOT_VERSION);
    }
    check(
        "Snapshot",
        "SNAPSHOT_VERSION",
        SNAPSHOT_VERSION,
        SNAPSHOT_PIN,
        &shape_digest(&samples),
    );
}

/// Fingerprint a saved block decision in the captured search wire shape.
fn search_with_blocker_limit() -> String {
    use mtg_contracts::kir::{BlockRestriction, Scope, StaticEffect, StaticRule};

    let mut catalog = FixtureCatalog::Witness.get().clone();
    let attacker = catalog
        .definitions
        .iter_mut()
        .find(|d| d.name == "Grizzly Bears")
        .unwrap();
    attacker.source_labels = None;
    attacker.static_abilities.push(StaticAbility {
        functioning_zone: FunctioningZone::Battlefield,
        effect: StaticEffect::Rule {
            rule: StaticRule::Block {
                affects: Scope::Source,
                restriction: BlockRestriction::AtMost { count: 1 },
            },
            condition: None,
        },
    });
    attacker.capabilities = attacker.requirements();
    let mut game = Game::new(
        Board::new(catalog, "formats-search-blocker-limit")
            .seats(2)
            .step(TurnStep::BeginningCombat)
            .permanent_or_hand("Grizzly Bears", 0, 0, false)
            .permanent_or_hand("Grizzly Bears", 1, 1, false)
            .permanent_or_hand("Grizzly Bears", 1, 1, false)
            .prepare(),
    );
    let attackers = loop {
        let decision = Driver::request(&mut game, u64::MAX);
        if decision.reason == Reason::Attackers {
            break decision;
        }
        assert_eq!(decision.reason, Reason::Priority);
        Driver::pass(&mut game);
    };
    let answer = attackers.attack(vec![(Driver::object(&game, 0), PlayerId(1))]);
    game.respond(attackers.player, attackers.id, answer)
        .unwrap();
    let blockers = loop {
        let decision = Driver::request(&mut game, u64::MAX);
        if decision.reason == Reason::Blockers {
            break decision;
        }
        assert_eq!(decision.reason, Reason::Priority);
        Driver::pass(&mut game);
    };
    let QueryResult::Incomplete { work, .. } = game.observe(blockers.player, Budget::new(1)) else {
        panic!("block decision observation should suspend");
    };
    let raw = game.save_work(&work).unwrap();
    let restored = game.restore_work(&raw).unwrap();
    assert_eq!(game.save_work(&restored).unwrap(), raw);
    let saved: Value = serde_json::from_str(&raw).unwrap();
    let decision = &saved["data"]["task"]["builder"]["decision"];
    assert_eq!(decision["shape"]["rules"][0]["rule"], "picked_at_most");
    assert_eq!(decision["shape"]["rules"][0]["count"], 1);

    // Search admission is priority-only. Fingerprint its captured decision
    // slots with the real block decision without admitting combat searches.
    let mut root = Game::new(
        Board::new(FixtureCatalog::Witness.get().clone(), "formats-search-root")
            .seats(2)
            .step(TurnStep::PrecombatMain)
            .permanent_or_hand("Grizzly Bears", 0, 0, false)
            .prepare(),
    );
    let priority = Driver::request(&mut root, u64::MAX);
    assert_eq!(priority.reason, Reason::Priority);
    let viewer = priority.player;
    let units = root.observation_work_units(viewer).unwrap() + 1;
    let QueryResult::Incomplete { work, .. } =
        root.search_context(viewer, "formats".into(), Budget::new(units))
    else {
        panic!("search should suspend after capturing an observation");
    };
    let search = root.save_work(&work).unwrap();
    root.restore_work(&search).unwrap();
    let mut saved: Value = serde_json::from_str(&search).unwrap();
    let task = &mut saved["data"]["task"];
    assert!(task["observation"].is_object());
    task["observation"]["decision"] = decision.clone();
    task["builder"]["decision"] = decision.clone();
    serde_json::to_string(&saved).unwrap()
}

/// A suspended observation keeps the counter predicate in its target offer.
fn work_with_counter_target() -> String {
    use mtg_contracts::kir::{Atom, Cmp, Pred};

    let mut catalog = FixtureCatalog::Base.get().clone();
    let spell = catalog
        .definitions
        .iter_mut()
        .find(|definition| definition.name == "Unsummon")
        .unwrap();
    spell.source_labels = None;
    let target = &mut spell.spell.as_mut().unwrap().targets[0];
    target.domain = TargetDomain::BattlefieldPermanent;
    let mut filter =
        mtg_contracts::PermanentFilter::any().with_types([mtg_contracts::CardKind::Creature]);
    filter.predicate = Some(Pred(vec![Atom::Counters {
        counter: None,
        cmp: Cmp::Exactly,
        value: 0.into(),
    }]));
    target.filter = Some(filter);
    spell.capabilities = spell.requirements();
    let mut game = Game::new(
        Board::new(catalog, "formats-counter-target")
            .step(TurnStep::PrecombatMain)
            .permanent_or_hand("Unsummon", 0, 0, true)
            .permanent_or_hand("Grizzly Bears", 0, 0, false)
            .permanent_or_hand("Island", 0, 0, false)
            .prepare(),
    );
    Driver::cast(&mut game, CardId(0));
    let offer = Driver::request(&mut game, u64::MAX);
    assert!(matches!(offer.reason, Reason::Targets { .. }));
    let QueryResult::Incomplete { work, .. } = game.observe(offer.player, Budget::new(1)) else {
        panic!("target observation should suspend");
    };
    let raw = game.save_work(&work).unwrap();
    assert!(raw.contains(r#""atom":"counters""#));
    game.restore_work(&raw).unwrap();
    raw
}

#[test]
fn saved_work_shape_fingerprint_is_pinned_to_work_version() {
    // Suspended observation and decision queries; a query finishing within its
    // budget has no work to save.
    let mut samples = vec![];
    for game in representatives() {
        for viewer in [PlayerId(0), PlayerId(1), PlayerId(2), PlayerId(3)] {
            for result in [
                game.observe(viewer, Budget::new(1)),
                game.player_decision_for(viewer, Budget::new(1)),
            ] {
                if let QueryResult::Incomplete { work, .. } = result {
                    samples.push(game.save_work(&work).unwrap());
                }
            }
        }
    }
    let game = stack_target_and_shields();
    let units = 6 + game.state.rules.objects.len() + game.state.rules.stack.len();
    let QueryResult::Incomplete { work, .. } = game.observe(PlayerId(0), Budget::new(units as u64))
    else {
        panic!("observation should suspend after the first regeneration shield");
    };
    let raw = game.save_work(&work).unwrap();
    let saved: Value = serde_json::from_str(&raw).unwrap();
    let builder = &saved["data"]["task"]["builder"];
    assert_eq!(builder["decision"]["candidates"][0]["candidate"], "stack");
    assert_eq!(
        builder["view"]["effects"][0]["effect"],
        "regeneration_shield"
    );
    game.restore_work(&raw).unwrap();
    samples.push(raw);
    samples.push(search_with_blocker_limit());
    samples.push(work_with_counter_target());
    let saga = saga_chapter();
    let viewer = PlayerId(0);
    let units = saga.observation_work_units(viewer).unwrap();
    let QueryResult::Incomplete { work, .. } = saga.observe(viewer, Budget::new(units - 1)) else {
        panic!("Saga observation should suspend");
    };
    let raw = saga.save_work(&work).unwrap();
    assert!(raw.contains(r#""counter":"lore""#));
    saga.restore_work(&raw).unwrap();
    samples.push(raw);
    for raw in &samples {
        let value: Value = serde_json::from_str(raw).unwrap();
        assert_eq!(value["work_version"], WORK_VERSION);
    }
    check(
        "SavedWork",
        "WORK_VERSION",
        WORK_VERSION,
        WORK_PIN,
        &shape_digest(&samples),
    );
}
