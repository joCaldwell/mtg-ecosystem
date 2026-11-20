//! CR 601.2c and 608.2b: the caster selects a player and distinct physical
//! cards in that player's graveyard. Recheck the announced incarnations only.
use super::*;

impl Game {
    pub(super) fn player_graveyard_card(&self, object: &Object, player: PlayerId) -> bool {
        object.zone == (Zone::Graveyard { owner: player })
            && !self.state.rules.tokens.contains_key(&object.id.card)
            && self
                .state
                .rules
                .cards
                .get(&object.id.card)
                .is_some_and(|c| c.owner == player)
    }

    pub(super) fn announced_graveyard_group(
        &self,
        caster: PlayerId,
        chooser: PlayerId,
        maximum: u8,
        entities: &[TargetEntity],
    ) -> Result<(PlayerId, Vec<ObjectId>), String> {
        let [TargetEntity::Player { player }, cards @ ..] = entities else {
            return Err("missing graveyard owner target".into());
        };
        if chooser != caster
            || !self.is_live(caster)
            || !self.entity_legal(TargetEntity::Player { player: *player })
            || cards.len() > usize::from(maximum)
        {
            return Err("invalid chooser, player or number of graveyard targets".into());
        }
        let mut selected = Vec::with_capacity(cards.len());
        for card in cards {
            let TargetEntity::Object { object } = card else {
                return Err("expected graveyard card".into());
            };
            if selected.contains(object)
                || !self
                    .current(*object)
                    .is_ok_and(|o| self.player_graveyard_card(o, *player))
            {
                return Err("duplicate or ineligible graveyard card target".into());
            }
            selected.push(*object);
        }
        Ok((*player, selected))
    }

    pub(super) fn remaining_graveyard_targets(
        &self,
        owner: PlayerId,
        cards: &[ObjectId],
    ) -> Vec<ObjectId> {
        cards
            .iter()
            .copied()
            .filter(|id| {
                self.current(*id)
                    .is_ok_and(|o| self.player_graveyard_card(o, owner))
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::equipped_indestructible_tests::{object, prepared};

    #[test]
    fn graveyard_target_group_requires_caster_and_preserves_physical_incarnations() {
        let mut p = prepared(
            &[("Forest", false), ("Plains", false), ("Island", false)],
            false,
        );
        for (i, c) in p.config.cards.iter_mut().take(3).enumerate() {
            let owner = PlayerId(if i == 2 { 2 } else { 1 });
            c.owner = owner;
            c.controller = owner;
            c.zone = Zone::Graveyard { owner };
        }
        p.profile_digest = digest(&p.config).unwrap();
        let mut g = Game::with_trace(p, TraceMode::Detailed);
        let caster = PlayerId(0);
        let owner = PlayerId(1);
        let who = TargetEntity::Player { player: owner };
        let ids = [object(&g, 0), object(&g, 1), object(&g, 2)];
        let targets = ids.map(|object| TargetEntity::Object { object });
        assert_eq!(
            g.announced_graveyard_group(caster, caster, 2, &[who])
                .unwrap(),
            (owner, vec![])
        );
        let entities = [who, targets[0], targets[1]];
        let (_, cards) = g
            .announced_graveyard_group(caster, caster, 2, &entities)
            .unwrap();
        assert!(
            g.announced_graveyard_group(caster, owner, 2, &entities)
                .is_err()
        );
        for (limit, selection) in [
            (1, entities),
            (2, [who, targets[0], targets[0]]),
            (2, [who, targets[0], targets[2]]),
        ] {
            assert!(
                g.announced_graveyard_group(caster, caster, limit, &selection)
                    .is_err()
            );
        }
        let moved = g
            .move_object_for(ids[0], Zone::Hand { owner }, BatchCause::Fixture)
            .unwrap();
        g.move_object_for(
            moved.successor,
            Zone::Graveyard { owner },
            BatchCause::Fixture,
        )
        .unwrap();
        assert_eq!(g.remaining_graveyard_targets(owner, &cards), [ids[1]]);
    }
}
