//! Private pre-event phasing selection and return timing, CR 502.1, 702.26a,
//! 702.26d, 702.26g–i, 702.26n. Phasing uses exact battlefield incarnations.
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;

/// Rule-owned projection of phased-out incarnations. Each indirect member is
/// tied to its parent, even when the instruction also selected it directly.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PhasingSchedule {
    members: BTreeMap<ObjectId, PhaseReturn>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub enum PhaseReturn {
    Direct { controller_at_phase_out: PlayerId },
    Indirect { attached_to: ObjectId },
}

impl PhasingSchedule {
    pub fn return_condition(&self, object: ObjectId) -> Option<PhaseReturn> {
        self.members.get(&object).copied()
    }

    /// Construct the simultaneous status transition from one unmodified game
    /// state. Objects already phased out cannot be selected a second time.
    /// The caller owns the eventual event and the installation of this projection.
    pub fn phase_out_controlled(
        &mut self,
        game: &Game,
        controller: PlayerId,
    ) -> Result<Vec<ObjectId>, String> {
        if !game.is_live(controller) {
            return Err("phasing requires a live invocation controller".into());
        }
        let mut direct = BTreeSet::new();
        let mut attachments = BTreeMap::<ObjectId, Vec<ObjectId>>::new();
        for o in game.state.rules.objects.values() {
            if o.zone != Zone::Battlefield || self.members.contains_key(&o.id) {
                continue;
            }
            if o.controller == controller {
                direct.insert(o.id);
            }
            if let Some(parent) = o.attached_to {
                attachments.entry(parent).or_default().push(o.id);
            }
        }
        let mut selected = direct.clone();
        let mut frontier: Vec<_> = direct.into_iter().collect();
        while let Some(id) = frontier.pop() {
            if let Some(children) = attachments.get(&id) {
                for &child in children {
                    if selected.insert(child) {
                        frontier.push(child);
                    }
                }
            }
        }
        let mut changes = BTreeMap::new();
        for &id in &selected {
            let o = game.current(id)?;
            let status = match o.attached_to {
                Some(parent) if selected.contains(&parent) => PhaseReturn::Indirect {
                    attached_to: parent,
                },
                _ => PhaseReturn::Direct {
                    controller_at_phase_out: o.controller,
                },
            };
            changes.insert(id, status);
        }
        for &id in &selected {
            let mut path = BTreeSet::new();
            let mut current = id;
            while let Some(PhaseReturn::Indirect { attached_to }) = changes.get(&current) {
                if !path.insert(current) {
                    return Err("cyclic phasing attachment selection".into());
                }
                current = *attached_to;
            }
        }
        self.members.extend(changes);
        Ok(selected.into_iter().collect())
    }

    /// CR 702.26n: the cyclic interval of seats passed on the transition
    /// to this untap includes any departed seat whose turn would have begun.
    /// Extra turns and skipped untaps are outside the existing scheduler.
    pub fn phase_in_at_untap(
        &mut self,
        game: &Game,
        previous_turn_owner: PlayerId,
    ) -> Result<Vec<ObjectId>, String> {
        if previous_turn_owner.0 >= 4 {
            return Err("invalid previous phasing seat".into());
        }
        let active = game
            .next_live(previous_turn_owner)
            .ok_or("no live untap owner")?;
        let mut due = BTreeSet::new();
        for distance in 1..=4 {
            let seat = PlayerId((previous_turn_owner.0 + distance) % 4);
            due.insert(seat);
            if seat == active {
                break;
            }
        }
        self.members
            .retain(|id, _| game.zone(*id) == Some(Zone::Battlefield));
        let mut attachments = BTreeMap::<ObjectId, Vec<ObjectId>>::new();
        let mut ready = BTreeSet::new();
        for (&id, &status) in &self.members {
            match status {
                PhaseReturn::Direct {
                    controller_at_phase_out,
                } if due.contains(&controller_at_phase_out) => {
                    ready.insert(id);
                }
                PhaseReturn::Indirect { attached_to } => {
                    attachments.entry(attached_to).or_default().push(id)
                }
                _ => {}
            }
        }
        let mut frontier: Vec<_> = ready.iter().copied().collect();
        while let Some(parent) = frontier.pop() {
            if let Some(children) = attachments.get(&parent) {
                for &id in children {
                    if ready.insert(id) {
                        frontier.push(id);
                    }
                }
            }
        }
        for id in &ready {
            self.members.remove(id);
        }
        Ok(ready.into_iter().collect())
    }
}

