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

/// How a frame holds an invocation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Hold {
    /// The frame is resolving this invocation, possibly beneath state-based
    /// actions, a legend-rule choice or trigger placement that will resume it.
    Resolving,
    /// The invocation waits beneath its owned child announcement, a payment
    /// or a mana ability.
    Suspended,
}

/// One thing a frame holds: an invocation it will resume, or the prepared
/// entries of a damage transaction, which keep source appearances.
pub enum Held<Damage> {
    Invocation { hold: Hold, id: StackId },
    Damage(Damage),
}

pub(super) type HeldRef<'a> = Held<&'a Vec<effects::PreparedDamage>>;
pub(super) type HeldMut<'a> = Held<&'a mut Vec<effects::PreparedDamage>>;

// The shared and exclusive traversals are one body, so they cannot diverge.
macro_rules! frame_walk {
    ($walk:ident, $walk_cast:ident, $walk_damage:ident, $Held:ident $(, $mutability:tt)?) => {
        fn $walk_cast<'a>(
            cast: &'a $($mutability)? Casting,
            visit: &mut dyn FnMut($Held<'a>),
        ) {
            match & $($mutability)? cast.completion {
                CastCompletion::Priority => {}
                CastCompletion::HandTrigger { frame } => visit($Held::Invocation {
                    hold: Hold::Suspended,
                    id: frame.stack_id,
                }),
                CastCompletion::Resolution {
                    permission: _,
                    progress,
                } => match progress {
                    CastOwner::LinkedPlay {
                        progress,
                        object: _,
                        receipt: _,
                    } => visit($Held::Invocation {
                        hold: Hold::Suspended,
                        id: progress.frame.stack_id,
                    }),
                    CastOwner::Cascade(progress) => visit($Held::Invocation {
                        hold: Hold::Suspended,
                        id: progress.frame.stack_id,
                    }),
                    CastOwner::RevealAndCast(progress) => visit($Held::Invocation {
                        hold: Hold::Suspended,
                        id: progress.frame.stack_id,
                    }),
                },
            }
        }

        fn $walk_damage<'a>(
            transaction: &'a $($mutability)? DamageTransaction,
            hold: Hold,
            visit: &mut dyn FnMut($Held<'a>),
        ) {
            let DamageTransaction {
                allocation_groups: _,
                id: _,
                kind: _,
                entries,
                applications: _,
                cause: _,
                continuation,
            } = transaction;
            visit($Held::Damage(entries));
            match & $($mutability)? *continuation {
                DamageContinuation::Combat => {}
                DamageContinuation::Instruction { frame } => visit($Held::Invocation {
                    hold,
                    id: frame.stack_id,
                }),
                DamageContinuation::Mana {
                    activation: _,
                    departed: _,
                } => {},
            }
        }

        fn $walk<'a>(
            frame: &'a $($mutability)? Frame,
            hold: Hold,
            visit: &mut dyn FnMut($Held<'a>),
        ) {
            match frame {
                Frame::GraveyardGroup { tx } => match & $($mutability)? tx.finish {
                    GroupFinish::Instruction { frame, result: _ } => visit($Held::Invocation {
                        hold,
                        id: frame.stack_id,
                    }),
                    GroupFinish::Cleanup => {}
                    GroupFinish::Payment { resume: _ } => {},
                    GroupFinish::StateBased {
                        resume: _,
                        checked: _,
                        kept: _,
                    } => {},
                },
                Frame::GraveyardReplacement { transaction } => visit($Held::Invocation {
                    hold,
                    id: transaction.frame.stack_id,
                }),
                Frame::PlayHideawayLinked { progress } => visit($Held::Invocation {
                    hold,
                    id: progress.frame.stack_id,
                }),
                Frame::LandEntry {
                    origin,
                    objects: _,
                    choices: _,
                } => match origin {
                    EntryOrigin::LinkedPlay {
                        progress,
                        object: _,
                        receipt: _,
                        back_face: _,
                    } => visit($Held::Invocation {
                        hold,
                        id: progress.frame.stack_id,
                    }),
                    EntryOrigin::Copy {
                        frame,
                        selection: _,
                    } => visit($Held::Invocation {
                        hold,
                        id: frame.stack_id,
                    }),
                    EntryOrigin::LandPlay {
                        player: _,
                        decision_id: _,
                        back_face: _,
                    } => {}
                    EntryOrigin::Search { frame } => visit($Held::Invocation {
                        hold,
                        id: frame.stack_id,
                    }),
                    EntryOrigin::Cascade { progress } => visit($Held::Invocation {
                        hold,
                        id: progress.frame.stack_id,
                    }),
                },
                Frame::ManaSourceChoice {
                    player: _,
                    source: _,
                    scope: _,
                    resume: _,
                } => {},
                Frame::RevealAndCast { progress } => visit($Held::Invocation {
                    hold,
                    id: progress.frame.stack_id,
                }),
                Frame::HandCast { frame, child: _ } => visit($Held::Invocation {
                    hold,
                    id: frame.stack_id,
                }),
                Frame::Cascade { progress } => visit($Held::Invocation {
                    hold,
                    id: progress.frame.stack_id,
                }),
                Frame::LegendRule {
                    resume: _,
                    groups: _,
                    kept: _,
                } => {},
                Frame::AdditionalCosts { cast }
                | Frame::AnnounceX { cast }
                | Frame::HybridCost { cast }
                | Frame::SacrificeCost { cast }
                | Frame::Modes { cast }
                | Frame::EntityTargets { cast }
                | Frame::StackTargets { cast }
                | Frame::PlayerTargets { cast }
                | Frame::SpendMana { cast }
                | Frame::Targets { cast }
                | Frame::Mana { cast }
                | Frame::Payment { cast } => $walk_cast(cast, visit),
                Frame::ForcedSacrifice {
                    frame,
                    remaining: _,
                    choices: _,
                    source_block: _,
                } => visit($Held::Invocation {
                    hold,
                    id: frame.stack_id,
                }),
                Frame::ResolutionPayment { frame, payment: _ } => visit($Held::Invocation {
                    hold,
                    id: frame.stack_id,
                }),
                Frame::OptionalDamage { frame } => visit($Held::Invocation {
                    hold,
                    id: frame.stack_id,
                }),
                Frame::ManaAbility { activation: _ } => {}
                Frame::DamageAllocation {
                    transaction,
                    effect: _,
                } => $walk_damage(transaction, hold, visit),
                Frame::DamageEffects { transaction } => $walk_damage(transaction, hold, visit),
                Frame::CounterPlacement { transaction } => visit($Held::Invocation {
                    hold,
                    id: transaction.frame.stack_id,
                }),
                Frame::TriggerOrder { placement: _ } => {}
                Frame::TriggerTargets {
                    placement: _,
                    stack_id: _,
                } => {},
                Frame::ResolutionChoice { frame, choice: _ } => visit($Held::Invocation {
                    hold,
                    id: frame.stack_id,
                }),
                Frame::CommanderReturn {
                    object: _,
                    destination: _,
                    continuation,
                } => match & $($mutability)? *continuation {
                    CommanderContinuation::CascadeBottom { progress } => {
                        visit($Held::Invocation {
                            hold,
                            id: progress.frame.stack_id,
                        })
                    }
                    CommanderContinuation::GraveyardShuffle {
                        frame,
                        objects: _,
                        remaining: _,
                        choices: _,
                        source_block: _,
                    }
                    | CommanderContinuation::HandReturn {
                        frame,
                        objects: _,
                        remaining: _,
                        choices: _,
                        source_block: _,
                    }
                    | CommanderContinuation::BoardReturn {
                        frame,
                        objects: _,
                        remaining: _,
                        choices: _,
                        source_block: _,
                    } => visit($Held::Invocation {
                        hold,
                        id: frame.stack_id,
                    }),
                    CommanderContinuation::LookTopMana { frame, revealed: _ } => {
                        visit($Held::Invocation {
                            hold,
                            id: frame.stack_id,
                        })
                    }
                    CommanderContinuation::LibrarySelect {
                        frame,
                        looked: _,
                        hand: _,
                        remainder: _,
                        source_block: _,
                    } => visit($Held::Invocation {
                        hold,
                        id: frame.stack_id,
                    }),
                    CommanderContinuation::SpellMove {
                        frame,
                        source_block: _,
                    } => visit($Held::Invocation {
                        hold,
                        id: frame.stack_id,
                    }),
                    CommanderContinuation::ResolutionDraw {
                        frame,
                        player: _,
                        source_block: _,
                    } => visit($Held::Invocation {
                        hold,
                        id: frame.stack_id,
                    }),
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
                Frame::Resolve { frame } => visit($Held::Invocation {
                    hold,
                    id: frame.stack_id,
                }),
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
                | Frame::Priority
                | Frame::Transition
                | Frame::EnterStep
                | Frame::Fault { reason: _ }
                | Frame::Finished { winner: _ } => {}
            }
        }
    };
}

frame_walk!(walk, walk_cast, walk_damage, HeldRef);
frame_walk!(walk_mut, walk_cast_mut, walk_damage_mut, HeldMut, mut);

macro_rules! frame_parent {
    ($name:ident $(, $mutable:tt)?) => {
    fn $name(&$($mutable)? self) -> Option<(&$($mutable)? FrameLink, Hold)> {
        match self {
            Self::Stabilize { resume, .. } | Self::LegendRule { resume, .. } => {
                Some((resume, Hold::Resolving))
            }
            Self::ManaSourceChoice { resume, .. } => Some((resume, Hold::Suspended)),
            Self::ManaAbility { activation } => Some((&$($mutable)? activation.resume, Hold::Suspended)),
            Self::GraveyardGroup { tx } => match &$($mutable)? tx.finish {
                GroupFinish::StateBased { resume, .. } => Some((resume, Hold::Resolving)),
                GroupFinish::Payment { resume } => Some((resume, Hold::Suspended)),
                _ => None,
            },
            Self::CommanderReturn {
                continuation:
                    CommanderContinuation::StateBased { resume }
                    | CommanderContinuation::StateBasedBatch { resume, .. },
                ..
            } => Some((resume, Hold::Resolving)),
            Self::TriggerOrder { placement } | Self::TriggerTargets { placement, .. } => {
                Some((&$($mutable)? placement.resume, Hold::Resolving))
            }
            Self::DamageAllocation { transaction, .. } | Self::DamageEffects { transaction } => {
                match &$($mutable)? transaction.continuation {
                    DamageContinuation::Mana { activation, .. } => {
                        Some((&$($mutable)? activation.resume, Hold::Suspended))
                    }
                    _ => None,
                }
            }
            _ => None,
        }
    }

    };
}
impl Frame {
    frame_parent!(parent_ref);
    frame_parent!(parent_mut, mut);

    pub fn parent(&self) -> Option<(FrameLink, Hold)> {
        self.parent_ref().map(|(link, hold)| (*link, hold))
    }

    pub fn for_each_held<'a>(&'a self, frames: &'a [Frame], visit: &mut dyn FnMut(HeldRef<'a>)) {
        let mut frame = self;
        let mut hold = Hold::Resolving;
        loop {
            walk(frame, hold, visit);
            let Some((parent, kind)) = frame.parent() else {
                break;
            };
            if kind == Hold::Suspended {
                hold = kind;
            }
            frame = &frames[parent.0];
        }
    }

    pub(super) fn for_each_held_mut<'a>(
        &'a mut self,
        mut frames: &'a mut [Frame],
        visit: &mut dyn FnMut(HeldMut<'a>),
    ) {
        let mut frame = self;
        let mut hold = Hold::Resolving;
        loop {
            let parent = frame.parent();
            walk_mut(frame, hold, visit);
            let Some((parent, kind)) = parent else { break };
            if kind == Hold::Suspended {
                hold = kind;
            }
            let (rest, top) = frames.split_at_mut(parent.0);
            frame = &mut top[0];
            frames = rest;
        }
    }
    pub fn resolving_invocation(&self, frames: &[Frame]) -> Option<StackId> {
        let mut resolving = None;
        self.for_each_held(frames, &mut |held| {
            if let Held::Invocation {
                hold: Hold::Resolving,
                id,
            } = held
            {
                resolving.get_or_insert(id);
            }
        });
        resolving
    }

    /// The announcement this frame is in the middle of, if it is a casting or
    /// activation stage. Every variant is named.
    pub fn announcement(&self) -> Option<&Casting> {
        match self {
            Frame::AdditionalCosts { cast }
            | Frame::AnnounceX { cast }
            | Frame::HybridCost { cast }
            | Frame::SacrificeCost { cast }
            | Frame::Modes { cast }
            | Frame::EntityTargets { cast }
            | Frame::StackTargets { cast }
            | Frame::PlayerTargets { cast }
            | Frame::SpendMana { cast }
            | Frame::Targets { cast }
            | Frame::Mana { cast }
            | Frame::Payment { cast } => Some(cast),
            Frame::GraveyardGroup { tx: _ }
            | Frame::GraveyardReplacement { transaction: _ }
            | Frame::PlayHideawayLinked { progress: _ }
            | Frame::LandEntry {
                origin: _,
                objects: _,
                choices: _,
            }
            | Frame::ManaSourceChoice {
                player: _,
                source: _,
                scope: _,
                resume: _,
            }
            | Frame::RevealAndCast { progress: _ }
            | Frame::HandCast { frame: _, child: _ }
            | Frame::Cascade { progress: _ }
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
            | Frame::OptionalDamage { frame: _ }
            | Frame::ManaAbility { activation: _ }
            | Frame::Setup
            | Frame::DamageAllocation {
                transaction: _,
                effect: _,
            }
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
            found |= matches!(held, Held::Invocation { hold: _, id: held } if held == id);
        });
        found
    }
}

