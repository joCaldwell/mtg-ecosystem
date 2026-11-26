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
        TargetDomain::BattlefieldPermanent
        | TargetDomain::ControlledBattlefieldPermanent
        | TargetDomain::BattlefieldCreature => (Route::Object, ANY),
        TargetDomain::AnyDamageTarget => (Route::Entity, ANY),
        TargetDomain::AnyPlayer | TargetDomain::OpponentPlayer => {
            (Route::Player, SPELL | TRIGGERED)
        }
        TargetDomain::GraveyardCard { .. }
        | TargetDomain::OwnGraveyardCreatureCard
        | TargetDomain::OwnGraveyardCard => (Route::Object, ANY),
        TargetDomain::StackSpell
        | TargetDomain::ControlledCreatureSpell
        | TargetDomain::StackActivatedOrTriggeredAbility => (Route::Stack, SPELL | ACTIVATED),
        // CR 115.1, 120.3: planeswalker faces are not admitted (see mechanics), so
        // the closed scenario's inhabitants are players, announced as
        // AnyPlayer is.
        TargetDomain::PlayerOrPlaneswalker => (Route::Player, SPELL | TRIGGERED),
        TargetDomain::ControlledBattlefieldCreature => (Route::Object, ACTIVATED),
        TargetDomain::CreatureOrPlaneswalker
        | TargetDomain::PlayerGraveyardCards { .. }
        | TargetDomain::BlockingBattlefieldCreature => return None,
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
            && p.optional_additional_costs
                .iter()
                .enumerate()
                .all(|(n, c)| {
                    !c.identity.is_empty()
                        && mana_ok(&c.mana)
                        && (c.mana.generic > Quantity::zero() || !c.mana.colored.is_empty())
                        && !p.optional_additional_costs[..n]
                            .iter()
                            .any(|o| o.identity == c.identity)
                }))
}

/// The optional-cost identities a node reads. The engine announces all
/// program targets, so it cannot admit a target in a cost-gated branch
/// (CR 601.2c, 702.33d).
/// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
fn cost_conditions<'a>(node: &'a kir::Node, out: &mut Vec<&'a String>) -> Result<(), String> {
    fn cond<'a>(c: &'a kir::Cond, out: &mut Vec<&'a String>) {
        match c {
            kir::Cond::CostPaid { identity } => out.push(identity),
            kir::Cond::Not { condition } => cond(condition, out),
            kir::Cond::All { conditions } => conditions.iter().for_each(|c| cond(c, out)),
            kir::Cond::Compare { .. }
            | kir::Cond::Exists { .. }
            | kir::Cond::ActivePlayer { .. }
            | kir::Cond::Satisfies { .. }
            | kir::Cond::CastBy { .. }
            | kir::Cond::TurnHistory { .. }
            | kir::Cond::ControlsOwnCommander { .. }
            | kir::Cond::OpponentControlsMore { .. }
            | kir::Cond::ControlledSinceLastUpkeep {} => {}
        }
    }
    match node {
        kir::Node::Do(_)
        | kir::Node::ChooseObject { .. }
        | kir::Node::TapTargets { .. }
        | kir::Node::SacrificeForTappedReturn { .. }
        | kir::Node::RemoveTargets { .. } => Ok(()),
        kir::Node::Seq(nodes) => nodes.iter().try_for_each(|n| cost_conditions(n, out)),
        kir::Node::If {
            condition,
            then,
            otherwise,
        } => {
            let before = out.len();
            cond(condition, out);
            if out.len() != before
                && (!kir::target_slots(then).is_empty()
                    || otherwise
                        .as_ref()
                        .is_some_and(|node| !kir::target_slots(node).is_empty()))
            {
                return cell("target", "cost_conditional_announcement");
            }
            cost_conditions(then, out)?;
            if let Some(otherwise) = otherwise {
                cost_conditions(otherwise, out)?;
            }
            Ok(())
        }
        kir::Node::Optional { node, .. }
        | kir::Node::UnlessPays { node, .. }
        | kir::Node::IfPays { node, .. } => cost_conditions(node, out),
        kir::Node::ForEach { body, .. } | kir::Node::Choose { body, .. } => {
            cost_conditions(body, out)
        }
        kir::Node::Delayed { .. } | kir::Node::Reflexive { .. } => Ok(()),
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
        cost_conditions(node, &mut paid)?;
    }
    if paid.iter().any(|id| {
        !p.optional_additional_costs
            .iter()
            .any(|c| c.identity == **id)
    }) {
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
    validate_target_bindings(p, Some(context))?;
    let env = kir::Env {
        context,
        targets: &p.targets,
        event,
        announced_x,
    };
    // CR 608.2c, n: the resolving spell's own shuffle completes the
    // invocation when its source leaves the stack.
    if p.instructions
        .iter()
        .enumerate()
        .any(|(index, instruction)| {
            matches!(
                instruction.verb(),
                Some(kir::Verb::Move {
                    what: kir::SetExpr::One {
                        object: kir::Ref::Source
                    },
                    to: kir::Destination::OwnerLibraryShuffled,
                })
            ) && (index + 1 != p.instructions.len()
                || p.resolution != SpellResolution::Instructions)
        })
    {
        return cell("program", "resolving_spell_shuffle_must_be_final");
    }
    for (index, instruction) in p.instructions.iter().enumerate() {
        if let Some(kir::Verb::Draw {
            player:
                kir::PlayerExpr::OwnerOf {
                    object: kir::Ref::Target { slot },
                },
            ..
        }) = instruction.verb()
            && (p.optional_target
                || !matches!(
                    index.checked_sub(1).and_then(|i| p.instructions[i].verb()),
                    Some(kir::Verb::Move {
                        what: kir::SetExpr::One { object: kir::Ref::Target { slot: moved } },
                        to: kir::Destination::OwnerLibraryShuffled,
                    }) if slot == moved
                ))
        {
            return cell(
                "program",
                "draw_requires_preceding_library_shuffle_of_target",
            );
        }
    }
    let followups: Vec<_> = p
        .instructions
        .iter()
        .enumerate()
        .filter(|(_, i)| {
            matches!(
                i,
                Instruction::Kernel {
                    node: kir::Node::Delayed { .. } | kir::Node::Reflexive { .. },
                    ..
                }
            )
        })
        .collect();
    if let [(offset, Instruction::Kernel { node, source_block })] = followups.as_slice() {
        let body = match node {
            kir::Node::Delayed { program, .. } | kir::Node::Reflexive { program, .. } => program,
            _ => unreachable!(),
        };
        if *offset + 1 != p.instructions.len()
            || body
                .instructions
                .iter()
                .any(|i| i.source_block() != *source_block)
        {
            return cell("program", "resolution_trigger_final_instruction_and_label");
        }
    } else if !followups.is_empty() {
        return cell("program", "multiple_resolution_triggers");
    }
    let mut used = BTreeSet::new();
    for Instruction::Kernel { node, .. } in &p.instructions {
        used.extend(kir::check(node, &env)?.target_slots);
    }
    if used.len() != p.targets.len() {
        return Err("unused target occurrence".into());
    }
    Ok(())
}

