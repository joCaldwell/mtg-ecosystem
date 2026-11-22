//! CR 107.3, 601.2b-c, 608.2b-c and 121.2: these spell effects use the same announced X.
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;

mod schema;
pub(super) use schema::extend_schema;

pub(super) fn has_x_instruction(p: &SpellProgram) -> bool {
    p.instructions.iter().any(|i| {
        matches!(
            i,
            kernel!(Draw {
                player: kir::PlayerExpr::Is { .. },
                amount: announced_x!()
            }) | kernel!(GainLife {
                amount: announced_x!(),
                ..
            }) | kernel!(LoseLife {
                amount: announced_x!(),
                ..
            }) | Instruction::DealDamageAnnouncedX { .. }
                | Instruction::MillAnnouncedX { .. }
        )
    }) || p.modes.iter().any(|m| has_x_instruction(&m.program))
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Shape {
    Draw,
    TargetDraw,
    TargetDrawLose,
    GainDraw,
    DrawDiscard,
    Damage(TargetDomain),
    DamageGain(TargetDomain),
    DamageDrawDiscard,
    TargetGain,
    MillGain,
    DrawOrDamage,
}

impl Shape {
    const DRAW: [Self; 5] = [
        Self::Draw,
        Self::TargetDraw,
        Self::TargetDrawLose,
        Self::GainDraw,
        Self::DrawDiscard,
    ];
    const EFFECTS: [Self; 8] = [
        Self::Damage(TargetDomain::AnyDamageTarget),
        Self::Damage(TargetDomain::BattlefieldCreature),
        Self::DamageGain(TargetDomain::AnyDamageTarget),
        Self::DamageGain(TargetDomain::BattlefieldCreature),
        Self::DamageDrawDiscard,
        Self::TargetGain,
        Self::MillGain,
        Self::DrawOrDamage,
    ];
    fn all() -> impl Iterator<Item = Self> {
        Self::DRAW.into_iter().chain(Self::EFFECTS)
    }

    fn domain(self) -> Option<TargetDomain> {
        match self {
            Self::TargetDraw | Self::TargetDrawLose | Self::TargetGain | Self::MillGain => {
                Some(TargetDomain::AnyPlayer)
            }
            Self::Damage(domain) | Self::DamageGain(domain) => Some(domain),
            Self::DamageDrawDiscard => Some(TargetDomain::AnyDamageTarget),
            _ => None,
        }
    }

    fn allows_arcane(self, kind: CardKind) -> bool {
        kind == CardKind::Sorcery && self == Self::DamageGain(TargetDomain::BattlefieldCreature)
    }

    fn instructions(self) -> Vec<Instruction> {
        let player = LifeRecipient::Target { target_slot: 0 };
        let draw = |recipient: LifeRecipient| {
            Instruction::act(
                Verb::Draw {
                    player: recipient.kernel(Context::Spell).expect("spell recipient"),
                    amount: kir::Amount::announced_x(),
                },
                0,
            )
        };
        let life = |recipient: LifeRecipient, gain| {
            let player = recipient.kernel(Context::Spell).expect("spell recipient");
            Instruction::announced_x_life(player, gain, 0)
        };
        let damage = Instruction::DealDamageAnnouncedX {
            target_slot: 0,
            source_block: 0,
        };
        let discard = Instruction::DiscardCards {
            recipient: None,
            count: 1.into(),
            source_block: 0,
        };
        let controller = LifeRecipient::Controller;
        match self {
            Self::Draw => vec![draw(controller)],
            Self::TargetDraw => vec![draw(player)],
            Self::TargetDrawLose => vec![draw(player.clone()), life(player, false)],
            Self::GainDraw => vec![life(controller.clone(), true), draw(controller)],
            Self::DrawDiscard => vec![draw(controller), discard],
            Self::Damage(_) => vec![damage],
            Self::DamageGain(_) => vec![damage, life(controller, true)],
            Self::DamageDrawDiscard => {
                let unit = Instruction::draw(PlayerExpr::you(Context::Spell), 1.into(), 0);
                vec![damage, unit.clone(), unit, discard]
            }
            Self::TargetGain => vec![life(player, true)],
            Self::MillGain => vec![
                Instruction::MillAnnouncedX {
                    target_slot: 0,
                    source_block: 0,
                },
                life(controller, true),
            ],
            Self::DrawOrDamage => vec![],
        }
    }

    fn modes(self) -> &'static [Self] {
        match self {
            Self::DrawOrDamage => &[Self::Draw, Self::Damage(TargetDomain::AnyDamageTarget)],
            _ => &[],
        }
    }

    // A representative program for computing the schema's exact requirements.
    // The actual compiler supplies the target occurrence's selection ID.
    fn program(self, timing: SpellTiming) -> SpellProgram {
        SpellProgram {
            timing,
            functioning_zone: FunctioningZone::Stack,
            resolution: SpellResolution::Instructions,
            additional_sacrifice: None,
            optional_additional_costs: vec![],
            targets: self
                .domain()
                .into_iter()
                .map(|domain| TargetSpec {
                    slot: 0,
                    selection_id: "target".into(),
                    domain,
                    filter: None,
                    chooser: Chooser::Controller,
                    stage: EvaluationStage::Announcement,
                })
                .collect(),
            instructions: self.instructions(),
            modes: self
                .modes()
                .iter()
                .map(|mode| SpellMode {
                    program: mode.program(timing),
                })
                .collect(),
        }
    }

    fn matches(self, p: &SpellProgram) -> bool {
        let target_matches = match (self.domain(), p.targets.as_slice()) {
            (None, []) => true,
            (Some(domain), [t]) => {
                t.slot == 0
                    && !t.selection_id.is_empty()
                    && t.domain == domain
                    && t.stage == EvaluationStage::Announcement
                    && t.chooser == Chooser::Controller
                    && t.filter.is_none()
            }
            _ => false,
        };
        if !target_matches
            || p.additional_sacrifice.is_some()
            || !p.optional_additional_costs.is_empty()
            || p.modes.len() != self.modes().len()
        {
            return false;
        }
        let mut instructions = self.instructions();
        if let Some(Instruction::DiscardCards {
            recipient: Some(ActionRecipient::Controller),
            ..
        }) = p.instructions.last()
            && let Some(Instruction::DiscardCards { recipient, .. }) = instructions.last_mut()
        {
            *recipient = Some(ActionRecipient::Controller);
        }
        p.instructions == instructions
            && self
                .modes()
                .iter()
                .zip(&p.modes)
                .all(|(shape, mode)| shape.matches(&mode.program))
    }
}

