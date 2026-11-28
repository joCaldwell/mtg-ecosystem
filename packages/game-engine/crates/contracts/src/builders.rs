//! Constructors for the IR structs that lowerers assemble most often.
//!
//! A struct literal names every field, so adding a field to `PermanentFilter`,
//! `TargetSpec` or `SpellProgram` touches every literal. These constructors
//! start from the value that imposes nothing beyond what the caller names; a
//! new field gets its neutral value here once. They are plain field setters:
//! serde shape, validation and runtime meaning stay with the types in `lib.rs`.
use super::*;

impl PermanentFilter {
    /// CR 115.1, 601.2c: no restriction; an empty type set means any permanent.
    pub fn any() -> Self {
        Self {
            types: vec![],
            non_types: vec![],
            non_colors: vec![],
            opponent: false,
            tapped: false,
            power: None,
            predicate: None,
        }
    }
    /// Adds alternative card types; a permanent of any listed type passes.
    pub fn with_types(mut self, types: impl IntoIterator<Item = CardKind>) -> Self {
        self.types.extend(types);
        self
    }
    /// Adds excluded card types ("nonland", "noncreature").
    pub fn with_non_types(mut self, non_types: impl IntoIterator<Item = CardKind>) -> Self {
        self.non_types.extend(non_types);
        self
    }
    /// Adds excluded colors ("nonblack").
    pub fn with_non_colors(mut self, non_colors: impl IntoIterator<Item = Color>) -> Self {
        self.non_colors.extend(non_colors);
        self
    }
    /// Requires an opponent to control the permanent.
    pub fn opponent(mut self) -> Self {
        self.opponent = true;
        self
    }
    /// Requires the current battlefield incarnation to be tapped (CR 110.5).
    pub fn tapped(mut self) -> Self {
        self.tapped = true;
        self
    }
    /// Requires derived creature power to satisfy `power` (CR 208.1, 608.2b).
    pub fn with_power(mut self, power: CreaturePowerFilter) -> Self {
        self.power = Some(power);
        self
    }
}

impl TargetSpec {
    pub fn requires_source_power(&self) -> bool {
        self.filter
            .as_ref()
            .is_some_and(|filter| filter.power == Some(CreaturePowerFilter::LessThanSource {}))
    }

    pub fn mana_value_at_most_announced_x(&self) -> bool {
        self.filter
            .as_ref()
            .and_then(|filter| filter.predicate.as_ref())
            .is_some_and(|predicate| predicate.0.iter().any(kir::mana_value_at_most_announced_x))
    }

    /// CR 601.2c, 115.1: the controller announces one target for `slot` as the
    /// spell or ability is put on the stack, with no filter beyond `domain`.
    pub fn new(slot: u32, selection_id: impl Into<String>, domain: TargetDomain) -> Self {
        Self {
            slot,
            selection_id: selection_id.into(),
            chooser: Chooser::Controller,
            stage: EvaluationStage::Announcement,
            domain,
            filter: None,
        }
    }
    pub fn with_chooser(mut self, chooser: Chooser) -> Self {
        self.chooser = chooser;
        self
    }
    pub fn with_stage(mut self, stage: EvaluationStage) -> Self {
        self.stage = stage;
        self
    }
    pub fn with_filter(mut self, filter: PermanentFilter) -> Self {
        self.filter = Some(filter);
        self
    }
}

impl SpellProgram {
    /// The variable bound for a single simultaneous target occurrence.
    pub fn variable_target_count(&self) -> Option<kir::VariableTargetCount> {
        self.instructions
            .iter()
            .find_map(|instruction| match instruction {
                Instruction::Kernel {
                    node:
                        kir::Node::TapTargets { variable, .. }
                        | kir::Node::RemoveTargets { variable, .. },
                    ..
                } => *variable,
                _ => None,
            })
    }

