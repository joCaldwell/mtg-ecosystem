//! Event table: self events whose runtime condition does not depend on which
//! permanent type the source is. Every other event keeps the shared trigger
//! lowerer, so a row here never changes an already supported trigger.
//!
//! CR 603.6a: enters-the-battlefield abilities trigger when the permanent
//! enters; the condition names the object, not one of its card types.
//! CR 603.2: "a land you control enters" is read from the event's resulting
//! battlefield, including this source when it is the entering land.
//! CR 701.20a: "becomes tapped" is the source's own untapped-to-tapped change.
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;

pub(super) fn lower(card: &Card<'_>, trigger: &SourceTrigger) -> Option<EventPattern> {
    use kir::AttachmentKind::{Enchanted, Equipped};
    // CR 301.5, 303.4: the creature the source is attached to.
    enum Host {
        Attacks,
        Dies,
        CombatDamageToPlayer,
    }
    use Host::{Attacks, CombatDamageToPlayer, Dies};
    let host = match trigger {
        SourceTrigger::Attacks {
            what: SourceAttacker::Equipped { noun },
        } => Some((Equipped, Attacks, noun)),
        SourceTrigger::Attacks {
            what: SourceAttacker::Enchanted { noun },
        } => Some((Enchanted, Attacks, noun)),
        SourceTrigger::Dies {
            what: TriggerObject::Equipped { noun },
        } => Some((Equipped, Dies, noun)),
        SourceTrigger::Dies {
            what: TriggerObject::Enchanted { noun },
        } => Some((Enchanted, Dies, noun)),
        SourceTrigger::DealsDamage {
            what,
            combat: Some(true),
            to,
            grouping: None,
        } if to == "any" => match what {
            SourceDamageObject::Equipped { noun } => Some((Equipped, CombatDamageToPlayer, noun)),
            SourceDamageObject::Enchanted { noun } => Some((Enchanted, CombatDamageToPlayer, noun)),
            _ => None,
        },
        _ => None,
    };
    if let Some((attachment, event, noun)) = host {
        // The printed host noun must match what the source attaches as.
        if noun != "creature" || !card.attaches_as(attachment) {
            return None;
        }
        let object = Pred::attached_creature();
        return Some(match event {
            Attacks => EventPattern::Attacks { attacker: object },
            Dies => EventPattern::Dies { object },
            CombatDamageToPlayer => EventPattern::DealsDamage {
                source: object,
                combat: true,
                recipient: PlayerExpr::EachPlayer,
            },
        });
    }
    match trigger {
        SourceTrigger::Enters {
            what: TriggerObject::This { noun },
        } if card.is_self(noun)
            // The shared lowerer already owns these two spellings.
            && !matches!(noun.as_str(), "creature" | "enchantment") =>
        {
            Some(EventPattern::self_enters())
        }
        SourceTrigger::Enters {
            what: TriggerObject::Filter { filter, count },
        } if filter.types == ["land"]
            && filter.control.as_deref() == Some("you")
            && !filter.other
            && !filter.non_token
            && filter.non_subtypes.is_empty()
            && filter.properties.is_none()
            && fixed_one(count) =>
        {
            Some(EventPattern::controlled_land_enters())
        }
        SourceTrigger::BecomesTapped {
            what: TriggerObject::This { noun },
        } if card.is_self(noun) => Some(EventPattern::self_becomes_tapped()),
        _ => None,
    }
}

