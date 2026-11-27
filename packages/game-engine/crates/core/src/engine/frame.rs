//! One exhaustive traversal of every invocation a `Frame` refers to.
//!
//! CR 608.2h/113.7a: an ability keeps resolving with its source's last-known
//! information. A frame names its invocations by `StackId` and holds progress
//! only: the stack entry, or the detached table once the entry has left the
//! stack, is the single owner of the invocation and of those facts.
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
//!
//! The traversal matches every `Frame`, continuation, completion, owner and
//! origin variant by name, with no wildcard arm and no `..`: a new variant, or
//! a new field on a variant, fails to compile here until it is classified.
//! Structs it reaches by field (`Casting`, `Resolving`, the progress and
//! transaction structs, `ManaActivation`, `TriggerPlacement`) are not
//! destructured, so a second invocation added to one of them must be added
//! here by hand. Traversals with other purposes remain where they are:
//! target-fact refresh in `removal.rs`, retained-payload charging in
//! `spell_cascade_grants.rs` and mana-frame unwrapping in `mana_permanents.rs`.
use super::*;

mod decider;
mod model;
pub use decider::Decider;
pub use model::*;

/// One thing a frame holds: an invocation it will resume, or the prepared
/// entries of a damage transaction, which keep source appearances.
pub enum Held<Damage> {
    Invocation(StackId),
    Damage(Damage),
}

pub(super) type HeldRef<'a> = Held<&'a Vec<effects::PreparedDamage>>;
pub(super) type HeldMut<'a> = Held<&'a mut Vec<effects::PreparedDamage>>;

// The shared and exclusive traversals are one body, so they cannot diverge.
macro_rules! frame_walk {
    ($walk:ident, $walk_damage:ident, $Held:ident $(, $mutability:tt)?) => {
        fn $walk_damage<'a>(
            transaction: &'a $($mutability)? DamageTransaction,
            visit: &mut dyn FnMut($Held<'a>),
        ) {
            let DamageTransaction {

                id: _,
                kind: _,
                entries,
                applications: _,
                cause: _,
                continuation,
            } = transaction;
            visit($Held::Damage(entries));
            match & $($mutability)? *continuation {
                DamageContinuation::Combat { .. } => {}
                DamageContinuation::Mana { activation: _ } => {}
                DamageContinuation::Instruction { frame } => visit($Held::Invocation(frame.stack_id)),
            }
        }

        fn $walk<'a>(
            frame: &'a $($mutability)? Frame,
            visit: &mut dyn FnMut($Held<'a>),
        ) {
            match frame {
                Frame::Kernel { task: _ } => {}
                Frame::GraveyardGroup { tx } => match & $($mutability)? tx.finish {
                    GroupFinish::Instruction { frame, result: _ } => visit($Held::Invocation(frame.stack_id)),
                    GroupFinish::Cleanup => {}
                    GroupFinish::Payment { resume: _ } => {},
                    GroupFinish::StateBased {
                        resume: _,
                        checked: _,
                        kept: _,
                    } => {},
                },
                Frame::ManaSourceChoice {
                    player: _,
                    source: _,
                    scope: _,
                    resume: _,
                } => {},
                Frame::LegendRule {
                    resume: _,
                    groups: _,
                    kept: _,
                } => {},
                Frame::ChooseModes { cast: _ }
                | Frame::AdditionalCosts { cast: _ }
                | Frame::AnnounceX { cast: _ }
                | Frame::HybridCost { cast: _ }
                | Frame::SacrificeCost { cast: _ }
                | Frame::ChosenObjectCost { cast: _, selected: _ }
                | Frame::EntityTargets { cast: _ }
                | Frame::StackTargets { cast: _ }
                | Frame::PlayerTargets { cast: _ }
                | Frame::SpendMana { cast: _ }
                | Frame::Targets { cast: _ }
                | Frame::Mana { cast: _ }
                | Frame::Payment { cast: _ } => {},
                Frame::ForcedSacrifice {
                    frame,
                    remaining: _,
                    choices: _,
                    source_block: _,
                } => visit($Held::Invocation(frame.stack_id)),
                Frame::ResolutionPayment { frame, payment: _ } => visit($Held::Invocation(frame.stack_id)),
                Frame::ManaAbility { activation: _ } => {}
                Frame::TurnFaceUp { payment: _ } => {}
                Frame::DamageEffects { transaction } => $walk_damage(transaction, visit),
                Frame::CounterPlacement { transaction } => visit($Held::Invocation(transaction.frame.stack_id)),
                Frame::TriggerOrder { placement: _ } => {}
                Frame::TriggerTargets {
                    placement: _,
                    stack_id: _,
                } => {},
                Frame::ResolutionChoice { frame, choice: _ } => visit($Held::Invocation(frame.stack_id)),
                Frame::CommanderReturn {
                    object: _,
                    destination: _,
                    continuation,
                } => match & $($mutability)? *continuation {
                    CommanderContinuation::BoardReturn {
                        frame,
                        objects: _,
                        remaining: _,
                        choices: _,
                        source_block: _,
                    } => visit($Held::Invocation(frame.stack_id)),
                    CommanderContinuation::SpellMove {
                        frame,
                        source_block: _,
                    } | CommanderContinuation::LookHand { frame, .. } => visit($Held::Invocation(frame.stack_id)),
                    CommanderContinuation::ResolutionDraw { .. } => {}
                    CommanderContinuation::Search {
                        frame,
                        player: _,
                        found: _,
                        remaining: _,
                        choices: _,
                        source_block: _,
                    } => visit($Held::Invocation(frame.stack_id)),
                    CommanderContinuation::Draw { player: _ } => {}
                    CommanderContinuation::StateBased { resume: _ } => {},
                    CommanderContinuation::StateBasedBatch {
                        remaining: _,
                        choices: _,
                        resume: _,
                    } => {},
                },
                Frame::Stabilize {
                    resume: _,
                    cleanup: _,
                    changed: _,
                } => {},
                Frame::Resolve { frame } => visit($Held::Invocation(frame.stack_id)),
                // No invocation, damage transaction or resumed frame.
                Frame::Setup
                | Frame::Attackers
                | Frame::Blockers { remaining: _ }
                | Frame::Damage
                | Frame::DamageChoice { source: _ }
                | Frame::Discard {
                    player: _,
                    count: _,
                }
                | Frame::CleanupClear
                | Frame::Draw
                | Frame::BattlefieldEntry { entry: _ }
                | Frame::Priority
                | Frame::Transition
                | Frame::EnterStep
                | Frame::Fault { reason: _ }
                | Frame::Finished { winner: _ } => {}
            }
        }
    };
}