    /// CR 115.3, 601.2c: the number of objects one printed counted target
    /// occurrence ("two target", "up to three target") announces. Its slots
    /// share one domain and filter; slot zero keeps the archive selection
    /// identity and slot `k` appends `/k`. No other compiler route mints a
    /// `/` identity, so the shape alone identifies the group. A group is one
    /// decision whose picks are distinct objects.
    pub fn target_group(&self) -> Option<usize> {
        let [first, rest @ ..] = self.targets.as_slice() else {
            return None;
        };
        (!rest.is_empty()
            && !first.selection_id.contains('/')
            && rest.iter().enumerate().all(|(index, spec)| {
                spec.selection_id == format!("{}/{}", first.selection_id, index + 1)
                    && spec.domain == first.domain
                    && spec.filter == first.filter
                    && spec.chooser == first.chooser
                    && spec.stage == first.stage
            }))
        .then_some(self.targets.len())
    }
    /// CR 608.2: a spell that resolves by following its instructions, cast
    /// from and functioning on the stack, with no costs or targets yet.
    pub fn instructions(timing: SpellTiming) -> Self {
        Self {
            optional_additional_costs: vec![],
            additional_sacrifice: None,
            timing,
            resolution: SpellResolution::Instructions,
            functioning_zone: FunctioningZone::Stack,
            optional_target: false,
            targets: vec![],
            instructions: vec![],
        }
    }
    /// CR 302.1, 608.3: a permanent spell resolves onto the battlefield.
    pub fn permanent(timing: SpellTiming) -> Self {
        Self::instructions(timing).with_resolution(SpellResolution::Permanent)
    }
    pub fn with_resolution(mut self, resolution: SpellResolution) -> Self {
        self.resolution = resolution;
        self
    }
    pub fn with_targets(mut self, targets: impl IntoIterator<Item = TargetSpec>) -> Self {
        self.targets.extend(targets);
        self
    }
    pub fn with_instructions(
        mut self,
        instructions: impl IntoIterator<Item = Instruction>,
    ) -> Self {
        self.instructions.extend(instructions);
        self
    }
}

/// CR 702.109a, 603.7: the delayed trigger a resolving spell whose dash
/// cost was paid creates: "return the permanent this spell becomes to its
/// owner's hand at the beginning of the next end step". The dash keyword
/// line `block` prints it.
impl kir::Node {
    pub fn dash_return(block: u32) -> Self {
        let mut body = SpellProgram::instructions(SpellTiming::Instant);
        body.instructions.push(Instruction::Kernel {
            source_block: block,
            node: kir::Node::Do(kir::Verb::ReturnDashedToHand {}),
        });
        kir::Node::Delayed {
            event: kir::DelayedEvent::NextEndStepAfterDash,
            program: Box::new(body),
        }
    }
}

impl TriggeredAbility {
    /// CR 702.74a: "When this permanent enters, if its evoke cost was paid,
    /// its controller sacrifices it." The keyword line `block` prints it.
    pub fn evoke_sacrifice(block: u32) -> Self {
        let mut program = SpellProgram::instructions(SpellTiming::Instant);
        program.instructions.push(Instruction::Kernel {
            source_block: block,
            node: kir::Node::Do(kir::Verb::Sacrifice {
                player: kir::PlayerExpr::ControllerOf {
                    object: kir::Ref::Source,
                },
                count: kir::Amount::fixed(1),
                filter: kir::Pred::source(),
                greatest_power: false,
            }),
        });
        Self {
            modal: None,
            functioning_zone: FunctioningZone::Battlefield,
            condition: TriggerCondition::Event {
                pattern: kir::EventPattern::self_enters(),
                intervening_if: Some(kir::Cond::CostPaid {
                    identity: EVOKE_COST.into(),
                }),
                once_each_turn: false,
                your_turn: false,
            },
            program,
        }
    }
}

impl TriggeredAbility {
    /// CR 702.30a: "At the beginning of your upkeep, if this permanent came
    /// under your control since the beginning of your last upkeep, sacrifice
    /// it unless you pay [cost]." The keyword line `block` prints it.
    pub fn echo(block: u32, cost: ManaCost) -> Self {
        let you = kir::PlayerExpr::you(Context::Triggered);
        let payment = if cost.colored.is_empty() {
            kir::Payment::Generic {
                amount: kir::Amount::Fixed {
                    value: cost.generic,
                },
            }
        } else {
            kir::Payment::Mana { mana: cost }
        };
        let mut program = SpellProgram::instructions(SpellTiming::Instant);
        program.instructions.push(Instruction::Kernel {
            source_block: block,
            node: kir::Node::UnlessPays {
                payer: you.clone(),
                payment,
                node: Box::new(kir::Node::Do(kir::Verb::Sacrifice {
                    player: you,
                    count: kir::Amount::fixed(1),
                    filter: kir::Pred::source(),
                    greatest_power: false,
                })),
            },
        });
        Self {
            modal: None,
            functioning_zone: FunctioningZone::Battlefield,
            condition: TriggerCondition::Event {
                pattern: kir::EventPattern::your_upkeep(),
                intervening_if: Some(kir::Cond::ControlledSinceLastUpkeep {}),
                once_each_turn: false,
                your_turn: false,
            },
            program,
        }
    }
}

