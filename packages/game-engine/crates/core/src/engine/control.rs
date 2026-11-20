//! Layer 2 for complete permanent-control Auras. Pinned CR 109.5, 110.2,
//! 302.6, 506.4, 611.3, 613.1b/7a/7e/8a-c and 800.4a-c.
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;

/// Scratch-only layer-2 instruction. The complete source recognizer excludes
/// text/type/ability-changing effects; only the meaning of "you" can depend on
/// another layer-2 effect. No final cached controller is an input.
#[derive(Clone, Copy)]
struct AuraControlEffect {
    source: usize,
    target: usize,
    timestamp: u64,
}

/// CR 109.5/611.3a/613.8a: the structural link is only a potential
/// dependency. Applying the other effect must *change* what this effect does.
/// This is a derivation of the pinned rules, not an official worked-example
/// ruling: assigning the same source controller leaves the recipient player
/// unchanged. Compare attempted operations, NOT whether this effect's target
/// already has that controller. A departed-player grant cannot apply (800.4b).
/// Test against the current scratch view, never final winners.
fn aura_control_depends(
    effect: AuraControlEffect,
    other: AuraControlEffect,
    controllers: &[PlayerId],
    live: &[PlayerId],
) -> bool {
    other.target == effect.source
        && live.contains(&controllers[other.source])
        && controllers[other.source] != controllers[effect.source]
}

/// The admitted producer always attaches a fresh Aura to an existing permanent;
/// it cannot create a source-attachment cycle. Reject structural cycles even if
/// equal controllers would hide their current semantic edges. General synthetic
/// CR 613.8b loop interpretation remains outside this runtime's source boundary.
fn validate_aura_control_graph(
    effects: &[AuraControlEffect],
    source_effects: &[Option<usize>],
) -> Result<(), String> {
    let mut prerequisites = vec![0; effects.len()];
    for effect in effects {
        if let Some(dependent) = source_effects[effect.target] {
            prerequisites[dependent] += 1;
        }
    }
    let mut ready: Vec<_> = prerequisites
        .iter()
        .enumerate()
        .filter_map(|(i, &count)| (count == 0).then_some(i))
        .collect();
    let mut visited = 0;
    while let Some(i) = ready.pop() {
        visited += 1;
        if let Some(dependent) = source_effects[effects[i].target] {
            prerequisites[dependent] -= 1;
            if prerequisites[dependent] == 0 {
                ready.push(dependent);
            }
        }
    }
    if visited != effects.len() {
        return Err("unsupported cyclic Aura control sources".into());
    }
    Ok(())
}

/// CR 613.7/613.8a,c: select the earliest remaining effect with no current
/// semantic prerequisites, apply ONE, then reevaluate. Each effect targets one
/// object, which generates at most one control effect: at most A potential
/// edges including fan-in, so each pass is O(A), not a pairwise scan.
fn next_aura_control_effect(
    effects: &[AuraControlEffect],
    applied: &[bool],
    source_effects: &[Option<usize>],
    controllers: &[PlayerId],
    live: &[PlayerId],
) -> Option<usize> {
    let mut blocked = vec![false; effects.len()];
    for (other, grant) in effects.iter().enumerate() {
        if !applied[other]
            && let Some(effect) = source_effects[grant.target]
            && !applied[effect]
            && aura_control_depends(effects[effect], *grant, controllers, live)
        {
            blocked[effect] = true;
        }
    }
    (0..effects.len())
        .filter(|&i| !applied[i] && !blocked[i])
        .min_by_key(|&i| effects[i].timestamp)
}

/// Dense battlefield indices bound scratch work to O(B + A²). No fixed-point,
/// cached final controller, recursive winner-following or persistent graph.
fn apply_aura_control_effects(
    effects: &[AuraControlEffect],
    controllers: &mut [PlayerId],
    live: &[PlayerId],
) -> Result<(), String> {
    let mut source_effects = vec![None; controllers.len()];
    for (i, effect) in effects.iter().enumerate() {
        source_effects[effect.source] = Some(i);
    }
    validate_aura_control_graph(effects, &source_effects)?;
    let mut applied = vec![false; effects.len()];
    for _ in effects {
        let next = next_aura_control_effect(effects, &applied, &source_effects, controllers, live)
            .ok_or("unsupported cyclic Aura control dependencies")?;
        let effect = effects[next];
        applied[next] = true;
        // A dead-default source can first be rescued by another Aura. A source
        // still dead when its effect applies grants nothing before 800.4a/c exile.
        if live.contains(&controllers[effect.source]) {
            controllers[effect.target] = controllers[effect.source];
        }
    }
    Ok(())
}

impl Game {
    fn supported_control_grant(&self, effect: &ControlEffect) -> bool {
        self.is_live(effect.controller)
            && match effect.source {
                ControlSource::Persistent => true,
                ControlSource::Static { object } => self.control_battlefield(object),
            }
    }

