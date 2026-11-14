//! Layer 2 for complete, independent creature-control Auras. Pinned CR 109.5,
//! 110.2, 302.6, 506.4, 613.1b/7a/7e and 800.4a-c.
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;

impl Game {
    fn supported_control_grant(&self, effect: &ControlEffect) -> bool {
        self.is_live(effect.controller)
            && match effect.source {
                ControlSource::Persistent => true,
                ControlSource::Static { object } => self
                    .current(object)
                    .is_ok_and(|source| source.zone == Zone::Battlefield),
            }
    }

    /// Derive the complete post-mutation view before changing any cached value.
    /// Synthetic grants retain their existing authority; they have no timestamp
    /// in the Aura clock, so composing the two orders is explicitly unsupported.
    fn derived_controllers(&self) -> Result<BTreeMap<ObjectId, PlayerId>, String> {
        let mut controllers = BTreeMap::new();
        let mut granted = BTreeSet::new();
        for object in self.state.rules.objects.values() {
            if object.zone != Zone::Battlefield {
                continue;
            }
            let grant = self
                .state
                .rules
                .control_effects
                .iter()
                .filter(|effect| effect.object == object.id && self.supported_control_grant(effect))
                .max_by_key(|effect| effect.id);
            if grant.is_some() {
                granted.insert(object.id);
            }
            controllers.insert(
                object.id,
                grant.map_or(object.default_controller, |effect| effect.controller),
            );
        }

        let mut candidates: BTreeMap<ObjectId, (u64, PlayerId)> = BTreeMap::new();
        let mut timestamps = BTreeSet::new();
        for source in self.state.rules.objects.values() {
            if source.zone != Zone::Battlefield {
                continue;
            }
            let definition = self.definition(source.id.card)?;
            if !definition
                .static_abilities
                .iter()
                .any(|ability| ability.effect == StaticEffect::ControlEnchantedCreature)
            {
                continue;
            }
            if !mtg_contracts::aura_control_source(definition) {
                return Err("unsupported dependent Aura control source".into());
            }
            let Some(target) = source.attached_to else {
                continue;
            };
            // A stale pre-SBA attachment is reachable, but supplies no grant.
            // Never follow a physical card to its successor incarnation.
            if !self.is_battlefield_creature(target) {
                continue;
            }
            let timestamp = source
                .attachment_timestamp
                .filter(|timestamp| {
                    *timestamp > 0 && *timestamp < self.state.rules.next_attachment_timestamp
                })
                .ok_or("missing or invalid Aura control timestamp")?;
            if !timestamps.insert(timestamp) {
                return Err("unsupported simultaneous Aura control timestamps".into());
            }
            let controller = controllers[&source.id];
            // A source with no living controller is about to be exiled under
            // 800.4a/c. It cannot transiently grant control during that cleanup.
            if !self.is_live(controller) {
                continue;
            }
            if granted.contains(&target) {
                return Err("unsupported composed Aura and synthetic control grants".into());
            }
            let candidate = candidates.entry(target).or_insert((timestamp, controller));
            if timestamp > candidate.0 {
                *candidate = (timestamp, controller);
            }
        }
        for (object, (_, controller)) in candidates {
            controllers.insert(object, controller);
        }
        // Actual admitted Aura sources are independent noncreatures. A private
        // synthetic static grant whose source itself needs control-related
        // exile would require another dependency pass, not ID-ordered cleanup.
        // Reject that unimplemented composition rather than cache a stale grant.
        if self.state.rules.control_effects.iter().any(|effect| {
            self.supported_control_grant(effect)
                && controllers.contains_key(&effect.object)
                && matches!(effect.source, ControlSource::Static { object }
                    if controllers.get(&object).is_some_and(|controller| !self.is_live(*controller)))
        }) {
            return Err("unsupported dependent synthetic control-source exile".into());
        }
        Ok(controllers)
    }

