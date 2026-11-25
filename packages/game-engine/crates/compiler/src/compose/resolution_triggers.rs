//! R4's one-shot delayed and reflexive ability rows (CR 400.7j, 603.7, 603.12).
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;
use kir::{DelayedEvent, Node, Verb};
use serde_json::{Value, json};

fn reject(kind: &str) -> Failure {
    unsupported("compose.step", format!("create-{kind}-trigger shape outside the lowering table"))
}

fn fields(value: &Value, keys: &[&str]) -> bool {
    value.as_object().is_some_and(|object|
        object.len() == keys.len() && keys.iter().all(|key| object.contains_key(*key)))
}

fn sole_step(sentence: &Value) -> Option<&Value> {
    if fields(sentence, &["steps"]) && let [step] = sentence["steps"].as_array()?.as_slice() {
        Some(step)
    } else {
        None
    }
}

fn body(step: &Value) -> Option<&Value> {
    let [sentence] = step["effects"].as_array()?.as_slice() else {
        return None;
    };
    sole_step(sentence)?;
    Some(sentence)
}

pub(super) fn is_delayed(step: &Value) -> bool {
    step["effect"] == "create-delayed-trigger"
}

pub(super) fn delayed(
    step: &Value, parent: &mut SpellProgram, context: Context, source_block: u32,
) -> Result<Instruction, Failure> {
    if !fields(step, &["effect", "trigger", "occurrence", "nextOnly", "effects", "targets"])
        || step["occurrence"] != "next"
        || step["nextOnly"] != true
        || !matches!(step["targets"].as_str(), Some("enclosing-ability" | "delayed-ability"))
    {
        return Err(reject("delayed"));
    }
    let header = &step["trigger"];
    let mut event = if *header == json!({"trigger":"phase", "phase":"end-step", "whose":"each-player"}) {
        DelayedEvent::NextEndStep
    } else if *header == json!({"trigger":"phase-in-turn", "phase":"upkeep", "turn":{"turn":"next"}}) {
        DelayedEvent::NextTurnUpkeep
    } else if *header == json!({"trigger":"phase", "phase":"upkeep", "whose":"your"}) {
        DelayedEvent::NextUpkeep { of: kir::PlayerExpr::you(context) }
    } else {
        return Err(reject("delayed"));
    };
    let sentence = body(step).ok_or_else(|| reject("delayed"))?;
    let child = sole_step(sentence).expect("body is one step");
    let mut program = SpellProgram::instructions(SpellTiming::Instant);
    if child["effect"] == "move-zone" {
        // The resolution table proves both pronoun references name the
        // immediately preceding exile result. Replace that one instruction
        // with a combined move and registration to capture its actual successor.
        let Some(Instruction::Kernel {
            node: Node::Do(Verb::Move { what: kir::SetExpr::One { object: kir::Ref::Target { slot } }, to: kir::Destination::Exile }),
            source_block: exile_block,
        }) = parent.instructions.last() else {
            return Err(reject("delayed"));
        };
        if event != DelayedEvent::NextEndStep || *exile_block != source_block || !is_owner_return(child) {
            return Err(reject("delayed"));
        }
        event = DelayedEvent::NextEndStepAfterExile { slot: *slot };
        parent.instructions.pop();
        program.instructions.push(Instruction::Kernel {
            node: Node::Do(Verb::ReturnCapturedExile {}),
            source_block,
        });
    } else if let Some(count) = controller_random_discard(child) {
        program.instructions.push(Instruction::discard(
            kir::PlayerExpr::you(Context::Triggered), kir::Amount::fixed(count), true, source_block,
        ));
    } else {
        lower_body(&mut program, sentence, source_block, |typed| {
            validate_damage_sources(typed, DamageSourceBinding::Invocation { first_it_is_self: false })
        })?;
    }
    Ok(Instruction::Kernel { node: Node::Delayed { event, program: Box::new(program) }, source_block })
}

fn controller_random_discard(step: &Value) -> Option<i64> {
    let count = step.pointer("/what/count/value")?.as_i64()?;
    (*step == json!({"effect":"discard", "who":{"player":"you"}, "random":true,
        "what":{"ref":"filter","filter":{"cls":"card"},"count":{"amount":"fixed","value":count}}})
        && count > 0 && u32::try_from(count).is_ok()).then_some(count)
}

pub(crate) fn is_owner_return(step: &Value) -> bool {
    fields(step, &["effect", "what", "to", "controller"])
        && step["effect"] == "move-zone"
        && step["controller"] == "owner"
        && step["to"] == json!({"zone":"battlefield"})
        && (step["what"] == json!({"ref":"that", "filter":{"cls":"card"}})
            || step["what"] == json!({"ref":"it"})
            || step["what"] == json!({"ref":"related", "number":"singular", "filter":{"cls":"card"},
                "relation":{"relation":"event-result","event":"exile","scope":"contextual"}}))
}

