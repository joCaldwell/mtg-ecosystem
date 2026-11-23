//! General ability composition matrix.
//!
//! A definition marked `composed` is admitted by the tables in this module
//! instead of by a whole-card family. Each table answers one independent
//! question: may this instruction resolve in this kind of stack object, may
//! this cost pay for this program, may this event start this program, may this
//! static or keyword stand beside unrelated abilities. A row is the complete
//! admission decision for its primitive; no row inspects sibling abilities,
//! printed text, card names or profiles.
//!
//! CR 113.1-113.3: a card's abilities are independent objects of four kinds.
//! CR 602.1-602.2: an activated ability is its cost, targets and effect.
//! CR 603.1-603.3: a triggered ability is its event, condition and effect.
//! CR 608.2: a resolving spell or ability follows its instructions in order.
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;

pub use kir::Context;

const SPELL: u8 = 1;
const ACTIVATED: u8 = 2;
const TRIGGERED: u8 = 4;
/// Uses only the frozen controller, announced targets and its own operands.
const ANY: u8 = SPELL | ACTIVATED | TRIGGERED;
/// Reads the invocation's battlefield source (CR 113.7a, 400.7).
const SOURCE: u8 = ACTIVATED | TRIGGERED;
/// How the runtime announces a target, which decides the payment path of an
/// activated ability (CR 601.2c, 602.2b).
#[derive(Clone, Copy, PartialEq, Eq)]
enum Route {
    Object,
    Entity,
    Player,
    Stack,
}

/// The target row: each domain's announcement route and the contexts whose
/// announcement, resolution recheck and recovery all support it
/// (CR 115.1, 603.3d, 608.2b).
fn domain(domain: TargetDomain) -> Option<(Route, u8)> {
    Some(match domain {
        TargetDomain::BattlefieldPermanent | TargetDomain::BattlefieldCreature => {
            (Route::Object, ANY)
        }
        TargetDomain::AnyDamageTarget => (Route::Entity, ANY),
        TargetDomain::AnyPlayer | TargetDomain::OpponentPlayer => {
            (Route::Player, SPELL | TRIGGERED)
        }
        TargetDomain::OwnGraveyardCreatureCard | TargetDomain::OwnGraveyardCard => {
            (Route::Object, SPELL | TRIGGERED)
        }
        TargetDomain::StackSpell | TargetDomain::StackActivatedOrTriggeredAbility => {
            (Route::Stack, SPELL | ACTIVATED)
        }
        TargetDomain::PlayerOrPlaneswalker => (Route::Player, SPELL),
        TargetDomain::CreatureOrPlaneswalker
        | TargetDomain::PlayerGraveyardCards { .. }
        | TargetDomain::ControlledBattlefieldPermanent
        | TargetDomain::ControlledBattlefieldCreature
        | TargetDomain::BlockingBattlefieldCreature
        | TargetDomain::ControlledCreatureSpell => return None,
    })
}

/// One ordinary instruction program of any context.
fn program(
    p: &SpellProgram,
    context: Context,
    event: Option<&TriggerCondition>,
) -> Result<(), String> {
    if p.functioning_zone != FunctioningZone::Stack
        || p.resolution != SpellResolution::Instructions
        || p.instructions.is_empty()
        || !p.modes.is_empty()
        || p.additional_sacrifice.is_some()
        || !p.optional_additional_costs.is_empty()
        // CR 602.5d: only an activated ability may carry sorcery timing.
        || (context == Context::Triggered && p.timing != SpellTiming::Instant)
    {
        return Err("composed program requires ordinary stack instructions".into());
    }
    let bit = match context {
        Context::Spell => SPELL,
        Context::Activated => ACTIVATED,
        Context::Triggered => TRIGGERED,
    };
    if p.targets.iter().any(|t| {
        domain(t.domain).is_none_or(|(_, contexts)| contexts & bit == 0)
            || t.chooser != Chooser::Controller
            || t.stage != EvaluationStage::Announcement
    }) {
        return Err("composition matrix does not admit this target domain".into());
    }
    validate_target_bindings(p)?;
    let env = kir::Env {
        context,
        targets: &p.targets,
        event,
        announced_x: false,
    };
    let mut used = BTreeSet::new();
    for Instruction::Kernel { node, .. } in &p.instructions {
        used.extend(kir::check(node, &env)?.target_slots);
    }
    if used.len() != p.targets.len() {
        return Err("unused target occurrence".into());
    }
    Ok(())
}

