//! CR 113.7a, 601.2, 608.2g. The damaged player is not the chooser.
use super::*;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HandCastPermission {
    /// The combat-damage trigger that issued this one-use permission. Its
    /// entry is the single owner of the trigger and of its source's facts.
    issuer: StackId,
    original: ObjectId,
    caster: PlayerId,
    child: StackId,
}

impl RulesTxn {
    fn validate_hand_cast_issuer(&self, issuer: &StackInstance) -> Result<(), String> {
        let StackKind::Triggered {
            ability: EffectiveTriggerAbility::Intrinsic { .. },
            witness:
                TriggerEventWitness::DamageDealt {
                    kind: DamageKind::Combat,
                    damage,
                    ..
                },
            grant: None,
            match_condition: None,
            ..
        } = &issuer.kind
        else {
            return Err("hand cast requires a printed combat damage trigger".into());
        };
        {
            return Err("invalid combat hand cast issuer".into());
        }
        Ok(())
    }

    pub(super) fn hand_cast_permission(
        &self,
        frame: &Resolving,
        original: ObjectId,
        child: StackId,
    ) -> Result<HandCastPermission, String> {
        self.validate_resolving_owned(frame, None)?;
        self.validate_hand_cast_issuer(self.entry(frame.stack_id)?)?;
        let caster = self.entry(frame.stack_id)?.controller;
        if frame.cursor != 0
            || self.top()?.id != frame.stack_id
            || child.0 != self.state.rules.next_stack_id
            || self.current(original)?.zone != (Zone::Hand { owner: caster })
        {
            return Err("invalid one-use hand cast authority".into());
        }
        Ok(HandCastPermission {
            issuer: frame.stack_id,
            original,
            caster,
            child,
        })
    }

    pub(super) fn validate_hand_cast_child(&self, child: &StackInstance) -> Result<(), String> {
        let Some(permission) = &child.hand_cast_permission else {
            return Ok(());
        };
        // The issuing trigger is validated while it has an owner. Once it has
        // finished, only the child's own identity remains to check.
        if let Ok(issuer) = self.entry(permission.issuer) {
            self.validate_hand_cast_issuer(issuer)?;
            if permission.caster != issuer.controller {
                return Err("hand cast child identity invariant".into());
            }
        }
        if !child.kind.is_spell()
            || child.id != permission.child
            || permission.issuer >= child.id
            || child.controller != permission.caster
            || child.source.card != permission.original.card
            || permission.original.incarnation.checked_add(1) != Some(child.source.incarnation)
            || child.cast_from
                != (Zone::Hand {
                    owner: permission.caster,
                })
            || child.alternative_cost != Some(cascade::AlternativeCost::WithoutManaCost)
        {
            return Err("hand cast child identity invariant".into());
        }
        Ok(())
    }

    pub(super) fn eligible_hand_spell(
        &self,
        object: &Object,
        player: PlayerId,
    ) -> Result<bool, String> {
        if object.zone != (Zone::Hand { owner: player }) {
            return Ok(false);
        }
        let definition = self.definition(object.id.card)?;
        Ok(!definition.characteristics.has_type(CardKind::Land)
            && definition.spell.as_ref().is_some_and(|program| {
                self.spell_proposal_obstacle(program, player, &definition.characteristics)
                    .is_none()
            }))
    }

    pub(super) fn hand_cast_candidates(&self, frame: &Resolving) -> Result<Vec<ObjectId>, String> {
        self.validate_hand_cast_issuer(self.entry(frame.stack_id)?)?;
        let player = self.entry(frame.stack_id)?.controller;
        let mut candidates = vec![];
        if self.is_live(player) && !self.cast_is_prohibited(player)? {
            for object in self.state.rules.objects.values() {
                if self.eligible_hand_spell(object, player)? {
                    candidates.push(object.id);
                }
            }
        }
        Ok(candidates)
    }

