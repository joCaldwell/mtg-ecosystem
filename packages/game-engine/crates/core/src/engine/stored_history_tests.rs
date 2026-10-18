//! Exact serialization-domain and real-game correctness, never timing evidence.
use super::*;

fn public_fixture() -> PermittedView {
    let game = Game::new(
        Prepared::admit(
            Catalog::read(include_str!("../../../../tests/fixtures/expected-ir.json")).unwrap(),
            ScenarioConfig::land_return(),
        )
        .unwrap(),
    );
    game.state.visibility[0].history.last().unwrap().to_public()
}
fn row_pointer(row: &StoredObject) -> usize {
    Arc::as_ptr(&row.0) as usize
}
fn write_view(view: &StoredPermittedView) -> Vec<u8> {
    let mut bytes = vec![];
    view.write_canonical(&mut bytes).unwrap();
    bytes
}
fn assert_views(history: &StoredHistory, public: &[PermittedView]) {
    assert_eq!(history.len(), public.len());
    assert_eq!(
        serde_json::to_string(history).unwrap(),
        serde_json::to_string(public).unwrap()
    );
    assert_eq!(canonical(history).unwrap(), canonical(&public).unwrap());
    for (view, expected) in history.iter().zip(public) {
        assert_eq!(&view.to_public(), expected);
        assert_eq!(write_view(view), canonical(expected).unwrap().as_bytes());
    }
}
#[test]
fn stored_history_shares_only_equal_previous_same_viewer_rows_and_keeps_all_ordered_values() {
    let first = public_fixture();
    assert!(!first.objects.is_empty());
    let mut unchanged = first.clone();
    unchanged.turn.number += 1;
    let mut changed = unchanged.clone();
    changed.objects[0].tapped = !changed.objects[0].tapped;
    let mut removed = changed.clone();
    removed.objects.clear();
    let mut returned = changed.clone();
    returned.turn.number += 2;
    let mut reordered = returned.clone();
    reordered.objects.reverse();
    let mut duplicate = reordered.clone();
    duplicate.objects.push(duplicate.objects[0].clone());
    let mut unequal_duplicate = duplicate.clone();
    unequal_duplicate.objects.last_mut().unwrap().marked_damage = 777.into();
    let mut other_viewer = first.clone();
    other_viewer.viewer = PlayerId(3);
    let public = vec![
        first.clone(),
        unchanged,
        changed.clone(),
        removed,
        returned,
        reordered,
        duplicate,
        unequal_duplicate,
        other_viewer,
    ];
    let mut stored = StoredHistory::default();
    for view in &public {
        stored.push_public(view.clone());
    }
    assert_views(&stored, &public);
    assert!(Arc::ptr_eq(
        &stored[0].objects[0].0,
        &stored[1].objects[0].0
    ));
    assert!(!Arc::ptr_eq(
        &stored[1].objects[0].0,
        &stored[2].objects[0].0
    ));
    assert!(
        !Arc::ptr_eq(&stored[2].objects[0].0, &stored[4].objects[0].0),
        "no nonadjacent interning after removal"
    );
    let mut other_history = StoredHistory::default();
    other_history.push_public(first);
    assert!(
        !Arc::ptr_eq(&stored[0].objects[0].0, &other_history[0].objects[0].0),
        "no cross-Knowledge/global row interner"
    );
    for a in &stored[7].objects {
        for b in &stored[8].objects {
            assert!(
                !Arc::ptr_eq(&a.0, &b.0),
                "different viewer never reuses rows even for forged identical handles"
            );
        }
    }
    assert_eq!(
        stored[6].objects.len(),
        public[6].objects.len(),
        "duplicates remain exact ordered entries"
    );
}
#[test]
fn stored_history_incremental_deserialization_compacts_and_does_not_add_wire_fields() {
    let mut public = vec![];
    for i in 0..75 {
        let mut view = public_fixture();
        view.turn.number = i + 1;
        public.push(view);
    }
    let json = serde_json::to_string(&public).unwrap();
    let restored: StoredHistory = serde_json::from_str(&json).unwrap();
    assert_views(&restored, &public);
    for index in 1..restored.len() {
        assert!(Arc::ptr_eq(
            &restored[0].objects[0].0,
            &restored[index].objects[0].0
        ));
    }
    let wire = serde_json::to_value(&public[0]).unwrap();
    let mut bad = wire.clone();
    bad["derived_memo"] = "forged".into();
    assert!(serde_json::from_value::<StoredPermittedView>(bad).is_err());
    let mut bad = wire;
    bad["objects"][0]["row_alias"] = "forged".into();
    assert!(serde_json::from_value::<StoredPermittedView>(bad).is_err());
}
#[test]
fn warmed_shared_rows_and_metadata_cow_invalidate_on_every_explicit_edit() {
    let first = public_fixture();
    let mut history = StoredHistory::default();
    for _ in 0..70 {
        history.push_public(first.clone());
    }
    for view in history.iter() {
        write_view(view);
    }
    let original = canonical(&history).unwrap();
    let mut branch = history.clone();
    branch[33].mutate_object(0, |row| row.oracle_id = "serialization-only:雪\n\0".into());
    let mut expected = vec![first.clone(); 70];
    expected[33].objects[0].oracle_id = "serialization-only:雪\n\0".into();
    assert_views(&branch, &expected);
    assert_eq!(canonical(&history).unwrap(), original);
    assert!(!Arc::ptr_eq(
        &branch[33].objects[0].0,
        &history[33].objects[0].0
    ));
    assert!(Arc::ptr_eq(
        &branch[32].objects[0].0,
        &history[32].objects[0].0
    ));
    // Warm an already-unique changed row, then edit it again: clear before &mut
    // exposure, not only when Arc::make_mut happened to clone a shared value.
    write_view(&branch[33]);
    branch[33].mutate_object(0, |row| row.marked_damage = (-7).into());
    expected[33].objects[0].marked_damage = (-7).into();
    assert_views(&branch, &expected);
    // Metadata edit is detached before closure exposure; there is no mutable
    // StoredViewData API with which to rewarm then silently change a field.
    for turn in [999, 1000] {
        write_view(&branch[33]);
        branch[33].update_public(|v| v.turn.number = turn);
        expected[33].turn.number = turn;
        assert_views(&branch, &expected);
    }
    branch[33].update_public(|v| v.viewer = PlayerId(3));
    expected[33].viewer = PlayerId(3);
    assert_views(&branch, &expected);
    for old in &branch[32].objects {
        for new in &branch[33].objects {
            assert!(
                !Arc::ptr_eq(&old.0, &new.0),
                "viewer change detaches all reused rows"
            );
        }
    }
    assert_eq!(canonical(&history).unwrap(), original);
}
#[test]
fn canonical_memos_retain_unique_rows_and_metadata_not_repeated_object_arrays() {
    // Serialization-domain synthetic rows, not fake admitted card definitions.
    let mut template = public_fixture();
    let one = template.objects[0].clone();
    template.objects = (0..128)
        .map(|i| {
            let mut row = one.clone();
            row.handle.0 = format!("viewer0-domain-{i}");
            row
        })
        .collect();
    let mut history = StoredHistory::default();
    for i in 0..100 {
        let mut view = template.clone();
        view.turn.number = i + 1;
        history.push_public(view);
    }
    let logical_bytes = canonical(&history).unwrap().len();
    let mut unique = BTreeSet::new();
    let mut row_bytes = 0;
    let mut metadata_bytes = 0;
    for view in history.iter() {
        let actual = write_view(view);
        assert_eq!(actual, canonical(&view.to_public()).unwrap().as_bytes());
        let segments = view.metadata_segments().unwrap();
        metadata_bytes += segments.before.len() + segments.after.len();
        for row in &view.objects {
            if unique.insert(row_pointer(row)) {
                row_bytes += row.bytes().unwrap().len();
            }
        }
    }
    assert_eq!(
        unique.len(),
        128,
        "one immutable value+canonical row buffer per unchanged row, not12800"
    );
    assert!(
        row_bytes + metadata_bytes < logical_bytes / 4,
        "structural retained memo proof, not a process-RSS/performance claim"
    );
    // Forks keep those same immutable row values/bytes; no canonical complete
    // View cache can silently grow on another State hash.
    let branch = history.clone();
    for (view, old) in branch.iter().zip(history.iter()) {
        for (row, old) in view.objects.iter().zip(&old.objects) {
            assert!(Arc::ptr_eq(&row.0, &old.0));
        }
    }
}
#[test]
fn every_public_view_field_roundtrips_and_metadata_changes_never_reuse_stale_segments() {
    let base = public_fixture();
    let mut value = serde_json::to_value(&base).unwrap();
    value["setup"] = serde_json::json!({"round":2,"phase":"declare","players":[{"player":0,"kept":false,"mulligans_taken":2}]});
    value["effects"] = serde_json::json!([]);
    let rich: PermittedView = serde_json::from_value(value.clone()).unwrap();
    let fields = [
        "setup",
        "effects",
        "combat",
        "viewer",
        "active",
        "turn_owner",
        "turn",
        "live_turn_order",
        "commanders",
        "priority",
        "players",
        "objects",
        "stack",
    ]
    .into_iter()
    .map(str::to_string)
    .collect::<BTreeSet<_>>();
    assert_eq!(
        value
            .as_object()
            .unwrap()
            .keys()
            .cloned()
            .collect::<BTreeSet<_>>(),
        fields,
        "future fields need exhaustive stored adapters/tests"
    );
    let mut stored: StoredPermittedView = rich.clone().into();
    write_view(&stored);
    for (field, replacement) in [
        ("setup", serde_json::Value::Null),
        ("viewer", serde_json::json!(3)),
        (
            "turn",
            serde_json::json!({"number":999,"step":"cleanup","lands_played":1}),
        ),
        ("objects", serde_json::json!([])),
        ("players", serde_json::json!([])),
    ] {
        let mut changed = value.clone();
        changed[field] = replacement;
        let public: PermittedView = serde_json::from_value(changed).unwrap();
        stored.update_public(|v| *v = public.clone());
        assert_eq!(write_view(&stored), canonical(&public).unwrap().as_bytes());
        stored.update_public(|v| *v = rich.clone());
        assert_eq!(write_view(&stored), canonical(&rich).unwrap().as_bytes());
    }
}
#[test]
fn real_state_wire_hash_and_owned_search_export_stay_complete_after_sharing() {
    let prepared = Prepared::admit_pod(
        Catalog::read(include_str!(
            "../../../../tests/fixtures/expected-match-ir.json"
        ))
        .unwrap(),
        MatchConfig::foundations([13; 32], PlayerId(0)),
    )
    .unwrap();
    let mut game = Game::new(prepared.clone());
    for _ in 0..64 {
        let request = loop {
            match game.advance(Budget::new(1)) {
                Advance::AwaitingDecision(r) => break r,
                Advance::Suspended { .. } => (),
                other => panic!("setup:{other:?}"),
            }
        };
        let response = match &request {
            Request::Mulligan { .. } => Response::Mulligan { take: false },
            Request::Priority { .. } => Response::Pass,
            Request::Attackers { .. } => Response::Attackers { attacks: vec![] },
            Request::Blockers { .. } => Response::Blockers { blocks: vec![] },
            Request::Discard {
                count, candidates, ..
            } => Response::Discard {
                objects: candidates.iter().take(*count).copied().collect(),
            },
            other => panic!("finite test:{other:?}"),
        };
        game.respond(request.player(), request.decision_id(), response)
            .unwrap();
    }
    // Search export is supported only at an installed ordinary Priority.
    // Keep the same fixed legal policy through any intervening combat/cleanup;
    // do not assume decision 64 happened to be a priority boundary.
    let mut reached_priority = false;
    for _ in 0..1024 {
        let request = match game.advance(Budget::new(1)) {
            Advance::Suspended { .. } => continue,
            Advance::AwaitingDecision(request) => request,
            other => panic!("priority drain:{other:?}"),
        };
        let response = match &request {
            Request::Priority { .. } => {
                reached_priority = true;
                break;
            }
            Request::Attackers { .. } => Response::Attackers { attacks: vec![] },
            Request::Blockers { .. } => Response::Blockers { blocks: vec![] },
            Request::Discard {
                count, candidates, ..
            } => Response::Discard {
                objects: candidates.iter().take(*count).copied().collect(),
            },
            other => panic!("priority drain request:{other:?}"),
        };
        game.respond(request.player(), request.decision_id(), response)
            .unwrap();
    }
    assert!(reached_priority, "fixed bounded priority drain exhausted");
    assert!(matches!(game.state.frame, Frame::Priority));
    assert!(game.state.pending.is_none());
    let expected = canonical(&game.state).unwrap();
    let mut actual = vec![];
    state_digest::write_state(&game.state, &mut actual).unwrap();
    assert_eq!(actual, expected.as_bytes());
    assert_eq!(game.state_digest().unwrap(), sha256(expected.as_bytes()));
    let restored = Game::restore(prepared.clone(), &game.snapshot().unwrap()).unwrap();
    assert_eq!(canonical(&restored.state).unwrap(), expected);
    let replay = Game::replay(prepared, &game.replay_journal()).unwrap();
    assert_eq!(canonical(&replay.state).unwrap(), expected);
    let mut query = game.search_context(PlayerId(0), "full-owned-history".into(), Budget::new(1));
    let context = loop {
        match query {
            QueryResult::Complete(QueryValue::SearchContext(c)) => break c,
            QueryResult::Incomplete { work, .. } => query = game.resume_work(work, Budget::new(1)),
            QueryResult::Rejected(reason) => panic!("context rejected:{reason:?}"),
            QueryResult::Complete(_) => panic!("context returned wrong query variant"),
        }
    };
    let public = game.state.visibility[0]
        .history
        .iter()
        .map(StoredPermittedView::to_public)
        .collect::<Vec<_>>();
    assert_eq!(
        context.permitted_history, public,
        "public export still materializes EVERY full owned view; cost not hidden"
    );
    assert_eq!(
        canonical(&context.permitted_history).unwrap(),
        canonical(&game.state.visibility[0].history).unwrap()
    );
}
#[test]
fn shared_row_cold_initializers_can_overlap_without_changing_any_bytes() {
    let first = public_fixture();
    let mut history = StoredHistory::default();
    for _ in 0..40 {
        history.push_public(first.clone());
    }
    let expected = canonical(&history).unwrap();
    std::thread::scope(|scope| {
        let mut joins = vec![];
        for _ in 0..3 {
            let history = history.clone();
            let expected = &expected;
            joins.push(scope.spawn(move || {
                assert_eq!(canonical(&history).unwrap(), *expected);
                for view in history.iter() {
                    assert_eq!(
                        write_view(view),
                        canonical(&view.to_public()).unwrap().as_bytes()
                    );
                }
            }));
        }
        for join in joins {
            join.join().unwrap();
        }
    });
}