fn validate_target_bindings(p: &SpellProgram) -> Result<(), String> {
    let mut ids = BTreeSet::new();
    for (index, target) in p.targets.iter().enumerate() {
        if usize::try_from(target.slot).ok() != Some(index)
            || target.selection_id.is_empty()
            || !ids.insert(&target.selection_id)
        {
            return Err("unbound, duplicate, or noncontiguous target occurrence".into());
        }
    }
    if p.targets
        .windows(2)
        .any(|targets| targets[0].domain != targets[1].domain)
    {
        return Err("mixed target domains are unsupported".into());
    }
    // Requests expose one common candidate set, and execution uses that common
    // predicate. Distinct per-slot filters need a different request contract;
    // accepting them here would apply the first slot's restriction to all slots.
    if p.targets
        .windows(2)
        .any(|targets| targets[0].filter != targets[1].filter)
    {
        return Err("mixed target filters are unsupported".into());
    }
    for t in &p.targets {
        if let Some(f) = &t.filter {
            if !matches!(
                t.domain,
                TargetDomain::BattlefieldPermanent
                    | TargetDomain::ControlledBattlefieldPermanent
                    | TargetDomain::StackSpell
            ) || (t.domain == TargetDomain::StackSpell
                && (f.types.is_empty()
                    || !f.non_types.is_empty()
                    || !f.non_colors.is_empty()
                    || f.opponent
                    || f.power.is_some()
                    || f.tapped))
                || f.types
                    .iter()
                    .enumerate()
                    .any(|(i, k)| f.types[..i].contains(k))
                || f.non_types
                    .iter()
                    .enumerate()
                    .any(|(i, k)| f.non_types[..i].contains(k))
                || f.non_colors.contains(&Color::C)
                || f.non_colors
                    .iter()
                    .enumerate()
                    .any(|(i, k)| f.non_colors[..i].contains(k))
                || (t.domain == TargetDomain::BattlefieldPermanent
                    && f.types
                        .iter()
                        .chain(&f.non_types)
                        .any(|k| matches!(k, CardKind::Instant | CardKind::Sorcery)))
            {
                return Err("unsupported permanent/spell target filter".into());
            }
        } else if matches!(
            t.domain,
            TargetDomain::BattlefieldPermanent | TargetDomain::ControlledBattlefieldPermanent
        ) {
            return Err("permanent target requires explicit filter".into());
        }
    }
    Ok(())
}

/// CR 701.21a: the runtime offers every controlled permanent whose derived
/// types include one of `types` and whose subtypes include every listed
/// subtype. The row admits one or more distinct permanent types, or exactly
/// one subtype.
fn sacrifice_filter(f: &SacrificeFilter) -> bool {
    let types_ok = f.types.iter().all(|k| {
        matches!(
            k,
            CardKind::Creature | CardKind::Artifact | CardKind::Enchantment | CardKind::Land
        )
    }) && f
        .types
        .iter()
        .enumerate()
        .all(|(n, k)| !f.types[..n].contains(k));
    let subtype_ok =
        |s: &String| !s.is_empty() && s.chars().all(|c| c.is_ascii_alphabetic() || c == '-');
    types_ok
        && match (f.types.is_empty(), f.subtypes.as_slice()) {
            (false, []) => true,
            (true, [subtype]) => subtype_ok(subtype),
            _ => false,
        }
}

/// The cost row: complete costs the runtime pays for the given program's
/// announcement route. CR 602.2b, 601.2f-h: costs are determined and paid
/// after targets, and an unpayable combination is never admitted.
fn cost(d: &Definition, a: &ActivatedAbility) -> Result<(), String> {
    let kind = d.characteristics.kind;
    let route = a
        .program
        .targets
        .first()
        .and_then(|t| domain(t.domain))
        .map(|(route, _)| route);
    let mana_ok = |m: &ManaCost| !m.generic.is_negative() && !m.colored.contains(&Color::C);
    let payable = match &a.cost {
        // CR 302.6: the targeted tap continuation applies the summoning
        // rule, which is correct only for a creature source; a stack target
        // would not tap the source at all.
        ActivationCost::TapSource => match route {
            None => true,
            Some(Route::Object | Route::Entity) => kind == CardKind::Creature,
            Some(Route::Player | Route::Stack) => false,
        },
        ActivationCost::Mana { cost } => {
            mana_ok(cost) && matches!(route, None | Some(Route::Object | Route::Stack))
        }
        // CR 701.21a: only the exact source permanent pays this cost.
        ActivationCost::SacrificeSource => {
            kind == CardKind::Creature && route != Some(Route::Player)
        }
        ActivationCost::SacrificePermanent {
            count,
            filter,
            mana,
        } => {
            *count == 1
                && sacrifice_filter(filter)
                && mana.as_ref().is_none_or(mana_ok)
                && match route {
                    None | Some(Route::Object | Route::Entity) => true,
                    Some(Route::Stack) => mana.is_some(),
                    Some(Route::Player) => false,
                }
        }
        ActivationCost::Composite { costs } => {
            costs.remove_source_plus_one_counters.is_none()
                && costs.mana.as_ref().is_none_or(mana_ok)
                && (costs.tap || costs.sacrifice)
                && match route {
                    None | Some(Route::Object | Route::Stack) => true,
                    // The entity announcement pays a combined cost only
                    // through its mana window.
                    Some(Route::Entity) => costs.mana.is_some(),
                    Some(Route::Player) => false,
                }
        }
        ActivationCost::DiscardSource { .. } | ActivationCost::TapPayLifeSacrificeSource { .. } => {
            false
        }
    };
    if !payable {
        return Err("composition matrix does not admit this activation cost".into());
    }
    Ok(())
}

