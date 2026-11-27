use super::*;
use mtg_contracts::kir::{BlockRestriction, CombatProhibitionScope, CombatRequirement, StaticRule};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Attacker {
    pub attacker: ObjectId,
    pub defender: PlayerId,
    pub planeswalker: Option<ObjectId>,
    pub defender_removed: bool,
    pub blocked: bool,
    pub blockers: Vec<ObjectId>,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "recipient", rename_all = "snake_case", deny_unknown_fields)]
pub enum DamageRecipient {
    Player { player: PlayerId },
    Permanent { object: ObjectId },
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DamageOccurrence {
    pub source: ObjectId,
    pub controller: PlayerId,
    pub recipient: DamageRecipient,
    pub amount: Quantity,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DamageResult {
    pub recipient: DamageRecipient,
    pub previous: Quantity,
    pub current: Quantity,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CreatureLossReason {
    LethalDamage,
    Deathtouch,
    ZeroToughness,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PlayerLossReason {
    Poison,
    CommanderDamage,
    Life,
    FailedDraw,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CommanderDamageResult {
    pub commander: CardId,
    pub recipient: PlayerId,
    pub amount: Quantity,
    pub previous: Quantity,
    pub current: Quantity,
}

impl RulesTxn {
    pub(super) fn combat_decision_player(&self) -> Option<PlayerId> {
        match &self.state.rules.frame {
            Frame::Attackers => self.state.rules.active,
            Frame::Blockers { remaining } => remaining.first().copied(),
            Frame::DamageChoice { source } => self.current(*source).ok().map(|o| o.controller),
            Frame::Discard { player, .. } => Some(*player),
            _ => None,
        }
    }
    pub(crate) fn creatures(&self, player: PlayerId, attacking: bool) -> Vec<ObjectId> {
        self.state
            .rules
            .objects
            .values()
            .filter(|o| {
                o.controller == player
                    && !o.tapped
                    && self.is_battlefield_creature(o.id)
                    && !self.combat_requirements(o.id, None).any(|rule| match rule {
                        CombatRequirement::CantAttackOrBlock => true,
                        CombatRequirement::CantAttack => attacking,
                        CombatRequirement::CantBlock => !attacking,
                        _ => false,
                    })
                    && (!attacking
                        || ((o.attack_ready || self.has_keyword(o.id, KeywordAbility::Haste))
                            && (!self.has_keyword(o.id, KeywordAbility::Defender)
                                // CR 609.4, 702.3b.
                                || self.combat_requirements(o.id, None).any(|rule| {
                                    rule == CombatRequirement::AttackAsThoughNoDefender
                                }))))
            })
            .map(|o| o.id)
            .collect()
    }
    pub(super) fn combat_requirements<'a>(
        &'a self,
        object: ObjectId,
        sources: Option<&'a [CardId]>,
    ) -> impl Iterator<Item = CombatRequirement> + 'a {
        use CombatProhibitionScope as S;
        let resolved = self.state.rules.effects.iter().filter_map(move |effect| {
            let ResolvedEffectKind::CombatProhibition {
                affects,
                restriction,
            } = &effect.kind
            else {
                return None;
            };
            // CR 611.2c, 613.11: these change combat rules. Read characteristics
            // after the layers, at declaration, including creatures that arrived later.
            (self.is_battlefield_creature(object)
                && match affects {
                    S::Target { target } => *target == object,
                    S::Creatures {} => true,
                    S::CreaturesWithoutFlying {} => !self
                        .current_keywords_with_sources(object, sources)
                        .contains(&KeywordAbility::Flying),
                })
            .then_some(*restriction)
        });
        self.combat_statics(object, sources).chain(resolved)
    }
    // CR 509.1a: additional allowances stack, and unlimited capacity
    // overrides the default and every additive allowance.
    fn block_capacity(&self, blocker: ObjectId) -> u32 {
        self.combat_statics(blocker, None)
            .fold(1u32, |count, rule| match rule {
                CombatRequirement::AdditionalBlocks(extra) => count.saturating_add(extra),
                CombatRequirement::UnlimitedBlocks => u32::MAX,
                _ => count,
            })
    }

    /// CR 509.1b: static per-pair block restrictions, read at the declaration.
    pub(super) fn block_rules_permit(&self, blocker: ObjectId, attacker: ObjectId) -> bool {
        self.static_declaration_rules()
            .all(|(source, rule)| match rule {
                StaticRule::Block {
                    affects,
                    restriction: BlockRestriction::Blockers { only, filter },
                } => {
                    !self.declaration_scope(affects, source, attacker)
                        || self.declaration_matches(filter, source, blocker) == *only
                }
                StaticRule::Block {
                    affects,
                    restriction: BlockRestriction::Attackers { only, filter },
                } => {
                    !self.declaration_scope(affects, source, blocker)
                        || self.declaration_matches(filter, source, attacker) == *only
                }
                _ => true,
            })
    }
    /// CR 509.1b, 702.111b: the least and greatest number of creatures a
    /// nonempty block of `attacker` may use.
    pub(super) fn blocker_count_bounds(&self, attacker: ObjectId) -> (u32, Option<u32>) {
        let mut least = if self.has_keyword(attacker, KeywordAbility::Menace) {
            2
        } else {
            1
        };
        let mut most: Option<u32> = None;
        for (source, rule) in self.static_declaration_rules() {
            if let StaticRule::Block {
                affects,
                restriction,
            } = rule
                && self.declaration_scope(affects, source, attacker)
            {
                match restriction {
                    BlockRestriction::AtLeast { count } => least = least.max(*count),
                    BlockRestriction::AtMost { count } => {
                        most = Some(most.map_or(*count, |m| m.min(*count)));
                    }
                    _ => {}
                }
            }
        }
        (least, most)
    }
    /// CR 508.1b-c: whether a static restriction forbids attacking `defender`.
    pub(super) fn attack_permitted(&self, attacker: ObjectId, defender: PlayerId) -> bool {
        self.attack_defender_permitted(attacker, defender, false)
    }

    fn attack_defender_permitted(
        &self,
        attacker: ObjectId,
        defender: PlayerId,
        planeswalker: bool,
    ) -> bool {
        self.static_declaration_rules()
            .all(|(source, rule)| match rule {
                StaticRule::AttackRestriction {
                    affects,
                    defender: who,
                    planeswalkers,
                } => {
                    !self.declaration_scope(affects, source, attacker)
                        || (planeswalker && !planeswalkers)
                        || !self.static_player(who, source.controller, defender)
                }
                _ => true,
            })
    }
    pub(super) fn is_battlefield_planeswalker(&self, object: ObjectId) -> bool {
        self.current(object)
            .is_ok_and(Object::present_on_battlefield)
            && self
                .characteristics(object)
                .is_ok_and(|c| c.has_type(CardKind::Planeswalker))
    }

    pub(super) fn is_permanent_damage_recipient(&self, object: ObjectId) -> bool {
        self.is_battlefield_creature(object) || self.is_battlefield_planeswalker(object)
    }

    fn attackable_planeswalkers(
        &self,
        attacker: ObjectId,
        player: PlayerId,
    ) -> impl Iterator<Item = (PlayerId, ObjectId)> + '_ {
        self.state.rules.objects.values().filter_map(move |o| {
            let defender = o.controller;
            (defender != player
                && self.is_live(defender)
                && self.is_battlefield_planeswalker(o.id)
                && self.attack_defender_permitted(attacker, defender, true))
            .then_some((defender, o.id))
        })
    }

    // CR 508.1d: static and resolved "attacks each combat/this turn if able".
    fn must_attack(&self, object: ObjectId) -> bool {
        self.combat_requirements(object, None)
            .any(|rule| rule == CombatRequirement::AttackEachCombat)
    }
    /// CR 508.1d, 702.141a: an encore token attacks its opponent this turn
    /// if able. Attacking that player obeys every other admitted requirement
    /// too, so the token may attack no other player while it can attack them.
    /// A player who left, or who now controls the token, is no defender it
    /// can attack, so no requirement remains (CR 506.2, 800.4a).
    fn required_defender(&self, object: ObjectId) -> Option<PlayerId> {
        let (defender, turn) = self.encore_mark(object)?;
        (turn == self.state.rules.turn.number
            && self.is_live(defender)
            && self.current(object).is_ok_and(|o| o.controller != defender)
            && self.attack_permitted(object, defender))
        .then_some(defender)
    }

    fn hand(&self, player: PlayerId) -> Vec<ObjectId> {
        self.state
            .rules
            .objects
            .values()
            .filter(|o| o.zone == (Zone::Hand { owner: player }))
            .map(|o| o.id)
            .collect()
    }
    // CR 101.1, 510.1a: the static rule changes the assignment amount.
    // https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
    fn combat_damage_amount(&self, source: ObjectId) -> Result<Quantity, String> {
        let facts = self.characteristics(source)?;
        let assigned = if self.assigns_with_toughness(source) {
            facts.toughness
        } else {
            facts.power
        };
        Ok(assigned
            .ok_or("missing combat damage characteristic")?
            .max(Quantity::zero()))
    }
    pub(super) fn assigns_with_toughness(&self, source: ObjectId) -> bool {
        self.combat_statics(source, None)
            .any(|rule| rule == CombatRequirement::AssignCombatDamageWithToughness)
    }

    fn recipients(&self, source: ObjectId) -> Vec<ObjectId> {
        let combat = &self.state.rules.combat;
        if let Some(attack) = combat.iter().find(|a| a.attacker == source) {
            return attack.blockers.clone();
        }
        combat
            .iter()
            .filter(|a| a.blockers.contains(&source))
            .map(|a| a.attacker)
            .collect()
    }

    fn needs_damage_split(&self, source: ObjectId, recipients: usize) -> bool {
        recipients > 1 || (recipients == 1 && self.trample_defender(source).is_some())
    }

    // CR 510.1e, 702.19b: variable damage from separate attackers may
    // jointly satisfy a trample gate when they share a blocker.
    fn joint_damage_groups(
        &self,
        source: ObjectId,
        candidates: &mut Vec<Candidate<Rules>>,
    ) -> Result<Option<Shape>, String> {
        let attacks = &self.state.rules.combat;
        let mut variable = Vec::new();
        for attack in attacks {
            if self.assigns_in_damage_step(attack.attacker)
                && self.needs_damage_split(attack.attacker, attack.blockers.len())
            {
                let total = self.combat_damage_amount(attack.attacker)?;
                if total > Quantity::zero() {
                    variable.push((attack, total));
                }
            }
        }
        if !variable.iter().any(|(a, _)| a.attacker == source)
            || !variable.iter().any(|(a, _)| {
                self.trample_defender(a.attacker).is_some()
                    && variable.iter().any(|(b, _)| {
                        a.attacker != b.attacker
                            && a.blockers.iter().any(|o| b.blockers.contains(o))
                    })
            })
        {
            return Ok(None);
        }
        let mut groups = Vec::new();
        let mut blocker_buckets = BTreeMap::<ObjectId, (Vec<u32>, Vec<u32>)>::new();
        let mut trample_buckets = Vec::new();
        let mut bucket = 0u32;
        for (attack, total) in variable {
            let source = candidate_index(
                candidates,
                Candidate::Object {
                    object: attack.attacker,
                },
            );
            let mut blockers = attack.blockers.clone();
            blockers.sort_by_key(|b| self.current(*b).ok().and_then(|o| o.keyword_timestamp));
            let deathtouch = self.has_keyword(attack.attacker, KeywordAbility::Deathtouch);
            let mut recipients = Vec::new();
            for blocker in blockers {
                recipients.push(candidate_index(
                    candidates,
                    Candidate::Object { object: blocker },
                ));
                let (damage, lethal) = blocker_buckets.entry(blocker).or_default();
                damage.push(bucket);
                if deathtouch {
                    lethal.push(bucket);
                }
                bucket += 1;
            }
            if let Some(defender) = self.trample_defender(attack.attacker) {
                let candidate = match defender {
                    DamageRecipient::Player { player } => Candidate::Player { player },
                    DamageRecipient::Permanent { object } => Candidate::Object { object },
                };
                recipients.push(candidate_index(candidates, candidate));
                trample_buckets.push((bucket, attack));
                bucket += 1;
            }
            groups.push(CombatDistribution {
                source,
                total,
                recipients,
            });
        }
        let mut lethal = Vec::new();
        for (player_bucket, attack) in trample_buckets {
            for &blocker in &attack.blockers {
                let remaining = self.blocker_damage_remaining(attack.attacker, blocker, false)?;
                if remaining > Quantity::zero() {
                    let (damage_buckets, deathtouch_buckets) = blocker_buckets[&blocker].clone();
                    lethal.push(CombatLethal {
                        player_bucket,
                        damage_buckets,
                        deathtouch_buckets,
                        remaining,
                    });
                }
            }
        }
        Ok(Some(Shape::CombatDistribute { groups, lethal }))
    }

    pub(super) fn combat_request(&self) -> Result<Option<Decision<Rules>>, String> {
        let Some(player) = self.combat_decision_player() else {
            return Ok(None);
        };
        let mut candidates = vec![];
        Ok(Some(match self.state.rules.frame {
            Frame::Attackers | Frame::Blockers { .. } => {
                let attacking = matches!(self.state.rules.frame, Frame::Attackers);
                let mut subjects = self.creatures(player, attacking);
                let walkers = if attacking {
                    let mut walkers = self
                        .state
                        .rules
                        .objects
                        .values()
                        .filter(|o| {
                            o.controller != player && self.is_battlefield_planeswalker(o.id)
                        })
                        .map(|o| {
                            Ok((
                                self.object_handle_id(player, o.id)?.clone(),
                                o.id,
                                o.controller,
                            ))
                        })
                        .collect::<Result<Vec<_>, String>>()?;
                    walkers.sort_by(|a, b| a.0.cmp(&b.0));
                    // The first attacker eligible to attack a player sets
                    // that player's candidate position. Visit attackers in
                    // handle order so those positions are public as well.
                    let mut keyed = subjects
                        .iter()
                        .map(|&object| Ok((self.object_handle_id(player, object)?, object)))
                        .collect::<Result<Vec<_>, String>>()?;
                    keyed.sort_by_key(|(handle, _)| *handle);
                    subjects = keyed.into_iter().map(|(_, object)| object).collect();
                    walkers
                } else {
                    vec![]
                };
                let mut slots = vec![];
                let mut rules = vec![];
                for subject in subjects {
                    let subject_index =
                        candidate_index(&mut candidates, Candidate::Object { object: subject });
                    let required = attacking.then(|| self.required_defender(subject)).flatten();
                    let mut allowed: Vec<_> = if attacking {
                        self.live_players()
                            .into_iter()
                            .filter(|p| {
                                *p != player
                                    && self.attack_permitted(subject, *p)
                                    && required.is_none_or(|d| d == *p)
                            })
                            .map(|p| {
                                candidate_index(&mut candidates, Candidate::Player { player: p })
                            })
                            .collect()
                    } else {
                        self.state
                            .rules
                            .combat
                            .iter()
                            .filter(|a| a.defender == player && self.can_block(subject, a.attacker))
                            .map(|a| {
                                candidate_index(
                                    &mut candidates,
                                    Candidate::Object { object: a.attacker },
                                )
                            })
                            .collect()
                    };
                    if attacking && required.is_none() {
                        allowed.extend(
                            walkers
                                .iter()
                                .filter(|(_, _, defender)| {
                                    self.is_live(*defender)
                                        && self.attack_defender_permitted(subject, *defender, true)
                                })
                                .map(|(_, object, _)| {
                                    candidate_index(
                                        &mut candidates,
                                        Candidate::Object { object: *object },
                                    )
                                }),
                        );
                    }
                    slots.push(Slot {
                        role: if attacking {
                            SlotRole::AttackWith
                        } else {
                            SlotRole::BlockWith
                        },
                        subject: Some(subject_index),
                        min: u32::from(
                            attacking
                                && !allowed.is_empty()
                                && (required.is_some() || self.must_attack(subject)),
                        ),
                        max: if attacking {
                            1
                        } else {
                            self.block_capacity(subject)
                                .min(allowed.len().max(1) as u32)
                        },
                        allowed,
                        repeat: false,
                    });
                }
                if !attacking {
                    // CR 509.1b, 702.111b: nonempty blocks of an attacker
                    // use between its least and greatest blocker counts.
                    let mut least = BTreeMap::<u32, Vec<u32>>::new();
                    let mut most = BTreeMap::<u32, Vec<u32>>::new();
                    for a in self
                        .state
                        .rules
                        .combat
                        .iter()
                        .filter(|a| a.defender == player)
                    {
                        let Some(index) = candidates
                            .iter()
                            .position(|c| c == &Candidate::Object { object: a.attacker })
                        else {
                            continue;
                        };
                        let (min, max) = self.blocker_count_bounds(a.attacker);
                        if min > 1 {
                            least.entry(min).or_default().push(index as u32);
                        }
                        if let Some(max) = max {
                            most.entry(max).or_default().push(index as u32);
                        }
                    }
                    rules.extend(
                        least
                            .into_iter()
                            .map(|(count, candidates)| JointRule::PickedAtLeast {
                                candidates,
                                count,
                            }),
                    );
                    rules.extend(
                        most.into_iter()
                            .map(|(count, candidates)| JointRule::PickedAtMost {
                                candidates,
                                count,
                            }),
                    );
                }
                self.decision(
                    player,
                    if attacking {
                        Reason::Attackers
                    } else {
                        Reason::Blockers
                    },
                    candidates,
                    Shape::Pick { slots, rules },
                )
            }
            Frame::DamageChoice { source } => {
                if let Some(shape) = self.joint_damage_groups(source, &mut candidates)? {
                    return Ok(Some(
                        self.decision(
                            player,
                            Reason::CombatDamage {
                                source,
                                power: self
                                    .characteristics(source)?
                                    .power
                                    .ok_or("missing creature power")?
                                    .max(Quantity::zero()),
                            },
                            candidates,
                            shape,
                        ),
                    ));
                }
                let total = self.combat_damage_amount(source)?;
                let recipients = self.recipients(source);
                let mut gates = vec![];
                candidates.extend(
                    recipients
                        .iter()
                        .map(|object| Candidate::Object { object: *object }),
                );
                if let Some(defender) = self.trample_defender(source) {
                    let requires = recipients
                        .into_iter()
                        .enumerate()
                        .map(|(i, o)| Ok((i as u32, self.lethal_requirement(source, o)?)))
                        .collect::<Result<Vec<_>, String>>()?;
                    gates.push(Gate {
                        bucket: candidates.len() as u32,
                        requires,
                    });
                    candidates.push(match defender {
                        DamageRecipient::Player { player } => Candidate::Player { player },
                        DamageRecipient::Permanent { object } => Candidate::Object { object },
                    });
                }
                let buckets = (0..candidates.len() as u32)
                    .map(|candidate| Bucket {
                        candidate,
                        min: 0.into(),
                        max: total.clone(),
                    })
                    .collect();
                self.decision(
                    player,
                    Reason::CombatDamage {
                        source,
                        power: self
                            .characteristics(source)?
                            .power
                            .ok_or("missing creature power")?
                            .max(Quantity::zero()),
                    },
                    candidates,
                    Shape::Distribute {
                        total,
                        buckets,
                        gates,
                    },
                )
            }
            Frame::Discard { count, .. } => {
                self.pick_objects(player, Reason::Discard, self.hand(player), count)
            }
            _ => return Ok(None),
        }))
    }
    pub(super) fn validate_attacks(
        &self,
        player: PlayerId,
        attacks: &[(ObjectId, PlayerId, Option<ObjectId>)],
    ) -> Result<(), String> {
        if !matches!(self.state.rules.frame, Frame::Attackers)
            || self.combat_decision_player() != Some(player)
            || !self.is_live(player)
        {
            return Err("invalid combat chooser".into());
        }
        let eligible = self.creatures(player, true);
        let mut seen = BTreeSet::new();
        if attacks.iter().any(|(o, p, planeswalker)| {
            !seen.insert(*o)
                || !eligible.contains(o)
                || *p == player
                || !self.is_live(*p)
                || !self.attack_defender_permitted(*o, *p, planeswalker.is_some())
                || planeswalker.is_some_and(|target| {
                    !self.is_battlefield_planeswalker(target)
                        || self
                            .current(target)
                            .is_ok_and(|target| target.controller != *p)
                })
                || self
                    .required_defender(*o)
                    .is_some_and(|d| d != *p || planeswalker.is_some())
        }) {
            return Err("invalid complete attack declaration".into());
        }
        // CR 508.1d: each eligible required attacker can independently
        // attack one of the living defenders it may attack in the admitted
        // combat model, which has no restriction across creatures.
        if eligible.iter().any(|&object| {
            !seen.contains(&object)
                && (self.required_defender(object).is_some() || self.must_attack(object))
                && ((0..4)
                    .map(PlayerId)
                    .any(|p| p != player && self.is_live(p) && self.attack_permitted(object, p))
                    || self
                        .attackable_planeswalkers(object, player)
                        .next()
                        .is_some())
        }) {
            return Err("attack declaration omits a creature that must attack".into());
        }
        Ok(())
    }
    pub(super) fn validate_blocks(
        &self,
        player: PlayerId,
        blocks: &[(ObjectId, ObjectId)],
    ) -> Result<(), String> {
        if !matches!(self.state.rules.frame, Frame::Blockers { .. })
            || self.combat_decision_player() != Some(player)
            || !self.is_live(player)
        {
            return Err("invalid combat chooser".into());
        }
        let eligible = self.creatures(player, false);
        let mut seen = BTreeSet::new();
        let mut uses = BTreeMap::<ObjectId, u32>::new();
        for &(blocker, attacker) in blocks {
            if !seen.insert((blocker, attacker))
                || !eligible.contains(&blocker)
                || !self.can_block(blocker, attacker)
                || !self
                    .state
                    .rules
                    .combat
                    .iter()
                    .any(|a| a.attacker == attacker && a.defender == player)
            {
                return Err("invalid complete block declaration".into());
            }
            let count = uses.entry(blocker).or_default();
            *count += 1;
            if *count > self.block_capacity(blocker) {
                return Err("blocker exceeds blocking capacity".into());
            }
        }
        // CR509.1b/702.111b: restrictions apply to the whole declaration.
        for a in self
            .state
            .rules
            .combat
            .iter()
            .filter(|a| a.defender == player)
        {
            let (least, most) = self.blocker_count_bounds(a.attacker);
            let count = blocks.iter().filter(|(_, x)| *x == a.attacker).count();
            if count > 0 && (count < least as usize || most.is_some_and(|m| count > m as usize)) {
                return Err("blocker count outside the attacker's restriction".into());
            }
        }
        Ok(())
    }
    fn validate_split(
        &self,
        source: ObjectId,
        assignments: &[(ObjectId, Quantity)],
        defender: Option<(DamageRecipient, &Quantity)>,
    ) -> Result<(), String> {
        let recipients = self.recipients(source);
        let mut seen = BTreeSet::new();
        let mut sum = Quantity::zero();
        for (o, n) in assignments {
            if !seen.insert(*o) || !recipients.contains(o) || n.is_negative() {
                return Err("invalid damage recipient or quantity".into());
            }
            sum.add(n);
        }
        if let Some((p, amount)) = defender {
            if self.trample_defender(source) != Some(p) || amount.is_negative() {
                return Err("invalid trample defender or quantity".into());
            }
            sum.add(amount);
            // CR702.19b/702.2c: ignore prevention/replacement; account for
            // marked damage and other assignments in this same step.
            if *amount > Quantity::zero() {
                for (o, n) in assignments {
                    if *n < self.lethal_requirement(source, *o)? {
                        return Err(
                            "trample damage to defender before lethal to every blocker".into()
                        );
                    }
                }
            }
        }
        // CR510.1c/e: no order; validate the entire source distribution.
        // https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
        if seen.len() != recipients.len() || sum != self.combat_damage_amount(source)? {
            return Err("incomplete damage distribution".into());
        }
        Ok(())
    }
    pub(super) fn remove_from_combat(&mut self, object: ObjectId) {
        // CR509.1g/506.4: removing an attacker does not remove its blockers.
        // The links below determine damage recipients (510.1d), not status.
        self.rules_mut().blocking_creatures.remove(&object);
        self.rules_mut().combat.retain(|a| a.attacker != object);
        for a in &mut self.rules_mut().combat {
            a.blockers.retain(|b| *b != object);
            a.defender_removed |= a.planeswalker == Some(object);
        }
        self.rules_mut().damage_choices.remove(&object);
        self.rules_mut().defender_damage_choices.remove(&object);
    }
    pub(crate) fn is_blocking_creature(&self, object: ObjectId) -> bool {
        self.state.rules.blocking_creatures.contains(&object)
            && self.is_battlefield_creature(object)
    }
    pub(super) fn clear_combat(&mut self) {
        self.rules_mut().combat.clear();
        self.rules_mut().blocking_creatures.clear();
        self.rules_mut().first_strike_participants.clear();
        self.rules_mut().damage_choices.clear();
        self.rules_mut().defender_damage_choices.clear();
        self.rules_mut().combat_defenders.clear();
    }
    pub(super) fn grant_turn_priority(&mut self) -> Result<(), String> {
        if matches!(self.state.rules.frame, Frame::Finished { .. } | Frame::Draw) {
            return Ok(());
        }
        self.rules_mut().priority = self
            .state
            .rules
            .active
            .or_else(|| self.next_live(self.state.rules.turn_owner))
            .ok_or("no priority recipient")?;
        self.rules_mut().frame = Frame::Priority;
        Ok(self.stabilize()?)
    }
    pub(super) fn schedule_stabilization(&mut self, resume: Frame, cleanup: bool) {
        self.rules_mut().frame = Frame::Stabilize {
            resume: self.save_frame(resume),
            cleanup,
            changed: false,
        };
    }
    pub(crate) fn combat_step(&mut self, frame: Frame) -> Result<(), String> {
        match frame {
            Frame::Attackers => {
                let input = self.rules_mut().pending.take().ok_or("missing attackers")?;

                let attacks = input
                    .declarations()?
                    .into_iter()
                    .map(|(subject, target)| match target {
                        Candidate::Player { player } => Ok((subject, *player, None)),
                        Candidate::Object { object } => {
                            Ok((subject, self.current(*object)?.controller, Some(*object)))
                        }
                        _ => Err("invalid combat target".to_owned()),
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                self.commit(
                    ProposedEvent::DeclareAttackers { attacks },
                    BatchCause::Combat,
                )?;
                self.grant_turn_priority()?;
            }
            Frame::Blockers { mut remaining } => {
                let input = self.rules_mut().pending.take().ok_or("missing blockers")?;

                let blocks = input
                    .declarations()?
                    .into_iter()
                    .map(|(subject, target)| match target {
                        Candidate::Object { object } => Ok((subject, *object)),
                        _ => Err("invalid combat target"),
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                self.commit(
                    ProposedEvent::DeclareBlockers {
                        player: input.player,
                        blocks,
                    },
                    BatchCause::Combat,
                )?;
                remaining.remove(0);
                if remaining.is_empty() {
                    self.grant_turn_priority()?;
                } else {
                    self.rules_mut().frame = Frame::Blockers { remaining };
                }
            }
            Frame::DamageChoice { source } => {
                let input = self
                    .rules_mut()
                    .pending
                    .take()
                    .ok_or("missing damage assignment")?;

                if let (Shape::CombatDistribute { groups, .. }, Answer::Distribute { amounts }) =
                    (&input.decision.shape, &input.answer)
                {
                    let mut amounts = amounts.iter();
                    for group in groups {
                        let Candidate::Object { object: source } =
                            input.decision.candidate(group.source)?
                        else {
                            return Err("invalid damage source".into());
                        };
                        let allocations = group
                            .recipients
                            .iter()
                            .map(|&i| {
                                Ok((
                                    input.decision.candidate(i)?,
                                    amounts.next().ok_or("missing damage allocation")?.clone(),
                                ))
                            })
                            .collect::<Result<Vec<_>, String>>()?;
                        self.store_damage_allocation(*source, allocations)?;
                    }
                } else {
                    self.store_damage_allocation(source, input.allocations()?)?;
                }
                self.rules_mut().frame = Frame::Damage;
                self.assign_damage()?;
            }
            Frame::Damage => self.assign_damage()?,
            Frame::Discard { player, .. } => {
                let input = self.rules_mut().pending.take().ok_or("missing discard")?;

                let objects = input.objects()?;
                self.complete_graveyard_group(
                    BatchCause::Cleanup,
                    EventGroup::Simultaneous {
                        occurrences: objects
                            .into_iter()
                            .map(|object| ProposedEvent::DiscardCard { object, player })
                            .collect(),
                    },
                    GroupFinish::Cleanup,
                )?;
            }
            Frame::CleanupClear => {
                self.commit(ProposedEvent::CleanupEffects, BatchCause::Cleanup)?;
                self.schedule_stabilization(Frame::Transition, true);
            }
            Frame::Stabilize {
                resume,
                cleanup,
                changed,
            } => self.stabilization_wave(self.saved_frame(resume)?.clone(), cleanup, changed)?,
            _ => return Err("invalid combat continuation".into()),
        }
        Ok(())
    }
    /// CR510.4/702.7b/702.4b: actual first/double strike determines the extra step.
    pub(super) fn first_strikers(&self) -> BTreeSet<ObjectId> {
        self.state
            .rules
            .combat
            .iter()
            .map(|a| a.attacker)
            .chain(self.state.rules.blocking_creatures.iter().copied())
            .filter(|o| {
                self.has_keyword(*o, KeywordAbility::FirstStrike)
                    || self.has_keyword(*o, KeywordAbility::DoubleStrike)
            })
            .collect()
    }
    fn assigns_in_damage_step(&self, object: ObjectId) -> bool {
        if self.state.rules.turn.step == TurnStep::FirstStrikeDamage {
            self.state.rules.first_strike_participants.contains(&object)
        } else {
            !self.state.rules.first_strike_participants.contains(&object)
                || self.has_keyword(object, KeywordAbility::DoubleStrike)
        }
    }
    fn combat_defender(&self, attack: &Attacker) -> Option<DamageRecipient> {
        if attack.defender_removed {
            return None;
        }
        if let Some(object) = attack.planeswalker {
            (self.is_battlefield_planeswalker(object)
                && self
                    .current(object)
                    .is_ok_and(|target| target.controller == attack.defender))
            .then_some(DamageRecipient::Permanent { object })
        } else {
            self.is_live(attack.defender)
                .then_some(DamageRecipient::Player {
                    player: attack.defender,
                })
        }
    }

    fn trample_defender(&self, source: ObjectId) -> Option<DamageRecipient> {
        self.state
            .rules
            .combat
            .iter()
            .find(|a| a.attacker == source)
            .filter(|_| self.has_keyword(source, KeywordAbility::Trample))
            .and_then(|a| self.combat_defender(a))
    }
    pub(crate) fn lethal_requirement(
        &self,
        source: ObjectId,
        blocker: ObjectId,
    ) -> Result<Quantity, String> {
        let remaining = self.blocker_damage_remaining(source, blocker, true)?;
        Ok(if self.has_keyword(source, KeywordAbility::Deathtouch) {
            remaining.min(Quantity::from(1))
        } else {
            remaining
        })
    }

    fn blocker_damage_remaining(
        &self,
        source: ObjectId,
        blocker: ObjectId,
        include_choices: bool,
    ) -> Result<Quantity, String> {
        let mut remaining = self
            .characteristics(blocker)?
            .toughness
            .ok_or("missing blocker toughness")?;
        remaining.subtract(&self.current(blocker)?.marked_damage);
        // CR 702.19b: account for other assignments in this damage step.
        // Joint offers leave variable assignments for their combined gates.
        for a in self.state.rules.combat.iter().filter(|a| {
            a.attacker != source
                && a.blockers.contains(&blocker)
                && self.assigns_in_damage_step(a.attacker)
        }) {
            let amount = if include_choices
                && let Some(choice) = self.state.rules.damage_choices.get(&a.attacker)
            {
                choice
                    .iter()
                    .find(|(o, _)| *o == blocker)
                    .map(|(_, n)| n.clone())
                    .unwrap_or_default()
            } else if a.blockers.len() == 1 && self.trample_defender(a.attacker).is_none() {
                self.combat_damage_amount(a.attacker)?
            } else {
                Quantity::zero()
            };
            if amount > Quantity::zero() && self.has_keyword(a.attacker, KeywordAbility::Deathtouch)
            {
                return Ok(Quantity::zero());
            }
            remaining.subtract(&amount);
        }
        Ok(remaining.max(Quantity::zero()))
    }

    fn store_damage_allocation(
        &mut self,
        source: ObjectId,
        allocations: Vec<(&Candidate<Rules>, Quantity)>,
    ) -> Result<(), String> {
        let mut assignments = vec![];
        self.rules_mut().defender_damage_choices.remove(&source);
        for (candidate, amount) in allocations {
            match candidate {
                Candidate::Player { player } => {
                    self.rules_mut().defender_damage_choices.insert(
                        source,
                        (DamageRecipient::Player { player: *player }, amount),
                    );
                }
                Candidate::Object { object }
                    if self.trample_defender(source)
                        == Some(DamageRecipient::Permanent { object: *object }) =>
                {
                    self.rules_mut().defender_damage_choices.insert(
                        source,
                        (DamageRecipient::Permanent { object: *object }, amount),
                    );
                }
                Candidate::Object { object } => assignments.push((*object, amount)),
                _ => return Err("invalid damage recipient".into()),
            }
        }
        self.rules_mut().damage_choices.insert(source, assignments);
        Ok(())
    }

    fn assign_damage(&mut self) -> Result<(), String> {
        let mut damage = vec![];
        let mut seen = BTreeSet::new();
        for a in self.state.rules.combat.clone() {
            let amount = self.combat_damage_amount(a.attacker)?;
            let controller = self.current(a.attacker)?.controller;
            if self.assigns_in_damage_step(a.attacker) && amount > Quantity::zero() {
                if self.needs_damage_split(a.attacker, a.blockers.len()) {
                    let choice = self
                        .state
                        .rules
                        .damage_choices
                        .get(&a.attacker)
                        .filter(|v| {
                            self.validate_split(
                                a.attacker,
                                v,
                                self.state
                                    .rules
                                    .defender_damage_choices
                                    .get(&a.attacker)
                                    .map(|(p, n)| (*p, n)),
                            )
                            .is_ok()
                        });
                    let Some(choice) = choice else {
                        self.rules_mut().damage_choices.remove(&a.attacker);
                        self.rules_mut().frame = Frame::DamageChoice { source: a.attacker };
                        return Ok(());
                    };
                    if let Some((recipient, amount)) =
                        self.state.rules.defender_damage_choices.get(&a.attacker)
                        && *amount > Quantity::zero()
                    {
                        damage.push(DamageOccurrence {
                            source: a.attacker,
                            controller,
                            recipient: *recipient,
                            amount: amount.clone(),
                        });
                    }
                    for (object, amount) in choice {
                        if *amount > Quantity::zero() {
                            damage.push(DamageOccurrence {
                                source: a.attacker,
                                controller,
                                recipient: DamageRecipient::Permanent { object: *object },
                                amount: amount.clone(),
                            });
                        }
                    }
                } else if let Some(object) = a.blockers.first() {
                    damage.push(DamageOccurrence {
                        source: a.attacker,
                        controller,
                        recipient: DamageRecipient::Permanent { object: *object },
                        amount,
                    });
                } else if (!a.blocked || self.trample_defender(a.attacker).is_some())
                    && let Some(recipient) = self.combat_defender(&a)
                {
                    damage.push(DamageOccurrence {
                        source: a.attacker,
                        controller,
                        recipient,
                        amount,
                    });
                }
            }
        }
        // CR 510.1: finish the attacker's assignments before asking any
        // defending player about their blockers' damage.
        let mut blockers: Vec<_> = self
            .state
            .rules
            .combat
            .iter()
            .flat_map(|a| a.blockers.iter().copied())
            .filter(|blocker| seen.insert(*blocker))
            .collect();
        blockers.sort_by_key(|blocker| {
            self.current(*blocker).map_or((u8::MAX, None), |object| {
                (self.apnap_rank(object.controller), object.keyword_timestamp)
            })
        });
        for blocker in blockers {
            if !self.assigns_in_damage_step(blocker) {
                continue;
            }
            let amount = self.combat_damage_amount(blocker)?;
            if amount == Quantity::zero() {
                continue;
            }
            let controller = self.current(blocker)?.controller;
            let recipients = self.recipients(blocker);
            let allocation = if let [attacker] = recipients.as_slice() {
                vec![(*attacker, amount)]
            } else if let Some(choice) = self
                .state
                .rules
                .damage_choices
                .get(&blocker)
                .filter(|choice| self.validate_split(blocker, choice, None).is_ok())
            {
                choice.clone()
            } else {
                self.rules_mut().damage_choices.remove(&blocker);
                self.rules_mut().frame = Frame::DamageChoice { source: blocker };
                return Ok(());
            };
            damage.extend(
                allocation
                    .into_iter()
                    .filter(|(_, amount)| *amount > Quantity::zero())
                    .map(|(object, amount)| DamageOccurrence {
                        source: blocker,
                        controller,
                        recipient: DamageRecipient::Permanent { object },
                        amount,
                    }),
            );
        }
        if !damage.is_empty() {
            return self.prepare_combat_damage(damage);
        }
        self.rules_mut().damage_choices.clear();
        self.rules_mut().defender_damage_choices.clear();
        self.grant_turn_priority()
    }
    fn stabilization_wave(
        &mut self,
        resume: Frame,
        cleanup: bool,
        changed: bool,
    ) -> Result<(), String> {
        self.stabilization_wave_with_legends(resume, cleanup, changed, None)
    }
    pub(crate) fn stabilization_wave_with_legends(
        &mut self,
        resume: Frame,
        cleanup: bool,
        changed: bool,
        kept: Option<Vec<ObjectId>>,
    ) -> Result<(), String> {
        let parent = self.save_frame(resume);
        let groups = self.legend_groups()?;
        if kept.is_none() && !groups.is_empty() {
            self.rules_mut().frame = Frame::LegendRule {
                resume: self.save_frame(Frame::Stabilize {
                    resume: parent,
                    cleanup,
                    changed,
                }),
                groups,
                kept: vec![],
            };
            return Ok(());
        }
        // Freeze the CR 702.2b/704.5h observation window before deciding SBAs.
        // CR 704.3 repeats checks, not old damage. Marks remain until CR 514.2.
        // https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
        let checked_deathtouch = self
            .state
            .rules
            .objects
            .values()
            .filter(|o| o.present_on_battlefield() && o.deathtouch_damage)
            .map(|o| o.id)
            .collect::<Vec<_>>();
        let mut occurrences = vec![];
        // Nothing changes during this sweep: index the static sources once.
        // A reason exists only for a battlefield creature.
        let sources = self.static_source_index();
        for o in self.state.rules.objects.values() {
            if let Some(sacrifice) = self.saga_sacrifice(o, Some(&sources))? {
                occurrences.push(sacrifice);
                continue;
            }
            let reason = self.creature_sba_reason_with_sources(o.id, Some(&sources))?;
            if let Some(reason) = reason {
                occurrences.push(ProposedEvent::StateBasedGraveyard {
                    object: o.id,
                    reason,
                });
            }
        }
        // CR 704.5i: zero loyalty is a graveyard move, independent of destruction.
        for o in self.state.rules.objects.values() {
            if o.present_on_battlefield()
                && o.loyalty_counters == Quantity::zero()
                && self
                    .characteristics_with_sources(o.id, Some(&sources))?
                    .has_type(CardKind::Planeswalker)
            {
                occurrences.push(ProposedEvent::Move {
                    object: o.id,
                    destination: Zone::Graveyard {
                        owner: self.state.rules.cards[&o.id.card].owner,
                    },
                });
            }
        }
        self.attachment_sbas(&mut occurrences)?;
        // CR 613.7m: before anything reads a continuous result, reject one
        // controller's simultaneous timestamps whose order would decide it.
        self.reject_simultaneous_timestamp_order()?;
        // CR 704.5j: put the unchosen legends into their owners' graveyards.
        // This is neither destruction nor sacrifice (CR 701.8/701.21).
        for group in &groups {
            let keep = kept
                .as_ref()
                .and_then(|kept| kept.iter().find(|id| group.contains(id)))
                .ok_or("missing legend keep choice")?;
            for object in group.iter().filter(|id| *id != keep) {
                if !occurrences.iter().any(|e| {
                    matches!(e,
                    ProposedEvent::StateBasedGraveyard { object: id, .. }
                    | ProposedEvent::Move { object: id, .. }
                    | ProposedEvent::Sacrifice { object: id, .. } if id == object)
                }) {
                    occurrences.push(ProposedEvent::Move {
                        object: *object,
                        destination: Zone::Graveyard {
                            owner: self.state.rules.cards[&object.card].owner,
                        },
                    });
                }
            }
        }
        // CR 704.3, 704.5q: annihilation is part of the same wave; a
        // permanent leaving in it loses its counters anyway (CR 122.2).
        let leaving: BTreeSet<_> = occurrences
            .iter()
            .filter_map(|e| match e {
                ProposedEvent::StateBasedGraveyard { object, .. }
                | ProposedEvent::Move { object, .. }
                | ProposedEvent::Sacrifice { object, .. } => Some(*object),
                _ => None,
            })
            .collect();
        occurrences.extend(
            self.state
                .rules
                .objects
                .values()
                .filter(|o| !leaving.contains(&o.id))
                .filter_map(Self::annihilation),
        );
        // CR 111.7/704.5d-e: the previous movement already collected dies
        // triggers using lookback. Ceasing is its own next SBA wave.
        occurrences.extend(
            self.state
                .rules
                .objects
                .values()
                .filter_map(|o| self.cessation(o)),
        );
        let losses = self
            .live_players()
            .into_iter()
            .filter_map(|p| {
                let x = &self.state.rules.players[p.0 as usize];
                if x.life <= Quantity::zero() {
                    Some((p, PlayerLossReason::Life))
                } else if x.poison_counters >= Quantity::from(10) {
                    Some((p, PlayerLossReason::Poison))
                } else if x.failed_draw {
                    Some((p, PlayerLossReason::FailedDraw))
                } else if self
                    .state
                    .rules
                    .commanders
                    .combat_loss_candidates()
                    .contains(&p)
                {
                    Some((p, PlayerLossReason::CommanderDamage))
                } else {
                    None
                }
            })
            .collect::<Vec<_>>();
        if occurrences.is_empty() && losses.is_empty() {
            self.consume_checked_deathtouch(&checked_deathtouch)?;
        }
        if occurrences.is_empty()
            && losses.is_empty()
            && self.offer_commander_sba(Frame::Stabilize {
                resume: parent,
                cleanup,
                changed,
            })?
        {
            return Ok(());
        }
        if occurrences.is_empty()
            && losses.is_empty()
            && self.begin_trigger_placement(self.saved_frame(parent)?.clone(), cleanup, changed)?
        {
            return Ok(());
        }
        if occurrences.is_empty() && losses.is_empty() {
            debug_assert!(
                self.state
                    .rules
                    .objects
                    .values()
                    .all(|o| o.zone != Zone::Battlefield || !o.deathtouch_damage)
            );
            self.rules_mut().frame = if cleanup && changed {
                self.rules_mut().cleanup_repeat = true;
                Frame::Priority
            } else {
                self.saved_frame(parent)?.clone()
            };
            if cleanup && changed && matches!(self.state.rules.frame, Frame::Priority) {
                self.rules_mut().priority = self
                    .state
                    .rules
                    .active
                    .or_else(|| self.next_live(self.state.rules.turn_owner))
                    .ok_or("no stabilization priority")?;
            }
            if matches!(self.state.rules.frame, Frame::Priority)
                && self.state.rules.pending.is_none()
                && self.state.rules.pending_concessions.is_empty()
                && self.all_live_passed()
            {
                self.begin_resolution()?;
            }
            return Ok(());
        }
        // CR 704.3/704.5: capture the whole wave before removal or winner tests.
        // All constituent changes commit in this single atomic rules unit.
        occurrences.extend(losses.iter().map(|(player, reason)| ProposedEvent::Lose {
            player: *player,
            reason: *reason,
        }));
        let resume = self.save_frame(Frame::Stabilize {
            resume: parent,
            cleanup,
            changed: true,
        });
        self.complete_graveyard_group(
            BatchCause::StateBased,
            EventGroup::Simultaneous { occurrences },
            GroupFinish::StateBased {
                resume,
                checked: checked_deathtouch,
                kept: kept.unwrap_or_default(),
            },
        )
    }
    pub(super) fn creature_sba_reason(
        &self,
        object: ObjectId,
    ) -> Result<Option<CreatureLossReason>, String> {
        self.creature_sba_reason_with_sources(object, None)
    }
    fn creature_sba_reason_with_sources(
        &self,
        object: ObjectId,
        sources: Option<&[CardId]>,
    ) -> Result<Option<CreatureLossReason>, String> {
        // One derivation decides "battlefield creature" and supplies toughness.
        let Ok(o) = self.current(object) else {
            return Ok(None);
        };
        if !o.present_on_battlefield() {
            return Ok(None);
        }
        let characteristics = match self.characteristics_with_sources(object, sources) {
            Ok(characteristics) if characteristics.has_type(CardKind::Creature) => characteristics,
            _ => return Ok(None),
        };
        let toughness = characteristics.toughness.ok_or("missing toughness")?;
        let reason = if toughness <= Quantity::zero() {
            Some(CreatureLossReason::ZeroToughness)
        } else if self
            .current_keywords_with_sources(object, sources)
            .contains(&KeywordAbility::Indestructible)
        {
            // CR 702.12b/704.5g-h: ignore destructive SBAs entirely.
            None
        } else if o.marked_damage >= toughness {
            Some(CreatureLossReason::LethalDamage)
        } else if o.deathtouch_damage {
            Some(CreatureLossReason::Deathtouch)
        } else {
            None
        };
        Ok(reason)
    }
    pub(super) fn consume_checked_deathtouch(
        &mut self,
        checked: &[ObjectId],
    ) -> Result<(), String> {
        let objects = checked
            .iter()
            .copied()
            .filter(|id| {
                self.current(*id)
                    .is_ok_and(|o| o.present_on_battlefield() && o.deathtouch_damage)
            })
            .collect::<Vec<_>>();
        if !objects.is_empty() {
            self.commit(
                ProposedEvent::ConsumeDeathtouchWindow { objects },
                BatchCause::DeathtouchCheck,
            )?;
        }
        Ok(())
    }
    pub(super) fn finish_if_ended(&mut self) -> Result<bool, String> {
        match self.live_players().as_slice() {
            [] => {
                self.rules_mut().pending = None;
                self.rules_mut().frame = Frame::Draw;
                self.record_event(Event::GameDrawn)?;
                Ok(true)
            }
            [winner] => {
                let winner = *winner;
                self.rules_mut().pending = None;
                self.rules_mut().frame = Frame::Finished { winner };
                self.rules_mut().priority = winner;
                self.record_event(Event::GameWon { winner })?;
                Ok(true)
            }
            _ => Ok(false),
        }
    }
    pub(super) fn revalidate_combat_departure(&mut self) -> Result<(), String> {
        match &self.state.rules.frame {
            Frame::Attackers if self.state.rules.active.is_none() => self.grant_turn_priority()?,
            Frame::Blockers { remaining } => {
                let remaining = remaining
                    .iter()
                    .copied()
                    .filter(|p| self.is_live(*p))
                    .collect::<Vec<_>>();
                if remaining.is_empty() {
                    self.grant_turn_priority()?;
                } else {
                    self.rules_mut().frame = Frame::Blockers { remaining };
                }
            }
            Frame::DamageChoice { source }
                if self.current(*source).is_err()
                    || (self.recipients(*source).len() < 2
                        && (self.recipients(*source).is_empty()
                            || self.trample_defender(*source).is_none())) =>
            {
                self.rules_mut().pending = None;
                self.rules_mut().frame = Frame::Damage;
            }
            Frame::Discard { player, .. } if !self.is_live(*player) => {
                self.rules_mut().pending = None;
                self.rules_mut().frame = Frame::CleanupClear;
            }
            _ => {}
        }
        if self
            .state
            .rules
            .pending
            .as_ref()
            .is_some_and(|p| self.validate_pending(p).is_err())
        {
            self.rules_mut().pending = None;
        }
        Ok(())
    }
    pub(super) fn validate_combat_state(&self) -> Result<(), String> {
        // CR 508.6, 702.121a: this is the historical set of players attacked
        // directly by declared attackers, in seat order. Neither attackers
        // removed later nor tokens created attacking change that set.
        let defenders = &self.state.rules.combat_defenders;
        if (!defenders.is_empty()
            && (!self.state.rules.combat_declared
                || self.state.rules.turn.step.phase() != Phase::Combat))
            || defenders.windows(2).any(|pair| pair[0] >= pair[1])
            || defenders.iter().any(|p| {
                !self.state.rules.original_seating.contains(p)
                    || Some(*p) == self.state.rules.active
            })
        {
            return Err("declared combat defender invariant".into());
        }
        // CR509.1g/506.4: an orphan blocker may have no damage link, but every
        // retained member is the exact creature still in combat. Declaration
        // provenance (including coherent extra members) is proved by replay.
        if self.state.rules.blocking_creatures.iter().any(|object| {
            !self.is_battlefield_creature(*object)
                || !self
                    .current(*object)
                    .is_ok_and(|o| self.is_live(o.controller))
                || self
                    .state
                    .rules
                    .combat
                    .iter()
                    .any(|a| a.attacker == *object)
        }) || self.state.rules.combat.iter().any(|a| {
            a.blockers
                .iter()
                .any(|b| !self.state.rules.blocking_creatures.contains(b))
        }) || (self.state.rules.turn.step.phase() != Phase::Combat
            && !self.state.rules.blocking_creatures.is_empty())
        {
            return Err("blocking creature membership invariant".into());
        }
        for (p, library) in &self.state.rules.libraries {
            let actual = self
                .state
                .rules
                .objects
                .values()
                .filter(|o| o.zone == (Zone::Library { owner: *p }))
                .map(|o| o.id.card)
                .collect::<BTreeSet<_>>();
            if library.iter().copied().collect::<BTreeSet<_>>() != actual
                || actual.len() != library.len()
            {
                return Err("library order membership invariant".into());
            }
        }
        if self
            .state
            .rules
            .objects
            .values()
            .any(|o| o.marked_damage.is_negative())
        {
            return Err("negative damage".into());
        }
        Ok(())
    }
}
