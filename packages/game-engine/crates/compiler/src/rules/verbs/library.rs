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
    let [
        Step::LookAt { who, what },
        Step::MoveZone {
            what: moved,
            to,
            order: Some(order),
            tapped: None,
            who: None,
        },
        ..,
    ] = steps
    else {
        return Err(reject());
    };
    if *who != json!({"player": "you"})
        || *moved != json!({"ref": "them"})
        || *to != json!({"zone": "library", "owner": "your", "position": "top"})
        || order != "any"
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
        if serde_json::to_value(who).ok().as_ref() != Some(&who_value) || !pronoun(what) {
            return Err(search_error("reveal"));
        }
        reveal = true;
        at += 1;
    }
    if let [
        shuffle @ Step::Shuffle { .. },
        movement @ Step::MoveZone { .. },
        ..,
    ] = &steps[at..]
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
                },
            })],
            at + 2,
        ));
    }
    let Some(Step::MoveZone {
        what,
        to,
        order,
        tapped,
        who: mover,
    }) = steps.get(at)
    else {
        return Err(search_error("found_cards"));
    };
    if order.is_some() || !pronoun(what) || mover.as_ref().is_some_and(|mover| *mover != who_value)
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
        },
    });
    let shuffle = cx.act(Verb::Shuffle { player });
    Ok((vec![search, shuffle], at + 1))
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
    matches!(movement, Step::MoveZone { what, to, order: None, tapped: None, who: None }
        if (*what == json!({"ref": "it"})
            || *what == json!({"ref": "that", "filter": {"cls": "card"}}))
        && *to == json!({"zone": "library", "owner": player, "position": "top"}))
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

/// "You", or "its controller": the last-known controller of the block's
/// sole announced target, removed by the preceding step (CR 608.2h).
fn searcher(cx: &mut StepCx<'_>, who: &PlayerSelector) -> Result<PlayerExpr, Failure> {
    match typed::<ast::PlayerRef>(who, "player")? {
        ast::PlayerRef::You(_) => Ok(PlayerExpr::you(cx.context)),
        ast::PlayerRef::Controller(controller) => Ok(PlayerExpr::ControllerOf {
            object: kir::Ref::Target {
                slot: preceding_target(cx.program, cx.source_block, &controller.of)?,
            },
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
fn pronoun(what: &Value) -> bool {
    matches!(
        typed::<ast::ObjectRef>(what, "object"),
        Ok(ast::ObjectRef::It(_) | ast::ObjectRef::That(_) | ast::ObjectRef::Them(_))
    )
}

/// CR 400.3, 110.2: the searcher's battlefield, or the hand or graveyard of
/// the found cards' owner (the searcher: it is their library).
fn destination(to: &Value, player: &PlayerExpr, tapped: bool) -> Result<kir::Destination, Failure> {
    let owner = if player.is_you() { "your" } else { "their" };
    if *to == json!({"zone": "battlefield"}) {
        return Ok(kir::Destination::Battlefield {
            controller: player.clone(),
            tapped,
        });
    }
    if tapped {
        return Err(search_error("destination"));
    }
    if *to == json!({"zone": "hand", "owner": owner}) {
        Ok(kir::Destination::OwnerHand)
    } else if *to == json!({"zone": "graveyard", "owner": owner}) {
        Ok(kir::Destination::OwnerGraveyard)
    } else {
        Err(search_error("destination"))
    }
}
