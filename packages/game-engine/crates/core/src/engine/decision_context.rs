//! Permitted descriptions copied from offline compiler labels and the current frame.
//! Descriptions grant no legality, numeric bound, payment plan or rules permission.
use super::*;

impl RulesTxn {
    /// `face`: the face the trigger came from (CR 113.7a, 712.18).
    pub(crate) fn trigger_source_label(
        &self,
        source: ObjectId,
        face: CardFace,
        ability: EffectiveTriggerAbility,
    ) -> Option<&str> {
        let (source, index, is_static) = match ability {
            EffectiveTriggerAbility::Intrinsic { index } => (source, index, false),
            EffectiveTriggerAbility::Followup { creator } => {
                let block = self
                    .resolution_trigger_program(creator)
                    .ok()?
                    .instructions
                    .first()?
                    .source_block();
                return self
                    .face_definition(source.card, self.resolution_trigger_face(creator).ok()?)
                    .ok()?
                    .source_labels
                    .as_ref()?
                    .blocks
                    .get(block as usize)
                    .map(String::as_str);
            }
        };
        let labels = self
            .face_definition(source.card, face)
            .ok()?
            .source_labels
            .as_ref()?;
        let blocks = if is_static {
            &labels.statics
        } else {
            &labels.triggered
        };
        labels
            .blocks
            .get(*blocks.get(index as usize)? as usize)
            .map(String::as_str)
    }
    pub(super) fn described_source(
        &self,
        viewer: PlayerId,
        object: ObjectId,
        kind: DecisionSourceKind,
        ability: Option<u32>,
    ) -> Option<DecisionSource> {
        self.described_face_source(viewer, object, None, kind, ability)
    }

    /// `face` overrides the object's current face for an ability that was
    /// activated or triggered from another face.
    fn described_face_source(
        &self,
        viewer: PlayerId,
        object: ObjectId,
        face: Option<CardFace>,
        kind: DecisionSourceKind,
        ability: Option<u32>,
    ) -> Option<DecisionSource> {
        if self.current(object).is_ok_and(|o| o.hideaway.is_some()) {
            return None;
        }
        let definition = match face {
            Some(face) => self.face_definition(object.card, face).ok()?,
            None => self.copiable_definition(object).ok()?,
        };
        if kind == DecisionSourceKind::ManaAbility && definition.mana_abilities.is_empty() {
            // Use precisely the runtime's intrinsic CR 305.6 production query.
            // Never describe a permanent ability as intrinsic when its labels are missing.
            let colors = self.mana_abilities(object).ok()?;
            if colors.is_empty() {
                return None;
            }
            let symbols = colors
                .iter()
                .map(|color| format!("{{{color:?}}}"))
                .collect::<Vec<_>>()
                .join(" or ");
            return Some(DecisionSource {
                kind,
                oracle_id: self.visible_ability_oracle_id(object).ok()?.into(),
                object: Some(self.handle_for(viewer, object).ok()?),
                ability: None,
                text: format!("Tap: Add {symbols}. (Intrinsic basic land mana ability.)"),
            });
        }
        let labels = definition.source_labels.as_ref()?;
        let blocks = match kind {
            DecisionSourceKind::Spell => None,
            DecisionSourceKind::Activated => Some(&labels.activated),
            DecisionSourceKind::Triggered => Some(&labels.triggered),
            DecisionSourceKind::ManaAbility => Some(&labels.mana),
            DecisionSourceKind::Effect => return None,
        };
        let text = match blocks {
            None => labels.text.clone(),
            Some(_) if kind == DecisionSourceKind::ManaAbility && ability.is_none() => {
                labels.text.clone()
            }
            Some(blocks) => labels
                .blocks
                .get(*blocks.get(ability? as usize)? as usize)?
                .clone(),
        };
        Some(DecisionSource {
            kind,
            oracle_id: self.visible_ability_oracle_id(object).ok()?.into(),
            object: self.handle_for(viewer, object).ok(),
            ability,
            text,
        })
    }
    fn stack_description(&self, viewer: PlayerId, entry: &StackInstance) -> Option<DecisionSource> {
        let (kind, ability) = match entry.kind {
            StackKind::Spell => (DecisionSourceKind::Spell, None),
            StackKind::Activated { ability, .. } => (DecisionSourceKind::Activated, Some(ability)),
            StackKind::Triggered {
                ability: EffectiveTriggerAbility::Intrinsic { index },
                ..
            } => (DecisionSourceKind::Triggered, Some(index)),
            StackKind::Triggered { ability, .. } => {
                return Some(DecisionSource {
                    kind: DecisionSourceKind::Triggered,
                    oracle_id: self.visible_ability_oracle_id(entry.source).ok()?.into(),
                    object: self.handle_for(viewer, entry.source).ok(),
                    ability: None,
                    text: self
                        .trigger_source_label(entry.source, entry.source_face, ability)?
                        .into(),
                });
            }
        };
        // CR 113.7a: an ability on the stack is described from its own face.
        let face = (!entry.kind.is_spell()).then_some(entry.source_face);
        self.described_face_source(viewer, entry.source, face, kind, ability)
    }
    pub(crate) fn permitted_decision_context(
        &self,
        viewer: PlayerId,
        request: Option<&Decision<Viewer>>,
    ) -> Option<DecisionContext> {
        let decision = request?;
        let mut context = self
            .frame_context(viewer, &decision.reason)
            .unwrap_or_default();
        context.candidates = decision
            .candidates
            .iter()
            .map(|c| self.candidate_label(viewer, c))
            .collect();
        Some(context)
    }

