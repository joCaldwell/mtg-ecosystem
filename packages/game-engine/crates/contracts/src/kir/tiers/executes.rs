//! The `Executes` tier of the surfaces outside kernel cells: card shells,
//! keywords, target announcement, activation costs, trigger events, statics
//! and mana abilities. A surface executes when the runtime has its executor in
//! this context; one that does not is rejected under `cell:<surface>:<detail>`.
//! A row is the complete decision for its surface; no row inspects sibling
//! abilities, printed text, card names or profiles. Kernel cells are decided
//! by [`kir::check`].
//!
//! CR 113.1-113.3: a card's abilities are independent objects of four kinds.
//! CR 602.1-602.2: an activated ability is its cost, targets and effect.
//! CR 603.1-603.3: a triggered ability is its event, condition and effect.
//! CR 608.2: a resolving spell or ability follows its instructions in order.
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use crate::*;

/// The snake-case name of a variant, for a cell code.
fn name<T: std::fmt::Debug>(value: &T) -> String {
    let debug = format!("{value:?}");
    let mut out = String::new();
    for (i, c) in debug
        .chars()
        .take_while(|c| c.is_alphanumeric())
        .enumerate()
    {
        if c.is_uppercase() && i > 0 {
            out.push('_');
        }
        out.push(c.to_ascii_lowercase());
    }
    out
}

/// A surface with no executor here: `cell:<surface>:<detail>`.
fn cell<T>(surface: &str, detail: impl AsRef<str>) -> Result<T, String> {
    Err(format!("cell:{surface}:{}", detail.as_ref()))
}

const SPELL: u8 = 1;
const ACTIVATED: u8 = 2;
const TRIGGERED: u8 = 4;
/// Uses only the frozen controller, announced targets and its own operands.
const ANY: u8 = SPELL | ACTIVATED | TRIGGERED;
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
        TargetDomain::ControlledBattlefieldCreature => (Route::Object, ACTIVATED),
        TargetDomain::CreatureOrPlaneswalker
        | TargetDomain::PlayerGraveyardCards { .. }
        | TargetDomain::ControlledBattlefieldPermanent
        | TargetDomain::BlockingBattlefieldCreature
        | TargetDomain::ControlledCreatureSpell => return None,
    })
}

/// CR 601.2b, 601.2f-h: a spell's own additional costs: one chosen
/// sacrifice of a reviewed quality, and optional fixed mana costs with
/// distinct identities (CR 702.33a). No ability declares either.
fn spell_costs(p: &SpellProgram, context: Context) -> bool {
    let mana_ok = |m: &ManaCost| !m.generic.is_negative() && !m.colored.contains(&Color::C);
    (p.additional_sacrifice.is_none() && p.optional_additional_costs.is_empty())
        || (context == Context::Spell
            && p.additional_sacrifice.as_ref().is_none_or(sacrifice_filter)
            && p.optional_additional_costs.iter().enumerate().all(|(n, c)| {
                !c.identity.is_empty()
                    && mana_ok(&c.mana)
                    && (c.mana.generic > Quantity::zero() || !c.mana.colored.is_empty())
                    && !p.optional_additional_costs[..n]
                        .iter()
                        .any(|o| o.identity == c.identity)
            }))
}

/// The optional-cost identities a node's conditions read.
fn cost_conditions<'a>(node: &'a kir::Node, out: &mut Vec<&'a String>) {
    fn cond<'a>(c: &'a kir::Cond, out: &mut Vec<&'a String>) {
        match c {
            kir::Cond::CostPaid { identity } => out.push(identity),
            kir::Cond::Not { condition } => cond(condition, out),
            kir::Cond::All { conditions } => conditions.iter().for_each(|c| cond(c, out)),
            kir::Cond::Compare { .. } | kir::Cond::Exists { .. } | kir::Cond::ActivePlayer { .. } => {}
        }
    }
    match node {
        kir::Node::Do(_) => {}
        kir::Node::Seq(nodes) => nodes.iter().for_each(|n| cost_conditions(n, out)),
        kir::Node::If {
            condition,
            then,
            otherwise,
        } => {
            cond(condition, out);
            cost_conditions(then, out);
            if let Some(otherwise) = otherwise {
                cost_conditions(otherwise, out);
            }
        }
        kir::Node::Optional { node, .. }
        | kir::Node::UnlessPays { node, .. }
        | kir::Node::IfPays { node, .. }
        | kir::Node::Delayed { node, .. } => cost_conditions(node, out),
        kir::Node::ForEach { body, .. } | kir::Node::Choose { body, .. } => {
            cost_conditions(body, out)
        }
        kir::Node::Reflexive { first, then } => {
            cost_conditions(first, out);
            cost_conditions(then, out);
        }
    }
}