frame_walk!(walk, walk_damage, HeldRef);
frame_walk!(walk_mut, walk_damage_mut, HeldMut, mut);

macro_rules! frame_parent {
    ($name:ident $(, $mutable:tt)?) => {
    fn $name(&$($mutable)? self) -> Option<&$($mutable)? FrameLink> {
        match self {
            Self::ResolutionPayment {payment, ..} => Some(&$($mutable)? payment.resume),
            Self::Kernel { task } => Some(&$($mutable)? task.resume),
            Self::Stabilize { resume, .. } | Self::LegendRule { resume, .. } => {
                Some(resume)
            }
            Self::BattlefieldEntry { entry } => Some(&$($mutable)? entry.resume),
            Self::ManaSourceChoice { resume, .. } => Some(resume),
            Self::ManaAbility { activation } => Some(&$($mutable)? activation.resume),
            Self::GraveyardGroup { tx } => match &$($mutable)? tx.finish {
                GroupFinish::StateBased { resume, .. } => Some(resume),
                GroupFinish::Payment { resume } => Some(resume),
                GroupFinish::Cleanup | GroupFinish::Instruction { .. } => None,
            },
            Self::CommanderReturn {
                continuation:
                    CommanderContinuation::StateBased { resume }
                    | CommanderContinuation::StateBasedBatch { resume, .. }
                    | CommanderContinuation::ResolutionDraw { resume },
                ..
            } => Some(resume),
            Self::TriggerOrder { placement } | Self::TriggerTargets { placement, .. } => {
                Some(&$($mutable)? placement.resume)
            }
            Self::DamageEffects { transaction } => {
                match &$($mutable)? transaction.continuation {
                    DamageContinuation::Mana { activation } => {
                        Some(&$($mutable)? activation.resume)
                    }
                    DamageContinuation::Combat { .. } | DamageContinuation::Instruction { .. } => None,
                }
            }
            Self::CommanderReturn { continuation: CommanderContinuation::BoardReturn { .. }
                | CommanderContinuation::SpellMove { .. }
                | CommanderContinuation::Search { .. }
                | CommanderContinuation::LookHand { .. }
                | CommanderContinuation::Draw { .. }, .. }




            | Self::ChooseModes { .. }
            | Self::AdditionalCosts { .. }
            | Self::AnnounceX { .. }
            | Self::HybridCost { .. }
            | Self::SacrificeCost { .. }
            | Self::ChosenObjectCost { .. }
            | Self::TurnFaceUp { .. }
            | Self::ForcedSacrifice { .. }
            | Self::Setup
            | Self::CounterPlacement { .. }
            | Self::EntityTargets { .. }
            | Self::StackTargets { .. }
            | Self::PlayerTargets { .. }
            | Self::SpendMana { .. }
            | Self::ResolutionChoice { .. }
            | Self::Attackers
            | Self::Blockers { .. }
            | Self::Damage
            | Self::DamageChoice { .. }
            | Self::Discard { .. }
            | Self::CleanupClear
            | Self::Draw
            | Self::Priority
            | Self::Transition
            | Self::EnterStep
            | Self::Targets { .. }
            | Self::Mana { .. }
            | Self::Payment { .. }
            | Self::Resolve { .. }
            | Self::Fault { .. }
            | Self::Finished { .. } => None,
        }
    }

    };
}
impl Frame {
    frame_parent!(parent_ref);
    frame_parent!(parent_mut, mut);