/// CR 120.2b, 122.6, 608.2h: a standalone instruction refers to the
/// permanent named by its entry or attack event.
/// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
pub(super) fn object_effect(
    card: &Card<'_>,
    trigger: &serde_json::Value,
    condition: Option<&serde_json::Value>,
    sentence: &Sentence,
    block: usize,
) -> Result<Option<TriggeredAbility>, Failure> {
    if !matches!(trigger["trigger"].as_str(), Some("enters" | "attacks"))
        || !sentence.bindings.is_empty()
        || sentence.condition.is_some()
        || sentence.condition_kind.is_some()
    {
        return Ok(None);
    }
    let [step] = sentence.steps.as_slice() else {
        return Ok(None);
    };
    let pronoun = serde_json::json!({"ref": "it"});
    if !matches!(
        step,
        Step::Damage {
            source: DamageSource::It {},
            optional: None,
            ..
        }
    ) && !matches!(step, Step::PutCounters { on, .. } if *on == pronoun)
        && !matches!(step, Step::Pump { what, .. } if *what == pronoun)
    {
        return Ok(None);
    }
    let reviewed = serde_json::from_value::<SourceTrigger>(trigger.clone())
        .ok()
        .and_then(|t| lower(card, &t));
    let pattern = match reviewed {
        Some(pattern) => pattern,
        None => match general(card, trigger)? {
            Some(pattern) => pattern,
            None => return Ok(None),
        },
    };
    let Some(role) = pattern.object_role() else {
        return Ok(None);
    };
    let source = Ref::EventObject { role };
    let mut program = SpellProgram::instructions(SpellTiming::Instant);
    let source_block = source_block(block)?;
    let scope = kir::SetExpr::One { object: source };
    let instruction = match step {
        Step::Damage { amount, to, .. } => {
            let mut slots = BTreeMap::new();
            let slot = match to {
                DamageDestination::AnyTarget { selection_id } => bind_target(
                    &mut program,
                    &mut slots,
                    selection_id,
                    TargetDomain::AnyDamageTarget,
                )?,
                DamageDestination::Object { reference } if reference["ref"] == "target" => {
                    bind_damage_object_target(&mut program, &mut slots, &object(reference)?)?
                }
                _ => return Ok(None),
            };
            let amount = if *amount
                == serde_json::json!({
                    "amount": "attribute", "of": pronoun, "attribute": "power"
                }) {
                kir::Amount::StatOf {
                    object: source,
                    stat: kir::Stat::Power,
                }
            } else {
                kir::Amount::fixed(fixed_positive_amount(&typed(amount, "amount")?)?)
            };
            Instruction::act(
                Verb::Damage {
                    source,
                    to: kir::DamageTo::Ref {
                        who: Ref::Target { slot },
                    },
                    amount,
                },
                source_block,
            )
        }
        Step::PutCounters { counter, count, .. } if counter.r#type == "+1/+1" => {
            Instruction::plus_one_counters(
                scope,
                fixed_positive_amount(&typed(count, "amount")?)?,
                source_block,
            )
        }
        // This identity is already the invocation's exact source (CR 400.7).
        // A general attacking-object continuous effect needs its own cell.
        Step::Pump {
            power,
            toughness,
            duration,
            for_each: None,
            ..
        } if pattern.object() == Some(&Pred::source()) => {
            until_end_of_turn(duration)?;
            Instruction::pump(
                kir::SetExpr::source(),
                signed_fixed(power)?,
                signed_fixed(toughness)?,
                source_block,
            )
        }
        _ => return Ok(None),
    };
    let condition = TriggerCondition::Event {
        intervening_if: condition
            .map(|raw| intervening(raw, &pattern))
            .transpose()?
            .map(|i| i.condition()),
        once_each_turn: false,
        your_turn: false,
        pattern,
    };
    program.instructions.push(instruction);
    Ok(Some(TriggeredAbility {
        modal: None,
        functioning_zone: FunctioningZone::Battlefield,
        condition,
        program,
    }))
}

