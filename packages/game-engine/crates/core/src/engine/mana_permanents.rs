//! CR 106.1b/4, 302.6, 602.5a, 605.1a/3a-c, 702.10c, 733.1-2, 903.4a:
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;

// Four Healers plus eight once-only sacrifice-draw artifact activations.
// Mana activations add no stack entries (605.3b). No recursion returns artifacts.

pub(super) fn zero_mana_cost() -> ManaCost {
    ManaCost {
        generic: Quantity::zero(),
        colored: vec![],
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum ManaStage {
    Payment,
    Spend,
    Color,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct ManaActivation {
    pub(super) player: PlayerId,
    pub(super) source: ObjectId,
    ability: ManaAbility,
    decision_id: u64,
    pub(super) stage: ManaStage,
    pub(super) resume: Box<Frame>,
    receipts: Vec<ManaActivationUndo>,
    paid: Option<Box<PermanentManaUndo>>,
}
/// A receipt describes only this activation's production, spending and source
/// costs. Reverse in LIFO order so a Signet refunds its input before the land
/// receipt subtracts that mana. Existing zero entries keep their exact shape.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PermanentManaUndo {
    pub player: PlayerId,
    pub source: ObjectBefore,
    pub counters: Quantity,
    pub produced: BTreeMap<Color, Quantity>,
    pub spent: BTreeMap<Color, Quantity>,
    pub absent: BTreeSet<Color>,
    pub sacrificed: bool,
    pub cost_triggers: Vec<TriggerId>,
}
impl Game {
    pub(super) fn is_permanent_mana(&self, object: ObjectId) -> bool {
        self.definition(object.card)
            .is_ok_and(|d| !d.mana_abilities.is_empty())
    }
    pub(super) fn mana_colors(&self, player: PlayerId, production: &ManaProduction) -> Vec<Color> {
        match production {
            ManaProduction::Fixed { symbols } => symbols.clone(),
            ManaProduction::AnyColor => vec![Color::W, Color::U, Color::B, Color::R, Color::G],
            ManaProduction::CommanderColor => self
                .prepared
                .config
                .commanders
                .iter()
                .find(|id| {
                    self.prepared
                        .config
                        .cards
                        .iter()
                        .any(|c| c.id == **id && c.owner == player)
                })
                .and_then(|id| self.prepared.config.cards.iter().find(|c| c.id == *id))
                .and_then(|c| {
                    self.prepared
                        .catalog
                        .definitions
                        .iter()
                        .find(|d| d.oracle_id == c.oracle_id)
                })
                .map(|d| d.characteristics.color_identity.clone())
                .unwrap_or_default(),
        }
    }
    pub(super) fn underlying_mana_frame<'a>(&self, frame: &'a Frame) -> &'a Frame {
        let mut f = frame;
        while let Frame::ManaAbility { activation } = f {
            f = &activation.resume;
        }
        f
    }
    pub(super) fn reserved_mana_source(&self, source: ObjectId) -> bool {
        let mut f = &self.state.frame;
        while let Frame::ManaAbility { activation } = f {
            if activation.source == source {
                return true;
            }
            f = &activation.resume;
        }
        // Mind Stone cannot tap itself for mana while paying its own tap cost.
        if let Frame::Mana { cast } | Frame::SpendMana { cast } | Frame::Payment { cast } = f
            && cast.origin == AnnouncementOrigin::Activated
            && let Ok(top) = self.top()
            && top.source == source
            && let StackKind::Activated { ability, .. } = top.kind
            && self.definition(source.card).is_ok_and(|d| {
                d.activated.get(ability as usize).is_some_and(|a| {
                    matches!(&a.cost,
                ActivationCost::Composite { costs } if costs.tap || costs.sacrifice)
                })
            })
        {
            return true;
        }
        false
    }
    pub(super) fn validate_permanent_mana_activation(
        &self,
        player: PlayerId,
        source: ObjectId,
    ) -> Result<(), String> {
        let o = self.current(source)?;
        let d = self.definition(source.card)?;
        if self.reserved_mana_source(source)
            || d.mana_abilities.len() != 1
            || o.zone != Zone::Battlefield
            || o.controller != player
            || !self.is_live(player)
            || o.tapped
            || (d.characteristics.kind == CardKind::Creature
                && !o.attack_ready
                && !self.has_keyword(source, KeywordAbility::Haste))
        {
            return Err("invalid permanent mana source or tap eligibility".into());
        }
        Ok(())
    }
    pub(super) fn begin_permanent_mana(
        &mut self,
        input: &AcceptedInput,
        source: ObjectId,
        resume: Frame,
    ) -> Result<(), String> {
        self.validate_permanent_mana_activation(input.player, source)?;
        let ability = self.definition(source.card)?.mana_abilities[0].clone();
        let activation = ManaActivation {
            player: input.player,
            source,
            decision_id: input.decision_id,
            ability,
            stage: ManaStage::Payment,
            resume: Box::new(resume),
            receipts: vec![],
            paid: None,
        };
        if activation.ability.costs.mana.is_none() {
            self.pay_mana_ability(activation, BTreeMap::new())
        } else {
            self.state.frame = Frame::ManaAbility {
                activation: Box::new(activation),
            };
            Ok(())
        }
    }
    pub(super) fn permanent_mana_request(&self, a: &ManaActivation) -> Result<Request, String> {
        Ok(match a.stage {
            ManaStage::Payment => Request::Mana {
                decision_id: self.state.revision,
                player: a.player,
                cost: a
                    .ability
                    .costs
                    .mana
                    .clone()
                    .ok_or("missing mana ability cost")?,
            },
            ManaStage::Spend => Request::SpendMana {
                decision_id: self.state.revision,
                player: a.player,
                cost: a
                    .ability
                    .costs
                    .mana
                    .clone()
                    .ok_or("missing mana ability cost")?,
                available: self.state.players[a.player.0 as usize].mana.clone(),
            },
            ManaStage::Color => Request::ManaColor {
                decision_id: self.state.revision,
                player: a.player,
                colors: self.mana_colors(a.player, &a.ability.production),
            },
        })
    }
    pub(super) fn validate_permanent_mana_response(
        &self,
        a: &ManaActivation,
        player: PlayerId,
        response: &Response,
    ) -> Result<(), String> {
        if a.player != player || !self.is_live(player) {
            return Err("wrong mana ability payer".into());
        }
        match (&a.stage, response) {
            (ManaStage::Payment, Response::ActivateMana { object }) => {
                self.validate_mana_activation(player, *object)
            }
            (ManaStage::Payment, Response::FinishMana { .. }) => Ok(()),
            (ManaStage::Spend, Response::SpendMana { spending }) => self
                .selected_payment_pool(
                    player,
                    a.ability.costs.mana.as_ref().ok_or("missing nested cost")?,
                    spending,
                )
                .map(|_| ()),
            (ManaStage::Color, Response::ManaColor { color })
                if self
                    .mana_colors(player, &a.ability.production)
                    .contains(color) =>
            {
                Ok(())
            }
            _ => Err("invalid mana ability response".into()),
        }
    }
    pub(super) fn permanent_mana_step(&mut self, mut a: ManaActivation) -> Result<(), String> {
        let input = self
            .state
            .pending
            .take()
            .ok_or("missing mana ability input")?;
        match input.response {
            Response::ActivateMana { object } if a.stage == ManaStage::Payment => {
                if self.is_permanent_mana(object) {
                    self.begin_permanent_mana(
                        &input,
                        object,
                        Frame::ManaAbility {
                            activation: Box::new(a),
                        },
                    )?;
                } else {
                    a.receipts
                        .push(self.activate_intrinsic_mana(&input, object)?);
                    self.state.frame = Frame::ManaAbility {
                        activation: Box::new(a),
                    };
                }
            }
            Response::FinishMana { undo_mana } if a.stage == ManaStage::Payment => {
                let cost = a
                    .ability
                    .costs
                    .mana
                    .as_ref()
                    .ok_or("missing nested mana cost")?;
                if self.payment_pool(a.player, cost).is_ok() {
                    a.stage = ManaStage::Spend;
                    self.state.frame = Frame::ManaAbility {
                        activation: Box::new(a),
                    };
                } else {
                    if undo_mana {
                        self.undo_mana_receipts(a.player, &a.receipts)?;
                        a.receipts.clear();
                    }
                    self.resume_mana(*a.resume, a.receipts)?;
                }
            }
            Response::SpendMana { spending } if a.stage == ManaStage::Spend => {
                self.pay_mana_ability(a, spending)?
            }
            Response::ManaColor { color } if a.stage == ManaStage::Color => {
                self.resolve_mana_ability(a, Some(color))?
            }
            _ => return Err("nested mana frame/input invariant".into()),
        }
        Ok(())
    }
    fn pay_mana_ability(
        &mut self,
        mut a: ManaActivation,
        spending: BTreeMap<Color, Quantity>,
    ) -> Result<(), String> {
        // No costs are paid until the nested payment succeeds (601.2g-h).
        let pool = &self.state.players[a.player.0 as usize].mana;
        let mut receipt = PermanentManaUndo {
            player: a.player,
            source: self.object_before(a.source)?,
            counters: self.current(a.source)?.plus_one_counters.clone(),
            produced: BTreeMap::new(),
            spent: spending.clone(),
            absent: [Color::W, Color::U, Color::B, Color::R, Color::G, Color::C]
                .into_iter()
                .filter(|c| !pool.contains_key(c))
                .collect(),
            sacrificed: a.ability.costs.sacrifice,
            cost_triggers: vec![],
        };
        let cause = BatchCause::ManaActivation {
            decision_id: a.decision_id,
            player: a.player,
            source: a.source,
        };
        if let Some(cost) = &a.ability.costs.mana {
            self.commit(
                ProposedEvent::PayManaSelected {
                    player: a.player,
                    cost: cost.clone(),
                    spending,
                },
                cause.clone(),
            )?;
        }
        let first_trigger = self.state.next_trigger_id;
        self.pay_permanent_source_costs(a.source, a.player, &a.ability.costs, cause)?;
        receipt.cost_triggers = self
            .state
            .pending_triggers
            .iter()
            .filter(|t| t.id.0 >= first_trigger)
            .map(|t| t.id)
            .collect();
        a.paid = Some(Box::new(receipt));
        match a.ability.production {
            ManaProduction::Fixed { .. } => self.resolve_mana_ability(a, None),
            _ if self.mana_colors(a.player, &a.ability.production).is_empty() => {
                self.resolve_mana_ability(a, None)
            }
            _ => {
                a.stage = ManaStage::Color;
                self.state.frame = Frame::ManaAbility {
                    activation: Box::new(a),
                };
                Ok(())
            }
        }
    }
    pub(super) fn pay_permanent_source_costs(
        &mut self,
        source: ObjectId,
        player: PlayerId,
        costs: &PermanentActivationCosts,
        cause: BatchCause,
    ) -> Result<(), String> {
        let o = self.current(source)?;
        if o.zone != Zone::Battlefield || o.controller != player || (costs.tap && o.tapped) {
            return Err("permanent cost source changed".into());
        }
        let mut children = vec![];
        if costs.tap {
            children.push(EventGroup::Occurrence {
                proposed: ProposedEvent::Tap { object: source },
            });
        }
        if costs.sacrifice {
            children.push(EventGroup::Occurrence {
                proposed: ProposedEvent::Sacrifice {
                    object: source,
                    player,
                },
            });
        }
        if !children.is_empty() {
            self.commit_batch(cause, EventGroup::Ordered { children })?;
        }
        Ok(())
    }
    fn resolve_mana_ability(
        &mut self,
        mut a: ManaActivation,
        color: Option<Color>,
    ) -> Result<(), String> {
        let symbols = match &a.ability.production {
            ManaProduction::Fixed { symbols } => symbols.clone(),
            _ => color.into_iter().collect(),
        };
        let mut receipt = a.paid.take().ok_or("unpaid mana ability")?;
        // A sacrifice cost changes the source incarnation, but the frozen
        // ability still resolves (113.7a); AddMana uses that current identity.
        let source = self.object_id(a.source.card).unwrap_or(a.source);
        let mut children = vec![];
        for color in symbols {
            receipt
                .produced
                .entry(color)
                .or_default()
                .add(&Quantity::from(1));
            children.push(EventGroup::Occurrence {
                proposed: ProposedEvent::AddMana {
                    player: a.player,
                    color,
                    amount: Quantity::from(1),
                    source,
                },
            });
        }
        if !children.is_empty() {
            self.commit_batch(
                BatchCause::ManaActivation {
                    decision_id: a.decision_id,
                    player: a.player,
                    source: a.source,
                },
                EventGroup::Ordered { children },
            )?;
        }
        a.receipts.push(ManaActivationUndo {
            object: a.source,
            player: a.player,
            color: Color::C,
            amount: Quantity::zero(),
            mana_was_absent: false,
            permanent: Some(receipt),
        });
        self.resume_mana(*a.resume, a.receipts)
    }
    fn resume_mana(
        &mut self,
        mut resume: Frame,
        receipts: Vec<ManaActivationUndo>,
    ) -> Result<(), String> {
        let retained = !receipts.is_empty();
        match &mut resume {
            Frame::ManaAbility { activation } => activation.receipts.extend(receipts),
            Frame::Mana { cast } => cast.mana_activations.extend(receipts),
            Frame::Priority => {
                if retained {
                    self.state.passes.clear();
                }
            }
            _ => return Err("invalid mana continuation".into()),
        }
        self.state.frame = resume;
        if matches!(self.state.frame, Frame::Priority) {
            self.stabilize()?;
        }
        Ok(())
    }
    fn undo_mana_receipts(
        &mut self,
        player: PlayerId,
        receipts: &[ManaActivationUndo],
    ) -> Result<(), String> {
        for r in receipts.iter().rev() {
            let proposed = if let Some(p) = &r.permanent {
                ProposedEvent::ReversePermanentMana { receipt: p.clone() }
            } else {
                ProposedEvent::ReverseMana {
                    player: r.player,
                    color: r.color,
                    amount: r.amount.clone(),
                    source: self.current(r.object).ok().map(|o| o.id),
                    remove_zero: r.mana_was_absent,
                }
            };
            self.commit(
                proposed,
                BatchCause::ManaActivation {
                    decision_id: self.state.revision,
                    player,
                    source: r.object,
                },
            )?;
        }
        Ok(())
    }
    pub(super) fn reverse_permanent_mana(
        &mut self,
        r: &PermanentManaUndo,
    ) -> Result<EventOutcome, String> {
        if !self.is_live(r.player) {
            return Err("departed mana reversal payer".into());
        }
        let pool = &mut self.state.players[r.player.0 as usize].mana;
        for (c, n) in &r.produced {
            let v = pool.get_mut(c).ok_or("missing produced mana")?;
            if *v < *n {
                return Err("produced mana already spent".into());
            }
            v.subtract(n);
        }
        for (c, n) in &r.spent {
            pool.entry(*c).or_default().add(n);
        }
        for c in &r.absent {
            if pool.get(c) == Some(&Quantity::zero()) {
                pool.remove(c);
            }
        }
        if self.is_live(r.source.owner) {
            if r.sacrificed {
                let now = self
                    .state
                    .objects
                    .get(&r.source.object.card)
                    .ok_or("missing sacrificed source")?;
                if now.id.incarnation
                    != r.source
                        .object
                        .incarnation
                        .checked_add(1)
                        .ok_or("incarnation overflow")?
                    || now.zone
                        != (Zone::Graveyard {
                            owner: r.source.owner,
                        })
                {
                    return Err("sacrificed mana source changed".into());
                }
                self.state.objects.insert(
                    r.source.object.card,
                    Object {
                        attached_to: None,
                        id: r.source.object,
                        controller: r.source.controller,
                        default_controller: r.source.default_controller,
                        zone: r.source.zone,
                        tapped: r.source.tapped,
                        attack_ready: r.source.attack_ready,
                        marked_damage: r.source.marked_damage.clone(),
                        deathtouch_damage: r.source.deathtouch_damage,
                        plus_one_counters: r.counters.clone(),
                    },
                );
            } else {
                let o = self
                    .state
                    .objects
                    .get_mut(&r.source.object.card)
                    .ok_or("missing surviving mana source")?;
                if o.id != r.source.object || o.zone != Zone::Battlefield || !o.tapped {
                    return Err("mana reversal source changed".into());
                }
                o.tapped = r.source.tapped;
            }
        }
        self.state
            .pending_triggers
            .retain(|t| !r.cost_triggers.contains(&t.id));
        Ok(EventOutcome::PermanentManaReversed {
            player: r.player,
            source: r.source.object,
        })
    }
    pub(super) fn revalidate_mana_departure(&mut self) -> Result<(), String> {
        let mut f = &self.state.frame;
        let mut invalid = false;
        while let Frame::ManaAbility { activation: a } = f {
            invalid |= !self.is_live(a.player)
                || (a.paid.is_none()
                    && self.current(a.source).map_or(true, |o| {
                        o.zone != Zone::Battlefield
                            || o.controller != a.player
                            || o.tapped
                            || (self
                                .characteristics(a.source)
                                .is_ok_and(|c| c.kind == CardKind::Creature)
                                && !o.attack_ready
                                && !self.has_keyword(a.source, KeywordAbility::Haste))
                    }));
            f = &a.resume;
        }
        if let Frame::Mana { cast } = f {
            invalid |= self.state.stack.last().is_none_or(|top| {
                top.id != cast.stack_id || self.announced_target_legality(top).iter().any(|v| !*v)
            });
        }
        if invalid {
            while let Frame::ManaAbility { activation } = self.state.frame.clone() {
                let mut a = *activation;
                if let Some(receipt) = a.paid.take() {
                    a.receipts.push(ManaActivationUndo {
                        object: a.source,
                        player: a.player,
                        color: Color::C,
                        amount: Quantity::zero(),
                        mana_was_absent: false,
                        permanent: Some(receipt),
                    });
                }
                if self.is_live(a.player) {
                    self.undo_mana_receipts(a.player, &a.receipts)?;
                } else {
                    match a.resume.as_mut() {
                        Frame::Mana { cast } => cast.mana_activations.append(&mut a.receipts),
                        Frame::ManaAbility { activation } => {
                            activation.receipts.append(&mut a.receipts)
                        }
                        Frame::Priority if !a.receipts.is_empty() => self.state.passes.clear(),
                        _ => {}
                    }
                }
                self.state.frame = *a.resume;
            }
            self.state.pending = None;
        }
        Ok(())
    }
    pub(super) fn validate_nested_mana_frame(&self, frame: &Frame) -> Result<(), String> {
        let mut f = frame;
        let mut sources = BTreeSet::new();
        while let Frame::ManaAbility { activation: a } = f {
            if !self.is_live(a.player)
                || !sources.insert(a.source)
                || self.definition(a.source.card)?.mana_abilities.first() != Some(&a.ability)
                || (a.stage == ManaStage::Color) != a.paid.is_some()
                || a.receipts.iter().any(|r| r.player != a.player)
            {
                return Err("nested mana snapshot invariant".into());
            }
            f = &a.resume;
        }
        Ok(())
    }
}