impl Game {
    pub(super) fn installed_phasing(&self) -> PhasingSchedule {
        PhasingSchedule {
            members: self
                .state
                .rules
                .objects
                .values()
                .filter_map(|o| o.phased_out.map(|status| (o.id, status)))
                .collect(),
        }
    }

    pub(super) fn validate_phasing_state(&self) -> Result<(), String> {
        for object in self.state.rules.objects.values() {
            let Some(status) = object.phased_out else {
                continue;
            };
            if object.zone != Zone::Battlefield
                || self.state.rules.blocking_creatures.contains(&object.id)
                || self
                    .state
                    .rules
                    .combat
                    .iter()
                    .any(|a| a.attacker == object.id || a.blockers.contains(&object.id))
            {
                return Err("invalid phased-out battlefield or combat membership".into());
            }
            match status {
                PhaseReturn::Direct {
                    controller_at_phase_out,
                } if controller_at_phase_out.0 >= 4 => {
                    return Err("invalid phasing return seat".into());
                }
                PhaseReturn::Indirect { attached_to } => {
                    if object.attached_to != Some(attached_to) {
                        return Err("invalid indirect phasing attachment".into());
                    }
                    let mut seen = BTreeSet::from([object.id]);
                    let mut parent = attached_to;
                    while let Ok(host) = self.current(parent) {
                        if !seen.insert(parent) {
                            return Err("cyclic indirect phasing return".into());
                        }
                        match host.phased_out {
                            Some(PhaseReturn::Indirect { attached_to }) => parent = attached_to,
                            Some(PhaseReturn::Direct { .. }) => break,
                            None => {
                                return Err("indirectly phased attachment has present host".into());
                            }
                        }
                    }
                }
                _ => {}
            }
        }
        Ok(())
    }

    // No source may emit this transition until the remaining consumers and
    // replay boundary accept its status. One rules operation updates the set.
    pub(super) fn phase_out_controller_permanents(
        &mut self,
        controller: PlayerId,
    ) -> Result<Vec<ObjectId>, String> {
        let mut schedule = self.installed_phasing();
        let ids = schedule.phase_out_controlled(self, controller)?;
        for &id in &ids {
            self.retain_source_lki(id)?;
        }
        for &id in &ids {
            self.state
                .rules
                .objects
                .get_mut(&id.card)
                .unwrap()
                .phased_out = schedule.return_condition(id);
            self.remove_from_combat(id);
        }
        Ok(ids)
    }

