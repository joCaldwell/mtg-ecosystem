//! Exact frozen R4 records exercise both refreshed operand graphs and row pins.
use super::*;
use serde_json::{Value, json};

const R4: &str = include_str!("../../../tests/fixtures/archive-manifest-phase1-r4.json");

#[test]
fn refreshed_binding_graphs_consume_exact_semantics() {
    for (raw, capability, node, key, bad) in [
        (
            include_str!("../../../tests/fixtures/phase1-r4/bond-land.json"),
            Capability::ConditionalOpponentCountEntryV1,
            "player-state-condition",
            "who",
            "another-controller",
        ),
        (
            include_str!("../../../tests/fixtures/phase1-r4/distinct-color-mana.json"),
            Capability::DistinctPermanentColorsV1,
            "characteristic-subject",
            "of",
            "another-object",
        ),
        (
            include_str!("../../../tests/fixtures/phase1-r4/fixed-opponent-drain.json"),
            Capability::OpponentLifeDrainV1,
            "event-aggregate",
            "event",
            "another-event",
        ),
        (
            include_str!("../../../tests/fixtures/phase1-r4/announced-x-opponent-drain.json"),
            Capability::OpponentLifeDrainV1,
            "event-aggregate",
            "measure",
            "life-gained",
        ),
    ] {
        for header in [R4] {
            let result = compile(header, raw);
            let CompileResult::Lowered { definition } = result else {
                panic!("{result:?}");
            };
            assert!(definition.capabilities.contains(&capability));
        }
        let forged = tests::changed_source(raw, |row| {
            let changed = row["binding"]["nodes"]
                .as_array_mut()
                .unwrap()
                .iter_mut()
                .find(|n| n["node"] == node)
                .unwrap();
            changed[key] = json!(bad);
        });
        assert!(matches!(
            compile(R4, &forged),
            CompileResult::Unsupported { path, .. } if path.starts_with("binding")
        ));
    }
}

#[test]
fn refreshed_exact_rows_use_their_own_archived_provenance() {
    for (raw, capability) in [
        (
            include_str!("../../../tests/fixtures/phase1-r4/library-top-mana.json"),
            Capability::LookTopMayRevealForManaV1,
        ),
        (
            include_str!("../../../tests/fixtures/phase1-r4/graveyard-replacement.json"),
            Capability::SelfGraveyardRevealShuffleV1,
        ),
        (
            include_str!("../../../tests/fixtures/phase1-r4/entry-copy.json"),
            Capability::EnterAsCopyOfPermanentEnteredThisTurnV1,
        ),
    ] {
        for header in [R4] {
            let result = compile(header, raw);
            let CompileResult::Lowered { definition } = result else {
                panic!("{result:?}");
            };
            assert_eq!(
                definition.provenance.archive_manifest_sha256,
                accepted_manifest(header).unwrap().manifest_sha256
            );
            assert!(definition.capabilities.contains(&capability));
            let forged = format!("{{ {}", raw.strip_prefix('{').unwrap());
            assert!(matches!(
                compile(header, &forged),
                CompileResult::Unsupported { path, .. } | CompileResult::Invalid { path, .. }
                    if path == "record"
            ));
        }
    }
}

/// One triggered block whose "that player" resolves to an event player.
fn event_player_block(role: &str, scope: Option<&str>) -> resolution::Resolution {
    let mut entry =
        json!({"path":"/effects/0/steps/0/who","ref":{"ref":"event-player","role":role}});
    if let Some(scope) = scope {
        entry["scope"] = json!(scope);
    }
    let card = json!({"artifactKind":"oracle-resolution","schemaVersion":1,"faces":[{"sourceFaceIndex":0,
        "blocks":[{"refs":[entry],"steps":[],"spans":{"unsupported":"synthetic"},"engineFeatures":[],
        "residuals":[]}]}],"residuals":[],"complete":true})
    .to_string();
    let raw = serde_json::value::RawValue::from_string(card.clone()).unwrap();
    resolution::Resolution::read(Some(raw.as_ref()), Some(&sha256(card.as_bytes()))).unwrap()
}

#[test]
fn kernel_event_players_belong_to_the_trigger_that_supplies_them() {
    let block = |trigger: Value| {
        json!([{"kind":"triggered","trigger":trigger,"effects":[{"steps":[{"effect":"lose-life",
            "who":{"player":"that-player"},"amount":{"amount":"fixed","value":1}}]}]}])
    };
    let upkeep = block(json!({"trigger":"phase","phase":"upkeep","whose":"each-player"}));
    let cast = block(json!({"trigger":"cast","who":{"player":"an-opponent"}}));
    let draws = block(json!({"trigger":"draws","who":{"player":"an-opponent"}}));
    // CR 102.1, 603.2: the binder's spelling is the kernel role the trigger supplies.
    for (role, trigger, condition) in [
        (
            "active-player",
            "phase",
            TriggerCondition::StepBegan {
                step: BeginningStep::Upkeep,
                turn_relation: TurnRelation::AnyPlayer,
            },
        ),
        (
            "caster",
            "cast",
            TriggerCondition::SpellCast {
                caster: CasterRelation::Opponent,
                spell: TypePredicate::Any {},
            },
        ),
    ] {
        let (kernel, supplier) = resolution::kernel_event_player(role).unwrap();
        assert_eq!(
            (Some(kernel), supplier),
            (kir::event_player_role(&condition), trigger)
        );
    }
    assert_eq!(
        resolution::kernel_event_player("activating-player").map(|(role, _)| role),
        Some(kir::EventPlayerRole::ActivatingPlayer)
    );
    assert!(
        event_player_block("active-player", None)
            .ordinary_antecedents(&upkeep)
            .is_ok()
    );
    assert!(
        event_player_block("caster", None)
            .ordinary_antecedents(&cast)
            .is_ok()
    );
    // A kernel role under another trigger, or borrowed from a nested ability.
    for (role, scope, ability) in [
        ("active-player", None, &cast),
        ("caster", None, &upkeep),
        ("active-player", Some("/effects/0/steps/0"), &upkeep),
        ("activating-player", None, &draws),
    ] {
        assert!(
            matches!(
                event_player_block(role, scope).ordinary_antecedents(ability),
                Err(f) if f.path == "resolution"
            ),
            "{role} {scope:?}"
        );
    }
    // An event player the kernel does not name keeps its trigger discriminant;
    // the lowerer that reads it decides (composition reads no such role yet).
    assert!(
        event_player_block("draws", None)
            .ordinary_antecedents(&draws)
            .is_ok()
    );
    assert_eq!(resolution::kernel_event_player("draws"), None);
}
