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
/// Bound to one family's event data, announcement or sibling instruction.
const FAMILY: u8 = 0;

/// The instruction row of the matrix. The match is exhaustive on purpose: a
/// new instruction does not compile until its portability is declared here.
///
/// A row is opened only where the runtime executor reads nothing but the
/// frozen controller, the announced targets, the exact source and the
/// instruction's own operands, and where recovery re-derives the same facts
/// for that kind of stack object.
fn contexts(instruction: &Instruction) -> u8 {
    use Instruction as I;
    match instruction {
        I::OptionalDrawCards { .. } => ANY,
        // CR 119.3: positive life changes for the controller, each opponent
        // or one announced player.
        I::Kernel { node, .. } => kernel_contexts(node),
        // CR 120.2b: noncombat damage from the invocation's source, using its
        // last known information if it has left (CR 608.2h).
        I::DealDamage { .. } => ANY,
        // CR 701.8, 701.13, 701.26, 400.3: one announced permanent, with its
        // legality rechecked on resolution (CR 608.2b).
        I::DestroyPermanent { optional, .. } => {
            if *optional {
                FAMILY
            } else {
                ANY
            }
        }
        I::ExilePermanent { .. }
        | I::TapPermanent { .. }
        | I::UntapPermanent { .. }
        | I::ReturnToOwnerHand { .. }
        | I::PutOnTopOwnerLibrary { .. }
        | I::DestroyTargetCreature { .. } => ANY,
        // Target-derived quantities read the single announced target's
        // captured facts (CR 608.2h); `program` requires that sole target.
        I::ChangeLifeFromTarget { .. } | I::DamageTargetController { .. } => ANY,
        // CR 111.1-111.4, 111.10: recovery re-derives every token from the
        // creating recipe of its source's spell, activated or triggered program.
        I::CreateCreatureTokens { .. }
        | I::CreateTokensFor { .. }
        | I::CreatePredefinedTokens { .. } => ANY,
        // CR 122.1, 122.6: +1/+1 counters on an announced creature or on the
        // exact battlefield source.
        I::PutPlusOneCounters { on, .. } => match on {
            CounterRecipient::Target { .. } => ANY,
            CounterRecipient::Source => SOURCE,
        },
        // CR 611.2c, 613.4c, 514.2: one captured creature until cleanup.
        I::ModifyPowerToughness { .. } => ANY,
        // CR 508.1d, 509.1b: the restriction effect authenticates a spell or
        // an activated ability as its origin.
        I::GrantCreatureRestriction { .. } => SPELL | ACTIVATED,
        // CR 400.7, 113.7a: these act on the exact battlefield source.
        I::ModifySourcePowerToughness { .. } => SOURCE,
        // CR 701.19a: the shield authenticates an activated ability.
        I::RegenerateSource { .. } => ACTIVATED,
        // CR 701.22, 701.25, 701.17: the controller's own library prefix.
        I::Scry { .. }
        | I::Surveil { .. }
        | I::OrderLibraryTop { .. }
        | I::OptionalShuffleLibrary { .. }
        | I::SelectLibraryCard { .. }
        | I::PutHandOnLibrary { .. } => ANY,
        I::MillCards { recipient, .. } => match recipient {
            None | Some(LifeRecipient::Target { .. }) => ANY,
            _ => FAMILY,
        },
        // CR 701.9a: hand choices stay private to each discarding player.
        I::DiscardCards { recipient, .. } => match recipient {
            None => ANY,
            Some(_) => FAMILY,
        },
        I::DiscardPlayerSet { .. } | I::DiscardTargetPlayer { .. } => ANY,
        // CR 701.6: the countered object is the announced stack target.
        I::CounterStackObject { .. }
        | I::CounterSpellAndMillController { .. }
        | I::CounterSpellUnlessPays { .. } => ANY,
        // CR 608.2h: non-targeted sets are read when the instruction is performed.
        I::DestroyAllCreatures { .. }
        | I::PumpSet { .. }
        | I::UntapControllerLands { .. } => ANY,
        I::DamageSet { amount, .. } => match amount {
            WipeAmount::Fixed { .. } => ANY,
            WipeAmount::AnnouncedX
            | WipeAmount::CreatureCount
            | WipeAmount::OtherCastsManaValue => FAMILY,
        },
        // CR 701.21a: each named player chooses and sacrifices.
        I::EachOpponentSacrifice { .. }
        | I::EachPlayerSacrifice { .. }
        | I::TargetPlayerSacrificeCreature { .. } => ANY,
        // CR 400.7, 608.2b: an announced graveyard card or player's graveyard.
        I::ReturnOwnGraveyardCreatureToHand { optional, .. }
        | I::ReturnOwnGraveyardCardToHand { optional, .. } => {
            if *optional {
                FAMILY
            } else {
                ANY
            }
        }
        I::ReturnOwnGraveyardCreatureToBattlefield { .. } | I::ExileTargetGraveyard { .. } => ANY,
        // CR 701.23: the controller searches their own library.
        I::SearchBasicLands { .. }
        | I::SearchBasicLandsSplit { .. }
        | I::SearchTypedLands { .. }
        | I::SearchColoredLibrary { .. } => ANY,
        // CR 615.1: a turn-long combat damage prevention effect.
        I::PreventCombatDamage { .. } => ANY,
        // These effects authenticate a paid resolving spell.
        I::GrantLandPlaysThisTurn { .. }
        | I::PhaseOutControllerPermanents { .. }
        | I::PhaseOutTargetPermanent { .. }
        | I::ProtectControllerUntilNextTurn { .. }
        | I::FreezeControllerLifeUntilNextTurn { .. } => SPELL,
        // Everything below keeps its reviewed family: it needs trigger event
        // data, an announced X, a resolving card, a linked sibling instruction
        // or a whole-program shape that the tables here do not describe.
        I::LookTopMayRevealForMana { .. }
        | I::RegisterControllerCreatureEntryThisTurn { .. }
        | I::Hideaway { .. }
        | I::PlayHideawayLinked { .. }
        | I::GrantHandCastCascade { .. }
        | I::OfferHandCastWithoutManaCost { .. }
        | I::Cascade { .. }
        | I::RevealAndCast { .. }
        | I::Demonstrate { .. }
        | I::ShufflePlayerGraveyard { .. }
        | I::ExileResolvingSpell { .. }
        | I::RemoveOpponentNonlandIndestructible { .. }
        | I::GainLifeDestroyed { .. }
        | I::ExileDamagedThisTurn { .. }
        | I::LoseLifeDeathController { .. }
        | I::SacrificeDamage { .. }
        | I::AttachEquipment { .. }
        | I::ReturnAuraFromGraveyard { .. }
        | I::FetchBasicLand { .. }
        | I::ExileLibraryForPlay { .. }
        | I::AddResolutionMana { .. }
        | I::PutSourcePlusOneCountersForCastColors { .. }
        | I::DistributePlusOneCounters { .. }
        | I::GrantKeywordsSet { .. }
        | I::GrantSourceKeywords { .. }
        | I::GrantKeywords { .. }
        | I::PreventNextDamage { .. }
        | I::ReturnEntryTargetCreature { .. }
        | I::MoveTriggeredSuccessor { .. }
        | I::LoseGameTargetPlayer { .. }
        | I::SacrificeCreature { .. }
        | I::DrawCardsIfAdditionalCost { .. }
        | I::DealDamageAnnouncedX { .. }
        | I::MillAnnouncedX { .. }
        | I::OptionalDrawUnlessCasterPays { .. }
        | I::OptionalDrawForSoleGreatestEntrant { .. }
        | I::DrawDamageAmount { .. }
        | I::DiscardTargetPlayerAtRandom { .. }
        | I::CopyControlledCreatureSpell { .. }
        | I::DealDamageToController { .. }
        // Optional blocks, delayed cast registration, mana-tap riders, colour
        // changes, proliferate, recipient searches and transformation each
        // bind a reviewed event, sibling block or second face.
        | I::ChooseOptional { .. }
        | I::DealDamageToManaActivator { .. }
        | I::Proliferate { .. }
        | I::RegisterControllerSpellCastThisTurn { .. }
        | I::SearchRecipientBasicLand { .. }
        | I::SetTargetColor { .. }
        | I::TransformSource { .. } => FAMILY,
    }
}

