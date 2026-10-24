//! Finite permanent removal. Pinned CR 115, 202.3a, 406.1–3, 601.2c/h,
//! 602.2b, 603.3d/5, 608.2b/c/h, 701.6a/8a-b/13a/19c, 704.5d/m/n,
//! 903.9a and 903.10a:
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;
use mtg_contracts::{EffectRecipient, PermanentFilter, TargetAmount, TargetAttribute};

// Reviewed derivation: docs/game-engine-removal.md. Earlier limits are intact.
pub(super) const LIVE_TOKEN_CAPACITY: u32 = 512;
pub(super) const CREATURE_CARDS: usize = 4 * 34;
pub(super) const CREATURES: usize = CREATURE_CARDS + LIVE_TOKEN_CAPACITY as usize;
pub(super) const NEW_TOKENS: usize = 36 + 4 * 4;
pub(super) const DAMAGE_BOUND: usize = 2 * CREATURES;
pub(super) const STACK_BOUND: usize = 400
    + 24
    + 4
    + four_colors::MAGE_ACTIVATIONS
    + 8 * (CREATURES + NEW_TOKENS)
    + CREATURE_CARDS
    + 4
    + 8
    + 1
    + 4;
pub(super) const EVENT_GROUP_BOUND: usize = 400 + LIVE_TOKEN_CAPACITY as usize + 4;

