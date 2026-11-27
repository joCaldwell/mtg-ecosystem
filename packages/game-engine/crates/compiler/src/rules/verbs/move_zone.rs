//! Targeted battlefield entry and owner-zone moves (CR 400.3, 401.4, 614.1c).
use super::*;

/// Move the announced card or the source to the printed destination.
pub(super) fn lower(
    cx: &mut StepCx<'_>,
    what: &serde_json::Value,
    to: &serde_json::Value,
    order: Option<&str>,
    tapped: Option<bool>,
    who: Option<&serde_json::Value>,
    controller: Option<&str>,
) -> Result<Instruction, Failure> {
    uncounted(cx)?;
    // CR 110.2a and 614.1c: a permanent card from your graveyard enters
    // under your control, tapped when the move specifies it.
    // https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
    if *to == serde_json::json!({"zone": "battlefield"}) {
        if order.is_some()
            || who.is_some()
            || tapped == Some(false)
            || !matches!(controller, None | Some("you"))
        {
            return Err(step("move-zone", "battlefield_entry_modifier"));
        }
        let entry = Destination::Battlefield {
            controller: PlayerExpr::you(cx.context),
            tapped: tapped == Some(true),
        };
        if let Some(choice) = chosen(cx, &object(what)?, entry)? {
            return Ok(choice);
        }
        let slot = bind_graveyard_target(cx.program, cx.slots, &object(what)?, false)?;
        if controller.is_none()
            && matches!(
                cx.program.targets[slot as usize].domain,
                TargetDomain::GraveyardCard { .. }
            )
        {
            return Err(step("move-zone", "battlefield_entry_controller"));
        }
        return Ok(Instruction::move_target(
            slot,
            Destination::Battlefield {
                controller: PlayerExpr::you(cx.context),
                tapped: tapped == Some(true),
            },
            cx.source_block,
        ));
    }
    if tapped.is_some() || order.is_some() {
        return Err(step("move-zone", "entry_modifier"));
    }
    if who.is_some() || controller.is_some() {
        return Err(step("move-zone", "mover"));
    }
    let what = object(what)?;
    if matches!(&what, ast::ObjectRef::Target(t) if t.filter.zone.as_deref().is_some_and(|zone| zone.zone == ast::Zone::Graveyard))
    {
        // CR 400.3, 401.4: "your library" denotes the owner’s library
        // only when the target is restricted to your graveyard.
        let own_library =
            *to == serde_json::json!({"zone": "library", "owner": "your", "position": "top"});
        let destination = if own_library {
            Destination::OwnerLibraryTop
        } else {
            lower_destination(to)?
        };
        if destination != Destination::OwnerLibraryTop {
            return Err(step("move-zone", "graveyard_target_destination"));
        }
        let slot = bind_graveyard_target(cx.program, cx.slots, &what, own_library)?;
        return Ok(Instruction::move_target(slot, destination, cx.source_block));
    }
    let to = lower_destination(to)?;
    // CR 400.7: "this <permanent>" names the source's battlefield incarnation.
    if is_this_permanent(&what) && cx.context != Context::Spell {
        return Ok(cx.act(Verb::Move {
            what: SetExpr::source(),
            to,
        }));
    }
    // CR 400.3, 401.4: any legal permanent can move to the top. Keep
    // the existing plain-creature cell for the unqualified occurrence.
    let slot = if to == Destination::OwnerLibraryTop && is_creature_target(&what) {
        bind_creature_target(cx.program, cx.slots, &what)?
    } else {
        bind_permanent_target(cx.program, cx.slots, &what)?
    };
    Ok(Instruction::move_target(slot, to, cx.source_block))
}