pub(super) fn reflexive(
    effects: &[Value],
    site: &control::Site<'_>,
    line: &str,
) -> Result<Option<Part>, Failure> {
    if !effects.iter().any(|sentence| sentence["steps"].as_array().is_some_and(|steps|
        steps.iter().any(|step| step["effect"] == "create-reflexive-trigger")))
    {
        return Ok(None);
    }
    let [before, after] = effects else {
        return Err(reject("reflexive"));
    };
    let (Some(pay), Some(trigger)) = (sole_step(before), sole_step(after)) else {
        return Err(reject("reflexive"));
    };
    let [cost] = pay["costs"].as_array().ok_or_else(|| reject("reflexive"))?.as_slice() else {
        return Err(reject("reflexive"));
    };
    if !fields(pay, &["effect", "who", "costs", "optional"])
        || pay["effect"] != "pay-cost" || pay["who"] != json!({"player":"you"}) || pay["optional"] != true
        || !fields(cost, &["cost", "symbols"]) || cost["cost"] != "mana"
        || !fields(trigger, &["effect", "event", "effects"])
        || trigger["effect"] != "create-reflexive-trigger"
        || trigger["event"] != json!({"event":"preceding-action","who":{"player":"you"},"performed":true})
    {
        return Err(reject("reflexive"));
    }
    let symbols = cost["symbols"].as_array().ok_or_else(|| reject("reflexive"))?;
    let mut printed_cost = String::new();
    for symbol in symbols {
        use std::fmt::Write;
        write!(printed_cost, "{{{}}}", symbol.as_str().ok_or_else(|| reject("reflexive"))?)
            .expect("writing to String");
    }
    let segments = site.resolution.segments(site.index)?;
    if !certified_sentence(&segments, line, "/effects/0")
        .is_some_and(|text| text.eq_ignore_ascii_case(&format!("you may pay {printed_cost}.")))
        || !certified_sentence(&segments, line, "/effects/1").is_some_and(|text| text.starts_with("When you do, "))
    {
        return Err(reject("reflexive"));
    }
    let payment = ingest::mana_cost(&printed_cost)?;
    let sentence = body(trigger).ok_or_else(|| reject("reflexive"))?;
    Ok(Some(Part::Reflexive(payment, sentence.clone())))
}

pub(super) fn lower_reflexive(
    payment: ManaCost,
    sentence: &Value,
    condition: &TriggerCondition,
    source_block: u32,
) -> Result<Instruction, Failure> {
    let mut program = SpellProgram::instructions(SpellTiming::Instant);
    let mut sentence = sentence.clone();
    if let Some(step) = sentence.pointer_mut("/steps/0")
        && step["effect"] == "damage" && (step["source"] == json!({"ref":"it"})
            || step["source"]["ref"] == "this")
    {
        let original = parts(std::slice::from_ref(&sentence))?;
        let [Part::Typed(typed)] = original.as_slice() else { return Err(reject("reflexive")); };
        validate_trigger_damage_sources(condition, std::slice::from_ref(typed))?;
        // The exact source reference was checked by resolution and the
        // parent event above. Reuse the ordinary directed damage cell.
        sentence["steps"][0]["source"] = json!({"ref":"self"});
    }
    lower_body(&mut program, &sentence, source_block, |typed| validate_trigger_damage_sources(condition, typed))?;
    Ok(Instruction::Kernel {
        node: Node::Reflexive { payment, program: Box::new(program) },
        source_block,
    })
}

fn lower_body(
    program: &mut SpellProgram, sentence: &Value, block: u32,
    damage: impl Fn(&[Sentence]) -> Result<(), Failure>,
) -> Result<(), Failure> {
    append_effects(program, parts(std::slice::from_ref(sentence))?, block as usize,
        &mut BTreeMap::new(), Context::Triggered, damage)
}

pub(super) fn departure(
    effects: &[Value], site: &control::Site<'_>, line: &str,
) -> Result<Option<SpellProgram>, Failure> {
    let [before, after] = effects else { return Ok(None) };
    let Some(trigger) = sole_step(after) else { return Ok(None) };
    if trigger["effect"] != "create-delayed-trigger" || trigger["trigger"]["trigger"] != "leaves" {
        return Ok(None);
    }
    let [exile, target] = before["steps"].as_array().map(Vec::as_slice).unwrap_or_default() else {
        return Err(reject("delayed"));
    };
    if !fields(before, &["steps"])
        || *exile != json!({"effect":"exile", "what":{"ref":"it"}})
        || !fields(target, &["effect","what"]) || target["effect"] != "declare-target"
        || !fields(trigger, &["effect","trigger","occurrence","effects","targets"])
        || trigger["trigger"] != json!({"trigger":"leaves","what":{"ref":"that","filter":{"types":["creature"]}}})
        || trigger["occurrence"] != "next" || trigger["targets"] != "delayed-ability"
        || !body(trigger).and_then(sole_step).is_some_and(is_departure_return)
        || !certified_sentence(&site.resolution.segments(site.index)?, line, "/effects/1")
            .is_some_and(|text| text == "When that creature leaves the battlefield, return this card from exile to the battlefield under its owner's control.")
    {
        return Err(reject("delayed"));
    }
    let mut parent = SpellProgram::instructions(SpellTiming::Instant);
    let what: ast::ObjectRef = typed(&target["what"], "effects.declare-target")?;
    let slot = crate::operands::bind_permanent_target(&mut parent, &mut BTreeMap::new(), &what)?;
    let source_block = source_block(site.index)?;
    let mut program = SpellProgram::instructions(SpellTiming::Instant);
    program.instructions.push(Instruction::Kernel {
        node: Node::Do(Verb::ReturnCapturedExile {}),
        source_block,
    });
    parent.instructions.push(Instruction::Kernel {
        node: Node::Delayed {
            event: DelayedEvent::TargetLeavesAfterExilingSource { slot },
            program: Box::new(program),
        },
        source_block,
    });
    Ok(Some(parent))
}

pub(crate) fn is_departure_return(step: &Value) -> bool {
    *step == json!({
        "effect":"move-zone", "what":{"ref":"this","noun":"card"},
        "to":{"zone":"battlefield"},"controller":"owner","from":{"zone":"exile"}
    })
}

fn certified_sentence<'a>(
    segments: &[(Vec<String>, usize, usize)], line: &'a str, field: &str,
) -> Option<&'a str> {
    segments.iter().find_map(|(fields, start, end)| {
        (fields == &[field]).then(|| line.get(*start..*end)).flatten()
    })
}
