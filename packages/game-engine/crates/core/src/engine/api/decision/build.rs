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
        Decision {
            id: self.state.rules.revision,
            player,
            reason,
            candidates,
            shape,
        }
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
            if priority && (hand || command || graveyard) {
                for face in self.definition(id.card)?.faces() {
                    if hand && self.validate_land_face(player, id.card, face).is_ok() {
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
    ) -> Result<Decision<Rules>, String> {
        let pool = ManaPool {
            unrestricted: self.state.rules.players[player.0 as usize].mana.clone(),
        };
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
        let mut candidates = vec![];
        let mut slots = vec![];
        for (index, symbol) in self
            .copiable_definition(entry.source)?
            .characteristics
            .hybrid_mana
            .iter()
            .enumerate()
        {
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
        Ok(self.decision(
            entry.controller,
            Reason::HybridCost,
            candidates,
            Shape::Pick {
                slots,
                rules: vec![],
            },
        ))
    }

    pub(in crate::engine) fn target_decision(&self) -> Result<Decision<Rules>, String> {
        let entry = self.top()?;
        let mut candidates = vec![];
        let mut slots = vec![];
        let program = self.program()?;
        let grouped = program.optional_target && program.targets.len() > 1;
        let specs = if grouped {
            &program.targets[..1]
        } else {
            &program.targets
        };
        for spec in specs {
            let allowed = match spec.domain {
                TargetDomain::AnyDamageTarget => self
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
                TargetDomain::ControlledBattlefieldCreature
                | TargetDomain::ControlledBattlefieldPermanent
                | TargetDomain::BattlefieldCreature
                | TargetDomain::BattlefieldPermanent
                | TargetDomain::OwnGraveyardCard
                | TargetDomain::OwnGraveyardCreatureCard => self
                    .state
                    .rules
                    .objects
                    .values()
                    .filter(|o| self.object_target_legal_for(entry, o.id, Some(spec)))
                    .map(|o| candidate_index(&mut candidates, Candidate::Object { object: o.id }))
                    .collect(),
                _ => return Err("target domain outside the admitted decision algebra".into()),
            };
            slots.push(Slot {
                role: SlotRole::Target { spec: spec.slot },
                subject: None,
                allowed,
                min: u32::from(!program.optional_target),
                max: if grouped {
                    program.targets.len() as u32
                } else {
                    1
                },
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
