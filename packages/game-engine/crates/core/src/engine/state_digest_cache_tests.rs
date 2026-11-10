use super::*;
use std::io;

fn fixture() -> Game {
    Game::new(
        Prepared::admit(
            Catalog::read(include_str!(
                "../../../../tests/fixtures/expected-effects-ir.json"
            ))
            .unwrap(),
            ScenarioConfig::effects_two_furnaces(),
        )
        .unwrap(),
    )
}
fn base_view() -> visibility::PermittedView {
    let game = fixture();
    let mut view = game.state.knowledge.visibility[0]
        .history
        .last()
        .unwrap()
        .to_public();
    let object = view
        .objects
        .iter()
        .find(|object| {
            object.owner == PlayerId(0)
                && object
                    .characteristics
                    .as_ref()
                    .is_some_and(|c| c.kind == CardKind::Creature)
        })
        .unwrap()
        .clone();
    view.objects = vec![object];
    view
}
fn check_views(views: &[visibility::PermittedView]) {
    let original = views.to_vec();
    let mut stored = stored_history::StoredHistory::default();
    for view in views {
        stored.push_public(view.clone());
    }
    let mut segmented = vec![b'['];
    for (index, view) in stored.iter().enumerate() {
        if index != 0 {
            segmented.push(b',');
        }
        view.write_canonical(&mut segmented).unwrap();
    }
    segmented.push(b']');
    assert_eq!(segmented, canonical(&views).unwrap().as_bytes());
    let mut previous = ObjectBytes::new();
    let mut bytes = vec![];
    raw(&mut bytes, b"[").unwrap();
    for (index, view) in views.iter().enumerate() {
        if index != 0 {
            raw(&mut bytes, b",").unwrap();
        }
        write_view(&mut bytes, view, &mut previous).unwrap();
        assert!(previous.len() <= view.objects.len());
        assert!(
            previous
                .keys()
                .all(|key| view.objects.iter().any(|object| &object.handle.0 == key))
        );
    }
    raw(&mut bytes, b"]").unwrap();
    assert_eq!(bytes, canonical(&views).unwrap().as_bytes());
    assert_eq!(sha256(&bytes), digest(&views).unwrap());
    assert_eq!(views, original);
}
#[test]
fn every_object_field_change_invalidates_only_exact_equal_row_bytes() {
    type Mutation = fn(&mut visibility::VisibleObject);
    let mutations: [(&str, Mutation); 19] = [
        ("marked damage", |o| o.marked_damage = (-123456789).into()),
        ("attack ready", |o| o.attack_ready = !o.attack_ready),
        ("handle", |o| o.handle.0 = "v0-雪\u{0}\"\\o".into()),
        ("oracle", |o| o.oracle_id = "oracle\n\u{0}é🦀".into()),
        ("owner", |o| o.owner = PlayerId(3)),
        ("controller", |o| o.controller = PlayerId(2)),
        ("zone", |o| o.zone = Zone::Library { owner: PlayerId(3) }),
        ("tap", |o| o.tapped = !o.tapped),
        ("nullable characteristics", |o| o.characteristics = None),
        ("kind", |o| {
            o.characteristics.as_mut().unwrap().kind = CardKind::Land
        }),
        ("subtypes", |o| {
            o.characteristics
                .as_mut()
                .unwrap()
                .subtypes
                .push("雪\u{0}\n".into())
        }),
        ("basic", |o| {
            let c = o.characteristics.as_mut().unwrap();
            c.basic = !c.basic
        }),
        ("legendary", |o| {
            let c = o.characteristics.as_mut().unwrap();
            c.legendary = !c.legendary
        }),
        ("mana generic", |o| {
            o.characteristics.as_mut().unwrap().mana_cost.generic = 999.into()
        }),
        ("mana colors", |o| {
            o.characteristics.as_mut().unwrap().mana_cost.colored = vec![Color::R, Color::B]
        }),
        ("colors", |o| {
            o.characteristics.as_mut().unwrap().colors = vec![Color::R]
        }),
        ("color identity", |o| {
            o.characteristics.as_mut().unwrap().color_identity = vec![Color::U]
        }),
        ("power nullable", |o| {
            o.characteristics.as_mut().unwrap().power = None
        }),
        ("toughness nullable", |o| {
            o.characteristics.as_mut().unwrap().toughness = None
        }),
    ];
    let base = base_view();
    assert!(!base.effects.is_empty());
    for (name, mutate) in mutations {
        let mut changed = base.clone();
        mutate(&mut changed.objects[0]);
        assert_ne!(changed, base, "mutation {name}");
        check_views(&[
            base.clone(),
            base.clone(),
            changed.clone(),
            changed,
            base.clone(),
        ]);
    }
    let mut changed = base.clone();
    changed.objects[0].characteristics.as_mut().unwrap().power = Some((-999).into());
    changed.objects[0]
        .characteristics
        .as_mut()
        .unwrap()
        .toughness = Some(1234567.into());
    check_views(&[base.clone(), changed, base]);
}
#[test]
fn row_removal_reinsertion_reorder_duplicates_and_other_view_fields_preserve_all_bytes() {
    let first = base_view();
    let mut second = first.objects[0].clone();
    second.handle.0 = "v0-reincarnation\u{0}".into();
    second.oracle_id = "second".into();
    let mut two = first.clone();
    two.objects.push(second.clone());
    let mut reordered = two.clone();
    reordered.objects.reverse();
    let mut duplicate = two.clone();
    duplicate.objects[1].handle = duplicate.objects[0].handle.clone();
    duplicate.objects[1].marked_damage = 321.into();
    let mut same_duplicate = duplicate.clone();
    same_duplicate.objects[1] = same_duplicate.objects[0].clone();
    let mut empty = two.clone();
    empty.objects.clear();
    let mut other_fields = two.clone();
    other_fields.active = None;
    other_fields.turn_owner = PlayerId(2);
    other_fields.priority = None;
    other_fields.viewer = PlayerId(3);
    other_fields.turn.number = 100;
    other_fields.players[0].life = (-7).into();
    other_fields.players[0].mana.insert(Color::B, 17.into());
    other_fields.effects.reverse();
    other_fields.commanders.reverse();
    check_views(&[
        first,
        two.clone(),
        reordered,
        duplicate.clone(),
        same_duplicate,
        duplicate,
        empty,
        two,
        other_fields,
    ]);
}
#[test]
fn all_state_history_rows_stay_ordered_complete_and_hash_equivalent_after_cache_transitions() {
    let mut game = fixture();
    for viewer in 0..4 {
        let base = game.state.knowledge.visibility[viewer]
            .history
            .last()
            .unwrap()
            .to_public();
        for index in 0..70 {
            let mut view = base.clone();
            if index % 4 == 1 {
                view.objects.reverse();
            }
            if index % 4 == 2 {
                view.objects.clear();
            }
            if index % 4 == 3 {
                view.objects[0].oracle_id = format!("v{viewer}x{index}:\n雪\u{0}");
            }
            game.state.knowledge.visibility[viewer]
                .history
                .push_public(view);
        }
    }
    let original = game.state.clone();
    let expected = canonical(&game.state).unwrap();
    let mut actual = vec![];
    write_state(&game.state, &mut actual).unwrap();
    assert_eq!(actual, expected.as_bytes());
    assert_eq!(game.state_digest().unwrap(), sha256(expected.as_bytes()));
    assert_eq!(
        game.fork().state_digest().unwrap(),
        game.state_digest().unwrap()
    );
    assert_eq!(game.state, original);
}
#[test]
fn mixed_generic_warm_and_typed_cold_entries_keep_all_fields_and_stale_row_cache_exact() {
    // Serialization-domain fixture only. Player keys10/2 and duplicate handles
    // must preserve their complete lexical/object-array form, even though they
    // would be rejected as a reachable match history.
    let mut base = base_view();
    base.commanders[0].history.command_casts =
        [(PlayerId(2), 2.into()), (PlayerId(10), 10.into())].into();
    base.commanders[0].history.combat_damage =
        [(PlayerId(2), 21.into()), (PlayerId(10), 22.into())].into();
    let mut history = stored_history::StoredHistory::default();
    for index in 0..72 {
        let mut view = base.clone();
        view.turn.number = index + 1;
        if index % 5 == 1 {
            view.objects[0].marked_damage = (index as i64).into();
        } else if index % 5 == 2 {
            let mut duplicate = view.objects[0].clone();
            duplicate.oracle_id = format!("duplicate\n雪\\{index}");
            view.objects.push(duplicate);
        } else if index % 5 == 3 {
            view.objects.clear();
        }
        history.push_public(view);
    }
    // Generic entry bytes are a transient oracle now, not a complete View
    // memo. Segmented writer reuse must still preserve all these same rows.
    for index in (1..history.len()).step_by(3) {
        assert_eq!(
            history.canonical_entry_bytes(index).unwrap(),
            canonical(&history[index]).unwrap().as_bytes()
        );
    }
    let mut game = fixture();
    game.state.knowledge.visibility[0].history = history;
    let expected = canonical(&game.state).unwrap();
    let mut actual = vec![];
    write_state(&game.state, &mut actual).unwrap();
    assert_eq!(actual, expected.as_bytes());
    let mut branch = game.fork();
    branch.state.knowledge.visibility[0].history[33].replace_public_objects(base.objects.clone());
    branch.state.knowledge.visibility[0].history[33].mutate_object(0, |row| row.tapped = true);
    branch.state.knowledge.visibility[0]
        .history
        .push_public(base.clone());
    for state in [&game.state, &branch.state] {
        let expected = canonical(state).unwrap();
        let mut actual = vec![];
        write_state(state, &mut actual).unwrap();
        assert_eq!(actual, expected.as_bytes());
        assert_eq!(state_digest(state).unwrap(), sha256(expected.as_bytes()));
    }
    assert_eq!(game.state_digest().unwrap(), sha256(expected.as_bytes()));
}
#[test]
fn overlapping_cold_digest_initializers_and_warm_forks_match_complete_state_oracle() {
    let mut game = fixture();
    for viewer in 0..4 {
        let base = game.state.knowledge.visibility[viewer].history[0].to_public();
        for index in 0..70 {
            let mut view = base.clone();
            if index % 4 == 0 {
                view.objects[0].oracle_id = format!("cold-{viewer}-{index}\n雪\u{0}");
            }
            game.state.knowledge.visibility[viewer]
                .history
                .push_public(view);
        }
        for index in (0..game.state.knowledge.visibility[viewer].history.len()).step_by(7) {
            game.state.knowledge.visibility[viewer]
                .history
                .canonical_entry_bytes(index)
                .unwrap();
        }
    }
    let expected = canonical(&game.state).unwrap();
    let expected_sha = sha256(expected.as_bytes());
    let barrier = std::sync::Barrier::new(3);
    std::thread::scope(|scope| {
        for _ in 0..3 {
            let branch = game.fork();
            let (barrier, expected, expected_sha) = (&barrier, &expected, &expected_sha);
            scope.spawn(move || {
                barrier.wait();
                let mut actual = vec![];
                write_state(&branch.state, &mut actual).unwrap();
                assert_eq!(actual, expected.as_bytes());
                assert_eq!(branch.state_digest().unwrap(), *expected_sha);
            });
        }
    });
    assert_eq!(game.state_digest().unwrap(), expected_sha);
}
#[derive(Default)]
struct PartialSink {
    bytes: Vec<u8>,
    calls: usize,
    fail_after: Option<usize>,
    flush_error: bool,
}
impl Write for PartialSink {
    fn write(&mut self, input: &[u8]) -> io::Result<usize> {
        self.calls += 1;
        if self.calls.is_multiple_of(11) {
            return Err(io::Error::new(io::ErrorKind::Interrupted, "retry test"));
        }
        if self
            .fail_after
            .is_some_and(|limit| self.bytes.len() >= limit)
        {
            return Err(io::Error::other("injected write failure"));
        }
        let remaining = self
            .fail_after
            .map_or(usize::MAX, |limit| limit - self.bytes.len());
        let count = input.len().min(7).min(remaining);
        self.bytes.extend_from_slice(&input[..count]);
        Ok(count)
    }
    fn flush(&mut self) -> io::Result<()> {
        if self.flush_error {
            Err(io::Error::other("injected flush failure"))
        } else {
            Ok(())
        }
    }
}
#[test]
fn buffering_preserves_capacity_edges_partial_writes_interrupts_and_error_propagation() {
    // Serialization-domain fixture only; arbitrary Oracle strings are not
    // admitted or restored as cards. Exact lengths cross the64KiB boundary.
    let prepared = Prepared::admit(
        Catalog::read(include_str!("../../../../tests/fixtures/expected-ir.json")).unwrap(),
        ScenarioConfig::land_return(),
    )
    .unwrap();
    let mut game = Game::new(prepared);
    game.state.knowledge.visibility[0].history[0].mutate_object(0, |row| row.oracle_id.clear());
    let base_length = canonical(&game.state).unwrap().len();
    assert!(base_length < HASH_BUFFER_BYTES - 1);
    for target in [
        HASH_BUFFER_BYTES - 1,
        HASH_BUFFER_BYTES,
        HASH_BUFFER_BYTES + 1,
        HASH_BUFFER_BYTES * 2 - 1,
        HASH_BUFFER_BYTES * 2,
        HASH_BUFFER_BYTES * 2 + 1,
    ] {
        game.state.knowledge.visibility[0].history[0]
            .mutate_object(0, |row| row.oracle_id = "x".repeat(target - base_length));
        let before = game.state.clone();
        let expected = canonical(&game.state).unwrap();
        assert_eq!(expected.len(), target);
        let mut partial = PartialSink::default();
        write_buffered_state(&game.state, &mut partial).unwrap();
        assert_eq!(partial.bytes, expected.as_bytes());
        assert_eq!(game.state_digest().unwrap(), sha256(expected.as_bytes()));
        for limit in [0, 1, HASH_BUFFER_BYTES - 1, HASH_BUFFER_BYTES, target - 1] {
            if limit >= target {
                continue;
            }
            let mut fail = PartialSink {
                fail_after: Some(limit),
                ..Default::default()
            };
            assert!(
                write_buffered_state(&game.state, &mut fail)
                    .unwrap_err()
                    .contains("injected write failure")
            );
            assert_eq!(fail.bytes, &expected.as_bytes()[..limit]);
        }
        let mut flush = PartialSink {
            flush_error: true,
            ..Default::default()
        };
        assert!(
            write_buffered_state(&game.state, &mut flush)
                .unwrap_err()
                .contains("injected flush failure")
        );
        assert_eq!(flush.bytes, expected.as_bytes());
        assert_eq!(game.state, before);
    }
}

