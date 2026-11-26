//! CR 601.2c and 608.2b. Select distinct cards in the specified graveyards
//! and recheck the announced incarnations at resolution.
use super::*;

impl RulesTxn {
    pub(crate) fn player_graveyard_card(&self, object: &Object, player: PlayerId) -> bool {
        object.zone == (Zone::Graveyard { owner: player })
            && !self.state.rules.tokens.contains_key(&object.id.card)
            && self
                .state
                .rules
                .cards
                .get(&object.id.card)
                .is_some_and(|c| c.owner == player)
    }
    pub(crate) fn graveyard_target_legal(
        &self,
        object: &Object,
        player: PlayerId,
        domain: TargetDomain,
    ) -> bool {
        if let TargetDomain::GraveyardCard { opponents_only } = domain {
            return matches!(object.zone, Zone::Graveyard { owner } if (!opponents_only || owner != player) && self.player_graveyard_card(object, owner));
        }
        self.player_graveyard_card(object, player)
            && match domain {
                TargetDomain::OwnGraveyardCard => true,
                TargetDomain::OwnGraveyardCreatureCard => self
                    .definition(object.id.card)
                    .is_ok_and(|d| d.characteristics.has_type(CardKind::Creature)),
                _ => false,
            }
    }
    /// CR 205.2b, 601.2c, 608.2b: any printed type alternative may
    /// satisfy a card target, including a card's secondary type.
    pub(super) fn graveyard_target_legal_for(
        &self,
        object: &Object,
        player: PlayerId,
        spec: &mtg_contracts::TargetSpec,
        x: Option<&Quantity>,
    ) -> bool {
        self.graveyard_target_legal(object, player, spec.domain)
            && spec.filter.as_ref().is_none_or(|filter| {
                (filter.types.is_empty()
                    || self.copiable_definition(object.id).is_ok_and(|definition| {
                        filter
                            .types
                            .iter()
                            .any(|kind| definition.characteristics.has_type(*kind))
                    }))
                    && filter.predicate.as_ref().is_none_or(|predicate| {
                        let facts = self.object_facts(object);
                        predicate.0.iter().all(|atom| {
                            if mtg_contracts::kir::mana_value_at_most_announced_x(atom) {
                                x.is_none_or(|chosen| {
                                    self.graveyard_mana_value(object.id)
                                        .is_some_and(|value| value <= *chosen)
                                })
                            } else {
                                crate::query::matches_atom(&facts, player, atom).is_yes()
                            }
                        })
                    })
            })
    }

    pub(super) fn graveyard_mana_value(&self, object: ObjectId) -> Option<Quantity> {
        self.copiable_definition(object)
            .ok()
            .map(|d| d.characteristics.printed_mana_value())
    }

    /// CR 400.7, 608.2b-c: only the original, still-present survivors
    /// of this target occurrence enter, from one common prior state.
    pub(super) fn return_announced_creatures(
        &mut self,
        frame: Resolving,
        source_block: u32,
    ) -> Result<(), String> {
        let entry = self.entry(frame.stack_id)?;
        let occurrences = entry
            .targets
            .iter()
            .zip(&frame.legal_targets)
            .filter(|(id, legal)| {
                **legal
                    && self
                        .current(**id)
                        .is_ok_and(|o| self.player_graveyard_card(o, entry.controller))
            })
            .map(|(&object, _)| ProposedEvent::EnterTapped { object })
            .collect::<Vec<_>>();
        let mut movements = vec![];
        if !occurrences.is_empty() {
            let batch = self.commit_batch(
                self.instruction_cause(&frame, source_block)?,
                EventGroup::Simultaneous { occurrences },
            )?;
            for occurrence in batch.occurrences {
                let EventOutcome::Moved { movement } = occurrence.actual else {
                    return Err("missing tapped graveyard entry".into());
                };
                movements.push(movement);
            }
        }
        self.finish_instruction(
            frame,
            InstructionOutcome::SetMoved {
                movements,
                destroyed_creatures: 0,
            },
        )
    }
}