/// A group leaf with its member slot erased: equal shapes are the same
/// action applied to different members. A leaf reading a resolution-time
/// count has no shape: each member's leaf would re-read the count after
/// earlier members changed (CR 608.2h).
fn group_leaf_shape(node: &kir::Node) -> Option<serde_json::Value> {
    fn erase(value: &mut serde_json::Value) -> bool {
        match value {
            serde_json::Value::Object(map) => {
                if matches!(
                    map.get("amount").and_then(|a| a.as_str()),
                    Some("count" | "greatest" | "devotion")
                ) {
                    return false;
                }
                if map.get("ref").and_then(|r| r.as_str()) == Some("target") {
                    map.insert("slot".into(), 0.into());
                }
                map.values_mut().all(erase)
            }
            serde_json::Value::Array(items) => items.iter_mut().all(erase),
            _ => true,
        }
    }
    let mut value = serde_json::to_value(node).ok()?;
    erase(&mut value).then_some(value)
}

pub(in crate::kir) fn validate_resolution_targets(p: &SpellProgram) -> Result<(), String> {
    if p.targets
        .iter()
        .any(|t| t.chooser != Chooser::Controller || t.stage != EvaluationStage::Announcement)
    {
        return cell("target", "resolution_trigger_announcement");
    }
    validate_target_bindings(p, None)
}