fn activated(d: &Definition, a: &ActivatedAbility) -> Result<(), String> {
    if a.functioning_zone != FunctioningZone::Battlefield
        || !matches!(
            d.characteristics.kind,
            CardKind::Creature | CardKind::Artifact | CardKind::Enchantment | CardKind::Land
        )
    {
        return Err("composed activation requires a battlefield permanent source".into());
    }
    cost(d, a)?;
    program(&a.program, Context::Activated, None)
}

/// The event row: conditions whose detection needs no program-specific data.
fn triggered(d: &Definition, a: &TriggeredAbility) -> Result<(), String> {
    let kind = d.characteristics.kind;
    let creature = d.characteristics.has_type(CardKind::Creature);
    let event = match &a.condition {
        TriggerCondition::SelfEntersBattlefield => true,
        // CR 700.4: "dies" is defined for creatures; the runtime witness
        // reads the primary card type.
        TriggerCondition::SelfDies => kind == CardKind::Creature,
        TriggerCondition::StepBegan { .. } => true,
        TriggerCondition::SpellCast { spell, .. } => match spell {
            TypePredicate::SpellColor { color } => *color != Color::C,
            _ => spell.validate_type_shape(true).is_ok(),
        },
        TriggerCondition::CreatureAttacked { attacker } => match attacker {
            AttackSubject::Source {} => creature,
            AttackSubject::Filter { types, .. } => types.validate().is_ok(),
        },
        TriggerCondition::OtherCreatureEntersBattlefield
        | TriggerCondition::AnyCreatureDies
        | TriggerCondition::OpponentCreatureDies
        | TriggerCondition::ControlledLandEntersBattlefield
        // CR 701.20a: the exact source's own untapped-to-tapped change.
        | TriggerCondition::SelfBecomesTapped => true,
        // CR 510.2, 120.3: damage this source or a filtered creature deals to
        // a player; event-bound recipients and quantities stay with families.
        TriggerCondition::DamageDealt { source, .. } => match source {
            DamageTriggerSource::Source {} => creature,
            DamageTriggerSource::CreatureFilter { types, .. } => types.validate().is_ok(),
        },
        TriggerCondition::DelayedControllerCreatureEntry
        | TriggerCondition::DelayedControllerSpellCast
        | TriggerCondition::ManaTapped { .. }
        | TriggerCondition::SagaChapter { .. }
        | TriggerCondition::SelfSpellCast
        | TriggerCondition::GroupedDamageDealt { .. }
        | TriggerCondition::SpellCastWithOrigin { .. }
        | TriggerCondition::EquippedCombatDamage
        | TriggerCondition::EquippedDies
        | TriggerCondition::ControlledCreatureEntersWithPower { .. }
        | TriggerCondition::SelfAuraToGraveyard
        | TriggerCondition::SelfMoved { .. } => false,
    };
    if !event
        || a.functioning_zone != FunctioningZone::Battlefield
        || a.intervening_if.is_some()
        || !matches!(
            kind,
            CardKind::Creature | CardKind::Artifact | CardKind::Enchantment | CardKind::Land
        )
        || a.death_filter.as_ref().is_some_and(|f| {
            a.condition != TriggerCondition::AnyCreatureDies
                || !f.controlled
                || f.non_subtypes.iter().any(|s| s != "Spirit")
                || f.non_subtypes.len() > 1
        })
    {
        return Err("composition matrix does not admit this trigger".into());
    }
    program(&a.program, Context::Triggered, Some(&a.condition))
}

