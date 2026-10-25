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
    for a in stored[7].objects.iter() {
        for b in stored[8].objects.iter() {
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
    for old in branch[32].objects.iter() {
        for new in branch[33].objects.iter() {
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
        metadata_bytes += segments
            .parts
            .iter()
            .map(|(bytes, _)| bytes.len())
            .sum::<usize>()
            + segments.after.len();
        for row in view.objects.iter() {
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
        for (row, old) in view.objects.iter().zip(old.objects.iter()) {
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
        MatchConfig::for_profile("commander-foundations-v1", [13; 32], PlayerId(0)).unwrap(),
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
        context.permitted_history.iter().collect::<Vec<_>>(),
        public,
        "explicit owned iteration still returns EVERY complete view; cost not hidden"
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

#[test]
fn reflexive_stored_objects_compare_all_independent_fields_and_detach_before_mutation() {
    fn assert_eq_type<T: Eq>() {}
    assert_eq_type::<VisibleObject>();
    let mut original = public_fixture().objects.into_iter().next().unwrap();
    // This historical profile omits published characteristics. For this
    // serialization-domain row, use its matching frozen catalog definition so
    // the Some -> None edit below changes the ninth field too. This is not an
    // admitted-position or observation-publication claim.
    let catalog =
        Catalog::read(include_str!("../../../../tests/fixtures/expected-ir.json")).unwrap();
    original.characteristics = Some(
        catalog
            .definitions
            .iter()
            .find(|definition| definition.oracle_id == original.oracle_id)
            .unwrap()
            .characteristics
            .clone(),
    );
    let left = StoredObject::new(original.clone());
    left.bytes().unwrap();
    let shared = left.clone();
    let independent = StoredObject::new(original.clone());
    assert!(Arc::ptr_eq(&left.0, &shared.0));
    assert!(!Arc::ptr_eq(&left.0, &independent.0));
    assert!(left == shared && left == independent);
    let edits: [fn(&mut VisibleObject); 9] = [
        |v| v.characteristics = None,
        |v| v.marked_damage = 991.into(),
        |v| v.attack_ready = !v.attack_ready,
        |v| v.handle.0.push_str("-new-identity"),
        |v| v.oracle_id.push_str("-late-field"),
        |v| v.owner = PlayerId((v.owner.0 + 1) % 4),
        |v| v.controller = PlayerId((v.controller.0 + 1) % 4),
        |v| {
            v.zone = if v.zone == Zone::Exile {
                Zone::Battlefield
            } else {
                Zone::Exile
            }
        },
        |v| v.tapped = !v.tapped,
    ];
    let original_bytes = canonical(&original).unwrap();
    for edit in edits {
        let mut branch = shared.clone();
        edit(&mut branch);
        assert!(!Arc::ptr_eq(&branch.0, &left.0));
        let old_deep_result = left.0.value == branch.0.value;
        assert!(!old_deep_result);
        assert_eq!(left == branch, old_deep_result);
        assert_eq!(left.bytes().unwrap(), original_bytes.as_bytes());
        assert_eq!(
            branch.bytes().unwrap(),
            canonical(&branch.0.value).unwrap().as_bytes()
        );
        let separate: StoredObject = serde_json::from_str(&canonical(&branch).unwrap()).unwrap();
        assert!(branch == separate);
        assert!(left == shared);
    }
}
#[test]
fn reflexive_stored_history_matches_public_deep_oracle_for_metadata_rows_order_and_cow() {
    let first = public_fixture();
    let mut public = vec![];
    let mut history = StoredHistory::default();
    for index in 0..97 {
        let mut view = first.clone();
        view.turn.number += index;
        public.push(view.clone());
        history.push_public(view);
    }
    for view in history.iter() {
        write_view(view);
    }
    let shared = history.clone();
    let independent: StoredHistory = serde_json::from_str(&canonical(&public).unwrap()).unwrap();
    assert!(history == shared && history == independent);
    assert_views(&history, &public);
    for index in [0, 31, 32, 64, 96] {
        let mut branch = shared.clone();
        let mut expected = public.clone();
        branch[index].mutate_object(0, |row| row.oracle_id.push_str("-late-object-field"));
        expected[index].objects[0]
            .oracle_id
            .push_str("-late-object-field");
        let old_deep_result = public == expected;
        assert_eq!(history == branch, old_deep_result);
        assert!(!old_deep_result);
        assert_views(&branch, &expected);
        assert_views(&history, &public);
    }
    let mut metadata = history.clone();
    metadata[96].data.turn.number += 1;
    let mut expected = public.clone();
    expected[96].turn.number += 1;
    assert_eq!(history == metadata, public == expected);
    assert_views(&metadata, &expected);
    let mut shorter = history.clone();
    shorter.clear();
    assert!(history != shorter);
    let mut reversed = public.clone();
    reversed.reverse();
    let reversed: StoredHistory = serde_json::from_str(&canonical(&reversed).unwrap()).unwrap();
    assert!(history != reversed);
    // Serialization-domain State values exercise the new StoredHistory Eq.
    // This deliberately edited history is NOT an admitted/replayed Game claim;
    // independent complete generic canonical bytes remain the oracle.
    let mut game = Game::new(
        Prepared::admit(
            Catalog::read(include_str!("../../../../tests/fixtures/expected-ir.json")).unwrap(),
            ScenarioConfig::land_return(),
        )
        .unwrap(),
    );
    game.state.visibility[0].history = history.clone();
    let mut branch = game.fork();
    assert!(game.state == branch.state);
    let before = canonical(&game.state).unwrap();
    branch.state.visibility[0].history[96].mutate_object(0, |row| row.tapped = !row.tapped);
    assert!(game.state != branch.state);
    assert_eq!(canonical(&game.state).unwrap(), before);
    for state in [&game.state, &branch.state] {
        let generic = canonical(state).unwrap();
        let mut actual = vec![];
        state_digest::write_state(state, &mut actual).unwrap();
        assert_eq!(actual, generic.as_bytes());
        assert_eq!(
            state_digest::state_digest(state).unwrap(),
            sha256(generic.as_bytes())
        );
    }
}

#[test]
fn reflexive_history_real_legal_firstwrites_keep_parent_siblings_and_partition_oracles() {
    let prepared = Prepared::admit_pod(
        Catalog::read(include_str!(
            "../../../../tests/fixtures/expected-match-ir.json"
        ))
        .unwrap(),
        MatchConfig::for_profile("commander-foundations-v1", [13; 32], PlayerId(0)).unwrap(),
    )
    .unwrap();
    let mut root = Game::new(prepared);
    let mut priority = None;
    for _ in 0..1024 {
        match root.advance(Budget::new(1)) {
            Advance::Suspended { .. } => (),
            Advance::AwaitingDecision(request) => match request {
                Request::Mulligan { .. } => {
                    root.respond(
                        request.player(),
                        request.decision_id(),
                        Response::Mulligan { take: false },
                    )
                    .unwrap();
                }
                Request::Priority { .. } => {
                    priority = Some(request);
                    break;
                }
                other => panic!("fixed opening boundary:{other:?}"),
            },
            other => panic!("fixed opening advance:{other:?}"),
        }
    }
    let request = priority.expect("bounded legal opening reaches Priority");
    assert!(root.state.pending.is_none());
    let before = canonical(&root.state).unwrap();
    let sibling = root.fork();
    let mut advanced = vec![];
    for quantum in [1, u64::MAX] {
        let mut branch = root.fork();
        assert!(branch.state == root.state);
        branch
            .respond(request.player(), request.decision_id(), Response::Pass)
            .unwrap();
        let mut ready = false;
        for _ in 0..32 {
            match branch.advance(Budget::new(quantum)) {
                Advance::Suspended { .. } => (),
                Advance::AwaitingDecision(_) => {
                    ready = true;
                    break;
                }
                other => panic!("firstwrite advance:{other:?}"),
            }
        }
        assert!(ready && branch.state.pending.is_none());
        assert!(branch.state != root.state);
        assert_eq!(canonical(&root.state).unwrap(), before);
        assert!(sibling.state == root.state);
        for (actual, untouched) in branch.state.visibility.iter().zip(&root.state.visibility) {
            let owned_actual = actual
                .history
                .iter()
                .map(StoredPermittedView::to_public)
                .collect::<Vec<_>>();
            let owned_untouched = untouched
                .history
                .iter()
                .map(StoredPermittedView::to_public)
                .collect::<Vec<_>>();
            assert_eq!(
                actual.history == untouched.history,
                owned_actual == owned_untouched
            );
        }
        let generic = canonical(&branch.state).unwrap();
        let mut emitted = vec![];
        state_digest::write_state(&branch.state, &mut emitted).unwrap();
        assert_eq!(emitted, generic.as_bytes());
        assert_eq!(branch.state_digest().unwrap(), sha256(generic.as_bytes()));
        advanced.push(branch);
    }
    assert!(advanced[0].state == advanced[1].state);
    assert_eq!(
        canonical(&advanced[0].state).unwrap(),
        canonical(&advanced[1].state).unwrap()
    );
    assert_eq!(canonical(&root.state).unwrap(), before);
}

// Normal structural mechanism checks only. Runtime sizeof/capacity facts are
// recorded when explicitly executed later; this authored source is not proof
// that the complete legal workload fits its existing process-memory cap.
fn capacity_public_rows(template: &VisibleObject, len: usize, extra: usize) -> Vec<VisibleObject> {
    let mut rows = Vec::with_capacity(len + extra);
    for index in 0..len {
        let mut row = template.clone();
        row.handle.0 = format!("capacity-domain-{index}");
        rows.push(row);
    }
    rows
}
#[test]
fn stored_capacity_conversion_measures_legacy_layout_and_fresh_destination_slots() {
    use std::mem::{align_of, size_of};
    let template = public_fixture();
    let row = template.objects.first().unwrap().clone();
    let source_size = size_of::<VisibleObject>();
    let destination_size = size_of::<StoredObject>();
    let source_align = align_of::<VisibleObject>();
    let destination_align = align_of::<StoredObject>();
    // These are measured concrete-layout prerequisites for the pinned compiler,
    // NOT a claim about every Rust type/target or the public Vec API contract.
    assert!(source_size > destination_size && destination_size > 0);
    assert_eq!(source_align, destination_align);
    assert_eq!(source_size % destination_size, 0);
    let mut proof = vec![];
    for len in [0, 1, 31, 32, 33, 256] {
        for extra in [0, 257] {
            let legacy_source = capacity_public_rows(&row, len, extra);
            let legacy_source_capacity = legacy_source.capacity();
            let legacy_source_address = legacy_source.as_ptr() as usize;
            // Exact old conversion, retained solely as a mechanism oracle.
            let legacy = legacy_source
                .into_iter()
                .map(StoredObject::new)
                .collect::<Vec<_>>();
            let expected_legacy_capacity =
                legacy_source_capacity.checked_mul(source_size).unwrap() / destination_size;
            assert_eq!(legacy.len(), len);
            assert_eq!(legacy.capacity(), expected_legacy_capacity);
            if legacy_source_capacity > 0 {
                assert_eq!(legacy.as_ptr() as usize, legacy_source_address);
            }
            let source = capacity_public_rows(&row, len, extra);
            let source_capacity = source.capacity();
            let mut view = template.clone();
            view.objects = source;
            // Cloning for the independent public oracle intentionally drops
            // spare capacity; it preserves EVERY logical field/value/order.
            let expected = view.clone();
            let stored: StoredPermittedView = view.into();
            let fresh_capacity = stored.data.objects.capacity();
            assert_eq!(stored.data.objects.len(), len);
            assert_eq!(
                fresh_capacity, len,
                "pinned non-ZST Vec::with_capacity implementation"
            );
            assert_eq!(stored.to_public(), expected);
            assert_eq!(canonical(&stored).unwrap(), canonical(&expected).unwrap());
            assert_eq!(
                write_view(&stored),
                canonical(&expected).unwrap().as_bytes()
            );
            assert_eq!(
                canonical(&legacy).unwrap(),
                canonical(&expected.objects).unwrap()
            );
            if extra > 0 {
                assert!(legacy.capacity() > fresh_capacity);
            }
            proof.push(serde_json::json!({
                "len":len, "source_size":source_size, "destination_size":destination_size,
                "source_alignment":source_align, "destination_alignment":destination_align,
                "source_capacity":source_capacity,
                "legacy_source_capacity":legacy_source_capacity,
                "legacy_destination_capacity":legacy.capacity(),
                "legacy_destination_slot_bytes":legacy.capacity().checked_mul(destination_size).unwrap(),
                "fresh_destination_capacity":fresh_capacity,
                "fresh_destination_slot_bytes":fresh_capacity.checked_mul(destination_size).unwrap(),
                "scope":"vector slot layout/capacity; excludes Row allocations, payloads, allocator bookkeeping, RSS and whole-process OOM causality"
            }));
        }
    }
    println!(
        "STORED_OBJECT_CAPACITY_PROOF={}",
        canonical(&proof).unwrap()
    );
}
#[test]
fn stored_capacity_conversion_keeps_full_history_wire_reuse_and_cow_isolation() {
    let template = public_fixture();
    let one = template.objects.first().unwrap().clone();
    for len in [0, 1, 31, 32, 33, 256] {
        let mut first = template.clone();
        first.objects = capacity_public_rows(&one, len, 257);
        let mut next = first.clone();
        next.objects = capacity_public_rows(&one, len, 513);
        next.turn.number += 1;
        let mut public = vec![first.clone(), next.clone()];
        let mut history = StoredHistory::default();
        history.push_public(first);
        history.push_public(next);
        assert_views(&history, &public);
        for view in history.iter() {
            assert_eq!(view.data.objects.capacity(), len);
        }
        if len > 0 {
            for index in 0..len {
                assert!(Arc::ptr_eq(
                    &history[0].objects[index].0,
                    &history[1].objects[index].0
                ));
            }
            let mut branch = history.clone();
            branch[1].mutate_object(len - 1, |row| row.oracle_id.push_str("-changed雪"));
            public[1].objects[len - 1].oracle_id.push_str("-changed雪");
            assert!(!Arc::ptr_eq(
                &branch[1].objects[len - 1].0,
                &history[1].objects[len - 1].0
            ));
            assert_views(&branch, &public);
            assert_ne!(branch, history);
            assert_ne!(branch[1].to_public(), history[1].to_public());
            assert_eq!(
                history[0].to_public().objects[len - 1],
                one_with_capacity_handle(&one, len - 1)
            );
        }
        let serialized = canonical(&history).unwrap();
        let restored: StoredHistory = serde_json::from_str(&serialized).unwrap();
        assert_eq!(restored, history);
        assert_eq!(canonical(&restored).unwrap(), serialized);
        for (actual, expected) in restored.iter().zip(history.iter()) {
            assert_eq!(actual.to_public(), expected.to_public());
        }
    }
}
fn one_with_capacity_handle(template: &VisibleObject, index: usize) -> VisibleObject {
    let mut row = template.clone();
    row.handle.0 = format!("capacity-domain-{index}");
    row
}

// Serialization-domain fixtures only: no invented card admission/replay claim.
fn collection_fixture() -> PermittedView {
    let mut view = public_fixture();
    assert!(!view.players.is_empty() && !view.objects.is_empty());
    let row = view.objects[0].clone();
    view.objects = capacity_public_rows(&row, 33, 0);
    view.commanders = vec![VisibleCommander {
        designation: "collection-domain-commander".into(),
        oracle_id: row.oracle_id,
        history: CommanderHistory {
            owner: PlayerId(0),
            color_identity: vec![Color::W],
            command_casts: BTreeMap::from([(PlayerId(0), 1.into())]),
            combat_damage: BTreeMap::from([(PlayerId(3), 2.into())]),
        },
    }];
    view
}
#[test]
fn stored_collections_share_complete_adjacent_arrays_and_decode_exact_original_wire() {
    let first = collection_fixture();
    let public = (0..65)
        .map(|i| {
            let mut next = first.clone();
            next.turn.number += i;
            next
        })
        .collect::<Vec<_>>();
    let mut history = StoredHistory::default();
    for view in &public {
        history.push_public(view.clone());
    }
    // Independent whole owned generic oracle; no cache writer constructs expected bytes.
    assert_views(&history, &public);
    for i in 1..history.len() {
        assert!(Arc::ptr_eq(
            &history[0].data.objects,
            &history[i].data.objects
        ));
        assert!(Arc::ptr_eq(
            &history[0].data.players,
            &history[i].data.players
        ));
        assert!(Arc::ptr_eq(
            &history[0].data.commanders,
            &history[i].data.commanders
        ));
    }
    let json = serde_json::to_string(&public).unwrap();
    let restored: StoredHistory = serde_json::from_str(&json).unwrap();
    assert_views(&restored, &public);
    assert!(!Arc::ptr_eq(
        &history[0].data.objects,
        &restored[0].data.objects
    ));
    for i in 1..restored.len() {
        assert!(Arc::ptr_eq(
            &restored[0].data.objects,
            &restored[i].data.objects
        ));
        assert!(Arc::ptr_eq(
            &restored[0].data.players,
            &restored[i].data.players
        ));
        assert!(Arc::ptr_eq(
            &restored[0].data.commanders,
            &restored[i].data.commanders
        ));
    }
    // Field adapters retain the old strict element/array decoder, not Arc metadata.
    let value = serde_json::to_value(&first).unwrap();
    for field in ["objects", "players", "commanders"] {
        let mut bad = value.clone();
        bad[field] = serde_json::json!({"shared_allocation":123});
        assert!(serde_json::from_value::<StoredPermittedView>(bad).is_err());
        let mut bad = value.clone();
        bad[field][0]["unknown_collection_field"] = true.into();
        assert!(serde_json::from_value::<StoredPermittedView>(bad).is_err());
    }
}
#[test]
fn stored_collections_full_fallback_keeps_late_nested_changes_and_order() {
    let first = collection_fixture();
    let before: StoredPermittedView = first.clone().into();
    let mut independent: StoredPermittedView = first.clone().into();
    assert!(!Arc::ptr_eq(
        &before.data.objects,
        &independent.data.objects
    ));
    assert!(!Arc::ptr_eq(
        &before.data.players,
        &independent.data.players
    ));
    assert!(!Arc::ptr_eq(
        &before.data.commanders,
        &independent.data.commanders
    ));
    assert_eq!(before, independent);
    independent.reuse_previous(Some(&before));
    assert!(Arc::ptr_eq(&before.data.objects, &independent.data.objects));
    let edits: [fn(&mut PermittedView); 6] = [
        |v| v.objects.last_mut().unwrap().oracle_id.push_str("-late雪"),
        |v| v.objects.reverse(),
        |v| v.objects.push(v.objects.last().unwrap().clone()),
        |v| {
            v.players
                .last_mut()
                .unwrap()
                .mana
                .insert(Color::G, 999.into());
        },
        |v| {
            v.commanders
                .last_mut()
                .unwrap()
                .history
                .combat_damage
                .insert(PlayerId(3), 999.into());
        },
        |v| {
            v.commanders
                .last_mut()
                .unwrap()
                .designation
                .push_str("-late雪")
        },
    ];
    for edit in edits {
        let mut public = first.clone();
        edit(&mut public);
        let mut changed: StoredPermittedView = public.clone().into();
        changed.reuse_previous(Some(&before));
        assert_eq!(before == changed, first == public);
        assert_ne!(before, changed);
        assert_eq!(changed.to_public(), public);
        assert_eq!(write_view(&changed), canonical(&public).unwrap().as_bytes());
        if first.objects != public.objects {
            assert!(!Arc::ptr_eq(&before.data.objects, &changed.data.objects));
        }
        if first.players != public.players {
            assert!(!Arc::ptr_eq(&before.data.players, &changed.data.players));
        }
        if first.commanders != public.commanders {
            assert!(!Arc::ptr_eq(
                &before.data.commanders,
                &changed.data.commanders
            ));
        }
    }
}
#[test]
fn stored_collections_viewer_and_predecessor_scope_never_become_global_interning() {
    let first = collection_fixture();
    let mut other = first.clone();
    other.viewer = PlayerId(3);
    let mut empty = first.clone();
    empty.objects.clear();
    empty.players.clear();
    empty.commanders.clear();
    let public = vec![first.clone(), other, first.clone(), empty, first.clone()];
    let mut history = StoredHistory::default();
    for view in &public {
        history.push_public(view.clone());
    }
    assert_views(&history, &public);
    // Equal values in another viewer/after a different predecessor do not intern.
    for i in [1, 2, 4] {
        assert!(!Arc::ptr_eq(
            &history[0].data.objects,
            &history[i].data.objects
        ));
        assert!(!Arc::ptr_eq(
            &history[0].data.players,
            &history[i].data.players
        ));
        assert!(!Arc::ptr_eq(
            &history[0].data.commanders,
            &history[i].data.commanders
        ));
    }
    let mut separate = StoredHistory::default();
    separate.push_public(first);
    assert!(!Arc::ptr_eq(
        &history[0].data.objects,
        &separate[0].data.objects
    ));
    assert!(!Arc::ptr_eq(
        &history[0].data.players,
        &separate[0].data.players
    ));
    assert!(!Arc::ptr_eq(
        &history[0].data.commanders,
        &separate[0].data.commanders
    ));
}
#[test]
fn stored_collections_warm_sibling_late_mutations_preserve_full_state_bytes() {
    let first = collection_fixture();
    let public = vec![first.clone(); 65];
    let mut history = StoredHistory::default();
    for view in &public {
        history.push_public(view.clone());
    }
    assert_views(&history, &public); // warm every row/metadata memo first
    let mut branch = history.clone();
    let mut expected = public.clone();
    branch[33].mutate_object(32, |v| v.marked_damage = 321.into());
    expected[33].objects[32].marked_damage = 321.into();
    assert!(!Arc::ptr_eq(
        &branch[33].data.objects,
        &history[33].data.objects
    ));
    assert!(Arc::ptr_eq(
        &branch[33].data.players,
        &history[33].data.players
    ));
    assert!(Arc::ptr_eq(
        &branch[33].data.commanders,
        &history[33].data.commanders
    ));
    assert!(Arc::ptr_eq(
        &branch[33].objects[0].0,
        &history[33].objects[0].0
    ));
    assert_views(&branch, &expected);
    branch[64].update_public(|v| {
        v.players[0].life.add(&7.into());
        v.commanders[0]
            .history
            .command_casts
            .insert(PlayerId(0), 10.into());
    });
    expected[64].players[0].life.add(&7.into());
    expected[64].commanders[0]
        .history
        .command_casts
        .insert(PlayerId(0), 10.into());
    assert!(!Arc::ptr_eq(
        &branch[64].data.players,
        &history[64].data.players
    ));
    assert!(!Arc::ptr_eq(
        &branch[64].data.commanders,
        &history[64].data.commanders
    ));
    assert!(Arc::ptr_eq(
        &branch[64].data.objects,
        &history[64].data.objects
    ));
    assert_views(&branch, &expected);
    assert_views(&history, &public);
    let mut game = Game::new(
        Prepared::admit(
            Catalog::read(include_str!("../../../../tests/fixtures/expected-ir.json")).unwrap(),
            ScenarioConfig::land_return(),
        )
        .unwrap(),
    );
    // Full serialization-domain State oracle, deliberately not admitted/replayed facts.
    game.state.visibility[0].history = history.clone();
    let mut sibling = game.fork();
    sibling.state.visibility[0].history = branch;
    assert!(game.state != sibling.state);
    for state in [&game.state, &sibling.state] {
        let generic = canonical(state).unwrap();
        let mut actual = vec![];
        state_digest::write_state(state, &mut actual).unwrap();
        assert_eq!(actual, generic.as_bytes());
        assert_eq!(
            state_digest::state_digest(state).unwrap(),
            sha256(generic.as_bytes())
        );
    }
    assert_views(&game.state.visibility[0].history, &public);
}
// Explicit allocation-capacity mechanism accounting only; no estimated RSS,
// allocator headers, nested row/string/map payloads or process-fit claim.
fn distinct_collection_slots<T>(vectors: &[&Arc<Vec<T>>]) -> (usize, usize) {
    let mut seen = BTreeSet::new();
    let mut bytes = 0usize;
    for vector in vectors {
        if seen.insert(Arc::as_ptr(vector) as usize) {
            bytes = bytes
                .checked_add(
                    vector
                        .capacity()
                        .checked_mul(std::mem::size_of::<T>())
                        .unwrap(),
                )
                .unwrap();
        }
    }
    (seen.len(), bytes)
}
#[test]
fn stored_collections_report_actual_distinct_capacity_bytes_without_rss_estimates() {
    let mut proof = vec![];
    for len in [0, 1, 33, 256] {
        let mut first = collection_fixture();
        let row = first.objects[0].clone();
        first.objects = capacity_public_rows(&row, len, 257);
        let public = (0..65)
            .map(|i| {
                let mut v = first.clone();
                v.turn.number += i;
                v
            })
            .collect::<Vec<_>>();
        // Explicit unshared candidate representation, not a historical timed run.
        let unshared = public
            .iter()
            .cloned()
            .map(StoredPermittedView::from)
            .collect::<Vec<_>>();
        let mut history = StoredHistory::default();
        for v in &public {
            history.push_public(v.clone());
        }
        assert_views(&history, &public);
        for v in &unshared {
            assert_eq!(write_view(v), canonical(&v.to_public()).unwrap().as_bytes());
        }
        let old_objects = distinct_collection_slots(
            &unshared.iter().map(|v| &v.data.objects).collect::<Vec<_>>(),
        );
        let new_objects =
            distinct_collection_slots(&history.iter().map(|v| &v.data.objects).collect::<Vec<_>>());
        let old_players = distinct_collection_slots(
            &unshared.iter().map(|v| &v.data.players).collect::<Vec<_>>(),
        );
        let new_players =
            distinct_collection_slots(&history.iter().map(|v| &v.data.players).collect::<Vec<_>>());
        let old_commanders = distinct_collection_slots(
            &unshared
                .iter()
                .map(|v| &v.data.commanders)
                .collect::<Vec<_>>(),
        );
        let new_commanders = distinct_collection_slots(
            &history
                .iter()
                .map(|v| &v.data.commanders)
                .collect::<Vec<_>>(),
        );
        for (unshared, shared) in [
            (old_objects, new_objects),
            (old_players, new_players),
            (old_commanders, new_commanders),
        ] {
            assert_eq!(unshared.0, 65);
            assert_eq!(shared.0, 1);
            assert_eq!(unshared.1, shared.1.checked_mul(65).unwrap());
        }
        proof.push(serde_json::json!({
            "views":65, "objects_per_view":len,
            "object_element_size":std::mem::size_of::<StoredObject>(),
            "player_element_size":std::mem::size_of::<VisiblePlayer>(),
            "commander_element_size":std::mem::size_of::<VisibleCommander>(),
            "unshared_object_capacity_bytes":old_objects.1, "shared_object_capacity_bytes":new_objects.1,
            "unshared_player_capacity_bytes":old_players.1, "shared_player_capacity_bytes":new_players.1,
            "unshared_commander_capacity_bytes":old_commanders.1, "shared_commander_capacity_bytes":new_commanders.1,
            "unshared_distinct_allocations_per_field":65, "shared_distinct_allocations_per_field":1,
            "scope":"observed Vec capacity*element-size only; excludes Arc control allocations, view structs, nested payloads, memos, allocator metadata/RSS/cgroup; serialization-domain mechanism, not a legal root or fit result"
        }));
    }
    println!(
        "STORED_COLLECTION_CAPACITY_PROOF={}",
        canonical(&proof).unwrap()
    );
}

#[test]
fn persistent_stack_preserves_exact_order_fields_memos_and_restore_sharing() {
    let mut base = public_fixture();
    let row: VisibleStackEntry = serde_json::from_value(serde_json::json!({
        "mode":1,"handle":"s","spell":"a","ability":null,"controller":0,
        "cast_complete":true,"targets":[{"announced":"old","current":null}],
        "entity_targets":[{"entity":"player","player":2}],
        "stack_targets":[{"announced":"s2","current":null}],"player_targets":[3]
    }))
    .unwrap();
    base.stack = vec![row.clone(); 3]; // duplicates must remain distinct ordered entries
    let mut changed = base.clone();
    changed.stack[2].controller = PlayerId(2);
    let mut reordered = changed.clone();
    reordered.stack.reverse();
    let mut popped = changed.clone();
    popped.stack.pop();
    let mut other = base.clone();
    other.viewer = PlayerId(3);
    let public = vec![base.clone(), base, changed, popped, reordered, other];
    let mut history = StoredHistory::default();
    for view in &public {
        history.push_public(view.clone());
    }
    assert_views(&history, &public); // warms row and metadata canonical memos
    let pointers = |i: usize| history[i].stack.node_pointers();
    assert_eq!(pointers(0), pointers(1));
    assert_eq!(pointers(1)[..2], pointers(2)[..2]);
    assert_ne!(pointers(1)[2], pointers(2)[2]);
    assert_eq!(pointers(2)[..2], pointers(3));
    assert!(pointers(5).iter().all(|p| !pointers(4).contains(p)));
    let restored: StoredHistory =
        serde_json::from_str(&serde_json::to_string(&history).unwrap()).unwrap();
    assert_views(&restored, &public);
    assert_eq!(restored.retention_charge(), history.retention_charge());
    assert_eq!(
        restored[0].stack.node_pointers(),
        restored[1].stack.node_pointers()
    );
    let mut sibling = history.clone();
    sibling[1].update_public(|v| v.stack[0].player_targets = vec![PlayerId(1)]);
    let mut edited = public.clone();
    edited[1].stack[0].player_targets = vec![PlayerId(1)];
    assert_views(&sibling, &edited);
    assert_views(&history, &public);
}

#[test]
fn collection_canonical_memos_follow_shared_values_instead_of_views() {
    let first = collection_fixture();
    let mut history = StoredHistory::default();
    for number in 1..65 {
        let mut view = first.clone();
        view.turn.number = number;
        history.push_public(view);
    }
    for view in history.iter() {
        assert_eq!(
            write_view(view),
            canonical(&view.to_public()).unwrap().as_bytes()
        );
        assert!(Arc::ptr_eq(&history[0].players_bytes, &view.players_bytes));
        assert!(Arc::ptr_eq(
            &history[0].commanders_bytes,
            &view.commanders_bytes
        ));
    }
    let mut branch = history.clone();
    branch[32].update_public(|v| {
        v.players[0].life.add(&1.into());
        v.commanders[0].designation.push_str("changed");
    });
    assert!(!Arc::ptr_eq(
        &history[32].players_bytes,
        &branch[32].players_bytes
    ));
    assert!(!Arc::ptr_eq(
        &history[32].commanders_bytes,
        &branch[32].commanders_bytes
    ));
    assert_eq!(
        write_view(&history[32]),
        canonical(&history[32].to_public()).unwrap().as_bytes()
    );
    assert_eq!(
        write_view(&branch[32]),
        canonical(&branch[32].to_public()).unwrap().as_bytes()
    );
}
