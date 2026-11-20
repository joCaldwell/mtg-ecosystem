//! Permitted descriptions copied from offline compiler labels and the current frame.
//! Descriptions grant no legality, numeric bound, payment plan or rules permission.
use super::*;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum DecisionSourceKind {
    Spell,
    Activated,
    Triggered,
    ManaAbility,
    Effect,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DecisionSource {
    pub kind: DecisionSourceKind,
    pub oracle_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object: Option<ObjectHandle>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ability: Option<u32>,
    pub text: String,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DecisionInstruction {
    pub source_block: u32,
    pub text: String,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DecisionMode {
    pub mode: u32,
    pub text: String,
}
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DecisionContext {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<DecisionSource>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub instruction: Option<DecisionInstruction>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub modes: Vec<DecisionMode>,
}
impl Game {
    pub(super) fn trigger_source_label(
        &self,
        source: ObjectId,
        ability: EffectiveTriggerAbility,
    ) -> Option<&str> {
        let (source, index, is_static) = match ability {
            EffectiveTriggerAbility::Intrinsic { index } => (source, index, false),
            EffectiveTriggerAbility::Granted {
                grant_source,
                static_ability_index,
            } => (grant_source, static_ability_index, true),
            EffectiveTriggerAbility::TemporaryGrant {
                grant_source,
                triggered_ability_index,
                ..
            } => (grant_source, triggered_ability_index, false),
        };
        let labels = self.definition(source.card).ok()?.source_labels.as_ref()?;
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
        if self.current(object).is_ok_and(|o| o.hideaway.is_some()) {
            return None;
        }
        let definition = self.definition(object.card).ok()?;
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
                oracle_id: definition.oracle_id.clone(),
                object: Some(self.handle_for(viewer, object).ok()?),
                ability: None,
                text: if self.has_granted_land_mana(object).unwrap_or(false) {
                    format!("Tap: Add {symbols}. (Available land mana abilities.)")
                } else {
                    format!("Tap: Add {symbols}. (Intrinsic basic land mana ability.)")
                },
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
            oracle_id: definition.oracle_id.clone(),
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
            StackKind::Triggered {
                ability, ref grant, ..
            } => {
                // The grantor describes this ability; the cast spell remains its
                // rules source. Preserve the viewer's captured public handle after departure.
                let (grant_source, index) = match ability {
                    EffectiveTriggerAbility::Granted {
                        grant_source,
                        static_ability_index,
                    } => (grant_source, static_ability_index),
                    EffectiveTriggerAbility::TemporaryGrant {
                        grant_source,
                        triggered_ability_index,
                        ..
                    } => (grant_source, triggered_ability_index),
                    EffectiveTriggerAbility::Intrinsic { .. } => unreachable!(),
                };
                return Some(DecisionSource {
                    kind: DecisionSourceKind::Triggered,
                    oracle_id: self.definition(grant_source.card).ok()?.oracle_id.clone(),
                    object: grant
                        .as_ref()?
                        .source_handles
                        .get(viewer.0 as usize)
                        .cloned(),
                    ability: Some(index),
                    text: self.trigger_source_label(entry.source, ability)?.into(),
                });
            }
        };
        self.described_source(viewer, entry.source, kind, ability)
    }
    pub(super) fn permitted_decision_context(
        &self,
        viewer: PlayerId,
        request: Option<&PlayerRequest>,
    ) -> Option<DecisionContext> {
        let request = request?;
        if let PlayerRequest::CascadeLandEntry { .. } = request
            && let Frame::Cascade { progress } = &self.state.rules.frame
        {
            return self
                .effect_decision_context(viewer, &self.cascade_land_source(progress).ok()??);
        }
        if let Some((player, source, ability)) = self.land_entry_offer().ok()? {
            return (player == viewer)
                .then(|| {
                    self.effect_decision_context(viewer, &EffectKey::Static { source, ability })
                })
                .flatten();
        }
        if let Frame::DamageAllocation { effect, .. } = &self.state.rules.frame {
            return self.effect_decision_context(viewer, effect);
        }

        if let Frame::DamageEffects { transaction } = &self.state.rules.frame
            && let DamageContinuation::Mana { activation, .. } = transaction.continuation.as_ref()
            && activation.player == viewer
        {
            return self
                .described_source(
                    viewer,
                    activation.source,
                    DecisionSourceKind::ManaAbility,
                    Some(activation.ability_index()),
                )
                .map(|source| DecisionContext {
                    source: Some(source),
                    ..Default::default()
                });
        }

        // No stack fallback for setup, combat, cleanup, rule choices or observers.
        // Otherwise a different player's pending source could leak through context.
        let entry = match (&self.state.rules.frame, request) {
            (Frame::PlayHideawayLinked { progress }, PlayerRequest::PlayHideawayLinked { .. }) => {
                &progress.frame.spell
            }
            (Frame::RevealAndCast { progress }, PlayerRequest::OptionalEffect { .. }) => {
                &progress.frame.spell
            }
            (Frame::HandCast { frame, .. }, PlayerRequest::HandCast { .. }) => &frame.spell,
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
                        Some(activation.ability_index()),
                    )
                    .map(|source| DecisionContext {
                        source: Some(source),
                        ..Default::default()
                    });
            }
            (
                Frame::ResolutionPayment { frame, .. }
                | Frame::ResolutionChoice { frame, .. }
                | Frame::OptionalDamage { frame }
                | Frame::ForcedSacrifice { frame, .. },
                _,
            ) => &frame.spell,
            (
                _,
                PlayerRequest::HybridCost { .. }
                | PlayerRequest::AnnounceX { .. }
                | PlayerRequest::Modes { .. }
                | PlayerRequest::Targets { .. }
                | PlayerRequest::EntityTargets { .. }
                | PlayerRequest::PlayerTargets { .. }
                | PlayerRequest::StackTargets { .. }
                | PlayerRequest::Mana { .. }
                | PlayerRequest::SpendMana { .. }
                | PlayerRequest::SacrificeCost { .. },
            ) => self.top().ok()?,
            _ => return None,
        };
        let mut context = DecisionContext {
            source: self.stack_description(viewer, entry),
            ..Default::default()
        };
        let labels = self
            .definition(entry.source.card)
            .ok()?
            .source_labels
            .as_ref()?;
        if entry.kind.is_spell()
            && let PlayerRequest::Modes { candidates, .. } = request
        {
            context.modes = candidates
                .iter()
                .filter_map(|&mode| {
                    Some(DecisionMode {
                        mode,
                        text: labels.spell_modes.get(mode as usize)?.clone(),
                    })
                })
                .collect();
        }
        let block = match &self.state.rules.frame {
            Frame::RevealAndCast { progress } => Some(progress.source_block),
            Frame::PlayHideawayLinked { progress } => Some(progress.source_block),
            Frame::ResolutionPayment { frame, .. }
            | Frame::ResolutionChoice { frame, .. }
            | Frame::OptionalDamage { frame } => {
                // source_block is a typed instruction provenance index, not text parsing.
                self.program_for(&frame.spell)
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

impl Game {
    pub(super) fn effect_decision_context(
        &self,
        viewer: PlayerId,
        effect: &EffectKey,
    ) -> Option<DecisionContext> {
        let (object, source_block, mode) = match effect {
            EffectKey::Static { source, ability } => {
                let labels = self.definition(source.card).ok()?.source_labels.as_ref()?;
                (*source, *labels.statics.get(*ability as usize)?, None)
            }
            EffectKey::Resolved { id } => {
                let effect = self.state.rules.effects.iter().find(|e| e.id == *id)?;
                (
                    effect.origin.source,
                    effect.origin.source_block,
                    effect.origin.mode,
                )
            }
        };
        let definition = self.definition(object.card).ok()?;
        let labels = definition.source_labels.as_ref()?;
        let text = mode
            .and_then(|mode| labels.spell_modes.get(mode as usize))
            .or_else(|| labels.blocks.get(source_block as usize))?
            .clone();
        Some(DecisionContext {
            source: Some(DecisionSource {
                kind: DecisionSourceKind::Effect,
                oracle_id: definition.oracle_id.clone(),
                object: self.handle_for(viewer, object).ok(),
                ability: None,
                text: text.clone(),
            }),
            instruction: Some(DecisionInstruction { source_block, text }),
            modes: vec![],
        })
    }
}
