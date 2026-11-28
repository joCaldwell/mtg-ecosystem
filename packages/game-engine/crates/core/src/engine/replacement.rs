//! E2 replacement before commit: static quantity replacements of token
//! creation and counter placement (CR 614.1a, 614.5, 614.6, 614.16, 616.1).
//! The proposal (the instruction's count, a permanent's entry counters, a
//! cost's loyalty) stays the original fact. Each leaf reads the live
//! replacement instances on the common prior battlefield, applies each
//! eligible instance once (CR 614.5), recomputes eligibility after every
//! application (CR 616.1f, 616.2) and commits the transformed count in the
//! same atomic group, recording the applied instances as lineage. Nothing is
//! repaired after commit.
//!
//! When the eligible effects do not commute (an addition and a
//! multiplication), the affected object's controller chooses which applies
//! next (CR 616.1, 616.1e). Simultaneous choosers act in APNAP order (CR
//! 101.4, 616.1). The rule step that reached the choice is
//! discarded unwritten; a `Frame::ReplacementOrder` asks the question and,
//! once answered, replays that same step with every answer given so far.
//! Effects of one kind commute and leave the same effects eligible afterward,
//! so they apply in instance order without a question.
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;
use mtg_contracts::QuantityModification;
use mtg_contracts::kir::{
    CounterKind, CounterRecipients, QuantityEvent, ReplacementOrigin, StaticRule,
};

/// The member a quantity replacement may modify.
#[derive(Clone, Copy, Debug)]
pub(super) enum QuantityMember<'a> {
    /// CR 122.6: counters of one kind put on one permanent, or on a
    /// permanent as it enters. `holder` is the incarnation that will hold
    /// them; `shown` is the object its controller can see while choosing.
    Counters {
        holder: ObjectId,
        controller: PlayerId,
        creature: bool,
        counter: &'a CounterKind,
        shown: Option<ObjectId>,
    },
    /// CR 111.1: the tokens one event creates under `controller`'s control.
    Tokens { controller: PlayerId },
}

/// A pending CR 616.1 choice: which of `offered` applies next to `holder`'s
/// counters of one kind, already modified by `applied`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OrderRequest {
    pub chooser: PlayerId,
    pub holder: ObjectId,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shown: Option<ObjectId>,
    pub counter: CounterKind,
    pub amount: Quantity,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub applied: Vec<ReplacementKey>,
    pub offered: Vec<ReplacementKey>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OrderAnswer {
    pub request: OrderRequest,
    pub chosen: ReplacementKey,
}

/// The suspended choice and the step it replays. `deferred` is the
/// authorized input that step had consumed; it is journaled already and is
/// handed back to the replayed step unchanged.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PendingOrder {
    /// Absent after a departure until replay derives the next live offer.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub request: Option<OrderRequest>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub answers: Vec<OrderAnswer>,
    pub resume: FrameLink,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub deferred: Option<Box<DeferredOrderInput>>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DeferredOrderInput {
    journal_index: usize,
    input: PendingInput,
}

/// The lineage of one replaced member: its original and committed counts
/// and the instances applied in order (CR 614.5, 614.16).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct QuantityApplication {
    /// The receiving incarnation; absent for a token creation.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub holder: Option<ObjectId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub counter: Option<CounterKind>,
    pub original: Quantity,
    pub amount: Quantity,
    pub applied: Vec<ReplacementKey>,
}

struct LiveRule<'a> {
    key: ReplacementKey,
    controller: PlayerId,
    event: &'a QuantityEvent,
    change: &'a QuantityModification,
}

/// The rule step that reached an unanswered choice carries it out in its
/// error, through any forks, to `Game::commit_rule_step`.
const ORDER_MARK: &str = "replacement order required: ";

pub(super) fn requested_order(error: &str) -> Option<OrderRequest> {
    let start = error.find(ORDER_MARK)? + ORDER_MARK.len();
    let mut stream = serde_json::Deserializer::from_str(&error[start..]).into_iter();
    stream.next()?.ok()
}

