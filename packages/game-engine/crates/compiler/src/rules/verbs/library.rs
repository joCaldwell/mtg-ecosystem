//! "Search", "shuffle", "scry" and "surveil" (CR 701.22-701.25).
//!
//! A printed search is a run of steps: search the player's own library for
//! cards with a filter, optionally reveal them, put them into one zone, then
//! shuffle that library. This run lowers to a `Search` and a `Shuffle`.
//! When exactly one card can be found and the text puts it on top after
//! shuffling, one `Search` shuffles the other cards (CR 701.24b).
//! Other arrangements reject under `verb:search:<detail>`.
//!
//! Pinned Comprehensive Rules 2026-09-25:
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;
use serde_json::{Value, json};

fn search_error(detail: &str) -> Failure {
    step("search", detail)
}

/// CR 701.22a, 701.25a: "scry N" / "surveil N" by the controller.
pub(super) fn look(
    cx: &mut StepCx<'_>,
    amount: &Value,
    surveil: bool,
) -> Result<Instruction, Failure> {
    uncounted(cx)?;
    let effect = if surveil { "surveil" } else { "scry" };
    let amount = match typed::<ast::Amount>(amount, "amount")? {
        ast::Amount::Fixed(n) if n.value >= 1 && u32::try_from(n.value).is_ok() => {
            Amount::fixed(n.value)
        }
        ast::Amount::Fixed(_) => return Err(step(effect, "amount")),
        other => return Err(operand("amount", variant(&other))),
    };
    let player = PlayerExpr::you(cx.context);
    Ok(cx.act(if surveil {
        Verb::Surveil { player, amount }
    } else {
        Verb::Scry { player, amount }
    }))
}

/// CR 401.2, 608.2c, 701.20e: one private look at your library's
/// fixed-size prefix, followed immediately by returning that exact pool
/// to the top in any order. R4 binds "them" to this look.
pub(super) fn reorder_top(cx: &mut StepCx<'_>, steps: &[Step]) -> Result<Instruction, Failure> {
    uncounted(cx)?;
    let reject = || step("look-at", "unlowered");
    let [Step::LookAt { who, what }, movement, ..] = steps else {
        return Err(reject());
    };
    let Some(PlainMove {
        what: moved,
        to,
        order: Some(ast::EffectMoveZoneOrder::Any),
        tapped: None,
        controller: None,
        who: None,
    }) = movement.plain_move()
    else {
        return Err(reject());
    };
    if *who != json!({"player": "you"})
        || !ast_is(moved, json!({"ref": "them"}))
        || !ast_is(
            to,
            json!({"zone": "library", "owner": "your", "position": "top"}),
        )
    {
        return Err(reject());
    }
    let Some(top) = what.get("top") else {
        return Err(reject());
    };
    if *what != json!({"top": top, "of": {"zone": "library", "owner": "your"}}) {
        return Err(reject());
    }
    let amount = fixed_positive_amount(&typed::<ast::Amount>(top, "amount")?)?;
    Ok(cx.act(Verb::ReorderLibraryTop {
        player: PlayerExpr::you(cx.context),
        amount: Amount::Fixed { value: amount },
    }))
}

