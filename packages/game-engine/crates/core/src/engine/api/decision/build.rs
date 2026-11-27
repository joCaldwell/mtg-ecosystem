use super::*;
use crate::engine::*;

impl RulesTxn {
    pub(in crate::engine) fn decision(
        &self,
        player: PlayerId,
        reason: Reason<Rules>,
        candidates: Vec<Candidate<Rules>>,
        shape: Shape,
    ) -> Decision<Rules> {
        let (candidates, shape) = self.public_candidate_order(player, &reason, candidates, shape);
        Decision {
            id: self.state.rules.revision,
            player,
            reason,
            candidates,
            shape,
        }
    }

    /// Order object candidates by the viewer's known facts and issued handles
    /// (CR 400.2, 708.5-708.6). Remap shape indices to the same candidates.
    fn public_candidate_order(
        &self,
        player: PlayerId,
        reason: &Reason<Rules>,
        mut candidates: Vec<Candidate<Rules>>,
        shape: Shape,
    ) -> (Vec<Candidate<Rules>>, Shape) {
        let object = |c: &Candidate<Rules>| match c {
            Candidate::Object { object } => Some(*object),
            _ => None,
        };
        // These choices already present the sequence of cards the viewer
        // looked at (CR 701.20e, 701.22a, 701.25a). Searching a library
        // does not grant that positional information (CR 701.23a).
        if matches!(
            reason,
            Reason::LookChooseHand
                | Reason::ScryBottom
                | Reason::SurveilGraveyard
                | Reason::OrderLibraryTop
                | Reason::OrderLibraryBottom
        ) {
            return (candidates, shape);
        }
        let positions: Vec<usize> = (0..candidates.len())
            .filter(|i| object(&candidates[*i]).is_some())
            .collect();
        if positions.len() < 2 {
            return (candidates, shape);
        }
        let key = |o: ObjectId| {
            let live = self.current(o).ok();
            let zone = live.map(|o| o.zone);
            let public = match zone {
                Some(Zone::Battlefield) => (
                    0,
                    live.and_then(|o| o.keyword_timestamp).unwrap_or(u64::MAX),
                ),
                Some(Zone::Stack) => (
                    1,
                    self.state
                        .rules
                        .stack
                        .iter()
                        .position(|e| e.source == o)
                        .map_or(u64::MAX, |p| p as u64),
                ),
                _ => (2, 0),
            };
            // Publication discovers the decider before minting newly visible
            // handles. That discovery discards the candidate table. A
            // projected decision requires an issued handle for every object.
            (public, self.object_handle_id(player, o).ok())
        };
        let mut sorted = positions.clone();
        sorted.sort_by_cached_key(|i| key(object(&candidates[*i]).expect("object candidate")));
        // `map[old] = new` for every candidate index.
        let mut map: Vec<u32> = (0..candidates.len() as u32).collect();
        for (slot, old) in positions.iter().zip(&sorted) {
            map[*old] = *slot as u32;
        }
        let mut reordered = candidates.clone();
        for (old, row) in candidates.drain(..).enumerate() {
            reordered[map[old] as usize] = row;
        }
        let m = |i: &u32| map.get(*i as usize).copied().unwrap_or(*i);
        let shape = match shape {
            Shape::Pick { slots, rules } => {
                let slots: Vec<Slot> = slots
                    .into_iter()
                    .map(|slot| {
                        let mut allowed: Vec<u32> = slot.allowed.iter().map(m).collect();
                        allowed.sort_unstable();
                        Slot {
                            subject: slot.subject.as_ref().map(m),
                            allowed,
                            ..slot
                        }
                    })
                    .collect();
                // Slots about one object each (attackers, blockers) follow
                // their subjects' public order among the subject slots;
                // other slots keep their places. Joint rules name slots by
                // ordinal and are remapped with them.
                let subjects: Vec<usize> = (0..slots.len())
                    .filter(|i| slots[*i].subject.is_some())
                    .collect();
                let mut by_subject = subjects.clone();
                by_subject.sort_by_key(|i| slots[*i].subject);
                let mut slot_map: Vec<u32> = (0..slots.len() as u32).collect();
                for (place, old) in subjects.iter().zip(&by_subject) {
                    slot_map[*old] = *place as u32;
                }
                let mut ordered = slots.clone();
                for (old, slot) in slots.into_iter().enumerate() {
                    ordered[slot_map[old] as usize] = slot;
                }
                let s = |i: &u32| slot_map.get(*i as usize).copied().unwrap_or(*i);
                Shape::Pick {
                    slots: ordered,
                    rules: rules
                        .into_iter()
                        .map(|rule| match rule {
                            JointRule::PickedAtLeast { candidates, count } => {
                                let mut candidates: Vec<u32> = candidates.iter().map(m).collect();
                                candidates.sort_unstable();
                                JointRule::PickedAtLeast { candidates, count }
                            }
                            JointRule::PickedAtMost { candidates, count } => {
                                let mut candidates: Vec<u32> = candidates.iter().map(m).collect();
                                candidates.sort_unstable();
                                JointRule::PickedAtMost { candidates, count }
                            }
                            JointRule::Distinct { slots } => {
                                let mut slots: Vec<u32> = slots.iter().map(s).collect();
                                slots.sort_unstable();
                                JointRule::Distinct { slots }
                            }
                            rule @ JointRule::Total { .. } => rule,
                        })
                        .collect(),
                }
            }
            Shape::Order { items, min, max } => {
                let mut items: Vec<u32> = items.iter().map(m).collect();
                items.sort_unstable();
                Shape::Order { items, min, max }
            }
            Shape::Distribute {
                total,
                buckets,
                gates,
            } => {
                // Buckets follow their candidates' public order; gates name
                // bucket ordinals.
                let mut order: Vec<usize> = (0..buckets.len()).collect();
                order.sort_by_key(|b| m(&buckets[*b].candidate));
                let mut ordinal = vec![0u32; buckets.len()];
                for (new, old) in order.iter().enumerate() {
                    ordinal[*old] = new as u32;
                }
                let o = |b: &u32| ordinal.get(*b as usize).copied().unwrap_or(*b);
                Shape::Distribute {
                    total,
                    buckets: order
                        .iter()
                        .map(|b| Bucket {
                            candidate: m(&buckets[*b].candidate),
                            ..buckets[*b].clone()
                        })
                        .collect(),
                    gates: gates
                        .into_iter()
                        .map(|gate| Gate {
                            bucket: o(&gate.bucket),
                            requires: {
                                let mut requires: Vec<_> = gate
                                    .requires
                                    .iter()
                                    .map(|(b, q)| (o(b), q.clone()))
                                    .collect();
                                requires.sort_by_key(|(b, _)| *b);
                                requires
                            },
                        })
                        .collect(),
                }
            }
            Shape::CombatDistribute { mut groups, lethal } => {
                for group in &mut groups {
                    group.source = m(&group.source);
                    group.recipients = group.recipients.iter().map(m).collect();
                }
                Shape::CombatDistribute { groups, lethal }
            }
            Shape::Pay {
                cost,
                pool,
                step: PayStep::Produce { sources },
            } => Shape::Pay {
                cost,
                pool,
                step: PayStep::Produce {
                    sources: {
                        let mut sources: Vec<u32> = sources.iter().map(m).collect();
                        sources.sort_unstable();
                        sources
                    },
                },
            },
            shape @ (Shape::Number { .. } | Shape::Pay { .. }) => shape,
        };
        (reordered, shape)
    }

