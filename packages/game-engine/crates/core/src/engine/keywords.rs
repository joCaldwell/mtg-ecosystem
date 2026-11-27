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
pub enum KeywordSource<'a> {
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
impl RulesTxn {
    /// Current checks include continuous grants (CR 613.1f); saved
    /// ObjectBefore keywords bind damage LKI to the exact source (702.15c/2e).
    pub(crate) fn has_keyword<'a>(
        &self,
        object: impl Into<KeywordSource<'a>>,
        keyword: KeywordAbility,
    ) -> bool {
        match object.into() {
            KeywordSource::Current(object) => self.current_keywords(object).contains(&keyword),
            KeywordSource::Historical(object) => object.keywords.contains(&keyword),
        }
    }
    // CR 105.1/702.16b–h. Evaluate the source colors at their applicable boundary, including
    // instantiated token colors. C denotes colorless and is not a sixth color.
    pub(crate) fn has_color(source: &Characteristics) -> bool {
        source.colors.iter().any(|color| *color != Color::C)
    }

    pub(super) fn printed_protection_from_everything(&self, object: ObjectId) -> bool {
        self.copiable_definition(object).is_ok_and(|d| {
            d.keywords
                .contains(&KeywordAbility::ProtectionFromEverything)
        }) && !self.printed_abilities_lost(object)
    }

    pub(super) fn effective_protection_colors(
        &self,
        object: ObjectId,
    ) -> impl Iterator<Item = Color> {
        self.current_keywords(object)
            .into_iter()
            .filter_map(KeywordAbility::fixed_protection_color)
    }

    pub(crate) fn protected_from(&self, target: ObjectId, source: &Characteristics) -> bool {
        // CR 702.16b–f,j. Printed protection from everything functions on
        // the battlefield. Only the separate each-color grant tests color.
        self.current(target)
            .is_ok_and(|o| o.present_on_battlefield())
            && (self.printed_protection_from_everything(target)
                || self
                    .effective_protection_colors(target)
                    .any(|color| source.colors.contains(&color))
                || (!self.state.rules.effects.is_empty()
                    && Self::has_color(source)
                    && self.has_keyword(target, KeywordAbility::ProtectionFromEachColor)))
    }

    // CR 608.2b: when the exact ability source has left, use its LKI,
    // even if the same physical card has since returned.
    pub(crate) fn targeting_characteristics<'a>(
        &'a self,
        entry: &'a StackInstance,
    ) -> Result<std::borrow::Cow<'a, Characteristics>, String> {
        self.targeting_source(entry, false)
    }

    /// CR 608.2b, 702.134a: mentor compares layer-7 power. Other target
    /// restrictions need only the source's color layer for protection.
    pub(super) fn targeting_source<'a>(
        &'a self,
        entry: &'a StackInstance,
        needs_power: bool,
    ) -> Result<std::borrow::Cow<'a, Characteristics>, String> {
        if self
            .current(entry.source)
            .is_ok_and(|o| o.phased_out.is_none())
        {
            if needs_power {
                return self
                    .characteristics(entry.source)
                    .map(std::borrow::Cow::Owned);
            }
            self.color_layer(entry.source)
        } else {
            entry
                .source_facts
                .as_ref()
                .filter(|facts| facts.object == entry.source)
                .map(|facts| std::borrow::Cow::Borrowed(&facts.characteristics))
                .ok_or_else(|| "missing last-known targeting source".into())
        }
    }

    pub(crate) fn can_block(&self, blocker: ObjectId, attacker: ObjectId) -> bool {
        let keywords = self.current_keywords(attacker);
        let attacker_flies = keywords.contains(&KeywordAbility::Flying);
        self.color_layer(blocker)
            .is_ok_and(|source| !self.protected_from(attacker, &source))
            && !self.has_restriction(attacker, CreatureRestriction::CantBeBlocked)
            && !self.creature_restrictions(blocker).any(|restriction| {
                restriction == CreatureRestriction::CantAttackOrBlock
                    || restriction == CreatureRestriction::CantBlock
                    || (!attacker_flies && restriction == CreatureRestriction::CanBlockOnlyFlying)
            })
            && (!attacker_flies
                || self.has_keyword(blocker, KeywordAbility::Flying)
                || self.has_keyword(blocker, KeywordAbility::Reach))
            && !self.landwalk_prevents_block(attacker, &keywords)
            && !self.evasion_prevents_block(blocker, attacker, &keywords)
            && self.block_rules_permit(blocker, attacker)
    }

    // CR 702.13b, 702.28b, 702.31b, 702.36b, 702.118b: checked against the
    // current characteristics at the declaration (CR 509.1b); different
    // evasion abilities are cumulative.
    // https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
    fn evasion_prevents_block(
        &self,
        blocker: ObjectId,
        attacker: ObjectId,
        keywords: &[KeywordAbility],
    ) -> bool {
        use KeywordAbility as K;
        let blocker_keywords = self.current_keywords(blocker);
        let has = |k| keywords.contains(&k);
        if has(K::Shadow) != blocker_keywords.contains(&K::Shadow)
            || (has(K::Horsemanship) && !blocker_keywords.contains(&K::Horsemanship))
        {
            return true;
        }
        if !(has(K::Fear) || has(K::Intimidate) || has(K::Skulk)) {
            return false;
        }
        let (Ok(blocking), Ok(attacking)) = (
            self.characteristics(blocker),
            self.characteristics(attacker),
        ) else {
            return true;
        };
        let artifact = blocking.has_type(CardKind::Artifact);
        let shares_color = blocking
            .colors
            .iter()
            .any(|c| *c != Color::C && attacking.colors.contains(c));
        (has(K::Fear) && !artifact && !blocking.colors.contains(&Color::B))
            || (has(K::Intimidate) && !artifact && !shares_color)
            || (has(K::Skulk)
                && blocking
                    .power
                    .as_ref()
                    .zip(attacking.power.as_ref())
                    .is_none_or(|(b, a)| b > a))
    }

    // CR 509.1b/702.14: each attack has its own defending player. Basic land
    // types are current subtypes, independently of the Basic supertype.
    // https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
    fn landwalk_prevents_block(&self, attacker: ObjectId, keywords: &[KeywordAbility]) -> bool {
        let land_types: Vec<_> = keywords
            .iter()
            .filter_map(|keyword| keyword.basic_landwalk_type())
            .collect();
        if land_types.is_empty() {
            return false;
        }
        let Some(attack) = self
            .state
            .rules
            .combat
            .iter()
            .find(|a| a.attacker == attacker)
        else {
            return false;
        };
        self.state.rules.objects.values().any(|object| {
            if object.controller != attack.defender || !object.present_on_battlefield() {
                return false;
            }
            let Ok(land) = self.structural_characteristics(object.id) else {
                return false;
            };
            land.has_type(CardKind::Land)
                && land_types
                    .iter()
                    .any(|land_type| land.subtypes.iter().any(|s| s == land_type.subtype()))
        })
    }
    /// Called inside the atomic final damage commit, after all validation and
    /// prevention. No SBA or trigger placement can occur between damage/gains.
    /// CR 702.15e: aggregate one gain per source, distinct simultaneous sources
    /// remain separate gains (including when a source damages several objects).
    pub(crate) fn damage_keywords(
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
                self.validate_fact_identity(fact)?;
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
                && !self.state.rules.players[appearance.controller.0 as usize]
                    .life_frozen_until_next_turn
            {
                gains
                    .entry((d.source.object, appearance.controller))
                    .or_default()
                    .add(&d.amount);
            }
            if self.has_keyword(appearance, KeywordAbility::Deathtouch)
                && let DamageRecipient::Permanent { object } = d.recipient
                && self.is_battlefield_creature(object)
            {
                let o = self
                    .rules_mut()
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
            let life = &mut self.rules_mut().players[player.0 as usize].life;
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