/// The general event table: an enters, dies, put-into-a-graveyard-from-the-
/// battlefield or attacks event whose object is this permanent, its named
/// self, or one filtered object, and the same-kind alternatives of such
/// events ("this creature or another Elf you control"), or the source's
/// entry or attack/death. The object filter
/// lowers term by term to a [`Pred`]; a printed term with no atom rejects.
/// `None` leaves the trigger to the reviewed lowerers' own rejection.
///
/// CR 603.2, 603.6a: an entering object is matched on the resulting
/// battlefield. CR 700.4, 603.10a: "dies" and "is put into a graveyard from
/// the battlefield" are one event, matched on the prior battlefield.
/// CR 508.3a: an attacking creature is matched as it is declared.
/// CR 201.5: "this" and the card's name are the source; "other" excludes it.
/// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
pub(super) fn general(
    card: &Card<'_>,
    trigger: &serde_json::Value,
) -> Result<Option<EventPattern>, Failure> {
    #[derive(Clone, Copy, PartialEq)]
    enum Kind {
        Enters,
        Dies,
        Attacks,
        Leaves,
        LeavesWithoutDying,
        Blocks,
    }
    impl Kind {
        fn pattern(self, object: Pred) -> EventPattern {
            match self {
                Self::Enters => EventPattern::Enters { object },
                Self::Dies => EventPattern::Dies { object },
                Self::Attacks => EventPattern::Attacks { attacker: object },
                Self::Leaves => EventPattern::LeavesBattlefield {
                    object,
                    without_dying: false,
                },
                Self::LeavesWithoutDying => EventPattern::LeavesBattlefield {
                    object,
                    without_dying: true,
                },
                Self::Blocks => EventPattern::Blocks { blocker: object },
            }
        }
    }
    fn event(card: &Card<'_>, raw: &serde_json::Value) -> Result<Option<(Kind, Pred)>, Failure> {
        let Some(object) = raw.as_object() else {
            return Ok(None);
        };
        let (kind, extra): (Kind, &[(&str, serde_json::Value)]) =
            match object.get("trigger").and_then(|t| t.as_str()) {
                Some("enters") => (Kind::Enters, &[]),
                Some("dies") => (Kind::Dies, &[]),
                Some("attacks") => (Kind::Attacks, &[]),
                // CR 603.6c, 509.3a: an alternative of an any-of event; the
                // single events lower in their own rows below.
                Some("leaves") if object.get("withoutDying") == Some(&serde_json::json!(true)) => (
                    Kind::LeavesWithoutDying,
                    &[("withoutDying", serde_json::Value::Null)],
                ),
                Some("leaves") => (Kind::Leaves, &[]),
                Some("blocks") => (Kind::Blocks, &[]),
                Some("put-into-graveyard") => (
                    Kind::Dies,
                    &[
                        ("from", serde_json::Value::Null),
                        ("to", serde_json::Value::Null),
                    ],
                ),
                _ => return Ok(None),
            };
        // Every other printed field (a grouping, an origin, an occurrence
        // count, a turn window) is a qualifier this table does not lower.
        if object.keys().any(|k| {
            !matches!(k.as_str(), "trigger" | "what") && !extra.iter().any(|(f, _)| f == k)
        }) {
            return Ok(None);
        }
        if extra.iter().any(|(field, _)| *field == "from")
            && (object.get("from") != Some(&serde_json::json!("battlefield"))
                || object.get("to") != Some(&serde_json::json!({"zone":"graveyard","owner":"any"})))
        {
            return Ok(None);
        }
        let Some(what) = object.get("what") else {
            return Ok(None);
        };
        let what: ast::ObjectRef = typed(what, "trigger.what")?;
        Ok(Some((kind, subject(card, &what)?)))
    }
    if matches!(trigger["trigger"].as_str(), Some("discards" | "cycle")) {
        return discard_event(trigger);
    }
    // CR 702.29d: "cycles or discards" a card is one discard event, the
    // two alternatives naming the same player and card.
    if trigger["trigger"] == "any-of"
        && trigger.as_object().is_some_and(|fields| fields.len() == 2)
        && let Some([cycle, discard]) = trigger["events"].as_array().map(Vec::as_slice)
        && cycle["trigger"] == "cycle"
        && discard["trigger"] == "discards"
        && cycle.as_object().map(serde_json::Map::len)
            == discard.as_object().map(serde_json::Map::len)
        && cycle["who"] == discard["who"]
        && cycle["what"] == discard["what"]
    {
        return discard_event(discard);
    }
    // CR 603.2, 701.21a: one player's sacrifice of either of two objects is
    // one event, matched by whichever alternative the permanent satisfies.
    if trigger["trigger"] == "any-of"
        && trigger.as_object().is_some_and(|fields| fields.len() == 2)
        && let Some([a, b]) = trigger["events"].as_array().map(Vec::as_slice)
        && a["trigger"] == "sacrifices"
        && b["trigger"] == "sacrifices"
        && a["who"] == b["who"]
        && let (
            Some(EventPattern::Sacrifices {
                player,
                object: first,
            }),
            Some(EventPattern::Sacrifices { object: second, .. }),
        ) = (general(card, a)?, general(card, b)?)
    {
        return Ok(Some(EventPattern::Sacrifices {
            player,
            object: Pred(vec![Atom::AnyOf {
                alternatives: vec![first, second],
            }]),
        }));
    }
    // CR 119.9, 701.21a: a player event named by "you", "an opponent" or
    // "a player". A sacrificed permanent is matched on the battlefield
    // immediately before it moved (CR 603.10a).
    if let Some(kind @ ("gains-life" | "sacrifices")) = trigger["trigger"].as_str() {
        let Some(fields) = trigger.as_object() else {
            return Ok(None);
        };
        let allowed = if kind == "sacrifices" { 3 } else { 2 };
        if fields.len() != allowed || (kind == "sacrifices") != fields.contains_key("what") {
            return Ok(None);
        }
        let player = match &trigger["who"] {
            who if *who == serde_json::json!({"player": "you"}) => {
                PlayerExpr::you(Context::Triggered)
            }
            who if *who == serde_json::json!({"player": "an-opponent"}) => PlayerExpr::EachOpponent,
            who if *who == serde_json::json!({"player": "a-player"}) => PlayerExpr::EachPlayer,
            _ => return Ok(None),
        };
        return Ok(Some(match kind {
            "gains-life" => EventPattern::GainsLife { player, nth: None },
            _ => {
                let what = &trigger["what"];
                let object = if what.get("ref").is_some() {
                    subject(card, &typed(what, "trigger.what")?)?
                } else {
                    lower_event_filter(&typed(what, "trigger.what")?)?
                };
                EventPattern::Sacrifices { player, object }
            }
        }));
    }
    if let Some(pattern) = blocking_pair(card, trigger)? {
        return Ok(Some(pattern));
    }
    // CR 603.6c, 509.3a, 509.3c, 120.3: an object event whose only other
    // printed field is a combat-damage restriction or "or becomes blocked".
    if let Some(kind @ ("leaves" | "blocks" | "becomes-blocked" | "is-dealt-damage")) =
        trigger["trigger"].as_str()
    {
        let Some(fields) = trigger.as_object() else {
            return Ok(None);
        };
        let extra = match kind {
            "blocks" => "orBecomesBlocked",
            "is-dealt-damage" => "combat",
            "leaves" => "withoutDying",
            _ => "",
        };
        if fields
            .keys()
            .any(|k| !matches!(k.as_str(), "trigger" | "what") && k != extra)
            || fields
                .get(extra)
                .is_some_and(|v| *v != serde_json::json!(true))
        {
            return Ok(None);
        }
        let object = subject(card, &typed(&trigger["what"], "trigger.what")?)?;
        return Ok(Some(match kind {
            "leaves" => EventPattern::LeavesBattlefield {
                object,
                without_dying: fields.contains_key("withoutDying"),
            },
            "becomes-blocked" => EventPattern::BecomesBlocked { attacker: object },
            "is-dealt-damage" => EventPattern::IsDealtDamage {
                object,
                combat: fields.contains_key("combat"),
            },
            _ if fields.contains_key(extra) => EventPattern::AnyOf {
                first: Box::new(EventPattern::Blocks {
                    blocker: object.clone(),
                }),
                second: Box::new(EventPattern::BecomesBlocked { attacker: object }),
            },
            _ => EventPattern::Blocks { blocker: object },
        }));
    }
    if trigger["trigger"] == "becomes-target"
        && trigger.as_object().is_some_and(|fields| fields.len() == 3)
    {
        use kir::TargetingKind;
        let mut of = trigger["of"].clone();
        let Some(fields) = of.as_object_mut() else {
            return Ok(None);
        };
        let by = match fields.remove("control") {
            None => PlayerExpr::EachPlayer,
            Some(control) if control == "you" => PlayerExpr::you(Context::Triggered),
            Some(control) if control == "opponent" => PlayerExpr::EachOpponent,
            _ => return Ok(None),
        };
        let kind = if of == serde_json::json!({"cls":"spell"}) {
            TargetingKind::Spell
        } else if of == serde_json::json!({"cls":"ability"}) {
            TargetingKind::Ability
        } else if of == serde_json::json!({"allOf":[{"anyOf":[{"cls":"spell"},{"cls":"ability"}]}]})
        {
            TargetingKind::SpellOrAbility
        } else {
            return Ok(None);
        };
        let object = subject(card, &typed(&trigger["what"], "trigger.what")?)?;
        return Ok(Some(EventPattern::BecomesTarget { object, by, kind }));
    }
    // CR 603.2c: one ability with two disjoint events, a cast and an
    // entry. Neither branch shares an invocation operand with the other.
    if trigger["trigger"] == "any-of"
        && trigger.as_object().is_some_and(|fields| fields.len() == 2)
        && let Some([a, b]) = trigger["events"].as_array().map(Vec::as_slice)
    {
        let (entry, cast) = if a["trigger"] == "cast" {
            (b, a)
        } else {
            (a, b)
        };
        // CR 119.9: the source's entry or a player's life gain.
        let gain = if a["trigger"] == "gains-life" { a } else { b };
        if gain["trigger"] == "gains-life"
            && let Some((Kind::Enters, object)) = event(card, if gain == a { b } else { a })?
            && object == Pred::source()
            && let Some(EventPattern::GainsLife { player, nth: None }) = general(card, gain)?
        {
            return Ok(Some(EventPattern::AnyOf {
                first: Box::new(EventPattern::Enters { object }),
                second: Box::new(EventPattern::GainsLife { player, nth: None }),
            }));
        }
        if let (
            Some((Kind::Enters, object)),
            Ok(SourceTrigger::Cast {
                who,
                what,
                not_from,
            }),
        ) = (event(card, entry)?, serde_json::from_value(cast.clone()))
        {
            return Ok(Some(EventPattern::AnyOf {
                first: Box::new(EventPattern::Enters { object }),
                second: Box::new(cast_event(&who, &what, not_from.as_ref())?),
            }));
        }
    }
    if trigger["trigger"] == "deals-damage"
        && trigger["combat"] == true
        && trigger["to"] == "any"
        && trigger.as_object().is_some_and(|fields| fields.len() == 4)
        && trigger["what"]["ref"] == "filter"
    {
        let source = subject(card, &typed(&trigger["what"], "trigger.what")?)?;
        return Ok(Some(EventPattern::DealsDamage {
            source,
            combat: true,
            recipient: PlayerExpr::EachPlayer,
        }));
    }
    let mut parts = match trigger.get("trigger").and_then(|t| t.as_str()) {
        Some("any-of") => {
            let Some(events) = trigger.get("events").and_then(|e| e.as_array()) else {
                return Ok(None);
            };
            if trigger.as_object().is_some_and(|o| o.len() != 2) || events.len() < 2 {
                return Ok(None);
            }
            let mut parts = vec![];
            for e in events {
                let Some(part) = event(card, e)? else {
                    return Ok(None);
                };
                parts.push(part);
            }
            parts
        }
        _ => match event(card, trigger)? {
            Some(part) => vec![part],
            None => return Ok(None),
        },
    };
    let kind = parts[0].0;
    if parts.iter().any(|(k, _)| *k != kind) {
        let [(a, a_subject), (b, b_subject)] = parts.as_slice() else {
            return Ok(None);
        };
        if *a_subject != Pred::source() || *b_subject != Pred::source() {
            return Ok(None);
        }
        // CR 603.2c: the source's entry and its death, attack or departure,
        // or its attack and its block; no occurrence is both.
        let (first, other) = match (a, b) {
            (Kind::Enters, Kind::Dies | Kind::Attacks | Kind::Leaves) => (Kind::Enters, *b),
            (Kind::Dies | Kind::Attacks | Kind::Leaves, Kind::Enters) => (Kind::Enters, *a),
            (Kind::Attacks, Kind::Blocks) | (Kind::Blocks, Kind::Attacks) => {
                (Kind::Attacks, Kind::Blocks)
            }
            _ => return Ok(None),
        };
        return Ok(Some(EventPattern::AnyOf {
            first: Box::new(first.pattern(Pred::source())),
            second: Box::new(other.pattern(Pred::source())),
        }));
    }
    let object = if parts.len() == 1 {
        parts.remove(0).1
    } else {
        // CR 603.2: one occurrence triggers the ability once, whichever
        // alternative its object satisfies.
        Pred(vec![Atom::AnyOf {
            alternatives: parts.into_iter().map(|(_, p)| p).collect(),
        }])
    };
    Ok(Some(kind.pattern(object)))
}