pub(super) fn validate_definition(d: &Definition) -> Result<(), String> {
    if d.activated.iter().any(|a| has_x_instruction(&a.program))
        || d.triggered.iter().any(|a| has_x_instruction(&a.program))
    {
        return Err("announced X effects require a spell source".into());
    }
    let Some(p) = d.spell.as_ref().filter(|p| has_x_instruction(p)) else {
        return Ok(());
    };
    let c = &d.characteristics;
    if !d.x_mana_cost
        || !matches!(c.kind, CardKind::Instant | CardKind::Sorcery)
        || !c.additional_types.is_empty()
        || !d.keywords.is_empty()
        || !d.static_abilities.is_empty()
        || !d.mana_abilities.is_empty()
        || !d.activated.is_empty()
        || !d.triggered.is_empty()
        || !Shape::all().any(|shape| {
            (shape != Shape::DrawOrDamage || c.kind == CardKind::Sorcery)
                && (c.subtypes.is_empty()
                    || (shape.allows_arcane(c.kind) && c.subtypes == ["Arcane"]))
                && shape.matches(p)
        })
    {
        return Err("unsupported announced X spell definition".into());
    }
    Ok(())
}

#[cfg(test)]
pub(super) fn witness() -> Definition {
    let mut d = targeted_counted_draw::witness();
    d.x_mana_cost = true;
    d.spell.as_mut().unwrap().instructions = Shape::TargetDrawLose.instructions();
    d.capabilities = d.requirements();
    d
}

