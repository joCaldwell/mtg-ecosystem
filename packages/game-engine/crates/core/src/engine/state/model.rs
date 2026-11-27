//! The objects a rules state is made of: identities, zones, players,
//! physical cards, object rows and stack entries.
use crate::engine::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ObjectId {
    pub card: CardId,
    pub incarnation: u32,
    #[serde(default, skip_serializing_if = "CardFace::is_front")]
    pub face: CardFace,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Player {
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub life_frozen_until_next_turn: bool,
    #[serde(default, skip_serializing_if = "tokens::quantity_is_zero")]
    pub poison_counters: Quantity,
    #[serde(default, skip_serializing_if = "tokens::quantity_is_zero")]
    pub energy_counters: Quantity,
    /// CR 503.1a, 702.30a: how many of this player's upkeep steps have begun.
    #[serde(default, skip_serializing_if = "is_zero_u64")]
    pub upkeeps: u64,
    pub failed_draw: bool,
    pub life: Quantity,
    pub mana: BTreeMap<Color, Quantity>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub restricted_mana: Vec<RestrictedMana>,
    pub status: PlayerStatus,
    pub last_known: Option<PlayerLastKnown>,
}
impl Player {
    pub fn can_pay_life(&self, amount: &Quantity) -> bool {
        *amount == Quantity::zero()
            || (*amount > Quantity::zero()
                && !self.life_frozen_until_next_turn
                && self.life >= *amount)
    }
    // CR 119.8, 120.3a: damage still occurs at its full amount. Only the
    // resulting life change is prohibited.
    pub(in crate::engine) fn lose_life(&mut self, amount: &Quantity) -> (Quantity, Quantity) {
        let previous = self.life.clone();
        if !self.life_frozen_until_next_turn {
            self.life.subtract(amount);
        }
        (previous, self.life.clone())
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Card {
    pub owner: PlayerId,
    pub oracle_id: String,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Object {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hideaway: Option<Box<HideawayLink>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub phased_out: Option<PhaseReturn>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub attached_to: Option<ObjectId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub attachment_timestamp: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(in crate::engine) keyword_timestamp: Option<u64>,
    /// CR 613.7m: the relative order of objects timestamped together, fixed
    /// when the timestamp is assigned: the controller's APNAP position then.
    #[serde(default, skip_serializing_if = "is_zero_rank")]
    pub(in crate::engine) keyword_rank: u8,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(in crate::engine) battlefield_entry_turn: Option<u64>,
    /// CR 702.30a: on the battlefield, the controller's upkeep count when
    /// this permanent last came under that player's control.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(in crate::engine) control_upkeeps: Option<u64>,
    #[serde(default, skip_serializing_if = "tokens::quantity_is_zero")]
    pub plus_one_counters: Quantity,
    #[serde(default, skip_serializing_if = "tokens::quantity_is_zero")]
    pub loyalty_counters: Quantity,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub loyalty_used_turn: Option<u64>,
    /// CR 122.1, 613.7c: the other counter kinds on this permanent, each
    /// positive, in kind order. A zone change starts a new object without
    /// them (CR 122.2, 400.7).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub counters: Vec<CounterStack>,
    /// CR 701.27a, 712.18: how many times this permanent has transformed
    /// since it entered; an odd count has its other face up. A zone change
    /// starts a new object at zero (CR 400.7).
    #[serde(default, skip_serializing_if = "is_zero_count")]
    pub transforms: u32,
    /// CR 715.3d: the player who may play this exiled card because it was
    /// exiled as a resolving Adventure. Leaving exile ends the permission.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub adventure_player: Option<PlayerId>,
    /// CR 708.2a, 702.37c: this spell or permanent is face down, a 2/2
    /// creature with no text, name, subtypes or mana cost. A zone change
    /// other than resolving onto the battlefield turns it face up (CR 708.9).
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub face_down: bool,
    pub marked_damage: Quantity,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub deathtouch_damage: bool,
    /// CR 702.84a: this incarnation returned by its unearth ability; it has
    /// haste and is exiled instead of leaving the battlefield.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub(in crate::engine) unearthed: bool,
    /// CR 702.109a: this incarnation is the permanent a spell whose dash
    /// cost was paid became; it has haste.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub(in crate::engine) dashed: bool,
    pub attack_ready: bool,
    pub id: ObjectId,
    pub controller: PlayerId,
    pub default_controller: PlayerId,
    pub zone: Zone,
    pub tapped: bool,
}
impl StackInstance {
    /// CR 113.7a: the face whose program this spell or ability runs.
    pub fn program_face(&self) -> CardFace {
        if self.kind.is_spell() {
            self.source.face
        } else {
            self.source_face
        }
    }
}
/// CR 122.1, 613.7c: every counter of one kind other than +1/+1 on a
/// permanent shares the timestamp of the latest one put on it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CounterStack {
    pub kind: mtg_contracts::kir::CounterKind,
    pub count: Quantity,
    pub timestamp: u64,
}
impl Object {
    /// CR 122.1: how many counters of `kind` this object has.
    pub fn counter_count(&self, kind: &mtg_contracts::kir::CounterKind) -> Quantity {
        if *kind == mtg_contracts::kir::CounterKind::PlusOnePlusOne {
            return self.plus_one_counters.clone();
        }
        if *kind == mtg_contracts::kir::CounterKind::Loyalty {
            return self.loyalty_counters.clone();
        }
        self.counters
            .iter()
            .find(|stack| stack.kind == *kind)
            .map_or_else(Quantity::zero, |stack| stack.count.clone())
    }
    /// CR 122.1: the kinds of counters this object has, in kind order.
    pub fn counter_kinds(&self) -> impl Iterator<Item = mtg_contracts::kir::CounterKind> + '_ {
        (self.plus_one_counters > Quantity::zero())
            .then_some(mtg_contracts::kir::CounterKind::PlusOnePlusOne)
            .into_iter()
            .chain(
                (self.loyalty_counters > Quantity::zero())
                    .then_some(mtg_contracts::kir::CounterKind::Loyalty),
            )
            .chain(self.counters.iter().map(|stack| stack.kind.clone()))
    }
    /// CR 122.1, 613.7c: add `amount` counters of `kind`; every counter of
    /// that kind takes `timestamp`. Returns the previous count.
    pub(in crate::engine) fn add_counters(
        &mut self,
        kind: &mtg_contracts::kir::CounterKind,
        amount: &Quantity,
        timestamp: u64,
    ) -> Quantity {
        if *kind == mtg_contracts::kir::CounterKind::PlusOnePlusOne {
            let previous = self.plus_one_counters.clone();
            self.plus_one_counters.add(amount);
            return previous;
        }
        if *kind == mtg_contracts::kir::CounterKind::Loyalty {
            let previous = self.loyalty_counters.clone();
            self.loyalty_counters.add(amount);
            return previous;
        }
        match self.counters.binary_search_by(|stack| stack.kind.cmp(kind)) {
            Ok(i) => {
                let stack = &mut self.counters[i];
                let previous = stack.count.clone();
                stack.count.add(amount);
                stack.timestamp = timestamp;
                previous
            }
            Err(i) => {
                self.counters.insert(
                    i,
                    CounterStack {
                        kind: kind.clone(),
                        count: amount.clone(),
                        timestamp,
                    },
                );
                Quantity::zero()
            }
        }
    }
    /// CR 122.1: an effect removes as many of `amount` counters of `kind` as
    /// the object has (never fewer than zero remain); returns the previous
    /// count. A cost uses [`Self::pay_counters`] instead.
    pub(in crate::engine) fn remove_counters(
        &mut self,
        kind: &mtg_contracts::kir::CounterKind,
        amount: &Quantity,
    ) -> Quantity {
        let single = match kind {
            mtg_contracts::kir::CounterKind::PlusOnePlusOne => Some(&mut self.plus_one_counters),
            mtg_contracts::kir::CounterKind::Loyalty => Some(&mut self.loyalty_counters),
            _ => None,
        };
        if let Some(count) = single {
            let previous = count.clone();
            if *amount > Quantity::zero() {
                count.subtract(&std::cmp::min(previous.clone(), amount.clone()));
            }
            return previous;
        }
        let Ok(i) = self.counters.binary_search_by(|stack| stack.kind.cmp(kind)) else {
            return Quantity::zero();
        };
        let previous = self.counters[i].count.clone();
        if *amount <= Quantity::zero() {
            return previous;
        }
        if previous <= *amount {
            self.counters.remove(i);
        } else {
            self.counters[i].count.subtract(amount);
        }
        previous
    }
    /// CR 118.3, 602.2b, 606.4: removing counters as a cost removes exactly
    /// `amount`; a cost the object cannot pay in full is rejected before any
    /// change. Returns the previous count.
    pub(in crate::engine) fn pay_counters(
        &mut self,
        kind: &mtg_contracts::kir::CounterKind,
        amount: &Quantity,
    ) -> Result<Quantity, String> {
        if amount.is_negative() || self.counter_count(kind) < *amount {
            return Err("insufficient counters to pay the cost".into());
        }
        Ok(self.remove_counters(kind, amount))
    }
    pub fn present_on_battlefield(&self) -> bool {
        self.zone == Zone::Battlefield && self.phased_out.is_none()
    }
    /// CR 701.27a, 712.8d-e: the face that is up.
    pub fn face(&self) -> CardFace {
        face_after(self.id.face, self.transforms)
    }
}
/// CR 701.27a: each transformation turns the permanent over once.
pub(crate) fn face_after(face: CardFace, transforms: u32) -> CardFace {
    if transforms % 2 == 1 {
        face.other()
    } else {
        face
    }
}
pub(crate) fn is_zero_rank(rank: &u8) -> bool {
    *rank == 0
}
pub(crate) fn is_zero_count(count: &u32) -> bool {
    *count == 0
}
pub(crate) fn is_zero_u64(count: &u64) -> bool {
    *count == 0
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StackInstance {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(in crate::engine) target_triggers: Vec<triggers::TargetTriggerDraft>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mode_mask: Option<u8>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cost_lock: Option<costs::CostLock>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub hybrid_colors: Vec<HybridPayment>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub selected_additional_costs: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub x: Option<Quantity>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub death_controller: Option<PlayerId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sacrificed: Option<ObjectBefore>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub sacrificed_set: Vec<ObjectId>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(in crate::engine) chosen_payment: Vec<chosen_cost::PaidObject>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub effect_source_handles: Vec<publish::StoredObjectHandle>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_facts: Option<ObjectBefore>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub entity_targets: Vec<TargetEntity>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub announced_entity_targets: Vec<Vec<Option<publish::StoredObjectHandle>>>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub stack_targets: Vec<StackId>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub announced_stack_targets: Vec<Vec<publish::HandleId<b's'>>>,
    #[serde(default, skip_serializing_if = "StackKind::is_spell")]
    pub kind: StackKind,
    /// CR 113.7a, 712.18: the face whose activated or triggered ability
    /// this is, fixed when it was activated or triggered.
    #[serde(default, skip_serializing_if = "CardFace::is_front")]
    pub source_face: CardFace,
    /// CR 701.27f: the source's transform count when this ability was put
    /// onto the stack.
    #[serde(default, skip_serializing_if = "is_zero_count")]
    pub source_transforms: u32,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub player_targets: Vec<PlayerId>,
    pub id: StackId,
    pub source: ObjectId,
    pub controller: PlayerId,
    pub targets: Vec<ObjectId>,
    pub announced_targets: Vec<Vec<publish::StoredObjectHandle>>,
    pub paid: Option<ManaCost>,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub mana_uncounterable: bool,
    pub total_cost: ManaCost,
    pub cast_from: Zone,
}

#[cfg(test)]
mod tests {
    use super::*;
    use mtg_contracts::kir::CounterKind;

    fn id(card: u32, incarnation: u32) -> ObjectId {
        ObjectId {
            card: CardId(card),
            incarnation,
            face: CardFace::Front,
        }
    }

    fn permanent(loyalty: i64, plus_one: i64) -> Object {
        Object {
            hideaway: None,
            phased_out: None,
            attached_to: None,
            attachment_timestamp: None,
            keyword_timestamp: None,
            keyword_rank: 0,
            battlefield_entry_turn: Some(1),
            control_upkeeps: None,
            plus_one_counters: plus_one.into(),
            loyalty_counters: loyalty.into(),
            loyalty_used_turn: None,
            counters: vec![CounterStack {
                kind: CounterKind::MinusOneMinusOne,
                count: 2.into(),
                timestamp: 4,
            }],
            transforms: 0,
            adventure_player: None,
            face_down: false,
            marked_damage: Quantity::zero(),
            deathtouch_damage: false,
            unearthed: false,
            dashed: false,
            attack_ready: true,
            id: id(0, 0),
            controller: PlayerId(0),
            default_controller: PlayerId(0),
            zone: Zone::Battlefield,
            tapped: false,
        }
    }

    /// CR 122.1: an effect removes loyalty counters like any other kind,
    /// clamped at the number the permanent has; zero or negative amounts
    /// remove nothing.
    #[test]
    fn effect_removal_has_a_clamped_loyalty_arm() {
        let mut o = permanent(3, 2);
        assert_eq!(
            o.remove_counters(&CounterKind::Loyalty, &1.into()),
            3.into()
        );
        assert_eq!(o.loyalty_counters, 2.into());
        assert_eq!(
            o.remove_counters(&CounterKind::Loyalty, &5.into()),
            2.into()
        );
        assert_eq!(o.loyalty_counters, Quantity::zero());
        assert_eq!(
            o.remove_counters(&CounterKind::Loyalty, &1.into()),
            Quantity::zero()
        );
        assert_eq!(o.loyalty_counters, Quantity::zero());
        let mut o = permanent(3, 2);
        for amount in [0, -2] {
            for kind in [
                CounterKind::Loyalty,
                CounterKind::PlusOnePlusOne,
                CounterKind::MinusOneMinusOne,
            ] {
                o.remove_counters(&kind, &amount.into());
            }
        }
        assert_eq!(o, permanent(3, 2));
        // Other kinds and +1/+1 counters are unaffected by loyalty removal.
        o.remove_counters(&CounterKind::Loyalty, &3.into());
        assert_eq!(o.plus_one_counters, 2.into());
        assert_eq!(o.counter_count(&CounterKind::MinusOneMinusOne), 2.into());
    }

    /// CR 118.3, 606.4: a cost removes exactly its amount or is rejected
    /// with no change; an effect removing the same amount clamps instead.
    #[test]
    fn cost_removal_rejects_insufficient_counters_atomically() {
        for kind in [
            CounterKind::Loyalty,
            CounterKind::PlusOnePlusOne,
            CounterKind::MinusOneMinusOne,
        ] {
            let mut o = permanent(2, 2);
            assert!(o.pay_counters(&kind, &3.into()).is_err());
            assert!(o.pay_counters(&kind, &(-1).into()).is_err());
            assert_eq!(o, permanent(2, 2), "{kind:?}");
            assert_eq!(o.pay_counters(&kind, &2.into()), Ok(2.into()));
            assert_eq!(o.counter_count(&kind), Quantity::zero());
            let mut clamped = permanent(2, 2);
            assert_eq!(clamped.remove_counters(&kind, &3.into()), 2.into());
            assert_eq!(clamped, o, "{kind:?}");
        }
        let mut o = permanent(2, 2);
        assert!(o.pay_counters(&CounterKind::Lore, &1.into()).is_err());
        assert_eq!(
            o.pay_counters(&CounterKind::Lore, &Quantity::zero()),
            Ok(Quantity::zero())
        );
        assert_eq!(o, permanent(2, 2));
    }

    /// CR 733.1: a captured row restores every stored object field exactly,
    /// including loyalty and its use, attachment, phasing, an Adventure
    /// permission, transforms and face-down status.
    #[test]
    fn captured_rows_restore_the_complete_object() {
        let mut o = permanent(4, 1);
        o.loyalty_used_turn = Some(3);
        o.attached_to = Some(id(7, 2));
        o.attachment_timestamp = Some(9);
        o.phased_out = Some(PhaseReturn::Indirect {
            attached_to: id(7, 2),
        });
        o.adventure_player = Some(PlayerId(2));
        o.transforms = 3;
        o.face_down = true;
        o.keyword_timestamp = Some(5);
        o.keyword_rank = 1;
        o.control_upkeeps = Some(2);
        o.unearthed = true;
        o.dashed = true;
        o.deathtouch_damage = true;
        o.marked_damage = 2.into();
        o.tapped = true;
        o.controller = PlayerId(1);
        o.default_controller = PlayerId(3);
        o.id = id(0, 4);
        let before = ObjectBefore {
            transforms: o.transforms,
            face_down: o.face_down,
            keyword_timestamp: o.keyword_timestamp,
            keyword_rank: o.keyword_rank,
            battlefield_entry_turn: o.battlefield_entry_turn,
            control_upkeeps: o.control_upkeeps,
            unearthed: o.unearthed,
            dashed: o.dashed,
            keywords: vec![],
            deathtouch_damage: o.deathtouch_damage,
            plus_one_counters: o.plus_one_counters.clone(),
            counters: o.counters.clone(),
            loyalty_counters: o.loyalty_counters.clone(),
            loyalty_used_turn: o.loyalty_used_turn,
            attached_to: o.attached_to,
            attachment_timestamp: o.attachment_timestamp,
            phased_out: o.phased_out,
            hideaway: o.hideaway.clone(),
            adventure_player: o.adventure_player,
            object: o.id,
            owner: PlayerId(0),
            controller: o.controller,
            default_controller: o.default_controller,
            zone: o.zone,
            tapped: o.tapped,
            marked_damage: o.marked_damage.clone(),
            attack_ready: o.attack_ready,
            characteristics: serde_json::from_value(serde_json::json!({
                "basic": false, "color_identity": [], "colors": [], "kind": "artifact",
                "legendary": false, "mana_cost": {"colored": [], "generic": "1"},
                "power": null, "subtypes": [], "toughness": null
            }))
            .unwrap(),
        };
        assert_eq!(before.restored_object(), o);
    }
}
