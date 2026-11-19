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
        let block = match &frame.spell.kind {
            StackKind::Spell => 0,
            StackKind::Activated {
                ability: 0,
                source_handles,
            } => {
                let definition = self.definition(frame.spell.source.card)?;
                if !definition
                    .activated
                    .first()
                    .is_some_and(artifact_loss_activation)
                    || self.current(frame.spell.source).is_ok()
                {
                    return Err("targeted loss requires its paid sacrificed artifact".into());
                }
                self.validate_activated_entry(&frame.spell, 0, source_handles)?;
                1
            }
            _ => return Err("unsupported targeted loss invocation".into()),
        };
        if frame.cursor != 0
            || frame.legal_targets != [true]
            || !matches!(program.instructions.as_slice(), [Instruction::LoseGameTargetPlayer {target_slot:0,source_block}] if *source_block == block)
        {
            return Err("targeted loss instruction does not authorize departure".into());
        }
        Ok((Self::wipe_cause(frame, block), *player))
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
    fn paid_artifact_loss_uses_sacrificed_source_and_selected_player() {
        let mut p = game().prepared.as_ref().clone();
        let catalog = Arc::make_mut(&mut p.catalog);
        let d = catalog
            .definitions
            .iter_mut()
            .find(|d| d.oracle_id == p.config.cards[1].oracle_id)
            .unwrap();
        let subject = "this artifact";
        let text = format!(
            "This artifact enters tapped.\n{{U}}, {{T}}, Sacrifice {subject}: Target player loses the game."
        );
        let bound = ArtifactLossSource::parse(&text).unwrap();
        d.name = "Synthetic Silent Chime".into();
        d.characteristics.kind = CardKind::Artifact;
        d.characteristics.colors.clear();
        d.characteristics.color_identity = vec![Color::U];
        d.characteristics.mana_cost = ManaCost {
            generic: 1.into(),
            colored: vec![],
        };
        d.source_labels = Some(SourceLabels {
            text: text.clone(),
            blocks: text.lines().map(str::to_owned).collect(),
            statics: vec![0],
            activated: vec![1],
            ..SourceLabels::default()
        });
        d.provenance.printing_id = "synthetic-silent-chime".into();
        d.provenance.source_json = canonical(&serde_json::json!({"id":d.provenance.printing_id,
            "oracle_id":d.oracle_id,"name":d.name,"layout":"normal","type_line":"Artifact","colors":[],
            "color_identity":["U"],"mana_cost":"{1}","oracle_text":text})).unwrap();
        d.provenance.source_sha256 = sha256(d.provenance.source_json.as_bytes());
        d.provenance.abilities_json = canonical(&bound.abilities).unwrap();
        d.provenance.ast_sha256 = sha256(format!("[{}]", d.provenance.abilities_json).as_bytes());
        let spell = d.spell.as_mut().unwrap();
        let mut program = spell.clone();
        program.timing = SpellTiming::Instant;
        program.targets[0].selection_id = bound.selection;
        program.instructions = vec![Instruction::LoseGameTargetPlayer {
            target_slot: 0,
            source_block: 1,
        }];
        spell.resolution = SpellResolution::Permanent;
        spell.targets.clear();
        spell.instructions.clear();
        d.static_abilities = vec![StaticAbility {
            functioning_zone: FunctioningZone::Battlefield,
            effect: StaticEffect::EntersTapped,
        }];
        d.activated = vec![ActivatedAbility {
            functioning_zone: FunctioningZone::Battlefield,
            cost: ActivationCost::Composite {
                costs: PermanentActivationCosts {
                    tap: true,
                    mana: Some(bound.mana),
                    sacrifice: true,
                    remove_source_plus_one_counters: None,
                },
            },
            program,
        }];
        d.capabilities = d.requirements();
        catalog.validate().unwrap();
        p.catalog_digest = digest(catalog).unwrap();
        let mut cast = Game::with_trace(p.clone(), TraceMode::Detailed);
        let land = cast.object_id(CardId(0)).unwrap();
        answer(&mut cast, 1, Response::ActivateMana { object: land });
        answer(&mut cast, 1, Response::Cast { card: CardId(1) });
        answer(&mut cast, 1, Response::FinishMana { undo_mana: false });
        cast.complete_test_mana();
        for _ in 0..4 {
            answer(&mut cast, 1, Response::Pass);
        }
        assert!(matches!(
            cast.advance(Budget::unlimited()),
            Advance::AwaitingDecision(_)
        ));
        let entered = cast.object_id(CardId(1)).unwrap();
        assert_eq!(cast.current(entered).unwrap().zone, Zone::Battlefield);
        assert!(cast.current(entered).unwrap().tapped);
        assert!(cast.validate_activation(PlayerId(0), entered, 0).is_err());
        recover(&cast);
        p.config.cards[1].zone = Zone::Battlefield;
        p.config.cards[1].owner = PlayerId(1);
        p.profile_digest = digest(&p.config).unwrap();
        let mut g = Game::with_trace(p, TraceMode::Detailed);
        let object = g.object_id(CardId(1)).unwrap();
        let island = g.object_id(CardId(0)).unwrap();
        answer(&mut g, 1, Response::ActivateAbility { object, ability: 0 });
        recover(&g);
        let victim = PlayerId(2);
        answer(
            &mut g,
            1,
            Response::PlayerTargets {
                players: vec![victim],
            },
        );
        answer(&mut g, 1, Response::ActivateMana { object: island });
        answer(&mut g, 1, Response::FinishMana { undo_mana: false });
        g.complete_test_mana();
        assert!(g.current(object).is_err());
        assert_eq!(
            g.state.rules.objects[&CardId(1)].zone,
            Zone::Graveyard { owner: PlayerId(1) }
        );
        let invoked = g.top().unwrap();
        assert_eq!(invoked.source, object);
        assert_eq!(invoked.paid.as_ref(), Some(&invoked.total_cost));
        assert_eq!(invoked.player_targets, [victim]);
        recover(&g);
        for _ in 0..4 {
            answer(&mut g, 1, Response::Pass);
        }
        assert!(matches!(
            g.advance(Budget::unlimited()),
            Advance::AwaitingDecision(_)
        ));
        assert!(!g.is_live(victim));
        assert!(g.events().iter().any(|e|matches!(e,Event::PlayersLost{losses} if losses==&[(victim,PlayerLossReason::TargetedEffect)])));
        recover(&g);
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
