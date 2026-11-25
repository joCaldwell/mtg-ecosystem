//! CR 602.2b, 601.2g-h, 701.9, 701.13, 701.26: choose exact objects
//! after targets and any mana window, then pay all terms in one rule step.
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;
use ChosenCostAction::{Discard, ExileFromGraveyard, Tap};

mod mana;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(in crate::engine) struct PaidObject {
    before: ObjectBefore,
    outcome: CostOutcome,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
enum CostOutcome {
    Moved(MoveOutcome),
    Tapped { later_untapped: bool },
}

impl RulesTxn {
    pub(super) fn chosen_cost_frame(cast: Casting) -> Frame {
        Frame::ChosenObjectCost {
            cast,
            selected: vec![],
        }
    }

    pub(super) fn invocation_chosen_cost(
        &self,
        entry: &StackInstance,
    ) -> Result<Option<ChosenObjectCost>, String> {
        let Some(index) = entry.kind.activated_index() else {
            return Ok(None);
        };
        let definition = self.copiable_definition(entry.source)?;
        match &definition
            .activated
            .get(index as usize)
            .ok_or("missing activation cost")?
            .cost
        {
            ActivationCost::ChooseObjects(cost) => Ok(Some(cost.clone())),
            _ => Ok(None),
        }
    }

    fn source_tap_payable(&self, player: PlayerId, source: ObjectId) -> bool {
        // CR 302.6 applies to the source's {T} symbol only.
        self.current(source).is_ok_and(|o| {
            o.present_on_battlefield()
                && o.controller == player
                && !o.tapped
                && (o.attack_ready
                    || !self.is_battlefield_creature(source)
                    || self.has_keyword(source, KeywordAbility::Haste))
        })
    }

    fn eligible_cost_objects(
        &self,
        player: PlayerId,
        source: ObjectId,
        cost: &ChosenObjectCost,
    ) -> Result<Vec<ObjectId>, String> {
        if cost.tap_source && !self.source_tap_payable(player, source) {
            return Ok(vec![]);
        }
        let mut eligible = vec![];
        for object in self.state.rules.objects.values() {
            if (cost.another || cost.action == Tap && cost.tap_source) && object.id == source {
                continue;
            }
            let matches_zone = match cost.action {
                Discard => object.zone == (Zone::Hand { owner: player }),
                ExileFromGraveyard => object.zone == (Zone::Graveyard { owner: player }),
                Tap => {
                    object.present_on_battlefield() && object.controller == player && !object.tapped
                }
            };
            if matches_zone
                && (cost.action == Tap || !self.state.rules.tokens.contains_key(&object.id.card))
                && (cost.filter == ChosenCostFilter::Any
                    || cost.filter.matches(&self.characteristics(object.id)?))
            {
                eligible.push(object.id);
            }
        }
        Ok(eligible)
    }

    pub(super) fn can_choose_cost(
        &self,
        player: PlayerId,
        source: ObjectId,
        cost: &ChosenObjectCost,
    ) -> Result<bool, String> {
        let eligible = self.eligible_cost_objects(player, source, cost)?;
        let Ok(count) = usize::try_from(cost.count) else {
            return Ok(false);
        };
        if eligible.len() < count {
            return Ok(false);
        }
        match &cost.mana {
            Some(mana) => self.can_fund_chosen_cost(player, source, cost, mana, &eligible, count),
            None => Ok(true),
        }
    }

    fn next_chosen_objects(
        &self,
        entry: &StackInstance,
        cost: &ChosenObjectCost,
        prefix: &[ObjectId],
    ) -> Result<Vec<ObjectId>, String> {
        let mut eligible = self.eligible_cost_objects(entry.controller, entry.source, cost)?;
        let Ok(count) = usize::try_from(cost.count) else {
            return Ok(vec![]);
        };
        if count > eligible.len()
            || prefix.len() >= count
            || !valid_selection_prefix(prefix, &eligible)
        {
            return Ok(vec![]);
        }
        // Each set is reachable in one order. Offer an object only if enough
        // higher identities remain to complete the fixed count.
        if let Some(last) = prefix.last() {
            eligible.retain(|id| id > last);
        }
        eligible.truncate(eligible.len().saturating_sub(count - prefix.len() - 1));
        Ok(eligible)
    }

