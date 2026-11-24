//! "Deals N damage" (CR 120).
use super::*;

/// CR 120.2b: the resolving object's source deals noncombat damage to one
/// announced "any target", or (CR 608.2h) to every creature of reviewed
/// board sets and optionally each player.
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
    if *source != (DamageSource::SelfReference {}) {
        return Err(step("damage", "source"));
    }
    let amount: ast::Amount = typed(amount, "amount")?;
    let refs = match to {
        DamageDestination::AnyTarget { selection_id } => {
            uncounted(cx)?;
            return Ok(Instruction::damage_target(
                bind_target(
                    cx.program,
                    cx.slots,
                    selection_id,
                    TargetDomain::AnyDamageTarget,
                )?,
                kir::Amount::Fixed {
                    value: fixed_positive_amount(&amount)?,
                },
                cx.source_block,
            ));
        }
        DamageDestination::Object { reference } => std::slice::from_ref(reference),
        DamageDestination::Each { refs } => refs.as_slice(),
        DamageDestination::Player { .. } => {
            uncounted(cx)?;
            return Err(step("damage", "player_destination"));
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
                sets.push(lower_reviewed_set(set, cx.context)?);
            }
            ast::AmountCounterCountOn::PlayerRef(other) => {
                return Err(operand("damage_recipient", variant(&**other)));
            }
        }
    }
    let amount = lower_damage_amount(&amount, cx.counted, cx.context)?;
    Ok(SetDamage::instruction(
        sets,
        each_player,
        amount,
        cx.source_block,
    ))
}