/// CR 509.3b, 509.3d: "[this] blocks a [filtered] creature", "[this]
/// becomes blocked by a [filtered] creature", or both as alternatives:
/// once per blocking pair, each creature read as it was declared (CR
/// 509.3f). A "one or more" grouping counts the pairs together and is not
/// lowered here.
fn blocking_pair(
    card: &Card<'_>,
    trigger: &serde_json::Value,
) -> Result<Option<EventPattern>, Failure> {
    fn creature(other: &serde_json::Value) -> Result<Option<Pred>, Failure> {
        if other["ref"] != "filter"
            || other.as_object().map(serde_json::Map::len) != Some(3)
            || other["count"] != serde_json::json!({"amount": "fixed", "value": 1})
        {
            return Ok(None);
        }
        let mut pred = lower_event_filter(&typed(&other["filter"], "trigger.blocking")?)?;
        // CR 509.1a: only creatures block or attack.
        let kind = Atom::Type {
            kind: CardKind::Creature,
        };
        if !pred.0.contains(&kind) {
            pred.0.push(kind);
        }
        Ok(Some(pred))
    }
    let one = |event: &serde_json::Value| -> Result<Option<EventPattern>, Failure> {
        let (field, blocks) = match event["trigger"].as_str() {
            Some("blocks") => ("blocking", true),
            Some("becomes-blocked") => ("by", false),
            _ => return Ok(None),
        };
        let Some(fields) = event.as_object() else {
            return Ok(None);
        };
        if fields.len() != 3 || !fields.contains_key(field) || !fields.contains_key("what") {
            return Ok(None);
        }
        // CR 301.5, 303.4: the creature this Equipment or Aura is attached to.
        let host = [
            ("equipped", kir::AttachmentKind::Equipped),
            ("enchanted", kir::AttachmentKind::Enchanted),
        ]
        .into_iter()
        .find(|(name, _)| event["what"] == serde_json::json!({"ref": name, "noun": "creature"}));
        let source = match host {
            Some((_, attachment)) if card.attaches_as(attachment) => Pred::attached_creature(),
            Some(_) => return Ok(None),
            None => subject(card, &typed(&event["what"], "trigger.what")?)?,
        };
        let Some(other) = creature(&event[field])? else {
            return Ok(None);
        };
        Ok(Some(if blocks {
            EventPattern::BlocksCreature {
                blocker: source,
                attacker: other,
            }
        } else {
            EventPattern::BlockedByCreature {
                attacker: source,
                blocker: other,
            }
        }))
    };
    if trigger["trigger"] == "any-of"
        && trigger.as_object().map(serde_json::Map::len) == Some(2)
        && let Some([a, b]) = trigger["events"].as_array().map(Vec::as_slice)
    {
        return Ok(match (one(a)?, one(b)?) {
            (
                Some(first @ EventPattern::BlocksCreature { .. }),
                Some(second @ EventPattern::BlockedByCreature { .. }),
            ) => Some(EventPattern::AnyOf {
                first: Box::new(first),
                second: Box::new(second),
            }),
            _ => None,
        });
    }
    one(trigger)
}

