//! Face-down spells and permanents: casting with morph or megamorph and the
//! turn-face-up special action (CR 702.37, 708, 116.2b).
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;
use std::sync::LazyLock;

/// CR 708.2a, 702.37a: "a 2/2 face-down creature with no text, no name, no
/// subtypes, and no mana cost". These are its copiable values (CR 707.2); with
/// no mana cost or color indicator it is colorless (CR 105.2c, 202.2). As a
/// spell it is an ordinary creature spell (CR 702.37c, 708.4).
static FACE_DOWN: LazyLock<Definition> = LazyLock::new(|| Definition {
    layout: FaceLayout::Single,
    back_face: None,
    source_labels: None,
    x_mana_cost: false,
    flashback: None,
    face_down: None,
    oracle_id: String::new(),
    name: String::new(),
    characteristics: Characteristics {
        kind: CardKind::Creature,
        additional_types: vec![],
        subtypes: vec![],
        all_creature_types: false,
        basic: false,
        legendary: false,
        snow: false,
        hybrid_mana: vec![],
        mana_cost: ManaCost {
            generic: Quantity::zero(),
            colored: vec![],
        },
        colors: vec![],
        color_identity: vec![],
        power: Some(Quantity::from(2)),
        toughness: Some(Quantity::from(2)),
        front_face_mana_value: None,
    },
    intrinsic_zone: None,
    spell: Some(SpellProgram::permanent(SpellTiming::MainPhase)),
    modal_spell: None,
    activated: vec![],
    mana_abilities: vec![],
    triggered: vec![],
    static_abilities: vec![],
    keywords: vec![],
    capabilities: BTreeSet::new(),
    provenance: SourceProvenance {
        printing_id: String::new(),
        source_index: 0,
        source_sha256: String::new(),
        ast_sha256: String::new(),
        binding_sha256: String::new(),
        archive_manifest_sha256: String::new(),
        parser_sha256: String::new(),
        source_json: String::new(),
        abilities_json: String::new(),
    },
    composed: false,
});

/// The copiable values of every face-down object this engine creates.
pub(crate) fn face_down_definition() -> &'static Definition {
    &FACE_DOWN
}

impl RulesTxn {
    /// CR 708.2: whether this exact incarnation is face down now.
    pub(crate) fn is_face_down(&self, id: ObjectId) -> bool {
        self.state
            .rules
            .objects
            .get(&id.card)
            .is_some_and(|o| o.id == id && o.face_down)
    }
}

/// CR 702.37e: the stage of paying a morph cost to turn a permanent face up.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FaceUpStage {
    /// Mana abilities may be activated (CR 605.3a).
    Mana,
    Spend,
}

/// CR 116.2b, 702.37e: a player's special action turning `object` face up by
/// paying `cost`, the morph cost it would have face up. It uses no stack; the
/// player receives priority afterwards (CR 117.3c).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FaceUpPayment {
    pub player: PlayerId,
    pub object: ObjectId,
    pub cost: ManaCost,
    pub stage: FaceUpStage,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(super) receipts: Vec<ManaActivationUndo>,
}

/// CR 106.6, 702.37e: a morph cost is paid by neither a spell nor an
/// ability, so it spends mana as a resolution payment does.
const FACE_UP_USE: payments::ManaUse = payments::ManaUse::Resolution;

impl RulesTxn {
    /// CR 702.37e: the morph cost `player` pays to turn `object` face up at
    /// priority: a face-down permanent they control whose face-up card has
    /// a morph or megamorph ability.
    fn face_up_cost(&self, player: PlayerId, object: ObjectId) -> Result<ManaCost, String> {
        let o = self.current(object)?;
        if !o.present_on_battlefield() || !o.face_down || o.controller != player {
            return Err("turning face up needs a face-down permanent you control".into());
        }
        Ok(self
            .face_definition(object.card, CardFace::Front)?
            .face_down
            .as_ref()
            .ok_or("a face-down permanent without a morph cost cannot turn face up")?
            .cost
            .clone())
    }

    /// CR 116.2b: any time the player has priority.
    pub(super) fn validate_turn_face_up(
        &self,
        player: PlayerId,
        object: ObjectId,
    ) -> Result<(), String> {
        if !matches!(self.state.rules.frame, Frame::Priority) || self.state.rules.priority != player
        {
            return Err("turning face up requires priority".into());
        }
        self.face_up_cost(player, object).map(|_| ())
    }

    pub(crate) fn begin_turn_face_up(
        &mut self,
        player: PlayerId,
        object: ObjectId,
    ) -> Result<(), String> {
        self.validate_turn_face_up(player, object)?;
        let cost = self.face_up_cost(player, object)?;
        self.rules_mut().frame = Frame::TurnFaceUp {
            payment: FaceUpPayment {
                player,
                object,
                cost,
                stage: FaceUpStage::Mana,
                receipts: vec![],
            },
        };
        Ok(())
    }

