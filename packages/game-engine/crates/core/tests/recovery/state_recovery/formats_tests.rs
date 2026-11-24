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
//! printed mana ability, its rider's damage transaction and a kept receipt.
use super::*;
use mtg_contracts::{canonical, sha256};
use mtg_core::testkit::formats::*;
use serde_json::Value;
use std::collections::BTreeSet;

const OBSERVATION_PIN: (u32, &str) = (
    57,
    "069e4186a70e0a1d45a8a55a1ae2f679459462ac5ff4d4ddb98b7a25449433a2",
);
const SNAPSHOT_PIN: (u32, &str) = (
    86,
    "dcef367b6fdfe442e555a5b014a30db86829e467b3c9ce7cfdb44b406694ebe9",
);
const WORK_PIN: (u32, &str) = (
    53,
    "b33a47e5d01c6830ba59d896dd6b8a6cefaee639ad545e4ca709fe29258e78db",
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
        let selected_answer = game.next_test_decision().player_targets(vec![PlayerId(1)]);
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
    assert!(matches!(
        game.state.rules.frame,
        Frame::ResolutionChoice { .. }
    ));
    let restored =
        Game::restore(game.prepared.as_ref().clone(), &game.snapshot().unwrap()).unwrap();
    assert_eq!(
        restored.state_digest().unwrap(),
        game.state_digest().unwrap()
    );
    game
}

#[test]
fn snapshot_shape_fingerprint_is_pinned_to_snapshot_version() {
    let samples = representatives()
        .into_iter()
        .chain([
            mid_resolution(),
            super::kernel_control_tests::suspended_control(),
        ])
        .chain(mana_riders())
        .map(|g| g.snapshot().unwrap())
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
    assert!(!samples.is_empty(), "no suspended work to fingerprint");
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
