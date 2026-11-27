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
    /// CR 107.4e, 602.2b, 605.3a: hybrid symbols are announced before the
    /// nested payment.
    Hybrid,
    Payment,
    Spend,
    Color,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ManaActivation {
    pub(super) player: PlayerId,
    pub source: ObjectId,
    /// CR 605.3a: which of the source's printed (or, when `granted`,
    /// granted) mana abilities this is.
    #[serde(default, skip_serializing_if = "is_zero")]
    index: u32,
    /// CR 113.10a: the ability is one a battlefield static grants the source.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    granted: bool,
    ability: ManaAbility,
    /// CR 608.2c: the next rider to perform once the mana is added.
    #[serde(default, skip_serializing_if = "is_zero")]
    rider: u32,
    decision_id: u64,
    pub stage: ManaStage,
    /// The announced alternative of each printed hybrid symbol, in order.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    hybrid: Vec<HybridPayment>,
    pub(super) resume: FrameLink,
    receipts: Vec<ManaActivationUndo>,
    paid: Option<Box<PermanentManaUndo>>,
}
impl ManaActivation {
    /// CR 601.2f, 602.2b: the fixed mana cost plus each announced hybrid
    /// color.
    fn payment_cost(&self) -> Option<ManaCost> {
        if self.ability.hybrid_mana.is_empty() {
            return self.ability.costs.mana.clone();
        }
        let mut cost = self
            .ability
            .costs
            .mana
            .clone()
            .unwrap_or_else(zero_mana_cost);
        for payment in &self.hybrid {
            payment.add_to(&mut cost);
        }
        Some(cost)
    }

    /// The announcement names one allowed alternative per printed symbol,
    /// and exists exactly after the hybrid stage.
    fn hybrid_bound(&self) -> bool {
        let symbols = &self.ability.hybrid_mana;
        if self.stage == ManaStage::Hybrid || symbols.is_empty() {
            return self.hybrid.is_empty()
                && (self.stage == ManaStage::Hybrid) != symbols.is_empty();
        }
        // CR 107.4e, 605.3a: only two-color hybrid is admitted on a mana
        // ability; any other alternative is an invariant failure.
        symbols.len() == self.hybrid.len()
            && symbols.iter().zip(&self.hybrid).all(|(symbol, payment)| {
                payment.color().is_some()
                    && symbol.colors.contains(payment)
                    && symbol.colors.iter().all(|c| c.color().is_some())
            })
    }