#[cfg(test)]
pub(super) fn effects_witness() -> Definition {
    let mut d = witness();
    d.characteristics.kind = CardKind::Sorcery;
    d.spell = Some(Shape::DrawOrDamage.program(SpellTiming::MainPhase));
    d.capabilities = d.requirements();
    d
}

#[test]
fn announced_x_draw_requires_exact_binding_and_sequence() {
    let base = witness();
    let mut malformed = serde_json::to_value(Shape::Draw.instructions().remove(0)).unwrap();
    malformed["recipient"] = serde_json::json!({"controller":null});
    assert!(serde_json::from_value::<Instruction>(malformed).is_err());
    let mut wrong_target = base.clone();
    wrong_target.spell.as_mut().unwrap().targets[0]
        .selection_id
        .clear();
    assert!(wrong_target.validate().is_err());
    let mut wrong_cap = base.clone();
    wrong_cap.capabilities.remove(&Capability::LifeLossV1);
    assert!(wrong_cap.validate().is_err());
    for shape in Shape::DRAW {
        let mut d = base.clone();
        let p = d.spell.as_mut().unwrap();
        if shape.domain().is_none() {
            p.targets.clear();
        }
        p.instructions = shape.instructions();
        d.capabilities = d.requirements();
        assert!(d.validate().is_ok(), "{:?}", d.validate());
        assert!(!d.capabilities.contains(&Capability::BoardWipesV1));
        assert!(!d.capabilities.contains(&Capability::AnnouncedXEffectsV1));
        d.x_mana_cost = false;
        assert!(d.validate().is_err());
        d.x_mana_cost = true;
        d.spell
            .as_mut()
            .unwrap()
            .instructions
            .push(Instruction::OptionalDrawCards { source_block: 0 });
        d.capabilities = d.requirements();
        assert!(d.validate().is_err());
    }
}

#[test]
fn announced_x_effects() {
    use Capability::*;
    let mut d = effects_witness();
    assert_eq!(
        d.capabilities,
        BTreeSet::from([
            NormalLayoutV1,
            CastSorceryV1,
            ModalChoiceV1,
            AnyDamageTargetV1,
            NoncombatDamageV1,
            AnnouncedXDrawV1,
            AnnouncedXEffectsV1,
        ])
    );
    d.validate().unwrap();
    d.x_mana_cost = false;
    assert!(d.validate().is_err());
    d.x_mana_cost = true;
    d.spell.as_mut().unwrap().modes.swap(0, 1);
    assert!(d.validate().is_err());
    for shape in Shape::EFFECTS {
        d.spell = Some(shape.program(SpellTiming::MainPhase));
        d.characteristics.subtypes = if shape.allows_arcane(CardKind::Sorcery) {
            vec!["Arcane".into()]
        } else {
            vec![]
        };
        d.capabilities = d.requirements();
        d.validate().unwrap();
        assert!(d.capabilities.remove(&AnnouncedXEffectsV1));
        assert!(d.validate().is_err());
    }
    for (shape, index, replacement) in [
        (
            Shape::DamageDrawDiscard,
            1,
            Instruction::draw(PlayerExpr::you(Context::Spell), 2.into(), 0),
        ),
        (
            Shape::MillGain,
            1,
            Instruction::announced_x_life(kir::PlayerExpr::target(0), true, 0),
        ),
        (
            Shape::DamageGain(TargetDomain::BattlefieldCreature),
            0,
            Instruction::DealDamageAnnouncedX {
                target_slot: 0,
                source_block: 1,
            },
        ),
    ] {
        let mut p = shape.program(SpellTiming::MainPhase);
        p.instructions[index] = replacement;
        d.spell = Some(p);
        d.capabilities = d.requirements();
        assert!(d.validate().is_err());
    }
}
