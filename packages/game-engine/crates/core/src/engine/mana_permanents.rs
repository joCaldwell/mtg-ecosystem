//! CR 106.1b/4, 106.7, 119.4, 302.6, 602.5a, 605.1a/3a-c, 702.10c, 733.1-2,
//! 903.4a:
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;
use mtg_contracts::kir::{Node, Verb};

fn is_zero(n: &u32) -> bool {
    *n == 0
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ManaSourceScope {
    Intrinsic,
    Mixed,
}

// Four Healers plus eight once-only sacrifice-draw artifact activations.
// Mana activations add no stack entries (605.3b). No recursion returns artifacts.

pub fn zero_mana_cost() -> ManaCost {
    ManaCost {
        generic: Quantity::zero(),
        colored: vec![],
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ManaStage {
    Payment,
    Spend,
    Color,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ManaActivation {
    pub(super) player: PlayerId,
    pub source: ObjectId,
    /// CR 605.3a: which of the source's printed mana abilities this is.
    #[serde(default, skip_serializing_if = "is_zero")]
    index: u32,
    ability: ManaAbility,
    /// CR 608.2c: the next rider to perform once the mana is added.
    #[serde(default, skip_serializing_if = "is_zero")]
    rider: u32,
    decision_id: u64,
    pub stage: ManaStage,
    pub(super) resume: FrameLink,
    receipts: Vec<ManaActivationUndo>,
    paid: Option<Box<PermanentManaUndo>>,
}
impl ManaActivation {
    fn payment_cost(&self) -> Option<ManaCost> {
        self.ability.costs.mana.clone()
    }

    /// The activation is the source's printed mana ability `index`.
    fn matches_definition(&self, definition: &Definition) -> bool {
        definition.mana_abilities.get(self.index as usize) == Some(&self.ability)
            && self.rider as usize <= self.ability.riders.len()
    }

    /// CR 733.1: a player may reverse a legal mana ability, so reversal is
    /// offered only for an ability whose whole effect a receipt restores.
    /// Life payments and riders (damage, life gain) are kept.
    fn reversible(&self) -> bool {
        self.ability.riders.is_empty() && self.ability.costs.pay_life.is_none()
    }

    fn cause(&self) -> BatchCause {
        BatchCause::ManaActivation {
            decision_id: self.decision_id,
            player: self.player,
            source: self.source,
        }
    }
}

/// CR 602.5b: restriction belongs to the ability on this incarnation, across
/// control changes, during each player's turn. Index zero reserves the eventual
/// selector without expanding the singleton public action.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ManaAbilityKind {
    Mana,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ManaAbilityUse {
    pub source: ObjectId,
    /// CR 712.18: the face whose ability was used; a transformed permanent's
    /// other face has different abilities.
    #[serde(default, skip_serializing_if = "CardFace::is_front")]
    pub face: CardFace,
    pub kind: ManaAbilityKind,
    pub index: u32,
    pub turn: u64,
}
testkit_fields! {
    /// A receipt describes only this activation's production, spending and source
    /// costs. Reverse in LIFO order so a Signet refunds its input before the land
    /// receipt subtracts that mana. Existing zero entries keep their exact shape.
    #[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
    #[serde(deny_unknown_fields)]
    pub struct PermanentManaUndo {
        pub player: PlayerId,
        pub source: ObjectBefore,
        pub counters: Quantity,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub ability_use: Option<ManaAbilityUse>,
        #[serde(default, skip_serializing_if = "std::ops::Not::not")]
        pub previously_used: bool,
        pub produced: BTreeMap<Color, Quantity>,
        pub spent: BTreeMap<Color, Quantity>,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        pub spent_restricted: Vec<RestrictedMana>,
        pub absent: BTreeSet<Color>,
        pub sacrificed: bool,
        pub cost_triggers: Vec<TriggerId>,
        /// CR 733.1: the activation paid life or performed riders, so it is
        /// kept rather than reversed.
        #[serde(default, skip_serializing_if = "std::ops::Not::not")]
        pub kept: bool,
    }
}

/// CR 733.1: whether every receipt can be reversed. A set with a kept
/// activation is kept whole, which the bounded keep-all/undo-all choice
/// allows: no reversed ability's mana was spent on a kept one.
pub(super) fn reversible_receipts(receipts: &[ManaActivationUndo]) -> bool {
    receipts
        .iter()
        .all(|r| r.permanent.as_ref().is_none_or(|p| !p.kept))
}

impl RulesTxn {
    pub(super) fn mana_use_key(&self, source: ObjectId, index: u32) -> ManaAbilityUse {
        ManaAbilityUse {
            source,
            face: self.current(source).map_or(source.face, Object::face),
            kind: ManaAbilityKind::Mana,
            index,
            turn: self.state.rules.turn.number,
        }
    }
    /// Whether the source's first printed mana ability was used this turn.
    pub(crate) fn mana_ability_used(&self, source: ObjectId) -> bool {
        self.mana_ability_index_used(source, 0)
    }
    fn mana_ability_index_used(&self, source: ObjectId, index: u32) -> bool {
        self.state
            .rules
            .mana_ability_uses
            .contains(&self.mana_use_key(source, index))
    }
    pub(super) fn validate_mana_uses(&self) -> Result<(), String> {
        if self.state.rules.mana_ability_uses.len() > self.state.rules.objects.len() {
            return Err("mana use capacity invariant".into());
        }
        for k in &self.state.rules.mana_ability_uses {
            if k.turn != self.state.rules.turn.number
                || self.current(k.source)?.zone != Zone::Battlefield
                || self
                    .face_definition(k.source.card, k.face)?
                    .mana_abilities
                    .get(k.index as usize)
                    .is_none_or(|a| a.restriction.is_none())
            {
                return Err("mana ability use invariant".into());
            }
        }
        Ok(())
    }

    pub(super) fn is_permanent_mana(&self, object: ObjectId) -> bool {
        self.copiable_definition(object)
            .is_ok_and(|d| !d.mana_abilities.is_empty())
    }
    pub(super) fn mana_colors(
        &self,
        player: PlayerId,
        production: &ManaProduction,
    ) -> Result<Vec<Color>, String> {
        Ok(match production {
            ManaProduction::Fixed { symbols } => symbols.clone(),
            ManaProduction::AnyColor => vec![Color::W, Color::U, Color::B, Color::R, Color::G],
            ManaProduction::Choose { colors, .. } => colors.clone(),
            ManaProduction::LandsCouldProduce {
                controller,
                colors_only,
            } => self
                .lands_could_produce(player, *controller, *colors_only)?
                .into_iter()
                .collect(),
            // CR 702.124c, 903.4: both designations contribute their
            // original color identities, independently of zone or control.
            ManaProduction::CommanderColor => {
                let mut commanders = self
                    .state
                    .rules
                    .commanders
                    .records
                    .values()
                    .filter(|history| history.owner == player)
                    .peekable();
                match commanders.next() {
                    None => Vec::new(),
                    Some(history) if commanders.peek().is_none() => history.color_identity.clone(),
                    Some(history) => std::iter::once(history)
                        .chain(commanders)
                        .flat_map(|owner| owner.color_identity.iter().copied())
                        .collect::<BTreeSet<_>>()
                        .into_iter()
                        .collect(),
                }
            }
        })
    }
    /// CR 106.7: the types of mana the lands `controller` names, relative
    /// to `player`, could produce: every type an ability of theirs would
    /// produce if it resolved now, ignoring costs.
    fn lands_could_produce(
        &self,
        player: PlayerId,
        controller: ManaLandController,
        colors_only: bool,
    ) -> Result<BTreeSet<Color>, String> {
        let table = self.could_produce_table()?;
        let mut colors: BTreeSet<Color> = table
            .iter()
            .filter(|(land, _)| self.names_land(player, controller, land.1))
            .flat_map(|(_, types)| types.iter().copied())
            .collect();
        if colors_only {
            colors.remove(&Color::C);
        }
        Ok(colors)
    }

    /// Whether a land controlled by `land_controller` is one of the lands
    /// `controller` names relative to `player` (CR 109.5, 102.3).
    fn names_land(
        &self,
        player: PlayerId,
        controller: ManaLandController,
        land_controller: PlayerId,
    ) -> bool {
        match controller {
            ManaLandController::You => land_controller == player,
            ManaLandController::Opponents => {
                land_controller != player && self.is_live(land_controller)
            }
        }
    }

    /// CR 106.7: the types each battlefield land could produce, keyed by
    /// (land, controller). Abilities with a defined production seed the
    /// table; a "could produce" ability then adds the types of the lands it
    /// names, relative to its own land's controller, until nothing changes.
    /// This least fixed point is independent of object order, and a cycle
    /// of "could produce" lands adds no type that none of them defines.
    fn could_produce_table(
        &self,
    ) -> Result<BTreeMap<(ObjectId, PlayerId), BTreeSet<Color>>, String> {
        let mut table = BTreeMap::new();
        let mut reads = vec![];
        for o in self.state.rules.objects.values() {
            if !o.present_on_battlefield() || !self.characteristics(o.id)?.has_type(CardKind::Land)
            {
                continue;
            }
            let key = (o.id, o.controller);
            let mut types: BTreeSet<Color> =
                self.intrinsic_mana_colors(o.id)?.into_iter().collect();
            for ability in &self.copiable_definition(o.id)?.mana_abilities {
                match &ability.production {
                    ManaProduction::LandsCouldProduce {
                        controller,
                        colors_only,
                    } => reads.push((key, *controller, *colors_only)),
                    production => types.extend(self.mana_colors(o.controller, production)?),
                }
            }
            table.insert(key, types);
        }
        loop {
            let mut changed = false;
            for &(key, controller, colors_only) in &reads {
                let mut found: BTreeSet<Color> = table
                    .iter()
                    .filter(|(land, _)| self.names_land(key.1, controller, land.1))
                    .flat_map(|(_, types)| types.iter().copied())
                    .collect();
                if colors_only {
                    found.remove(&Color::C);
                }
                let types = table.get_mut(&key).ok_or("missing could-produce land")?;
                let before = types.len();
                types.extend(found);
                changed |= types.len() != before;
            }
            if !changed {
                return Ok(table);
            }
        }
    }

    pub(crate) fn mana_continuation_charge(&self) -> Result<usize, String> {
        let mut frame = &self.state.rules.frame;
        let mut charge = 0usize;
        loop {
            let activation = match frame {
                Frame::ManaSourceChoice { resume, .. } => {
                    charge = charge.saturating_add(stored_history::payload_charge(frame));
                    frame = self.saved_frame(*resume)?;
                    continue;
                }
                Frame::ManaAbility { activation } => activation,
                // Payment owns the resolution and its nested mana receipts.
                // kernel_charge already accounts for every saved Kernel task.
                Frame::ResolutionPayment { .. } => {
                    return Ok(self
                        .state
                        .rules
                        .frames
                        .iter()
                        .filter(|parent| !matches!(parent, Frame::Kernel { .. }))
                        .fold(
                            stored_history::payload_charge(&self.state.rules.frame),
                            |charge, parent| {
                                charge.saturating_add(stored_history::payload_charge(parent))
                            },
                        ));
                }
                _ => break,
            };
            frame = self.saved_frame(activation.resume)?;
        }
        Ok(charge)
    }

    pub(super) fn underlying_mana_frame<'a>(
        &'a self,
        frame: &'a Frame,
    ) -> Result<&'a Frame, String> {
        let mut f = frame;
        loop {
            f = match f {
                Frame::GraveyardGroup { tx } => match &tx.finish {
                    GroupFinish::Payment { resume } => self.saved_frame(*resume)?,
                    _ => break,
                },
                Frame::ManaAbility { activation } => self.saved_frame(activation.resume)?,
                // A mana rider's damage transaction suspends its activation.
                Frame::DamageEffects { transaction } => match &transaction.continuation {
                    DamageContinuation::Mana { activation } => {
                        self.saved_frame(activation.resume)?
                    }
                    DamageContinuation::Combat | DamageContinuation::Instruction { .. } => break,
                },
                Frame::ManaSourceChoice { resume, .. } => self.saved_frame(*resume)?,
                _ => break,
            };
        }
        Ok(f)
    }
    pub(super) fn reserved_mana_source(&self, source: ObjectId) -> Result<bool, String> {
        let mut f = &self.state.rules.frame;
        if let Frame::ManaSourceChoice {
            source: selected,
            resume,
            ..
        } = f
        {
            if *selected == source {
                return Ok(true);
            }
            f = self.saved_frame(*resume)?;
        }
        while let Frame::ManaAbility { activation } = f {
            if activation.source == source {
                return Ok(true);
            }
            f = self.saved_frame(activation.resume)?;
        }
        // Mind Stone cannot tap itself for mana while paying its own tap cost.
        if let Frame::Mana { cast } | Frame::SpendMana { cast } | Frame::Payment { cast } = f
            && cast.origin == AnnouncementOrigin::Activated
            && let Ok(top) = self.top()
            && top.source == source
            && let StackKind::Activated { ability, .. } = top.kind
            && self.copiable_definition(source).is_ok_and(|d| {
                d.activated.get(ability as usize).is_some_and(|a| {
                    matches!(&a.cost,
                        ActivationCost::Composite { costs } if costs.tap || costs.sacrifice)
                        || matches!(&a.cost, ActivationCost::ChooseObjects(cost) if cost.tap_source)
                })
            })
        {
            return Ok(true);
        }
        Ok(false)
    }
    pub(crate) fn validate_permanent_mana_activation(
        &self,
        player: PlayerId,
        source: ObjectId,
    ) -> Result<(), String> {
        if self.reserved_mana_source(source)? {
            return Err("reserved permanent mana source".into());
        }
        if self.mana_ability_selection(source)? {
            return self.mixed_mana_choices(player, source).map(|_| ());
        }
        self.validate_permanent_mana_costs(player, source, 0)
    }

    /// CR 605.3a: the player chooses which mana ability of the source to
    /// activate when it has an intrinsic one (CR 305.6) or several printed.
    fn mana_ability_selection(&self, source: ObjectId) -> Result<bool, String> {
        let printed = self.copiable_definition(source)?.mana_abilities.len();
        if printed == 0 {
            return Err("unsupported permanent mana source".into());
        }
        Ok(printed > 1 || !self.intrinsic_mana_colors(source)?.is_empty())
    }

    fn validate_permanent_mana_costs(
        &self,
        player: PlayerId,
        source: ObjectId,
        index: u32,
    ) -> Result<(), String> {
        let o = self.current(source)?;
        let d = self.copiable_definition(source)?;
        let a = d
            .mana_abilities
            .get(index as usize)
            .ok_or("missing mana ability")?;
        if !o.present_on_battlefield()
            || o.controller != player
            || !self.is_live(player)
            || (a.costs.tap && o.tapped)
            || a.costs
                .pay_life
                .as_ref()
                .is_some_and(|life| !self.state.rules.players[player.0 as usize].can_pay_life(life))
            || a.costs
                .remove_source_plus_one_counters
                .as_ref()
                .is_some_and(|n| o.plus_one_counters < *n)
            || (a.restriction.is_some() && self.mana_ability_index_used(source, index))
            || (a.costs.tap
                && self.target_is_creature(source)
                && !o.attack_ready
                && !self.has_keyword(source, KeywordAbility::Haste))
        {
            return Err("invalid permanent mana source or tap eligibility".into());
        }
        Ok(())
    }

    pub(crate) fn mixed_mana_choices(
        &self,
        player: PlayerId,
        source: ObjectId,
    ) -> Result<Vec<ManaAbilityIdentity>, String> {
        let definition = self.copiable_definition(source)?;
        let mut choices: Vec<_> = self
            .intrinsic_choice_colors(player, source)
            .unwrap_or_default()
            .into_iter()
            .map(|color| ManaAbilityIdentity::Intrinsic { color })
            .collect();
        for index in 0..definition.mana_abilities.len() {
            let index = u32::try_from(index).map_err(|_| "mana ability index overflow")?;
            if self
                .validate_permanent_mana_costs(player, source, index)
                .is_ok()
            {
                choices.push(ManaAbilityIdentity::Printed { index });
            }
        }
        if choices.is_empty() {
            return Err("no payable mixed mana ability".into());
        }
        Ok(choices)
    }
    pub(super) fn mana_source_colors(
        &self,
        player: PlayerId,
        source: ObjectId,
        scope: ManaSourceScope,
    ) -> Result<Vec<Color>, String> {
        match scope {
            ManaSourceScope::Mixed => Err("mixed provider requires an ability identity".into()),
            ManaSourceScope::Intrinsic if self.is_permanent_mana(source) => {
                Err("printed provider cannot select an intrinsic-only scope".into())
            }
            ManaSourceScope::Intrinsic => self.intrinsic_choice_colors(player, source),
        }
    }

    pub(super) fn mana_source_choice_step(
        &mut self,
        player: PlayerId,
        source: ObjectId,
        scope: ManaSourceScope,
        resume: Frame,
    ) -> Result<(), String> {
        let input = self
            .rules_mut()
            .pending
            .take()
            .ok_or("missing mana source choice input")?;
        if scope == ManaSourceScope::Mixed {
            let Candidate::ManaAbility { ability } = *input.one()? else {
                return Err("expected mana ability identity".into());
            };
            if input.player != player
                || !self.mana_resume_player(&resume, player)
                || !self.mixed_mana_choices(player, source)?.contains(&ability)
            {
                return Err("invalid selected mana ability".into());
            }
            return match ability {
                ManaAbilityIdentity::Intrinsic { color } => {
                    let receipt = self.activate_selected_intrinsic_mana(&input, source, color)?;
                    self.resume_mana(resume, vec![receipt])
                }
                ManaAbilityIdentity::Printed { index } => {
                    self.start_mana_ability(&input, source, index, resume)
                }
            };
        }
        let Candidate::Color { color } = *input.one()? else {
            return Err("mana source choice input invariant".into());
        };
        if input.player != player || !self.mana_resume_player(&resume, player) {
            return Err("invalid mana source choice".into());
        }
        if !self
            .mana_source_colors(player, source, scope)?
            .contains(&color)
        {
            return Err("invalid mana source choice".into());
        }
        let receipt = self.activate_selected_intrinsic_mana(&input, source, color)?;
        self.resume_mana(resume, vec![receipt])
    }

    pub(super) fn begin_payment_mana(
        &mut self,
        input: &AcceptedInput,
        source: ObjectId,
        resume: Frame,
    ) -> Result<(), String> {
        if self.is_permanent_mana(source) {
            self.begin_permanent_mana(input, source, resume)
        } else {
            self.begin_intrinsic_mana(input, source, resume)
        }
    }

    pub(super) fn begin_permanent_mana(
        &mut self,
        input: &AcceptedInput,
        source: ObjectId,
        resume: Frame,
    ) -> Result<(), String> {
        self.validate_permanent_mana_activation(input.player, source)?;
        if !self.mana_resume_player(&resume, input.player) {
            return Err("invalid permanent mana continuation".into());
        }
        if self.mana_ability_selection(source)? {
            self.rules_mut().frame = Frame::ManaSourceChoice {
                player: input.player,
                source,
                scope: ManaSourceScope::Mixed,
                resume: self.save_frame(resume),
            };
            Ok(())
        } else {
            self.start_mana_ability(input, source, 0, resume)
        }
    }

    fn start_mana_ability(
        &mut self,
        input: &AcceptedInput,
        source: ObjectId,
        index: u32,
        resume: Frame,
    ) -> Result<(), String> {
        let ability = self
            .copiable_definition(source)?
            .mana_abilities
            .get(index as usize)
            .ok_or("missing selected mana ability")?
            .clone();
        let activation = ManaActivation {
            player: input.player,
            source,
            index,
            rider: 0,
            decision_id: input.decision_id,
            ability,
            stage: ManaStage::Payment,
            resume: self.save_frame(resume),
            receipts: vec![],
            paid: None,
        };
        if activation.payment_cost().is_none() {
            self.pay_mana_ability(activation, BTreeMap::new())
        } else {
            self.rules_mut().frame = Frame::ManaAbility {
                activation: Box::new(activation),
            };
            Ok(())
        }
    }
    pub(super) fn permanent_mana_request(
        &self,
        a: &ManaActivation,
    ) -> Result<Decision<Rules>, String> {
        match a.stage {
            ManaStage::Payment | ManaStage::Spend => self.payment_decision(
                a.player,
                a.payment_cost().ok_or("missing mana ability cost")?,
                Some(a.source),
                a.stage == ManaStage::Payment,
            ),
            ManaStage::Color => {
                let candidates = self
                    .mana_colors(a.player, &a.ability.production)?
                    .into_iter()
                    .map(|color| Candidate::Color { color })
                    .collect();
                Ok(self.pick(
                    a.player,
                    Reason::ManaColor {
                        source: Some(a.source),
                    },
                    candidates,
                    1,
                    1,
                ))
            }
        }
    }

    pub(super) fn permanent_mana_step(&mut self, mut a: ManaActivation) -> Result<(), String> {
        let input = self
            .rules_mut()
            .pending
            .take()
            .ok_or("missing mana ability input")?;
        match &input.answer {
            Answer::Pay {
                pay: PayAnswer::Activate { source },
            } if a.stage == ManaStage::Payment => {
                let object = input.mana_source(*source)?;
                self.begin_payment_mana(
                    &input,
                    object,
                    Frame::ManaAbility {
                        activation: Box::new(a),
                    },
                )?;
            }
            Answer::Pay {
                pay:
                    PayAnswer::Finish {
                        undo_on_failure: undo_mana,
                    },
            } if a.stage == ManaStage::Payment => {
                let cost = a.payment_cost().ok_or("missing nested mana cost")?;
                // CR 601.2g-h: a nested activation may have changed the other
                // costs (a life payment, the source's tap), so every cost is
                // rechecked before the spend decision; otherwise the outer
                // activation is cancelled and its legal nested ones remain.
                if self.can_pay_offered(a.player, &cost)?
                    && self
                        .validate_permanent_mana_costs(a.player, a.source, a.index)
                        .is_ok()
                {
                    a.stage = ManaStage::Spend;
                    self.rules_mut().frame = Frame::ManaAbility {
                        activation: Box::new(a),
                    };
                } else {
                    if *undo_mana && self.undo_mana_receipts(a.player, &a.receipts)? {
                        a.receipts.clear();
                    }
                    self.resume_mana(self.saved_frame(a.resume)?.clone(), a.receipts)?;
                }
            }
            Answer::Pay {
                pay: PayAnswer::Spend { unrestricted },
            } if a.stage == ManaStage::Spend => {
                self.pay_mana_ability(a, unrestricted.clone())?;
            }
            Answer::Pick { .. } if a.stage == ManaStage::Color => {
                let Candidate::Color { color } = *input.one()? else {
                    return Err("invalid mana color".into());
                };
                self.resolve_mana_ability(a, Some(color))?;
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
        self.validate_permanent_mana_costs(a.player, a.source, a.index)?;
        // No costs are paid until the nested payment succeeds (601.2g-h).
        let pool = &self.state.rules.players[a.player.0 as usize].mana;
        let mut receipt = PermanentManaUndo {
            player: a.player,
            source: self.object_before(a.source)?,
            counters: self.current(a.source)?.plus_one_counters.clone(),
            ability_use: a
                .ability
                .restriction
                .map(|_| self.mana_use_key(a.source, a.index)),
            previously_used: self.mana_ability_index_used(a.source, a.index),
            produced: BTreeMap::new(),
            spent: spending.clone(),
            spent_restricted: vec![],
            absent: [Color::W, Color::U, Color::B, Color::R, Color::G, Color::C]
                .into_iter()
                .filter(|c| !pool.contains_key(c))
                .collect(),
            sacrificed: a.ability.costs.sacrifice,
            cost_triggers: vec![],
            kept: !a.reversible(),
        };
        let cause = a.cause();
        if let Some(cost) = a.payment_cost() {
            self.commit(
                ProposedEvent::PayManaSelected {
                    player: a.player,
                    cost,
                    spending,
                    restricted: vec![],
                },
                cause.clone(),
            )?;
        }
        let first_trigger = self.state.rules.next_trigger_id;
        self.pay_permanent_source_costs(a.source, a.player, &a.ability.costs, cause)?;
        if let Some(k) = receipt.ability_use {
            self.rules_mut().mana_ability_uses.insert(k);
        }
        receipt.cost_triggers = self
            .state
            .rules
            .pending_triggers
            .iter()
            .filter(|t| t.id.0 >= first_trigger)
            .map(|t| t.id)
            .collect();
        a.paid = Some(Box::new(receipt));
        match a.ability.production {
            ManaProduction::Fixed { .. } => self.resolve_mana_ability(a, None),
            // CR 106.7: no type of mana is defined, so the ability adds none.
            _ if self
                .mana_colors(a.player, &a.ability.production)?
                .is_empty() =>
            {
                self.resolve_mana_ability(a, None)
            }
            _ => {
                a.stage = ManaStage::Color;
                self.rules_mut().frame = Frame::ManaAbility {
                    activation: Box::new(a),
                };
                Ok(())
            }
        }
    }
    pub(super) fn permanent_source_costs_payable(
        &self,
        source: ObjectId,
        player: PlayerId,
        costs: &PermanentActivationCosts,
    ) -> bool {
        self.current(source).is_ok_and(|o| {
            o.present_on_battlefield()
                && o.controller == player
                && !(costs.tap && o.tapped)
                && costs
                    .remove_source_plus_one_counters
                    .as_ref()
                    .is_none_or(|n| o.plus_one_counters >= *n)
                && costs.pay_life.as_ref().is_none_or(|life| {
                    self.state.rules.players[player.0 as usize].can_pay_life(life)
                })
        })
    }
    pub(super) fn pay_permanent_source_costs(
        &mut self,
        source: ObjectId,
        player: PlayerId,
        costs: &PermanentActivationCosts,
        cause: BatchCause,
    ) -> Result<(), String> {
        if !self.permanent_source_costs_payable(source, player, costs) {
            return Err("unpayable permanent source costs".into());
        }
        // CR 118.3/605.3b: this is removal as a cost, never a placement.
        // The enclosing staged rule step commits cost/use/production atomically.
        if let Some(n) = &costs.remove_source_plus_one_counters {
            self.rules_mut()
                .objects
                .get_mut(&source.card)
                .ok_or("missing source")?
                .plus_one_counters
                .subtract(n);
        }
        let mut children = vec![];
        if costs.tap {
            children.push(EventGroup::Occurrence {
                proposed: ProposedEvent::Tap { object: source },
            });
        }
        // CR 119.4: paying life is loss of life, never damage.
        if let Some(life) = &costs.pay_life {
            children.push(EventGroup::Occurrence {
                proposed: ProposedEvent::LoseLife {
                    player,
                    amount: life.clone(),
                },
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
        a: ManaActivation,
        color: Option<Color>,
    ) -> Result<(), String> {
        let units = match &a.ability.production {
            ManaProduction::Fixed { symbols } => {
                symbols.iter().map(|c| (*c, Quantity::from(1))).collect()
            }
            // CR 106.1a: N mana of the one chosen color.
            ManaProduction::Choose { amount, .. } => {
                color.into_iter().map(|c| (c, amount.clone())).collect()
            }
            _ => color.into_iter().map(|c| (c, Quantity::from(1))).collect(),
        };
        self.produce_mana_units(a, units)
    }
    fn produce_mana_units(
        &mut self,
        mut a: ManaActivation,
        units: Vec<(Color, Quantity)>,
    ) -> Result<(), String> {
        let cause = a.cause();
        let receipt = a.paid.as_mut().ok_or("unpaid mana ability")?;
        // A sacrifice cost changes the source incarnation, but the frozen
        // ability still resolves (113.7a); AddMana uses that current identity.
        let source = self.object_id(a.source.card).unwrap_or(a.source);
        let mut children = vec![];
        for (color, amount) in units {
            receipt.produced.entry(color).or_default().add(&amount);
            children.push(EventGroup::Occurrence {
                proposed: ProposedEvent::AddMana {
                    player: a.player,
                    color,
                    amount,
                    source,
                    restriction: None,
                },
            });
        }
        if !children.is_empty() {
            self.commit_batch(cause, EventGroup::Ordered { children })?;
        }
        self.perform_mana_riders(a)
    }

    /// CR 605.3b, 608.2c: the riders, in printed order, after the mana is
    /// added. Damage runs through the ordinary damage transaction (CR 615,
    /// 616), whose continuation resumes here with the next rider.
    fn perform_mana_riders(&mut self, mut a: ManaActivation) -> Result<(), String> {
        while let Some(node) = a.ability.riders.get(a.rider as usize).cloned() {
            a.rider += 1;
            let amount = |amount: &mtg_contracts::kir::Amount| {
                amount
                    .as_fixed()
                    .cloned()
                    .ok_or_else(|| "unaudited mana rider amount".to_string())
            };
            match &node {
                Node::Do(Verb::GainLife { amount: n, .. }) => {
                    self.commit(
                        ProposedEvent::GainLife {
                            player: a.player,
                            amount: amount(n)?,
                        },
                        a.cause(),
                    )?;
                }
                Node::Do(Verb::Damage { amount: n, .. }) => {
                    let amount = amount(n)?;
                    return self.begin_mana_rider_damage(a, amount);
                }
                _ => return Err("unaudited mana rider".into()),
            }
        }
        self.finish_mana_ability(a)
    }

    /// CR 120.3a: the source deals the damage to its controller through the
    /// ordinary damage transaction, which the step dispatcher advances like
    /// any other: replacement and prevention effects apply (CR 615, 616) and
    /// the activation stays suspended in the transaction until it commits.
    fn begin_mana_rider_damage(
        &mut self,
        a: ManaActivation,
        amount: Quantity,
    ) -> Result<(), String> {
        let damage = effects::PreparedDamage {
            source_appearance: None,
            source_handles: self.capture_trigger_source_handles(a.source)?,
            id: DamageId(0),
            source: self.object_before(a.source)?,
            controller: a.player,
            recipient: DamageRecipient::Player { player: a.player },
            original_amount: amount.clone(),
            amount,
            applied: vec![],
        };
        let cause = a.cause();
        self.begin_damage(
            DamageKind::Noncombat,
            vec![damage],
            cause,
            DamageContinuation::Mana {
                activation: Box::new(a),
            },
        )
    }

    /// The damage transaction committed: perform the remaining riders, or,
    /// if the payer or the payment it suspended departed meanwhile, keep the
    /// legal activation and return to the retained frame (CR 733.1, 800.4a).
    pub(in crate::engine) fn resume_mana_riders(
        &mut self,
        a: ManaActivation,
    ) -> Result<(), String> {
        if self.mana_rider_resume_available(&a)? {
            return self.perform_mana_riders(a);
        }
        if let frame @ Frame::ResolutionPayment { .. } =
            frame::retained_effect_frame(&self.state.rules.frames, self.saved_frame(a.resume)?)
        {
            self.rules_mut().frame = frame.clone();
            return self.revalidate_resolution_payment_departure();
        }
        self.rules_mut().passes.clear();
        self.rules_mut().frame = Frame::Priority;
        Ok(self.stabilize()?)
    }

    fn mana_rider_resume_available(&self, a: &ManaActivation) -> Result<bool, String> {
        Ok(self.is_live(a.player)
            && self.mana_resume_player(self.saved_frame(a.resume)?, a.player)
            && match self.underlying_mana_frame(self.saved_frame(a.resume)?)? {
                Frame::Priority => self.state.rules.priority == a.player,
                Frame::ResolutionPayment { payment, .. } => {
                    payment.payer == a.player && payment.stage == ResolutionPaymentStage::Mana
                }
                Frame::Mana { cast } => self
                    .top()
                    .is_ok_and(|top| top.id == cast.stack_id && top.controller == a.player),
                _ => false,
            })
    }

    /// The suspended rider damage is exactly the activation's current rider.
    pub(in crate::engine) fn validate_mana_rider_damage(
        &self,
        tx: &effects::DamageTransaction,
        a: &ManaActivation,
    ) -> Result<(), String> {
        let [damage] = tx.entries.as_slice() else {
            return Err("invalid mana rider damage group".into());
        };
        let rider = a
            .rider
            .checked_sub(1)
            .and_then(|i| a.ability.riders.get(i as usize));
        let Some(Node::Do(Verb::Damage { amount, .. })) = rider else {
            return Err("unbound mana rider damage".into());
        };
        if a.paid.is_none()
            || !a.matches_definition(self.copiable_definition(a.source)?)
            || amount.as_fixed() != Some(&damage.original_amount)
            || tx.cause != a.cause()
            || damage.recipient != (DamageRecipient::Player { player: a.player })
            || damage.controller != a.player
            || damage.source.object != a.source
        {
            return Err("unbound mana rider damage".into());
        }
        if self.mana_rider_resume_available(a)? {
            self.validate_nested_mana_frame(self.saved_frame(a.resume)?)?;
        }
        Ok(())
    }

    fn finish_mana_ability(&mut self, mut a: ManaActivation) -> Result<(), String> {
        let receipt = a.paid.take().ok_or("unpaid mana ability")?;
        a.receipts.push(ManaActivationUndo {
            object: a.source,
            player: a.player,
            color: Color::C,
            amount: Quantity::zero(),
            mana_was_absent: false,
            permanent: Some(receipt),
        });
        self.resume_mana(self.saved_frame(a.resume)?.clone(), a.receipts)
    }
    pub(super) fn resume_mana(
        &mut self,
        mut resume: Frame,
        receipts: Vec<ManaActivationUndo>,
    ) -> Result<(), String> {
        let retained = !receipts.is_empty();
        match &mut resume {
            Frame::ManaAbility { activation } => activation.receipts.extend(receipts),
            Frame::Mana { cast } => cast.mana_activations.extend(receipts),
            Frame::ResolutionPayment { payment, .. }
                if payment.stage == ResolutionPaymentStage::Mana =>
            {
                payment.receipts.extend(receipts)
            }
            Frame::Priority => {
                if retained {
                    self.rules_mut().passes.clear();
                }
            }
            _ => return Err("invalid mana continuation".into()),
        }
        self.rules_mut().frame = resume;
        if matches!(self.state.rules.frame, Frame::Priority) {
            return Ok(self.stabilize()?);
        }
        self.revalidate_resolution_payment_departure()
    }
    /// CR 733.1: reverse every receipt, newest first, and report true; or,
    /// when one of them is kept, reverse none and report false.
    pub(super) fn undo_mana_receipts(
        &mut self,
        player: PlayerId,
        receipts: &[ManaActivationUndo],
    ) -> Result<bool, String> {
        if !reversible_receipts(receipts) {
            return Ok(false);
        }
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
                    decision_id: self.state.rules.revision,
                    player,
                    source: r.object,
                },
            )?;
        }
        Ok(true)
    }
    pub(super) fn reverse_permanent_mana(
        &mut self,
        r: &PermanentManaUndo,
    ) -> Result<EventOutcome, String> {
        if !self.is_live(r.player) {
            return Err("departed mana reversal payer".into());
        }
        let p = &mut self.rules_mut().players[r.player.0 as usize];
        // CR 106.6: no payment spends restricted mana, so none is refunded.
        if !r.spent_restricted.is_empty() {
            return Err("restricted mana has no spend decision".into());
        }
        let pool = &mut p.mana;
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
                    .rules
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
                self.rules_mut().objects.insert(
                    r.source.object.card,
                    Object {
                        hideaway: None,
                        phased_out: None,
                        attached_to: None,
                        attachment_timestamp: None,
                        battlefield_entry_turn: r.source.battlefield_entry_turn,
                        keyword_timestamp: r.source.keyword_timestamp,
                        id: r.source.object,
                        controller: r.source.controller,
                        default_controller: r.source.default_controller,
                        zone: r.source.zone,
                        tapped: r.source.tapped,
                        attack_ready: r.source.attack_ready,
                        marked_damage: r.source.marked_damage.clone(),
                        deathtouch_damage: r.source.deathtouch_damage,
                        plus_one_counters: r.counters.clone(),
                        transforms: 0,
                        adventure_player: None,
                    },
                );
                // CR 733.1/800.4c: restore only the recorded object, then
                // derive its controller from surviving exact-incarnation Auras.
                // Departed owners/sources remain absent; payment authority and
                // the receipt's mana/counter/use bookkeeping stay unchanged.
                self.reconcile_controllers()?;
            } else {
                let o = self
                    .rules_mut()
                    .objects
                    .get_mut(&r.source.object.card)
                    .ok_or("missing surviving mana source")?;
                if o.id == r.source.object && o.zone == Zone::Battlefield {
                    if r.ability_use.is_none() && !o.tapped {
                        return Err("mana reversal source changed".into());
                    }
                    o.tapped = r.source.tapped;
                    // CR 733: exact restoration bypasses placement/replacement.
                    o.plus_one_counters = r.counters.clone();
                }
                // CR 400.7/733: a departed source is never restored or rebound.
            }
        }
        if let Some(k) = r.ability_use {
            self.rules_mut().mana_ability_uses.remove(&k);
            if r.previously_used {
                self.rules_mut().mana_ability_uses.insert(k);
            }
        }
        self.rules_mut()
            .pending_triggers
            .retain(|t| !r.cost_triggers.contains(&t.id));
        Ok(EventOutcome::PermanentManaReversed {
            player: r.player,
            source: r.source.object,
        })
    }
    pub(super) fn revalidate_mana_departure(&mut self) -> Result<(), String> {
        if let Frame::ManaSourceChoice {
            player,
            source,
            scope,
            resume: _,
        } = &self.state.rules.frame
        {
            // Copies, so the choice can be discarded while they are in use.
            let (player, source, scope) = (*player, *source, *scope);
            let (player, source, scope) = (&player, &source, &scope);
            let invalid =
                if *scope == ManaSourceScope::Mixed {
                    match self.mixed_mana_choices(*player, *source) {
                        Err(_) => true,
                        Ok(choices) => {
                            let selected = self.state.rules.pending.as_ref().and_then(|input| {
                                match input.one() {
                                    Ok(Candidate::ManaAbility { ability }) => Some(*ability),
                                    _ => None,
                                }
                            });
                            if let Some(ability) = selected
                                && choices.contains(&ability)
                            {
                                false
                            } else {
                                self.rules_mut().pending = None;
                                // CR 605.3a: the choice stands while the source
                                // still offers one and a payable ability remains.
                                !self.mana_ability_selection(*source).unwrap_or(false)
                            }
                        }
                    }
                } else {
                    self.mana_source_colors(*player, *source, *scope).is_err()
                };
            if invalid && let Frame::ManaSourceChoice { resume, .. } = &self.state.rules.frame {
                let resume = self.saved_frame(*resume)?.clone();
                let rules = self.rules_mut();
                rules.pending = None;
                rules.frame = resume;
            }
        }
        let mut f = &self.state.rules.frame;
        if let Frame::ManaSourceChoice { resume, .. } = f {
            f = self.saved_frame(*resume)?;
        }
        let mut invalid = false;
        while let Frame::ManaAbility { activation: a } = f {
            invalid |= !self.is_live(a.player)
                || (a.paid.is_none()
                    && self
                        .validate_permanent_mana_costs(a.player, a.source, a.index)
                        .is_err());
            f = self.saved_frame(a.resume)?;
        }
        if let Frame::Mana { cast } = f {
            invalid |= self.state.rules.stack.last().is_none_or(|top| {
                top.id != cast.stack_id || self.announced_target_legality(top).iter().any(|v| !*v)
            });
        }
        if invalid {
            if let Frame::ManaSourceChoice { resume, .. } = &self.state.rules.frame {
                let resume = self.saved_frame(*resume)?.clone();
                self.rules_mut().frame = resume;
            }
            while let Frame::ManaAbility { activation } = &self.state.rules.frame {
                let mut a = ManaActivation::clone(activation);
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
                if !self.is_live(a.player) || !self.undo_mana_receipts(a.player, &a.receipts)? {
                    match self.saved_frame_mut(a.resume)? {
                        Frame::Mana { cast } => cast.mana_activations.append(&mut a.receipts),
                        Frame::ManaAbility { activation } => {
                            activation.receipts.append(&mut a.receipts)
                        }
                        Frame::Priority if !a.receipts.is_empty() => {
                            self.rules_mut().passes.clear()
                        }
                        _ => {}
                    }
                }
                self.rules_mut().frame = self.saved_frame(a.resume)?.clone();
            }
            self.rules_mut().pending = None;
        }
        // CR 106.7: a departure can remove the last land that defined a
        // "could produce" type. The paid ability then adds no mana.
        if let Frame::ManaAbility { activation } = &self.state.rules.frame
            && activation.stage == ManaStage::Color
            && self.is_live(activation.player)
            && self
                .mana_colors(activation.player, &activation.ability.production)?
                .is_empty()
        {
            let a = ManaActivation::clone(activation);
            self.rules_mut().pending = None;
            return self.resolve_mana_ability(a, None);
        }
        self.revalidate_resolution_payment_departure()
    }
    pub(crate) fn validate_nested_mana_frame(&self, frame: &Frame) -> Result<(), String> {
        let mut f = frame;
        let mut sources = BTreeSet::new();
        if let Frame::ManaSourceChoice {
            player,
            source,
            scope,
            resume,
        } = f
        {
            if match scope {
                ManaSourceScope::Mixed => self.mixed_mana_choices(*player, *source)?.is_empty(),
                ManaSourceScope::Intrinsic => {
                    self.mana_source_colors(*player, *source, *scope)?.len() < 2
                }
            } || !self.mana_resume_player(self.saved_frame(*resume)?, *player)
            {
                return Err("mana source choice snapshot invariant".into());
            }
            sources.insert(*source);
            f = self.saved_frame(*resume)?;
        }
        while let Frame::ManaAbility { activation: a } = f {
            if !self.is_live(a.player)
                || !sources.insert(a.source)
                || !a.matches_definition(self.copiable_definition(a.source)?)
                || !self.mana_resume_player(self.saved_frame(a.resume)?, a.player)
                || (a.stage == ManaStage::Color) != a.paid.is_some()
                || a.receipts.iter().any(|r| r.player != a.player)
            {
                return Err("nested mana snapshot invariant".into());
            }
            f = self.saved_frame(a.resume)?;
        }
        if matches!(f, Frame::ManaSourceChoice { .. }) {
            return Err("buried mana source choice continuation".into());
        }
        Ok(())
    }
}