    fn candidate_label(&self, viewer: PlayerId, candidate: &Candidate<Viewer>) -> CandidateLabel {
        let describe = |handle, kind, ability| {
            self.resolve_handle(viewer, handle)
                .ok()
                .and_then(|id| self.described_source(viewer, id, kind, ability))
        };
        let mut source = None;
        let text = match candidate {
            Candidate::Action { action } => match action {
                Action::Pass => "Pass priority".into(),
                Action::PlayLand { .. } => "Play land".into(),
                Action::Cast { card } => {
                    source = describe(card, DecisionSourceKind::Spell, None);
                    "Cast spell".into()
                }
                Action::PlayLandBackFace { object } | Action::CastBackFace { card: object } => {
                    let cast = matches!(action, Action::CastBackFace { .. });
                    let definition = self
                        .resolve_handle(viewer, object)
                        .ok()
                        .and_then(|id| self.face_definition(id.card, CardFace::Back).ok());
                    if cast {
                        source = definition.and_then(|d| {
                            Some(DecisionSource {
                                kind: DecisionSourceKind::Spell,
                                oracle_id: d.oracle_id.clone(),
                                object: Some(object.clone()),
                                ability: None,
                                text: d.source_labels.as_ref()?.text.clone(),
                            })
                        });
                    }
                    let verb = if cast { "Cast" } else { "Play" };
                    definition.map_or_else(
                        || format!("{verb} back face"),
                        |d| format!("{verb} {}", d.name),
                    )
                }
                Action::ActivateAbility { object, ability } => {
                    source = describe(object, DecisionSourceKind::Activated, Some(*ability));
                    "Activate ability".into()
                }
                Action::ActivateMana { object } => {
                    source = describe(object, DecisionSourceKind::ManaAbility, Some(0));
                    "Activate mana ability".into()
                }
            },
            Candidate::Object { object } => object.0.clone(),
            Candidate::Player { player } => format!("Player {}", player.0 + 1),
            Candidate::Trigger { trigger } => self
                .resolve_trigger_handle(viewer, trigger)
                .ok()
                .and_then(|id| self.visible_pending_trigger(viewer, id).ok())
                .and_then(|row| row.label)
                .unwrap_or_else(|| trigger.0.clone()),
            Candidate::DamageEffect { effect, damage } => self
                .resolve_damage_effect_pair(viewer, effect, damage)
                .ok()
                .and_then(|(effect, damage)| {
                    self.visible_damage_effect_candidate(
                        viewer,
                        &DamageEffectCandidate { effect, damage },
                    )
                    .ok()
                })
                .and_then(|row| row.label)
                .unwrap_or_else(|| effect.0.clone()),
            Candidate::Color { color } => format!("{color:?}"),
            Candidate::ManaAbility { ability } => match ability {
                ManaAbilityIdentity::Intrinsic { color } => format!("Add {{{color:?}}}"),
                ManaAbilityIdentity::Printed { index } => format!("Mana ability {}", index + 1),
            },
            Candidate::AdditionalCost { id, .. } => id.clone(),
            Candidate::Modes { labels, .. } => labels.join(" / "),
            Candidate::HybridPayment { payment } => match payment {
                HybridPayment::Mana(color) => format!("{{{color:?}}}"),
                HybridPayment::PayTwoLife => "Pay 2 life".into(),
            },
            Candidate::Flag { flag } => match flag {
                Flag::Accept => "Accept",
                Flag::Decline => "Decline",
                Flag::Keep => "Keep",
                Flag::Mulligan => "Mulligan",
                Flag::CommandZone => "Command zone",
                Flag::Destination => "Original destination",
            }
            .into(),
        };
        CandidateLabel { text, source }
    }

