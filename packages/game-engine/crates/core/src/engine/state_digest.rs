//! Canonical State bytes with bounded history expansion. The existing contract
//! is serde_json::to_value followed by compact serialization (lexical object
//! keys, ordered arrays). Serializing State directly would change struct-key
//! order. Keep a Value for the history-free skeleton and for just one original
//! history entry at a time; never drop or summarize a remembered observation.
use super::*;
use std::io::{BufWriter, Write};

const HASH_BUFFER_BYTES: usize = 64 * 1024;
type ObjectBytes = BTreeMap<String, (visibility::VisibleObject, Vec<u8>)>;

fn json<W: Write, T: Serialize>(writer: &mut W, value: &T) -> Result<(), String> {
    serde_json::to_writer(writer, value).map_err(|e| e.to_string())
}
fn raw<W: Write>(writer: &mut W, bytes: &[u8]) -> Result<(), String> {
    writer.write_all(bytes).map_err(|e| e.to_string())
}
fn object_entries(value: &serde_json::Value) -> Result<Vec<(&String, &serde_json::Value)>, String> {
    let mut entries = value
        .as_object()
        .ok_or("canonical state object invariant")?
        .iter()
        .collect::<Vec<_>>();
    entries.sort_unstable_by(|a, b| a.0.cmp(b.0));
    Ok(entries)
}
fn write_view<W: Write>(
    writer: &mut W,
    view: &visibility::PermittedView,
    previous: &mut ObjectBytes,
) -> Result<(), String> {
    // Keep every serialized field dynamically. Only object rows get a local
    // accelerator, whose full typed equality witness includes all row fields.
    let mut skeleton = view.clone();
    skeleton.objects.clear();
    let value = serde_json::to_value(&skeleton).map_err(|e| e.to_string())?;
    drop(skeleton);
    let mut objects_seen = false;
    let mut next = ObjectBytes::new();
    raw(writer, b"{")?;
    for (index, (key, value)) in object_entries(&value)?.into_iter().enumerate() {
        if index != 0 {
            raw(writer, b",")?;
        }
        json(writer, key)?;
        raw(writer, b":")?;
        if key != "objects" {
            json(writer, value)?;
            continue;
        }
        if !value.as_array().is_some_and(Vec::is_empty) {
            return Err("canonical view skeleton still contains objects".into());
        }
        objects_seen = true;
        raw(writer, b"[")?;
        // Preserve the original row order, including malformed duplicate
        // handles in an untrusted envelope. Removing a cache entry permits at
        // most one reuse per key; every duplicate is still emitted separately.
        for (index, object) in view.objects.iter().enumerate() {
            if index != 0 {
                raw(writer, b",")?;
            }
            let cached = match previous.remove(&object.handle.0) {
                Some(cached) if cached.0 == *object => cached,
                _ => {
                    let value = serde_json::to_value(object).map_err(|e| e.to_string())?;
                    let bytes = serde_json::to_vec(&value).map_err(|e| e.to_string())?;
                    (object.clone(), bytes)
                }
            };
            raw(writer, &cached.1)?;
            next.insert(object.handle.0.clone(), cached);
        }
        raw(writer, b"]")?;
    }
    raw(writer, b"}")?;
    if !objects_seen {
        return Err("canonical view objects field missing".into());
    }
    // Only the current view's rows survive: removed handles/incarnations never
    // accumulate with history length, and caches never cross viewers/digests.
    *previous = next;
    Ok(())
}
fn write_knowledge<W: Write>(
    writer: &mut W,
    skeleton: &serde_json::Value,
    knowledge: &visibility::Knowledge,
) -> Result<(), String> {
    let mut history_seen = false;
    raw(writer, b"{")?;
    for (index, (key, value)) in object_entries(skeleton)?.into_iter().enumerate() {
        if index != 0 {
            raw(writer, b",")?;
        }
        json(writer, key)?;
        raw(writer, b":")?;
        if key == "history" {
            if !value.as_array().is_some_and(Vec::is_empty) {
                return Err("canonical knowledge skeleton still contains history".into());
            }
            history_seen = true;
            raw(writer, b"[")?;
            let mut objects = ObjectBytes::new();
            for (index, view) in knowledge.history.iter().enumerate() {
                if index != 0 {
                    raw(writer, b",")?;
                }
                write_view(writer, view, &mut objects)?;
            }
            raw(writer, b"]")?;
        } else {
            json(writer, value)?;
        }
    }
    raw(writer, b"}")?;
    if !history_seen {
        return Err("canonical knowledge history field missing".into());
    }
    Ok(())
}
pub(super) fn write_state<W: Write>(state: &State, writer: &mut W) -> Result<(), String> {
    // History clones share immutable chunks. Clear only these cloned histories
    // before Value conversion; the real histories and their caches are untouched.
    let mut skeleton = state.clone();
    for knowledge in &mut skeleton.visibility {
        knowledge.history.clear();
    }
    let value = serde_json::to_value(&skeleton).map_err(|e| e.to_string())?;
    drop(skeleton);
    let mut visibility_seen = false;
    raw(writer, b"{")?;
    for (index, (key, value)) in object_entries(&value)?.into_iter().enumerate() {
        if index != 0 {
            raw(writer, b",")?;
        }
        json(writer, key)?;
        raw(writer, b":")?;
        if key == "visibility" {
            visibility_seen = true;
            let rows = value
                .as_array()
                .ok_or("canonical visibility array invariant")?;
            if rows.len() != state.visibility.len() {
                return Err("canonical visibility length invariant".into());
            }
            raw(writer, b"[")?;
            for (index, (row, knowledge)) in rows.iter().zip(&state.visibility).enumerate() {
                if index != 0 {
                    raw(writer, b",")?;
                }
                write_knowledge(writer, row, knowledge)?;
            }
            raw(writer, b"]")?;
        } else {
            json(writer, value)?;
        }
    }
    raw(writer, b"}")?;
    if !visibility_seen {
        return Err("canonical State visibility field missing".into());
    }
    Ok(())
}
fn write_buffered_state<W: Write>(state: &State, sink: &mut W) -> Result<(), String> {
    // Serializer fragments are often single bytes. Buffer only the byte
    // sink, preserving every canonical byte while batching SHA updates.
    let mut writer = BufWriter::with_capacity(HASH_BUFFER_BYTES, sink);
    write_state(state, &mut writer)?;
    writer.flush().map_err(|e| e.to_string())
}
pub(super) fn state_digest(state: &State) -> Result<String, String> {
    let mut sink = Sha256Sink::new();
    write_buffered_state(state, &mut sink)?;
    Ok(sink.finalize_hex())
}