/// The static row: effects that are complete without any sibling ability.
fn static_ability(d: &Definition, a: &StaticAbility) -> Result<(), String> {
    let kind = d.characteristics.kind;
    let permanent = !matches!(kind, CardKind::Instant | CardKind::Sorcery);
    let battlefield = a.functioning_zone == FunctioningZone::Battlefield;
    let ok = match &a.effect {
        // CR 614.12: the source's own entry replacement.
        StaticEffect::EntersTapped => permanent && battlefield,
        // CR 101.2, 113.6: functions on the stack for this spell only.
        StaticEffect::CantBeCountered => !permanent && a.functioning_zone == FunctioningZone::Stack,
        // CR 611.3a-c, 613.4c: live controller-relative creature modifiers.
        StaticEffect::ModifyCreatures { scope, .. } => {
            battlefield
                && matches!(scope, CreatureScope::Source | CreatureScope::Controller)
                && static_creature_modification_valid(kind, false, false, &a.effect)
        }
        StaticEffect::ControllerCreatureAnthem { power, toughness } => {
            battlefield
                && matches!(kind, CardKind::Creature | CardKind::Enchantment)
                && *power > Quantity::zero()
                && power == toughness
        }
        _ => false,
    };
    if !ok {
        return Err("composition matrix does not admit this static ability".into());
    }
    Ok(())
}

/// The mana row: tap-only fixed, any-color or commander-color production.
/// CR 605.1a, 605.3: mana abilities do not use the stack.
fn mana_ability(d: &Definition, a: &ManaAbility) -> Result<(), String> {
    if !matches!(
        d.characteristics.kind,
        CardKind::Creature | CardKind::Artifact | CardKind::Land
    ) || !a.costs.tap
        || a.costs.sacrifice
        || a.costs.mana.is_some()
        || a.costs.remove_source_plus_one_counters.is_some()
        || a.restriction.is_some()
        || a.self_damage.is_some()
        || !match &a.production {
            ManaProduction::Fixed { symbols } => !symbols.is_empty(),
            ManaProduction::AnyColor | ManaProduction::CommanderColor => true,
            _ => false,
        }
    {
        return Err("composition matrix does not admit this mana ability".into());
    }
    Ok(())
}

/// The keyword row (CR 702): evergreen keywords with no parameters.
fn keyword(d: &Definition, k: KeywordAbility) -> bool {
    use KeywordAbility as K;
    match d.characteristics.kind {
        CardKind::Creature => matches!(
            k,
            K::Flash
                | K::Defender
                | K::Trample
                | K::FirstStrike
                | K::DoubleStrike
                | K::Menace
                | K::Hexproof
                | K::Shroud
                | K::Flying
                | K::Reach
                | K::Vigilance
                | K::Haste
                | K::Lifelink
                | K::Deathtouch
                | K::Indestructible
        ),
        CardKind::Artifact | CardKind::Enchantment => matches!(k, K::Flash | K::Indestructible),
        CardKind::Land => k == K::Indestructible,
        CardKind::Instant | CardKind::Sorcery => false,
    }
}