/// CR 401.2, 608.2c-d, 701.20e: the hand choice and the bottom order
/// partition exactly the preceding private look. R5's binding proves
/// the same pool and its remainder across both sentences.
pub(super) fn look_choose_hand(
    look: &Sentence,
    move_cards: &Sentence,
    context: Context,
) -> Result<Option<Verb>, Failure> {
    let [Step::LookAt { who, what }] = look.steps.as_slice() else {
        return Ok(None);
    };
    let [Step::Simultaneous { steps: members }] = move_cards.steps.as_slice() else {
        return Ok(None);
    };
    if [look, move_cards].into_iter().any(|sentence| {
        sentence.condition.is_some()
            || sentence.condition_kind.is_some()
            || !sentence.bindings.is_empty()
    }) || *who != json!({"player":"you"})
        || members
            != &[
                json!({
                    "effect": "move-zone",
                    "what": {"ref":"from-pool","pool":{"ref":"them"},"count":{"amount":"fixed","value":1}},
                    "to": {"zone":"hand","owner":"your"},
                }),
                json!({
                    "effect":"move-zone", "what":{"ref":"rest"},
                    "to":{"zone":"library","owner":"your","position":"bottom"},
                    "order":"any",
                }),
            ]
    {
        return Ok(None);
    }
    let Some(top) = what.get("top") else {
        return Ok(None);
    };
    if *what != json!({"top":top,"of":{"zone":"library","owner":"your"}}) {
        return Ok(None);
    }
    let amount = fixed_positive_amount(&typed::<ast::Amount>(top, "amount")?)?;
    Ok(Some(Verb::LookChooseHand {
        player: PlayerExpr::you(context),
        amount: Amount::Fixed { value: amount },
    }))
}

/// CR 701.23a, 608.2c: lower the search run starting at `steps[0]`.
/// Returns its instructions and the number of printed steps it consumed.
pub(super) fn search(
    cx: &mut StepCx<'_>,
    steps: &[Step],
) -> Result<(Vec<Instruction>, usize), Failure> {
    uncounted(cx)?;
    let Some(Step::Search { who, zones, find }) = steps.first() else {
        return Err(search_error("shape"));
    };
    let who_value = serde_json::to_value(who).map_err(|_| search_error("player"))?;
    let player = searcher(cx, who)?;
    let you = player.is_you();
    // CR 701.23a: exactly the searcher's own library.
    if *zones
        != json!({"selection": "all", "of": [{"zone": "library", "owner": if you { "your" } else { "their" }}]})
    {
        return Err(search_error("zones"));
    }
    let (filter, maximum, up_to) = found(find)?;
    // CR 701.23d: a search simply for a quantity must find that many.
    if up_to && filter.0.is_empty() {
        return Err(search_error("up_to_quantity"));
    }
    // The rest of the run: an optional reveal, one move, one shuffle.
    let mut at = 1;
    let mut reveal = false;
    if let Some(Step::Reveal { who, what }) = steps.get(at) {
        if serde_json::to_value(who).ok().as_ref() != Some(&who_value)
            || !typed::<ast::ObjectRef>(what, "object").is_ok_and(|what| pronoun(&what))
        {
            return Err(search_error("reveal"));
        }
        reveal = true;
        at += 1;
    }
    if let [shuffle @ Step::Shuffle { .. }, movement, ..] = &steps[at..]
        && movement
            .effect()
            .is_some_and(|e| matches!(e, ast::Effect::MoveZone(_)))
    {
        if !you || maximum != 1 || !top_after_shuffle(movement, &who_value) {
            return Err(search_error("found_cards"));
        }
        validate_search_shuffle(shuffle, &who_value)?;
        return Ok((
            vec![cx.act(Verb::Search {
                player,
                search: kir::SearchSpec {
                    filter,
                    maximum: Amount::fixed(1),
                    to: Destination::OwnerLibraryTop,
                    reveal,
                    from: kir::SelectionZone::Library,
                    optional: false,
                    random: false,
                },
            })],
            at + 2,
        ));
    }
    // CR 608.2c, d, f: reveal both before choosing the one that enters
    // tapped. Put the remainder into hand in the same movement event.
    if you
        && reveal
        && up_to
        && maximum == 2
        && matches!(cx.context, Context::Spell | Context::Activated)
        && matches!(steps.get(1), Some(Step::Reveal { what, .. })
            if *what == json!({"ref":"them"})
                || *what == json!({"ref":"that","filter":{"cls":"card"},"plural":true}))
        && let Some(Step::Simultaneous { steps: members }) = steps.get(at)
        && split_basic_lands(members)
        && matches!(
            &filter.0[..],
            [
                kir::Atom::Basic,
                kir::Atom::Type {
                    kind: CardKind::Land
                }
            ] | [
                kir::Atom::Type {
                    kind: CardKind::Land
                },
                kir::Atom::Basic
            ]
        )
    {
        validate_search_shuffle(
            steps.get(at + 1).ok_or_else(|| search_error("shuffle"))?,
            &who_value,
        )?;
        let search = cx.act(Verb::Search {
            player: player.clone(),
            search: kir::SearchSpec {
                filter,
                maximum: Amount::fixed(maximum),
                to: Destination::OneTappedRestOwnerHand,
                reveal,
                from: kir::SelectionZone::Library,
                optional: false,
                random: false,
            },
        });
        return Ok((vec![search, cx.act(Verb::Shuffle { player })], at + 2));
    }
    let Some(PlainMove {
        what,
        to,
        order,
        tapped,
        controller: None,
        who: mover,
    }) = steps.get(at).and_then(Step::plain_move)
    else {
        return Err(search_error("found_cards"));
    };
    if order.is_some()
        || !pronoun(what)
        || mover.is_some_and(|mover| !ast_is(mover, who_value.clone()))
    {
        return Err(search_error("found_cards"));
    }
    let to = destination(to, &player, tapped.unwrap_or(false))?;
    at += 1;
    validate_search_shuffle(
        steps.get(at).ok_or_else(|| search_error("shuffle"))?,
        &who_value,
    )?;
    let search = cx.act(Verb::Search {
        player: player.clone(),
        search: kir::SearchSpec {
            filter,
            maximum: Amount::fixed(maximum),
            to,
            reveal,
            from: kir::SelectionZone::Library,
            optional: false,
            random: false,
        },
    });
    let shuffle = cx.act(Verb::Shuffle { player });
    Ok((vec![search, shuffle], at + 1))
}