/// A printed object filter, typed as the AST names it.
pub(super) fn typed_filter(filter: &serde_json::Value) -> Result<ast::ObjectFilter, Failure> {
    typed(filter, "trigger.what")
}

/// The event player "you", "an opponent" or "a player" (CR 109.5, 102.3).
fn event_player(who: &serde_json::Value) -> Option<PlayerExpr> {
    match who {
        who if *who == serde_json::json!({"player": "you"}) => {
            Some(PlayerExpr::you(Context::Triggered))
        }
        who if *who == serde_json::json!({"player": "an-opponent"}) => {
            Some(PlayerExpr::EachOpponent)
        }
        who if *who == serde_json::json!({"player": "a-player"}) => Some(PlayerExpr::EachPlayer),
        _ => None,
    }
}

/// CR 701.9a, 702.29a: "[player] discards [a filtered card]" or "[player]
/// cycles [a card]", one event per card. The card is matched where it was
/// put (CR 603.10); "another" excludes this permanent, which a battlefield
/// ability's discarded card never is. "This card" functions from the zone
/// the card was put into (CR 702.29c) and rejects by name, as do a cause,
/// a grouping and the remaining qualifiers.
fn discard_event(trigger: &serde_json::Value) -> Result<Option<EventPattern>, Failure> {
    let reject = |detail: &str| {
        Err(unsupported(
            "compose.block",
            format!("trigger:discard:{detail}"),
        ))
    };
    let Some(fields) = trigger.as_object() else {
        return Ok(None);
    };
    let cycled = trigger["trigger"] == "cycle";
    if let Some(field) = fields
        .keys()
        .find(|k| !matches!(k.as_str(), "trigger" | "who" | "what"))
    {
        return reject(field);
    }
    let Some(player) = event_player(&trigger["who"]) else {
        return reject("player");
    };
    let card = match fields.get("what") {
        None if !cycled => Pred::default(),
        // CR 702.29c: "when you discard (or cycle) this card" functions from
        // the graveyard the card was put into.
        Some(what)
            if (*what == serde_json::json!({"ref": "this", "noun": "card"})
                || *what == serde_json::json!({"ref": "self"}))
                && player.is_you() =>
        {
            Pred::source()
        }
        Some(what) if what["ref"] == "this" || what["ref"] == "self" => {
            return reject("source-card");
        }
        Some(what)
            if what["ref"] == "filter"
                && what.as_object().map(serde_json::Map::len) == Some(3)
                && what["count"] == serde_json::json!({"amount": "fixed", "value": 1}) =>
        {
            // CR 108.1: the printed "card" class names the discarded object
            // itself, here and in each alternative.
            fn strip(filter: &mut serde_json::Value) -> bool {
                let Some(fields) = filter.as_object_mut() else {
                    return false;
                };
                if fields.remove("cls") != Some(serde_json::json!("card")) {
                    return false;
                }
                match fields.get_mut("anyOf").and_then(|a| a.as_array_mut()) {
                    Some(children) => children.iter_mut().all(|child| {
                        child.get("cls").is_none()
                            || strip(child) && child.as_object().is_some_and(|c| !c.is_empty())
                    }),
                    None => true,
                }
            }
            let mut filter = what["filter"].clone();
            if !strip(&mut filter) {
                return reject("filter");
            }
            lower_event_filter(&typed(&filter, "trigger.what")?)?
        }
        _ => return reject("what"),
    };
    Ok(Some(EventPattern::Discards {
        player,
        card,
        cycled,
        nth: None,
    }))
}