    pub(in crate::engine) fn pick(
        &self,
        player: PlayerId,
        reason: Reason<Rules>,
        candidates: Vec<Candidate<Rules>>,
        min: u32,
        max: u32,
    ) -> Decision<Rules> {
        let allowed = (0..candidates.len() as u32).collect();
        self.decision(
            player,
            reason,
            candidates,
            Shape::Pick {
                slots: vec![Slot {
                    role: SlotRole::Choice,
                    subject: None,
                    allowed,
                    min,
                    max,
                    repeat: false,
                }],
                rules: vec![],
            },
        )
    }

    pub(in crate::engine) fn pick_flags(
        &self,
        player: PlayerId,
        reason: Reason<Rules>,
        flags: &[Flag],
    ) -> Decision<Rules> {
        self.pick(
            player,
            reason,
            flags
                .iter()
                .map(|flag| Candidate::Flag { flag: *flag })
                .collect(),
            1,
            1,
        )
    }

    pub(in crate::engine) fn pick_objects(
        &self,
        player: PlayerId,
        reason: Reason<Rules>,
        objects: Vec<ObjectId>,
        count: usize,
    ) -> Decision<Rules> {
        let count =
            u32::try_from(count).expect("objects are bounded by the admitted card capacity");
        self.pick(
            player,
            reason,
            objects
                .into_iter()
                .map(|object| Candidate::Object { object })
                .collect(),
            count,
            count,
        )
    }

    pub(in crate::engine) fn order(
        &self,
        player: PlayerId,
        reason: Reason<Rules>,
        candidates: Vec<Candidate<Rules>>,
        count: u32,
    ) -> Decision<Rules> {
        let items = (0..candidates.len() as u32).collect();
        self.decision(
            player,
            reason,
            candidates,
            Shape::Order {
                items,
                min: count,
                max: count,
            },
        )
    }