/// One ordinary instruction program of any context.
fn program(
    p: &SpellProgram,
    context: Context,
    event: Option<&TriggerCondition>,
    announced_x: bool,
) -> Result<(), String> {
    if p.functioning_zone != FunctioningZone::Stack
        || p.resolution != SpellResolution::Instructions
        || p.instructions.is_empty()
        || !spell_costs(p, context)
        // CR 602.5d: only an activated ability may carry sorcery timing.
        || (context == Context::Triggered && p.timing != SpellTiming::Instant)
    {
        return cell("program", "extra_costs_or_resolution");
    }
    // CR 702.33d: a "kicked" condition reads a cost this spell declares.
    let mut paid = vec![];
    for Instruction::Kernel { node, .. } in &p.instructions {
        cost_conditions(node, &mut paid);
    }
    if paid
        .iter()
        .any(|id| !p.optional_additional_costs.iter().any(|c| c.identity == **id))
    {
        return cell("program", "undeclared_cost_condition");
    }
    let bit = match context {
        Context::Spell => SPELL,
        Context::Activated => ACTIVATED,
        Context::Triggered => TRIGGERED,
    };
    if let Some(t) = p.targets.iter().find(|t| {
        domain(t.domain).is_none_or(|(_, contexts)| contexts & bit == 0)
            || t.chooser != Chooser::Controller
            || t.stage != EvaluationStage::Announcement
    }) {
        return cell("target", format!("{}:{}", name(&t.domain), name(&context)));
    }
    validate_target_bindings(p)?;
    let env = kir::Env {
        context,
        targets: &p.targets,
        event,
        announced_x,
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
            // CR 110.1: "a permanent" names any controlled permanent.
            (true, []) | (false, []) => true,
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
        // CR 602.2b, 701.21a: tapping the source and "another" are part of
        // the one chosen sacrifice cost.
        ActivationCost::SacrificePermanent {
            count,
            filter,
            mana,
            tap: _,
            another: _,
        } => {
            *count >= 1
                && sacrifice_filter(filter)
                && mana.as_ref().is_none_or(mana_ok)
                && match route {
                    None | Some(Route::Object | Route::Entity) => true,
                    Some(Route::Stack) => mana.is_some(),
                    Some(Route::Player) => false,
                }
        }
        // CR 118.3, 119.4, 122.1: life and +1/+1 counter removal from the
        // source are paid with the other source terms after any mana.
        ActivationCost::Composite { costs } => {
            costs
                .remove_source_plus_one_counters
                .as_ref()
                .is_none_or(|n| *n > Quantity::zero())
                && costs.pay_life.as_ref().is_none_or(|n| *n > Quantity::zero())
                && costs.mana.as_ref().is_none_or(mana_ok)
                && (costs.tap
                    || costs.sacrifice
                    || costs.pay_life.is_some()
                    || costs.remove_source_plus_one_counters.is_some())
                && match route {
                    None | Some(Route::Object | Route::Stack) => true,
                    // The entity announcement pays a combined cost only
                    // through its mana window.
                    Some(Route::Entity) => costs.mana.is_some(),
                    Some(Route::Player) => false,
                }
        }
        // CR 118.3, 119.4, 701.21a: "{T}, Pay N life, Sacrifice [this]" is
        // paid as one ordered batch at activation; with no announcement the
        // runtime validates every term before paying any.
        ActivationCost::TapPayLifeSacrificeSource { life } => {
            route.is_none() && !life.is_negative()
        }
        ActivationCost::DiscardSource { .. } => false,
    };
    if !payable {
        let route = match route {
            None => "untargeted",
            Some(Route::Object) => "object",
            Some(Route::Entity) => "entity",
            Some(Route::Player) => "player",
            Some(Route::Stack) => "stack",
        };
        return cell("cost", format!("{}:{route}", name(&a.cost)));
    }
    Ok(())
}