    /// Only layer-2 consequences: no stabilization, trigger placement, priority
    /// change or continuation replacement. The caller owns the atomic boundary.
    pub(super) fn reconcile_controllers(&mut self) -> Result<(), String> {
        let controllers = self.derived_controllers()?;
        let mut changed = vec![];
        let mut exiled = vec![];
        for (object, controller) in controllers {
            let previous = self.current(object)?.controller;
            if !self.is_live(controller) {
                exiled.push(object);
            } else if previous != controller {
                changed.push((object, previous, controller));
            }
        }
        if changed.is_empty() && exiled.is_empty() {
            return Ok(());
        }
        let authority = self.control_before()?;
        // Publish the complete layer-2 view before deriving any layer-6 source
        // facts. Its meaning cannot depend on iteration order among objects.
        for (object, _, controller) in &changed {
            let cached = self.state.rules.objects.get_mut(&object.card).unwrap();
            cached.controller = *controller;
            cached.attack_ready = false;
            self.remove_from_combat(*object);
        }
        for (object, previous, controller) in changed {
            let fact = self.object_before(object)?;
            self.retain_damage_control_facts(&fact);
            self.record_event(Event::ControlChanged {
                object,
                previous,
                controller,
            })?;
        }
        let prior = exiled
            .iter()
            .map(|object| self.object_before(*object))
            .collect::<Result<Vec<_>, _>>()?;
        for object in exiled {
            // A dedicated movement batch avoids commit -> apply_group ->
            // reconciliation recursion, while retaining consumed authority,
            // ordinary zone history, exact incarnation and bounded LKI.
            self.exile_without_controller(object, authority.clone())?;
        }
        for fact in prior {
            self.retain_source_facts(&fact);
        }
        Ok(())
    }

