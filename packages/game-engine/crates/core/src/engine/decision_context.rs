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
impl RulesTxn {
    pub(crate) fn trigger_source_label(
        &self,
        source: ObjectId,
        ability: EffectiveTriggerAbility,
    ) -> Option<&str> {
        let (source, index, is_static) = match ability {
            EffectiveTriggerAbility::Intrinsic { index } => (source, index, false),
        };
        let labels = self
            .copiable_definition(source)
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
        if self.current(object).is_ok_and(|o| o.hideaway.is_some()) {
            return None;
        }
        let definition = self.copiable_definition(object).ok()?;
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
        };
        self.described_source(viewer, entry.source, kind, ability)
    }
    pub(crate) fn permitted_decision_context(
        &self,
        viewer: PlayerId,
        request: Option<&PlayerRequest>,
    ) -> Option<DecisionContext> {
        let request = request?;
        if let Frame::Kernel { task } = &self.state.rules.frame {
            let entry = self.entry(self.kernel_root().ok()?.stack_id).ok()?;
            let (source_block, candidate) = self.kernel_choice_context(task).ok()?;
            let labels = self
                .copiable_definition(entry.source)
                .ok()?
                .source_labels
                .as_ref();
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
                modes: vec![],
            });
        }

        // No stack fallback for setup, combat, cleanup, rule choices or observers.
        // Otherwise a different player's pending source could leak through context.
        let entry = match (&self.state.rules.frame, request) {
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
            .copiable_definition(entry.source)
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
        let (object, source_block) = match effect {
            // SourceLabels maps indexed static abilities but has no block
            // mapping for intrinsic keywords. The candidate names its source.
            EffectKey::IntrinsicProtectionFromEverything { .. }
            | EffectKey::IntrinsicProtectionFromColor { .. } => return None,
            EffectKey::Resolved { id } => {
                let effect = self.state.rules.effects.iter().find(|e| e.id == *id)?;
                (effect.origin.source, effect.origin.source_block)
            }
        };
        let definition = self.copiable_definition(object).ok()?;
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
            modes: vec![],
        })
    }
}
