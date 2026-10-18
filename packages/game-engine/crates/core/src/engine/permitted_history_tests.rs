//! Bounded independent old-owned serialization domain, not memory evidence.
use super::*;

fn fixture() -> PermittedView {
    let g = Game::new(
        Prepared::admit(
            Catalog::read(include_str!("../../../../tests/fixtures/expected-ir.json")).unwrap(),
            ScenarioConfig::land_return(),
        )
        .unwrap(),
    );
    // Generic public serde is deliberately independent of stored to_public().
    serde_json::from_value(
        serde_json::to_value(g.state.visibility[0].history.last().unwrap()).unwrap(),
    )
    .unwrap()
}
fn rich_fixture() -> PermittedView {
    // Untrusted serialization-domain values, never an admitted position/capture.
    let mut v = fixture();
    let row = v.objects.first_mut().unwrap();
    row.oracle_id = "serialization:雪\n\u{0}".into();
    row.marked_damage = (-7).into();
    row.attack_ready = true;
    row.tapped = true;
    let h = row.handle.clone();
    let target = VisibleTarget {
        announced: h.clone(),
        current: None,
    };
    v.setup = Some(VisibleSetup {
        round: 3,
        phase: VisibleSetupPhase::Bottom,
        players: vec![VisibleSetupPlayer {
            player: PlayerId(3),
            kept: false,
            mulligans_taken: 3,
        }],
    });
    v.effects = vec![VisibleEffect::ModifyPowerToughness {
        handle: EffectHandle("effect:雪".into()),
        origin: VisibleEffectSource {
            source: target.clone(),
            oracle_id: "source:雪".into(),
            controller: PlayerId(2),
        },
        target: target.clone(),
        power: (-2).into(),
        toughness: 3.into(),
        expires_in_cleanup_of_turn: 777,
    }];
    v.combat = vec![VisibleAttacker {
        attacker: h.clone(),
        defender: PlayerId(3),
        blocked: true,
        blockers: vec![h.clone(), h.clone()],
    }];
    v.active = None;
    v.priority = None;
    v.live_turn_order = vec![PlayerId(3), PlayerId(1)];
    v.commanders = vec![VisibleCommander {
        designation: "past:雪".into(),
        oracle_id: "commander".into(),
        history: CommanderHistory {
            owner: PlayerId(2),
            color_identity: vec![Color::B, Color::G],
            command_casts: BTreeMap::from([(PlayerId(2), 11.into()), (PlayerId(10), (-13).into())]),
            combat_damage: BTreeMap::from([(PlayerId(3), 21.into())]),
        },
    }];
    v.stack = vec![VisibleStackEntry {
        mode: Some(1),
        triggered: Some(VisibleTriggered {
            handle: StackHandle("trigger".into()),
            source: target.clone(),
            oracle_id: "trigger-source".into(),
            ability: 2,
        }),
        entity_targets: vec![
            VisibleEntityTarget::Player {
                player: PlayerId(1),
            },
            VisibleEntityTarget::Object {
                object: target.clone(),
            },
        ],
        handle: Some(StackHandle("stack".into())),
        stack_targets: vec![VisibleStackTarget {
            announced: StackHandle("past-stack".into()),
            current: None,
        }],
        spell: Some(h),
        ability: Some(VisibleAbility {
            handle: StackHandle("ability".into()),
            source: target.clone(),
            oracle_id: "ability-source".into(),
            ability: 0,
        }),
        player_targets: vec![PlayerId(2)],
        controller: PlayerId(1),
        cast_complete: true,
        targets: vec![target],
    }];
    v
}
#[test]
fn permitted_history_old_array_oracle_keeps_all_fields_duplicates_order_and_owned_access() {
    let rich = rich_fixture();
    for n in [0, 1, 75] {
        let mut old = vec![rich.clone(); n];
        for (i, v) in old.iter_mut().enumerate() {
            v.viewer = PlayerId((i % 4) as u8); // Deliberately mixed, not a seal.
            v.turn.number = i as u64 + 1;
            if i % 3 == 0 {
                v.objects.push(v.objects[0].clone());
            }
            if i % 3 == 1 {
                v.objects.reverse();
            }
        }
        let history = PermittedHistory::from_views(old.clone());
        assert_eq!(history.len(), old.len());
        assert_eq!(history.is_empty(), old.is_empty());
        assert_eq!(
            serde_json::to_string(&history).unwrap(),
            serde_json::to_string(&old).unwrap()
        );
        assert_eq!(canonical(&history).unwrap(), canonical(&old).unwrap());
        assert_eq!(history.iter().collect::<Vec<_>>(), old);
        assert_eq!(
            history.iter().rev().collect::<Vec<_>>(),
            old.iter().rev().cloned().collect::<Vec<_>>()
        );
        assert!(history.get(n).is_none());
        let cold: PermittedHistory = strict_read(&canonical(&old).unwrap()).unwrap();
        assert_eq!(cold, history);
        assert_eq!(format!("{cold:?}"), format!("{history:?}"));
        let clone = history.clone();
        if let Some(mut returned) = history.get(0) {
            returned.objects[0].oracle_id.push_str("independent edit");
            returned.objects.clear();
            assert_eq!(history.get(0).unwrap(), old[0]);
            assert_eq!(clone, history);
        }
    }
}
#[test]
fn permitted_history_decoder_is_exact_array_not_an_alias_or_game_validator() {
    let old = vec![rich_fixture()];
    let good = canonical(&old).unwrap();
    let mut value: serde_json::Value = serde_json::from_str(&good).unwrap();
    value[0]["cache"] = true.into();
    assert!(strict_read::<PermittedHistory>(&value.to_string()).is_err());
    let mut value: serde_json::Value = serde_json::from_str(&good).unwrap();
    value[0]["objects"][0]["alias"] = 1.into();
    assert!(strict_read::<PermittedHistory>(&value.to_string()).is_err());
    for bad in [
        format!("{{\"history\":{good}}}"),
        format!("{good} null"),
        good.replacen("\"viewer\":0", "\"viewer\":0,\"viewer\":0", 1),
    ] {
        assert!(strict_read::<PermittedHistory>(&bad).is_err(), "{bad}");
    }
    // Independently allocated equal values compare equal; a changed complete
    // field must never be ignored because pointers/lengths/handles coincide.
    for field in [
        "setup",
        "effects",
        "combat",
        "active",
        "priority",
        "commanders",
        "stack",
    ] {
        let mut changed: serde_json::Value = serde_json::from_str(&good).unwrap();
        let original = changed[0][field].clone();
        changed[0][field] = match field {
            "setup" => serde_json::Value::Null,
            "active" | "priority" => 1.into(),
            _ => serde_json::json!([]),
        };
        assert_ne!(original, changed[0][field]);
        let changed: PermittedHistory = strict_read(&changed.to_string()).unwrap();
        assert_ne!(
            changed,
            PermittedHistory::from_views(old.clone()),
            "ignored {field}"
        );
    }
}
#[test]
fn permitted_history_keeps_warmed_rows_alive_and_detached_after_source_edits_and_drop() {
    let original = rich_fixture();
    let mut stored = StoredHistory::default();
    for _ in 0..70 {
        stored.push_public(original.clone());
    }
    let mut exported = PermittedHistory::default();
    for view in stored.iter() {
        exported.push_stored(view.clone());
    }
    let before = canonical(&vec![original.clone(); 70]).unwrap();
    for i in 0..70 {
        exported
            .stored(i)
            .unwrap()
            .write_canonical(&mut vec![])
            .unwrap();
    }
    stored[33].mutate_object(0, |r| r.oracle_id = "changed source".into());
    stored[34].update_public(|v| v.turn.number = 1001);
    assert_ne!(canonical(&stored).unwrap(), before);
    drop(stored);
    assert_eq!(canonical(&exported).unwrap(), before);
    let sibling = exported.clone();
    let mut owned = exported.get(33).unwrap();
    owned.objects[0].marked_damage = 999.into();
    assert_eq!(sibling.get(33).unwrap(), original);
    assert_eq!(exported, sibling);
}

