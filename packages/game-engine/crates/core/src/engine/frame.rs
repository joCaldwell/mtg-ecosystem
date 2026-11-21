//! One exhaustive traversal of every invocation copy a `Frame` holds, and the
//! lease that reconciles a rule step's private frame copy when it commits.
//!
//! CR 608.2h/113.7a: an ability keeps resolving with its source's last-known
//! information. The live stack entry owns those facts; a frame holds copies.
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

/// How a frame holds an invocation copy.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Hold {
    /// The frame is resolving this invocation, possibly beneath state-based
    /// actions, a legend-rule choice or trigger placement that will resume it.
    Resolving,
    /// The invocation waits beneath its owned child announcement, a payment
    /// or a mana ability.
    Suspended,
}

/// One thing a frame holds: an invocation copy, or the prepared entries of a
/// damage transaction, which keep source appearances.
pub(super) enum Held<Invocation, Damage> {
    Invocation { hold: Hold, entry: Invocation },
    Damage(Damage),
}

pub(super) type HeldRef<'a> = Held<&'a StackInstance, &'a Vec<effects::PreparedDamage>>;
pub(super) type HeldMut<'a> = Held<&'a mut StackInstance, &'a mut Vec<effects::PreparedDamage>>;

/// A rule step's obligation to reconcile the frame it leaves behind. The step
/// dispatches on a private copy; whatever it installs is stale wherever the
/// step moved an invocation's source. `Game::commit_frame` discharges the
/// lease, and a failed step discards its staged state instead. The type is
/// neither `Clone` nor constructible elsewhere, but the compiler only warns
/// when one is dropped, and direct writes to `rules.frame` remain possible:
/// the guarantee is the commit in `Game::step`, not this token.
#[must_use = "commit the lease so the installed frame carries its owners' last-known facts"]
pub(super) struct FrameLease(());

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
                    entry: & $($mutability)? frame.spell,
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
                        entry: & $($mutability)? progress.frame.spell,
                    }),
                    CastOwner::Cascade(progress) => visit($Held::Invocation {
                        hold: Hold::Suspended,
                        entry: & $($mutability)? progress.frame.spell,
                    }),
                    CastOwner::RevealAndCast(progress) => visit($Held::Invocation {
                        hold: Hold::Suspended,
                        entry: & $($mutability)? progress.frame.spell,
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
                    entry: & $($mutability)? frame.spell,
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
                        entry: & $($mutability)? frame.spell,
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
                    entry: & $($mutability)? transaction.frame.spell,
                }),
                Frame::PlayHideawayLinked { progress } => visit($Held::Invocation {
                    hold,
                    entry: & $($mutability)? progress.frame.spell,
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
                        entry: & $($mutability)? progress.frame.spell,
                    }),
                    EntryOrigin::Copy {
                        frame,
                        selection: _,
                    } => visit($Held::Invocation {
                        hold,
                        entry: & $($mutability)? frame.spell,
                    }),
                    EntryOrigin::LandPlay {
                        player: _,
                        decision_id: _,
                        back_face: _,
                    } => {}
                    EntryOrigin::Search { frame } => visit($Held::Invocation {
                        hold,
                        entry: & $($mutability)? frame.spell,
                    }),
                    EntryOrigin::Cascade { progress } => visit($Held::Invocation {
                        hold,
                        entry: & $($mutability)? progress.frame.spell,
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
                    entry: & $($mutability)? progress.frame.spell,
                }),
                Frame::HandCast { frame, child: _ } => visit($Held::Invocation {
                    hold,
                    entry: & $($mutability)? frame.spell,
                }),
                Frame::Cascade { progress } => visit($Held::Invocation {
                    hold,
                    entry: & $($mutability)? progress.frame.spell,
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
                    entry: & $($mutability)? frame.spell,
                }),
                Frame::ResolutionPayment { frame, payment: _ } => visit($Held::Invocation {
                    hold,
                    entry: & $($mutability)? frame.spell,
                }),
                Frame::OptionalDamage { frame } => visit($Held::Invocation {
                    hold,
                    entry: & $($mutability)? frame.spell,
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
                    entry: & $($mutability)? transaction.frame.spell,
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
                    entry: & $($mutability)? frame.spell,
                }),
                Frame::CommanderReturn {
                    object: _,
                    destination: _,
                    continuation,
                } => match & $($mutability)? **continuation {
                    CommanderContinuation::CascadeBottom { progress } => {
                        visit($Held::Invocation {
                            hold,
                            entry: & $($mutability)? progress.frame.spell,
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
                        entry: & $($mutability)? frame.spell,
                    }),
                    CommanderContinuation::LookTopMana { frame, revealed: _ } => {
                        visit($Held::Invocation {
                            hold,
                            entry: & $($mutability)? frame.spell,
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
                        entry: & $($mutability)? frame.spell,
                    }),
                    CommanderContinuation::SpellMove {
                        frame,
                        source_block: _,
                    } => visit($Held::Invocation {
                        hold,
                        entry: & $($mutability)? frame.spell,
                    }),
                    CommanderContinuation::ResolutionDraw {
                        frame,
                        player: _,
                        source_block: _,
                    } => visit($Held::Invocation {
                        hold,
                        entry: & $($mutability)? frame.spell,
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
                    entry: & $($mutability)? frame.spell,
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
    /// Visits every invocation copy and damage transaction this frame holds,
    /// outermost first, through every resumed and suspended frame.
    pub(super) fn for_each_held<'a>(&'a self, visit: &mut dyn FnMut(HeldRef<'a>)) {
        walk(self, Hold::Resolving, visit)
    }

    pub(super) fn for_each_held_mut<'a>(&'a mut self, visit: &mut dyn FnMut(HeldMut<'a>)) {
        walk_mut(self, Hold::Resolving, visit)
    }

    /// The invocation this frame is resolving. An owner suspended beneath its
    /// child announcement or a mana ability is not resolving.
    pub(super) fn resolving_invocation(&self) -> Option<&StackInstance> {
        let mut resolving = None;
        self.for_each_held(&mut |held| {
            if let Held::Invocation {
                hold: Hold::Resolving,
                entry,
            } = held
            {
                resolving.get_or_insert(entry);
            }
        });
        resolving
    }
}

/// Carries the two fields a source departure rewrites from the live owner.
pub(super) fn carry_last_known(copy: &mut StackInstance, owner: &StackInstance) {
    if copy.source_facts != owner.source_facts {
        copy.source_facts.clone_from(&owner.source_facts);
    }
    if let (
        StackKind::Triggered { witness, .. },
        StackKind::Triggered {
            witness: current, ..
        },
    ) = (&mut copy.kind, &owner.kind)
        && witness != current
    {
        witness.clone_from(current);
    }
}

/// True when `issued` is `owner` as it was when a permission was issued:
/// equal except for the source facts the owner has retained since.
pub(super) fn issued_by(issued: &StackInstance, owner: &StackInstance) -> bool {
    if issued == owner {
        return true;
    }
    let mut then = issued.clone();
    then.source_facts.clone_from(&owner.source_facts);
    then == *owner
}

impl Game {
    /// Leases the suspended frame to one rule step.
    pub(super) fn lease_frame(&self) -> (Frame, FrameLease) {
        (self.state.rules.frame.clone(), FrameLease(()))
    }

    /// Discharges a step's lease on the frame it installed.
    pub(super) fn commit_frame(&mut self, lease: FrameLease) {
        let FrameLease(()) = lease;
        self.reconcile_frame();
    }

    /// Reconciles the suspended frame with the live stack entries that own
    /// its invocations, so no stale copy outlives a rule step.
    pub(super) fn reconcile_frame(&mut self) {
        let rules = &mut self.state.rules;
        let stack = &rules.stack;
        rules.frame.for_each_held_mut(&mut |held| match held {
            HeldMut::Invocation { hold: _, entry } => {
                if let Some(owner) = stack.iter().find(|owner| owner.id == entry.id) {
                    carry_last_known(entry, owner);
                }
            }
            // Source appearances have no second owner; departures write them.
            HeldMut::Damage(_) => {}
        });
    }
}
