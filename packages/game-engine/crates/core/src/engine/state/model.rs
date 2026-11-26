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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(in crate::engine) battlefield_entry_turn: Option<u64>,
    #[serde(default, skip_serializing_if = "tokens::quantity_is_zero")]
    pub plus_one_counters: Quantity,
    /// CR 701.27a, 712.18: how many times this permanent has transformed
    /// since it entered; an odd count has its other face up. A zone change
    /// starts a new object at zero (CR 400.7).
    #[serde(default, skip_serializing_if = "is_zero_count")]
    pub transforms: u32,
    /// CR 715.3d: the player who may play this exiled card because it was
    /// exiled as a resolving Adventure. Leaving exile ends the permission.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub adventure_player: Option<PlayerId>,
    pub marked_damage: Quantity,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub deathtouch_damage: bool,
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
impl Object {
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
pub(crate) fn is_zero_count(count: &u32) -> bool {
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
