//! The reuse path of publication: the record of what the last publication
//! read, the test that the current rules still project the same rows, and
//! the refresh of the two fields a view shows that may still differ.
use crate::engine::*;

/// What the last publication read: the rules it projected and each viewer's
/// knowledge as that publication left it. A derived accelerator, never state.
/// It shares the state's tables, so a later write to any of them allocates a
/// copy and this record keeps exactly what was published.
pub struct Published {
    pub(super) rules: RulesState,
    pub(super) cache: visibility::ProjectionCache,
    knowledge: Vec<Shared<visibility::Knowledge>>,
}

impl Frame {
    /// True when this frame, with every frame it resumes, shows no view
    /// anything: no reveal, look, offer, choice list or resolving invocation.
    /// Two such frames differ, for a view, only in whether priority is held.
    /// Every variant is named: a new frame must be classified here.
    fn shows_nothing<'a>(&'a self, mut frames: &'a [Frame]) -> bool {
        let mut frame = self;
        loop {
            match frame {
                Frame::Priority
                | Frame::Transition
                | Frame::EnterStep
                | Frame::CleanupClear
                | Frame::Damage
                | Frame::Draw
                | Frame::Attackers
                | Frame::Blockers { remaining: _ } => return true,
                Frame::Stabilize {
                    resume,
                    cleanup: _,
                    changed: _,
                } => {
                    let Some(parent) = frames.get(resume.0) else {
                        return false;
                    };
                    frame = parent;
                    frames = &frames[..resume.0];
                }
                Frame::BattlefieldEntry { entry: _ }
                | Frame::Kernel { task: _ }
                | Frame::GraveyardGroup { tx: _ }
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
                | Frame::ChooseModes { cast: _ }
                | Frame::AdditionalCosts { cast: _ }
                | Frame::AnnounceX { cast: _ }
                | Frame::HybridCost { cast: _ }
                | Frame::SacrificeCost { cast: _ }
                | Frame::ChosenObjectCost {
                    cast: _,
                    selected: _,
                }
                | Frame::ResolutionPayment {
                    frame: _,
                    payment: _,
                }
                | Frame::TurnFaceUp { payment: _ }
                | Frame::ManaAbility { activation: _ }
                | Frame::Setup
                | Frame::CounterPlacement { transaction: _ }
                | Frame::DamageEffects { transaction: _ }
                | Frame::TriggerOrder { placement: _ }
                | Frame::TriggerTargets {
                    placement: _,
                    stack_id: _,
                }
                | Frame::EntityTargets { cast: _ }
                | Frame::StackTargets { cast: _ }
                | Frame::PlayerTargets { cast: _ }
                | Frame::SpendMana { cast: _ }
                | Frame::ResolutionChoice {
                    frame: _,
                    choice: _,
                }
                | Frame::CommanderReturn {
                    object: _,
                    destination: _,
                    continuation: _,
                }
                | Frame::DamageChoice { source: _ }
                | Frame::Discard {
                    player: _,
                    count: _,
                }
                | Frame::Targets { cast: _ }
                | Frame::Mana { cast: _ }
                | Frame::Payment { cast: _ }
                | Frame::Resolve { frame: _ }
                | Frame::Fault { reason: _ }
                | Frame::Finished { winner: _ } => return false,
            }
        }
    }
}

impl Published {
    pub(super) fn of(state: &State, cache: visibility::ProjectionCache) -> Self {
        Self {
            rules: state.rules.clone(),
            cache,
            knowledge: state.knowledge.visibility.clone(),
        }
    }