/// The object an event names: the source (by "this" noun or by name) or one
/// filtered object.
fn subject(card: &Card<'_>, what: &ast::ObjectRef) -> Result<Pred, Failure> {
    match what {
        ast::ObjectRef::This(this)
            if card.is_self(&this.noun) || card.names_own_subtype(&this.noun) =>
        {
            Ok(Pred::source())
        }
        ast::ObjectRef::SelfObject(_) => Ok(Pred::source()),
        ast::ObjectRef::Filter(f)
            if f.up_to.is_none()
                && f.group_constraints.is_none()
                && f.random.is_none()
                && matches!(&*f.count, ast::Amount::Fixed(n) if n.value == 1) =>
        {
            lower_event_filter(&f.filter)
        }
        other => Err(unsupported(
            "trigger.what",
            format!("event object {}", variant(other)),
        )),
    }
}

/// Match complete frozen history clauses. Additional qualifiers are rejected
/// before evaluating any comparison (CR 603.4, 109.5).
fn turn_history(condition: &serde_json::Value) -> Option<kir::Intervening> {
    use kir::TurnFact::*;
    use serde_json::json;

    let fixed = |op, value| json!({"op": op, "value": {"amount": "fixed", "value": value}});
    let you = json!({"player": "you"});
    let comparison = condition
        .get("comparison")
        .cloned()
        .unwrap_or_else(|| fixed("ge", 1));
    let measure = |fact, comparison: &serde_json::Value| {
        let (cmp, value) = control::comparison(comparison).ok()?;
        Some(kir::Intervening::TurnHistory { fact, cmp, value })
    };
    let cast = |what| {
        json!({
            "condition": "cast-count", "during": "this-turn", "who": you,
            "what": what, "comparison": comparison
        })
    };
    if *condition == cast(json!({"cls": "spell"})) {
        return measure(SpellsCast, &comparison);
    }
    if *condition == cast(json!({"cls": "spell", "nonTypes": ["creature"]})) {
        return measure(NoncreatureSpellsCast, &comparison);
    }
    let mut no_cast = cast(json!({"cls": "spell"}));
    no_cast["comparison"] = fixed("ge", 1);
    if *condition == json!({"condition": "not", "of": no_cast}) {
        return measure(SpellsCast, &fixed("eq", 0));
    }
    no_cast["from"] = json!({"zone": "hand", "owner": "your"});
    if *condition == json!({"condition": "not", "of": no_cast}) {
        return measure(SpellsCastFromHand, &fixed("eq", 0));
    }
    if *condition == json!({"condition": "attacked", "during": "this-turn", "who": you}) {
        return measure(Attacked, &fixed("ge", 1));
    }
    if *condition
        == json!({
            "condition": "life-change", "change": "gained", "during": "this-turn",
            "who": you, "comparison": comparison
        })
    {
        return measure(LifeGained, &comparison);
    }
    if *condition
        == json!({
            "condition": "player-action-history", "action": {"action": "draw"},
            "during": "this-turn", "who": you, "comparison": comparison
        })
    {
        return measure(CardsDrawn, &comparison);
    }
    let change = |event, filter| {
        json!({
            "condition": "zone-change-history", "event": event, "during": "this-turn",
            "from": {"zone": "battlefield"},
            "subject": {"match": "filter", "filter": filter, "comparison": fixed("ge", 1)}
        })
    };
    let mut died = change("dies", json!({"types": ["creature"]}));
    died["to"] = json!({"zone": "graveyard"});
    if *condition == died {
        return measure(CreatureDied, &fixed("ge", 1));
    }
    died["controllerAtMove"] = you.clone();
    if *condition == died {
        return measure(CreatureDiedUnderYourControl, &fixed("ge", 1));
    }
    died.as_object_mut()?.remove("controllerAtMove");
    died["subject"]["comparison"] = fixed("eq", 0);
    if *condition == died {
        return measure(CreatureDied, &fixed("eq", 0));
    }
    let mut left = change("leaves-battlefield", json!({"cls": "permanent"}));
    left["controllerAtMove"] = you;
    if *condition == left {
        return measure(PermanentLeftUnderYourControl, &fixed("ge", 1));
    }
    None
}