/// The kernel row. `kir::check` decides scope, kinds and the audited cell for
/// the actual context; this row only keeps operands that read a reviewed
/// family's event data out of composition.
fn kernel_contexts(node: &kir::Node) -> u8 {
    // CR 119.3: the opponent drain pair; an announced X binds its family.
    if let Some(amount) = kir::drain(node) {
        return if amount.is_announced_x() { FAMILY } else { ANY };
    }
    match node.verb() {
        // CR 119.3: life changes for the controller, each opponent or one
        // announced player. An announced X binds its family.
        Some(Verb::GainLife { player, amount } | Verb::LoseLife { player, amount }) => {
            if player.is_event_player() || amount.is_announced_x() {
                FAMILY
            } else {
                ANY
            }
        }
        // CR 121.1-121.2: a fixed draw by the controller, or by every player;
        // one card for an announced player. An announced X, an event player
        // and the targeted counted draw bind their families.
        Some(Verb::Draw { player, amount }) => match (player, amount.as_fixed()) {
            (_, None) => FAMILY,
            (kir::PlayerExpr::EachPlayer, Some(_)) => ANY,
            (player, Some(_)) if player.is_you() => ANY,
            (player, Some(count))
                if player.target_slot().is_some() && *count == Quantity::from(1) =>
            {
                ANY
            }
            _ => FAMILY,
        },
        // CR 608.2h: a board-wide set is read as the instruction is performed.
        Some(
            Verb::Destroy {
                what: SetExpr::Matching { .. },
                ..
            }
            | Verb::Move {
                what: SetExpr::Matching { .. },
                ..
            },
        ) => ANY,
        _ => FAMILY,
    }
}