    /// The printed ability's index; none for a granted one.
    pub(crate) fn printed_index(&self) -> Option<u32> {
        (!self.granted).then_some(self.index)
    }
    fn slot(&self) -> ManaSlot {
        if self.granted {
            ManaSlot::Granted(self.index)
        } else {
            ManaSlot::Printed(self.index)
        }
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

/// CR 113.10a, 605.3a: one of an object's current mana abilities: printed on
/// it, or granted by a battlefield static (in the granting effects'
/// timestamp order, CR 613.7a).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ManaSlot {
    Printed(u32),
    Granted(u32),
}
impl ManaSlot {
    pub(crate) fn identity(self) -> ManaAbilityIdentity {
        match self {
            Self::Printed(index) => ManaAbilityIdentity::Printed { index },
            Self::Granted(index) => ManaAbilityIdentity::Granted { index },
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
        /// CR 122.1: the source's other counter kinds, restored exactly.
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        pub other_counters: Vec<CounterStack>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub ability_use: Option<ManaAbilityUse>,
        #[serde(default, skip_serializing_if = "std::ops::Not::not")]
        pub previously_used: bool,
        /// CR 602.5, 605.3a: printed mana ability `index` read its
        /// "Activate only if" condition as the activation began.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub condition: Option<ManaConditionEvidence>,
        pub produced: BTreeMap<Color, Quantity>,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        pub produced_restricted: Vec<RestrictedMana>,
        pub spent: BTreeMap<Color, Quantity>,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        pub spent_restricted: Vec<RestrictedMana>,
        pub absent: BTreeSet<Color>,
        pub sacrificed: bool,
        /// CR 733.1: every actual its costs published. A reversal removes
        /// the triggers and watcher firings of exactly these occurrences.
        pub cost_events: Vec<EventId>,
        /// CR 733.1: the activation paid life or performed riders, so it is
        /// kept rather than reversed.
        #[serde(default, skip_serializing_if = "std::ops::Not::not")]
        pub kept: bool,
        /// CR 733.1: the ledger change its costs published, whose exact
        /// inverse a reversal applies.
        #[serde(default, skip_serializing_if = "LedgerDelta::is_empty")]
        pub ledger: LedgerDelta,
    }

}

/// The evidence a conditioned mana ability's receipt keeps.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ManaConditionEvidence {
    pub index: u32,
    pub check: super::stack_costs::ActivationConditionCheck,
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
    /// CR 113.10a, 613.1f: the mana abilities battlefield statics grant
    /// `object` now, with each granting object, in timestamp order.
    pub(crate) fn granted_mana_abilities(&self, object: ObjectId) -> Vec<(ObjectId, &ManaAbility)> {
        self.granted_mana_with_sources(object, None)
    }
    pub(super) fn granted_mana_with_sources<'a>(
        &'a self,
        object: ObjectId,
        sources: Option<&'a [CardId]>,
    ) -> Vec<(ObjectId, &'a ManaAbility)> {
        if self.current(object).is_ok_and(|o| o.hideaway.is_some()) {
            return vec![];
        }
        use mtg_contracts::kir::Change;
        let changes =
            self.static_changes_keyed(object, mtg_contracts::kir::Layer::Ability, sources);
        // CR 613.7: a later removal of every ability takes an earlier grant,
        // except the grants of the removing effect itself.
        // CR 613.7m: simultaneous sources go in APNAP order; a removal
        // precedes a grant of the same rank.
        let order = |time: u64, key: &ability_layers::StaticKey| {
            (time, self.current(key.source).map_or(0, |o| o.keyword_rank))
        };
        let removals: Vec<_> = changes
            .iter()
            .filter(|(_, _, c)| matches!(c, Change::RemoveAllAbilities { .. }))
            .map(|(time, key, _)| (order(*time, key), *key))
            .collect();
        changes
            .into_iter()
            .filter(|(time, key, _)| {
                removals
                    .iter()
                    .all(|(removed, remover)| remover == key || *removed <= order(*time, key))
            })
            .flat_map(|(_, key, change)| match change {
                Change::AddManaAbilities { abilities } => {
                    abilities.iter().map(|a| (key.source, a)).collect()
                }
                _ => vec![],
            })
            .collect()
    }
    /// The printed mana abilities that still function (CR 113.10, 305.7).
    fn printed_mana_abilities(&self, object: ObjectId) -> Result<&[ManaAbility], String> {
        let definition = self.copiable_definition(object)?;
        Ok(if self.printed_abilities_lost(object) {
            &[]
        } else {
            &definition.mana_abilities
        })
    }
    /// The object's current mana abilities: printed, then granted.
    pub(crate) fn current_mana_abilities(
        &self,
        object: ObjectId,
    ) -> Result<Vec<(ManaSlot, ManaAbility)>, String> {
        let mut out = vec![];
        for (index, a) in self.printed_mana_abilities(object)?.iter().enumerate() {
            let index = u32::try_from(index).map_err(|_| "mana ability index overflow")?;
            out.push((ManaSlot::Printed(index), a.clone()));
        }
        for (index, (_, a)) in self.granted_mana_abilities(object).into_iter().enumerate() {
            let index = u32::try_from(index).map_err(|_| "mana ability index overflow")?;
            out.push((ManaSlot::Granted(index), a.clone()));
        }
        Ok(out)
    }
    fn mana_ability_at(&self, source: ObjectId, slot: ManaSlot) -> Result<ManaAbility, String> {
        match slot {
            ManaSlot::Printed(index) => self
                .printed_mana_abilities(source)?
                .get(index as usize)
                .cloned(),
            ManaSlot::Granted(index) => self
                .granted_mana_abilities(source)
                .get(index as usize)
                .map(|(_, a)| (*a).clone()),
        }
        .ok_or_else(|| "missing mana ability".into())
    }
    /// The activation is still the source's current ability at its slot;
    /// admitted grants cannot change while their activation is open.
    fn activation_matches(&self, a: &ManaActivation) -> Result<bool, String> {
        Ok(
            self.mana_ability_at(a.source, a.slot()).ok().as_ref() == Some(&a.ability)
                && a.rider as usize <= a.ability.riders.len(),
        )
    }
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
    /// Snapshot validation of a permanent mana receipt's condition evidence
    /// against the printed ability it names.
    pub(super) fn validate_mana_condition_receipt(
        &self,
        player: PlayerId,
        source: ObjectId,
        receipt: &PermanentManaUndo,
    ) -> Result<(), String> {
        if !restricted_mana::valid_buckets(&receipt.produced_restricted)
            || !restricted_mana::valid_buckets(&receipt.spent_restricted)
        {
            return Err("invalid restricted mana receipt".into());
        }
        // CR 122.1, 733.1: reversal restores exactly the counters the source
        // had as the activation began, which its captured facts also hold.
        // A ceased token keeps no definition to compare its identity with.
        if receipt.source.object != source {
            return Err("mana receipt source mismatch".into());
        }
        Self::validate_fact_counters(&receipt.source)?;
        if self.definition(source.card).is_ok() {
            self.validate_fact_identity(&receipt.source)?;
        } else if self.current(source).is_ok() {
            return Err("unknown mana receipt source".into());
        }
        // The source was on the battlefield and phased in as the activation
        // began, so it held no hideaway link or Adventure permission, and
        // its loyalty use and attachment are from this game's past.
        let row = &receipt.source;
        if row.zone != Zone::Battlefield
            || row.phased_out.is_some()
            || row.hideaway.is_some()
            || row.adventure_player.is_some()
            || row
                .loyalty_used_turn
                .is_some_and(|turn| turn == 0 || turn > self.state.rules.turn.number)
            || row.attached_to.is_some() != row.attachment_timestamp.is_some()
            || row.attachment_timestamp.is_some_and(|timestamp| {
                timestamp == 0 || timestamp >= self.state.rules.next_attachment_timestamp
            })
        {
            return Err("invalid mana receipt source row".into());
        }
        if receipt.counters.is_negative()
            || receipt.counters != receipt.source.plus_one_counters
            || receipt.other_counters != receipt.source.counters
            || receipt
                .other_counters
                .iter()
                .any(|stack| stack.timestamp >= self.state.rules.next_batch_id)
        {
            return Err("invalid mana receipt counters".into());
        }
        // A sacrifice can record a departure or death. Each recorded fact
        // still holds, and the cost events name published batches.
        if !receipt.sacrificed && !receipt.ledger.is_empty()
            || receipt.ledger.seats.iter().any(|row| {
                row.artifacts_entered != 0
                    || row.creatures_entered != 0
                    || row.nonlands_entered != 0
                    || row.tokens_created != 0
            })
            || receipt
                .ledger
                .check_holds(&self.state.rules.cast_history)
                .is_err()
            || !receipt.cost_events.windows(2).all(|w| w[0] < w[1])
            || receipt
                .cost_events
                .iter()
                .any(|e| e.occurrence.batch >= self.state.rules.next_batch_id)
        {
            return Err("invalid mana receipt turn history".into());
        }
        let Some(evidence) = &receipt.condition else {
            return Ok(());
        };
        // A token that ceased to exist after its activation keeps no
        // definition to compare with; any other source does.
        let definition = match self.copiable_definition(source) {
            Ok(definition) => definition,
            Err(_) if self.current(source).is_err() => return Ok(()),
            Err(e) => return Err(e),
        };
        let condition = definition
            .mana_abilities
            .get(evidence.index as usize)
            .and_then(|a| a.condition.as_ref())
            .ok_or("snapshot activation condition evidence invariant")?;
        self.validate_activation_condition_check(player, Some(condition), Some(&evidence.check))
    }
    /// Every permanent receipt's condition evidence in one held list: a
    /// spell's mana window, a resolution payment or a nested mana ability.
    pub(super) fn validate_mana_receipt_conditions(
        &self,
        receipts: &[ManaActivationUndo],
    ) -> Result<(), String> {
        for r in receipts {
            if let Some(p) = &r.permanent {
                self.validate_mana_condition_receipt(r.player, r.object, p)?;
            }
        }
        Ok(())
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
        self.printed_mana_abilities(object)
            .is_ok_and(|abilities| !abilities.is_empty())
            || !self.granted_mana_abilities(object).is_empty()
    }
    pub(super) fn mana_colors(
        &self,
        player: PlayerId,
        production: &ManaProduction,
    ) -> Result<Vec<Color>, String> {
        Ok(match production {
            ManaProduction::Fixed { symbols } => symbols.clone(),
            ManaProduction::Counted {
                color,
                filter,
                zone,
                ..
            } => {
                if self.mana_count(player, zone, filter) == 0 {
                    vec![]
                } else {
                    vec![*color]
                }
            }
            ManaProduction::Bundles { options } => options.iter().flatten().copied().collect(),
            ManaProduction::AnyColor | ManaProduction::ChooseDevotion {} => {
                vec![Color::W, Color::U, Color::B, Color::R, Color::G]
            }
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
            // CR 106.7: granted abilities are abilities the land has.
            for (_, ability) in self.current_mana_abilities(o.id)? {
                match &ability.production {
                    ManaProduction::LandsCouldProduce {
                        controller,
                        colors_only,
                    } => reads.push((key, *controller, *colors_only)),
                    ManaProduction::ChooseDevotion {} => {
                        for color in self.mana_colors(o.controller, &ability.production)? {
                            if self
                                .devotion(o.controller, &[color])
                                .is_some_and(|n| n > Quantity::zero())
                            {
                                types.insert(color);
                            }
                        }
                    }
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
                    DamageContinuation::Combat { .. }
                    | DamageContinuation::Instruction { .. }
                    | DamageContinuation::Kernel { .. } => {
                        break;
                    }
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
        self.validate_permanent_mana_costs(player, source, self.only_mana_slot(source)?)
    }

    /// The single mana ability of a source that offers no selection.
    fn only_mana_slot(&self, source: ObjectId) -> Result<ManaSlot, String> {
        Ok(self
            .current_mana_abilities(source)?
            .first()
            .ok_or("unsupported permanent mana source")?
            .0)
    }

    /// CR 605.3a: the player chooses which mana ability of the source to
    /// activate when it has an intrinsic one (CR 305.6) or several printed
    /// or granted ones (CR 113.10a).
    fn mana_ability_selection(&self, source: ObjectId) -> Result<bool, String> {
        let current = self.current_mana_abilities(source)?.len();
        if current == 0 {
            return Err("unsupported permanent mana source".into());
        }
        Ok(current > 1 || !self.intrinsic_mana_colors(source)?.is_empty())
    }

    fn validate_permanent_mana_costs(
        &self,
        player: PlayerId,
        source: ObjectId,
        slot: ManaSlot,
    ) -> Result<(), String> {
        let o = self.current(source)?;
        let a = &self.mana_ability_at(source, slot)?;
        let index = match slot {
            ManaSlot::Printed(index) => Some(index),
            ManaSlot::Granted(_) => None,
        };
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
            || (a.restriction.is_some()
                && index.is_none_or(|index| self.mana_ability_index_used(source, index)))
            || a.condition
                .as_ref()
                .is_some_and(|c| self.activation_condition_check(player, source, c).is_err())
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
        let mut choices: Vec<_> = self
            .intrinsic_choice_colors(player, source)
            .unwrap_or_default()
            .into_iter()
            .map(|color| ManaAbilityIdentity::Intrinsic { color })
            .collect();
        for (slot, _) in self.current_mana_abilities(source)? {
            if self
                .validate_permanent_mana_costs(player, source, slot)
                .is_ok()
            {
                choices.push(slot.identity());
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
                    self.start_mana_ability(&input, source, ManaSlot::Printed(index), resume)
                }
                ManaAbilityIdentity::Granted { index } => {
                    self.start_mana_ability(&input, source, ManaSlot::Granted(index), resume)
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
            let slot = self.only_mana_slot(source)?;
            self.start_mana_ability(input, source, slot, resume)
        }
    }

    fn start_mana_ability(
        &mut self,
        input: &AcceptedInput,
        source: ObjectId,
        slot: ManaSlot,
        resume: Frame,
    ) -> Result<(), String> {
        let ability = self.mana_ability_at(source, slot)?;
        let (index, granted) = match slot {
            ManaSlot::Printed(index) => (index, false),
            ManaSlot::Granted(index) => (index, true),
        };
        let activation = ManaActivation {
            player: input.player,
            source,
            index,
            granted,
            rider: 0,
            decision_id: input.decision_id,
            stage: if ability.hybrid_mana.is_empty() {
                ManaStage::Payment
            } else {
                ManaStage::Hybrid
            },
            ability,
            hybrid: vec![],
            resume: self.save_frame(resume),
            receipts: vec![],
            paid: None,
        };
        if activation.payment_cost().is_none() {
            self.pay_mana_ability(activation, BTreeMap::new(), vec![])
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
            ManaStage::Hybrid => Ok(self.hybrid_pick(a.player, &a.ability.hybrid_mana)),
            ManaStage::Payment | ManaStage::Spend => self.payment_decision(
                a.player,
                a.payment_cost().ok_or("missing mana ability cost")?,
                Some(a.source),
                a.stage == ManaStage::Payment,
                self.activation_mana_use(a.source, None)?,
            ),
            ManaStage::Color => {
                let candidates = match &a.ability.production {
                    ManaProduction::Bundles { options } => options
                        .iter()
                        .cloned()
                        .map(|symbols| Candidate::ManaBundle { symbols })
                        .collect(),
                    production => self
                        .mana_colors(a.player, production)?
                        .into_iter()
                        .map(|color| Candidate::Color { color })
                        .collect(),
                };
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
            Answer::Pick { .. } if a.stage == ManaStage::Hybrid => {
                if input.player != a.player {
                    return Err("hybrid announcement player invariant".into());
                }
                a.hybrid = input.hybrid_colors()?;
                a.stage = ManaStage::Payment;
                if !a.hybrid_bound() {
                    return Err(
                        "hybrid announcement requires one allowed color per printed symbol".into(),
                    );
                }
                self.rules_mut().frame = Frame::ManaAbility {
                    activation: Box::new(a),
                };
            }
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
                if self.can_pay_for(a.player, &cost, self.activation_mana_use(a.source, None)?)?
                    && self
                        .validate_permanent_mana_costs(a.player, a.source, a.slot())
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
            Answer::Pay { pay } if a.stage == ManaStage::Spend => {
                let (unrestricted, restricted) =
                    pay.spending().ok_or("expected mana allocation")?;
                self.pay_mana_ability(a, unrestricted.clone(), restricted.to_vec())?;
            }
            Answer::Pick { .. } if a.stage == ManaStage::Color => match input.one()? {
                Candidate::Color { color } => self.resolve_mana_ability(a, Some(*color))?,
                Candidate::ManaBundle { symbols } => {
                    self.produce_mana_units(a, symbols.iter().map(|&c| (c, 1.into())).collect())?;
                }
                _ => return Err("invalid mana production choice".into()),
            },
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
        self.validate_permanent_mana_costs(a.player, a.source, a.slot())?;
        // No costs are paid until the nested payment succeeds (601.2g-h).
        let pool = &self.state.rules.players[a.player.0 as usize].mana;
        let mut receipt = PermanentManaUndo {
            player: a.player,
            source: self.object_before(a.source)?,
            counters: self.current(a.source)?.plus_one_counters.clone(),
            other_counters: self.current(a.source)?.counters.clone(),
            ability_use: a
                .ability
                .restriction
                .filter(|_| !a.granted)
                .map(|_| self.mana_use_key(a.source, a.index)),
            previously_used: !a.granted && self.mana_ability_index_used(a.source, a.index),
            condition: a
                .ability
                .condition
                .as_ref()
                .map(|c| {
                    self.activation_condition_check(a.player, a.source, c)
                        .map(|check| ManaConditionEvidence {
                            index: a.index,
                            check,
                        })
                })
                .transpose()?,
            produced: BTreeMap::new(),
            produced_restricted: vec![],
            spent: spending.clone(),
            spent_restricted: restricted.clone(),
            absent: [Color::W, Color::U, Color::B, Color::R, Color::G, Color::C]
                .into_iter()
                .filter(|c| !pool.contains_key(c))
                .collect(),
            sacrificed: a.ability.costs.sacrifice,
            cost_events: vec![],
            kept: !a.reversible(),
            ledger: LedgerDelta::default(),
        };
        let cause = a.cause();
        if let Some(cost) = a.payment_cost() {
            self.validate_spending(
                a.player,
                &cost,
                self.activation_mana_use(a.source, None)?,
                &spending,
                &restricted,
            )?;
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
        let history = self.state.rules.cast_history.clone();
        let ((), journal) = self.with_actual_journal(|txn| {
            txn.pay_permanent_source_costs(a.source, a.player, &a.ability.costs, cause)
        })?;
        // CR 733.1: `kept` is the rules flag set above from the ability
        // alone, never inferred from what its costs wrote. A reversible
        // activation keeps the ledger change its costs published, and a
        // reversal applies its exact inverse; a reversible cost that changed
        // any turn fact outside that delta has no exact inverse and is refused.
        let mut expected = history;
        journal.ledger.apply(&mut expected)?;
        if !receipt.kept && expected != self.state.rules.cast_history {
            return Err("reversible mana cost changed an unrecorded turn fact".into());
        }
        receipt.ledger = journal.ledger;
        receipt.cost_events = journal.events;
        receipt.cost_events.sort();
        receipt.cost_events.dedup();
        if let Some(k) = receipt.ability_use {
            self.rules_mut().mana_ability_uses.insert(k);
        }
        a.paid = Some(Box::new(receipt));
        match a.ability.production {
            ManaProduction::Fixed { .. } | ManaProduction::Counted { .. } => {
                self.resolve_mana_ability(a, None)
            }
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
                .pay_counters(&mtg_contracts::kir::CounterKind::PlusOnePlusOne, n)?;
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
    fn mana_count(
        &self,
        player: PlayerId,
        zone: &mtg_contracts::kir::ZoneExpr,
        filter: &mtg_contracts::kir::Pred,
    ) -> usize {
        self.state
            .rules
            .objects
            .values()
            .filter(|object| {
                crate::query::matches_zone_pred(&self.object_facts(object), player, zone, filter)
                    .is_yes()
            })
            .count()
    }

    fn resolve_mana_ability(
        &mut self,
        a: ManaActivation,
        color: Option<Color>,
    ) -> Result<(), String> {
        let units = match &a.ability.production {
            ManaProduction::Counted {
                color,
                factor,
                filter,
                zone,
            } => {
                let count = self.mana_count(a.player, zone, filter);
                let mut amount = Quantity::from(
                    i64::try_from(count).map_err(|_| "mana count exceeds object limit")?,
                );
                amount.multiply(factor);
                if amount == Quantity::zero() {
                    vec![]
                } else {
                    vec![(*color, amount)]
                }
            }
            ManaProduction::Fixed { symbols } => {
                symbols.iter().map(|c| (*c, Quantity::from(1))).collect()
            }
            ManaProduction::ChooseDevotion {} => {
                let color = color.ok_or("devotion mana requires a chosen color")?;
                let amount = self
                    .devotion(a.player, &[color])
                    .ok_or("missing devotion characteristics")?;
                if amount == Quantity::zero() {
                    vec![]
                } else {
                    vec![(color, amount)]
                }
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
            let restriction = a.ability.spend_restriction;
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
        if let frame @ Frame::TurnFaceUp { .. } = self.saved_frame(a.resume)? {
            self.rules_mut().frame = frame.clone();
            return self.revalidate_face_up_departure();
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
                Frame::TurnFaceUp { payment } => {
                    payment.player == a.player && payment.stage == face_down::FaceUpStage::Mana
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
            || !self.activation_matches(a)?
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
            Frame::TurnFaceUp { payment } if payment.stage == face_down::FaceUpStage::Mana => {
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
        self.revalidate_face_up_departure()?;
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
        p.restricted_mana =
            mtg_view::subtract_restricted_mana(p.restricted_mana.clone(), &r.produced_restricted)?;
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
                // CR 733.1: the exact row the activation began with,
                // including loyalty, its use, attachment, transforms and
                // face-down status.
                self.rules_mut()
                    .objects
                    .insert(r.source.object.card, r.source.restored_object());
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
                    o.counters = r.other_counters.clone();
                }
                // CR 400.7/733: a departed source is never restored or rebound.
            }
        }
        // Checked before any write, so a stale receipt changes nothing here.
        r.ledger.revert(&mut self.rules_mut().cast_history)?;
        if let Some(k) = r.ability_use {
            self.rules_mut().mana_ability_uses.remove(&k);
            if r.previously_used {
                self.rules_mut().mana_ability_uses.insert(k);
            }
        }
        self.undo_resolution_departure(&r.cost_events);
        // A reversed cost's triggers never triggered, so their once-each-turn
        // uses are released with them (CR 733.1). Undo synthesizes nothing.
        let released = self
            .state
            .rules
            .pending_triggers
            .iter()
            .filter(|t| t.caused_by_any(&r.cost_events))
            .filter_map(Self::trigger_use)
            .collect::<Vec<_>>();
        for key in released {
            self.rules_mut().trigger_uses.remove(&key);
        }
        self.rules_mut()
            .pending_triggers
            .retain(|t| !t.caused_by_any(&r.cost_events));
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
                        .validate_permanent_mana_costs(a.player, a.source, a.slot())
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
                || !self.activation_matches(a)?
                || !self.mana_resume_player(self.saved_frame(a.resume)?, a.player)
                || (a.stage == ManaStage::Color) != a.paid.is_some()
                || !a.hybrid_bound()
                || (a.stage == ManaStage::Hybrid && !a.receipts.is_empty())
                || a.receipts.iter().any(|r| r.player != a.player)
            {
                return Err("nested mana snapshot invariant".into());
            }
            self.validate_mana_receipt_conditions(&a.receipts)?;
            // The suspended ability's own receipt names this exact printed
            // ability: it carries evidence exactly when that ability has a
            // condition.
            if let Some(paid) = &a.paid {
                if a.ability.condition.is_some()
                    != paid.condition.as_ref().is_some_and(|e| e.index == a.index)
                {
                    return Err("snapshot activation condition evidence invariant".into());
                }
                self.validate_mana_condition_receipt(a.player, a.source, paid)?;
            }
            f = self.saved_frame(a.resume)?;
        }
        if matches!(f, Frame::ManaSourceChoice { .. }) {
            return Err("buried mana source choice continuation".into());
        }
        Ok(())
    }
}
