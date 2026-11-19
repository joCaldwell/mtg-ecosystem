//! CR 104.3e, 104.5, 608.2b, 800.4a. A spell effect causes immediate
//! loss, independently of the state-based loss conditions.
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;

impl Game {
    pub(super) fn targeted_loss_authority(&self) -> Result<(BatchCause, PlayerId), String> {
        let Frame::Resolve { frame } = &self.state.rules.frame else {
            return Err("targeted loss requires a resolving spell".into());
        };
        self.validate_resolving(frame)?;
        let program = self.program_for(&frame.spell)?;
        let [player] = frame.spell.player_targets.as_slice() else {
            return Err("targeted loss requires one announced player".into());
        };
        if !frame.spell.kind.is_spell()
            || frame.cursor != 0
            || frame.legal_targets != [true]
            || !matches!(
                program.instructions.as_slice(),
                [Instruction::LoseGameTargetPlayer {
                    target_slot: 0,
                    source_block: 0
                }]
            )
        {
            return Err("targeted loss instruction does not authorize departure".into());
        }
        Ok((Self::wipe_cause(frame, 0), *player))
    }

    pub(super) fn resolve_targeted_loss(&mut self, frame: Resolving) -> Result<(), String> {
        let (cause, player) = self.targeted_loss_authority()?;
        if !matches!(&self.state.rules.frame, Frame::Resolve {frame: current} if *current == frame)
        {
            return Err("targeted loss resolving instance changed".into());
        }
        if !self.is_live(player) {
            return self.finish_spell(&frame.spell, true);
        }
        let batch = self.commit_batch(
            cause,
            EventGroup::Occurrence {
                proposed: ProposedEvent::Lose {
                    player,
                    reason: PlayerLossReason::TargetedEffect,
                },
            },
        )?;
        if self.finish_committed_player_losses(&batch)? {
            return Ok(());
        }
        // A self-targeted caster can leave with the physical spell. The usual
        // completion path handles the frozen resolving instance in that case.
        self.finish_spell(&frame.spell, true)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::equipped_indestructible_tests::{answer, recover};

    fn game() -> Game {
        let mut p = Prepared::admit(
            Catalog::read(include_str!("../../../../tests/fixtures/expected-ir.json")).unwrap(),
            ScenarioConfig::land_return(),
        )
        .unwrap();
        let catalog = Arc::make_mut(&mut p.catalog);
        let d = catalog
            .definitions
            .iter_mut()
            .find(|d| d.oracle_id == SPELL)
            .unwrap();
        d.oracle_id = "synthetic-targeted-loss".into();
        d.name = "Synthetic Targeted Loss".into();
        d.source_labels = None;
        d.characteristics.kind = CardKind::Sorcery;
        d.characteristics.subtypes.clear();
        let program = d.spell.as_mut().unwrap();
        program.timing = SpellTiming::MainPhase;
        program.targets = vec![TargetSpec {
            slot: 0,
            selection_id: "synthetic-target".into(),
            domain: TargetDomain::AnyPlayer,
            filter: None,
            chooser: Chooser::Controller,
            stage: EvaluationStage::Announcement,
        }];
        program.instructions = vec![Instruction::LoseGameTargetPlayer {
            target_slot: 0,
            source_block: 0,
        }];
        d.capabilities = d.requirements();
        p.config.cards[1].oracle_id = d.oracle_id.clone();
        catalog
            .definitions
            .sort_by(|a, b| a.oracle_id.cmp(&b.oracle_id));
        catalog.validate().unwrap();
        p.catalog_digest = digest(catalog).unwrap();
        p.profile_digest = digest(&p.config).unwrap();
        Game::with_trace(p, TraceMode::Detailed)
    }

    #[test]
    fn targeted_game_loss_binds_paid_spell_departure_and_terminal_result() {
        for (target, terminal) in [
            (PlayerId(1), false),
            (PlayerId(0), false),
            (PlayerId(1), true),
            (PlayerId(0), true),
        ] {
            let mut g = game();
            if terminal {
                g.concede(PlayerId(2)).unwrap();
                g.concede(PlayerId(3)).unwrap();
            }
            let source = g.object_id(CardId(0)).unwrap();
            answer(&mut g, 1, Response::ActivateMana { object: source });
            answer(&mut g, 1, Response::Cast { card: CardId(1) });
            answer(
                &mut g,
                1,
                Response::PlayerTargets {
                    players: vec![target],
                },
            );
            answer(&mut g, 1, Response::FinishMana { undo_mana: false });
            g.complete_test_mana();
            for _ in 0..g.live_players().len() {
                answer(&mut g, 1, Response::Pass);
            }
            for _ in 0..100 {
                if matches!(g.state.rules.frame, Frame::Resolve { .. }) {
                    break;
                }
                assert!(matches!(
                    g.advance(Budget::new(1)),
                    Advance::Suspended { .. }
                ));
            }
            if !terminal && target == PlayerId(1) {
                let mut departed = g.fork();
                departed.concede(target).unwrap();
                assert!(matches!(
                    departed.advance(Budget::unlimited()),
                    Advance::AwaitingDecision(_)
                ));
                assert!(!departed.events().iter().any(|event| matches!(event,
                    Event::PlayersLost { losses } if losses.iter().any(|(_,reason)| *reason == PlayerLossReason::TargetedEffect))));
                recover(&departed);
            }
            let (cause, victim) = g.targeted_loss_authority().unwrap();
            assert_eq!(victim, target);
            let wrong = PlayerId(if target == PlayerId(0) { 1 } else { 0 });
            let lose = |player| ProposedEvent::Lose {
                player,
                reason: PlayerLossReason::TargetedEffect,
            };
            let before = g.state.clone();
            assert!(g.commit(lose(wrong), cause.clone()).is_err());
            assert!(g.commit(lose(target), BatchCause::StateBased).is_err());
            assert_eq!(g.state, before);
            let end = g.advance(Budget::unlimited());
            if terminal {
                assert_eq!(end, Advance::Finished { winner: wrong });
            } else {
                assert!(matches!(end, Advance::AwaitingDecision(_)));
                assert!(g.state.rules.stack.is_empty());
                if target == PlayerId(0) {
                    assert!(g.object_id(CardId(1)).is_none());
                } else {
                    assert_eq!(
                        g.state.rules.objects[&CardId(1)].zone,
                        Zone::Graveyard { owner: PlayerId(0) }
                    );
                    // Seat one owned this permanent, although seat two controlled it.
                    assert!(g.object_id(CardId(2)).is_none());
                }
            }
            assert_eq!(
                g.state.rules.players[target.0 as usize]
                    .last_known
                    .as_ref()
                    .unwrap()
                    .life,
                Quantity::from(40)
            );
            assert!(
                g.events()
                    .iter()
                    .any(|event| matches!(event,Event::PlayersLost{losses}
                if losses == &[(target,PlayerLossReason::TargetedEffect)]))
            );
            recover(&g);
        }
    }
}