    fn beginning_candidates(
        &self,
        player: PlayerId,
        priority: bool,
    ) -> Result<Vec<Candidate<Rules>>, String> {
        let mut actions = Vec::new();
        if priority {
            actions.push(Action::Pass);
        }
        let visible = &self.state.knowledge.visibility[player.0 as usize]
            .handles
            .entries;
        for &id in visible.values() {
            // Visibility retains handles for known earlier incarnations.
            let Ok(obj) = self.current(id) else { continue };
            if !self.visible_to(player, obj) {
                continue;
            }
            let hand = obj.zone == (Zone::Hand { owner: player });
            let command = obj.zone == (Zone::Command { owner: player });
            let controlled = obj.present_on_battlefield() && obj.controller == player;
            // CR 712.11b-c, 712.12: evaluate each candidate using its chosen
            // face, including when the front and back have different types.
            let graveyard = obj.zone == (Zone::Graveyard { owner: player });
            // CR 715.3d: an exiled Adventure its card's controller may play.
            let adventure = obj.zone == Zone::Exile && obj.adventure_player == Some(player);
            if priority && (hand || command || graveyard || adventure) {
                for face in self.definition(id.card)?.faces() {
                    if (hand || adventure) && self.validate_land_face(player, id.card, face).is_ok()
                    {
                        actions.push(match face {
                            CardFace::Front => Action::PlayLand { object: id },
                            CardFace::Back => Action::PlayLandBackFace { object: id },
                        });
                    }
                    if self
                        .validate_priority_spell_cast(player, id.card, face)
                        .is_ok()
                    {
                        actions.push(match face {
                            CardFace::Front => Action::Cast { card: id },
                            CardFace::Back => Action::CastBackFace { card: id },
                        });
                    }
                }
                // CR 702.37a: a card with morph may also be cast face down.
                if (hand || command) && self.validate_face_down_cast(player, id.card).is_ok() {
                    actions.push(Action::CastFaceDown { card: id });
                }
            }
            // CR 116.2b, 702.37e: the turn-face-up special action.
            if priority && controlled && self.validate_turn_face_up(player, id).is_ok() {
                actions.push(Action::TurnFaceUp { object: id });
            }
            if controlled && self.validate_mana_activation(player, id).is_ok() {
                actions.push(Action::ActivateMana { object: id });
            }
            if priority && (controlled || hand || graveyard) && obj.hideaway.is_none() {
                for (index, _) in self.copiable_definition(id)?.activated.iter().enumerate() {
                    let ability =
                        u32::try_from(index).map_err(|_| "activated ability index limit")?;
                    if self.validate_activation(player, id, ability).is_ok() {
                        actions.push(Action::ActivateAbility {
                            object: id,
                            ability,
                        });
                    }
                }
            }
        }
        Ok(actions
            .into_iter()
            .map(|action| Candidate::Action { action })
            .collect())
    }

    pub(in crate::engine) fn priority_decision(
        &self,
        player: PlayerId,
    ) -> Result<Decision<Rules>, String> {
        Ok(self.pick(
            player,
            Reason::Priority,
            self.beginning_candidates(player, true)?,
            1,
            1,
        ))
    }

    pub(in crate::engine) fn payment_decision(
        &self,
        player: PlayerId,
        cost: ManaCost,
        source: Option<ObjectId>,
        produce: bool,
        use_on: payments::ManaUse,
    ) -> Result<Decision<Rules>, String> {
        let pool = self.mana_payment_pool(player, use_on)?;
        let (reason, step, candidates) = if produce {
            let candidates = self.beginning_candidates(player, false)?;
            (
                Reason::Mana { source },
                PayStep::Produce {
                    sources: (0..candidates.len() as u32).collect(),
                },
                candidates,
            )
        } else {
            (Reason::SpendMana { source }, PayStep::Spend, vec![])
        };
        Ok(self.decision(player, reason, candidates, Shape::Pay { cost, pool, step }))
    }

    pub(in crate::engine) fn hybrid_decision(&self) -> Result<Decision<Rules>, String> {
        let entry = self.top()?;
        let definition = self.stack_definition(entry)?;
        let symbols = match &entry.kind {
            StackKind::Activated { ability, .. } => {
                &definition
                    .activated
                    .get(*ability as usize)
                    .ok_or("missing hybrid activated ability")?
                    .hybrid_mana
            }
            _ => &definition.characteristics.hybrid_mana,
        };
        Ok(self.hybrid_pick(entry.controller, symbols))
    }

    /// CR 107.4e-f, 601.2b, 602.2b: one slot per printed hybrid symbol, in
    /// printed order, each offering that symbol's alternatives.
    pub(in crate::engine) fn hybrid_pick(
        &self,
        player: PlayerId,
        symbols: &[ColorHybridSymbol],
    ) -> Decision<Rules> {
        let mut candidates = vec![];
        let mut slots = vec![];
        for (index, symbol) in symbols.iter().enumerate() {
            let allowed = symbol
                .colors
                .iter()
                .map(|&payment| {
                    candidate_index(&mut candidates, Candidate::HybridPayment { payment })
                })
                .collect();
            slots.push(Slot {
                role: SlotRole::HybridSymbol {
                    index: index as u32,
                },
                subject: None,
                allowed,
                min: 1,
                max: 1,
                repeat: false,
            });
        }
        self.decision(
            player,
            Reason::HybridCost,
            candidates,
            Shape::Pick {
                slots,
                rules: vec![],
            },
        )
    }