    pub(super) fn face_up_request(&self, payment: &FaceUpPayment) -> Result<Decision<Rules>, String> {
        self.payment_decision(
            payment.player,
            payment.cost.clone(),
            Some(payment.object),
            payment.stage == FaceUpStage::Mana,
            FACE_UP_USE,
        )
    }

    /// CR 733.1: an abandoned special action reverses its mana activations
    /// where they can be reversed, and the player keeps priority.
    fn abandon_turn_face_up(&mut self, payment: &FaceUpPayment) -> Result<(), String> {
        if self.is_live(payment.player) {
            self.undo_mana_receipts(payment.player, &payment.receipts)?;
        }
        self.rules_mut().frame = Frame::Priority;
        Ok(self.stabilize()?)
    }

    pub(super) fn face_up_step(&mut self, mut payment: FaceUpPayment) -> Result<(), String> {
        let input = self
            .rules_mut()
            .pending
            .take()
            .ok_or("missing turn face up input")?;
        if input.player != payment.player {
            return Err("turn face up input from another player".into());
        }
        match (&input.answer, payment.stage) {
            (
                Answer::Pay {
                    pay: PayAnswer::Activate { source },
                },
                FaceUpStage::Mana,
            ) => {
                let object = input.mana_source(*source)?;
                return self.begin_payment_mana(&input, object, Frame::TurnFaceUp { payment });
            }
            (
                Answer::Pay {
                    pay:
                        PayAnswer::Finish {
                            undo_on_failure: undo_mana,
                        },
                },
                FaceUpStage::Mana,
            ) => {
                if !self.can_pay_for(payment.player, &payment.cost, FACE_UP_USE)? {
                    if *undo_mana {
                        return self.abandon_turn_face_up(&payment);
                    }
                    self.rules_mut().frame = Frame::Priority;
                    return Ok(self.stabilize()?);
                }
                payment.stage = FaceUpStage::Spend;
            }
            (Answer::Pay { pay }, FaceUpStage::Spend) => {
                // A mana ability's cost can have removed the permanent.
                if self.face_up_cost(payment.player, payment.object).as_ref() != Ok(&payment.cost) {
                    return self.abandon_turn_face_up(&payment);
                }
                let (unrestricted, restricted) =
                    pay.spending().ok_or("expected mana allocation")?;
                self.validate_spending(
                    payment.player,
                    &payment.cost,
                    FACE_UP_USE,
                    unrestricted,
                    restricted,
                )?;
                self.commit_batch(
                    BatchCause::SpecialAction {
                        player: payment.player,
                        object: payment.object,
                    },
                    EventGroup::Ordered {
                        children: vec![
                            EventGroup::Occurrence {
                                proposed: ProposedEvent::PayManaSelected {
                                    player: payment.player,
                                    cost: payment.cost.clone(),
                                    spending: unrestricted.clone(),
                                    restricted: restricted.to_vec(),
                                },
                            },
                            EventGroup::Occurrence {
                                proposed: ProposedEvent::TurnFaceUp {
                                    object: payment.object,
                                },
                            },
                        ],
                    },
                )?;
                // CR 117.3c: the player receives priority again.
                self.rules_mut().passes.clear();
                self.rules_mut().priority = payment.player;
                self.rules_mut().frame = Frame::Priority;
                return Ok(self.stabilize()?);
            }
            _ => return Err("turn face up input invariant".into()),
        }
        self.rules_mut().frame = Frame::TurnFaceUp { payment };
        Ok(())
    }

    /// Snapshot invariant: the payment names a legal special action.
    pub(crate) fn validate_face_up_payment(&self, payment: &FaceUpPayment) -> Result<(), String> {
        if payment.player.0 >= 4
            || payment.cost.generic.is_negative()
            || payment.receipts.iter().any(|r| r.player != payment.player)
            || (payment.stage == FaceUpStage::Spend
                && !self.can_pay_for(payment.player, &payment.cost, FACE_UP_USE)?)
        {
            return Err("turn face up payment invariant".into());
        }
        self.validate_mana_receipt_conditions(&payment.receipts)?;
        if matches!(self.state.rules.frame, Frame::TurnFaceUp { .. })
            && payment.stage == FaceUpStage::Mana
            && self.face_up_cost(payment.player, payment.object)? != payment.cost
        {
            return Err("turn face up payment names no legal special action".into());
        }
        Ok(())
    }

    /// A departure during the payment (a concession) abandons the action.
    pub(super) fn revalidate_face_up_departure(&mut self) -> Result<(), String> {
        let Frame::TurnFaceUp { payment } = &self.state.rules.frame else {
            return Ok(());
        };
        if self.is_live(payment.player)
            && self.face_up_cost(payment.player, payment.object).as_ref() == Ok(&payment.cost)
        {
            return Ok(());
        }
        let payment = payment.clone();
        self.rules_mut().pending = None;
        self.abandon_turn_face_up(&payment)
    }
}