fn validate_target_bindings(p: &SpellProgram, context: Option<Context>) -> Result<(), String> {
    // CR 115.1d, 601.2c, 608.2b: zero targets is a legal announcement.
    // A skipped occurrence can be read only by an independent leaf whose
    // entire action applies to that one object.
    if p.targets.iter().any(|t| t.selection_id.contains('/')) && p.target_group().is_none() {
        return cell("target", "group_identity_outside_one_group");
    }
    if let Some(count) = p.target_group() {
        // CR 115.3, 601.2c, 608.2b: one counted occurrence. Battlefield
        // removal and tap status change every legal member in one
        // simultaneous event (CR 603.6a, 603.10a look-back sees the whole
        // group). Order-free actions are one independent leaf per member, so
        // an unchosen or illegal member skips exactly its own leaves: P/T
        // and keyword changes, counters, combat restrictions, a spell's own
        // damage, and moving a card from its owner's graveyard to hand or
        // exile.
        let targeted: Vec<&kir::Node> = p
            .instructions
            .iter()
            .filter_map(|Instruction::Kernel { node, .. }| {
                (!kir::target_slots(node).is_empty()).then_some(node)
            })
            .collect();
        let battlefield = matches!(
            p.targets[0].domain,
            TargetDomain::BattlefieldCreature
                | TargetDomain::BattlefieldPermanent
                | TargetDomain::ControlledBattlefieldPermanent
        );
        let graveyard = matches!(
            p.targets[0].domain,
            TargetDomain::GraveyardCard { .. }
                | TargetDomain::OwnGraveyardCard
                | TargetDomain::OwnGraveyardCreatureCard
        );
        let batch = battlefield
            && matches!(targeted.as_slice(),
                [kir::Node::TapTargets { count: c, .. } | kir::Node::RemoveTargets { count: c, .. }]
                    if *c as usize == count);
        // Leaves come in runs of `count`: member `k` of a run reads slot `k`
        // and the run's leaves are one action up to that slot, so a leaf that
        // reads only some members cannot silently skip the rest.
        let order_free = |node: &kir::Node| match node.verb() {
            Some(
                kir::Verb::ModifyUntil { .. }
                | kir::Verb::PutCounters { .. }
                | kir::Verb::RestrictCombat { .. },
            ) => battlefield,
            Some(kir::Verb::Damage { .. }) => battlefield && context == Some(Context::Spell),
            Some(kir::Verb::Move {
                to: kir::Destination::OwnerHand | kir::Destination::Exile,
                ..
            }) => graveyard,
            _ => false,
        };
        let leaves = !targeted.is_empty()
            && targeted.len().is_multiple_of(count)
            && targeted.chunks(count).all(|run| {
                let shape = group_leaf_shape(run[0]);
                run.iter().enumerate().all(|(k, node)| {
                    order_free(node)
                        && node.group_leaf_slot() == Some(k as u32)
                        && shape.is_some()
                        && group_leaf_shape(node) == shape
                })
            });
        let sacrifice_return = !p.optional_target
            && matches!(targeted.as_slice(),
            [kir::Node::SacrificeForTappedReturn { count: c }] if *c as usize == count);
        if !(batch || leaves || sacrifice_return) {
            return cell("target", "group_requires_batch_or_order_free_leaves");
        }
    } else if p.targets.len() > 1 && p.optional_target {
        return cell("target", "up_to_many_requires_one_target_group");
    } else if p.optional_target
        && (!matches!(p.targets.as_slice(), [t] if matches!(t.domain,
                TargetDomain::BattlefieldCreature | TargetDomain::BattlefieldPermanent
                | TargetDomain::ControlledBattlefieldPermanent
                | TargetDomain::GraveyardCard { .. } | TargetDomain::OwnGraveyardCard | TargetDomain::OwnGraveyardCreatureCard))
            || p.instructions
                .iter()
                .any(|Instruction::Kernel { node, .. }| {
                    !kir::target_slots(node).is_empty()
                        && !matches!(node, kir::Node::Do(
                        kir::Verb::Destroy { what, .. }
                        | kir::Verb::Tap { what }
                        | kir::Verb::Untap { what }
                        | kir::Verb::Move { what, .. }
                        | kir::Verb::PutCounters { on: what, .. }
                        | kir::Verb::ModifyUntil { what, .. }
                    ) if *what == kir::SetExpr::target(0))
                }))
    {
        return cell("target", "up_to_one_requires_independent_object_leaves");
    }
    if p.target_group().is_none()
        && p.instructions
            .iter()
            .any(|Instruction::Kernel { node, .. }| {
                matches!(
                    node,
                    kir::Node::TapTargets { .. }
                        | kir::Node::RemoveTargets { .. }
                        | kir::Node::SacrificeForTappedReturn { .. }
                )
            })
    {
        return cell("target", "batch_node_requires_one_target_group");
    }
    // CR 109.2, 201.5: "another" excludes the ability's own source. With a
    // second occurrence it would instead name a different target, so it is
    // admitted for a permanent's ability with one occurrence or one group.
    if p.targets.iter().any(|t| {
        t.filter
            .as_ref()
            .and_then(|f| f.predicate.as_ref())
            .is_some_and(kir::excludes_source)
    }) && !(matches!(context, Some(Context::Activated | Context::Triggered))
        && (p.targets.len() == 1 || p.target_group().is_some()))
    {
        return cell("target", "another_requires_one_ability_occurrence");
    }
    let mut ids = BTreeSet::new();
    for (index, target) in p.targets.iter().enumerate() {
        if usize::try_from(target.slot).ok() != Some(index)
            || target.selection_id.is_empty()
            || !ids.insert(&target.selection_id)
        {
            return Err("unbound, duplicate, or noncontiguous target occurrence".into());
        }
    }
    // CR 115.3, 601.2c: separate fight occurrences choose independently.
    // This exception stays in the homogeneous object announcement route.
    let fight_pair = matches!(p.targets.as_slice(), [_, _])
        && p.instructions.iter().any(|i| {
            matches!(
                i.verb(),
                Some(kir::Verb::Fight {
                    first: kir::Ref::Target { slot: 0 },
                    second: kir::Ref::Target { slot: 1 },
                })
            )
        });
    if !fight_pair
        && p.targets
            .windows(2)
            .any(|pair| pair[0].domain != pair[1].domain || pair[0].filter != pair[1].filter)
    {
        return Err("mixed target domains or filters are unsupported".into());
    }
    for t in &p.targets {
        if let Some(f) = &t.filter {
            if !matches!(
                t.domain,
                TargetDomain::BattlefieldPermanent
                    | TargetDomain::ControlledBattlefieldPermanent
                    | TargetDomain::StackSpell
                    | TargetDomain::GraveyardCard { .. }
                    | TargetDomain::OwnGraveyardCard
                    | TargetDomain::OwnGraveyardCreatureCard
            ) || (t.domain == TargetDomain::StackSpell
                && (f.types.is_empty() && f.non_types.is_empty() && f.predicate.is_none()
                    || !f.non_colors.is_empty()
                    || f.opponent
                    || f.power.is_some()
                    || f.tapped))
                || (matches!(
                    t.domain,
                    TargetDomain::GraveyardCard { .. }
                        | TargetDomain::OwnGraveyardCard
                        | TargetDomain::OwnGraveyardCreatureCard
                ) && ((f.types.is_empty() && f.predicate.is_none())
                    || !f.non_types.is_empty()
                    || !f.non_colors.is_empty()
                    || f.opponent
                    || f.power.is_some()
                    || f.tapped))
                || (matches!(t.domain, TargetDomain::OwnGraveyardCreatureCard)
                    && (!f.types.is_empty() || f.predicate.is_none()))
                || f.predicate.as_ref().is_some_and(|predicate| {
                    if t.domain == TargetDomain::StackSpell {
                        !matches!(predicate.0.as_slice(),
                            [kir::Atom::ManaValue { value: kir::Amount::Fixed { value }, .. }]
                                if !value.is_negative())
                    } else {
                        f.power.is_some()
                            || kir::audit_target_predicate(
                                predicate,
                                matches!(
                                    t.domain,
                                    TargetDomain::GraveyardCard { .. }
                                        | TargetDomain::OwnGraveyardCard
                                        | TargetDomain::OwnGraveyardCreatureCard
                                ),
                            )
                            .is_err()
                    }
                })
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
                || (t.domain == TargetDomain::ControlledBattlefieldPermanent && f.opponent)
                || (matches!(
                    t.domain,
                    TargetDomain::BattlefieldPermanent
                        | TargetDomain::ControlledBattlefieldPermanent
                ) && f
                    .types
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
fn cost(d: &Definition, a: &ActivatedAbility, p: &SpellProgram) -> Result<(), String> {
    let kind = d.characteristics.kind;
    let route = p
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
        ActivationCost::ExileSource { mana } => {
            a.functioning_zone == FunctioningZone::Graveyard
                && mana_ok(mana)
                && matches!(
                    route,
                    None | Some(Route::Object | Route::Entity | Route::Stack)
                )
        }
        ActivationCost::ChooseObjects(cost) => {
            use ChosenCostAction::{Discard, ExileFromGraveyard, Tap};
            use ChosenCostFilter::{Any, ArtifactOrCreature, Subtype, Type};
            a.functioning_zone == FunctioningZone::Battlefield
                && cost.count > 0
                && cost.mana.as_ref().is_none_or(mana_ok)
                && (!cost.another || cost.action != Discard)
                && match (&cost.action, &cost.filter) {
                    (Tap, Type(CardKind::Instant | CardKind::Sorcery)) => false,
                    (Discard, Subtype(_) | ArtifactOrCreature) => false,
                    (ExileFromGraveyard, ArtifactOrCreature) => false,
                    (_, Subtype(name)) => {
                        !name.is_empty()
                            && name.chars().all(|c| c.is_ascii_alphabetic() || c == '-')
                    }
                    (_, Any | Type(_) | ArtifactOrCreature) => true,
                }
                && matches!(route, None | Some(Route::Object | Route::Entity))
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
                && costs
                    .pay_life
                    .as_ref()
                    .is_none_or(|n| *n > Quantity::zero())
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
    // CR 602.2b, 700.2: a modal activation is a battlefield ability whose
    // every selection is checked below as its own program.
    if let Some(modal) = &a.modal {
        // The announcement after modes reuses the targeted activation
        // route, which pays only these costs.
        if a.functioning_zone != FunctioningZone::Battlefield
            || matches!(
                a.cost,
                ActivationCost::TapPayLifeSacrificeSource { .. }
                    | ActivationCost::DiscardSource { .. }
            )
        {
            return cell("modal", "activation_source_or_cost");
        }
        modal_shape(&a.program, modal, true)?;
    }
    if matches!(a.cost, ActivationCost::ExileSource { .. }) {
        if a.restriction.is_some() {
            return cell("activated", "off_battlefield_restriction");
        }
        cost(d, a, &a.program)?;
        return program(&a.program, Context::Activated, None, false);
    }
    // CR 702.84a: unearth is a sorcery-speed graveyard ability of a
    // permanent card paying one fixed mana cost.
    if a.functioning_zone == FunctioningZone::Graveyard
        && matches!(a.cost, ActivationCost::Mana { .. })
    {
        let ActivationCost::Mana { cost } = &a.cost else {
            unreachable!()
        };
        if !a.is_unearth()
            || !matches!(
                d.characteristics.kind,
                CardKind::Creature | CardKind::Artifact | CardKind::Enchantment
            )
            || cost.generic.is_negative()
            || cost.colored.contains(&Color::C)
            || (cost.generic == Quantity::zero() && cost.colored.is_empty())
        {
            return cell("activated", "unearth");
        }
        return program(&a.program, Context::Activated, None, false);
    }
    // CR 702.29a/e, 602.2a: cycling and typecycling use fixed mana and the
    // exact hand card, then draw one or search, reveal and shuffle.
    if a.functioning_zone == FunctioningZone::Hand {
        if !hand_discard_activation(a) {
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
    if let Some(r) = &a.restriction
        && (r.window.is_none() && !r.once_each_turn && r.condition.is_none()
            || r.window.is_some() && a.program.timing == SpellTiming::MainPhase
            || r.condition
                .as_ref()
                .is_some_and(|c| kir::ActivationCondition::of(c).is_err()))
    {
        return cell("activated", "restriction");
    }
    if a.programs()
        .flat_map(|p| &p.targets)
        .any(|t| t.domain == TargetDomain::ControlledBattlefieldCreature)
        && !(a.modal.is_none()
            && d.characteristics.subtypes == ["Equipment"]
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
    for p in a.programs() {
        cost(d, a, p)?;
        program(p, Context::Activated, None, false)?;
    }
    Ok(())
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

/// The event row: an admitted event pattern and intervening condition
/// (CR 603.2, 603.4) on a battlefield permanent's triggered ability.
fn triggered(d: &Definition, a: &TriggeredAbility) -> Result<(), String> {
    let kind = d.characteristics.kind;
    let pattern = a.condition.pattern();
    let key = a.condition.reporting_key();
    kir::check_event(pattern, d.characteristics.is_attachment())
        .map_err(|e| format!("cell:trigger:{key}:{e}"))?;
    if let Some(condition) = a.condition.intervening_if()
        && !kir::Intervening::of(condition).is_ok_and(|i| i.admits(pattern))
    {
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
    if let Some(modal) = &a.modal {
        modal_shape(&a.program, modal, true)?;
    }
    for p in a.programs() {
        program(p, Context::Triggered, Some(&a.condition), false)?;
    }
    Ok(())
}

/// The mana row: a tap ability of a creature, artifact or land that adds
/// fixed, chosen, any-color, commander-color or "could produce" mana, may
/// also cost mana (CR 605.3a, 601.2g-h) or life (CR 119.4), and may perform
/// admitted riders after adding mana ([`kir::mana_rider`]).
/// CR 605.1a, 605.3: mana abilities do not use the stack.
/// CR 106.1, 106.4, 106.7: the admitted productions.
pub(crate) fn mana_production_supported(production: &ManaProduction) -> bool {
    let distinct = |colors: &[Color]| {
        colors
            .iter()
            .enumerate()
            .all(|(i, c)| !colors[..i].contains(c))
    };
    match production {
        ManaProduction::Fixed { symbols } => !symbols.is_empty(),
        ManaProduction::Bundles { options } => {
            (2..=128).contains(&options.len())
                && options.iter().enumerate().all(|(i, bundle)| {
                    (1..=5).contains(&bundle.len())
                        && bundle.windows(2).all(|pair| pair[0] <= pair[1])
                        && !options[..i].contains(bundle)
                })
        }
        ManaProduction::AnyColor
        | ManaProduction::CommanderColor
        | ManaProduction::LandsCouldProduce { .. } => true,
        ManaProduction::Choose { colors, amount } => {
            colors.len() >= 2 && distinct(colors) && *amount >= Quantity::from(1)
        }
    }
}

fn mana_ability(d: &Definition, a: &ManaAbility) -> Result<(), String> {
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
        || a.spend_restriction.is_some_and(|r| !r.is_canonical())
        || !mana_production_supported(&a.production)
    {
        return cell("mana", name(&a.production));
    }
    if let Some(error) = a.riders.iter().find_map(|r| kir::mana_rider(r).err()) {
        return cell("mana", format!("rider:{error}"));
    }
    // A mana cost would pay nested mana after the condition was read; no
    // admitted conditioned mana ability needs that order.
    if a.condition
        .as_ref()
        .is_some_and(|c| kir::ActivationCondition::of(c).is_err() || a.costs.mana.is_some())
    {
        return cell("mana", "condition");
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
                // CR 702.16a-f: printed protection from one fixed color.
                | K::ProtectionFromWhite
                | K::ProtectionFromBlue
                | K::ProtectionFromBlack
                | K::ProtectionFromRed
                | K::ProtectionFromGreen
                // CR 702.13, 702.14, 702.28, 702.31, 702.36, 702.118.
                | K::Plainswalk
                | K::Islandwalk
                | K::Swampwalk
                | K::Mountainwalk
                | K::Forestwalk
                | K::Intimidate
                | K::Shadow
                | K::Horsemanship
                | K::Fear
                | K::Skulk
        ),
        CardKind::Artifact | CardKind::Enchantment => matches!(k, K::Flash | K::Indestructible),
        CardKind::Land => k == K::Indestructible,
        CardKind::Instant | CardKind::Sorcery => false,
    }
}

/// Complete admission of a composed definition. Nothing here defers to a
/// whole-card family, so a rejected row can never be reached another way.
pub(crate) fn validate(d: &Definition) -> Result<(), String> {
    let p = &d.provenance;
    let source: serde_json::Value = strict_read(&p.source_json)?;
    let abilities_json = match &d.back_face {
        Some(back) => format!("[{},{}]", p.abilities_json, back.provenance.abilities_json),
        None => format!("[{}]", p.abilities_json),
    };
    if !accepted_provenance(&p.archive_manifest_sha256, &p.parser_sha256)
        || sha256(p.source_json.as_bytes()) != p.source_sha256
        || sha256(abilities_json.as_bytes()) != p.ast_sha256
    {
        return Err("invalid source/AST provenance".into());
    }
    // Scryfall's layout names the printed relationship of the two parts
    // (https://scryfall.com/docs/api/layouts); every other layout rejects.
    let layout = match source["layout"].as_str() {
        Some("modal_dfc") => FaceLayout::ModalDoubleFaced,
        Some("transform") => FaceLayout::Transforming,
        Some("adventure") => FaceLayout::Adventurer,
        _ => FaceLayout::Single,
    };
    if d.layout != layout || d.back_face.is_some() == layout.is_single() {
        return Err("definition layout disagrees with its printed source".into());
    }
    let Some(back) = &d.back_face else {
        if d.characteristics.front_face_mana_value.is_some() || adventure_subtype(d) {
            return Err("single-face definition has two-part characteristics".into());
        }
        return validate_face(d, &source);
    };
    let printed = source["card_faces"]
        .as_array()
        .filter(|faces| faces.len() == 2)
        .ok_or("two-part definition requires two printed faces")?;
    if !back.composed
        || !back.layout.is_single()
        || back.back_face.is_some()
        || back.oracle_id != d.oracle_id
        || back.characteristics.color_identity != d.characteristics.color_identity
        || source["color_identity"]
            != serde_json::to_value(&d.characteristics.color_identity).map_err(|e| e.to_string())?
        || [d, back].into_iter().zip(printed).any(|(face, source)| {
            source["name"] != face.name || face.source_labels.is_none() || face.flashback.is_some()
        })
        || d.characteristics.front_face_mana_value.is_some()
        || adventure_subtype(d)
    {
        return Err("unsupported or inconsistent two-part definition".into());
    }
    let permanent = |d: &Definition| {
        !matches!(
            d.characteristics.kind,
            CardKind::Instant | CardKind::Sorcery
        )
    };
    let shape = match d.layout {
        FaceLayout::ModalDoubleFaced => {
            back.characteristics.front_face_mana_value.is_none() && !adventure_subtype(back)
        }
        // CR 712.2, 712.8e, 712.10, 202.3b: both faces of an admitted
        // transforming card are permanents; the back face has no mana cost
        // and takes its mana value from the front face's mana cost. A
        // permanent that stops being a creature leaves combat (CR 506.4);
        // that transition has no reviewed executor, so either both faces
        // are creatures or neither is.
        FaceLayout::Transforming => {
            let c = &back.characteristics;
            permanent(d)
                && permanent(back)
                && d.characteristics.has_type(CardKind::Creature) == c.has_type(CardKind::Creature)
                && c.mana_cost.generic == Quantity::zero()
                && c.mana_cost.colored.is_empty()
                && c.hybrid_mana.is_empty()
                && !back.x_mana_cost
                && !d.x_mana_cost
                && c.front_face_mana_value.as_ref()
                    == Some(
                        &Characteristics {
                            front_face_mana_value: None,
                            ..d.characteristics.clone()
                        }
                        .printed_mana_value(),
                    )
                && !adventure_subtype(back)
        }
        // CR 715.2-715.3: the inset Adventure is an instant or sorcery spell
        // with no ability other than its spell effect; the card's normal
        // characteristics are a permanent's.
        FaceLayout::Adventurer => {
            permanent(d)
                && !permanent(back)
                && adventure_subtype(back)
                && back.spell.is_some()
                && back.characteristics.front_face_mana_value.is_none()
                && back.activated.is_empty()
                && back.triggered.is_empty()
                && back.mana_abilities.is_empty()
                && back.static_abilities.is_empty()
                && back.keywords.is_empty()
        }
        FaceLayout::Single => false,
    };
    if !shape {
        return Err("unsupported or inconsistent two-part definition".into());
    }
    let mut back_provenance = back.provenance.clone();
    back_provenance.abilities_json.clone_from(&p.abilities_json);
    if &back_provenance != p {
        return Err("two-part source provenance differs by face".into());
    }
    validate_face(d, &printed[0])?;
    validate_face(back, &printed[1])
}

/// CR 715.2: the Adventure subtype marks an adventurer's inset spell.
fn adventure_subtype(d: &Definition) -> bool {
    d.characteristics.subtypes.iter().any(|s| s == "Adventure")
}

fn validate_face(d: &Definition, source: &serde_json::Value) -> Result<(), String> {
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
    if let Some(labels) = &d.source_labels
        && (source["oracle_text"].as_str() != Some(labels.text.as_str())
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
            || d.spell_programs()
                .chain(d.activated.iter().flat_map(ActivatedAbility::programs))
                .chain(d.triggered.iter().flat_map(TriggeredAbility::programs))
                .flat_map(|program| &program.instructions)
                .any(|instruction| instruction.source_block() as usize >= labels.blocks.len())
            || !labels.spell_modes.is_empty())
    {
        return Err("invalid source label provenance".into());
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
    // reject, including snow and Sagas. A spell's X and hybrid or
    // Phyrexian symbols are announced while casting (CR 107.3, 107.4e-f,
    // 601.2b/f); a land has no mana cost to announce (CR 305.1).
    // CR 205.4: the basic supertype is admitted only on a land, whose shell
    // below then requires the land's intrinsic mana (CR 305.6).
    // CR 107.4e-f: a two-color hybrid symbol names two distinct colors and a
    // Phyrexian symbol one color and its 2-life alternative.
    let hybrid_symbol = |symbol: &ColorHybridSymbol| match symbol.colors {
        [HybridPayment::Mana(a), HybridPayment::Mana(b)] => {
            a != b && a != Color::C && b != Color::C
        }
        [HybridPayment::Mana(a), HybridPayment::PayTwoLife] => a != Color::C,
        _ => false,
    };
    if ((!c.hybrid_mana.is_empty() || d.x_mana_cost) && d.spell.is_none())
        || !c.hybrid_mana.iter().all(hybrid_symbol)
        || c.snow
        || (c.basic && c.kind != CardKind::Land)
        || c.subtypes.iter().any(|s| {
            // CR 715.2: only an instant or sorcery Adventure; `validate`
            // admits it only as an adventurer's back part.
            (unsupported_rules_subtype(s)
                && !(s == "Adventure"
                    && matches!(c.kind, CardKind::Instant | CardKind::Sorcery)
                    && c.subtypes == ["Adventure"]))
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
            && !p.optional_target
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
                    // CR 604.3, 613.4a: a characteristic-defining ability
                    // supplies the toughness; zero is a state-based death.
                    if d.static_abilities.iter().any(|a| {
                        matches!(
                            a.effect,
                            kir::StaticEffect::DefinePowerToughness {
                                toughness: Some(_),
                                ..
                            }
                        )
                    }) {
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
                && d.activated.iter().all(|a| {
                    matches!(
                        a.functioning_zone,
                        FunctioningZone::Hand | FunctioningZone::Graveyard
                    )
                })
                && d.triggered.is_empty()
                && d.mana_abilities.is_empty()
                && d.spell.as_ref().is_some_and(|p| p.timing == cast_timing)
        }
    };
    if !shell {
        return cell("shell", name(&c.kind));
    }
    if d.modal_spell.is_some() && !matches!(c.kind, CardKind::Instant | CardKind::Sorcery) {
        return cell("modal", "nonspell");
    }
    // CR 702.34a: flashback is a fixed mana alternative cost of an instant
    // or sorcery whose own cost announces no X or hybrid choice.
    if let Some(cost) = &d.flashback
        && (!matches!(c.kind, CardKind::Instant | CardKind::Sorcery)
            || d.x_mana_cost
            || !c.hybrid_mana.is_empty()
            || cost.generic.is_negative()
            || cost.colored.contains(&Color::C))
    {
        return cell("keyword", "flashback");
    }
    // CR 702.74a, 118.9: evoke is a fixed mana alternative cost of a
    // permanent card whose own cost announces no X or hybrid choice, and the
    // definition carries exactly the one sacrifice trigger that reads it.
    let evoke_triggers = d
        .triggered
        .iter()
        .filter(|t| {
            matches!(
                t.condition.intervening_if(),
                Some(kir::Cond::CostPaid { .. })
            )
        })
        .collect::<Vec<_>>();
    match &d.alternative_cost {
        None if evoke_triggers.is_empty() => {}
        Some(cost)
            if cost.identity == crate::EVOKE_COST
                && matches!(
                    c.kind,
                    CardKind::Creature | CardKind::Artifact | CardKind::Enchantment
                )
                && !d.x_mana_cost
                && c.hybrid_mana.is_empty()
                && !cost.mana.generic.is_negative()
                && !cost.mana.colored.contains(&Color::C)
                && matches!(evoke_triggers.as_slice(), [t]
                    if **t == TriggeredAbility::evoke_sacrifice(cost.source_block)) => {}
        _ => return cell("keyword", "evoke"),
    }
    // CR 702.30a: an echo trigger is exactly the keyword's own ability on a
    // permanent card, paying one fixed mana cost.
    for t in &d.triggered {
        if matches!(
            t.condition.intervening_if(),
            Some(kir::Cond::ControlledSinceLastUpkeep {})
        ) {
            let echo = t.program.instructions.first().and_then(|i| match i {
                Instruction::Kernel {
                    node:
                        kir::Node::UnlessPays {
                            payment: kir::Payment::Mana { mana },
                            ..
                        },
                    source_block,
                } => Some((*source_block, mana.clone())),
                Instruction::Kernel {
                    node:
                        kir::Node::UnlessPays {
                            payment: kir::Payment::Generic { amount },
                            ..
                        },
                    source_block,
                } => amount.as_fixed().map(|generic| {
                    (
                        *source_block,
                        ManaCost {
                            generic: generic.clone(),
                            colored: vec![],
                        },
                    )
                }),
                _ => None,
            });
            if !matches!(
                c.kind,
                CardKind::Creature | CardKind::Artifact | CardKind::Enchantment
            ) || !echo.is_some_and(|(block, mana)| {
                !mana.generic.is_negative()
                    && !mana.colored.contains(&Color::C)
                    && (mana.generic > Quantity::zero() || !mana.colored.is_empty())
                    && *t == TriggeredAbility::echo(block, mana)
            }) {
                return cell("keyword", "echo");
            }
        }
    }
    if matches!(c.kind, CardKind::Instant | CardKind::Sorcery) {
        if let Some(modal) = &d.modal_spell {
            modal_shape(
                d.spell.as_ref().ok_or("missing spell program")?,
                modal,
                false,
            )?;
        }
        for variant in d.spell_programs() {
            program(variant, Context::Spell, None, d.x_mana_cost)?;
        }
    }
    if d.static_abilities
        .iter()
        .filter(|a| {
            matches!(
                a.effect,
                kir::StaticEffect::PayLifeOrEnterTapped { .. }
                    | kir::StaticEffect::RevealOrEnterTapped { .. }
            )
        })
        .count()
        > 1
    {
        return cell("static", "multiple_entry_choices");
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

/// CR 700.2a–d: a legal selection has every target the selected modes
/// require. This row admits one target at most, using an existing casting,
/// activation or trigger-placement decision with a mandatory slot: a
/// battlefield permanent or creature (possibly one its controller controls),
/// a card in its controller's graveyard, any damage target or a player.
fn modal_shape(base: &SpellProgram, modal: &ModalSpell, ability: bool) -> Result<(), String> {
    let count = modal.labels.len();
    if !(2..=ModalSpell::MAX_MODES).contains(&count)
        || modal.labels.iter().any(|label| label.trim().is_empty())
        || modal.min == 0
        || modal.min > modal.max
        || modal.max as usize > count
        || !modal
            .selections
            .keys()
            .copied()
            .eq(ModalSpell::masks(count, modal.min, modal.max))
        || base.functioning_zone != FunctioningZone::Stack
        || base.resolution != SpellResolution::Instructions
        || !base.targets.is_empty()
        || !base.instructions.is_empty()
        || base.additional_sacrifice.is_some()
        || !base.optional_additional_costs.is_empty()
    {
        return cell("modal", "announcement_shape");
    }
    for variant in modal.selections.values() {
        if variant.timing != base.timing
            || variant.additional_sacrifice.is_some()
            || !variant.optional_additional_costs.is_empty()
            || variant
                .instructions
                .iter()
                .any(|i| i.source_block() != modal.source_block)
            || !matches!(
                variant.targets.as_slice(),
                [] | [TargetSpec {
                    domain: TargetDomain::BattlefieldPermanent
                        | TargetDomain::BattlefieldCreature
                        | TargetDomain::ControlledBattlefieldPermanent
                        | TargetDomain::ControlledBattlefieldCreature
                        | TargetDomain::GraveyardCard { .. }
                        | TargetDomain::OwnGraveyardCard
                        | TargetDomain::OwnGraveyardCreatureCard
                        | TargetDomain::AnyDamageTarget
                        | TargetDomain::AnyPlayer
                        | TargetDomain::PlayerOrPlaneswalker
                        | TargetDomain::OpponentPlayer,
                    ..
                }]
            )
            || (ability && variant.optional_target)
        {
            return cell("modal", "selection_shape");
        }
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
            modal: None,
            functioning_zone: FunctioningZone::Battlefield,
            restriction: None,
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
        assert!(zero.validate().unwrap_err().contains("cell:cost:composite"));
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
                        condition: None,
                        costs: PermanentActivationCosts {
                            remove_source_plus_one_counters: None,
                            tap: true,
                            mana: None,
                            sacrifice: false,
                            pay_life,
                        },
                        production,
                        spend_restriction: None,
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

        // Cost row: the entity route admits pure tap, and the stack route
        // admits mana. Pure tap on the stack route remains unaudited.
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
        counter(mana).validate().unwrap();
        assert!(counter(ActivationCost::TapSource).validate().is_err());

        // Target row: a triggered ability cannot announce a stack target.
        let trigger = |domain: TargetDomain, instruction: Instruction| {
            with(|d| {
                d.triggered.push(TriggeredAbility {
                    modal: None,
                    functioning_zone: FunctioningZone::Battlefield,
                    condition: TriggerCondition::on(kir::EventPattern::self_enters()),
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
            optional_target: false,
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
    fn hybrid_symbols_must_be_reviewed_pairs() {
        let symbol = |colors| ColorHybridSymbol { colors };
        let spell = |colors| with(|d| d.characteristics.hybrid_mana = vec![symbol(colors)]);
        use HybridPayment::{Mana, PayTwoLife};
        for colors in [
            [Mana(Color::C), PayTwoLife],
            [PayTwoLife, PayTwoLife],
            [Mana(Color::G), Mana(Color::G)],
            [PayTwoLife, Mana(Color::G)],
            [Mana(Color::G), Mana(Color::C)],
        ] {
            let d = spell(colors);
            assert!(
                d.validate()
                    .unwrap_err()
                    .contains("cell:shell:characteristics"),
                "{colors:?}"
            );
            assert!(kir::definition_tiers(&d).is_err());
        }
        for colors in [
            [Mana(Color::G), PayTwoLife],
            [Mana(Color::G), Mana(Color::U)],
        ] {
            let d = spell(colors);
            d.validate().unwrap();
            assert!(
                kir::definition_tiers(&d)
                    .unwrap()
                    .admits(kir::ExecutionTier::Executes)
            );
        }
    }

    /// CR 115.3, 603.6a, 603.10a, 608.2b: a counted occurrence removes its
    /// battlefield members in one batch; per-member removal leaves and a
    /// graveyard group entering the battlefield one at a time reject.
    #[test]
    fn counted_target_groups_admit_batches_and_order_free_leaves() {
        let group = |domain: TargetDomain, n: u32| -> Vec<TargetSpec> {
            (0..n)
                .map(|slot| {
                    let mut t = target(domain);
                    t.slot = slot;
                    t.selection_id = if slot == 0 {
                        "line:0:1".into()
                    } else {
                        format!("line:0:1/{slot}")
                    };
                    t
                })
                .collect()
        };
        let leaf = |verb: fn(u32) -> Verb, n: u32| -> Vec<Instruction> {
            (0..n).map(|slot| Instruction::act(verb(slot), 0)).collect()
        };
        let creatures = group(TargetDomain::BattlefieldCreature, 2);
        let batch = Instruction::Kernel {
            node: kir::Node::RemoveTargets {
                count: 2,
                removal: kir::GroupRemoval::Destroy,
            },
            source_block: 0,
        };
        let p = program(vec![batch.clone()], creatures.clone());
        assert_eq!(p.target_group(), Some(2));
        super::program(&p, Context::Spell, None, false).unwrap();
        let destroy = |slot| Verb::Destroy {
            what: kir::SetExpr::target(slot),
            regeneration_forbidden: false,
        };
        let sequential = program(leaf(destroy, 2), creatures.clone());
        assert!(
            super::program(&sequential, Context::Spell, None, false)
                .unwrap_err()
                .contains("group_requires_batch_or_order_free_leaves")
        );
        let cards = group(TargetDomain::OwnGraveyardCreatureCard, 2);
        let enter = |slot| Verb::Move {
            what: kir::SetExpr::target(slot),
            to: kir::Destination::Battlefield {
                controller: PlayerExpr::you(Context::Spell),
                tapped: false,
            },
        };
        assert!(
            super::program(&program(leaf(enter, 2), cards), Context::Spell, None, false).is_err()
        );
        // Order-free leaves need one identical action per member: a follow-up
        // leaf on member zero alone would skip the other member.
        let counter = |slot| Verb::PutCounters {
            on: kir::SetExpr::target(slot),
            counter: kir::CounterKind::PlusOnePlusOne,
            amount: kir::Amount::fixed(1),
        };
        let both = program(leaf(counter, 2), creatures.clone());
        super::program(&both, Context::Spell, None, false).unwrap();
        let mut partial = both.clone();
        partial.instructions.push(Instruction::act(counter(0), 0));
        assert!(
            super::program(&partial, Context::Spell, None, false)
                .unwrap_err()
                .contains("group_requires_batch_or_order_free_leaves")
        );
        let mut mismatched = both;
        mismatched.instructions[1] = Instruction::act(destroy(1), 0);
        assert!(super::program(&mismatched, Context::Spell, None, false).is_err());
        // A resolution-time count is never re-read once per member.
        let counted = |slot| Verb::PutCounters {
            on: kir::SetExpr::target(slot),
            counter: kir::CounterKind::PlusOnePlusOne,
            amount: kir::Amount::Count {
                set: kir::SetExpr::battlefield(kir::Pred::creatures()),
            },
        };
        assert!(
            super::program(
                &program(leaf(counted, 2), creatures.clone()),
                Context::Spell,
                None,
                false
            )
            .is_err()
        );
        // A group identity outside one whole group, and a batch without one.
        let mut split = creatures.clone();
        split[1].selection_id = "line:0:9/1".into();
        assert!(
            super::program(
                &program(vec![batch.clone()], split),
                Context::Spell,
                None,
                false
            )
            .is_err()
        );
        let mut other = creatures;
        other[1].selection_id = "line:0:9".into();
        assert!(super::program(&program(vec![batch], other), Context::Spell, None, false).is_err());
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