/// True when `instruction` may appear in a composed program of `context`.
pub fn admits_instruction(instruction: &Instruction, context: Context) -> bool {
    contexts(instruction)
        & match context {
            Context::Spell => SPELL,
            Context::Activated => ACTIVATED,
            Context::Triggered => TRIGGERED,
        }
        != 0
}

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
fn program(p: &SpellProgram, context: Context) -> Result<(), String> {
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
    if let Some(i) = p
        .instructions
        .iter()
        .find(|i| !admits_instruction(i, context))
    {
        let op = serde_json::to_value(i)
            .ok()
            .and_then(|v| v["op"].as_str().map(str::to_owned))
            .unwrap_or_default();
        return Err(format!(
            "composition matrix does not admit {op} in this context"
        ));
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
    // CR 608.2h: a quantity or recipient read from "that permanent" is bound
    // to the one announced target, so the whole invocation is removed when
    // that target is illegal instead of resolving with missing information.
    if p.targets.len() != 1
        && p.instructions.iter().any(|i| {
            matches!(
                i,
                Instruction::ChangeLifeFromTarget { .. }
                    | Instruction::DamageTargetController { .. }
                    | Instruction::CreateTokensFor {
                        recipient: EffectRecipient::TargetController { .. },
                        ..
                    }
            )
        })
    {
        return Err("target-derived instructions require exactly one target".into());
    }
    // Target slots, domains, filters and operand ranges use the one shared
    // per-instruction binder; no whole-program family shape is consulted.
    validate_program_bindings(p, false, false, &|_| {
        Err("composed programs are not modal".into())
    })
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
    program(&a.program, Context::Activated)
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
    program(&a.program, Context::Triggered)
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
    // Whole-card characteristics the composition tables do not describe stay
    // with their families: second faces, hybrid and X costs, snow,
    // attachments and Sagas.
    if d.modal_land.is_some()
        || d.transform_faces.is_some()
        || !c.hybrid_mana.is_empty()
        || d.x_mana_cost
        || c.snow
        || c.basic
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

/// Publish composed definitions in the executable schema.
///
/// `$defs/Definition` keeps every family shape rule and now excludes the
/// marker and its capability. A catalog entry marked `composed` is instead
/// checked against `$defs/ComposedDefinition`: the plain structural schema of
/// the Rust IR types (closed enums, required fields, no unknown fields),
/// without any family shape rule, plus the marker/capability equivalence.
/// The matrix itself is enforced by `validate`, as family semantics are.
pub(crate) fn extend_schema(schema: &mut serde_json::Value) {
    use serde_json::{Value, json};
    let cap = json!({"contains":{"const":Capability::ComposedAbilityV1}});
    let marked = json!({"required":["composed"],"properties":{"composed":{"const":true}}});
    schema["$defs"]["Definition"]
        .as_object_mut()
        .expect("definition schema")
        .entry("allOf")
        .or_insert_with(|| json!([]))
        .as_array_mut()
        .expect("definition allOf")
        .push(json!({"not":marked,"properties":{"capabilities":{"not":cap}}}));
    fn rename(value: &mut Value) {
        match value {
            Value::Object(object) => {
                if let Some(Value::String(reference)) = object.get_mut("$ref")
                    && let Some(name) = reference.strip_prefix("#/$defs/")
                {
                    *reference = format!("#/$defs/Composed{name}");
                }
                object.values_mut().for_each(rename);
            }
            Value::Array(items) => items.iter_mut().for_each(rename),
            _ => {}
        }
    }
    let plain = serde_json::to_value(schemars::schema_for!(Catalog)).expect("schema is JSON");
    for (name, definition) in plain["$defs"].as_object().expect("derived definitions") {
        let mut definition = definition.clone();
        rename(&mut definition);
        schema["$defs"][format!("Composed{name}")] = definition;
    }
    schema["$defs"]["ComposedDefinition"]
        .as_object_mut()
        .expect("composed definition schema")
        .entry("allOf")
        .or_insert_with(|| json!([]))
        .as_array_mut()
        .expect("composed definition allOf")
        .push(json!({"properties":{"capabilities":cap}}));
    let items = &mut schema["properties"]["definitions"]["items"];
    assert_eq!(*items, json!({"$ref":"#/$defs/Definition"}));
    *items = json!({
        "if":marked,
        "then":{"$ref":"#/$defs/ComposedDefinition"},
        "else":{"$ref":"#/$defs/Definition"}
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A complete creature definition from the frozen wipes catalog.
    fn bears() -> Definition {
        let mut d = Catalog::read(include_str!(
            "../../../tests/fixtures/expected-wipes-ir.json"
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
        Instruction::DealDamage {
            target_slot: 0,
            amount: 1.into(),
            source_block: 0,
        }
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
        forged.capabilities.insert(Capability::ComposedAbilityV1);
        assert!(forged.validate().is_err());
        // Without the marker the same ability is not a family shape.
        let mut unmarked = valid.clone();
        unmarked.composed = false;
        unmarked.capabilities = unmarked.requirements();
        assert!(unmarked.validate().is_err());

        // Instruction row: event-bound and announced-X operations stay out.
        for instruction in [
            Instruction::DrawDamageAmount { source_block: 0 },
            Instruction::act(
                Verb::Draw {
                    player: PlayerExpr::you(Context::Spell),
                    amount: kir::Amount::announced_x(),
                },
                0,
            ),
            Instruction::RevealAndCast { source_block: 0 },
        ] {
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
                        vec![Instruction::CounterStackObject {
                            target_slot: 0,
                            source_block: 0,
                        }],
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
                Instruction::CounterStackObject {
                    target_slot: 0,
                    source_block: 0
                }
            )
            .validate()
            .is_err()
        );

        // Event row: an event that carries family data is not composable.
        let mut equipped = trigger(TargetDomain::AnyDamageTarget, damage());
        equipped.triggered[0].condition = TriggerCondition::EquippedDies;
        equipped.capabilities = equipped.requirements();
        assert!(equipped.validate().is_err());

        // Shell: attachments, snow and hybrid cards keep their families.
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

        // Provenance is checked exactly as for a family definition.
        let mut edited = valid.clone();
        edited.provenance.abilities_json.push(' ');
        assert!(edited.validate().is_err());
    }

    #[test]
    fn schema_publishes_the_marker_capability_equivalence() {
        let schema = catalog_schema();
        // A catalog entry is a composed definition exactly when it is marked.
        let items = &schema["properties"]["definitions"]["items"];
        assert_eq!(items["if"]["properties"]["composed"]["const"], true);
        assert_eq!(items["then"]["$ref"], "#/$defs/ComposedDefinition");
        assert_eq!(items["else"]["$ref"], "#/$defs/Definition");
        // Family definitions exclude the marker and its capability.
        let family = schema["$defs"]["Definition"]["allOf"].as_array().unwrap();
        assert!(family.iter().any(|rule| {
            rule["not"]["properties"]["composed"]["const"] == true
                && rule["properties"]["capabilities"]["not"]["contains"]["const"]
                    == "composed_ability_v1"
        }));
        // Composed definitions keep the strict structural IR and require it.
        let composed = &schema["$defs"]["ComposedDefinition"];
        assert_eq!(composed["additionalProperties"], false);
        assert_eq!(composed["properties"]["composed"]["type"], "boolean");
        assert!(composed["allOf"].as_array().unwrap().iter().any(|rule| {
            rule["properties"]["capabilities"]["contains"]["const"] == "composed_ability_v1"
        }));
        assert_eq!(
            composed["properties"]["activated"]["items"]["$ref"],
            "#/$defs/ComposedActivatedAbility"
        );
        assert!(schema["$defs"]["ComposedInstruction"].is_object());
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
            modes: vec![],
        }
    }
    fn gain(context: Context) -> Instruction {
        Instruction::act(
            Verb::GainLife {
                player: PlayerExpr::you(context),
                amount: Amount::fixed(2),
            },
            0,
        )
    }

    #[test]
    fn instruction_rows_separate_portable_source_and_family_bound_operations() {
        for context in [Context::Spell, Context::Activated, Context::Triggered] {
            assert!(admits_instruction(&gain(context), context));
            assert!(!admits_instruction(
                &Instruction::DrawDamageAmount { source_block: 0 },
                context
            ));
            assert!(!admits_instruction(
                &Instruction::act(
                    Verb::GainLife {
                        player: PlayerExpr::event(kir::EventPlayerRole::Caster),
                        amount: Amount::fixed(1)
                    },
                    0
                ),
                context
            ));
        }
        let regenerate = Instruction::RegenerateSource { source_block: 0 };
        assert!(!admits_instruction(&regenerate, Context::Spell));
        assert!(admits_instruction(&regenerate, Context::Activated));
        assert!(!admits_instruction(&regenerate, Context::Triggered));
        let pump = Instruction::ModifySourcePowerToughness {
            power: 1.into(),
            toughness: 1.into(),
            duration: Duration::UntilEndOfTurn,
            source_block: 0,
        };
        assert!(!admits_instruction(&pump, Context::Spell));
        assert!(admits_instruction(&pump, Context::Activated));
        assert!(admits_instruction(&pump, Context::Triggered));
        // CR 121.2: the controller's fixed draws are portable, as the counted
        // controller draw always was; an announced player's draw is portable
        // only as a single draw, and an announced X binds its family.
        let draw = |player, count: i64| Instruction::draw(player, count.into(), 0);
        let you = PlayerExpr::you(Context::Triggered);
        assert!(admits_instruction(
            &draw(you.clone(), 1),
            Context::Triggered
        ));
        assert!(admits_instruction(&draw(you, 2), Context::Triggered));
        assert!(admits_instruction(
            &draw(PlayerExpr::target(0), 1),
            Context::Spell
        ));
        assert!(!admits_instruction(
            &draw(PlayerExpr::target(0), 2),
            Context::Spell
        ));
        assert!(!admits_instruction(
            &Instruction::act(
                Verb::Draw {
                    player: PlayerExpr::you(Context::Spell),
                    amount: Amount::announced_x()
                },
                0
            ),
            Context::Spell
        ));
    }

    #[test]
    fn programs_reject_modes_empty_bodies_family_instructions_and_unbound_targets() {
        assert!(
            super::program(
                &program(vec![gain(Context::Activated)], vec![]),
                Context::Activated
            )
            .is_ok()
        );
        assert!(super::program(&program(vec![], vec![]), Context::Activated).is_err());
        let mut modal = program(vec![], vec![]);
        modal.modes = vec![
            SpellMode {
                program: program(vec![gain(Context::Activated)], vec![]),
            },
            SpellMode {
                program: program(vec![gain(Context::Activated)], vec![]),
            },
        ];
        assert!(super::program(&modal, Context::Spell).is_err());
        let x = program(
            vec![Instruction::drain(
                Context::Spell,
                kir::Amount::announced_x(),
                0,
            )],
            vec![],
        );
        assert!(super::program(&x, Context::Spell).is_err());
        let unbound = program(
            vec![Instruction::DealDamage {
                target_slot: 0,
                amount: 1.into(),
                source_block: 0,
            }],
            vec![],
        );
        assert!(super::program(&unbound, Context::Triggered).is_err());
        let mut sorcery_speed = program(vec![gain(Context::Activated)], vec![]);
        sorcery_speed.timing = SpellTiming::MainPhase;
        assert!(super::program(&sorcery_speed, Context::Activated).is_ok());
        assert!(super::program(&sorcery_speed, Context::Spell).is_ok());
        assert!(super::program(&sorcery_speed, Context::Triggered).is_err());
    }
}