impl ActivatedAbility {
    /// CR 702.84a: "Unearth [cost]" means "[Cost]: Return this card from
    /// your graveyard to the battlefield. It gains haste. Exile it at the
    /// beginning of the next end step. If it would leave the battlefield,
    /// exile it instead of putting it anywhere else. Activate only as a
    /// sorcery." The keyword line `block` prints it.
    pub fn unearth(block: u32, cost: ManaCost) -> Self {
        let mut body = SpellProgram::instructions(SpellTiming::Instant);
        body.instructions.push(Instruction::Kernel {
            source_block: block,
            node: kir::Node::Do(kir::Verb::ExileCapturedReturn {}),
        });
        let mut program = SpellProgram::instructions(SpellTiming::MainPhase);
        program.instructions.push(Instruction::Kernel {
            source_block: block,
            node: kir::Node::Delayed {
                event: kir::DelayedEvent::NextEndStepAfterUnearth,
                program: Box::new(body),
            },
        });
        Self {
            functioning_zone: FunctioningZone::Graveyard,
            x_mana_cost: false,
            hybrid_mana: vec![],
            restriction: None,
            cost: ActivationCost::Mana { cost },
            program,
            modal: None,
            cost_changes: vec![],
        }
    }

    /// CR 702.141a: "Encore [cost]" means "[Cost], Exile this card from
    /// your graveyard: For each opponent, create a token that's a copy of
    /// this card that attacks that opponent this turn if able. The tokens
    /// gain haste. Sacrifice them at the beginning of the next end step.
    /// Activate only as a sorcery." The keyword line `block` prints it.
    pub fn encore(block: u32, cost: ManaCost) -> Self {
        let mut body = SpellProgram::instructions(SpellTiming::Instant);
        body.instructions.push(Instruction::Kernel {
            source_block: block,
            node: kir::Node::Do(kir::Verb::SacrificeEncoreTokens {}),
        });
        let mut program = SpellProgram::instructions(SpellTiming::MainPhase);
        program.instructions.push(Instruction::Kernel {
            source_block: block,
            node: kir::Node::Delayed {
                event: kir::DelayedEvent::NextEndStepAfterEncore,
                program: Box::new(body),
            },
        });
        Self {
            functioning_zone: FunctioningZone::Graveyard,
            x_mana_cost: false,
            hybrid_mana: vec![],
            restriction: None,
            cost: ActivationCost::ExileSource { mana: cost },
            program,
            modal: None,
            cost_changes: vec![],
        }
    }

    /// CR 702.141a: this is exactly an encore ability.
    pub fn is_encore(&self) -> bool {
        let ActivationCost::ExileSource { mana } = &self.cost else {
            return false;
        };
        let block = self.program.instructions.first().map(|i| match i {
            Instruction::Kernel { source_block, .. } => *source_block,
        });
        block.is_some_and(|block| *self == Self::encore(block, mana.clone()))
    }

    /// CR 702.84a: this is exactly an unearth ability.
    pub fn is_unearth(&self) -> bool {
        let ActivationCost::Mana { cost } = &self.cost else {
            return false;
        };
        let block = self.program.instructions.first().map(|i| match i {
            Instruction::Kernel { source_block, .. } => *source_block,
        });
        block.is_some_and(|block| *self == Self::unearth(block, cost.clone()))
    }
}