    pub(super) fn chosen_cost_request(
        &self,
        selected: &[ObjectId],
    ) -> Result<Option<Decision<Rules>>, String> {
        let entry = self.top()?;
        let cost = self
            .invocation_chosen_cost(entry)?
            .ok_or("missing chosen-object cost")?;
        let eligible = self.next_chosen_objects(entry, &cost, selected)?;
        Ok((!eligible.is_empty()).then(|| {
            self.pick_objects(
                entry.controller,
                Reason::ChosenObjectCost {
                    action: cost.action,
                },
                eligible,
                1,
            )
        }))
    }

    pub(super) fn chosen_cost_step(
        &mut self,
        cast: Casting,
        mut selected: Vec<ObjectId>,
    ) -> Result<(), String> {
        let entry = self.top()?;
        let cost = self
            .invocation_chosen_cost(entry)?
            .ok_or("missing chosen-object cost")?;
        let eligible = self.next_chosen_objects(entry, &cost, &selected)?;
        if eligible.is_empty() {
            let undo = cast.undo_mana;
            self.reverse_cast(cast, "chosen-object cost no longer payable", undo)?;
            return Ok(self.stabilize()?);
        }
        let object = self
            .rules_mut()
            .pending
            .take()
            .ok_or("missing cost selection")?
            .object()?;
        if eligible.binary_search(&object).is_err() {
            return Err("ineligible cost selection".into());
        }
        selected.push(object);
        if u32::try_from(selected.len()) == Ok(cost.count) {
            self.payment_step_with_objects(cast, &selected)
        } else {
            self.rules_mut().frame = Frame::ChosenObjectCost { cast, selected };
            Ok(())
        }
    }

    pub(super) fn chosen_payment_events(
        &self,
        entry: &StackInstance,
        selected: &[ObjectId],
    ) -> Result<Option<EventGroup>, String> {
        let Some(cost) = self.invocation_chosen_cost(entry)? else {
            return if selected.is_empty() {
                Ok(None)
            } else {
                Err("unexpected cost selection".into())
            };
        };
        let eligible = self.eligible_cost_objects(entry.controller, entry.source, &cost)?;
        if u32::try_from(selected.len()) != Ok(cost.count)
            || !valid_selection_prefix(selected, &eligible)
        {
            return Err("chosen-object cost is not payable".into());
        }
        let occurrences = selected
            .iter()
            .map(|&object| match cost.action {
                Discard => ProposedEvent::DiscardCard {
                    object,
                    player: entry.controller,
                },
                ExileFromGraveyard => ProposedEvent::Move {
                    object,
                    destination: Zone::Exile,
                },
                Tap => ProposedEvent::Tap { object },
            })
            .collect();
        let group = EventGroup::Simultaneous { occurrences };
        Ok(Some(if cost.tap_source {
            EventGroup::Ordered {
                children: vec![
                    EventGroup::Occurrence {
                        proposed: ProposedEvent::Tap {
                            object: entry.source,
                        },
                    },
                    group,
                ],
            }
        } else {
            group
        }))
    }

    pub(super) fn commit_chosen_payment(
        &mut self,
        entry: &StackInstance,
        group: Option<EventGroup>,
    ) -> Result<(), String> {
        let Some(group) = group else {
            return Ok(());
        };
        let batch = self.commit_batch(
            BatchCause::Payment {
                stack_id: entry.id,
                player: entry.controller,
            },
            group,
        )?;
        let paid = batch
            .occurrences
            .into_iter()
            .map(|event| {
                let (id, outcome) = match event.actual {
                    EventOutcome::CardDiscarded { movement, .. }
                    | EventOutcome::Moved { movement } => {
                        (movement.previous, CostOutcome::Moved(movement))
                    }
                    EventOutcome::Tapped { object } => (
                        object,
                        CostOutcome::Tapped {
                            later_untapped: false,
                        },
                    ),
                    _ => return Err("unexpected chosen-object payment outcome"),
                };
                let before = event
                    .before
                    .objects
                    .into_iter()
                    .find(|o| o.object == id)
                    .ok_or("missing chosen-object payment facts")?;
                Ok(PaidObject { before, outcome })
            })
            .collect::<Result<Vec<_>, _>>()?;
        self.rules_mut()
            .stack
            .last_mut()
            .ok_or("missing chosen-cost activation")?
            .chosen_payment = paid;
        Ok(())
    }