    pub fn parent(&self) -> Option<FrameLink> {
        self.parent_ref().copied()
    }

    pub fn for_each_held<'a>(&'a self, frames: &'a [Frame], visit: &mut dyn FnMut(HeldRef<'a>)) {
        let mut frame = self;
        loop {
            walk(frame, visit);
            let Some(parent) = frame.parent() else {
                break;
            };
            frame = &frames[parent.0];
        }
    }

    pub(super) fn for_each_held_mut<'a>(
        &'a mut self,
        mut frames: &'a mut [Frame],
        visit: &mut dyn FnMut(HeldMut<'a>),
    ) {
        let mut frame = self;
        loop {
            let parent = frame.parent();
            walk_mut(frame, visit);
            let Some(parent) = parent else { break };
            let (rest, top) = frames.split_at_mut(parent.0);
            frame = &mut top[0];
            frames = rest;
        }
    }
    /// The announcement this frame is in the middle of, if it is a casting or
    /// activation stage. Every variant is named.
    pub fn announcement(&self) -> Option<&Casting> {
        match self {
            Frame::ChooseModes { cast }
            | Frame::AdditionalCosts { cast }
            | Frame::AnnounceX { cast }
            | Frame::HybridCost { cast }
            | Frame::SacrificeCost { cast }
            | Frame::ChosenObjectCost { cast, .. }
            | Frame::EntityTargets { cast }
            | Frame::StackTargets { cast }
            | Frame::PlayerTargets { cast }
            | Frame::SpendMana { cast }
            | Frame::Targets { cast }
            | Frame::Mana { cast }
            | Frame::Payment { cast } => Some(cast),
            Frame::GraveyardGroup { tx: _ }
            | Frame::ManaSourceChoice {
                player: _,
                source: _,
                scope: _,
                resume: _,
            }
            | Frame::LegendRule {
                resume: _,
                groups: _,
                kept: _,
            }
            | Frame::ForcedSacrifice {
                frame: _,
                remaining: _,
                choices: _,
                source_block: _,
            }
            | Frame::ResolutionPayment {
                frame: _,
                payment: _,
            }
            | Frame::TurnFaceUp { payment: _ }
            | Frame::Kernel { task: _ }
            | Frame::ManaAbility { activation: _ }
            | Frame::Setup
            | Frame::CounterPlacement { transaction: _ }
            | Frame::DamageEffects { transaction: _ }
            | Frame::TriggerOrder { placement: _ }
            | Frame::TriggerTargets {
                placement: _,
                stack_id: _,
            }
            | Frame::ResolutionChoice {
                frame: _,
                choice: _,
            }
            | Frame::CommanderReturn {
                object: _,
                destination: _,
                continuation: _,
            }
            | Frame::Attackers
            | Frame::Blockers { remaining: _ }
            | Frame::Damage
            | Frame::DamageChoice { source: _ }
            | Frame::Discard {
                player: _,
                count: _,
            }
            | Frame::CleanupClear
            | Frame::Stabilize {
                resume: _,
                cleanup: _,
                changed: _,
            }
            | Frame::Draw
            | Frame::BattlefieldEntry { entry: _ }
            | Frame::Priority
            | Frame::Transition
            | Frame::EnterStep
            | Frame::Resolve { frame: _ }
            | Frame::Fault { reason: _ }
            | Frame::Finished { winner: _ } => None,
        }
    }

    /// Whether this frame will resume the invocation `id`.
    pub fn holds(&self, frames: &[Frame], id: StackId) -> bool {
        let mut found = false;
        self.for_each_held(frames, &mut |held| {
            found |= matches!(held, Held::Invocation(held) if held == id);
        });
        found
    }
}

impl RulesTxn {
    /// The single owner of an invocation: its live stack entry, or the
    /// detached entry a frame is still resolving after it left the stack.
    pub(crate) fn entry(&self, id: StackId) -> Result<&StackInstance, RuleError> {
        let rules = &self.state.rules;
        rules
            .stack
            .iter()
            .rev()
            .find(|entry| entry.id == id)
            .or_else(|| rules.detached.get(&id))
            .ok_or_else(|| "missing invocation owner".into())
    }