fn split_basic_lands(members: &[Value]) -> bool {
    members
        == [
            json!({
                "effect": "move-zone",
                "what": {"ref": "from-pool", "pool": {"ref": "them"},
                    "count": {"amount": "fixed", "value": 1}},
                "to": {"zone": "battlefield"},
                "tapped": true,
            }),
            json!({
                "effect": "move-zone",
                "what": {"ref": "rest"},
                "to": {"zone": "hand", "owner": "your"},
            }),
        ]
}

/// CR 701.24a: the mandatory shuffle of the library that was searched.
fn validate_search_shuffle(shuffle: &Step, player: &Value) -> Result<(), Failure> {
    if matches!(shuffle, Step::Shuffle { who, library, optional: None }
        if serde_json::to_value(who).ok().as_ref() == Some(player)
            && *library == json!({"zone": "library", "owner": player}))
    {
        Ok(())
    } else {
        Err(search_error("shuffle"))
    }
}

/// CR 701.24b: the found singleton stays outside the shuffle.
fn top_after_shuffle(movement: &Step, player: &Value) -> bool {
    matches!(movement.plain_move(), Some(PlainMove { what, to, order: None, tapped: None, controller: None, who: None })
        if (ast_is(what, json!({"ref": "it"}))
            || ast_is(what, json!({"ref": "that", "filter": {"cls": "card"}})))
        && ast_is(to, json!({"zone": "library", "owner": player, "position": "top"})))
}

/// CR 701.24a: "shuffle your library", outside a search run.
pub(super) fn shuffle(
    cx: &mut StepCx<'_>,
    who: &PlayerSelector,
    library: &Value,
    optional: Option<bool>,
) -> Result<Instruction, Failure> {
    uncounted(cx)?;
    let player = typed::<ast::PlayerRef>(who, "player")?;
    if !matches!(player, ast::PlayerRef::You(_))
        || optional.is_some()
        || *library != json!({"zone": "library", "owner": {"player": "you"}})
    {
        return Err(step("shuffle", "library"));
    }
    Ok(cx.act(Verb::Shuffle {
        player: PlayerExpr::you(cx.context),
    }))
}