    fn frame_context(&self, viewer: PlayerId, reason: &Reason<Viewer>) -> Option<DecisionContext> {
        if let Frame::Kernel { task } = &self.state.rules.frame {
            let entry = self.entry(self.kernel_root().ok()?.stack_id).ok()?;
            let (source_block, candidate) = self.kernel_choice_context(task).ok()?;
            let labels = self.stack_definition(entry).ok()?.source_labels.as_ref();
            let instruction = if let Some(object) = candidate {
                let handle = self.handle_for(viewer, object).ok()?;
                Some(DecisionInstruction {
                    source_block,
                    text: format!("Include {} among the chosen objects?", handle.0),
                })
            } else {
                labels
                    .and_then(|labels| labels.blocks.get(source_block as usize))
                    .map(|text| DecisionInstruction {
                        source_block,
                        text: text.clone(),
                    })
            };
            return Some(DecisionContext {
                source: self.stack_description(viewer, entry),
                instruction,
                candidates: vec![],
            });
        }

        // No stack fallback for setup, combat, cleanup, rule choices or observers.
        // Otherwise a different player's pending source could leak through context.
        let entry = match (&self.state.rules.frame, reason) {
            (Frame::BattlefieldEntry { entry }, Reason::OptionalEffect) => {
                return self
                    .battlefield_entry_payment_source(viewer, entry)
                    .map(|source| DecisionContext {
                        source: Some(source),
                        ..Default::default()
                    });
            }
            (Frame::ManaSourceChoice { player, source, .. }, _) if *player == viewer => {
                return self
                    .described_source(viewer, *source, DecisionSourceKind::ManaAbility, None)
                    .map(|source| DecisionContext {
                        source: Some(source),
                        ..Default::default()
                    });
            }
            (Frame::ManaAbility { activation }, _) if activation.player == viewer => {
                return self
                    .described_source(
                        viewer,
                        activation.source,
                        DecisionSourceKind::ManaAbility,
                        Some(0),
                    )
                    .map(|source| DecisionContext {
                        source: Some(source),
                        ..Default::default()
                    });
            }
            (
                Frame::ResolutionPayment { frame, .. }
                | Frame::ResolutionChoice { frame, .. }
                | Frame::ForcedSacrifice { frame, .. },
                _,
            ) => self.entry(frame.stack_id).ok()?,
            (
                _,
                Reason::HybridCost
                | Reason::ChooseModes
                | Reason::AnnounceX
                | Reason::Targets { .. }
                | Reason::Mana { .. }
                | Reason::SpendMana { .. }
                | Reason::ChosenObjectCost { .. }
                | Reason::SacrificeCost,
            ) => self.top().ok()?,
            _ => return None,
        };
        let mut context = DecisionContext {
            source: self.stack_description(viewer, entry),
            ..Default::default()
        };
        let labels = self.stack_definition(entry).ok()?.source_labels.as_ref()?;
        let block = match &self.state.rules.frame {
            Frame::ResolutionPayment { frame, .. } | Frame::ResolutionChoice { frame, .. } => {
                // source_block is a typed instruction provenance index, not text parsing.
                self.program_for(self.entry(frame.stack_id).ok()?)
                    .ok()?
                    .instructions
                    .get(frame.cursor as usize)
                    .map(Instruction::source_block)
            }
            Frame::ForcedSacrifice { source_block, .. } => Some(*source_block),
            _ => None,
        };
        context.instruction = block.and_then(|source_block| {
            labels
                .blocks
                .get(source_block as usize)
                .map(|text| DecisionInstruction {
                    source_block,
                    text: text.clone(),
                })
        });
        Some(context)
    }
}

impl RulesTxn {
    pub(crate) fn effect_decision_context(
        &self,
        viewer: PlayerId,
        effect: &EffectKey,
    ) -> Option<DecisionContext> {
        let (object, face, source_block) = match effect {
            // SourceLabels maps indexed static abilities but has no block
            // mapping for intrinsic keywords. The candidate names its source.
            EffectKey::IntrinsicProtectionFromEverything { .. }
            | EffectKey::IntrinsicProtectionFromColor { .. } => return None,
            EffectKey::Resolved { id } => {
                let effect = self.state.rules.effects.iter().find(|e| e.id == *id)?;
                (
                    effect.origin.source,
                    effect.origin.source_face,
                    effect.origin.source_block,
                )
            }
        };
        // CR 113.7a: label the effect from the face whose program created it.
        let definition = self.face_definition(object.card, face).ok()?;
        let labels = definition.source_labels.as_ref()?;
        let text = labels.blocks.get(source_block as usize)?.clone();
        Some(DecisionContext {
            source: Some(DecisionSource {
                kind: DecisionSourceKind::Effect,
                oracle_id: self.visible_ability_oracle_id(object).ok()?.into(),
                object: self.handle_for(viewer, object).ok(),
                ability: None,
                text: text.clone(),
            }),
            instruction: Some(DecisionInstruction { source_block, text }),
            candidates: vec![],
        })
    }
}