impl Game {
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

impl Game {
    pub(in crate::engine) fn saved_frame(&self, link: FrameLink) -> &Frame {
        &self.state.rules.frames[link.0]
    }
    pub(in crate::engine) fn save_frame(&mut self, frame: Frame) -> FrameLink {
        let frames = &mut self.rules_mut().frames;
        let link = FrameLink(frames.len());
        frames.push(frame);
        link
    }

    /// Rule routines may still read the installed frame while preparing its
    /// successor. Drop abandoned branches only once that successor is installed.
    pub(in crate::engine) fn retire_returned_frames(&mut self) {
        let rules = self.rules_mut();
        let Some((top, _)) = rules.frame.parent() else {
            rules.frames.clear();
            return;
        };
        rules.frames.truncate(top.0 + 1);
        if rules
            .frames
            .iter()
            .enumerate()
            .all(|(depth, frame)| frame.parent().map(|(link, _)| link.0) == depth.checked_sub(1))
        {
            return;
        }
        let mut path = vec![top.0];
        let mut next = top;
        while let Some((parent, _)) = rules.frames[next.0].parent() {
            path.push(parent.0);
            next = parent;
        }
        for (depth, from) in path.iter().rev().copied().enumerate() {
            rules.frames.swap(depth, from);
            if let Some((link, _)) = rules.frames[depth].parent_mut() {
                link.0 = depth - 1;
            }
        }
        rules.frames.truncate(path.len());
        rules.frame.parent_mut().expect("saved parent").0.0 = path.len() - 1;
    }
}