fn applies(rule: &LiveRule<'_>, member: &QuantityMember<'_>, produced_by_effect: bool) -> bool {
    // CR 614.16: an effect-only clause matches what a resolving effect or
    // another replacement produces.
    let origin =
        |origin: &ReplacementOrigin| *origin == ReplacementOrigin::Any || produced_by_effect;
    match (rule.event, member) {
        (QuantityEvent::CreateTokens { origin: o }, QuantityMember::Tokens { controller }) => {
            *controller == rule.controller && origin(o)
        }
        (
            QuantityEvent::PutCounters {
                counter,
                recipients,
                origin: o,
            },
            QuantityMember::Counters {
                controller,
                creature,
                counter: kind,
                ..
            },
        ) => {
            *controller == rule.controller
                && counter.as_ref().is_none_or(|c| c == *kind)
                && (*recipients == CounterRecipients::Permanents || *creature)
                && origin(o)
        }
        _ => false,
    }
}

impl RulesTxn {
    /// The functioning quantity replacements on the battlefield, in instance
    /// order (CR 604.1, 611.3a).
    fn live_quantity_rules(&self) -> Result<Vec<LiveRule<'_>>, String> {
        let mut rules = vec![];
        for holder in self.battlefield_static_sources(None) {
            let abilities = &self.copiable_definition(holder.id)?.static_abilities;
            if !abilities.iter().any(|a| {
                matches!(
                    &a.effect,
                    StaticEffect::Rule {
                        rule: StaticRule::ModifyQuantity { .. },
                        ..
                    }
                )
            }) || self.printed_abilities_lost(holder.id)?
            {
                continue;
            }
            for (index, ability) in abilities.iter().enumerate() {
                if let StaticEffect::Rule {
                    rule: StaticRule::ModifyQuantity { event, change },
                    condition: None,
                } = &ability.effect
                    && ability.functioning_zone == FunctioningZone::Battlefield
                {
                    rules.push(LiveRule {
                        key: ReplacementKey {
                            source: holder.id,
                            ability: u32::try_from(index).map_err(|_| "static ability index")?,
                        },
                        controller: holder.controller,
                        event,
                        change,
                    });
                }
            }
        }
        rules.sort_by_key(|rule| rule.key);
        Ok(rules)
    }

    pub(super) fn has_quantity_rules(&self) -> Result<bool, String> {
        Ok(!self.live_quantity_rules()?.is_empty())
    }

    /// CR 614.5, 616.1: fold every eligible instance over `original`, once
    /// each, recomputing eligibility after each application. A member
    /// produced by an effect or by another replacement (CR 614.16) is
    /// `produced_by_effect`; after any application it is.
    pub(super) fn replace_quantity(
        &self,
        member: QuantityMember<'_>,
        original: &Quantity,
        produced_by_effect: bool,
    ) -> Result<QuantityApplication, String> {
        let mut amount = original.clone();
        let mut applied: Vec<ReplacementKey> = vec![];
        if *original > Quantity::zero() {
            let rules = self.live_quantity_rules()?;
            loop {
                let effect = produced_by_effect || !applied.is_empty();
                let eligible: Vec<&LiveRule<'_>> = rules
                    .iter()
                    .filter(|rule| !applied.contains(&rule.key) && applies(rule, &member, effect))
                    .collect();
                let Some(first) = eligible.first() else {
                    break;
                };
                let chosen = if eligible
                    .iter()
                    .all(|r| r.change.commutes_with(first.change))
                {
                    first
                } else {
                    let QuantityMember::Counters {
                        holder,
                        controller,
                        counter,
                        shown,
                        ..
                    } = member
                    else {
                        return Err("token quantity replacements must commute".into());
                    };
                    let offered: Vec<ReplacementKey> = eligible.iter().map(|r| r.key).collect();
                    let request = OrderRequest {
                        chooser: controller,
                        holder,
                        shown,
                        counter: counter.clone(),
                        amount: amount.clone(),
                        applied: applied.clone(),
                        offered,
                    };
                    let key = self.replacement_answer(&request)?;
                    eligible
                        .iter()
                        .find(|r| r.key == key)
                        .ok_or("replacement answer outside its offer")?
                };
                amount = chosen.change.apply(&amount);
                applied.push(chosen.key);
            }
        }
        let (holder, counter) = match member {
            QuantityMember::Counters {
                holder, counter, ..
            } => (Some(holder), Some(counter.clone())),
            QuantityMember::Tokens { .. } => (None, None),
        };
        Ok(QuantityApplication {
            holder,
            counter,
            original: original.clone(),
            amount,
            applied,
        })
    }

    /// The recorded answer to `request`, or the error that suspends the
    /// rule step for it.
    fn replacement_answer(&self, request: &OrderRequest) -> Result<ReplacementKey, String> {
        if let Some(answer) = self
            .replacement_answers
            .iter()
            .find(|answer| answer.request == *request)
        {
            return Ok(answer.chosen);
        }
        if !self.is_live(request.chooser) || self.distinct_sources(request).is_err() {
            return Err("unanswerable replacement order".into());
        }
        Err(format!(
            "{ORDER_MARK}{}",
            serde_json::to_string(request).map_err(|e| e.to_string())?
        ))
    }

    /// The offered instances' permanents, which name them uniquely.
    fn distinct_sources(&self, request: &OrderRequest) -> Result<Vec<ObjectId>, String> {
        let sources: Vec<ObjectId> = request.offered.iter().map(|k| k.source).collect();
        if sources.len() < 2 || sources.iter().collect::<BTreeSet<_>>().len() != sources.len() {
            return Err("replacement order candidates are not distinct permanents".into());
        }
        Ok(sources)
    }

    /// The member's counts and instances, recorded for the committing leaf.
    pub(super) fn record_quantity(&mut self, application: QuantityApplication) {
        if !application.applied.is_empty() {
            self.quantity_log.push(application);
        }
    }

    /// CR 616.1: the affected object's controller picks the next instance;
    /// the object it modifies is the slot's subject when its controller can
    /// see it.
    pub(super) fn replacement_order_request(
        &self,
        order: &PendingOrder,
    ) -> Result<Option<Decision<Rules>>, String> {
        let Some(request) = &order.request else {
            return Ok(None);
        };
        let mut candidates: Vec<Candidate<Rules>> = self
            .distinct_sources(request)?
            .into_iter()
            .map(|object| Candidate::Object { object })
            .collect();
        let allowed = (0..candidates.len() as u32).collect();
        let subject = request.shown.map(|object| {
            candidates.push(Candidate::Object { object });
            candidates.len() as u32 - 1
        });
        Ok(Some(self.decision(
            request.chooser,
            Reason::ReplacementOrder {
                subject: request.shown,
            },
            candidates,
            Shape::Pick {
                slots: vec![Slot {
                    role: SlotRole::Choice,
                    subject,
                    allowed,
                    min: 1,
                    max: 1,
                    repeat: false,
                }],
                rules: vec![],
            },
        )))
    }

    fn order_answer(request: &OrderRequest, input: &PendingInput) -> Result<OrderAnswer, String> {
        if input.player != request.chooser {
            return Err("wrong replacement order chooser".into());
        }
        let Candidate::Object { object } = input.one()? else {
            return Err("invalid replacement order answer".into());
        };
        let chosen = *request
            .offered
            .iter()
            .find(|key| key.source == *object)
            .ok_or("replacement order answer outside its offer")?;
        Ok(OrderAnswer {
            request: request.clone(),
            chosen,
        })
    }

    fn replacement_resume(&mut self, link: FrameLink) -> Result<Frame, RuleError> {
        if link.0.checked_add(1) != Some(self.state.rules.frames.len()) {
            return Err("replacement order resume is not the top frame".into());
        }
        self.rules_mut()
            .frames
            .pop()
            .ok_or_else(|| "missing replacement order resume".into())
    }

    /// Replay the suspended step with the answers and input it still owns.
    pub(super) fn replacement_order_step(
        &mut self,
        mut order: PendingOrder,
    ) -> Result<(), RuleError> {
        if let Some(request) = &order.request {
            let input = self
                .rules_mut()
                .pending
                .take()
                .ok_or("missing replacement order answer")?;
            order.answers.push(Self::order_answer(request, &input)?);
        }
        let resume = self.replacement_resume(order.resume)?;
        self.rules_mut().frame = resume.clone();
        // A departure can invalidate an accepted proposal from the original
        // frame. Reissue that choice if one still exists. Proliferate's
        // already-selected group replays for just its surviving members.
        if order.request.is_none()
            && let Some(input) = &order.deferred
            && !matches!(
                resume,
                Frame::ResolutionChoice {
                    choice: ResolutionChoice::Proliferate,
                    ..
                }
            )
            && self.frame_decision()?.is_some()
            && self.validate_pending(&input.input).is_err()
        {
            return Ok(());
        }
        self.rules_mut().pending = order.deferred.map(|deferred| deferred.input);
        self.replacement_answers = order.answers.into();
        self.step_frame(resume)
    }

    /// CR 800.4a, 616.1: leaving removes objects and can end control effects.
    /// Replay the unwritten step against that battlefield before offering any
    /// further choices. Preserve answers whose remaining offer still matches.
    pub(super) fn revalidate_replacement_departure(&mut self) -> Result<(), String> {
        let Frame::ReplacementOrder { order } = &self.state.rules.frame else {
            return Ok(());
        };
        let mut order = (**order).clone();
        if let Some(request) = order.request.take()
            && let Some(input) = self.rules_mut().pending.take()
        {
            order.answers.push(Self::order_answer(&request, &input)?);
        }
        let rules = self.live_quantity_rules()?;
        let live = |key: &ReplacementKey| rules.iter().any(|rule| rule.key == *key);
        order.answers.retain_mut(|answer| {
            let request = &mut answer.request;
            request.offered.retain(live);
            self.is_live(request.chooser)
                && request
                    .shown
                    .is_none_or(|object| self.current(object).is_ok())
                && request.applied.iter().all(live)
                && request.offered.contains(&answer.chosen)
        });
        self.rules_mut().frame = Frame::ReplacementOrder {
            order: Box::new(order),
        };
        Ok(())
    }

    /// The installing half of a suspension: on the committed state, keep the
    /// step's frame and input and ask `request`.
    pub(in crate::engine) fn suspend_for_replacement_order(
        &mut self,
        request: OrderRequest,
    ) -> Result<(), RuleError> {
        if !self.state.rules.pending_concessions.is_empty() {
            return Err("replacement order during a departure".into());
        }
        let mut order = match &self.state.rules.frame {
            Frame::ReplacementOrder { order } => {
                let mut order = (**order).clone();
                if let Some(previous) = order.request.take() {
                    let input = self
                        .rules_mut()
                        .pending
                        .take()
                        .ok_or("missing replacement order answer")?;
                    order.answers.push(Self::order_answer(&previous, &input)?);
                }
                order
            }
            frame => {
                let frame = frame.clone();
                let deferred = match self.rules_mut().pending.take() {
                    Some(input) => {
                        let journal = &self.state.audit.journal;
                        let journal_index = journal
                            .len()
                            .checked_sub(1)
                            .filter(|&index| journal[index] == input.input)
                            .ok_or("replacement order input outside the journal")?;
                        Some(Box::new(DeferredOrderInput {
                            journal_index,
                            input,
                        }))
                    }
                    None => None,
                };
                PendingOrder {
                    request: None,
                    answers: vec![],
                    resume: self.save_frame(frame),
                    deferred,
                }
            }
        };
        if order.answers.iter().any(|answer| answer.request == request) {
            return Err("replacement order asked twice".into());
        }
        order.request = Some(request);
        self.rules_mut().frame = Frame::ReplacementOrder {
            order: Box::new(order),
        };
        let rules = self.rules_mut();
        rules.rule_steps = rules
            .rule_steps
            .checked_add(1)
            .ok_or_else(|| RuleError::resource("execution boundary resource limit"))?;
        Ok(())
    }

    /// A suspended order names live, distinct battlefield instances and a
    /// deferred input the journal already holds (CR 616.1).
    pub(super) fn validate_replacement_order(&self) -> Result<(), String> {
        let Frame::ReplacementOrder { order } = &self.state.rules.frame else {
            return Ok(());
        };
        if order.resume.0.checked_add(1) != Some(self.state.rules.frames.len())
            || matches!(
                self.saved_frame(order.resume)?,
                Frame::ReplacementOrder { .. }
            )
        {
            return Err("replacement order resume is not the top frame".into());
        }
        if let Some(request) = &order.request {
            let rules = self.live_quantity_rules()?;
            self.distinct_sources(request)?;
            if !self.is_live(request.chooser)
                || request.amount <= Quantity::zero()
                || request
                    .offered
                    .iter()
                    .chain(&request.applied)
                    .any(|key| !rules.iter().any(|rule| rule.key == *key))
                || request
                    .offered
                    .iter()
                    .any(|key| request.applied.contains(key))
            {
                return Err("invalid replacement order".into());
            }
        } else if self.state.rules.pending.is_some() {
            return Err("replacement replay has an unexpected answer".into());
        }
        if order
            .answers
            .iter()
            .any(|a| !a.request.offered.contains(&a.chosen))
        {
            return Err("invalid replacement order".into());
        }
        if let Some(deferred) = &order.deferred {
            // Recomputed offers can discard answers, but their journal
            // entries remain. The deferred input keeps its original ordinal.
            let journal = &self.state.audit.journal;
            let index = deferred.journal_index;
            if index >= journal.len() || journal[index] != deferred.input.input {
                return Err("replacement order input disagrees with the journal".into());
            }
        }
        Ok(())
    }
}