#[cfg(test)]
#[path = "state_digest_cache_tests.rs"]
mod cache_tests;

#[cfg(test)]
mod tests {
    use super::*;
    fn check(game: &Game) {
        let expected = canonical(&game.state).unwrap();
        let mut actual = vec![];
        write_state(&game.state, &mut actual).unwrap();
        assert_eq!(actual, expected.as_bytes());
        assert_eq!(game.state_digest().unwrap(), sha256(expected.as_bytes()));
    }
    fn prepared() -> Prepared {
        Prepared::admit_pod(
            Catalog::read(include_str!(
                "../../../../tests/fixtures/expected-match-ir.json"
            ))
            .unwrap(),
            MatchConfig::foundations([0; 32], PlayerId(0)),
        )
        .unwrap()
    }
    #[test]
    fn canonical_state_bytes_match_all_historical_profile_families_and_match_setup() {
        let groups = [
            (
                include_str!("../../../../tests/fixtures/expected-ir.json"),
                vec![
                    ScenarioConfig::land_return(),
                    ScenarioConfig::commander_foundation(),
                    ScenarioConfig::stack_response(),
                    ScenarioConfig::casting_reversal(),
                    ScenarioConfig::turns_permanents(),
                    ScenarioConfig::combat_game(),
                    ScenarioConfig::combat_interactions(),
                ],
            ),
            (
                include_str!("../../../../tests/fixtures/expected-commander-ir.json"),
                vec![
                    ScenarioConfig::command_casting(),
                    ScenarioConfig::commander_lifecycle(),
                    ScenarioConfig::commander_exile(),
                ],
            ),
            (
                include_str!("../../../../tests/fixtures/expected-damage-ir.json"),
                vec![
                    ScenarioConfig::commander_damage(),
                    ScenarioConfig::commander_damage_borrowed(),
                ],
            ),
            (
                include_str!("../../../../tests/fixtures/expected-library-ir.json"),
                vec![ScenarioConfig::commander_library()],
            ),
            (
                include_str!("../../../../tests/fixtures/expected-action-ir.json"),
                vec![
                    ScenarioConfig::actions(),
                    ScenarioConfig::actions_borrowed(),
                    ScenarioConfig::actions_empty_library(),
                    ScenarioConfig::mixed_mana(),
                ],
            ),
            (
                include_str!("../../../../tests/fixtures/expected-stack-ir.json"),
                vec![
                    ScenarioConfig::stack_costs(),
                    ScenarioConfig::stack_costs_borrowed(),
                    ScenarioConfig::stack_costs_empty_library(),
                    ScenarioConfig::stack_costs_library(),
                ],
            ),
            (
                include_str!("../../../../tests/fixtures/expected-damage-life-ir.json"),
                vec![
                    ScenarioConfig::damage_life(),
                    ScenarioConfig::damage_life_low(),
                    ScenarioConfig::damage_life_borrowed(),
                    ScenarioConfig::damage_life_empty_library(),
                    ScenarioConfig::damage_life_library(),
                ],
            ),
            (
                include_str!("../../../../tests/fixtures/expected-trigger-ir.json"),
                vec![
                    ScenarioConfig::triggers(),
                    ScenarioConfig::triggers_low(),
                    ScenarioConfig::triggers_borrowed(),
                    ScenarioConfig::triggers_empty_library(),
                    ScenarioConfig::triggers_nonactive(),
                ],
            ),
            (
                include_str!("../../../../tests/fixtures/expected-effects-ir.json"),
                vec![
                    ScenarioConfig::effects(),
                    ScenarioConfig::effects_furnace(),
                    ScenarioConfig::effects_two_furnaces(),
                    ScenarioConfig::effects_borrowed(),
                    ScenarioConfig::effects_low(),
                ],
            ),
            (
                include_str!("../../../../tests/fixtures/expected-choices-ir.json"),
                vec![
                    ScenarioConfig::choices(),
                    ScenarioConfig::choices_furnace(),
                    ScenarioConfig::conditions(),
                    ScenarioConfig::conditions_nonempty(),
                    ScenarioConfig::conditions_borrowed(),
                    ScenarioConfig::conditions_no_targets(),
                ],
            ),
        ];
        for (catalog, configs) in groups {
            for config in configs {
                check(&Game::new(
                    Prepared::admit(Catalog::read(catalog).unwrap(), config).unwrap(),
                ));
            }
        }
        let mut game = Game::new(prepared());
        check(&game);
        let _ = game.advance(Budget::new(37));
        check(&game);
        let _ = game.advance(Budget::unlimited());
        check(&game);
    }
    #[test]
    fn every_mutation_preserves_nested_canonical_order_escaping_and_complete_history() {
        let mut game = Game::new(prepared());
        let _ = game.advance(Budget::unlimited());
        let original = game.state.clone();
        let trace = game.trace.clone();
        // Numeric keys 1,2,10,100 must follow JSON lexical ordering, not numeric
        // BTreeMap order. Non-ASCII, control/quote/backslash escaping is exact.
        for i in 0..70 {
            let mut view = game.state.visibility[0].history.last().unwrap().clone();
            view.objects[0].oracle_id = format!("\"\\\n\u{0000}é🦀/{i}");
            view.players[0].life = Quantity::from(-(i + 1));
            game.state.visibility[0].history.push(view);
        }
        check(&game);
        let mut bytes = vec![];
        write_state(&game.state, &mut bytes).unwrap();
        let value: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(
            value["visibility"][0]["history"].as_array().unwrap().len(),
            71
        );
        assert_eq!(game.trace, trace);
        assert_eq!(game.state.visibility[1], original.visibility[1]);
        let before = game.state.clone();
        check(&game);
        assert_eq!(game.state, before);
        struct Reject;
        impl Write for Reject {
            fn write(&mut self, _: &[u8]) -> std::io::Result<usize> {
                Err(std::io::Error::other("test sink failure"))
            }
            fn flush(&mut self) -> std::io::Result<()> {
                Ok(())
            }
        }
        assert!(write_state(&game.state, &mut Reject).is_err());
        assert_eq!(game.state, before);
    }
    #[test]
    fn present_optional_effects_and_all_knowledge_maps_match_old_bytes() {
        let mut game = Game::new(prepared());
        let _ = game.advance(Budget::unlimited());
        // Serialization-domain fixture: make every formerly skipped Knowledge
        // field present, without making invalid metadata into a restored game.
        let mut value = serde_json::to_value(&game.state.visibility[0]).unwrap();
        value["effect_handles"] =
            serde_json::json!([[{"effect":"resolved","id":2},"effect\\handle"]]);
        value["next_effect_handle"] = 3.into();
        value["damage_handles"] = serde_json::json!([[[4, 5], "damage\nhandle"]]);
        value["next_damage_handle"] = 6.into();
        value["trigger_handles"] = serde_json::json!({"2":"trigger2","10":"trigger10"});
        value["next_trigger_handle"] = 11.into();
        value["ability_handles"] = serde_json::json!({"2":"ability2","10":"ability10"});
        value["next_ability_handle"] = 11.into();
        game.state.visibility[0] = serde_json::from_value(value).unwrap();
        game.state.effects.push(ResolvedEffect {
            id: EffectId(2),
            origin: EffectOrigin {
                mode: Some(1),
                stack_id: StackId(3),
                source: ObjectId {
                    card: CardId(0),
                    incarnation: 0,
                },
                controller: PlayerId(0),
                source_handles: vec![ObjectHandle("historical\nsource".into())],
                cursor: 2,
                source_block: 1,
            },
            target_handles: vec![],
            expires_in_cleanup_of_turn: 10,
            kind: ResolvedEffectKind::PreventCombatDamage,
        });
        game.state.next_effect_id = 3;
        game.state.next_trigger_id = 11;
        game.state.next_damage_transaction_id = 8;
        check(&game);
    }
    #[test]
    fn actual_pending_inputs_midgame_departures_and_recovery_match_old_digest() {
        let p = Prepared::admit(
            Catalog::read(include_str!("../../../../tests/fixtures/expected-ir.json")).unwrap(),
            ScenarioConfig::combat_game(),
        )
        .unwrap();
        let mut game = Game::with_trace(p.clone(), TraceMode::Off);
        for index in 0..90 {
            check(&game);
            match game.advance(Budget::new(1)) {
                Advance::AwaitingDecision(r) => {
                    let response = match &r {
                        Request::Priority { .. } => Response::Pass,
                        Request::Attackers { .. } => Response::Attackers { attacks: vec![] },
                        Request::Discard {
                            count, candidates, ..
                        } => Response::Discard {
                            objects: candidates.iter().take(*count).copied().collect(),
                        },
                        _ => panic!("unexpected {r:?}"),
                    };
                    game.respond(r.player(), r.decision_id(), response).unwrap();
                    check(&game);
                }
                Advance::Suspended { .. } => {}
                Advance::Finished { .. } | Advance::Draw => break,
                x => panic!("{x:?}"),
            };
            if index == 40 {
                game.concede(PlayerId(3)).unwrap();
            }
        }
        let restored = Game::restore(p, &game.snapshot().unwrap()).unwrap();
        check(&restored);
        assert_eq!(restored.state, game.state);
    }
}
