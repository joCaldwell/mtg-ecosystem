use super::*;

fn object(card: u32, incarnation: u32) -> ObjectId {
    ObjectId {
        card: CardId(card),
        incarnation,
    }
}

#[test]
fn reverse_index_preserves_wire_equality_and_lexical_first_lookup() {
    // Malformed duplicate IDs retain the old lookup semantics until the normal
    // snapshot reachability checks reject them. Never silently canonicalize data.
    let wire = r#"{"v0-o10":{"card":3,"incarnation":1},"v0-o2":{"card":3,"incarnation":1},"v0-o7":{"card":7,"incarnation":0}}"#;
    let handles: ObjectHandles = serde_json::from_str(wire).unwrap();
    let forward: BTreeMap<String, ObjectId> = serde_json::from_str(wire).unwrap();
    let cold: ObjectHandles = serde_json::from_str(wire).unwrap();
    assert!(handles.reverse.get().is_none());
    for id in [object(3, 1), object(7, 0), object(99, 0)] {
        let expected = forward
            .iter()
            .find(|(_, value)| **value == id)
            .map(|(handle, _)| ObjectHandle(handle.clone()));
        assert_eq!(handles.handle_for(id).cloned(), expected);
    }
    assert!(handles.reverse.get().is_some());
    assert_eq!(handles, cold);
    assert_eq!(canonical(&handles).unwrap(), canonical(&forward).unwrap());
    assert_eq!(digest(&handles).unwrap(), digest(&cold).unwrap());
}

#[test]
fn shared_reverse_index_invalidates_only_the_mutated_branch() {
    let mut original = ObjectHandles::default();
    original.insert("v0-o0".into(), object(1, 0));
    assert_eq!(
        original.handle_for(object(1, 0)),
        Some(&ObjectHandle("v0-o0".into()))
    );
    let mut branch = original.clone();
    assert!(Arc::ptr_eq(&original.reverse, &branch.reverse));
    branch.insert("v0-o0".into(), object(1, 0));
    branch.retain_objects(&[object(1, 0)].into());
    assert!(Arc::ptr_eq(&original.reverse, &branch.reverse));
    branch.insert("v0-o1".into(), object(1, 1));
    assert!(!Arc::ptr_eq(&original.reverse, &branch.reverse));
    assert!(original.handle_for(object(1, 1)).is_none());
    assert!(branch.handle_for(object(1, 1)).is_some());
    branch.retain_objects(&[object(1, 1)].into());
    assert!(branch.handle_for(object(1, 0)).is_none());
    assert!(original.handle_for(object(1, 0)).is_some());
    branch.insert("v0-o1".into(), object(2, 0));
    assert!(branch.handle_for(object(1, 1)).is_none());
    assert!(branch.handle_for(object(2, 0)).is_some());
    let restored: ObjectHandles = strict_read(&canonical(&branch).unwrap()).unwrap();
    assert!(restored.reverse.get().is_none());
    assert_eq!(restored, branch);
    assert_eq!(
        restored.handle_for(object(2, 0)),
        branch.handle_for(object(2, 0))
    );
}

#[test]
fn indexed_visibility_refresh_keeps_handles_until_visibility_or_incarnation_changes() {
    let prepared = Prepared::admit(
        Catalog::read(include_str!("../../../../tests/fixtures/expected-ir.json")).unwrap(),
        ScenarioConfig::land_return(),
    )
    .unwrap();
    let mut game = Game::new(prepared);
    let before = game.state.clone();
    for viewer in (0..4).map(PlayerId) {
        let knowledge = &game.state.knowledge.visibility[viewer.0 as usize];
        for (handle, object) in &knowledge.handles.entries {
            assert_eq!(game.handle_for(viewer, *object).unwrap().0, *handle);
        }
    }
    game.refresh_visibility().unwrap();
    assert_eq!(game.state, before);
    // Private visibility-domain fixture: owner still sees their hand; other
    // viewers must lose their old handle when the public object becomes hidden.
    let id = game.object_id(CardId(0)).unwrap();
    let owner = game.state.rules.cards[&id.card].owner;
    let other = PlayerId((owner.0 + 1) % 4);
    let old = game.handle_for(other, id).unwrap();
    game.state.rules.objects.get_mut(&id.card).unwrap().zone = Zone::Hand { owner };
    game.refresh_visibility().unwrap();
    assert!(game.resolve_handle(other, &old).is_err());
    assert!(game.handle_for(other, id).is_err());
    assert!(game.handle_for(owner, id).is_ok());
    game.state.rules.objects.get_mut(&id.card).unwrap().zone = Zone::Battlefield;
    game.refresh_visibility().unwrap();
    let revealed = game.handle_for(other, id).unwrap();
    assert_ne!(revealed, old);
    let next = ObjectId {
        incarnation: id.incarnation + 1,
        ..id
    };
    game.state.rules.objects.get_mut(&id.card).unwrap().id = next;
    game.refresh_visibility().unwrap();
    assert!(game.resolve_handle(other, &revealed).is_err());
    assert_ne!(game.handle_for(other, next).unwrap(), revealed);
}
