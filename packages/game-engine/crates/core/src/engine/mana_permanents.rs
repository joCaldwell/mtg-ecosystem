//! CR 106.1b/4, 302.6, 602.5a, 605.1a/3a-c, 702.10c, 733.1-2, 903.4a:
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;
use mtg_contracts::{disjoint_pain_mana_family, hybrid_filter_colors};

mod hybrid_filter;
use hybrid_filter::uses_indexed_paid_mana;

mod mana_damage;
mod mana_life_gain;
use mana_life_gain::ManaLifeUndo;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum ManaSourceScope {
    Intrinsic,
    Disjoint,
    Mixed,
    PrintedDisjoint { index: u32 },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct ManaBranch {
    index: u32,
    color: Color,
}

/// A producing ability and its chosen output are separate. An intrinsic ability
/// is identified by its basic subtype's mana color (CR 305.6), while a printed
/// ability is identified by its index on this source incarnation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ManaSourceChoice {
    Intrinsic(Color),
    Printed(ManaBranch),
}

impl ManaSourceChoice {
    fn color(self) -> Color {
        match self {
            Self::Intrinsic(color) => color,
            Self::Printed(branch) => branch.color,
        }
    }

    fn from_color(choices: &[Self], color: Color) -> Result<Self, String> {
        let mut matching = choices.iter().copied().filter(|c| c.color() == color);
        let choice = matching.next().ok_or("invalid mana source choice")?;
        if matching.next().is_some() {
            return Err("mana color identifies multiple source abilities".into());
        }
        Ok(choice)
    }
}

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
    FilterInput,
    Payment,
    Spend,
    Color,
    Combination { total: Quantity },
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct ManaActivation {
    pub(super) player: PlayerId,
    pub(super) source: ObjectId,
    ability: ManaAbility,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    printed_index: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    filter_input: Option<Color>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    branch: Option<ManaBranch>,
    decision_id: u64,
    pub(super) stage: ManaStage,
    pub(super) resume: Box<Frame>,
    receipts: Vec<ManaActivationUndo>,
    paid: Option<Box<PermanentManaUndo>>,
}
impl ManaActivation {
    pub(super) fn ability_index(&self) -> u32 {
        self.printed_index
            .or(self.branch.map(|branch| branch.index))
            .unwrap_or_default()
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
    pub kind: ManaAbilityKind,
    pub index: u32,
    pub turn: u64,
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ability_use: Option<ManaAbilityUse>,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub previously_used: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(super) combination_total: Option<Quantity>,
    pub produced: BTreeMap<Color, Quantity>,
    pub spent: BTreeMap<Color, Quantity>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub spent_restricted: Vec<RestrictedMana>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub produced_restricted: Vec<RestrictedMana>,
    pub absent: BTreeSet<Color>,
    pub sacrificed: bool,
    pub cost_triggers: Vec<TriggerId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(super) branch: Option<ManaBranch>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(super) damage: Option<Box<ManaDamageUndo>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(super) life_gain: Option<ManaLifeUndo>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct ManaDamageUndo {
    pub(super) life: Vec<ManaDamageLife>,
    pub(super) shields: Vec<ManaDamageShield>,
    triggers: Vec<(TriggerId, PlayerId)>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct ManaDamageLife {
    pub(super) player: PlayerId,
    before: Quantity,
    after: Quantity,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct ManaDamageShield {
    pub(super) effect: EffectId,
    before: Quantity,
    after: Quantity,
}

impl Game {
    pub(super) fn mana_use_key(&self, source: ObjectId) -> ManaAbilityUse {
        ManaAbilityUse {
            source,
            kind: ManaAbilityKind::Mana,
            index: 0,
            turn: self.state.rules.turn.number,
        }
    }
    pub(super) fn mana_ability_used(&self, source: ObjectId) -> bool {
        self.state
            .rules
            .mana_ability_uses
            .contains(&self.mana_use_key(source))
    }
    pub(super) fn validate_mana_uses(&self) -> Result<(), String> {
        if self.state.rules.mana_ability_uses.len() > self.state.rules.objects.len() {
            return Err("mana use capacity invariant".into());
        }
        for k in &self.state.rules.mana_ability_uses {
            if k.index != 0
                || k.turn != self.state.rules.turn.number
                || self.current(k.source)?.zone != Zone::Battlefield
                || self
                    .copiable_definition(k.source)?
                    .mana_abilities
                    .first()
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
            ManaProduction::Fixed { symbols }
            | ManaProduction::FixedWithLifeGain { symbols, .. }
            | ManaProduction::RestrictedFixed { symbols, .. }
            | ManaProduction::Choice { colors: symbols } => symbols.clone(),
            ManaProduction::HybridFilter { colors } => colors.to_vec(),
            ManaProduction::GreatestControlledPowerCombination
            | ManaProduction::AnyColor
            | ManaProduction::AnyOneColor { .. } => {
                vec![Color::W, Color::U, Color::B, Color::R, Color::G]
            }
            ManaProduction::OpponentLandColors => return self.opponent_land_mana_colors(player),
            ManaProduction::EachPermanentColor { scope } => {
                self.permanent_colors(*scope, player)?.into_iter().collect()
            }
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
    pub(super) fn mana_continuation_charge(&self) -> usize {
        let mut frame = &self.state.rules.frame;
        let mut charge = 0usize;
        loop {
            let activation = match frame {
                Frame::ManaSourceChoice {
                    player,
                    source,
                    scope,
                    resume,
                } => {
                    #[derive(Serialize)]
                    struct Wrapper<'a> {
                        stage: &'static str,
                        player: &'a PlayerId,
                        source: &'a ObjectId,
                        scope: &'a ManaSourceScope,
                        resume: (),
                    }
                    charge = charge.saturating_add(
                        stored_history::payload_charge(&Wrapper {
                            stage: "mana_source_choice",
                            player,
                            source,
                            scope,
                            resume: (),
                        })
                        .saturating_sub(4),
                    );
                    frame = resume;
                    continue;
                }
                Frame::ManaAbility { activation } => activation,
                Frame::DamageEffects { transaction }
                | Frame::DamageAllocation { transaction, .. } => {
                    let DamageContinuation::Mana { activation, .. } =
                        transaction.continuation.as_ref()
                    else {
                        break;
                    };
                    charge = charge
                        .saturating_add(stored_history::payload_charge(&transaction.entries))
                        .saturating_add(stored_history::payload_charge(&transaction.applications));
                    activation
                }
                // This new root owns the entire suspended resolution and any
                // nested activation wrappers and receipts. Count their bytes once.
                Frame::ResolutionPayment { .. } => {
                    return stored_history::payload_charge(&self.state.rules.frame);
                }
                Frame::Mana { cast }
                | Frame::SpendMana { cast }
                | Frame::SacrificeCost { cast }
                | Frame::Payment { cast } => {
                    charge =
                        charge.saturating_add(Self::mana_receipt_charge(&cast.mana_activations));
                    break;
                }
                _ => break,
            };
            if let Some(index) = activation.printed_index {
                charge = charge.saturating_add(stored_history::payload_charge(&(
                    index,
                    activation.filter_input,
                )));
            }
            if let ManaStage::Combination { total } = &activation.stage {
                charge = charge.saturating_add(stored_history::payload_charge(total));
            }
            charge = charge.saturating_add(Self::mana_receipt_charge(&activation.receipts));
            if let Some(paid) = &activation.paid
                && (paid.branch.is_some()
                    || paid.combination_total.is_some()
                    || paid.life_gain.is_some()
                    || !paid.spent_restricted.is_empty()
                    || !paid.produced_restricted.is_empty())
            {
                charge = charge.saturating_add(stored_history::payload_charge(paid));
            }
            frame = &activation.resume;
        }
        charge
    }

    fn mana_receipt_charge(receipts: &[ManaActivationUndo]) -> usize {
        receipts.iter().fold(0usize, |charge, receipt| {
            if receipt.permanent.as_ref().is_some_and(|r| {
                r.branch.is_some()
                    || r.combination_total.is_some()
                    || r.life_gain.is_some()
                    || !r.spent_restricted.is_empty()
                    || !r.produced_restricted.is_empty()
            }) {
                charge.saturating_add(stored_history::payload_charge(receipt))
            } else {
                charge
            }
        })
    }

    pub(super) fn underlying_mana_frame<'a>(&self, frame: &'a Frame) -> &'a Frame {
        let mut f = frame;
        loop {
            f = match f {
                Frame::GraveyardGroup { tx } => match &tx.finish {
                    GroupFinish::Payment { resume } => resume,
                    _ => break,
                },
                Frame::ManaAbility { activation } => &activation.resume,
                Frame::ManaSourceChoice { resume, .. } => resume,
                Frame::DamageEffects { transaction }
                | Frame::DamageAllocation { transaction, .. } => {
                    match transaction.continuation.as_ref() {
                        DamageContinuation::Mana { activation, .. }
                            if self.mana_damage_resume_available(activation) =>
                        {
                            &activation.resume
                        }
                        _ => break,
                    }
                }
                _ => break,
            };
        }
        f
    }
    pub(super) fn reserved_mana_source(&self, source: ObjectId) -> bool {
        let mut f = &self.state.rules.frame;
        if let Frame::ManaSourceChoice {
            source: selected,
            resume,
            ..
        } = f
        {
            if *selected == source {
                return true;
            }
            f = resume;
        }
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
            && self.copiable_definition(source).is_ok_and(|d| {
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
        if self.reserved_mana_source(source) {
            return Err("reserved permanent mana source".into());
        }
        let definition = self.copiable_definition(source)?;
        if definition.mana_abilities.len() != 1
            && !disjoint_pain_mana_family(definition)
            && !uses_indexed_paid_mana(definition)
        {
            return Err("unsupported permanent mana source".into());
        }
        if !self.intrinsic_mana_colors(source)?.is_empty() {
            return self.mixed_mana_choices(player, source).map(|_| ());
        }
        self.validate_permanent_mana_costs(player, source, 0)
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
                .remove_source_plus_one_counters
                .as_ref()
                .is_some_and(|n| o.plus_one_counters < *n)
            || (a.restriction.is_some() && self.mana_ability_used(source))
            || (a.costs.tap
                && self.target_is_creature(source)
                && !o.attack_ready
                && !self.has_keyword(source, KeywordAbility::Haste))
        {
            return Err("invalid permanent mana source or tap eligibility".into());
        }
        Ok(())
    }

    pub(super) fn mixed_mana_choices(
        &self,
        player: PlayerId,
        source: ObjectId,
    ) -> Result<Vec<ManaAbilityIdentity>, String> {
        let definition = self.copiable_definition(source)?;
        let grant = self.has_granted_land_mana(source)?;
        let printed = definition.mana_abilities.len() == 1
            || disjoint_pain_mana_family(definition)
            || uses_indexed_paid_mana(definition);
        let mut choices: Vec<_> = self
            .intrinsic_choice_colors(player, source)
            .unwrap_or_default()
            .into_iter()
            .map(|color| ManaAbilityIdentity::Intrinsic { color })
            .collect();
        // Identical grants are interchangeable in the closed mana rules. They
        // stay distinct from the land's intrinsic and printed abilities.
        if grant && self.validate_simple_tap_mana(player, source).is_ok() {
            choices.extend(
                Self::granted_land_mana_colors()
                    .map(|color| ManaAbilityIdentity::GrantedLandTap { color }),
            );
        }
        for index in 0..if printed {
            definition.mana_abilities.len()
        } else {
            0
        } {
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
    fn mana_source_choices(
        &self,
        player: PlayerId,
        source: ObjectId,
        scope: ManaSourceScope,
    ) -> Result<Vec<ManaSourceChoice>, String> {
        match scope {
            ManaSourceScope::Mixed => Err("mixed provider requires an ability identity".into()),
            ManaSourceScope::Intrinsic if self.is_permanent_mana(source) => {
                Err("printed provider cannot select an intrinsic-only scope".into())
            }
            ManaSourceScope::Intrinsic => Ok(self
                .intrinsic_choice_colors(player, source)?
                .into_iter()
                .map(ManaSourceChoice::Intrinsic)
                .collect()),
            ManaSourceScope::Disjoint | ManaSourceScope::PrintedDisjoint { .. } => {
                let definition = self.copiable_definition(source)?;
                if !disjoint_pain_mana_family(definition) {
                    return Err("invalid disjoint mana source".into());
                }
                let mut choices = Vec::new();
                for (index, ability) in definition.mana_abilities.iter().enumerate() {
                    let index = u32::try_from(index).map_err(|_| "mana ability index overflow")?;
                    if matches!(scope,ManaSourceScope::PrintedDisjoint{index:selected} if selected!=index)
                    {
                        continue;
                    }
                    self.validate_permanent_mana_costs(player, source, index)?;
                    for color in self.mana_colors(player, &ability.production)? {
                        choices.push(ManaSourceChoice::Printed(ManaBranch { index, color }));
                    }
                }
                if choices.is_empty() {
                    return Err("missing printed mana branch".into());
                }
                Ok(choices)
            }
        }
    }

    pub(super) fn mana_source_colors(
        &self,
        player: PlayerId,
        source: ObjectId,
        scope: ManaSourceScope,
    ) -> Result<Vec<Color>, String> {
        let choices = self.mana_source_choices(player, source, scope)?;
        let colors: Vec<_> = choices.iter().map(|c| c.color()).collect();
        if colors.iter().collect::<BTreeSet<_>>().len() != colors.len() {
            return Err("mana source requires a choice of ability".into());
        }
        Ok(colors)
    }

    pub(super) fn mana_source_choice_step(
        &mut self,
        player: PlayerId,
        source: ObjectId,
        scope: ManaSourceScope,
        resume: Frame,
    ) -> Result<(), String> {
        let input = self
            .state
            .rules
            .pending
            .take()
            .ok_or("missing mana source choice input")?;
        if scope == ManaSourceScope::Mixed {
            let Response::ChooseManaAbility { ability } = input.response else {
                return Err("expected mana ability identity".into());
            };
            if input.player != player
                || !self.mana_resume_player(&resume, player)
                || !self.mixed_mana_choices(player, source)?.contains(&ability)
            {
                return Err("invalid selected mana ability".into());
            }
            return match ability {
                ManaAbilityIdentity::GrantedLandTap { color } => {
                    let receipt = self.commit_unrestricted_tap_mana(&input, source, color)?;
                    self.resume_mana(resume, vec![receipt])
                }
                ManaAbilityIdentity::Intrinsic { color } => {
                    let receipt = self.activate_selected_intrinsic_mana(&input, source, color)?;
                    self.resume_mana(resume, vec![receipt])
                }
                ManaAbilityIdentity::Printed { index } => {
                    if uses_indexed_paid_mana(self.copiable_definition(source)?) && index == 1 {
                        return self.start_indexed_mana_ability(
                            &input,
                            source,
                            None,
                            Some(index),
                            resume,
                        );
                    }
                    if !disjoint_pain_mana_family(self.copiable_definition(source)?) {
                        return self.start_mana_ability(&input, source, None, resume);
                    }
                    let scope = ManaSourceScope::PrintedDisjoint { index };
                    let choices = self.mana_source_choices(player, source, scope)?;
                    if let [ManaSourceChoice::Printed(branch)] = choices.as_slice() {
                        self.start_mana_ability(&input, source, Some(*branch), resume)
                    } else {
                        self.state.rules.frame = Frame::ManaSourceChoice {
                            player,
                            source,
                            scope,
                            resume: Box::new(resume),
                        };
                        Ok(())
                    }
                }
            };
        }
        let Response::ManaColor { color } = input.response else {
            return Err("mana source choice input invariant".into());
        };
        if input.player != player || !self.mana_resume_player(&resume, player) {
            return Err("invalid mana source choice".into());
        }
        let choices = self.mana_source_choices(player, source, scope)?;
        match ManaSourceChoice::from_color(&choices, color)? {
            ManaSourceChoice::Intrinsic(color) => {
                let receipt = self.activate_selected_intrinsic_mana(&input, source, color)?;
                self.resume_mana(resume, vec![receipt])
            }
            ManaSourceChoice::Printed(branch) => {
                self.start_mana_ability(&input, source, Some(branch), resume)
            }
        }
    }

    pub(super) fn begin_payment_mana(
        &mut self,
        input: &AcceptedInput,
        source: ObjectId,
        resume: Frame,
    ) -> Result<(), String> {
        if self.has_granted_land_mana(source)? {
            self.validate_mana_activation(input.player, source)?;
            if !self.mana_resume_player(&resume, input.player) {
                return Err("invalid granted mana continuation".into());
            }
            self.state.rules.frame = Frame::ManaSourceChoice {
                player: input.player,
                source,
                scope: ManaSourceScope::Mixed,
                resume: Box::new(resume),
            };
            Ok(())
        } else if self.is_permanent_mana(source) {
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
        if !self.intrinsic_mana_colors(source)?.is_empty()
            || uses_indexed_paid_mana(self.copiable_definition(source)?)
        {
            self.state.rules.frame = Frame::ManaSourceChoice {
                player: input.player,
                source,
                scope: ManaSourceScope::Mixed,
                resume: Box::new(resume),
            };
            Ok(())
        } else if disjoint_pain_mana_family(self.copiable_definition(source)?) {
            self.state.rules.frame = Frame::ManaSourceChoice {
                player: input.player,
                source,
                scope: ManaSourceScope::Disjoint,
                resume: Box::new(resume),
            };
            Ok(())
        } else {
            self.start_mana_ability(input, source, None, resume)
        }
    }

    fn start_mana_ability(
        &mut self,
        input: &AcceptedInput,
        source: ObjectId,
        branch: Option<ManaBranch>,
        resume: Frame,
    ) -> Result<(), String> {
        self.start_indexed_mana_ability(input, source, branch, None, resume)
    }

    fn start_indexed_mana_ability(
        &mut self,
        input: &AcceptedInput,
        source: ObjectId,
        branch: Option<ManaBranch>,
        printed_index: Option<u32>,
        resume: Frame,
    ) -> Result<(), String> {
        let index = printed_index
            .or(branch.map(|b| b.index))
            .unwrap_or_default();
        let ability = self
            .copiable_definition(source)?
            .mana_abilities
            .get(index as usize)
            .ok_or("missing selected mana ability")?
            .clone();
        let stage = if matches!(ability.production, ManaProduction::HybridFilter { .. }) {
            ManaStage::FilterInput
        } else {
            ManaStage::Payment
        };
        let activation = ManaActivation {
            player: input.player,
            source,
            decision_id: input.decision_id,
            ability,
            branch,
            printed_index,
            filter_input: None,
            stage,
            resume: Box::new(resume),
            receipts: vec![],
            paid: None,
        };
        if activation.stage != ManaStage::FilterInput && activation.payment_cost().is_none() {
            self.pay_mana_ability(activation, BTreeMap::new(), vec![])
        } else {
            self.state.rules.frame = Frame::ManaAbility {
                activation: Box::new(activation),
            };
            Ok(())
        }
    }
    pub(super) fn permanent_mana_request(&self, a: &ManaActivation) -> Result<Request, String> {
        Ok(match a.stage {
            ManaStage::FilterInput => Request::ManaColor {
                decision_id: self.state.rules.revision,
                player: a.player,
                colors: a.filter_input_colors()?,
            },
            ManaStage::Payment => Request::Mana {
                decision_id: self.state.rules.revision,
                player: a.player,
                cost: a.payment_cost().ok_or("missing mana ability cost")?,
            },
            ManaStage::Spend => Request::SpendMana {
                decision_id: self.state.rules.revision,
                player: a.player,
                cost: a.payment_cost().ok_or("missing mana ability cost")?,
                available: self.state.rules.players[a.player.0 as usize].mana.clone(),
                restricted_available: self
                    .eligible_restricted(a.player, self.activation_mana_consumer(a.source)),
            },
            ManaStage::Combination { ref total } => Request::ManaCombination {
                decision_id: self.state.rules.revision,
                player: a.player,
                total: total.clone(),
                colors: a.filter_colors().map_or_else(
                    || MANA_COMBINATION_COLORS.to_vec(),
                    |colors| colors.to_vec(),
                ),
            },
            ManaStage::Color => Request::ManaColor {
                decision_id: self.state.rules.revision,
                player: a.player,
                colors: self.mana_colors(a.player, &a.ability.production)?,
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
            (ManaStage::FilterInput, Response::ManaColor { color })
                if a.filter_colors()
                    .is_some_and(|colors| colors.contains(color)) =>
            {
                Ok(())
            }
            (ManaStage::Payment, Response::ActivateMana { object }) => {
                self.validate_mana_activation(player, *object)
            }
            (ManaStage::Payment, Response::FinishMana { .. }) => Ok(()),
            (ManaStage::Spend, Response::SpendMana { spending }) => self
                .selected_payment_pool(
                    player,
                    &a.payment_cost().ok_or("missing nested cost")?,
                    spending,
                )
                .map(|_| ()),
            (
                ManaStage::Spend,
                Response::SpendManaAllocated {
                    spending,
                    restricted,
                },
            ) => self
                .allocated_mana_pools(
                    player,
                    &a.payment_cost().ok_or("missing nested cost")?,
                    spending,
                    restricted,
                    self.activation_mana_consumer(a.source),
                )
                .map(|_| ()),
            (ManaStage::Combination { total }, Response::ManaCombination { amounts }) => {
                let colors = a.filter_colors();
                validate_mana_combination(
                    colors
                        .as_ref()
                        .map_or(MANA_COMBINATION_COLORS.as_slice(), |pair| pair.as_slice()),
                    total,
                    amounts,
                )
            }
            (ManaStage::Color, Response::ManaColor { color })
                if self
                    .mana_colors(player, &a.ability.production)?
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
            .rules
            .pending
            .take()
            .ok_or("missing mana ability input")?;
        match input.response {
            Response::ManaColor { color } if a.stage == ManaStage::FilterInput => {
                self.validate_permanent_mana_response(
                    &a,
                    input.player,
                    &Response::ManaColor { color },
                )?;
                a.filter_input = Some(color);
                a.stage = ManaStage::Payment;
                self.state.rules.frame = Frame::ManaAbility {
                    activation: Box::new(a),
                };
            }
            Response::ActivateMana { object } if a.stage == ManaStage::Payment => {
                self.begin_payment_mana(
                    &input,
                    object,
                    Frame::ManaAbility {
                        activation: Box::new(a),
                    },
                )?;
            }
            Response::FinishMana { undo_mana } if a.stage == ManaStage::Payment => {
                let cost = a.payment_cost().ok_or("missing nested mana cost")?;
                if self.can_pay_with_restricted(
                    a.player,
                    &cost,
                    self.activation_mana_consumer(a.source),
                )? {
                    a.stage = ManaStage::Spend;
                    self.state.rules.frame = Frame::ManaAbility {
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
                self.pay_mana_ability(a, spending, vec![])?
            }
            Response::SpendManaAllocated {
                spending,
                restricted,
            } if a.stage == ManaStage::Spend => self.pay_mana_ability(a, spending, restricted)?,
            Response::ManaCombination { amounts }
                if matches!(a.stage, ManaStage::Combination { .. }) =>
            {
                self.validate_permanent_mana_response(
                    &a,
                    input.player,
                    &Response::ManaCombination {
                        amounts: amounts.clone(),
                    },
                )?;
                let units = MANA_COMBINATION_COLORS
                    .into_iter()
                    .zip(amounts)
                    .filter(|(_, n)| *n > Quantity::zero())
                    .collect();
                self.produce_mana_units(a, units)?;
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
        restricted: Vec<RestrictedMana>,
    ) -> Result<(), String> {
        self.validate_permanent_mana_costs(a.player, a.source, a.ability_index())?;
        // No costs are paid until the nested payment succeeds (601.2g-h).
        let pool = &self.state.rules.players[a.player.0 as usize].mana;
        let mut receipt = PermanentManaUndo {
            player: a.player,
            source: self.object_before(a.source)?,
            counters: self.current(a.source)?.plus_one_counters.clone(),
            ability_use: a.ability.restriction.map(|_| self.mana_use_key(a.source)),
            previously_used: self.mana_ability_used(a.source),
            combination_total: None,
            produced: BTreeMap::new(),
            spent: spending.clone(),
            spent_restricted: restricted.clone(),
            produced_restricted: vec![],
            absent: [Color::W, Color::U, Color::B, Color::R, Color::G, Color::C]
                .into_iter()
                .filter(|c| !pool.contains_key(c))
                .collect(),
            sacrificed: a.ability.costs.sacrifice,
            cost_triggers: vec![],
            branch: a.branch,
            damage: None,
            life_gain: None,
        };
        let cause = a.cause();
        if let Some(cost) = a.payment_cost() {
            self.commit(
                ProposedEvent::PayManaSelected {
                    player: a.player,
                    cost,
                    spending,
                    restricted,
                },
                cause.clone(),
            )?;
        }
        let first_trigger = self.state.rules.next_trigger_id;
        self.pay_permanent_source_costs(a.source, a.player, &a.ability.costs, cause)?;
        if let Some(k) = receipt.ability_use {
            self.state.rules.mana_ability_uses.insert(k);
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
        if let Some(branch) = a.branch {
            return self.resolve_mana_ability(a, Some(branch.color));
        }
        match a.ability.production {
            ManaProduction::HybridFilter { .. } => {
                let total = Quantity::from(2);
                a.paid
                    .as_mut()
                    .ok_or("missing paid filter activation")?
                    .combination_total = Some(total.clone());
                a.stage = ManaStage::Combination { total };
                self.state.rules.frame = Frame::ManaAbility {
                    activation: Box::new(a),
                };
                Ok(())
            }
            ManaProduction::GreatestControlledPowerCombination => {
                let mut total = Quantity::zero();
                for o in self
                    .state
                    .rules
                    .objects
                    .values()
                    .filter(|o| o.present_on_battlefield() && o.controller == a.player)
                {
                    if self.is_battlefield_creature(o.id) {
                        total = total.max(
                            self.characteristics(o.id)?
                                .power
                                .ok_or("creature without power")?,
                        );
                    }
                }
                a.paid
                    .as_mut()
                    .ok_or("missing paid activation")?
                    .combination_total = Some(total.clone());
                if total == Quantity::zero() {
                    return self.produce_mana_units(a, vec![]);
                }
                a.stage = ManaStage::Combination { total };
                self.state.rules.frame = Frame::ManaAbility {
                    activation: Box::new(a),
                };
                Ok(())
            }
            ManaProduction::Fixed { .. }
            | ManaProduction::FixedWithLifeGain { .. }
            | ManaProduction::RestrictedFixed { .. }
            | ManaProduction::EachPermanentColor { .. } => self.resolve_mana_ability(a, None),
            _ if self
                .mana_colors(a.player, &a.ability.production)?
                .is_empty() =>
            {
                self.resolve_mana_ability(a, None)
            }
            _ => {
                a.stage = ManaStage::Color;
                self.state.rules.frame = Frame::ManaAbility {
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
            self.state
                .rules
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
        if let ManaProduction::AnyOneColor { amount } = &a.ability.production {
            let units = self.single_color_mana_units(a.player, color, amount)?;
            return self.produce_mana_units(a, units);
        }
        let symbols = match &a.ability.production {
            ManaProduction::Fixed { symbols }
            | ManaProduction::FixedWithLifeGain { symbols, .. }
            | ManaProduction::RestrictedFixed { symbols, .. } => symbols.clone(),
            ManaProduction::EachPermanentColor { .. } => {
                // CR 605.3b/608.2h: resolve once, after the activation costs
                // commit. The whole output is fixed before any mana is added.
                self.mana_colors(a.player, &a.ability.production)?
            }
            _ => color.into_iter().collect(),
        };
        self.produce_mana_units(
            a,
            symbols
                .into_iter()
                .map(|c| (c, Quantity::from(1)))
                .collect(),
        )
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
        let restriction = match a.ability.production {
            ManaProduction::RestrictedFixed { spend, .. } => Some(spend),
            _ => None,
        };
        for (color, amount) in units {
            if let Some(restriction) = restriction {
                restricted_mana::credit(
                    &mut receipt.produced_restricted,
                    &RestrictedMana {
                        restriction,
                        color,
                        amount: amount.clone(),
                    },
                );
            } else {
                receipt.produced.entry(color).or_default().add(&amount);
            }
            children.push(EventGroup::Occurrence {
                proposed: ProposedEvent::AddMana {
                    player: a.player,
                    color,
                    amount,
                    source,
                    restriction,
                },
            });
        }
        let life_index = if let ManaProduction::FixedWithLifeGain { life, .. } =
            &a.ability.production
        {
            let index = u32::try_from(children.len()).map_err(|_| "mana effect index overflow")?;
            children.push(EventGroup::Occurrence {
                proposed: ProposedEvent::GainLife {
                    player: a.player,
                    amount: life.clone(),
                },
            });
            Some(index)
        } else {
            None
        };
        if !children.is_empty() {
            let batch = self.commit_batch(cause, EventGroup::Ordered { children })?;
            if let Some(index) = life_index {
                receipt.life_gain = mana_life_gain::actual(&batch, index, a.player)?;
            }
        }
        if let Some(amount) = a.ability.self_damage.clone() {
            return self.begin_mana_damage(a, amount);
        }
        self.finish_mana_ability(a)
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
        self.resume_mana(*a.resume, a.receipts)
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
                    self.state.rules.passes.clear();
                }
            }
            _ => return Err("invalid mana continuation".into()),
        }
        self.state.rules.frame = resume;
        if matches!(self.state.rules.frame, Frame::Priority) {
            return self.stabilize();
        }
        self.revalidate_resolution_payment_departure()
    }
    pub(super) fn undo_mana_receipts(
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
                    decision_id: self.state.rules.revision,
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
        if let Some(life) = &r.life_gain {
            self.reverse_mana_life(r.player, life)?;
        }
        if let Some(damage) = &r.damage {
            self.reverse_mana_damage(r.player, damage)?;
        }
        let p = &mut self.state.rules.players[r.player.0 as usize];
        for unit in &r.produced_restricted {
            restricted_mana::debit(&mut p.restricted_mana, unit)?;
        }
        for unit in &r.spent_restricted {
            restricted_mana::credit(&mut p.restricted_mana, unit);
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
                self.state.rules.objects.insert(
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
                        lore_counters: 0.into(),
                    },
                );
                // CR 733.1/800.4c: restore only the recorded object, then
                // derive its controller from surviving exact-incarnation Auras.
                // Departed owners/sources remain absent; payment authority and
                // the receipt's mana/counter/use bookkeeping stay unchanged.
                self.reconcile_controllers()?;
            } else {
                let o = self
                    .state
                    .rules
                    .objects
                    .get_mut(&r.source.object.card)
                    .ok_or("missing surviving mana source")?;
                if o.id == r.source.object && o.zone == Zone::Battlefield {
                    if (r.ability_use.is_none() && !o.tapped)
                        || (r.branch.is_some() && o.plus_one_counters != r.counters)
                    {
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
            self.state.rules.mana_ability_uses.remove(&k);
            if r.previously_used {
                self.state.rules.mana_ability_uses.insert(k);
            }
        }
        self.state
            .rules
            .pending_triggers
            .retain(|t| !r.cost_triggers.contains(&t.id));
        Ok(EventOutcome::PermanentManaReversed {
            player: r.player,
            source: r.source.object,
        })
    }
    pub(super) fn revalidate_mana_departure(&mut self) -> Result<(), String> {
        if let Frame::DamageEffects { transaction } = &mut self.state.rules.frame
            && let DamageContinuation::Mana { departed, .. } = transaction.continuation.as_mut()
        {
            // Let the paid invocation finish before unwinding the suspended
            // payment. Source and recipient departures are handled by damage.
            *departed = true;
            return Ok(());
        }
        if let Frame::ManaSourceChoice {
            player,
            source,
            scope,
            resume,
        } = &self.state.rules.frame
        {
            let invalid = if *scope == ManaSourceScope::Mixed {
                match self.mixed_mana_choices(*player, *source) {
                    Err(_) => true,
                    Ok(choices) => {
                        let selected = self.state.rules.pending.as_ref().and_then(|input| {
                            match &input.response {
                                Response::ChooseManaAbility { ability } => Some(*ability),
                                _ => None,
                            }
                        });
                        if let Some(ability) = selected
                            && choices.contains(&ability)
                        {
                            false
                        } else {
                            self.state.rules.pending = None;
                            !uses_indexed_paid_mana(self.copiable_definition(*source)?)
                                && !self.has_granted_land_mana(*source)?
                                && (self.intrinsic_mana_colors(*source)?.is_empty()
                                    || !self.is_permanent_mana(*source))
                        }
                    }
                }
            } else {
                self.mana_source_colors(*player, *source, *scope).is_err()
            };
            if invalid {
                let resume = *resume.clone();
                self.state.rules.pending = None;
                self.state.rules.frame = resume;
            }
        }
        let mut f = &self.state.rules.frame;
        if let Frame::ManaSourceChoice { resume, .. } = f {
            f = resume;
        }
        let mut invalid = false;
        while let Frame::ManaAbility { activation: a } = f {
            invalid |= !self.is_live(a.player)
                || (a.paid.is_none()
                    && self
                        .validate_permanent_mana_costs(a.player, a.source, a.ability_index())
                        .is_err());
            f = &a.resume;
        }
        if let Frame::Mana { cast } = f {
            invalid |= self.state.rules.stack.last().is_none_or(|top| {
                top.id != cast.stack_id || self.announced_target_legality(top).iter().any(|v| !*v)
            });
        }
        if invalid {
            if let Frame::ManaSourceChoice { resume, .. } = self.state.rules.frame.clone() {
                self.state.rules.frame = *resume;
            }
            while let Frame::ManaAbility { activation } = self.state.rules.frame.clone() {
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
                        Frame::Priority if !a.receipts.is_empty() => {
                            self.state.rules.passes.clear()
                        }
                        _ => {}
                    }
                }
                self.state.rules.frame = *a.resume;
            }
            self.state.rules.pending = None;
        } else if let Frame::ManaAbility { activation } = &self.state.rules.frame
            && activation.stage == ManaStage::Color
            && activation.ability.production == ManaProduction::OpponentLandColors
            && self
                .opponent_land_mana_colors(activation.player)?
                .is_empty()
        {
            let activation = *activation.clone();
            self.state.rules.pending = None;
            return self.resolve_mana_ability(activation, None);
        }
        self.revalidate_resolution_payment_departure()
    }
    pub(super) fn validate_nested_mana_frame(&self, frame: &Frame) -> Result<(), String> {
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
                ManaSourceScope::Intrinsic if self.catalog_has_land_type_grant => self
                    .mana_source_colors(*player, *source, *scope)?
                    .is_empty(),
                _ => self.mana_source_colors(*player, *source, *scope)?.len() < 2,
            } || !self.mana_resume_player(resume, *player)
            {
                return Err("mana source choice snapshot invariant".into());
            }
            sources.insert(*source);
            f = resume;
        }
        while let Frame::ManaAbility { activation: a } = f {
            if !self.is_live(a.player)
                || !sources.insert(a.source)
                || a.branch.is_some()
                || !a.validate_filter_stage(self.copiable_definition(a.source)?)
                || !self.mana_resume_player(&a.resume, a.player)
                || matches!(a.stage, ManaStage::Color | ManaStage::Combination { .. })
                    != a.paid.is_some()
                || matches!(&a.stage,ManaStage::Combination{total} if *total<=Quantity::zero() || a.paid.as_ref().and_then(|receipt| receipt.combination_total.as_ref()) != Some(total) || (a.ability.production != ManaProduction::GreatestControlledPowerCombination && a.filter_colors().is_none()))
                || (a.stage == ManaStage::Color
                    && a.ability.production == ManaProduction::GreatestControlledPowerCombination)
                || a.receipts.iter().any(|r| r.player != a.player)
                || (a.stage == ManaStage::Color
                    && a.ability.production == ManaProduction::OpponentLandColors
                    && self.opponent_land_mana_colors(a.player)?.is_empty())
            {
                return Err("nested mana snapshot invariant".into());
            }
            f = &a.resume;
        }
        if matches!(f, Frame::ManaSourceChoice { .. }) {
            return Err("buried mana source choice continuation".into());
        }
        Ok(())
    }
}

#[cfg(test)]
mod source_choice_tests {
    use super::*;

    #[test]
    fn mana_source_selection_preserves_ability_identity_for_overlapping_outputs() {
        let intrinsic = ManaSourceChoice::Intrinsic(Color::G);
        let free = ManaSourceChoice::Printed(ManaBranch {
            index: 3,
            color: Color::U,
        });
        let paid = ManaSourceChoice::Printed(ManaBranch {
            index: 1,
            color: Color::G,
        });
        assert_eq!(
            ManaSourceChoice::from_color(&[paid, free], Color::G),
            Ok(paid)
        );
        assert_eq!(
            ManaSourceChoice::from_color(&[intrinsic, free], Color::U),
            Ok(free)
        );
        assert!(ManaSourceChoice::from_color(&[intrinsic, paid], Color::G).is_err());
        assert!(ManaSourceChoice::from_color(&[intrinsic, free], Color::W).is_err());
    }
}