fn activated(d: &Definition, a: &ActivatedAbility) -> Result<(), String> {
    // CR 702.29a, 602.2a: cycling is the one hand activation, on any card
    // type: its fixed mana and this exact hand card, then one draw.
    if a.functioning_zone == FunctioningZone::Hand {
        if !hand_discard_draw_activation(a) {
            return cell("activated", "hand");
        }
        return program(&a.program, Context::Activated, None, false);
    }
    if a.functioning_zone != FunctioningZone::Battlefield
        || !matches!(
            d.characteristics.kind,
            CardKind::Creature | CardKind::Artifact | CardKind::Enchantment | CardKind::Land
        )
    {
        return cell("activated", "source");
    }
    if a.program
        .targets
        .iter()
        .any(|t| t.domain == TargetDomain::ControlledBattlefieldCreature)
        && !(d.characteristics.subtypes == ["Equipment"]
            && matches!(a.cost, ActivationCost::Mana { .. })
            && a.program.timing == SpellTiming::MainPhase
            && matches!(a.program.targets.as_slice(), [t] if t.filter.is_none())
            && matches!(
                a.program.instructions.as_slice(),
                [Instruction::Kernel {
                    node: kir::Node::Do(kir::Verb::Attach {
                        what: kir::Ref::Source,
                        to: kir::Ref::Target { slot: 0 }
                    }),
                    ..
                }]
            ))
    {
        return cell("activated", "equip");
    }
    cost(d, a)?;
    program(&a.program, Context::Activated, None, false)
}

/// CR 303.4a: the Aura's one announced target encodes the enchant predicate.
fn enchant_target(targets: &[TargetSpec]) -> bool {
    let [t] = targets else { return false };
    if t.slot != 0
        || t.selection_id.is_empty()
        || t.stage != EvaluationStage::Announcement
        || t.chooser != Chooser::Controller
    {
        return false;
    }
    match t.domain {
        TargetDomain::BattlefieldCreature => t.filter.is_none(),
        TargetDomain::BattlefieldPermanent => t.filter.as_ref().is_some_and(|f| {
            matches!(
                f.types.as_slice(),
                [] | [CardKind::Artifact | CardKind::Land]
            ) && f.non_types.is_empty()
                && f.non_colors.is_empty()
                && !f.opponent
                && !f.tapped
                && f.power.is_none()
        }),
        _ => false,
    }
}

/// The event row: conditions whose detection needs no program-specific data.
fn triggered(d: &Definition, a: &TriggeredAbility) -> Result<(), String> {
    let kind = d.characteristics.kind;
    let creature = d.characteristics.has_type(CardKind::Creature);
    let event = match &a.condition {
        TriggerCondition::AttachedCreature { attachment, .. } => match attachment {
            kir::AttachmentKind::Equipped => d.characteristics.subtypes == ["Equipment"],
            kir::AttachmentKind::Enchanted => d.characteristics.subtypes == ["Aura"],
        },
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
    };
    let key = a.condition.reporting_key();
    if !event {
        return cell("trigger", key);
    }
    if a.intervening_if.is_some() {
        return cell("trigger", format!("{key}:intervening_if"));
    }
    if a.functioning_zone != FunctioningZone::Battlefield
        || !matches!(
            kind,
            CardKind::Creature | CardKind::Artifact | CardKind::Enchantment | CardKind::Land
        )
    {
        return cell("trigger", format!("{key}:source"));
    }
    if a.death_filter.as_ref().is_some_and(|f| {
        a.condition != TriggerCondition::AnyCreatureDies
            || !f.controlled
            || f.non_subtypes.iter().any(|s| s != "Spirit")
            || f.non_subtypes.len() > 1
    }) {
        return cell("trigger", format!("{key}:death_filter"));
    }
    program(&a.program, Context::Triggered, Some(&a.condition), false)
}