/// CR 603.4, 109.5: "you" is the frozen ability controller. A graveyard
/// comparison reads only cards in that player's own graveyard (CR 404).
pub(super) fn intervening(
    condition: &serde_json::Value,
    pattern: &EventPattern,
) -> Result<kir::Intervening, Failure> {
    use kir::ControllerMeasure;
    if let Some(history) = turn_history(condition).filter(|history| history.admits(pattern)) {
        return Ok(history);
    }
    let shape = condition["condition"].as_str();
    let fields = condition.as_object().map(|o| o.len());
    let you = serde_json::json!({"player": "you"});
    if *condition
        == serde_json::json!({
            "condition": "control", "who": you,
            "what": {"ref": "commander", "owner": you, "count": {"amount": "fixed", "value": 1}}
        })
    {
        return Ok(kir::Intervening::ControlsOwnCommander);
    }
    for (printed, kind) in [("land", CardKind::Land), ("creature", CardKind::Creature)] {
        if *condition
            == serde_json::json!({
                "condition": "control-count", "who": {"player": "an-opponent"},
                "of": {"types": [printed]}, "comparison": {
                    "op": "gt", "value": {"amount": "count", "of": {"types": [printed], "control": "you"}}
                }
            })
        {
            return Ok(kir::Intervening::OpponentControlsMore { kind });
        }
    }
    let measure = match shape {
        Some("life-total" | "cards-in-hand") if fields == Some(3) && condition["who"] == you => {
            if shape == Some("life-total") {
                ControllerMeasure::Life
            } else {
                ControllerMeasure::HandCount
            }
        }
        Some("count") if fields == Some(3) => {
            let of = &condition["of"];
            let zone = &of["zone"];
            if of["cls"] != "card"
                || zone.as_object().map(|o| o.len()) != Some(2)
                || zone["zone"] != "graveyard"
                || !(zone["owner"] == "your" || zone["owner"] == you)
            {
                return Err(unsupported_intervening());
            }
            let filter = match of.as_object().map(|o| o.len()) {
                Some(2) => Pred::default(),
                Some(3) if of["permanent"] == true => ControllerMeasure::permanent_cards(),
                Some(3) if let Some([kind]) = of["types"].as_array().map(Vec::as_slice) => {
                    let kind = kind.as_str().ok_or_else(unsupported_intervening)?;
                    Pred(vec![Atom::Type {
                        kind: card_kind(kind, "abilities.condition")?,
                    }])
                }
                _ => return Err(unsupported_intervening()),
            };
            ControllerMeasure::GraveyardCount(filter)
        }
        _ => return board_intervening(condition),
    };
    let (cmp, value) =
        control::comparison(&condition["comparison"]).map_err(|_| unsupported_intervening())?;
    let candidate = kir::Intervening::ControllerComparison {
        measure,
        cmp,
        value,
    };
    kir::Intervening::of(&candidate.condition())
        .ok()
        .filter(|intervening| intervening.admits(pattern))
        .ok_or_else(unsupported_intervening)
}

