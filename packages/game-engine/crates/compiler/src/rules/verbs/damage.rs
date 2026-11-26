//! "Deals N damage" (CR 120).
use super::*;

/// CR 120.2b: the resolving object's source deals noncombat damage to one
/// announced "any target", or (CR 608.2h) to every creature of reviewed
/// board sets and optionally players.
/// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
pub(super) fn lower(
    cx: &mut StepCx<'_>,
    source: &DamageSource,
    amount: &serde_json::Value,
    to: &DamageDestination,
    optional: Option<bool>,
) -> Result<Instruction, Failure> {
    if optional.is_some() {
        return Err(step("damage", "optional"));
    }
    // The block validator has proved a first-step "it" refers to this
    // invocation's source. Other pronouns and named objects stay rejected.
    match source {
        DamageSource::SelfReference {} => {}
        DamageSource::It {} if cx.context != Context::Spell => {}
        DamageSource::This { noun }
            if cx.context != Context::Spell
                && matches!(
                    noun.as_str(),
                    "creature" | "artifact" | "enchantment" | "land" | "permanent"
                ) => {}
        DamageSource::It {} | DamageSource::This { .. } => return Err(step("damage", "source")),
    }
    let amount: ast::Amount = typed(amount, "amount")?;
    let refs = match to {
        DamageDestination::AnyTarget { selection_id } => {
            // Directed damage has previously reviewed tiers. The new source
            // forms enter through the separately audited set/player cells.
            if *source != (DamageSource::SelfReference {}) {
                return Err(step("damage", "source:directed"));
            }
            uncounted(cx)?;
            return Ok(Instruction::damage_target(
                bind_target(
                    cx.program,
                    cx.slots,
                    selection_id,
                    TargetDomain::AnyDamageTarget,
                )?,
                // CR 107.3a: a spell's X is the value announced for it;
                // definition admission scopes X to a spell whose cost has it.
                match amount {
                    ast::Amount::X(_) if cx.context == Context::Spell => kir::Amount::announced_x(),
                    _ => kir::Amount::Fixed {
                        value: fixed_positive_amount(&amount)?,
                    },
                },
                cx.source_block,
            ));
        }
        DamageDestination::Object { reference } => {
            if reference["ref"] == "target" {
                let what = object(reference)?;
                uncounted(cx)?;
                let amount = match amount {
                    ast::Amount::X(_) if cx.context == Context::Spell => kir::Amount::announced_x(),
                    _ => kir::Amount::Fixed {
                        value: fixed_positive_amount(&amount)?,
                    },
                };
                let slot = bind_damage_object_target(cx.program, cx.slots, &what)?;
                return Ok(Instruction::damage_target(slot, amount, cx.source_block));
            }
            std::slice::from_ref(reference)
        }
        DamageDestination::Each { refs } => refs.as_slice(),
        DamageDestination::Player { reference } => {
            let players = match typed::<ast::PlayerRef>(reference, "damage_recipient")? {
                ast::PlayerRef::You(_) => PlayerExpr::you(cx.context),
                ast::PlayerRef::EachPlayer(_) => PlayerExpr::EachPlayer,
                ast::PlayerRef::EachOpponent(_) => PlayerExpr::EachOpponent,
                ast::PlayerRef::ThatPlayer(_) => cx
                    .player_binding
                    .event_role()
                    .map(PlayerExpr::event)
                    .ok_or_else(|| operand("damage_recipient", "unbound_event_player"))?,
                other => return Err(operand("damage_recipient", variant(&other))),
            };
            let bound = cx.bound_x();
            let amount = lower_damage_amount(&amount, bound.as_ref(), cx.context)?;
            return Ok(Instruction::damage(
                kir::DamageTo::Players { players },
                amount,
                cx.source_block,
            ));
        }
    };
    let recipients: Vec<ast::AmountCounterCountOn> = typed(&refs, "damage_recipient")?;
    let mut sets = vec![];
    let mut each_player = false;
    for recipient in &recipients {
        match recipient {
            ast::AmountCounterCountOn::PlayerRef(player)
                if matches!(**player, ast::PlayerRef::EachPlayer(_)) && !each_player =>
            {
                each_player = true;
            }
            ast::AmountCounterCountOn::ObjectRef(set) => {
                sets.push(lower_board_set(set, cx.context)?);
            }
            ast::AmountCounterCountOn::PlayerRef(other) => {
                return Err(operand("damage_recipient", variant(&**other)));
            }
        }
    }
    let bound = cx.bound_x();
    let amount = lower_damage_amount(&amount, bound.as_ref(), cx.context)?;
    Ok(SetDamage::instruction(
        sets,
        each_player,
        amount,
        cx.source_block,
    ))
}