/// The mana row: a tap ability of a creature, artifact or land that adds
/// fixed, chosen, any-color, commander-color or "could produce" mana, may
/// also cost mana (CR 605.3a, 601.2g-h) or life (CR 119.4), and may perform
/// admitted riders after adding mana ([`kir::mana_rider`]).
/// CR 605.1a, 605.3: mana abilities do not use the stack.
fn mana_ability(d: &Definition, a: &ManaAbility) -> Result<(), String> {
    let distinct = |colors: &[Color]| {
        colors
            .iter()
            .enumerate()
            .all(|(i, c)| !colors[..i].contains(c))
    };
    if !matches!(
        d.characteristics.kind,
        CardKind::Creature | CardKind::Artifact | CardKind::Land
    ) || !a.costs.tap
        || a.costs.sacrifice
        || a.costs
            .mana
            .as_ref()
            .is_some_and(|m| m.generic.is_negative() || m.colored.contains(&Color::C))
        || a.costs
            .pay_life
            .as_ref()
            .is_some_and(|life| *life < Quantity::from(1))
        || a.costs.remove_source_plus_one_counters.is_some()
        || a.restriction.is_some()
        || !match &a.production {
            ManaProduction::Fixed { symbols } => !symbols.is_empty(),
            ManaProduction::AnyColor
            | ManaProduction::CommanderColor
            | ManaProduction::LandsCouldProduce { .. } => true,
            ManaProduction::Choose { colors, amount } => {
                colors.len() >= 2 && distinct(colors) && *amount >= Quantity::from(1)
            }
        }
    {
        return cell("mana", name(&a.production));
    }
    if let Some(error) = a.riders.iter().find_map(|r| kir::mana_rider(r).err()) {
        return cell("mana", format!("rider:{error}"));
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
            || d.spell
                .iter()
                .chain(d.activated.iter().map(|a| &a.program))
                .chain(d.triggered.iter().map(|a| &a.program))
                .flat_map(|program| &program.instructions)
                .any(|instruction| instruction.source_block() as usize >= labels.blocks.len())
            || !labels.spell_modes.is_empty()
        {
            return Err("invalid source label provenance".into());
        }
    }
    surfaces(d)
}