impl TriggeredAbility {
    /// CR 702.116a: "Whenever this creature attacks, for each opponent other
    /// than defending player, you may create a token that's a copy of this
    /// creature that's tapped and attacking that player or a planeswalker
    /// they control. If one or more tokens are created this way, exile the
    /// tokens at end of combat."
    pub fn myriad(block: u32) -> Self {
        let kernel = |node| Instruction::Kernel {
            source_block: block,
            node,
        };
        let mut exile = SpellProgram::instructions(SpellTiming::Instant);
        exile
            .instructions
            .push(kernel(kir::Node::Do(kir::Verb::ExileMyriadTokens {})));
        let mut program = SpellProgram::instructions(SpellTiming::Instant);
        program.instructions.push(kernel(kir::Node::ForEach {
            var: 0,
            over: kir::Domain::Players {
                players: kir::PlayerExpr::EachOpponent,
            },
            body: Box::new(kir::Node::Optional {
                chooser: kir::PlayerExpr::you(kir::Context::Triggered),
                node: Box::new(kir::Node::Do(kir::Verb::CreateMyriadToken {
                    defender: kir::PlayerExpr::Is {
                        who: kir::Ref::BoundVar { id: 0 },
                    },
                })),
            }),
        }));
        program.instructions.push(kernel(kir::Node::Delayed {
            event: kir::DelayedEvent::EndOfCombat,
            program: Box::new(exile),
        }));
        Self {
            modal: None,
            functioning_zone: FunctioningZone::Battlefield,
            condition: TriggerCondition::Event {
                pattern: kir::EventPattern::Attacks {
                    attacker: kir::Pred::source(),
                },
                intervening_if: None,
                once_each_turn: false,
                your_turn: false,
            },
            program,
        }
    }

    /// CR 702.25a: "Whenever this creature becomes blocked by a creature
    /// without flanking, the blocking creature gets -1/-1 until end of turn."
    pub fn flanking(block: u32) -> Self {
        let mut program = SpellProgram::instructions(SpellTiming::Instant);
        program.instructions.push(Instruction::Kernel {
            source_block: block,
            node: kir::Node::Do(kir::Verb::ModifyUntil {
                what: kir::SetExpr::One {
                    object: kir::Ref::EventObject {
                        role: kir::EventObjectRole::Blocker,
                    },
                },
                layer: kir::Layer::PowerToughnessModify,
                change: kir::Change::PowerToughness {
                    power: kir::Amount::fixed(-1),
                    toughness: kir::Amount::fixed(-1),
                },
                until: kir::Until::EndOfTurn,
            }),
        });
        Self {
            modal: None,
            functioning_zone: FunctioningZone::Battlefield,
            condition: TriggerCondition::on(kir::EventPattern::BlockedByCreature {
                attacker: kir::Pred::source(),
                blocker: kir::Pred(vec![
                    kir::Atom::Type {
                        kind: CardKind::Creature,
                    },
                    kir::Atom::NotKeyword {
                        keyword: KeywordAbility::Flanking,
                    },
                ]),
            }),
            program,
        }
    }

    /// CR 702.25a: this is exactly a flanking ability.
    pub fn is_flanking(&self) -> bool {
        let block = self.program.instructions.first().map(|i| match i {
            Instruction::Kernel { source_block, .. } => *source_block,
        });
        block.is_some_and(|block| *self == Self::flanking(block))
    }

    /// CR 702.116a: this is exactly a myriad ability.
    pub fn is_myriad(&self) -> bool {
        let block = self.program.instructions.first().map(|i| match i {
            Instruction::Kernel { source_block, .. } => *source_block,
        });
        block.is_some_and(|block| *self == Self::myriad(block))
    }