fn unsupported_intervening() -> Failure {
    unsupported(
        "abilities.condition",
        "unsupported complete intervening-if condition",
    )
}

/// CR 603.4, 109.5: an intervening "if you control a [filter]" or "if you
/// control N or more [filter]" clause, read when the event occurs and again
/// as the ability resolves. The filter may say "other" (CR 201.5).
fn board_intervening(condition: &serde_json::Value) -> Result<kir::Intervening, Failure> {
    let reject = unsupported_intervening;
    let fields = condition.as_object().map(|o| o.len()).unwrap_or(0);
    if condition.get("who") != Some(&serde_json::json!({"player": "you"})) {
        return Err(reject());
    }
    let (filter, cmp, value) = match condition.get("condition").and_then(|c| c.as_str()) {
        Some("control") if fields == 3 => {
            let what = &condition["what"];
            if what.as_object().map(|o| o.len()) != Some(3)
                || what["ref"] != "filter"
                || what["count"] != serde_json::json!({"amount": "fixed", "value": 1})
            {
                return Err(reject());
            }
            (&what["filter"], Cmp::AtLeast, Quantity::from(1))
        }
        Some("control-count") if fields == 4 => {
            let (cmp, value) = control::comparison(&condition["comparison"])?;
            (&condition["of"], cmp, value)
        }
        _ => return Err(reject()),
    };
    if filter.get("control").is_some() || filter.get("zone").is_some() {
        return Err(reject());
    }
    let filter: ast::ObjectFilter = typed(filter, "filter")?;
    let mut pred = lower_event_filter(&filter)?;
    pred.0.push(Atom::ControlledBy {
        player: PlayerExpr::you(Context::Triggered),
    });
    let intervening = kir::Intervening::BattlefieldCount {
        filter: pred,
        cmp,
        value,
    };
    // The kernel form must name exactly this clause.
    match kir::Intervening::of(&intervening.condition()) {
        Ok(found) if found == intervening => Ok(intervening),
        _ => Err(reject()),
    }
}
