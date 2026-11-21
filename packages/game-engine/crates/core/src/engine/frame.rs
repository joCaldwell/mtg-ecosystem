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
pub use decider::Decider;

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
            match & $($mutability)? **continuation {
                DamageContinuation::Combat => {}
                DamageContinuation::Instruction { frame } => visit($Held::Invocation {
                    hold,
                    id: frame.stack_id,
                }),
                DamageContinuation::Mana {
                    activation,
                    departed: _,
                } => $walk(& $($mutability)? activation.resume, Hold::Suspended, visit),
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
                    GroupFinish::Payment { resume } => $walk(resume, Hold::Suspended, visit),
                    GroupFinish::StateBased {
                        resume,
                        checked: _,
                        kept: _,
                    } => $walk(resume, hold, visit),
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
                    resume,
                } => $walk(resume, Hold::Suspended, visit),
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
                    resume,
                    groups: _,
                    kept: _,
                } => $walk(resume, hold, visit),
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
                Frame::ManaAbility { activation } => {
                    $walk(& $($mutability)? activation.resume, Hold::Suspended, visit)
                }
                Frame::DamageAllocation {
                    transaction,
                    effect: _,
                } => $walk_damage(transaction, hold, visit),
                Frame::DamageEffects { transaction } => $walk_damage(transaction, hold, visit),
                Frame::CounterPlacement { transaction } => visit($Held::Invocation {
                    hold,
                    id: transaction.frame.stack_id,
                }),
                Frame::TriggerOrder { placement } => {
                    $walk(& $($mutability)? placement.resume, hold, visit)
                }
                Frame::TriggerTargets {
                    placement,
                    stack_id: _,
                } => $walk(& $($mutability)? placement.resume, hold, visit),
                Frame::ResolutionChoice { frame, choice: _ } => visit($Held::Invocation {
                    hold,
                    id: frame.stack_id,
                }),
                Frame::CommanderReturn {
                    object: _,
                    destination: _,
                    continuation,
                } => match & $($mutability)? **continuation {
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
                    CommanderContinuation::StateBased { resume } => $walk(resume, hold, visit),
                    CommanderContinuation::StateBasedBatch {
                        remaining: _,
                        choices: _,
                        resume,
                    } => $walk(resume, hold, visit),
                },
                Frame::Stabilize {
                    resume,
                    cleanup: _,
                    changed: _,
                } => $walk(resume, hold, visit),
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

impl Frame {
    /// Visits every invocation and damage transaction this frame holds,
    /// outermost first, through every resumed and suspended frame.
    pub fn for_each_held<'a>(&'a self, visit: &mut dyn FnMut(HeldRef<'a>)) {
        walk(self, Hold::Resolving, visit)
    }

    pub(super) fn for_each_held_mut<'a>(&'a mut self, visit: &mut dyn FnMut(HeldMut<'a>)) {
        walk_mut(self, Hold::Resolving, visit)
    }

    /// The invocation this frame is resolving. An owner suspended beneath its
    /// child announcement or a mana ability is not resolving.
    pub fn resolving_invocation(&self) -> Option<StackId> {
        let mut resolving = None;
        self.for_each_held(&mut |held| {
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
    pub fn holds(&self, id: StackId) -> bool {
        let mut found = false;
        self.for_each_held(&mut |held| {
            found |= matches!(held, Held::Invocation { hold: _, id: held } if held == id);
        });
        found
    }
}

impl Game {
    /// The single owner of an invocation: its live stack entry, or the
    /// detached entry a frame is still resolving after it left the stack.
    pub(crate) fn entry(&self, id: StackId) -> Result<&StackInstance, String> {
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
    ) -> Result<BatchCause, String> {
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
    pub(crate) fn unstack(&mut self, index: usize) -> Result<StackId, String> {
        let rules = &mut self.state.rules;
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
        let rules = &mut self.state.rules;
        if rules.detached.is_empty() {
            return;
        }
        let frame = &rules.frame;
        rules.detached.retain(|id, _| frame.holds(*id));
    }
}
