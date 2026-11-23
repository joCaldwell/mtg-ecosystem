//! CR 400.7, 406.3–4, 607.2a, 608.2h and 702.75a–b.
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;

/// Stored on the exact exile object. Moving or removing that object discards
/// the link, independently of what happens to its source or either controller.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HideawayLink {
    card: ObjectId,
    source: ObjectId,
    source_block: u32,
    source_handles: [publish::StoredObjectHandle; 4],
    lookers: [bool; 4],
}

impl HideawayLink {
    pub fn matches(&self, source: ObjectId, source_block: u32, card: ObjectId) -> bool {
        self.source == source && self.source_block == source_block && self.card == card
    }

    pub(super) fn source_handle(&self, viewer: PlayerId) -> &publish::StoredObjectHandle {
        &self.source_handles[viewer.0 as usize]
    }

    pub(super) fn may_look(&self, viewer: PlayerId) -> bool {
        self.lookers[viewer.0 as usize]
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RandomBottom {
    selected: ObjectId,
    order: Vec<ObjectId>,
    remaining: u32,
}

impl RulesTxn {
    pub(crate) fn linked_hideaway_cards(
        &self,
        source: ObjectId,
        source_block: u32,
    ) -> Vec<ObjectId> {
        self.state
            .rules
            .objects
            .values()
            .filter(|o| {
                o.zone == Zone::Exile
                    && o.hideaway
                        .as_ref()
                        .is_some_and(|link| link.matches(source, source_block, o.id))
            })
            .map(|o| o.id)
            .collect()
    }

    pub(crate) fn turn_up_linked_hideaway(
        &mut self,
        source: ObjectId,
        source_block: u32,
        card: ObjectId,
    ) -> Result<HideawayLink, String> {
        let o = self
            .rules_mut()
            .objects
            .get_mut(&card.card)
            .filter(|o| o.id == card && o.zone == Zone::Exile)
            .ok_or("missing linked exile incarnation")?;
        let link = o
            .hideaway
            .take_if(|link| link.matches(source, source_block, card))
            .ok_or("card was not hidden by this ability")?;
        Ok(*link)
    }

    pub(crate) fn restore_hideaway_link(
        &mut self,
        mut receipt: HideawayLink,
    ) -> Result<(), String> {
        if let Ok(source) = self.current(receipt.source)
            && source.present_on_battlefield()
            && self.is_live(source.controller)
        {
            receipt.lookers[source.controller.0 as usize] = true;
        }
        // A reversed cast may have been attempted by a player other than the owner.
        let owner = self.state.rules.cards[&receipt.card.card].owner;
        let o = self
            .rules_mut()
            .objects
            .get_mut(&receipt.card.card)
            .filter(|o| o.id == receipt.card && o.zone == Zone::Exile && o.hideaway.is_none())
            .ok_or("reversal did not restore exact unlinked exile incarnation")?;
        o.controller = owner;
        o.default_controller = owner;
        o.hideaway = Some(Box::new(receipt));
        Ok(())
    }

    pub(super) fn grant_hideaway_looks(&mut self, controllers: &BTreeMap<ObjectId, PlayerId>) {
        let live = std::array::from_fn::<_, 4, _>(|seat| self.is_live(PlayerId(seat as u8)));
        for o in self.rules_mut().objects.values_mut() {
            if let Some(link) = &mut o.hideaway
                && let Some(p) = controllers.get(&link.source)
                && live[p.0 as usize]
            {
                link.lookers[p.0 as usize] = true;
            }
        }
    }

    pub(super) fn validate_hideaway_links(&self) -> Result<(), String> {
        for o in self.state.rules.objects.values() {
            let Some(link) = &o.hideaway else { continue };
            let owner = self
                .state
                .rules
                .cards
                .get(&o.id.card)
                .ok_or("missing hideaway card owner")?
                .owner;
            let producer = false;
            let mut controller_permitted = true;
            if let Some(current) = self.state.rules.objects.get(&link.source.card) {
                if link.source.incarnation > current.id.incarnation
                    || (current.id == link.source && current.zone != Zone::Battlefield)
                {
                    return Err("invalid hideaway source incarnation".into());
                }
                if current.id == link.source
                    && current.present_on_battlefield()
                    && self.is_live(current.controller)
                {
                    controller_permitted = link.may_look(current.controller);
                }
            }
            if o.id != link.card
                || o.zone != Zone::Exile
                || o.controller != owner
                || !link.may_look(owner)
                || !producer
                || !controller_permitted
            {
                return Err("invalid face-down exile link".into());
            }
        }
        Ok(())
    }
}