/// CR 701.24c: shuffle this resolving spell, or the sole announced
/// permanent, into the physical object's owner's library.
pub(super) fn shuffle_into(
    cx: &mut StepCx<'_>,
    who: &PlayerSelector,
    what: &[Value],
    library: &Value,
) -> Result<Instruction, Failure> {
    uncounted(cx)?;
    let reject = || step("shuffle-into-library", "object_or_owner");
    if cx.context != Context::Spell {
        return Err(reject());
    }
    let [what] = what else {
        return Err(reject());
    };
    let who = typed::<ast::PlayerRef>(who, "player")?;
    let object = match who {
        ast::PlayerRef::You(_)
            if *what == json!({"ref":"this","noun":"spell"})
                && *library == json!({"zone":"library","owner":"its-owner"}) =>
        {
            SetExpr::source()
        }
        ast::PlayerRef::Owner(owner)
            if *what == json!({"ref":"it"})
                && *library == json!({"zone":"library","owner":"their"}) =>
        {
            let slot = bind_permanent_target(cx.program, cx.slots, &owner.of)?;
            if cx.program.optional_target {
                return Err(reject());
            }
            SetExpr::target(slot)
        }
        _ => return Err(reject()),
    };
    Ok(cx.act(Verb::Move {
        what: object,
        to: Destination::OwnerLibraryShuffled,
    }))
}

/// "You", or "its controller": the last-known controller of the announced
/// target the pronoun names (CR 608.2c, 608.2h).
fn searcher(cx: &mut StepCx<'_>, who: &PlayerSelector) -> Result<PlayerExpr, Failure> {
    match typed::<ast::PlayerRef>(who, "player")? {
        ast::PlayerRef::You(_) => Ok(PlayerExpr::you(cx.context)),
        ast::PlayerRef::Controller(controller) => Ok(PlayerExpr::ControllerOf {
            object: antecedent::target(cx, &controller.of)?,
        }),
        other => Err(operand("player", variant(&other))),
    }
}

/// The found cards: a filter, a fixed positive count and "up to".
fn found(find: &Value) -> Result<(kir::Pred, i64, bool), Failure> {
    let ast::ObjectRef::Filter(found) = typed::<ast::ObjectRef>(find, "object")? else {
        return Err(search_error("for"));
    };
    if found.group_constraints.is_some() || found.random.is_some() {
        return Err(search_error("for"));
    }
    let maximum = match *found.count {
        ast::Amount::Fixed(n) if n.value >= 1 && u32::try_from(n.value).is_ok() => n.value,
        ref other => return Err(operand("amount", variant(other))),
    };
    let up_to = match found.up_to {
        None => false,
        Some(true) => true,
        Some(false) => return Err(search_error("for")),
    };
    Ok((lower_card_filter(&found.filter)?, maximum, up_to))
}

/// A pronoun naming the found cards; the archive binding proves it names
/// this search's result (`resolution::step_producer`).
fn pronoun(what: &ast::ObjectRef) -> bool {
    matches!(
        what,
        ast::ObjectRef::It(_) | ast::ObjectRef::That(_) | ast::ObjectRef::Them(_)
    )
}

/// CR 400.3, 110.2: the searcher's battlefield, or the hand or graveyard of
/// the found cards' owner (the searcher: it is their library).
fn destination(
    to: &ast::ZoneRef,
    player: &PlayerExpr,
    tapped: bool,
) -> Result<kir::Destination, Failure> {
    let owner = if player.is_you() { "your" } else { "their" };
    if ast_is(to, json!({"zone": "battlefield"})) {
        return Ok(kir::Destination::Battlefield {
            controller: player.clone(),
            tapped,
        });
    }
    if tapped {
        return Err(search_error("destination"));
    }
    if ast_is(to, json!({"zone": "hand", "owner": owner})) {
        Ok(kir::Destination::OwnerHand)
    } else if ast_is(to, json!({"zone": "graveyard", "owner": owner})) {
        Ok(kir::Destination::OwnerGraveyard)
    } else {
        Err(search_error("destination"))
    }
}