    pub(super) fn mark_chosen_cost_untapped(&mut self, object: ObjectId) {
        let matches = |p: &PaidObject| {
            p.before.object == object
                && matches!(
                    p.outcome,
                    CostOutcome::Tapped {
                        later_untapped: false
                    }
                )
        };
        let rules = &self.state.rules;
        if rules
            .stack
            .iter()
            .chain(rules.detached.values())
            .any(|e| e.chosen_payment.iter().any(matches))
        {
            let rules = self.rules_mut();
            for receipt in rules
                .stack
                .iter_mut()
                .chain(rules.detached.values_mut())
                .flat_map(|e| &mut e.chosen_payment)
            {
                if matches(receipt) {
                    receipt.outcome = CostOutcome::Tapped {
                        later_untapped: true,
                    };
                }
            }
        }
    }

    pub(super) fn revalidate_chosen_cost_departure(&mut self) -> Result<(), String> {
        let Frame::ChosenObjectCost { cast, selected } = &self.state.rules.frame else {
            return Ok(());
        };
        if !selected.is_empty() && self.chosen_cost_request(selected)?.is_none() {
            let cast = cast.clone();
            self.rules_mut().frame = Self::chosen_cost_frame(cast);
            self.rules_mut().pending = None;
        }
        Ok(())
    }

    pub(super) fn validate_chosen_cost_frame(
        &self,
        entry: &StackInstance,
        cast: &Casting,
        prefix: &[ObjectId],
    ) -> Result<(), String> {
        let cost = self
            .invocation_chosen_cost(entry)?
            .ok_or("snapshot lacks chosen-object cost")?;
        let spending = cast
            .spending
            .as_ref()
            .ok_or("snapshot lacks chosen-cost mana selection")?;
        self.validate_announced_allocation(
            entry,
            &entry.total_cost,
            spending,
            &cast.spending_restricted,
        )?;
        if (!prefix.is_empty() && self.next_chosen_objects(entry, &cost, prefix)?.is_empty())
            || (cost.mana.is_none() && (!cast.mana_activations.is_empty() || cast.undo_mana))
        {
            return Err("snapshot chosen-object cost progress invariant".into());
        }
        Ok(())
    }

    pub(super) fn validate_chosen_announcement_stage(
        &self,
        entry: &StackInstance,
        cost: &ChosenObjectCost,
        stage: &Frame,
    ) -> Result<(), String> {
        let target = self.program_for(entry)?.targets.first().map(|t| t.domain);
        let valid = match stage {
            Frame::Targets { cast } => {
                cast.spending.is_none()
                    && matches!(
                        target,
                        Some(
                            TargetDomain::BattlefieldCreature
                                | TargetDomain::BattlefieldPermanent
                                | TargetDomain::ControlledBattlefieldCreature
                                | TargetDomain::ControlledBattlefieldPermanent
                        )
                    )
            }
            Frame::EntityTargets { cast } => {
                cast.spending.is_none() && target == Some(TargetDomain::AnyDamageTarget)
            }
            Frame::Mana { cast } | Frame::SpendMana { cast } => {
                cost.mana.is_some() && cast.spending.is_none()
            }
            Frame::Payment { cast } => {
                cost.mana.is_some()
                    && cast.spending.is_none()
                    && !self.can_pay_announced(entry, &entry.total_cost)?
            }
            Frame::ChosenObjectCost { .. } | Frame::Fault { .. } | Frame::Finished { .. } => true,
            _ => false,
        };
        if !valid {
            return Err("snapshot chosen-object cost announcement order invariant".into());
        }
        Ok(())
    }