    /// True when `rules` projects the same rows, handles and public tables as
    /// the published rules: every field is equal except those no projection
    /// reads, and the priority, turn step and view-neutral frame that the
    /// reuse path refreshes itself. Both states are destructured in full, so
    /// a new rules field must be classified here before this compiles.
    fn projects_same_rows(&self, rules: &RulesTables) -> bool {
        let RulesTables {
            mana_ability_uses,
            activation_uses: _,
            trigger_uses: _,
            tokens,
            retained_auxiliary_charge: _,
            typed_fault: _,
            match_state,
            effects,
            next_effect_id,
            next_damage_transaction_id,
            pending_triggers,
            resolution_triggers: _,
            resolution_trigger_clock: _,
            next_trigger_id,
            libraries,
            combat,
            combat_declared,
            combat_defenders,
            damage_choices,
            defender_damage_choices,
            first_strike_participants,
            blocking_creatures,
            cleanup_repeat,
            simultaneous_losses,
            losing_players,
            // Decision identity and execution counters: token keys read the
            // revision, no projection does.
            revision: _,
            rule_steps: _,
            players,
            cards,
            objects,
            active,
            turn_owner,
            turn,
            original_seating,
            cast_history,
            commanders,
            commander_returns,
            control_effects,
            timestamps,
            // A queued concession and an accepted input suppress the decision
            // token only; the departure itself changes players and objects.
            pending_concessions: _,
            // Refreshed by the reuse path together with the frame.
            priority: _,
            passes: _,
            next_stack_id,
            stack,
            detached,
            frames,
            frame,
            pending: _,
            next_batch_id: _,
            next_event_sequence: _,
        } = rules;
        let was = &self.rules;
        // Cheapest and most often different first. The large tables compare
        // by shared allocation before they compare by value.
        (frames == &was.frames && *frame == was.frame || (frame.shows_nothing(frames) && was.frame.shows_nothing(&was.frames)))
            && objects.shares_rows(&was.objects)
            && Shared::same(stack, &was.stack)
            && Shared::same(effects, &was.effects)
            && Shared::same(libraries, &was.libraries)
            && Arc::ptr_eq(cards, &was.cards)
            && Arc::ptr_eq(tokens, &was.tokens)
            // Rows show whether an object entered this turn; the step and the
            // land count are refreshed by the reuse path.
            && turn.number == was.turn.number
            && *active == was.active
            && *turn_owner == was.turn_owner
            && *players == was.players
            && *pending_triggers == was.pending_triggers
            && *combat == was.combat
            && *combat_declared == was.combat_declared
            && *combat_defenders == was.combat_defenders
            && *control_effects == was.control_effects
            && *commanders == was.commanders
            && *commander_returns == was.commander_returns
            && *match_state == was.match_state
            && *mana_ability_uses == was.mana_ability_uses
            && *damage_choices == was.damage_choices
            && *defender_damage_choices == was.defender_damage_choices
            && *first_strike_participants == was.first_strike_participants
            && *blocking_creatures == was.blocking_creatures
            && *cleanup_repeat == was.cleanup_repeat
            && *simultaneous_losses == was.simultaneous_losses
            && *losing_players == was.losing_players
            && *original_seating == was.original_seating
            && *cast_history == was.cast_history
            && *detached == was.detached
            && *next_effect_id == was.next_effect_id
            && *next_damage_transaction_id == was.next_damage_transaction_id
            && *next_trigger_id == was.next_trigger_id
            && *timestamps == was.timestamps
            && *next_stack_id == was.next_stack_id
    }

    pub(super) fn same_viewer(&self, viewer: usize, state: &State) -> bool {
        self.knowledge
            .get(viewer)
            .zip(state.knowledge.visibility.get(viewer))
            .is_some_and(|(old, current)| Shared::same(old, current))
    }

    /// True when no viewer's knowledge was written since this publication.
    fn knowledge_untouched(&self, knowledge: &KnowledgeState) -> bool {
        self.knowledge.len() == knowledge.visibility.len()
            && self
                .knowledge
                .iter()
                .zip(&knowledge.visibility)
                .all(|(was, now)| Shared::same(was, now))
    }
}

impl RulesTxn {
    /// The reuse path. Returns false, having changed nothing, unless every
    /// viewer's last view still shows the rows of the current rules.
    pub(super) fn republish_rows(&mut self, decider: Option<&Decider>) -> Result<bool, String> {
        let Some(published) = &self.published else {
            return Ok(false);
        };
        let state = &self.state;
        if !published.knowledge_untouched(&state.knowledge)
            || !published.projects_same_rows(&state.rules)
            || state
                .knowledge
                .visibility
                .iter()
                .any(|knowledge| knowledge.history.last().is_none())
        {
            return Ok(false);
        }
        let revision = state.rules.revision;
        let priority = matches!(state.rules.frame, Frame::Priority).then_some(state.rules.priority);
        for v in 0..state.knowledge.visibility.len() {
            let knowledge = &self.state.knowledge.visibility[v];
            let key = decider
                .filter(|decider| decider.player.0 as usize == v)
                .map(|decider| (revision, decider.reason.code()));
            let last = knowledge.history.last().ok_or("missing published view")?;
            let view = (last.priority != priority || last.turn != self.state.rules.turn)
                .then(|| last.with_priority_and_turn(priority, self.state.rules.turn.clone()));
            if view.is_none() && knowledge.decision_key() == key {
                // Nothing to write: this viewer's knowledge stays shared.
                continue;
            }
            let knowledge = &mut *self.state.knowledge.visibility[v];
            knowledge.set_decision_key(v, key)?;
            if let Some(view) = view {
                knowledge.history.push(view);
            }
        }
        Ok(true)
    }
}