    /// CR 800.4a: all frozen losing owners' objects are absent from the virtual
    /// battlefield even during the existing sequential physical removal wave.
    fn control_battlefield(&self, object: ObjectId) -> bool {
        self.current(object).is_ok_and(|o| {
            o.zone == Zone::Battlefield && self.is_live(self.state.rules.cards[&object.card].owner)
        })
    }

    /// Derive the complete post-mutation view before changing any cached value.
    /// Synthetic grants retain their existing authority; they have no timestamp
    /// in the Aura clock, so composing the two orders is explicitly unsupported.
    fn derived_controllers(&self) -> Result<BTreeMap<ObjectId, PlayerId>, String> {
        let mut controllers = BTreeMap::new();
        let mut granted = BTreeSet::new();
        for object in self.state.rules.objects.values() {
            if !self.control_battlefield(object.id) {
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

        let indices: BTreeMap<_, _> = controllers
            .keys()
            .enumerate()
            .map(|(i, id)| (*id, i))
            .collect();
        let mut scratch: Vec<_> = controllers.values().copied().collect();
        let mut effects = vec![];
        let mut timestamps = BTreeSet::new();
        for source in self.state.rules.objects.values() {
            if !self.control_battlefield(source.id) {
                continue;
            }
            let definition = self.definition(source.id.card)?;
            if !definition
                .static_abilities
                .iter()
                .any(|ability| ability.effect == StaticEffect::ControlEnchantedPermanent)
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
            if !self.control_battlefield(target)
                || !self.attachment_target_type_valid(source.id.card, target.card)?
            {
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
            if granted.contains(&target) {
                return Err("unsupported composed Aura and synthetic control grants".into());
            }
            effects.push(AuraControlEffect {
                source: indices[&source.id],
                target: indices[&target],
                timestamp,
            });
        }
        apply_aura_control_effects(&effects, &mut scratch, &self.live_players())?;
        for (controller, final_controller) in controllers.values_mut().zip(scratch) {
            *controller = final_controller;
        }
        // Synthetic grants have no timestamp in the Aura clock. In particular,
        // a static grant whose exact source needs control-related exile would
        // require removing that grant and reevaluating. Preserve the explicit
        // boundary rather than publish an ID-ordered or stale interpretation.
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
        kernel_game(
            &[
                ("Control Magic", 1, 1),
                ("Mind Control", 2, second_controller),
                ("Grizzly Bears", 0, 0),
                ("Forest", 0, 0),
            ],
            &[],
            mode,
        )
    }

    fn kernel_game(rows: &[(&str, u8, u8)], extras: &[&str], mode: TraceMode) -> Game {
        let mut catalog = Catalog::read(include_str!(
            "../../../../tests/fixtures/expected-attachments-ir.json"
        ))
        .unwrap();
        for raw in [
            include_str!("../../../../tests/fixtures/cd0d7141-46d2-4aa3-bc77-6b3b4513803e.json"),
            include_str!("../../../../tests/fixtures/5912546a-acc2-448c-b042-64bdac5ec129.json"),
        ]
        .into_iter()
        .chain(extras.iter().copied())
        {
            let mtg_compiler::CompileResult::Lowered { definition } = mtg_compiler::compile(
                include_str!("../../../../tests/fixtures/archive-manifest-current.json"),
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

    // New typed-domain kernel tests below are UNRUN pending the parent's
    // combined native pass. Exact complete frozen definitions, private setup;
    // no paid replay, snapshot reachability, catalog or consumer admission claim.
    fn typed_game(mode: TraceMode) -> Game {
        kernel_game(
            &[
                ("Steal Artifact", 1, 1),    // 0
                ("Annex", 2, 2),             // 1
                ("Conquer", 3, 3),           // 2
                ("Silver Myr", 0, 0),        // 3: artifact creature
                ("Seat of the Synod", 0, 0), // 4: artifact land
                ("Grizzly Bears", 0, 0),     // 5: creature only
                ("Forest", 0, 0),            // 6: land only
                ("Bonesplitter", 0, 0),      // 7: artifact, Equipment
                ("Rancor", 0, 0),            // 8: old noncontrol Aura
                ("Control Magic", 1, 1),     // 9
            ],
            &[
                include_str!(
                    "../../../../tests/fixtures/cd8ae9f2-edac-473a-8846-c08219e617c3.json"
                ),
                include_str!(
                    "../../../../tests/fixtures/5f1f49dd-b7ce-494a-9b4b-ac702d3664c5.json"
                ),
                include_str!(
                    "../../../../tests/fixtures/be15e131-0753-4ceb-a36b-4f9172241080.json"
                ),
                include_str!(
                    "../../../../tests/fixtures/66e8f7f8-3a6d-46ba-837c-b9713ddf7f40.json"
                ),
                include_str!(
                    "../../../../tests/fixtures/39451b4d-cd7a-40da-b457-cb51b609173f.json"
                ),
            ],
            mode,
        )
    }

    fn rejected_unchanged(game: &mut Game, group: EventGroup) {
        let before = game.fork();
        assert!(game.commit_batch(BatchCause::Fixture, group).is_err());
        assert_eq!(game.state, before.state);
        assert_eq!(game.trace, before.trace);
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

    // General-control kernel tests are UNRUN. These use frozen complete
    // definitions with private board construction, not paid casting histories.
    fn general_game(rows: &[(&str, u8, u8)], mode: TraceMode) -> Game {
        kernel_game(
            rows,
            &[include_str!(
                "../../../../tests/fixtures/d599a38c-7719-443e-bb59-55bd43a8fee6.json"
            )],
            mode,
        )
    }

    fn chronology_game(mode: TraceMode) -> Game {
        general_game(
            &[
                ("Control Magic", 0, 0), // A
                ("Mind Control", 1, 1),  // C: independent competing grant
                ("Confiscate", 2, 2),    // B: controls A
                ("Grizzly Bears", 3, 3),
            ],
            mode,
        )
    }

    #[test]
    fn general_aura_kernel_chronology_applies_dependencies_once_before_competing_grants() {
        // CR 109.5/611.3a/613.7/613.8a-c. In A,C,B order C is independent
        // and oldest eligible, then B, then A; A,B,C instead applies B,A,C.
        // These private histories distinguish dependency order from recursively
        // following the final winning Aura's controller. No paid-cast claim.
        for (order, expected) in [([0, 1, 2], 2), ([0, 2, 1], 1)] {
            let mut expected_state = None;
            for mode in [TraceMode::Off, TraceMode::Compact, TraceMode::Detailed] {
                let mut game = chronology_game(mode);
                let target = object(&game, 3);
                let source = object(&game, 0);
                for card in order {
                    attach(
                        &mut game,
                        card,
                        Some(if card == 2 { source } else { target }),
                    );
                }
                assert_eq!(game.current(target).unwrap().controller, PlayerId(expected));
                assert_eq!(game.current(source).unwrap().controller, PlayerId(2));
                game.validate_control_state().unwrap();
                if let Some(state) = &expected_state {
                    assert_eq!(&game.state, state);
                } else {
                    expected_state = Some(game.state.clone());
                }
            }
        }
    }

    #[test]
    fn general_aura_kernel_same_controller_link_compares_attempted_operation() {
        // Derived from pinned CR 109.5/611.3a/613.8a,c, not an official worked
        // example. B's attempted assignment stays creature -> player 0 after A
        // assigns B -> player 0, so B is initially independent. A depends on D.
        // Apply B,D,A once: B/A end under player 1, creature stays under player 0.
        let mut game = general_game(
            &[
                ("Control Magic", 0, 0), // B
                ("Confiscate", 0, 0),    // A
                ("Confiscate", 1, 1),    // D
                ("Grizzly Bears", 0, 0),
            ],
            TraceMode::Detailed,
        );
        let b = object(&game, 0);
        let a = object(&game, 1);
        let target = object(&game, 3);
        attach(&mut game, 0, Some(target));
        attach(&mut game, 1, Some(b));
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
        let events = game.events().len();
        attach(&mut game, 2, Some(a));
        assert_eq!(game.current(b).unwrap().controller, PlayerId(1));
        assert_eq!(game.current(a).unwrap().controller, PlayerId(1));
        assert_eq!(game.current(target).unwrap().controller, PlayerId(0));
        assert!(game.current(target).unwrap().attack_ready);
        assert_eq!(game.state.rules.combat.len(), 1);
        assert!(!game.events()[events..].iter().any(|event| matches!(event,
            Event::ControlChanged { object, .. } if *object == target)));
        game.validate_control_state().unwrap();
        // The source/target already sharing a controller does NOT suppress B's
        // dependency on an unequal assignment to its source.
        attach(&mut game, 1, None);
        assert_eq!(game.current(target).unwrap().controller, PlayerId(0));
        attach(&mut game, 2, Some(b));
        assert_eq!(game.current(target).unwrap().controller, PlayerId(1));
    }

    #[test]
    fn general_aura_kernel_synthetic_cycles_reject_even_when_values_hide_edges() {
        // SYNTHETIC: current cast-only producers cannot create these cycles.
        // No assertion of CR 613.8b outcomes; structural cycles remain outside
        // the supported producer boundary. Self-attachment is separately illegal.
        for size in [2, 3] {
            for equal in [false, true] {
                let rows: Vec<_> = (0..4)
                    .map(|i| ("Confiscate", i, if equal { 0 } else { i }))
                    .collect();
                let mut game = general_game(&rows, TraceMode::Detailed);
                for card in 0..size - 1 {
                    let target = object(&game, card + 1);
                    attach(&mut game, card, Some(target));
                }
                let first = object(&game, 0);
                let last = object(&game, size - 1);
                let before = game.fork();
                let error = game
                    .commit(
                        ProposedEvent::Attach {
                            object: last,
                            target: Some(first),
                        },
                        BatchCause::Fixture,
                    )
                    .unwrap_err();
                assert!(error.contains("cyclic Aura control"));
                assert_eq!(game.state, before.state);
                assert_eq!(game.trace, before.trace);

                // Also reject a cycle with an external prerequisite. Exercise
                // the scratch seam directly: graph rejection precedes cache writes.
                let cycle_target = object(&game, 0);
                let external = object(&game, 3);
                game.state
                    .rules
                    .objects
                    .get_mut(&last.card)
                    .unwrap()
                    .attached_to = Some(cycle_target);
                game.state
                    .rules
                    .objects
                    .get_mut(&last.card)
                    .unwrap()
                    .attachment_timestamp = Some(3);
                game.state
                    .rules
                    .objects
                    .get_mut(&external.card)
                    .unwrap()
                    .attached_to = Some(cycle_target);
                game.state
                    .rules
                    .objects
                    .get_mut(&external.card)
                    .unwrap()
                    .attachment_timestamp = Some(4);
                game.state.rules.next_attachment_timestamp = 5;
                let before = game.fork();
                assert!(
                    game.reconcile_controllers()
                        .unwrap_err()
                        .contains("cyclic Aura control")
                );
                assert_eq!(game.state, before.state);
                assert_eq!(game.trace, before.trace);
            }
        }
    }

    #[test]
    fn general_aura_kernel_synthetic_capacity_chain_is_iteration_order_independent() {
        // SYNTHETIC dense scratch population at the four-seat physical-card
        // bound. This tests allocation/iteration, not paid reachability or tokens.
        let count = capacities::OWNED_LIMIT * 4;
        let effects: Vec<_> = (1..count)
            .map(|source| AuraControlEffect {
                source,
                target: source - 1,
                timestamp: source as u64,
            })
            .collect();
        let initial: Vec<_> = (0..count).map(|i| PlayerId((i % 4) as u8)).collect();
        let live = [PlayerId(0), PlayerId(1), PlayerId(2), PlayerId(3)];
        for order in [effects.clone(), effects.iter().rev().copied().collect()] {
            let mut controllers = initial.clone();
            apply_aura_control_effects(&order, &mut controllers, &live).unwrap();
            assert!(controllers.iter().all(|p| *p == initial[count - 1]));
        }
        // Empty population and independent grants remain bounded as well.
        apply_aura_control_effects(&[], &mut [], &live).unwrap();
    }

    #[test]
    fn general_aura_kernel_fan_in_uses_timestamp_after_all_prerequisites() {
        let mut game = general_game(
            &[
                ("Control Magic", 0, 0),
                ("Confiscate", 1, 1),
                ("Confiscate", 2, 2),
                ("Grizzly Bears", 3, 3),
            ],
            TraceMode::Detailed,
        );
        let source = object(&game, 0);
        let target = object(&game, 3);
        attach(&mut game, 0, Some(target));
        attach(&mut game, 1, Some(source));
        let events = game.events().len();
        attach(&mut game, 2, Some(source));
        assert_eq!(game.current(source).unwrap().controller, PlayerId(2));
        assert_eq!(game.current(target).unwrap().controller, PlayerId(2));
        let changes: Vec<_> = game.events()[events..]
            .iter()
            .filter_map(|event| match event {
                Event::ControlChanged {
                    object,
                    previous,
                    controller,
                } => Some((*object, *previous, *controller)),
                _ => None,
            })
            .collect();
        // Scratch's older grant never leaks a cache transition or event.
        assert_eq!(
            changes,
            vec![
                (source, PlayerId(1), PlayerId(2)),
                (target, PlayerId(1), PlayerId(2))
            ]
        );
        attach(&mut game, 2, None);
        assert_eq!(game.current(target).unwrap().controller, PlayerId(1));
        attach(&mut game, 1, None);
        assert_eq!(game.current(target).unwrap().controller, PlayerId(0));
        game.validate_control_state().unwrap();
    }

    #[test]
    fn general_aura_kernel_source_removal_and_reincarnation_never_follow_successor() {
        let mut game = chronology_game(TraceMode::Detailed);
        let source = object(&game, 0);
        let target = object(&game, 3);
        attach(&mut game, 0, Some(target));
        attach(&mut game, 2, Some(source));
        let hand = game
            .move_object(source, Zone::Hand { owner: PlayerId(0) })
            .unwrap()
            .successor;
        assert_eq!(game.current(target).unwrap().controller, PlayerId(3));
        let returned = game.move_object(hand, Zone::Battlefield).unwrap().successor;
        assert_eq!(game.current(returned).unwrap().controller, PlayerId(0));
        assert_eq!(
            game.current(object(&game, 2)).unwrap().attached_to,
            Some(source)
        );
        assert_eq!(game.current(returned).unwrap().attached_to, None);
        game.validate_control_state().unwrap();
        game.validate_attachment_state().unwrap();
    }

    #[test]
    fn general_aura_kernel_source_cache_is_never_derivation_authority() {
        let mut game = chronology_game(TraceMode::Detailed);
        let target = object(&game, 3);
        attach(&mut game, 0, Some(target));
        let source = object(&game, 0);
        attach(&mut game, 2, Some(source));
        let expected = game.derived_controllers().unwrap();
        for cached in game.state.rules.objects.values_mut() {
            cached.controller = PlayerId(3);
        }
        assert_eq!(game.derived_controllers().unwrap(), expected);
        assert!(game.validate_control_state().is_err());
    }

    #[test]
    fn general_aura_kernel_source_with_dead_default_can_be_preserved_before_application() {
        let mut game = chronology_game(TraceMode::Detailed);
        let source = object(&game, 0);
        let target = object(&game, 3);
        attach(&mut game, 0, Some(target));
        attach(&mut game, 2, Some(source));
        // Private boundary setup: source owner survives its default controller.
        game.state
            .rules
            .objects
            .get_mut(&source.card)
            .unwrap()
            .default_controller = PlayerId(1);
        game.state.rules.losing_players.push(PlayerId(1));
        let view = game.derived_controllers().unwrap();
        assert_eq!(view[&source], PlayerId(2));
        assert_eq!(view[&target], PlayerId(2));
        game.reconcile_controllers().unwrap();
        let events = game.events().len();
        attach(&mut game, 2, None);
        assert_eq!(game.current(object(&game, 0)).unwrap().zone, Zone::Exile);
        assert_eq!(game.current(target).unwrap().controller, PlayerId(3));
        assert!(!game.events()[events..].iter().any(|event| matches!(
            event,
            Event::ControlChanged {
                controller: PlayerId(1),
                ..
            }
        )));
    }

    #[test]
    fn general_aura_kernel_all_frozen_losing_owners_are_absent_before_physical_removal() {
        let mut game = general_game(
            &[
                ("Confiscate", 0, 0),
                ("Confiscate", 1, 2), // later losing owner's stolen source
                ("Control Magic", 2, 2),
                ("Grizzly Bears", 3, 3),
            ],
            TraceMode::Detailed,
        );
        let target = object(&game, 3);
        let controlled_source = object(&game, 2);
        attach(&mut game, 2, Some(target));
        attach(&mut game, 1, Some(controlled_source));
        game.state.rules.losing_players = vec![PlayerId(0), PlayerId(1)];
        let view = game.derived_controllers().unwrap();
        assert!(!view.contains_key(&object(&game, 0)));
        assert!(!view.contains_key(&object(&game, 1)));
        assert_eq!(view[&controlled_source], PlayerId(2));
        assert_eq!(view[&target], PlayerId(2));
        // An attachment to a losing owner's target is likewise absent.
        game.state
            .rules
            .objects
            .get_mut(&controlled_source.card)
            .unwrap()
            .attached_to = Some(object(&game, 1));
        assert_eq!(game.derived_controllers().unwrap()[&target], PlayerId(3));
    }

    #[test]
    fn general_aura_kernel_late_failure_rolls_back_whole_group_and_trace() {
        let mut game = chronology_game(TraceMode::Detailed);
        let target = object(&game, 3);
        attach(&mut game, 0, Some(target));
        let source = object(&game, 0);
        let confiscate = object(&game, 2);
        rejected_unchanged(
            &mut game,
            EventGroup::Ordered {
                children: vec![
                    EventGroup::Occurrence {
                        proposed: ProposedEvent::Attach {
                            object: confiscate,
                            target: Some(source),
                        },
                    },
                    EventGroup::Occurrence {
                        proposed: ProposedEvent::Attach {
                            object: confiscate,
                            target: Some(confiscate),
                        },
                    },
                ],
            },
        );
    }

    #[test]
    fn general_aura_kernel_dependent_synthetic_source_exile_still_rejects_atomically() {
        let mut game = general_game(
            &[
                ("Control Magic", 0, 0),
                ("Confiscate", 2, 2),
                ("Grizzly Bears", 3, 3),
                ("Forest", 3, 3),
            ],
            TraceMode::Detailed,
        );
        let source = object(&game, 0);
        let target = object(&game, 2);
        let synthetic_target = object(&game, 3);
        attach(&mut game, 0, Some(target));
        attach(&mut game, 1, Some(source));
        game.state
            .rules
            .objects
            .get_mut(&source.card)
            .unwrap()
            .default_controller = PlayerId(1);
        game.state.rules.losing_players.push(PlayerId(1));
        game.state.rules.control_effects.push(ControlEffect {
            id: 1,
            object: synthetic_target,
            controller: PlayerId(2),
            source: ControlSource::Static { object: source },
        });
        game.reconcile_controllers().unwrap();
        let before = game.fork();
        let confiscate = object(&game, 1);
        let error = game
            .commit(
                ProposedEvent::Attach {
                    object: confiscate,
                    target: None,
                },
                BatchCause::Fixture,
            )
            .unwrap_err();
        assert!(error.contains("dependent synthetic control-source exile"));
        assert_eq!(game.state, before.state);
        assert_eq!(game.trace, before.trace);
    }

    #[test]
    fn general_aura_kernel_domains_include_control_sources_but_preserve_legacy_fences() {
        for name in [
            "Control Magic",
            "Grizzly Bears",
            "Forest",
            "Bonesplitter",
            "Glorious Anthem",
            "Rancor",
        ] {
            let mut game = general_game(&[("Confiscate", 0, 0), (name, 1, 1)], TraceMode::Detailed);
            let target = object(&game, 1);
            assert!(
                game.attachment_target_type_valid(CardId(0), CardId(1))
                    .unwrap()
            );
            attach(&mut game, 0, Some(target));
            assert_eq!(game.current(target).unwrap().controller, PlayerId(0));
        }
        let mut game = general_game(
            &[
                ("Confiscate", 0, 0),
                ("Forest", 1, 1),
                ("Bonesplitter", 1, 1),
                ("Rancor", 1, 1),
                ("Fog", 1, 1),
            ],
            TraceMode::Detailed,
        );
        for (source, target) in [(2, 1), (3, 1), (0, 4), (0, 0)] {
            let source = object(&game, source);
            let target = object(&game, target);
            rejected_unchanged(
                &mut game,
                EventGroup::Occurrence {
                    proposed: ProposedEvent::Attach {
                        object: source,
                        target: Some(target),
                    },
                },
            );
        }
    }

    #[test]
    fn general_aura_kernel_synthetic_grant_composition_remains_rejected() {
        let mut game = chronology_game(TraceMode::Detailed);
        let source = object(&game, 0);
        let target = object(&game, 3);
        attach(&mut game, 0, Some(target));
        // Synthetic persistent authority on an Aura may feed its "you".
        game.state.rules.control_effects.push(ControlEffect {
            id: 1,
            object: source,
            controller: PlayerId(1),
            source: ControlSource::Persistent,
        });
        game.reconcile_controllers().unwrap();
        assert_eq!(game.current(target).unwrap().controller, PlayerId(1));
        // But an Aura grant to that same source has no shared clock and rejects.
        let confiscate = object(&game, 2);
        rejected_unchanged(
            &mut game,
            EventGroup::Occurrence {
                proposed: ProposedEvent::Attach {
                    object: confiscate,
                    target: Some(source),
                },
            },
        );
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
            ability_use: None,
            previously_used: false,
            player: PlayerId(1),
            source: game.object_before(target).unwrap(),
            counters: 3.into(),
            produced: BTreeMap::from([(Color::U, 1.into())]),
            spent: BTreeMap::from([(Color::G, 1.into())]),
            absent: BTreeSet::from([Color::U]),
            sacrificed: true,
            cost_triggers: vec![],
            branch: None,
            damage: None,
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

    #[test]
    fn typed_aura_control_kernel_domains_use_all_types_and_preserve_legacy_boundaries() {
        for (source, target) in [
            (0, 3),
            (0, 4),
            (0, 7),
            (1, 4),
            (1, 6),
            (2, 4),
            (2, 6),
            (9, 3),
            (7, 3),
            (8, 3),
        ] {
            let mut game = typed_game(TraceMode::Detailed);
            let target = object(&game, target);
            attach(&mut game, source, Some(target));
            let source = game.current(object(&game, source)).unwrap();
            assert_eq!(source.attached_to, Some(target));
            assert_eq!(source.attachment_timestamp, Some(1));
            if mtg_contracts::aura_control_source(game.definition(source.id.card).unwrap()) {
                assert_eq!(game.current(target).unwrap().controller, source.controller);
            }
            game.validate_attachment_state().unwrap();
            game.validate_control_state().unwrap();
        }
        for (source, target) in [
            (0, 5),
            (0, 6),
            (0, 1),
            (1, 3),
            (1, 7),
            (2, 5),
            (9, 4),
            (7, 4),
            (8, 4),
        ] {
            let mut game = typed_game(TraceMode::Detailed);
            let proposed = ProposedEvent::Attach {
                object: object(&game, source),
                target: Some(object(&game, target)),
            };
            rejected_unchanged(&mut game, EventGroup::Occurrence { proposed });
        }
    }

    #[test]
    fn typed_aura_control_kernel_artifact_land_competes_on_one_attachment_clock() {
        let mut expected = None;
        for mode in [TraceMode::Off, TraceMode::Compact, TraceMode::Detailed] {
            let mut game = typed_game(mode);
            let target = object(&game, 4);
            for (source, controller) in [(2, 3), (0, 1), (1, 2)] {
                attach(&mut game, source, Some(target));
                assert_eq!(
                    game.current(target).unwrap().controller,
                    PlayerId(controller)
                );
            }
            let source = object(&game, 1);
            let clock = game.state.rules.next_attachment_timestamp;
            attach(&mut game, 1, Some(target));
            assert_eq!(game.state.rules.next_attachment_timestamp, clock);
            assert_eq!(game.current(source).unwrap().attachment_timestamp, Some(3));
            let mut duplicate_timestamp = game.fork();
            duplicate_timestamp
                .state
                .rules
                .objects
                .get_mut(&CardId(0))
                .unwrap()
                .attachment_timestamp = Some(3);
            assert!(duplicate_timestamp.validate_attachment_state().is_err());
            assert!(duplicate_timestamp.validate_control_state().is_err());
            let authority = game.control_before().unwrap().unwrap();
            assert_eq!(authority.next_attachment_timestamp, clock);
            assert_eq!(authority.attachments.len(), 3);
            attach(&mut game, 1, None);
            assert_eq!(game.current(target).unwrap().controller, PlayerId(1));
            game.move_object(object(&game, 0), Zone::Graveyard { owner: PlayerId(1) })
                .unwrap();
            assert_eq!(game.current(target).unwrap().controller, PlayerId(3));
            attach(&mut game, 2, None);
            let target = game.current(target).unwrap();
            assert_eq!(target.controller, PlayerId(0));
            assert_eq!(target.default_controller, PlayerId(0));
            game.validate_control_state().unwrap();
            if let Some(state) = &expected {
                assert_eq!(&game.state, state);
            } else {
                expected = Some(game.state.clone());
            }
        }
    }

    #[test]
    fn typed_aura_control_kernel_same_controller_artifact_creature_keeps_combat_readiness() {
        let mut game = typed_game(TraceMode::Detailed);
        let target = object(&game, 3);
        attach(&mut game, 9, Some(target));
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
        attach(&mut game, 0, Some(target));
        attach(&mut game, 9, None);
        assert_eq!(game.current(target).unwrap().controller, PlayerId(1));
        assert!(game.current(target).unwrap().attack_ready);
        assert_eq!(game.state.rules.combat.len(), 1);
        assert_eq!(control_changes(&game), changes);
        attach(&mut game, 0, None);
        assert_eq!(game.current(target).unwrap().controller, PlayerId(0));
        assert!(!game.current(target).unwrap().attack_ready);
        assert!(game.state.rules.combat.is_empty());
    }

    #[test]
    fn typed_aura_control_kernel_stale_snapshot_shape_then_sba_never_follows_successor() {
        for (source, target) in [(0, 3), (0, 4), (1, 6)] {
            let mut game = typed_game(TraceMode::Detailed);
            let old = object(&game, target);
            attach(&mut game, source, Some(old));
            for forged in [
                ObjectId {
                    incarnation: old.incarnation + 1,
                    ..old
                },
                ObjectId {
                    card: CardId(9999),
                    ..old
                },
                object(&game, 5),
            ] {
                let mut invalid = game.fork();
                invalid
                    .state
                    .rules
                    .objects
                    .get_mut(&CardId(source))
                    .unwrap()
                    .attached_to = Some(forged);
                assert!(invalid.validate_attachment_state().is_err());
            }
            game.move_object(old, Zone::Hand { owner: PlayerId(0) })
                .unwrap();
            game.validate_attachment_state().unwrap();
            game.validate_control_state().unwrap();
            let hand = object(&game, target);
            game.move_object(hand, Zone::Battlefield).unwrap();
            let successor = object(&game, target);
            assert_ne!(old, successor);
            assert_eq!(game.current(successor).unwrap().controller, PlayerId(0));
            game.validate_attachment_state().unwrap();
            let mut sbas = vec![];
            game.attachment_sbas(&mut sbas).unwrap();
            assert!(sbas.contains(&ProposedEvent::Move {
                object: object(&game, source),
                destination: Zone::Graveyard {
                    owner: PlayerId(if source == 0 { 1 } else { 2 })
                }
            }));
        }
    }

    #[test]
    fn typed_aura_control_kernel_atomic_allocation_and_mixed_simultaneous_rejection() {
        let mut game = typed_game(TraceMode::Detailed);
        let target = object(&game, 4);
        let source = object(&game, 0);
        let wrong_type = object(&game, 5);
        rejected_unchanged(
            &mut game,
            EventGroup::Ordered {
                children: vec![
                    EventGroup::Occurrence {
                        proposed: ProposedEvent::Attach {
                            object: source,
                            target: Some(target),
                        },
                    },
                    EventGroup::Occurrence {
                        proposed: ProposedEvent::Attach {
                            object: source,
                            target: Some(wrong_type),
                        },
                    },
                ],
            },
        );
        let tap = object(&game, 6);
        rejected_unchanged(
            &mut game,
            EventGroup::Simultaneous {
                occurrences: vec![
                    ProposedEvent::Attach {
                        object: source,
                        target: Some(target),
                    },
                    ProposedEvent::Tap { object: tap },
                ],
            },
        );
        game.state.rules.control_effects.push(ControlEffect {
            id: 1,
            object: target,
            controller: PlayerId(0),
            source: ControlSource::Persistent,
        });
        rejected_unchanged(
            &mut game,
            EventGroup::Occurrence {
                proposed: ProposedEvent::Attach {
                    object: source,
                    target: Some(target),
                },
            },
        );
        game.state.rules.control_effects.clear();
        game.state.rules.next_attachment_timestamp = u64::MAX;
        rejected_unchanged(
            &mut game,
            EventGroup::Occurrence {
                proposed: ProposedEvent::Attach {
                    object: source,
                    target: Some(target),
                },
            },
        );
        game.move_object(source, Zone::Stack).unwrap();
        let source = object(&game, 0);
        rejected_unchanged(
            &mut game,
            EventGroup::Occurrence {
                proposed: ProposedEvent::EnterAttached {
                    object: source,
                    target,
                },
            },
        );
        game.state.rules.next_attachment_timestamp = 1;
        rejected_unchanged(
            &mut game,
            EventGroup::Occurrence {
                proposed: ProposedEvent::EnterAttached {
                    object: source,
                    target: wrong_type,
                },
            },
        );
    }

    #[test]
    fn typed_aura_control_kernel_entry_checks_targetability_but_ongoing_attachment_does_not() {
        // Synthetic resolved keyword grants isolate targeting from attachment
        // legality; this is not a complete-source casting/replay witness.
        for (source_id, target_id) in [(0, 4), (1, 6)] {
            for keyword in [KeywordAbility::Hexproof, KeywordAbility::Shroud] {
                let mut game = typed_game(TraceMode::Detailed);
                let source = object(&game, source_id);
                let target = object(&game, target_id);
                game.move_object(source, Zone::Stack).unwrap();
                let source = object(&game, source_id);
                game.state.rules.effects.push(ResolvedEffect {
                    target_handles: vec![],
                    id: EffectId(1),
                    origin: EffectOrigin {
                        mode: None,
                        stack_id: StackId(0),
                        source: target,
                        controller: PlayerId(0),
                        source_handles: vec![],
                        cursor: 0,
                        source_block: 0,
                    },
                    expires_in_cleanup_of_turn: game.turn().number,
                    kind: ResolvedEffectKind::GrantKeywordsSet {
                        targets: vec![target],
                        modifier: None,
                        keywords: vec![keyword],
                    },
                });
                assert!(!game.targetable_by(target, game.current(source).unwrap().controller));
                rejected_unchanged(
                    &mut game,
                    EventGroup::Occurrence {
                        proposed: ProposedEvent::EnterAttached {
                            object: source,
                            target,
                        },
                    },
                );
                game.state.rules.effects.clear();
                let outcome = game
                    .commit(
                        ProposedEvent::EnterAttached {
                            object: source,
                            target,
                        },
                        BatchCause::Fixture,
                    )
                    .unwrap();
                let EventOutcome::EnteredAttached {
                    movement,
                    attachment_timestamp,
                    ..
                } = outcome
                else {
                    panic!("attached entry outcome")
                };
                assert_eq!(attachment_timestamp, 1);
                assert_eq!(
                    game.current(movement.successor).unwrap().attached_to,
                    Some(target)
                );
                game.state.rules.effects.push(ResolvedEffect {
                    target_handles: vec![],
                    id: EffectId(2),
                    origin: EffectOrigin {
                        mode: None,
                        stack_id: StackId(0),
                        source: target,
                        controller: PlayerId(0),
                        source_handles: vec![],
                        cursor: 0,
                        source_block: 0,
                    },
                    expires_in_cleanup_of_turn: game.turn().number,
                    kind: ResolvedEffectKind::GrantKeywordsSet {
                        targets: vec![target],
                        modifier: None,
                        keywords: vec![keyword],
                    },
                });
                assert!(!game.targetable_by(target, PlayerId(0)));
                game.validate_attachment_state().unwrap();
                game.validate_control_state().unwrap();
                let mut sbas = vec![];
                game.attachment_sbas(&mut sbas).unwrap();
                assert!(!sbas.iter().any(|event| matches!(event, ProposedEvent::Move {object, ..} if *object == movement.successor)));
                assert_eq!(
                    game.current(target).unwrap().controller,
                    game.current(movement.successor).unwrap().controller
                );
            }
        }
    }
}