    pub(super) fn validate_hand_cast_state(&self) -> Result<(), String> {
        let mut consumed = BTreeSet::new();
        for child in &self.state.rules.stack {
            if let Some(permission) = &child.hand_cast_permission {
                self.validate_hand_cast_child(child)?;
                if !consumed.insert(permission.issuer) {
                    return Err("hand casting permission spent twice".into());
                }
            }
        }
        let (frame, child, announcing) =
            match self.underlying_mana_frame(&self.state.rules.frame)? {
                Frame::HandCast { frame, child } => (frame, *child, false),
                Frame::AdditionalCosts { cast }
                | Frame::HybridCost { cast }
                | Frame::AnnounceX { cast }
                | Frame::Modes { cast }
                | Frame::Targets { cast }
                | Frame::EntityTargets { cast }
                | Frame::StackTargets { cast }
                | Frame::PlayerTargets { cast }
                | Frame::Mana { cast }
                | Frame::SpendMana { cast }
                | Frame::Payment { cast }
                | Frame::SacrificeCost { cast } => {
                    let top = self.top()?;
                    let CastCompletion::HandTrigger { frame } = &cast.completion else {
                        if top.hand_cast_permission.is_some() {
                            return Err("hand cast lost its owning trigger".into());
                        }
                        return Ok(());
                    };
                    let permission = top
                        .hand_cast_permission
                        .as_ref()
                        .ok_or("hand cast lost its permission")?;
                    if cast.stack_id != top.id || cast.original_object != permission.original {
                        return Err("hand cast announcement identity mismatch".into());
                    }
                    (frame, Some(top.id), true)
                }
                _ => return Ok(()),
            };
        self.validate_resolving_owned(frame, child)?;
        self.validate_hand_cast_issuer(self.entry(frame.stack_id)?)?;
        if frame.cursor != 0 {
            return Err("hand cast trigger cursor".into());
        }
        if let Some(child) = child {
            let top = self.top()?;
            let permission = top
                .hand_cast_permission
                .as_ref()
                .ok_or("hand cast child permission missing")?;
            // The permission names its issuer; the issuing trigger's entry
            // owns its source's last-known facts (CR 608.2h/113.7a).
            if permission.issuer != frame.stack_id
                || top.id != child
                || top.paid.is_some() == announcing
            {
                return Err("hand cast parent and child mismatch".into());
            }
        } else if consumed.contains(&frame.stack_id) {
            return Err("hand casting permission already spent".into());
        }
        Ok(())
    }

    pub(super) fn hand_cast_request(&self) -> Result<Option<Request>, String> {
        let Frame::HandCast { frame, child: None } = &self.state.rules.frame else {
            return Ok(None);
        };
        let candidates = self.hand_cast_candidates(frame)?;
        Ok((!candidates.is_empty()).then_some(Request::HandCast {
            player: self.entry(frame.stack_id)?.controller,
            decision_id: self.state.rules.revision,
            candidates,
        }))
    }

    pub(super) fn validate_hand_cast_response(
        &self,
        actor: PlayerId,
        response: &Response,
    ) -> Result<(), String> {
        let Some(Request::HandCast {
            player, candidates, ..
        }) = self.hand_cast_request()?
        else {
            return Err("no hand cast offer".into());
        };
        if actor == player
            && (matches!(response, Response::Pass)
                || matches!(response, Response::Cast { card } if candidates.iter().any(|o| o.card == *card)))
        {
            Ok(())
        } else {
            Err("unauthorized hand cast choice".into())
        }
    }

    pub(super) fn revalidate_hand_cast_departure(&mut self) -> Result<(), String> {
        if let Frame::HandCast { frame, child: None } = &self.state.rules.frame
            && let Some(input) = &self.state.rules.pending
            && input.player == self.entry(frame.stack_id)?.controller
        {
            let candidates = self.hand_cast_candidates(frame)?;
            let invalid = match input.response {
                Response::Pass => candidates.is_empty(),
                Response::Cast { card } => !candidates.iter().any(|object| object.card == card),
                _ => return Err("unexpected retained hand cast response".into()),
            };
            if invalid {
                self.rules_mut().pending = None;
            }
        }
        Ok(())
    }
    pub(super) fn hand_cast_step(
        &mut self,
        frame: Box<Resolving>,
        child: Option<StackId>,
    ) -> Result<(), String> {
        self.validate_resolving_owned(&frame, child)?;
        if child.is_none() {
            let candidates = self.hand_cast_candidates(&frame)?;
            if let Some(input) = self.rules_mut().pending.take() {
                if input.player != self.entry(frame.stack_id)?.controller {
                    return Err("hand cast actor mismatch".into());
                }
                match input.response {
                    Response::Pass => (),
                    Response::Cast { card } => {
                        if let Some(object) = candidates.into_iter().find(|o| o.card == card) {
                            return self.announce_combat_hand_spell(frame, object);
                        }
                    }
                    _ => return Err("invalid hand cast response".into()),
                }
            } else if !candidates.is_empty() {
                return Err("missing hand cast choice".into());
            }
        }
        self.finish_ability(frame.stack_id, true)
    }
}