    pub(super) fn validate_paid_chosen_cost(&self, entry: &StackInstance) -> Result<(), String> {
        let paid = &entry.chosen_payment;
        let Some(cost) = self
            .invocation_chosen_cost(entry)?
            .filter(|_| entry.paid.is_some())
        else {
            return if paid.is_empty() {
                Ok(())
            } else {
                Err("snapshot unexpected chosen-object cost payment".into())
            };
        };
        let chosen = if cost.tap_source {
            let Some((source, rest)) = paid.split_first() else {
                return Err("snapshot missing source tap payment".into());
            };
            if source.before.object != entry.source
                || source.before.characteristics.has_type(CardKind::Creature)
                    && !source.before.attack_ready
                    && !source.before.keywords.contains(&KeywordAbility::Haste)
            {
                return Err("snapshot chosen-object source tap invariant".into());
            }
            self.validate_paid_object(entry.controller, source, Tap)?;
            rest
        } else {
            paid.as_slice()
        };
        if u32::try_from(chosen.len()) != Ok(cost.count)
            || !chosen
                .windows(2)
                .all(|w| w[0].before.object.card < w[1].before.object.card)
        {
            return Err("snapshot chosen-object cost count or order invariant".into());
        }
        for object in chosen {
            if ((cost.another || cost.tap_source && cost.action == Tap)
                && object.before.object == entry.source)
                || !cost.filter.matches(&object.before.characteristics)
            {
                return Err("snapshot chosen-object filter invariant".into());
            }
            self.validate_paid_object(entry.controller, object, cost.action)?;
        }
        Ok(())
    }

    fn validate_paid_object(
        &self,
        player: PlayerId,
        paid: &PaidObject,
        action: ChosenCostAction,
    ) -> Result<(), String> {
        let id = paid.before.object;
        let live = self.state.rules.objects.get(&id.card);
        if self
            .state
            .rules
            .cards
            .get(&id.card)
            .is_none_or(|c| c.owner != paid.before.owner)
            || live.is_some_and(|o| o.id.incarnation < id.incarnation)
        {
            return Err("snapshot chosen-object payment identity invariant".into());
        }
        self.validate_characteristic_identity(id, &paid.before.characteristics)?;
        let valid = match (&paid.outcome, action) {
            (CostOutcome::Tapped { later_untapped }, Tap) => {
                paid.before.zone == Zone::Battlefield
                    && paid.before.controller == player
                    && !paid.before.tapped
                    && (live.is_none_or(|o| {
                        o.id != id || (o.zone == Zone::Battlefield && (o.tapped || *later_untapped))
                    }))
            }
            (CostOutcome::Moved(movement), Discard | ExileFromGraveyard) => {
                let (from, to) = if action == Discard {
                    (
                        Zone::Hand { owner: player },
                        Zone::Graveyard { owner: player },
                    )
                } else {
                    (Zone::Graveyard { owner: player }, Zone::Exile)
                };
                paid.before.owner == player
                    && paid.before.zone == from
                    && movement.previous == id
                    && movement.from == from
                    && movement.destination == to
                    && movement.successor.card == id.card
                    && id.incarnation.checked_add(1) == Some(movement.successor.incarnation)
                    && match live {
                        Some(o) => {
                            o.id.incarnation > movement.successor.incarnation
                                || (o.id == movement.successor && o.zone == to)
                        }
                        // A resolving ability can retain its receipt after
                        // its controller and all cards they own have left.
                        None => !self.is_live(player),
                    }
            }
            _ => false,
        };
        if !valid {
            return Err("snapshot chosen-object cost outcome invariant".into());
        }
        Ok(())
    }
}

fn valid_selection_prefix(prefix: &[ObjectId], eligible: &[ObjectId]) -> bool {
    prefix.windows(2).all(|w| w[0] < w[1])
        && prefix.iter().all(|id| eligible.binary_search(id).is_ok())
}