/// Complete admission of a composed definition. Nothing here defers to a
/// whole-card family, so a rejected row can never be reached another way.
pub(crate) fn validate(d: &Definition) -> Result<(), String> {
    let c = &d.characteristics;
    if d.oracle_id.is_empty()
        || d.name.is_empty()
        || c.mana_cost.generic.is_negative()
        || c.colors.contains(&Color::C)
        || c.color_identity.contains(&Color::C)
        || d.capabilities != d.requirements()
    {
        return Err("invalid identity, characteristics, or capability closure".into());
    }
    if !accepted_provenance(
        &d.provenance.archive_manifest_sha256,
        &d.provenance.parser_sha256,
    ) || sha256(d.provenance.source_json.as_bytes()) != d.provenance.source_sha256
        || sha256(format!("[{}]", d.provenance.abilities_json).as_bytes())
            != d.provenance.ast_sha256
    {
        return Err("invalid source/AST provenance".into());
    }
    if let Some(labels) = &d.source_labels {
        let source: serde_json::Value = strict_read(&d.provenance.source_json)?;
        if source["oracle_text"].as_str() != Some(labels.text.as_str())
            || labels.blocks.iter().any(|s| s.is_empty())
            || labels
                .activated
                .iter()
                .chain(&labels.triggered)
                .chain(&labels.mana)
                .chain(&labels.statics)
                .any(|b| *b as usize >= labels.blocks.len())
            || labels.activated.len() != d.activated.len()
            || labels.triggered.len() != d.triggered.len()
            || labels.mana.len() != d.mana_abilities.len()
            || labels.statics.len() != d.static_abilities.len()
            || !labels.spell_modes.is_empty()
        {
            return Err("invalid source label provenance".into());
        }
    }
    // Whole-card characteristics the composition tables do not describe
    // reject: second faces, hybrid and X costs, snow, attachments and Sagas.
    // CR 205.4: the basic supertype is admitted only on a land, whose shell
    // below then requires the land's intrinsic mana (CR 305.6).
    if d.modal_land.is_some()
        || d.transform_faces.is_some()
        || !c.hybrid_mana.is_empty()
        || d.x_mana_cost
        || c.snow
        || (c.basic && c.kind != CardKind::Land)
        || c.subtypes.iter().any(|s| {
            unsupported_rules_subtype(s)
                || matches!(s.as_str(), "Aura" | "Equipment")
                || (c.kind != CardKind::Land && basic_land_color(s).is_some())
        })
        || c.subtypes.iter().collect::<BTreeSet<_>>().len() != c.subtypes.len()
        || c.additional_types
            .iter()
            .any(|k| !matches!(k, CardKind::Artifact | CardKind::Enchantment) || *k == c.kind)
        || c.additional_types.windows(2).any(|w| w[0] >= w[1])
        || (!c.additional_types.is_empty()
            && !matches!(c.kind, CardKind::Creature | CardKind::Artifact))
    {
        return Err("composition matrix does not admit these card characteristics".into());
    }
    if d.keywords.iter().any(|k| !keyword(d, *k))
        || d.keywords
            .iter()
            .enumerate()
            .any(|(n, k)| d.keywords[..n].contains(k))
    {
        return Err("composition matrix does not admit this keyword".into());
    }
    let cast_timing = if c.kind == CardKind::Instant || d.keywords.contains(&KeywordAbility::Flash)
    {
        SpellTiming::Instant
    } else {
        SpellTiming::MainPhase
    };
    let permanent_spell = |p: &SpellProgram| {
        p.functioning_zone == FunctioningZone::Stack
            && p.timing == cast_timing
            && p.resolution == SpellResolution::Permanent
            && p.targets.is_empty()
            && p.instructions.is_empty()
            && p.modes.is_empty()
            && p.additional_sacrifice.is_none()
            && p.optional_additional_costs.is_empty()
    };
    let shell = match c.kind {
        // CR 302, 208: a creature card has printed power and toughness.
        CardKind::Creature => {
            c.power.is_some()
                && c.toughness.as_ref().is_some_and(|n| *n > Quantity::zero())
                && d.intrinsic_zone.is_none()
                && d.spell.as_ref().is_some_and(permanent_spell)
        }
        CardKind::Artifact | CardKind::Enchantment => {
            c.power.is_none()
                && c.toughness.is_none()
                && d.intrinsic_zone.is_none()
                && d.spell.as_ref().is_some_and(permanent_spell)
        }
        // CR 305.1, 305.9: lands are played, never cast, and have no mana cost.
        // CR 305.6: a land with basic land types has their intrinsic mana
        // abilities, so its subtypes are either all basic land types with no
        // printed mana ability, or all descriptive.
        CardKind::Land => {
            let typed = c
                .subtypes
                .iter()
                .filter(|s| basic_land_color(s).is_some())
                .count();
            (typed == 0 || (typed == c.subtypes.len() && d.mana_abilities.is_empty()))
                && c.additional_types.is_empty()
                && c.mana_cost.generic == Quantity::zero()
                && c.mana_cost.colored.is_empty()
                && c.power.is_none()
                && c.toughness.is_none()
                && d.spell.is_none()
                && d.intrinsic_zone == Some(FunctioningZone::Battlefield)
        }
        CardKind::Instant | CardKind::Sorcery => {
            !c.legendary
                && c.power.is_none()
                && c.toughness.is_none()
                && d.intrinsic_zone.is_none()
                && d.activated.is_empty()
                && d.triggered.is_empty()
                && d.mana_abilities.is_empty()
                && d.spell.as_ref().is_some_and(|p| p.timing == cast_timing)
        }
    };
    if !shell {
        return Err("composition matrix does not admit this card shell".into());
    }
    if matches!(c.kind, CardKind::Instant | CardKind::Sorcery) {
        program(
            d.spell.as_ref().ok_or("missing spell program")?,
            Context::Spell,
            None,
        )?;
    }
    for a in &d.static_abilities {
        static_ability(d, a)?;
    }
    for a in &d.mana_abilities {
        mana_ability(d, a)?;
    }
    u32::try_from(d.activated.len()).map_err(|_| "activated ability index limit")?;
    u32::try_from(d.triggered.len()).map_err(|_| "trigger ability index limit")?;
    for a in &d.activated {
        activated(d, a)?;
    }
    for a in &d.triggered {
        triggered(d, a)?;
    }
    Ok(())
}