    /// The batch cause of the instruction a frame is executing.
    pub(crate) fn instruction_cause(
        &self,
        frame: &Resolving,
        source_block: u32,
    ) -> Result<BatchCause, RuleError> {
        let entry = self.entry(frame.stack_id)?;
        Ok(BatchCause::Instruction {
            stack_id: frame.stack_id,
            player: entry.controller,
            source: entry.source,
            cursor: frame.cursor,
            source_block,
        })
    }

    /// Removes the stack entry at `index`. The entry stays owned by the
    /// detached table until the rule step commits; `release_detached` then
    /// keeps it only while the installed frame still resumes it (CR 608.2m).
    pub(crate) fn unstack(&mut self, index: usize) -> Result<StackId, RuleError> {
        let rules = self.rules_mut();
        if index >= rules.stack.len() {
            return Err("missing stack entry".into());
        }
        let entry = rules.stack.remove(index);
        let id = entry.id;
        rules.detached.insert(id, entry);
        Ok(id)
    }

    /// Drops every detached invocation the installed frame no longer resumes.
    pub(crate) fn release_detached(&mut self) {
        let rules = self.rules_mut();
        if rules.detached.is_empty() {
            return;
        }
        let frame = &rules.frame;
        rules
            .detached
            .retain(|id, _| frame.holds(&rules.frames, *id));
    }
}

impl RulesTxn {
    pub(in crate::engine) fn saved_frame(&self, link: FrameLink) -> Result<&Frame, String> {
        self.state
            .rules
            .frames
            .get(link.0)
            .ok_or_else(|| "invalid saved frame link".into())
    }
    pub(in crate::engine) fn saved_frame_mut(
        &mut self,
        link: FrameLink,
    ) -> Result<&mut Frame, String> {
        self.rules_mut()
            .frames
            .get_mut(link.0)
            .ok_or_else(|| "invalid saved frame link".into())
    }
    pub(in crate::engine) fn save_frame(&mut self, frame: Frame) -> FrameLink {
        let frames = &mut self.rules_mut().frames;
        let link = FrameLink(frames.len());
        frames.push(frame);
        link
    }

    /// Rule routines may still read the installed frame while preparing its
    /// successor. Drop abandoned branches only once that successor is installed.
    pub(in crate::engine) fn retire_returned_frames(&mut self) -> Result<(), String> {
        let base = self
            .kernel_leaf
            .as_ref()
            .map_or(0, |leaf| leaf.marker.0 + 1);
        let rules = self.rules_mut();
        if base > rules.frames.len() {
            return Err("invalid kernel leaf marker".into());
        }
        let Some(top) = rules.frame.parent() else {
            rules.frames.truncate(base);
            return Ok(());
        };
        if top.0 < base || top.0 >= rules.frames.len() {
            return Err("invalid saved frame link".into());
        }
        rules.frames.truncate(top.0 + 1);
        if rules
            .frames
            .iter()
            .enumerate()
            .skip(base)
            .all(|(depth, frame)| {
                frame.parent().map(|link| link.0) == (depth > base).then(|| depth - 1)
            })
        {
            return Ok(());
        }
        let mut path = vec![top.0];
        let mut next = top;
        while let Some(parent) = rules.frames[next.0].parent() {
            if parent.0 < base || parent.0 >= next.0 {
                return Err("invalid saved frame ancestry".into());
            }
            path.push(parent.0);
            next = parent;
        }
        for (depth, from) in path.iter().rev().copied().enumerate() {
            let depth = base + depth;
            rules.frames.swap(depth, from);
            if let Some(link) = rules.frames[depth].parent_mut() {
                link.0 = depth - 1;
            }
        }
        rules.frames.truncate(base + path.len());
        rules.frame.parent_mut().ok_or("missing saved parent")?.0 = base + path.len() - 1;
        Ok(())
    }
}

/// Unwrap owning continuations without copying or mutating them.
pub fn retained_effect_frame<'a>(frames: &'a [Frame], mut frame: &'a Frame) -> &'a Frame {
    loop {
        frame = match frame {
            Frame::ManaAbility { activation } => &frames[activation.resume.0],
            Frame::DamageEffects { transaction } => match &transaction.continuation {
                DamageContinuation::Mana { activation } => &frames[activation.resume.0],
                DamageContinuation::Combat { .. } | DamageContinuation::Instruction { .. } => {
                    return frame;
                }
            },
            Frame::ManaSourceChoice { resume, .. }
            | Frame::Stabilize { resume, .. }
            | Frame::LegendRule { resume, .. } => &frames[resume.0],
            Frame::CommanderReturn {
                continuation:
                    CommanderContinuation::StateBased { resume }
                    | CommanderContinuation::StateBasedBatch { resume, .. },
                ..
            } => &frames[resume.0],
            _ => return frame,
        };
    }
}