    pub(in crate::engine) fn target_decision(&self) -> Result<Decision<Rules>, String> {
        let entry = self.top()?;
        let mut candidates = vec![];
        let mut slots = vec![];
        let program = self.program()?;
        // CR 115.3, 601.2c: one counted occurrence is one pick of distinct
        // objects: exactly its count, or any number up to it.
        let grouped = program.target_group().is_some();
        let specs = if grouped {
            &program.targets[..1]
        } else {
            &program.targets
        };
        for spec in specs {
            let allowed = match spec.domain {
                TargetDomain::AnyDamageTarget | TargetDomain::PlayerOrPlaneswalker => self
                    .eligible_entities()
                    .into_iter()
                    .map(|entity| {
                        let target = match entity {
                            TargetEntity::Player { player } => Candidate::Player { player },
                            TargetEntity::Object { object } => Candidate::Object { object },
                        };
                        candidate_index(&mut candidates, target)
                    })
                    .collect(),
                TargetDomain::AnyPlayer | TargetDomain::OpponentPlayer => self
                    .eligible_players(spec.domain, entry.controller)
                    .into_iter()
                    .map(|player| candidate_index(&mut candidates, Candidate::Player { player }))
                    .collect(),
                TargetDomain::StackSpell
                | TargetDomain::ControlledCreatureSpell
                | TargetDomain::StackActivatedOrTriggeredAbility => self
                    .eligible_stack_targets_for(spec, entry.controller, Some(entry.id))
                    .into_iter()
                    .map(|stack| candidate_index(&mut candidates, Candidate::Stack { stack }))
                    .collect(),
                TargetDomain::ControlledBattlefieldCreature
                | TargetDomain::ControlledBattlefieldPermanent
                | TargetDomain::BattlefieldCreature
                | TargetDomain::BattlefieldPermanent
                | TargetDomain::GraveyardCard { .. }
                | TargetDomain::OwnGraveyardCard
                | TargetDomain::OwnGraveyardCreatureCard => self
                    .state
                    .rules
                    .objects
                    .values()
                    .filter(|o| self.object_target_legal_for(entry, o.id, spec))
                    .map(|o| candidate_index(&mut candidates, Candidate::Object { object: o.id }))
                    .collect(),
                _ => return Err("target domain outside the admitted decision algebra".into()),
            };
            let (min, max) = if let Some(variable) = program.variable_target_count() {
                use mtg_contracts::kir::VariableTargetCount::*;
                let eligible =
                    u32::try_from(candidates.len()).map_err(|_| "target choice limit")?;
                let announced = entry.x.as_ref().and_then(Quantity::to_u32);
                match variable {
                    Any => (0, eligible),
                    ExactlyX => {
                        let x = announced.ok_or("invalid X target count")?;
                        (x, x)
                    }
                    UpToX => (0, eligible.min(announced.unwrap_or(u32::MAX))),
                }
            } else {
                let min = match (grouped, program.optional_target) {
                    (_, true) => 0,
                    (true, false) => program.targets.len() as u32,
                    (false, false) => 1,
                };
                (
                    min,
                    if grouped {
                        program.targets.len() as u32
                    } else {
                        1
                    },
                )
            };
            slots.push(Slot {
                role: SlotRole::Target { spec: spec.slot },
                subject: None,
                allowed,
                min,
                max,
                repeat: false,
            });
        }
        // A repeated occurrence of the same "target" cannot select a single
        // object twice. Distinct occurrences can (CR 115.3).
        let mut occurrences = BTreeMap::<&str, Vec<u32>>::new();
        for (slot, spec) in specs.iter().enumerate() {
            occurrences
                .entry(&spec.selection_id)
                .or_default()
                .push(slot as u32);
        }
        let rules = occurrences
            .into_values()
            .filter(|slots| slots.len() > 1)
            .map(|slots| JointRule::Distinct { slots })
            .collect();
        Ok(self.decision(
            entry.controller,
            Reason::Targets {
                specs: specs.to_vec(),
            },
            candidates,
            Shape::Pick { slots, rules },
        ))
    }
}

pub(in crate::engine) fn candidate_index(
    candidates: &mut Vec<Candidate<Rules>>,
    candidate: Candidate<Rules>,
) -> u32 {
    if let Some(index) = candidates.iter().position(|row| *row == candidate) {
        return index as u32;
    }
    let index = candidates.len() as u32;
    candidates.push(candidate);
    index
}