/// The surface rows alone: card shell, keywords, statics, mana abilities,
/// activated and triggered abilities and the spell program. The public tier
/// queries run these, so a definition whose surface no runtime executes
/// earns no tier, however its capabilities and cells are reviewed.
pub(crate) fn surfaces(d: &Definition) -> Result<(), String> {
    let c = &d.characteristics;
    // Whole-card characteristics the composition tables do not describe
    // reject: second faces, snow and Sagas. A spell's X and hybrid or
    // Phyrexian symbols are announced while casting (CR 107.3, 107.4e-f,
    // 601.2b/f); a land has no mana cost to announce (CR 305.1).
    // CR 205.4: the basic supertype is admitted only on a land, whose shell
    // below then requires the land's intrinsic mana (CR 305.6).
    if ((!c.hybrid_mana.is_empty() || d.x_mana_cost) && d.spell.is_none())
        || c.snow
        || (c.basic && c.kind != CardKind::Land)
        || c.subtypes.iter().any(|s| {
            unsupported_rules_subtype(s)
                || (s == "Aura" && (c.kind != CardKind::Enchantment || c.subtypes != ["Aura"]))
                || (s == "Equipment"
                    && (c.kind != CardKind::Artifact || c.subtypes != ["Equipment"]))
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
        return cell("shell", "characteristics");
    }
    if let Some(k) = d.keywords.iter().find(|k| !keyword(d, **k)) {
        return cell("keyword", format!("{}:{}", name(k), name(&c.kind)));
    }
    if d.keywords
        .iter()
        .enumerate()
        .any(|(n, k)| d.keywords[..n].contains(k))
    {
        return cell("keyword", "duplicate");
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
            && (if c.subtypes == ["Aura"] {
                enchant_target(&p.targets)
            } else {
                p.targets.is_empty()
            })
            && p.instructions.is_empty()
            // CR 601.2b: a permanent spell may require one sacrifice.
            && p.additional_sacrifice.as_ref().is_none_or(sacrifice_filter)
            && p.optional_additional_costs.is_empty()
    };
    let shell = match c.kind {
        // CR 302, 208: a creature card has printed power and toughness.
        CardKind::Creature => {
            c.power.is_some()
                && c.toughness.as_ref().is_some_and(|n| {
                    if *n > Quantity::zero() {
                        return true;
                    }
                    // CR 122.6 and 704.5f: entry counters contribute before
                    // the first state-based toughness check.
                    let mut entry_toughness = n.clone();
                    for ability in &d.static_abilities {
                        if let kir::StaticEffect::EntersWithPlusOneCounters { count } =
                            &ability.effect
                        {
                            entry_toughness.add(count);
                        }
                    }
                    entry_toughness > Quantity::zero()
                })
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
                && d.activated
                    .iter()
                    .all(|a| a.functioning_zone == FunctioningZone::Hand)
                && d.triggered.is_empty()
                && d.mana_abilities.is_empty()
                && d.spell.as_ref().is_some_and(|p| p.timing == cast_timing)
        }
    };
    if !shell {
        return cell("shell", name(&c.kind));
    }
    if matches!(c.kind, CardKind::Instant | CardKind::Sorcery) {
        program(
            d.spell.as_ref().ok_or("missing spell program")?,
            Context::Spell,
            None,
            // CR 107.3a: X in a spell's text is the value announced for its
            // own cost; an ability's X is not announced (CR 107.3).
            d.x_mana_cost,
        )?;
    }
    if d.static_abilities
        .iter()
        .filter(|a| matches!(a.effect, kir::StaticEffect::PayLifeOrEnterTapped { .. }))
        .count()
        > 1
    {
        return cell("static", "multiple_entry_life_payments");
    }
    for a in &d.static_abilities {
        a.validate_static(d)?;
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::kir::Context;
    fn bears() -> Definition {
        let mut d = Catalog::read(include_str!(
            "../../../../../tests/fixtures/expected-ir.json"
        ))
        .unwrap()
        .definitions
        .into_iter()
        .find(|d| d.name == "Grizzly Bears")
        .unwrap();
        d.source_labels = None;
        d
    }
    fn ability(cost: ActivationCost, program: SpellProgram) -> ActivatedAbility {
        ActivatedAbility {
            functioning_zone: FunctioningZone::Battlefield,
            cost,
            program,
        }
    }
    fn with(change: impl FnOnce(&mut Definition)) -> Definition {
        let mut d = bears();
        d.composed = true;
        change(&mut d);
        d.capabilities = d.requirements();
        d
    }
    fn target(domain: TargetDomain) -> TargetSpec {
        TargetSpec {
            slot: 0,
            selection_id: "line:0:1".into(),
            chooser: Chooser::Controller,
            stage: EvaluationStage::Announcement,
            domain,
            filter: None,
        }
    }
    fn damage() -> Instruction {
        Instruction::damage_target(0, kir::Amount::Fixed { value: 1.into() }, 0)
    }

    /// CR 118.3, 119.4: a life payment is a mana-ability cost only. A
    /// deserialized stack activation whose composite cost pays life is not
    /// admitted at any tier, so it can never keep the reviewed composite
    /// cost's Hosting tier.
    #[test]
    fn composite_activation_life_payment_executes_only() {
        let tap = |pay_life: Option<Quantity>| {
            with(|d| {
                d.activated.push(ability(
                    ActivationCost::Composite {
                        costs: PermanentActivationCosts {
                            remove_source_plus_one_counters: None,
                            tap: true,
                            mana: Some(ManaCost {
                                generic: 1.into(),
                                colored: vec![],
                            }),
                            sacrifice: true,
                            pay_life,
                        },
                    },
                    program(vec![gain(Context::Activated)], vec![]),
                ))
            })
        };
        tap(None).validate().unwrap();
        assert!(
            kir::definition_tiers(&tap(None))
                .unwrap()
                .admits(kir::ExecutionTier::Hosting)
        );
        // CR 119.4: a stack activation's life payment executes, and its cell
        // keeps the card at Executes until reviewed.
        let paid = tap(Some(1.into()));
        paid.validate().unwrap();
        assert!(
            kir::definition_cells(&paid)
                .unwrap()
                .contains(&kir::Cell::ActivationPayLife)
        );
        let tiers = kir::definition_tiers(&paid).unwrap();
        assert!(tiers.admits(kir::ExecutionTier::Executes));
        assert!(!tiers.admits(kir::ExecutionTier::HeadlessQualified));
        assert!(!tiers.admits(kir::ExecutionTier::Hosting));
        // A zero life payment is no printed cost and earns no tier.
        let zero = tap(Some(0.into()));
        assert!(
            zero.validate()
                .unwrap_err()
                .contains("cell:cost:composite")
        );
        assert!(kir::definition_cells(&zero).is_err());
    }

    /// The new mana shapes execute only: their cells start at Executes and
    /// every rider outside `kir::mana_rider` rejects the mana row.
    #[test]
    fn mana_ability_shapes_are_executes_only_cells() {
        let mana =
            |production: ManaProduction, riders: Vec<kir::Node>, pay_life: Option<Quantity>| {
                let mut d = with(|d| {
                    d.mana_abilities.push(ManaAbility {
                        restriction: None,
                        costs: PermanentActivationCosts {
                            remove_source_plus_one_counters: None,
                            tap: true,
                            mana: None,
                            sacrifice: false,
                            pay_life,
                        },
                        production,
                        riders,
                        source_block: 0,
                    })
                });
                d.capabilities = d.requirements();
                d
            };
        let fixed = mana(
            ManaProduction::Fixed {
                symbols: vec![Color::G],
            },
            vec![],
            None,
        );
        fixed.validate().unwrap();
        assert!(kir::definition_cells(&fixed).unwrap().is_empty());
        let damage = kir::Node::Do(Verb::Damage {
            source: kir::Ref::Source,
            to: kir::DamageTo::Ref {
                who: kir::Ref::controller(Context::Activated),
            },
            amount: kir::Amount::Fixed { value: 1.into() },
        });
        let pain = mana(
            ManaProduction::Choose {
                colors: vec![Color::G, Color::U],
                amount: 1.into(),
            },
            vec![damage],
            Some(1.into()),
        );
        pain.validate().unwrap();
        let cells = kir::definition_cells(&pain).unwrap();
        assert_eq!(
            cells,
            BTreeSet::from([
                kir::Cell::ManaChoose,
                kir::Cell::ManaAbilityPayLife,
                kir::Cell::ManaRiderDamageController
            ])
        );
        let tiers = kir::definition_tiers(&pain).unwrap();
        for tier in [
            kir::ExecutionTier::HeadlessQualified,
            kir::ExecutionTier::Search,
            kir::ExecutionTier::Hosting,
        ] {
            assert!(!tiers.admits(tier));
        }
        // A rider read from a target or another player is outside the row.
        let opponent = kir::Node::Do(Verb::GainLife {
            player: PlayerExpr::EachOpponent,
            amount: kir::Amount::Fixed { value: 1.into() },
        });
        let grove = mana(
            ManaProduction::Fixed {
                symbols: vec![Color::G],
            },
            vec![opponent],
            None,
        );
        assert!(grove.validate().unwrap_err().contains("cell:mana:rider"));
        // A one-color choice is a fixed production, not a choice.
        let single = mana(
            ManaProduction::Choose {
                colors: vec![Color::G],
                amount: 1.into(),
            },
            vec![],
            None,
        );
        assert!(single.validate().is_err());
    }

    #[test]
    fn composed_definitions_validate_only_through_the_matrix() {
        let tap_gain = || {
            ability(
                ActivationCost::TapSource,
                program(vec![gain(Context::Activated)], vec![]),
            )
        };
        let valid = with(|d| d.activated.push(tap_gain()));
        valid.validate().unwrap();
        assert!(valid.capabilities.contains(&Capability::ComposedAbilityV1));

        // The marker and its capability are inseparable in both directions.
        let mut hidden = valid.clone();
        hidden.capabilities.remove(&Capability::ComposedAbilityV1);
        assert!(hidden.validate().is_err());
        let mut forged = bears();
        forged.composed = false;
        assert!(forged.validate().is_err());
        // Without the marker the same ability is not a family shape.
        let mut unmarked = valid.clone();
        unmarked.composed = false;
        unmarked.capabilities = unmarked.requirements();
        assert!(unmarked.validate().is_err());

        // Instruction row: event-bound and announced-X operations stay out.
        {
            let instruction = Instruction::act(
                Verb::Draw {
                    player: PlayerExpr::you(Context::Spell),
                    amount: kir::Amount::announced_x(),
                },
                0,
            );
            let d = with(|d| {
                d.activated.push(ability(
                    ActivationCost::TapSource,
                    program(vec![instruction], vec![]),
                ))
            });
            assert!(d.validate().is_err());
        }

        // Cost row: an entity-routed target cannot be paid with pure mana,
        // and a stack target cannot be paid by tapping.
        let mana = ActivationCost::Mana {
            cost: ManaCost {
                generic: 1.into(),
                colored: vec![],
            },
        };
        let pinger = |cost: ActivationCost| {
            with(|d| {
                d.activated.push(ability(
                    cost,
                    program(vec![damage()], vec![target(TargetDomain::AnyDamageTarget)]),
                ))
            })
        };
        pinger(ActivationCost::TapSource).validate().unwrap();
        assert!(pinger(mana.clone()).validate().is_err());
        let counter = |cost: ActivationCost| {
            with(|d| {
                d.activated.push(ability(
                    cost,
                    program(
                        vec![Instruction::act(
                            Verb::Counter {
                                what: kir::Ref::Target { slot: 0 },
                            },
                            0,
                        )],
                        vec![target(TargetDomain::StackSpell)],
                    ),
                ))
            })
        };
        assert!(counter(mana).validate().is_err());
        assert!(counter(ActivationCost::TapSource).validate().is_err());

        // Target row: a triggered ability cannot announce a stack target.
        let trigger = |domain: TargetDomain, instruction: Instruction| {
            with(|d| {
                d.triggered.push(TriggeredAbility {
                    functioning_zone: FunctioningZone::Battlefield,
                    condition: TriggerCondition::SelfEntersBattlefield,
                    intervening_if: None,
                    death_filter: None,
                    program: program(vec![instruction], vec![target(domain)]),
                })
            })
        };
        trigger(TargetDomain::AnyDamageTarget, damage())
            .validate()
            .unwrap();
        assert!(
            trigger(
                TargetDomain::StackSpell,
                Instruction::act(
                    Verb::Counter {
                        what: kir::Ref::Target { slot: 0 }
                    },
                    0
                )
            )
            .validate()
            .is_err()
        );

        // Shell: an Aura on the wrong type, snow, hybrid and basic nonland cards reject.
        let aura = with(|d| {
            d.activated.push(tap_gain());
            d.characteristics.subtypes = vec!["Aura".into()];
        });
        assert!(aura.validate().is_err());
        let snow = with(|d| {
            d.activated.push(tap_gain());
            d.characteristics.snow = true;
        });
        assert!(snow.validate().is_err());
        let basic_nonland = with(|d| {
            d.activated.push(tap_gain());
            d.characteristics.basic = true;
        });
        assert!(basic_nonland.validate().is_err());

        // Provenance is checked exactly as for a family definition.
        let mut edited = valid.clone();
        edited.provenance.abilities_json.push(' ');
        assert!(edited.validate().is_err());
    }

    fn program(instructions: Vec<Instruction>, targets: Vec<TargetSpec>) -> SpellProgram {
        SpellProgram {
            optional_additional_costs: vec![],
            additional_sacrifice: None,
            timing: SpellTiming::Instant,
            resolution: SpellResolution::Instructions,
            functioning_zone: FunctioningZone::Stack,
            targets,
            instructions,
        }
    }
    fn gain(context: Context) -> Instruction {
        Instruction::act(
            Verb::GainLife {
                player: PlayerExpr::you(context),
                amount: kir::Amount::fixed(2),
            },
            0,
        )
    }

    #[test]
    fn program_targets_must_be_bound_and_used() {
        let mut p = program(vec![damage()], vec![target(TargetDomain::AnyDamageTarget)]);
        super::program(&p, Context::Spell, None, false).unwrap();
        p.targets.clear();
        assert!(super::program(&p, Context::Spell, None, false).is_err());
        p.targets.push(target(TargetDomain::AnyDamageTarget));
        p.instructions = vec![gain(Context::Spell)];
        assert!(super::program(&p, Context::Spell, None, false).is_err());
    }
}
