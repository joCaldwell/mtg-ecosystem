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
    /// CR 608.2: a spell that resolves by following its instructions, cast
    /// from and functioning on the stack, with no costs, targets or modes yet.
    pub fn instructions(timing: SpellTiming) -> Self {
        Self {
            optional_additional_costs: vec![],
            additional_sacrifice: None,
            timing,
            resolution: SpellResolution::Instructions,
            functioning_zone: FunctioningZone::Stack,
            targets: vec![],
            instructions: vec![],
            modes: vec![],
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
    pub fn with_functioning_zone(mut self, zone: FunctioningZone) -> Self {
        self.functioning_zone = zone;
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
    pub fn with_modes(mut self, modes: impl IntoIterator<Item = SpellMode>) -> Self {
        self.modes.extend(modes);
        self
    }
    pub fn with_optional_additional_costs(
        mut self,
        costs: impl IntoIterator<Item = OptionalAdditionalManaCost>,
    ) -> Self {
        self.optional_additional_costs.extend(costs);
        self
    }
    pub fn with_additional_sacrifice(mut self, sacrifice: SacrificeFilter) -> Self {
        self.additional_sacrifice = Some(sacrifice);
        self
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
            targets: vec![],
            instructions: vec![],
            modes: vec![],
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