    pub(super) fn phase_in_for_next_untap(
        &mut self,
        previous_owner: PlayerId,
    ) -> Result<Vec<ObjectId>, String> {
        let mut schedule = self.installed_phasing();
        if schedule.members.is_empty() {
            return Ok(vec![]);
        }
        let ids = schedule.phase_in_at_untap(self, previous_owner)?;
        for id in &ids {
            let object = self.current(*id)?;
            let lost_host = matches!(object.phased_out, Some(PhaseReturn::Direct { .. }))
                && object
                    .attached_to
                    .is_some_and(|host| self.zone(host) != Some(Zone::Battlefield));
            let returning = self.state.rules.objects.get_mut(&id.card).unwrap();
            returning.phased_out = None;
            if lost_host {
                returning.attached_to = None;
                returning.attachment_timestamp = None;
            }
        }
        self.reconcile_controllers()?;
        Ok(ids)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::equipped_indestructible_tests::{equip, object, prepared};

    fn synthetic_game(alternate_spell: Option<&str>) -> Game {
        let mut catalog = Catalog::read(include_str!(
            "../../../../tests/fixtures/synthetic-controlled-phasing-ir.json"
        ))
        .unwrap();
        if let Some(source) = alternate_spell {
            let life = Catalog::read(source).unwrap().definitions.remove(0);
            catalog.definitions[0] = life;
        }
        let spell = catalog.definitions[0].oracle_id.clone();
        let land = catalog.definitions[1].oracle_id.clone();
        let attacker = if catalog.definitions[0]
            .capabilities
            .contains(&Capability::ControllerProtectionUntilNextTurnV1)
        {
            let base = Catalog::read(include_str!(
                "../../../../tests/fixtures/expected-wipes-ir.json"
            ))
            .unwrap();
            let creature = base
                .definitions
                .into_iter()
                .find(|d| d.name == "Grizzly Bears")
                .unwrap();
            let id = creature.oracle_id.clone();
            catalog.definitions.push(creature);
            Some(id)
        } else {
            None
        };
        catalog
            .definitions
            .sort_by(|a, b| a.oracle_id.cmp(&b.oracle_id));
        catalog.validate().unwrap();
        let mut config = ScenarioConfig::turns_permanents();
        config.profile = "synthetic-controlled-phasing".into();
        config.initial.turn_step = TurnStep::PrecombatMain;
        config.commanders.clear();
        config.cards.clear();
        for (seat, zone, definition) in [
            (0, Zone::Hand { owner: PlayerId(0) }, 0),
            (0, Zone::Battlefield, 1),
            (1, Zone::Battlefield, 1),
        ] {
            config.cards.push(CardSetup {
                id: CardId(config.cards.len() as u32),
                oracle_id: if definition == 0 {
                    spell.clone()
                } else {
                    land.clone()
                },
                owner: PlayerId(seat),
                controller: PlayerId(seat),
                zone,
            });
        }
        if let Some(oracle_id) = attacker {
            config.cards.push(CardSetup {
                id: CardId(3),
                oracle_id,
                owner: PlayerId(1),
                controller: PlayerId(1),
                zone: Zone::Battlefield,
            });
        }
        for seat in (0..4).map(PlayerId) {
            for _ in 0..4 {
                config.cards.push(CardSetup {
                    id: CardId(config.cards.len() as u32),
                    oracle_id: land.clone(),
                    owner: seat,
                    controller: seat,
                    zone: Zone::Library { owner: seat },
                });
            }
        }
        let prepared = Prepared {
            catalog_digest: digest(&catalog).unwrap(),
            profile_digest: digest(&config).unwrap(),
            catalog: Arc::new(catalog),
            config,
            match_config: None,
        };
        Game::with_trace(prepared, TraceMode::Detailed)
    }

    #[test]
    fn paid_life_freeze_retains_damage_blocks_positive_payment_and_expires_on_next_turn() {
        use crate::engine::equipped_indestructible_tests::{answer, cast, recover, request};
        let mut g = synthetic_game(Some(include_str!(
            "../../../../tests/fixtures/synthetic-controller-life-freeze-ir.json"
        )));
        request(&mut g, 1);
        cast(&mut g, 1, 0, vec![]);
        let player = PlayerId(0);
        assert!(
            g.visible_player(player)
                .unwrap()
                .life_frozen_until_next_turn
        );
        assert!(
            !g.visible_player(PlayerId(1))
                .unwrap()
                .life_frozen_until_next_turn
        );
        recover(&g);
        let mut probe = g.fork();
        let amount = Quantity::from(3);
        let source = probe.object_before(object(&probe, 2)).unwrap();
        let outcome = probe
            .apply_noncombat_damage(&NoncombatDamageOccurrence {
                source,
                controller: PlayerId(1),
                recipient: DamageRecipient::Player { player },
                amount: amount.clone(),
            })
            .unwrap();
        assert!(
            matches!(outcome,EventOutcome::NoncombatDamageDealt{result,..} if result.previous==result.current)
        );
        for event in [
            ProposedEvent::GainLife {
                player,
                amount: amount.clone(),
            },
            ProposedEvent::LoseLife {
                player,
                amount: amount.clone(),
            },
        ] {
            assert_eq!(
                probe.commit(event, BatchCause::Fixture).unwrap(),
                EventOutcome::LifeChangeProhibited { player }
            );
        }
        assert!(!probe.state.rules.players[0].can_pay_life(&amount));
        assert!(probe.state.rules.players[0].can_pay_life(&Quantity::zero()));
        for _ in 0..600 {
            let r = request(&mut g, 1);
            if !g
                .visible_player(player)
                .unwrap()
                .life_frozen_until_next_turn
            {
                break;
            }
            let response = match r {
                Request::Priority { .. } => Response::Pass,
                Request::Attackers { .. } => Response::Attackers { attacks: vec![] },
                other => panic!("unexpected turn request {other:?}"),
            };
            answer(&mut g, 1, response);
        }
        assert!(
            !g.visible_player(player)
                .unwrap()
                .life_frozen_until_next_turn
        );
        assert_eq!(g.state.rules.turn_owner, player);
        assert!(g.state.rules.players[0].can_pay_life(&amount));
        recover(&g);
    }

    #[test]
    fn paid_controller_protection_excludes_targets_and_recovers_until_next_turn() {
        use crate::engine::equipped_indestructible_tests::{answer, cast, recover, request};
        let mut g = synthetic_game(Some(include_str!(
            "../../../../tests/fixtures/synthetic-controller-protection-ir.json"
        )));
        let player = PlayerId(0);
        request(&mut g, 1);
        cast(&mut g, 1, 0, vec![]);
        assert!(g.player_protected(player));
        assert!(
            !g.eligible_players(TargetDomain::AnyPlayer, PlayerId(1))
                .contains(&player)
        );
        assert!(g.targetable_player(PlayerId(1)));
        recover(&g);
        let mut attacked = false;
        for _ in 0..600 {
            let r = request(&mut g, 1);
            if !g.player_protected(player) {
                break;
            }
            let response = match r {
                Request::Priority { .. } => Response::Pass,
                Request::Attackers {
                    player: seat,
                    candidates,
                    defenders,
                    ..
                } if seat == PlayerId(1) => {
                    let attacker = object(&g, 3);
                    assert!(candidates.contains(&attacker) && defenders.contains(&player));
                    attacked = true;
                    Response::Attackers {
                        attacks: vec![(attacker, player)],
                    }
                }
                Request::Blockers { .. } => Response::Blockers { blocks: vec![] },
                Request::DamageEffect { candidates, .. } => Response::DamageEffect {
                    damage: candidates[0].damage,
                    effect: candidates[0].effect.clone(),
                },
                Request::Attackers { .. } => Response::Attackers { attacks: vec![] },
                other => panic!("unexpected turn request {other:?}"),
            };
            answer(&mut g, 1, response);
        }
        assert!(attacked);
        let outcomes = g
            .trace
            .records
            .iter()
            .filter_map(|r| {
                if let EvidenceRecord::Batch { batch } = &r.record {
                    Some(&batch.occurrences)
                } else {
                    None
                }
            })
            .flatten()
            .map(|o| &o.actual)
            .collect::<Vec<_>>();
        assert!(outcomes.iter().any(|o| matches!(o,EventOutcome::DamageEffectApplied {application} if application.prevented == Quantity::from(2) && application.after == Quantity::zero())));
        assert!(outcomes.iter().any(|o| matches!(o, EventOutcome::DamageCommitted {actual,commander_damage,..} if actual.is_empty() && commander_damage.is_empty())));
        assert!(g.targetable_player(player));
        assert_eq!(g.state.rules.turn_owner, player);
        recover(&g);
    }

    #[test]
    fn paid_controller_phasing_replays_status_and_normal_untap_with_stable_handles() {
        use crate::engine::equipped_indestructible_tests::{answer, cast, recover, request};
        let mut g = synthetic_game(None);
        request(&mut g, 1);
        let land = object(&g, 1);
        let opposing = object(&g, 2);
        let handle = g.handle_for(PlayerId(1), land).unwrap();
        cast(&mut g, 1, 0, vec![]);
        assert!(g.current(land).unwrap().tapped);
        assert_eq!(
            g.current(land).unwrap().phased_out,
            Some(PhaseReturn::Direct {
                controller_at_phase_out: PlayerId(0)
            })
        );
        assert!(g.current(opposing).unwrap().present_on_battlefield());
        assert!(g.mana_abilities(land).unwrap().is_empty());
        let filter = SacrificeFilter {
            types: vec![CardKind::Land],
            subtypes: vec![],
        };
        assert!(g.sacrifice_candidates(PlayerId(0), &filter).is_empty());
        assert_eq!(g.sacrifice_candidates(PlayerId(1), &filter), [opposing]);
        assert!(
            g.commit(
                ProposedEvent::Sacrifice {
                    player: PlayerId(0),
                    object: land
                },
                BatchCause::Fixture
            )
            .is_err()
        );

        assert_eq!(g.handle_for(PlayerId(1), land).unwrap(), handle);
        recover(&g);
        for _ in 0..600 {
            if g.current(land).unwrap().present_on_battlefield() {
                break;
            }
            let r = request(&mut g, 1);
            let response = match r {
                Request::Priority { .. } => Response::Pass,
                Request::Attackers { .. } => Response::Attackers { attacks: vec![] },
                other => panic!("unexpected turn request: {other:?}"),
            };
            answer(&mut g, 1, response);
        }
        request(&mut g, 1);
        assert_eq!(g.state.rules.turn_owner, PlayerId(0));
        assert!(g.current(land).unwrap().present_on_battlefield());
        assert!(!g.current(land).unwrap().tapped);
        assert_eq!(g.sacrifice_candidates(PlayerId(0), &filter), [land]);
        assert_eq!(g.handle_for(PlayerId(1), land).unwrap(), handle);
        recover(&g);
    }

    #[test]
    fn controlled_phasing_freezes_attachment_identity_and_return_seat_on_live_table() {
        let mut p = prepared(
            &[
                ("Darksteel Plate", false),
                ("Grizzly Bears", false),
                ("Forest", false),
                ("Forest", false),
            ],
            false,
        );
        p.config.cards[1].owner = PlayerId(2);
        p.profile_digest = digest(&p.config).unwrap();
        let mut game = Game::with_trace(p, TraceMode::Detailed);
        equip(&mut game, 1, 0, 1);
        let plate = object(&game, 0);
        let bear = object(&game, 1);
        let mut schedule = PhasingSchedule::default();
        let before = game.state.clone();
        let ids = schedule.phase_out_controlled(&game, PlayerId(0)).unwrap();
        assert_eq!(ids, (0..4).map(|i| object(&game, i)).collect::<Vec<_>>());
        assert_eq!(
            schedule.return_condition(plate),
            Some(PhaseReturn::Indirect { attached_to: bear })
        );
        assert_eq!(
            schedule.return_condition(bear),
            Some(PhaseReturn::Direct {
                controller_at_phase_out: PlayerId(0)
            })
        );
        assert!(
            schedule
                .phase_out_controlled(&game, PlayerId(0))
                .unwrap()
                .is_empty()
        );
        let successor = ObjectId {
            card: bear.card,
            incarnation: bear.incarnation + 1,
        };
        assert_eq!(schedule.return_condition(successor), None);
        assert!(
            schedule
                .phase_in_at_untap(&game, PlayerId(0))
                .unwrap()
                .is_empty()
        );
        assert_eq!(schedule.phase_in_at_untap(&game, PlayerId(3)).unwrap(), ids);
        assert_eq!(game.state, before);
        assert!(
            game.commit_batch(
                BatchCause::Fixture,
                EventGroup::Occurrence {
                    proposed: ProposedEvent::PhaseOutControlled {
                        controller: PlayerId(0)
                    }
                }
            )
            .is_err()
        );
        game.phase_out_controller_permanents(PlayerId(0)).unwrap();
        assert!(!game.controlled_creatures(PlayerId(0)).contains(&bear));
        for viewer in (0..4).map(PlayerId) {
            assert!(
                game.visible_object(viewer, game.current(bear).unwrap())
                    .unwrap()
                    .unwrap()
                    .phased_out
            );
        }
        assert_eq!(game.zone(bear), Some(Zone::Battlefield));
        assert!(!game.current(bear).unwrap().present_on_battlefield());
        assert_eq!(game.current(plate).unwrap().attached_to, Some(bear));
        game.phase_in_for_next_untap(PlayerId(3)).unwrap();
        assert!(game.current(bear).unwrap().present_on_battlefield());
        assert_eq!(game.current(plate).unwrap().attached_to, Some(bear));
    }
}
