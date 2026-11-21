//! CR 106.12a, 603.3, 605.1b and 733.1.
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;

impl Game {
    pub(super) fn collect_mana_tap(
        &mut self,
        batch: &CommittedBatch,
        tapped: ObjectId,
        previous: Option<&ObjectBefore>,
        player: PlayerId,
    ) -> Result<Vec<TriggerId>, String> {
        if !self.catalog_has_mana_tap_observer {
            return Ok(vec![]);
        }
        // Multi-unit production is one activation. Link to the first actual
        // positive mana addition, never to a cost tap or a proposed quantity.
        let Some(occurrence) = batch.occurrences.iter().find(|r| {
            matches!(&r.actual, EventOutcome::ManaAdded { player: actor, amount, .. }
                if *actor == player && *amount > Quantity::zero())
                && matches!(&r.proposed, ProposedEvent::AddMana { source, .. } if source.card == tapped.card)
        }) else {
            return Ok(vec![]);
        };
        if !matches!(&batch.cause, BatchCause::ManaActivation { source, player: actor, .. }
            if *source == tapped && *actor == player)
        {
            return Err("invalid mana-tap batch authority".into());
        }
        let facts = if self
            .current(tapped)
            .is_ok_and(|o| o.present_on_battlefield())
        {
            self.object_before(tapped)?
        } else if let Some(facts) =
            previous.filter(|f| f.object == tapped && f.zone == Zone::Battlefield)
        {
            facts.clone()
        } else {
            return Err("missing mana-tap source identity".into());
        };
        if !facts.characteristics.has_type(CardKind::Land) {
            return Ok(vec![]);
        }
        let first_trigger = self.state.rules.next_trigger_id;
        self.collect_trigger_witness(
            None,
            TriggerOccurrenceId::Batch(occurrence.id.clone()),
            TriggerEventWitness::ManaTapped {
                land: Box::new(facts),
                player,
            },
        )?;
        Ok(self
            .state
            .rules
            .pending_triggers
            .iter()
            .filter(|t| t.id.0 >= first_trigger)
            .map(|t| t.id)
            .collect())
    }

    pub(super) fn cancel_reversed_tap_triggers(
        &mut self,
        source: Option<ObjectId>,
        player: PlayerId,
        triggers: &[TriggerId],
    ) -> Result<(), String> {
        if triggers.is_empty() {
            return Ok(());
        }
        if triggers.windows(2).any(|ids| ids[0] >= ids[1])
            || triggers
                .iter()
                .any(|id| id.0 >= self.state.rules.next_trigger_id)
        {
            return Err("invalid mana-tap reversal identities".into());
        }
        for pending in self
            .state
            .rules
            .pending_triggers
            .iter()
            .filter(|t| triggers.contains(&t.id))
        {
            let (tapped, actor) = match &pending.witness {
                TriggerEventWitness::ManaTapped { land, player } => (land.object, *player),
                TriggerEventWitness::Tapped { object, controller } => (*object, *controller),
                _ => return Err("unrelated mana reversal trigger".into()),
            };
            if actor != player || source.is_some_and(|expected| expected != tapped) {
                return Err("mana-tap reversal does not match its pending trigger".into());
            }
        }
        self.state
            .rules
            .pending_triggers
            .retain(|t| !triggers.contains(&t.id));
        Ok(())
    }

    pub(super) fn mana_tap_activator(entry: &StackInstance) -> Result<PlayerId, String> {
        let StackKind::Triggered {
            witness: TriggerEventWitness::ManaTapped { player, .. },
            ..
        } = &entry.kind
        else {
            return Err("missing witnessed mana activator".into());
        };
        Ok(*player)
    }
}

impl Prepared {
    pub(super) fn has_mana_tap_observer(&self) -> bool {
        self.catalog
            .definitions
            .iter()
            .any(|d| d.capabilities.contains(&Capability::ManaTapTriggerV1))
    }
}
