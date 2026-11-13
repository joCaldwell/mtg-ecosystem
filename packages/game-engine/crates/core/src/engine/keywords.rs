//! Intrinsic and continuous layer-6 granted keywords. CR 702.2/9/10/15/17/20, 120.3f, 510.2, 704.5h.
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LifelinkGain {
    pub source: ObjectId,
    pub player: PlayerId,
    pub amount: Quantity,
    pub previous: Quantity,
    pub current: Quantity,
}
pub(super) enum KeywordSource<'a> {
    Current(ObjectId),
    Historical(&'a ObjectBefore),
}
impl From<ObjectId> for KeywordSource<'_> {
    fn from(o: ObjectId) -> Self {
        Self::Current(o)
    }
}
impl<'a> From<&'a ObjectBefore> for KeywordSource<'a> {
    fn from(o: &'a ObjectBefore) -> Self {
        Self::Historical(o)
    }
}
impl Game {
    /// Current checks include continuous grants (CR 613.1f); saved
    /// ObjectBefore keywords bind damage LKI to the exact source (702.15c/2e).
    pub(super) fn has_keyword<'a>(
        &self,
        object: impl Into<KeywordSource<'a>>,
        keyword: KeywordAbility,
    ) -> bool {
        match object.into() {
            KeywordSource::Current(object) => self.current_keywords(object).contains(&keyword),
            KeywordSource::Historical(object) => object.keywords.contains(&keyword),
        }
    }
    pub(super) fn can_block(&self, blocker: ObjectId, attacker: ObjectId) -> bool {
        !self.has_restriction(attacker, CreatureRestriction::CantBeBlocked)
            && !self.has_restriction(blocker, CreatureRestriction::CantAttackOrBlock)
            && (!self.has_keyword(attacker, KeywordAbility::Flying)
                || self.has_keyword(blocker, KeywordAbility::Flying)
                || self.has_keyword(blocker, KeywordAbility::Reach))
    }
    /// Called inside the atomic final damage commit, after all validation and
    /// prevention. No SBA or trigger placement can occur between damage/gains.
    /// CR 702.15e: aggregate one gain per source, distinct simultaneous sources
    /// remain separate gains (including when a source damages several objects).
    pub(super) fn damage_keywords(
        &mut self,
        actual: &[ActualDamage],
        // Transactional assignment facts may predate control or grant changes.
        // None is reserved for genuine legacy leaves with fresh source facts.
        source_appearances: Option<&BTreeMap<DamageId, ObjectBefore>>,
    ) -> Result<Vec<LifelinkGain>, String> {
        if let Some(appearances) = source_appearances {
            if appearances.len() != actual.len() {
                return Err("incomplete damage-time source appearances".into());
            }
            for damage in actual {
                let fact = appearances
                    .get(&damage.id)
                    .ok_or("missing damage-time source appearance")?;
                self.validate_characteristic_identity(fact.object, &fact.characteristics)?;
                if fact.object != damage.source.object
                    || fact.owner != damage.source.owner
                    || fact.default_controller != damage.source.default_controller
                    || fact.zone != damage.source.zone
                    || fact.controller.0 >= 4
                    || fact.default_controller.0 >= 4
                    || fact.marked_damage.is_negative()
                    || self
                        .state
                        .rules
                        .cards
                        .get(&fact.object.card)
                        .is_none_or(|c| c.owner != fact.owner)
                {
                    return Err("invalid damage-time source appearance".into());
                }
            }
        }
        let mut gains: BTreeMap<(ObjectId, PlayerId), Quantity> = BTreeMap::new();
        for d in actual {
            if d.amount <= Quantity::zero() {
                continue;
            }
            // CR 611.3a-b/702.2b,e/702.15b-c: source properties are current
            // when damage is dealt, or latest LKI if that exact source left.
            // Assignment-time deathtouch still separately proves the legal
            // trample split (702.2c); it does not preserve a lost damage grant.
            let appearance = match source_appearances {
                Some(appearances) => appearances
                    .get(&d.id)
                    .ok_or("missing damage-time source appearance")?,
                None => &d.source,
            };
            if self.has_keyword(appearance, KeywordAbility::Lifelink)
                && self.is_live(appearance.controller)
            {
                gains
                    .entry((d.source.object, appearance.controller))
                    .or_default()
                    .add(&d.amount);
            }
            if self.has_keyword(appearance, KeywordAbility::Deathtouch)
                && let DamageRecipient::Creature { object } = d.recipient
            {
                let o = self
                    .state
                    .rules
                    .objects
                    .get_mut(&object.card)
                    .ok_or("missing deathtouch recipient")?;
                if o.id != object || o.zone != Zone::Battlefield {
                    return Err("stale deathtouch recipient".into());
                }
                o.deathtouch_damage = true;
            }
        }
        let mut results = vec![];
        for ((source, player), amount) in gains {
            let life = &mut self.state.rules.players[player.0 as usize].life;
            let previous = life.clone();
            life.add(&amount);
            results.push(LifelinkGain {
                source,
                player,
                amount,
                previous,
                current: life.clone(),
            });
        }
        Ok(results)
    }
}