/// CR 608.2c-d, 110.2: move cards of your hand or graveyard chosen as the
/// instruction resolves. `None` when `what` names no such cards.
pub(super) fn chosen(
    cx: &StepCx<'_>,
    what: &ast::ObjectRef,
    to: Destination,
) -> Result<Option<Instruction>, Failure> {
    let Some(ChosenCards {
        from,
        filter,
        count,
        up_to,
        random,
    }) = chosen_cards(what)?
    else {
        return Ok(None);
    };
    if from == kir::SelectionZone::Hand && !matches!(to, Destination::Battlefield { .. }) {
        return Err(operand("chosen_cards", "hand_destination"));
    }
    Ok(Some(cx.act(Verb::Search {
        player: PlayerExpr::you(cx.context),
        search: kir::SearchSpec {
            filter,
            maximum: Amount::fixed(i64::from(count)),
            to,
            reveal: false,
            from,
            optional: up_to,
            random,
        },
    })))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    /// CR 608.2c-d, 115.10: chosen cards come only from your own hand or
    /// graveyard by a fixed count; only a graveyard selection may be random.
    #[test]
    fn chosen_cards_reject_random_foreign_and_counted_selections() {
        let card = |zone: serde_json::Value, extra: serde_json::Value| {
            let mut what = json!({
                "ref": "filter",
                "filter": {"cls": "card", "types": ["creature"], "zone": zone},
                "count": {"amount": "fixed", "value": 1}
            });
            what.as_object_mut()
                .unwrap()
                .extend(extra.as_object().unwrap().clone());
            serde_json::from_value::<ast::ObjectRef>(what).unwrap()
        };
        let yours = json!({"zone": "graveyard", "owner": "your"});
        for (what, code) in [
            (
                card(
                    json!({"zone": "hand", "owner": "your"}),
                    json!({"random": true}),
                ),
                "operand:chosen_cards:random",
            ),
            (
                card(yours.clone(), json!({"random": true, "upTo": true})),
                "operand:chosen_cards:random",
            ),
            (
                card(json!({"zone": "graveyard", "owner": "any"}), json!({})),
                "operand:chosen_cards:zone",
            ),
            (
                card(yours.clone(), json!({"count": {"amount": "x"}})),
                "operand:chosen_cards:count",
            ),
        ] {
            assert_eq!(chosen_cards(&what).unwrap_err().reason, code);
        }
        let chosen = chosen_cards(&card(
            json!({"zone": "hand", "owner": "your"}),
            json!({"upTo": true, "count": {"amount": "fixed", "value": 2}}),
        ))
        .unwrap()
        .unwrap();
        assert_eq!(
            chosen,
            ChosenCards {
                from: kir::SelectionZone::Hand,
                filter: kir::Pred(vec![kir::Atom::Type {
                    kind: CardKind::Creature
                }]),
                count: 2,
                up_to: true,
                random: false,
            }
        );
    }

    #[test]
    fn foreign_graveyard_entry_requires_an_explicit_controller() {
        for owner in ["your", "any", "an-opponent"] {
            let mut program = SpellProgram::instructions(SpellTiming::MainPhase);
            let mut slots = BTreeMap::new();
            let mut cx = StepCx {
                program: &mut program,
                slots: &mut slots,
                player_binding: PlayerBinding::Unbound,
                event_is_source: false,
                event_types: 0,
                prior: Introduced::default(),
                pending: Introduced::default(),
                subject: None,
                context: Context::Spell,
                source_block: 0,
                x: None,
                x_consumed: false,
                result_from: None,
            };
            let target = json!({
                "ref": "target",
                "filter": {"cls": "card", "types": ["creature"], "zone": {"zone": "graveyard", "owner": owner}},
                "count": {"amount": "fixed", "value": 1},
                "selectionId": "graveyard-card"
            });
            let result = lower(
                &mut cx,
                &target,
                &json!({"zone": "battlefield"}),
                None,
                None,
                None,
                None,
            );
            if owner == "your" {
                assert!(result.is_ok(), "{result:?}");
            } else {
                assert_eq!(
                    result.unwrap_err().reason,
                    "verb:move-zone:battlefield_entry_controller"
                );
            }
        }
    }
}