    /// Whether any program of this ability names a myriad verb.
    pub fn names_myriad(&self) -> bool {
        fn names(node: &kir::Node) -> bool {
            match node {
                kir::Node::Do(
                    kir::Verb::CreateMyriadToken { .. } | kir::Verb::ExileMyriadTokens {},
                ) => true,
                kir::Node::Do(_) => false,
                kir::Node::Seq(nodes) => nodes.iter().any(names),
                kir::Node::If {
                    then, otherwise, ..
                } => names(then) || otherwise.as_deref().is_some_and(names),
                kir::Node::Optional { node, .. }
                | kir::Node::UnlessPays { node, .. }
                | kir::Node::IfPays { node, .. } => names(node),
                kir::Node::ForEach { body, .. }
                | kir::Node::Choose { body, .. }
                | kir::Node::ChooseCreatureType { body, .. } => names(body),
                kir::Node::Delayed { program, .. } | kir::Node::Reflexive { program, .. } => {
                    program.instructions.iter().any(|i| match i {
                        Instruction::Kernel { node, .. } => names(node),
                    })
                }
                kir::Node::ChooseObject { .. }
                | kir::Node::TapTargets { .. }
                | kir::Node::SacrificeForTappedReturn { .. }
                | kir::Node::RemoveTargets { .. } => false,
            }
        }
        self.programs().any(|p| {
            p.instructions.iter().any(|i| match i {
                Instruction::Kernel { node, .. } => names(node),
            })
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Each constructor equals the literal it replaces and serializes to the
    // same JSON, so a migrated call site keeps its IR byte for byte.
    fn same<T: Serialize + PartialEq + fmt::Debug>(built: T, literal: T) {
        assert_eq!(
            serde_json::to_string(&built).unwrap(),
            serde_json::to_string(&literal).unwrap()
        );
        assert_eq!(built, literal);
    }

    #[test]
    fn permanent_filter_builders_set_only_the_named_fields() {
        same(
            PermanentFilter::any(),
            PermanentFilter {
                types: vec![],
                non_types: vec![],
                non_colors: vec![],
                opponent: false,
                tapped: false,
                power: None,
                predicate: None,
            },
        );
        let power = CreaturePowerFilter::AtLeast {
            value: "4".parse().unwrap(),
        };
        same(
            PermanentFilter::any()
                .with_types([CardKind::Creature])
                .with_types([CardKind::Artifact])
                .with_non_types([CardKind::Land])
                .with_non_colors([Color::B])
                .opponent()
                .tapped()
                .with_power(power.clone()),
            PermanentFilter {
                types: vec![CardKind::Creature, CardKind::Artifact],
                non_types: vec![CardKind::Land],
                non_colors: vec![Color::B],
                opponent: true,
                tapped: true,
                power: Some(power),
                predicate: None,
            },
        );
        // The unrestricted filter keeps the archived wire shape: optional
        // fields are omitted, required lists are present.
        assert_eq!(
            serde_json::to_value(PermanentFilter::any()).unwrap(),
            serde_json::json!({"types":[],"non_types":[],"non_colors":[],"opponent":false})
        );
    }

    #[test]
    fn target_spec_new_is_an_unfiltered_controller_announcement() {
        same(
            TargetSpec::new(2, "line:0:1", TargetDomain::BattlefieldCreature),
            TargetSpec {
                slot: 2,
                selection_id: "line:0:1".into(),
                chooser: Chooser::Controller,
                stage: EvaluationStage::Announcement,
                domain: TargetDomain::BattlefieldCreature,
                filter: None,
            },
        );
        let filter = PermanentFilter::any().with_types([CardKind::Artifact]);
        same(
            TargetSpec::new(0, "line:0:0", TargetDomain::BattlefieldPermanent)
                .with_chooser(Chooser::Controller)
                .with_stage(EvaluationStage::Announcement)
                .with_filter(filter.clone()),
            TargetSpec {
                slot: 0,
                selection_id: "line:0:0".into(),
                chooser: Chooser::Controller,
                stage: EvaluationStage::Announcement,
                domain: TargetDomain::BattlefieldPermanent,
                filter: Some(filter),
            },
        );
    }

    #[test]
    fn spell_program_constructors_start_empty_on_the_stack() {
        let literal = SpellProgram {
            optional_additional_costs: vec![],
            additional_sacrifice: None,
            timing: SpellTiming::Instant,
            resolution: SpellResolution::Instructions,
            functioning_zone: FunctioningZone::Stack,
            optional_target: false,
            targets: vec![],
            instructions: vec![],
        };
        same(
            SpellProgram::instructions(SpellTiming::Instant),
            literal.clone(),
        );
        same(
            SpellProgram::permanent(SpellTiming::MainPhase),
            SpellProgram {
                timing: SpellTiming::MainPhase,
                resolution: SpellResolution::Permanent,
                ..literal.clone()
            },
        );
        let target = TargetSpec::new(0, "line:0:0", TargetDomain::StackSpell);
        let instruction = Instruction::Kernel {
            node: kir::Node::Do(kir::Verb::Counter {
                what: kir::Ref::Target { slot: 0 },
            }),
            source_block: 0,
        };
        same(
            SpellProgram::instructions(SpellTiming::Instant)
                .with_targets([target.clone()])
                .with_instructions([instruction.clone()]),
            SpellProgram {
                targets: vec![target],
                instructions: vec![instruction],
                ..literal
            },
        );
    }
}