    pub(super) fn validate_control_state(&self) -> Result<(), String> {
        for (object, controller) in self.derived_controllers()? {
            if !self.is_live(controller) || self.current(object)?.controller != controller {
                return Err("snapshot cached controller invariant".into());
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Exact complete-source definitions, private board setup. These tests prove
    // reconciliation laws, not paid casting history or restore reachability.
    fn game(second_controller: u8, mode: TraceMode) -> Game {
        let mut catalog = Catalog::read(include_str!(
            "../../../../tests/fixtures/expected-attachments-ir.json"
        ))
        .unwrap();
        for raw in [
            include_str!("../../../../tests/fixtures/cd0d7141-46d2-4aa3-bc77-6b3b4513803e.json"),
            include_str!("../../../../tests/fixtures/5912546a-acc2-448c-b042-64bdac5ec129.json"),
        ] {
            let mtg_compiler::CompileResult::Lowered { definition } = mtg_compiler::compile(
                include_str!("../../../../tests/fixtures/archive-manifest.json"),
                raw,
            ) else {
                panic!("complete frozen Aura control source must lower")
            };
            catalog.definitions.push(*definition);
        }
        catalog
            .definitions
            .sort_by(|a, b| a.oracle_id.cmp(&b.oracle_id));
        catalog.validate().unwrap();
        let rows = [
            ("Control Magic", 1, 1),
            ("Mind Control", 2, second_controller),
            ("Grizzly Bears", 0, 0),
            ("Forest", 0, 0),
        ];
        let config = ScenarioConfig {
            initial: ScenarioInitial::default(),
            mode: Mode::CommanderMatch,
            players: 4,
            profile: "private-aura-control-kernel".into(),
            commanders: vec![],
            cards: rows
                .iter()
                .enumerate()
                .map(|(i, (name, owner, controller))| CardSetup {
                    id: CardId(i as u32),
                    oracle_id: catalog
                        .definitions
                        .iter()
                        .find(|d| d.name == *name)
                        .unwrap()
                        .oracle_id
                        .clone(),
                    owner: PlayerId(*owner),
                    controller: PlayerId(*controller),
                    zone: Zone::Battlefield,
                })
                .collect(),
        };
        Game::with_trace(
            Prepared {
                catalog_digest: digest(&catalog).unwrap(),
                profile_digest: digest(&config).unwrap(),
                catalog: Arc::new(catalog),
                config,
                match_config: None,
            },
            mode,
        )
    }

    fn object(game: &Game, card: u32) -> ObjectId {
        game.object_id(CardId(card)).unwrap()
    }

    fn attach(game: &mut Game, card: u32, target: Option<ObjectId>) {
        game.commit(
            ProposedEvent::Attach {
                object: object(game, card),
                target,
            },
            BatchCause::Fixture,
        )
        .unwrap();
    }

    fn control_changes(game: &Game) -> usize {
        game.events()
            .iter()
            .filter(|event| matches!(event, Event::ControlChanged { .. }))
            .count()
    }

    #[test]
    fn aura_control_timestamp_wins_independently_of_ids_and_trace() {
        let mut expected = None;
        for mode in [TraceMode::Off, TraceMode::Compact, TraceMode::Detailed] {
            let mut game = game(2, mode);
            let target = object(&game, 2);
            let cached = game.state.rules.objects.get_mut(&target.card).unwrap();
            cached.plus_one_counters = 3.into();
            cached.marked_damage = 1.into();
            cached.tapped = true;
            attach(&mut game, 1, Some(target));
            assert_eq!(game.current(target).unwrap().controller, PlayerId(2));
            attach(&mut game, 0, Some(target));
            let cached = game.current(target).unwrap();
            assert_eq!(cached.controller, PlayerId(1));
            assert_eq!(cached.default_controller, PlayerId(0));
            assert_eq!(cached.plus_one_counters, 3.into());
            assert_eq!(cached.marked_damage, 1.into());
            assert!(cached.tapped);
            assert!(!cached.attack_ready);
            assert_eq!(
                game.current(object(&game, 1)).unwrap().attachment_timestamp,
                Some(1)
            );
            assert_eq!(
                game.current(object(&game, 0)).unwrap().attachment_timestamp,
                Some(2)
            );
            game.validate_control_state().unwrap();
            if let Some(state) = &expected {
                assert_eq!(&game.state, state);
            } else {
                expected = Some(game.state.clone());
            }
        }
    }

    #[test]
    fn aura_control_same_controller_changes_preserve_combat_and_readiness() {
        let mut game = game(1, TraceMode::Detailed);
        let target = object(&game, 2);
        attach(&mut game, 0, Some(target));
        game.state
            .rules
            .objects
            .get_mut(&target.card)
            .unwrap()
            .attack_ready = true;
        game.state.rules.combat.push(Attacker {
            attacker: target,
            defender: PlayerId(3),
            blocked: false,
            blockers: vec![],
        });
        let changes = control_changes(&game);
        attach(&mut game, 1, Some(target));
        let timestamp = game.current(object(&game, 1)).unwrap().attachment_timestamp;
        attach(&mut game, 1, Some(target));
        attach(&mut game, 0, None);
        assert_eq!(
            game.current(object(&game, 1)).unwrap().attachment_timestamp,
            timestamp
        );
        assert!(game.current(target).unwrap().attack_ready);
        assert_eq!(game.state.rules.combat.len(), 1);
        assert_eq!(control_changes(&game), changes);
        attach(&mut game, 1, None);
        assert_eq!(game.current(target).unwrap().controller, PlayerId(0));
        assert!(!game.current(target).unwrap().attack_ready);
        assert!(game.state.rules.combat.is_empty());
        assert_eq!(control_changes(&game), changes + 1);
    }

    #[test]
    fn aura_control_simultaneous_target_departure_has_no_transient_restoration() {
        for reverse in [false, true] {
            let mut game = game(2, TraceMode::Detailed);
            let target = object(&game, 2);
            attach(&mut game, 0, Some(target));
            let changes = control_changes(&game);
            let mut occurrences = vec![
                ProposedEvent::Move {
                    object: object(&game, 0),
                    destination: Zone::Graveyard { owner: PlayerId(1) },
                },
                ProposedEvent::Move {
                    object: target,
                    destination: Zone::Graveyard { owner: PlayerId(0) },
                },
            ];
            if reverse {
                occurrences.reverse();
            }
            let batch = game
                .commit_batch(
                    BatchCause::Fixture,
                    EventGroup::Simultaneous { occurrences },
                )
                .unwrap();
            assert_eq!(control_changes(&game), changes);
            let prior = batch
                .occurrences
                .iter()
                .find(|occurrence| {
                    occurrence
                        .before
                        .objects
                        .iter()
                        .any(|fact| fact.object == target)
                })
                .unwrap();
            assert_eq!(
                prior
                    .before
                    .objects
                    .iter()
                    .find(|fact| fact.object == target)
                    .unwrap()
                    .controller,
                PlayerId(1)
            );
            game.validate_control_state().unwrap();
        }
    }

    #[test]
    fn aura_control_stale_attachment_and_forged_cache_are_distinct() {
        let mut game = game(2, TraceMode::Detailed);
        let target = object(&game, 2);
        attach(&mut game, 0, Some(target));
        let mut forged = game.fork();
        forged
            .state
            .rules
            .objects
            .get_mut(&target.card)
            .unwrap()
            .controller = PlayerId(0);
        assert!(forged.validate_control_state().is_err());
        let next = game
            .move_object(target, Zone::Hand { owner: PlayerId(0) })
            .unwrap()
            .successor;
        game.validate_attachment_state().unwrap();
        game.validate_control_state().unwrap();
        let successor = game.move_object(next, Zone::Battlefield).unwrap().successor;
        assert_eq!(game.current(successor).unwrap().controller, PlayerId(0));
        assert_eq!(
            game.current(object(&game, 0)).unwrap().attached_to,
            Some(target)
        );
        game.validate_control_state().unwrap();
    }

    #[test]
    fn aura_control_dead_default_exiles_immediately_with_consumed_authority() {
        let mut game = game(2, TraceMode::Detailed);
        let target = object(&game, 2);
        // Default control is not ownership: owner zero survives controller three.
        game.state
            .rules
            .objects
            .get_mut(&target.card)
            .unwrap()
            .default_controller = PlayerId(3);
        attach(&mut game, 0, Some(target));
        game.apply_departure(PlayerId(3)).unwrap();
        assert_eq!(game.current(target).unwrap().controller, PlayerId(1));
        let frame = game.state.rules.frame.clone();
        let priority = game.state.rules.priority;
        let pending = game.state.rules.pending_triggers.clone();
        attach(&mut game, 0, None);
        let successor = object(&game, 2);
        assert_eq!(successor.incarnation, target.incarnation + 1);
        assert_eq!(game.current(successor).unwrap().zone, Zone::Exile);
        assert_eq!(game.state.rules.frame, frame);
        assert_eq!(game.state.rules.priority, priority);
        assert_eq!(game.state.rules.pending_triggers, pending);
        let batch = game
            .trace
            .records
            .iter()
            .find_map(|record| match &record.record {
                EvidenceRecord::Batch { batch }
                    if batch.cause == BatchCause::ControlReconciliation =>
                {
                    Some(batch)
                }
                _ => None,
            })
            .unwrap();
        assert!(
            matches!(batch.result(&[]), Some(EventOutcome::Moved { movement }) if movement.previous == target && movement.destination == Zone::Exile)
        );
        let authority = batch.before.control.as_ref().unwrap();
        assert_eq!(
            authority.live_players,
            vec![PlayerId(0), PlayerId(1), PlayerId(2)]
        );
        assert!(
            authority
                .objects
                .iter()
                .any(|fact| fact.object == target && fact.default_controller == PlayerId(3))
        );
    }

    #[test]
    fn aura_control_mixed_synthetic_and_simultaneous_attachment_dependencies_reject_atomically() {
        let mut game = game(2, TraceMode::Detailed);
        let target = object(&game, 2);
        game.state.rules.control_effects.push(ControlEffect {
            id: 1,
            object: target,
            controller: PlayerId(0),
            source: ControlSource::Persistent,
        });
        let before = game.fork();
        assert!(
            game.commit(
                ProposedEvent::Attach {
                    object: object(&game, 0),
                    target: Some(target)
                },
                BatchCause::Fixture
            )
            .is_err()
        );
        assert_eq!(game.state, before.state);
        assert_eq!(game.trace, before.trace);
        game.state.rules.control_effects.clear();
        let before = game.fork();
        assert!(
            game.commit_batch(
                BatchCause::Fixture,
                EventGroup::Simultaneous {
                    occurrences: vec![
                        ProposedEvent::Attach {
                            object: object(&game, 0),
                            target: Some(target)
                        },
                        ProposedEvent::Tap {
                            object: object(&game, 3)
                        },
                    ]
                }
            )
            .is_err()
        );
        assert_eq!(game.state, before.state);
        assert_eq!(game.trace, before.trace);

        // Synthetic-only dependency, not an admitted source/history: the grant
        // cannot survive an exile of its own exact static-source incarnation.
        let mut dependent = self::game(2, TraceMode::Detailed);
        let source = object(&dependent, 3);
        dependent
            .state
            .rules
            .objects
            .get_mut(&source.card)
            .unwrap()
            .default_controller = PlayerId(3);
        dependent.state.rules.losing_players.push(PlayerId(3));
        dependent.state.rules.control_effects.push(ControlEffect {
            id: 1,
            object: object(&dependent, 2),
            controller: PlayerId(1),
            source: ControlSource::Static { object: source },
        });
        let before = dependent.fork();
        assert!(
            dependent
                .reconcile_controllers()
                .unwrap_err()
                .contains("dependent synthetic control-source exile")
        );
        assert_eq!(dependent.state, before.state);
        assert_eq!(dependent.trace, before.trace);
    }

    #[test]
    fn aura_control_low_level_sacrificed_receipt_restores_surviving_authority_only() {
        // Explicit synthetic kernel receipt: no currently admitted complete
        // source has a sacrifice mana cost. This is not a paid-sacrifice or
        // replay/restore-history witness, and does not broaden cost admission.
        let mut game = game(2, TraceMode::Detailed);
        let target = object(&game, 2);
        attach(&mut game, 1, Some(target));
        attach(&mut game, 0, Some(target));
        game.state
            .rules
            .objects
            .get_mut(&target.card)
            .unwrap()
            .plus_one_counters = 3.into();
        let receipt = PermanentManaUndo {
            player: PlayerId(1),
            source: game.object_before(target).unwrap(),
            counters: 3.into(),
            produced: BTreeMap::from([(Color::U, 1.into())]),
            spent: BTreeMap::from([(Color::G, 1.into())]),
            absent: BTreeSet::from([Color::U]),
            sacrificed: true,
            cost_triggers: vec![],
        };
        game.state.rules.players[1].mana.insert(Color::U, 1.into());
        game.move_object(target, Zone::Graveyard { owner: PlayerId(0) })
            .unwrap();
        game.move_object(object(&game, 0), Zone::Graveyard { owner: PlayerId(1) })
            .unwrap();
        let frame = game.state.rules.frame.clone();
        game.commit(
            ProposedEvent::ReversePermanentMana {
                receipt: Box::new(receipt),
            },
            BatchCause::Fixture,
        )
        .unwrap();
        let restored = game.current(target).unwrap();
        assert_eq!(restored.controller, PlayerId(2));
        assert_eq!(restored.default_controller, PlayerId(0));
        assert_eq!(restored.plus_one_counters, 3.into());
        assert!(!restored.attack_ready);
        assert_eq!(
            game.current(object(&game, 0)).unwrap().zone,
            Zone::Graveyard { owner: PlayerId(1) }
        );
        assert_eq!(
            game.state.rules.players[1].mana.get(&Color::G),
            Some(&1.into())
        );
        assert!(!game.state.rules.players[1].mana.contains_key(&Color::U));
        assert_eq!(game.state.rules.frame, frame);
        game.validate_control_state().unwrap();
    }
}
