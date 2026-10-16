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
    let mut view = game.state.visibility[0].history.last().unwrap().clone();
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
        let base = game.state.visibility[viewer]
            .history
            .last()
            .unwrap()
            .clone();
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
            game.state.visibility[viewer].history.push(view);
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
    game.state.visibility[0].history[0].objects[0]
        .oracle_id
        .clear();
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
        game.state.visibility[0].history[0].objects[0].oracle_id = "x".repeat(target - base_length);
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