#[test]
fn permitted_history_incremental_decode_ignores_untrusted_sequence_size_hint() {
    use serde::de::{DeserializeSeed, SeqAccess, Visitor};
    struct Hint(Option<serde_json::Value>);
    impl<'de> SeqAccess<'de> for Hint {
        type Error = serde_json::Error;
        fn next_element_seed<T: DeserializeSeed<'de>>(
            &mut self,
            seed: T,
        ) -> Result<Option<T::Value>, Self::Error> {
            self.0
                .take()
                .map(|value| seed.deserialize(value))
                .transpose()
        }
        fn size_hint(&self) -> Option<usize> {
            Some(usize::MAX)
        }
    }
    impl<'de> serde::Deserializer<'de> for Hint {
        type Error = serde_json::Error;
        fn deserialize_any<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Self::Error> {
            visitor.visit_seq(self)
        }
        serde::forward_to_deserialize_any! {
            bool i8 i16 i32 i64 u8 u16 u32 u64 f32 f64 char str string
            bytes byte_buf option unit unit_struct newtype_struct seq tuple
            tuple_struct map struct enum identifier ignored_any
        }
    }
    let view = rich_fixture();
    let history =
        PermittedHistory::deserialize(Hint(Some(serde_json::to_value(&view).unwrap()))).unwrap();
    assert_eq!(history.len(), 1);
    assert_eq!(history.get(0), Some(view));
}