/// Resolution-start facts survive removal, token cessation, controller departure
/// and commander return. They never derive from the successor incarnation.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct TargetFacts {
    controller: PlayerId,
    power: Option<Quantity>,
    mana_value: Quantity,
}
impl Game {
    pub(super) fn removal_enabled(&self) -> bool {
        self.prepared.config.profile == REMOVAL_PROFILE
            && self.prepared.catalog.schema_version == 18
    }
    fn matches_permanent_filter(
        &self,
        object: ObjectId,
        controller: PlayerId,
        f: &PermanentFilter,
    ) -> bool {
        self.current(object).is_ok_and(|o| {
            o.zone == Zone::Battlefield && (!f.opponent || o.controller != controller)
        }) && self.definition(object.card).is_ok_and(|d| {
            let c = &d.characteristics;
            !matches!(c.kind, CardKind::Instant | CardKind::Sorcery)
                && (f.types.is_empty() || f.types.contains(&c.kind))
                && !f.non_types.contains(&c.kind)
                && !c.colors.iter().any(|c| f.non_colors.contains(c))
        })
    }
    pub(super) fn permanent_target_legal(
        &self,
        object: ObjectId,
        controller: PlayerId,
        spec: &TargetSpec,
    ) -> bool {
        self.removal_enabled()
            && self.targetable_by(object, controller)
            && spec
                .filter
                .as_ref()
                .is_some_and(|f| self.matches_permanent_filter(object, controller, f))
    }
    pub(super) fn eligible_stack_targets_for(
        &self,
        spec: &TargetSpec,
        exclude: Option<StackId>,
    ) -> Vec<StackId> {
        self.eligible_stack_targets(spec.domain, exclude)
            .into_iter()
            .filter(|id| {
                spec.filter.as_ref().is_none_or(|f| {
                    self.removal_enabled()
                        && self
                            .state
                            .stack
                            .iter()
                            .find(|e| e.id == *id)
                            .is_some_and(|e| {
                                self.definition(e.source.card)
                                    .is_ok_and(|d| f.types.contains(&d.characteristics.kind))
                            })
                })
            })
            .collect()
    }
    pub(super) fn capture_target_facts(
        &self,
        entry: &StackInstance,
        legal: &[bool],
    ) -> Result<Vec<Option<TargetFacts>>, String> {
        let p = self.program_for(entry)?;
        if !self.removal_enabled() || !p.targets.iter().any(|t| t.filter.is_some()) {
            return Ok(vec![]);
        }
        p.targets
            .iter()
            .enumerate()
            .map(|(slot, t)| {
                if !legal[slot] {
                    return Ok(None);
                }
                let (controller, c) = if t.domain == TargetDomain::StackSpell {
                    let e = self
                        .state
                        .stack
                        .iter()
                        .find(|e| Some(&e.id) == entry.stack_targets.get(slot))
                        .ok_or("missing LKI stack target")?;
                    (
                        e.controller,
                        self.definition(e.source.card)?.characteristics.clone(),
                    )
                } else {
                    let object = *entry
                        .targets
                        .get(slot)
                        .ok_or("missing LKI permanent target")?;
                    (
                        self.current(object)?.controller,
                        self.characteristics(object)?,
                    )
                };
                let mut mana_value = c.mana_cost.generic;
                mana_value.add(&Quantity::from(
                    i64::try_from(c.mana_cost.colored.len()).map_err(|_| "mana value overflow")?,
                ));
                Ok(Some(TargetFacts {
                    controller,
                    power: c.power,
                    mana_value,
                }))
            })
            .collect()
    }
    pub(super) fn validate_target_facts(&self, frame: &Resolving) -> Result<(), String> {
        let expected = self.removal_enabled()
            && self
                .program_for(&frame.spell)?
                .targets
                .iter()
                .any(|t| t.filter.is_some());
        if (!expected && !frame.target_facts.is_empty())
            || (expected
                && (frame.target_facts.len() != frame.legal_targets.len()
                    || frame
                        .target_facts
                        .iter()
                        .zip(&frame.legal_targets)
                        .any(|(f, l)| {
                            f.is_some() != *l
                                || f.as_ref().is_some_and(|f| {
                                    f.controller.0 >= 4 || f.mana_value.is_negative()
                                })
                        })))
        {
            return Err("snapshot removal target facts invariant".into());
        }
        Ok(())
    }
    pub(super) fn removal_target_controller(
        &self,
        frame: &Resolving,
        slot: u32,
    ) -> Result<PlayerId, String> {
        Ok(self.target_facts(frame, slot)?.controller)
    }
    fn effect_recipient(&self, frame: &Resolving, r: &EffectRecipient) -> Result<PlayerId, String> {
        match r {
            EffectRecipient::Controller => Ok(frame.spell.controller),
            EffectRecipient::TargetController { target_slot } => {
                Ok(self.target_facts(frame, *target_slot)?.controller)
            }
        }
    }
    fn target_facts<'a>(&self, frame: &'a Resolving, slot: u32) -> Result<&'a TargetFacts, String> {
        frame
            .target_facts
            .get(slot as usize)
            .and_then(Option::as_ref)
            .ok_or_else(|| "missing legal target LKI".into())
    }
    fn removal_cause(frame: &Resolving, block: u32) -> BatchCause {
        BatchCause::Instruction {
            stack_id: frame.spell.id,
            player: frame.spell.controller,
            source: frame.spell.source,
            cursor: frame.cursor,
            source_block: block,
        }
    }
    pub(super) fn execute_removal_instruction(
        &mut self,
        frame: Resolving,
        instruction: Instruction,
    ) -> Result<(), String> {
        if !self.removal_enabled() {
            return Err("unsupported removal profile".into());
        }
        let (event, block) = match instruction {
            Instruction::DestroyPermanent {
                target_slot,
                source_block,
                ..
            }
            | Instruction::ExilePermanent {
                target_slot,
                source_block,
            } => {
                let target = frame.spell.targets[target_slot as usize];
                if frame.legal_targets.get(target_slot as usize) != Some(&true)
                    || !self.object_target_legal(&frame.spell, target)
                {
                    return self
                        .finish_instruction(frame, InstructionOutcome::IllegalTarget { target });
                }
                if matches!(instruction, Instruction::DestroyPermanent { .. })
                    && matches!(
                        instruction,
                        Instruction::DestroyPermanent { optional: true, .. }
                    )
                    && self.is_live(frame.spell.controller)
                {
                    self.state.frame = Frame::ResolutionChoice {
                        choice: ResolutionChoice::OptionalEffect {
                            player: frame.spell.controller,
                            target,
                            source_block,
                        },
                        frame,
                    };
                    return Ok(());
                }
                (
                    if matches!(instruction, Instruction::ExilePermanent { .. }) {
                        ProposedEvent::Move {
                            object: target,
                            destination: Zone::Exile,
                        }
                    } else {
                        ProposedEvent::DestroyPermanent { object: target }
                    },
                    source_block,
                )
            }
            Instruction::ChangeLifeFromTarget {
                recipient,
                amount,
                gain,
                source_block,
            } => {
                let player = self.effect_recipient(&frame, &recipient)?;
                let amount = match amount {
                    TargetAmount::Fixed { amount } => amount,
                    TargetAmount::Attribute {
                        target_slot,
                        attribute,
                    } => {
                        let f = self.target_facts(&frame, target_slot)?;
                        match attribute {
                            TargetAttribute::Power => f.power.clone().ok_or("missing LKI power")?,
                            TargetAttribute::ManaValue => f.mana_value.clone(),
                        }
                    }
                };
                // CR107.1b: a negative quantity is zero for an effect's result.
                if !self.is_live(player) || amount <= Quantity::zero() {
                    return self
                        .finish_instruction(frame, InstructionOutcome::Life { actual: None });
                }
                (
                    if gain {
                        ProposedEvent::GainLife { player, amount }
                    } else {
                        ProposedEvent::LoseLife { player, amount }
                    },
                    source_block,
                )
            }
            Instruction::CreateTokensFor {
                recipient,
                count,
                token,
                source_block,
            } => {
                let controller = self.effect_recipient(&frame, &recipient)?;
                if !self.is_live(controller) {
                    return self
                        .finish_instruction(frame, InstructionOutcome::Effect { actual: None });
                }
                (
                    ProposedEvent::CreateCreatureTokens {
                        controller,
                        source: frame.spell.source,
                        count,
                        token: Box::new(token),
                    },
                    source_block,
                )
            }
            Instruction::DamageTargetController {
                target_slot,
                amount,
                source_block,
            } => {
                let player = self.target_facts(&frame, target_slot)?.controller;
                if !self.is_live(player) {
                    return self
                        .finish_instruction(frame, InstructionOutcome::Damage { actual: None });
                }
                let damage = NoncombatDamageOccurrence {
                    source: self.source_facts(&frame.spell)?,
                    controller: frame.spell.controller,
                    recipient: DamageRecipient::Player { player },
                    amount,
                };
                return self.prepare_noncombat_damage(frame, damage, source_block);
            }
            _ => return Err("not removal instruction".into()),
        };
        let actual = self.commit(event, Self::removal_cause(&frame, block))?;
        self.finish_instruction(
            frame,
            InstructionOutcome::Effect {
                actual: Some(actual),
            },
        )
    }
    pub(super) fn removal_choice_step(
        &mut self,
        frame: Resolving,
        player: PlayerId,
        target: ObjectId,
        block: u32,
    ) -> Result<(), String> {
        if !self.is_live(player) {
            self.state.pending = None;
            return self
                .finish_instruction(frame, InstructionOutcome::PlayerActionSkipped { player });
        }
        let input = self
            .state
            .pending
            .take()
            .ok_or("missing optional effect answer")?;
        let Response::OptionalEffect { perform } = input.response else {
            return Err("invalid optional effect answer".into());
        };
        if input.player != player {
            return Err("wrong optional effect player".into());
        }
        let actual = if perform && self.object_target_legal(&frame.spell, target) {
            Some(self.commit(
                ProposedEvent::DestroyPermanent { object: target },
                Self::removal_cause(&frame, block),
            )?)
        } else {
            None
        };
        self.finish_instruction(frame, InstructionOutcome::Effect { actual })
    }
}