#[test]
fn memoized_complete_views_preserve_every_field_through_fork_mutation_and_old_reference() {
    // Serialization-domain fixtures, not admitted game histories: populate all
    // optional fields, then mutate each root field independently after warming.
    let mut value = serde_json::to_value(base_view()).unwrap();
    value["setup"] = serde_json::json!({"round":2,"phase":"declare","players":[{"player":0,"kept":false,"mulligans_taken":2}]});
    value["effects"] = serde_json::json!([{"effect":"prevent_combat_damage","handle":"雪\\e","origin":{"source":{"announced":"a","current":null},"oracle_id":"oracle\n雪","controller":0},"expires_in_cleanup_of_turn":9}]);
    value["combat"] =
        serde_json::json!([{"attacker":"a","defender":1,"blocked":true,"blockers":["b"]}]);
    value["commanders"] = serde_json::json!([{"designation":"c","oracle_id":"oracle","history":{"owner":0,"color_identity":["W"],"command_casts":{"0":"2"},"combat_damage":{"1":"21"}}}]);
    value["stack"] = serde_json::json!([{"mode":1,"handle":"s","spell":"a","ability":null,"controller":0,"cast_complete":true,"targets":[{"announced":"old","current":null}],"entity_targets":[{"entity":"player","player":2}],"stack_targets":[{"announced":"s2","current":null}],"player_targets":[3]}]);
    value["active"] = 0.into();
    value["priority"] = 0.into();
    let rich: visibility::PermittedView = serde_json::from_value(value.clone()).unwrap();
    let alternatives = [
        ("setup", serde_json::Value::Null),
        ("effects", serde_json::json!([])),
        ("combat", serde_json::json!([])),
        ("viewer", serde_json::json!(3)),
        ("active", serde_json::Value::Null),
        ("turn_owner", serde_json::json!(3)),
        (
            "turn",
            serde_json::json!({"number":999,"step":"cleanup","lands_played":1}),
        ),
        ("live_turn_order", serde_json::json!([])),
        ("commanders", serde_json::json!([])),
        ("priority", serde_json::Value::Null),
        ("players", serde_json::json!([])),
        ("objects", serde_json::json!([])),
        ("stack", serde_json::json!([])),
    ];
    let fields = value
        .as_object()
        .unwrap()
        .keys()
        .cloned()
        .collect::<BTreeSet<_>>();
    assert_eq!(
        fields,
        alternatives
            .iter()
            .map(|(key, _)| key.to_string())
            .collect(),
        "new View fields need an explicit mutation case"
    );
    let mut game = fixture();
    game.state.knowledge.visibility[0]
        .history
        .push_public(rich.clone());
    let index = game.state.knowledge.visibility[0].history.len() - 1;
    let original_sha = game.state_digest().unwrap();
    let original_bytes = game.state.knowledge.visibility[0]
        .history
        .canonical_entry_bytes(index)
        .unwrap()
        .to_vec();
    for (field, replacement) in alternatives {
        let mut altered = value.clone();
        altered[field] = replacement;
        let changed: visibility::PermittedView = serde_json::from_value(altered).unwrap();
        assert_ne!(rich, changed, "mutation {field} is ineffective");
        let mut branch = game.fork();
        branch.state.knowledge.visibility[0].history[index] = changed.clone().into();
        check_views(&[rich.clone(), changed]); // Frozen former typed-object encoder.
        let expected = canonical(&branch.state).unwrap();
        let mut actual = vec![];
        write_state(&branch.state, &mut actual).unwrap();
        assert_eq!(actual, expected.as_bytes(), "field {field}");
        assert_eq!(
            branch.state_digest().unwrap(),
            sha256(expected.as_bytes()),
            "field {field}"
        );
        assert_ne!(
            branch.state_digest().unwrap(),
            original_sha,
            "field {field}"
        );
        assert_eq!(game.state_digest().unwrap(), original_sha);
        assert_eq!(
            game.state.knowledge.visibility[0]
                .history
                .canonical_entry_bytes(index)
                .unwrap(),
            original_bytes
        );
        // Rewarm then mutate the already-unique entry again.
        branch.state.knowledge.visibility[0].history[index] = rich.clone().into();
        assert_eq!(branch.state_digest().unwrap(), original_sha);
    }
}
