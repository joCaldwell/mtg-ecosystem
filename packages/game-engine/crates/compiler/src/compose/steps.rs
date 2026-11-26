//! Complete AST shapes for predefined token creation (CR 111.10).
use super::*;
use serde_json::Value;

fn keys(value: &Value, expected: &[&str]) -> bool {
    value
        .as_object()
        .is_some_and(|o| o.len() == expected.len() && expected.iter().all(|k| o.contains_key(*k)))
}
/// `{"effect":"create-token","count":N,"token":{"predefined":K}}`: the
/// controller creates N predefined tokens.
fn predefined_tokens(step: &Value) -> Option<(u32, PredefinedToken)> {
    if !keys(step, &["effect", "count", "token"])
        || step["effect"] != "create-token"
        || !keys(&step["count"], &["amount", "value"])
        || step["count"]["amount"] != "fixed"
        || !keys(&step["token"], &["predefined"])
    {
        return None;
    }
    let count = u32::try_from(step["count"]["value"].as_u64()?).ok()?;
    let kind = match step["token"]["predefined"].as_str()? {
        "Treasure" => PredefinedToken::Treasure,
        "Gold" => PredefinedToken::Gold,
        "Food" => PredefinedToken::Food,
        "Clue" => PredefinedToken::Clue,
        _ => return None,
    };
    (count > 0).then_some((count, kind))
}

pub(super) fn claims(step: &Value) -> bool {
    predefined_tokens(step).is_some()
}

pub(super) fn lower(
    step: &Value,
    context: Context,
    source_block: u32,
) -> Result<Instruction, Failure> {
    let (count, kind) =
        predefined_tokens(step).ok_or_else(|| unsupported("compose.step", "no step table row"))?;
    Ok(Instruction::create_tokens(
        context,
        count,
        kir::TokenSpec::Predefined { kind },
        source_block,
    ))
}

/// CR 601.2c, 608.2b-c, 118.12: the declaration and later reference are
/// one occurrence; the intervening sacrifice is mandatory at resolution.
pub(super) fn sacrifice_return(
    effects: &[serde_json::Value],
    site: &control::Site<'_>,
    program: &mut SpellProgram,
    slots: &mut BTreeMap<String, u32>,
    source_block: u32,
) -> Result<bool, Failure> {
    use serde_json::json;
    let [declaration, sacrifice, movement] = effects else {
        return Ok(false);
    };
    let what = &declaration["steps"][0]["what"];
    let Some(count) = what["count"]["value"]
        .as_u64()
        .and_then(|n| u32::try_from(n).ok())
    else {
        return Ok(false);
    };
    let Some(selection) = what["selectionId"].as_str() else {
        return Ok(false);
    };
    if !(2..=7).contains(&count)
        || !program.targets.is_empty()
        || *declaration
            != json!({"steps":[{"effect":"declare-target","what":{
            "ref":"target","filter":{"types":["creature"],"cls":"card","zone":{"zone":"graveyard","owner":"your"}},
            "count":{"amount":"fixed","value":count},"selectionId":selection}}]})
        || *sacrifice
            != json!({"steps":[{"effect":"sacrifice","who":{"player":"you"},"what":{
            "ref":"filter","filter":{"types":["creature"]},"count":{"amount":"fixed","value":1}}}]})
        || *movement
            != json!({"steps":[{"effect":"move-zone","what":{"ref":"choice-result","number":"plural",
            "filter":{"cls":"card"},"which":"chosen","scope":"contextual"},"to":{"zone":"battlefield"},"tapped":true}],
            "condition":{"condition":"you-do"},"conditionKind":"if"})
        || site
            .resolution
            .target_slot_at(site.index, "/effects/0/steps/0/what")?
            != Some(0)
        || site
            .resolution
            .target_slot_at(site.index, "/effects/2/steps/0/what")?
            != Some(0)
    {
        return Ok(false);
    }
    for index in 0..count {
        let id = if index == 0 {
            selection.to_owned()
        } else {
            format!("{selection}/{index}")
        };
        bind_target(program, slots, &id, TargetDomain::OwnGraveyardCreatureCard)?;
    }
    program.instructions.push(Instruction::Kernel {
        node: kir::Node::SacrificeForTappedReturn { count },
        source_block,
    });
    Ok(true)
}

/// CR 115.1, 601.2c, 608.2b, 701.24c: the declaration contributes a
/// mandatory target at announcement. The shuffle uses that same target
/// and its owner at resolution, after the shared target legality check.
pub(super) fn declared_shuffle(
    effects: &[Value],
    site: &control::Site<'_>,
    program: &mut SpellProgram,
    slots: &mut BTreeMap<String, u32>,
    source_block: u32,
) -> Result<bool, Failure> {
    use serde_json::json;
    let [declaration, shuffle] = effects else {
        return Ok(false);
    };
    let what = &declaration["steps"][0]["what"];
    let step = "/effects/1/steps/0";
    let who = format!("{step}/who");
    let owner = format!("{step}/library/owner");
    if !program.targets.is_empty()
        || *declaration != json!({"steps":[{"effect":"declare-target","what":what}]})
        || *shuffle
            != json!({"steps":[{
                "effect":"shuffle-into-library",
                "who":{"player":"owner","of":{"ref":"it"}},
                "what":[{"ref":"it"}],
                "library":{"zone":"library","owner":"their"}
            }]})
        || site
            .resolution
            .target_slot_at(site.index, "/effects/0/steps/0/what")?
            != Some(0)
        || site
            .resolution
            .target_slot_at(site.index, &format!("{step}/what/0"))?
            != Some(0)
        || !site
            .resolution
            .binds_target_player_selection(site.index, &who, &owner, 0)?
    {
        return Ok(false);
    }
    let what: ast::ObjectRef = typed(what, "effects.declare-target")?;
    let slot = bind_permanent_target(program, slots, &what)?;
    if program.optional_target {
        return Err(unsupported(
            "compose.step",
            "declare-target shuffle requires a mandatory permanent target",
        ));
    }
    program.instructions.push(Instruction::act(
        kir::Verb::Move {
            what: kir::SetExpr::target(slot),
            to: kir::Destination::OwnerLibraryShuffled,
        },
        source_block,
    ));
    Ok(true)
}
