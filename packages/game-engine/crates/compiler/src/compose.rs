//! Compositional lowering: one ability block at a time.
//!
//! The only lowering pipeline: every printed block is lowered independently
//! from its complete typed AST by small tables (`costs`, `events`,
//! `keywords`) plus the shared step lowerer, and the resulting definition is
//! admitted only by `mtg_contracts::composition`.
//!
//! Invariants: the exact archived source, AST, binding and resolution hashes; the normalized line/aggregate mirror; complete typed
//! decoding with unknown fields rejected; and an explicit rejection whenever a
//! clause, operand or reference is not consumed. Nothing here reads a card
//! name, an Oracle identity or a profile.
//!
//! CR 113.1-113.3, 602.1, 603.1, 608.2 (pinned 2026-09-25).
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;

mod activation_restrictions;
#[cfg(test)]
mod antecedent_tests;
pub(crate) mod control;
mod costs;
mod events;
pub(crate) mod keywords;
mod modal;
mod printed;
pub(super) mod resolution_triggers;
pub(super) mod revealed;
mod sagas;
mod source_action;
mod source_authority;
mod steps;
#[cfg(test)]
mod tests;

/// Facts about the printed card that block tables may consult. A table never
/// sees sibling blocks, so a row cannot depend on the rest of the card.
pub(super) struct Card<'a> {
    pub kind: CardKind,
    pub additional_types: &'a [CardKind],
    pub subtypes: &'a [String],
}
impl Card<'_> {
    /// CR 301.5, 303.4: an Equipment is attached as equipment and an Aura as
    /// an enchantment; the reviewed host rows name a sole subtype.
    pub fn attaches_as(&self, kind: kir::AttachmentKind) -> bool {
        self.subtypes
            == [match kind {
                kir::AttachmentKind::Equipped => "Equipment",
                kir::AttachmentKind::Enchanted => "Aura",
            }]
    }
    fn has_type(&self, kind: CardKind) -> bool {
        self.kind == kind || self.additional_types.contains(&kind)
    }
    /// CR 201.5: "this Aura", "this Equipment" name the source by the
    /// subtype it has.
    pub fn names_own_subtype(&self, noun: &str) -> bool {
        let subtype = match noun {
            "aura" => "Aura",
            "equipment" => "Equipment",
            _ => return false,
        };
        self.subtypes.iter().any(|s| s == subtype)
    }
    /// CR 201.5: after archive-bound canonicalization a self reference is
    /// `{"ref":"this","noun":N}`. The printed noun must name one of the
    /// card's own types, or the reference is not to this object.
    pub fn is_self(&self, noun: &str) -> bool {
        match noun {
            "creature" => self.has_type(CardKind::Creature),
            "artifact" => self.has_type(CardKind::Artifact),
            "enchantment" => self.has_type(CardKind::Enchantment),
            "land" => self.has_type(CardKind::Land),
            "planeswalker" => self.has_type(CardKind::Planeswalker),
            "permanent" => !matches!(self.kind, CardKind::Instant | CardKind::Sorcery),
            _ => false,
        }
    }
}

/// CR 207.2c: an ability word has no rules meaning, but it is printed. An
/// empty annotation, or one the block does not open with, is an archive
/// defect rather than something to ignore.
fn ability_word(word: &Option<String>, line: &str) -> Result<(), Failure> {
    if let Some(word) = word
        && (word.trim().is_empty() || !line.starts_with(&format!("{word} \u{2014} ")))
    {
        return Err(unsupported(
            "abilities.abilityWord",
            "empty or unprinted annotation",
        ));
    }
    Ok(())
}

/// The block mirror: exactly the shared ability fields, with sentences and
/// the activation restriction kept raw so the step and timing tables can
/// consume shapes outside the shared typed mirror. Unknown kinds and unknown
/// fields reject.
#[derive(Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
enum Block {
    Keywords {
        #[serde(rename = "abilityWord")]
        ability_word: Option<String>,
        keywords: Vec<SourceKeyword>,
    },
    Static {
        #[serde(rename = "abilityWord")]
        ability_word: Option<String>,
        effect: serde_json::Value,
    },
    Triggered {
        #[serde(rename = "abilityWord")]
        ability_word: Option<String>,
        condition: Option<serde_json::Value>,
        trigger: Box<serde_json::Value>,
        effects: Vec<serde_json::Value>,
        /// "This ability triggers only once each turn."
        limit: Option<serde_json::Value>,
    },
    Chapter {
        chapters: Vec<i64>,
        #[serde(rename = "abilityWord")]
        ability_word: Option<String>,
        effects: Vec<serde_json::Value>,
    },
    Spell {
        #[serde(rename = "abilityWord")]
        ability_word: Option<String>,
        effects: Vec<serde_json::Value>,
    },
    Loyalty {
        cost: LoyaltyCost,
        effects: Vec<serde_json::Value>,
    },
    Activated {
        restriction: Option<serde_json::Value>,
        #[serde(rename = "abilityWord")]
        ability_word: Option<String>,
        costs: Vec<SourceCost>,
        effects: Vec<serde_json::Value>,
        /// CR 601.2f, 602.2b: "This ability costs ... less to activate".
        #[serde(rename = "costModifications")]
        cost_modifications: Option<Vec<serde_json::Value>>,
    },
    /// CR 601.2b, 601.2f-h: "As an additional cost to cast this spell, ...".
    #[serde(rename = "additional-cost")]
    AdditionalCost {
        #[serde(rename = "abilityWord")]
        ability_word: Option<String>,
        costs: Vec<serde_json::Value>,
        optional: Option<serde_json::Value>,
        #[serde(rename = "costModifications")]
        cost_modifications: Option<serde_json::Value>,
        #[serde(rename = "variableConstraints")]
        variable_constraints: Option<serde_json::Value>,
    },
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct LoyaltyCost {
    sign: i64,
    amount: i64,
}

/// One printed sentence: either the shared typed mirror or step-table rows.
#[derive(Clone)]
enum Part {
    Typed(Sentence),
    Table(Vec<serde_json::Value>),
    Revealed(Vec<serde_json::Value>),
    Reflexive(ManaCost, serde_json::Value),
    Departure(SpellProgram),
}

/// CR 113.10a, 605.1a: a quoted ability a static grants. The admitted form
/// is a complete mana ability with no activation instructions; its "you"
/// and its tap cost name the recipient (the archive binds them to the
/// quoted ability's own scope), so no card-specific source predicate
/// applies. Any other quoted form rejects by name.
fn quoted_ability(
    raw: &serde_json::Value,
    block: usize,
    kind: CardKind,
) -> Result<QuotedAbility, Failure> {
    let quoted = || unsupported("static", "static:modifier:grant:quoted");
    let Ok(Block::Activated {
        restriction: None,
        ability_word: None,
        costs,
        effects,
        cost_modifications: None,
    }) = serde_json::from_value::<Block>(raw.clone())
    else {
        return Err(quoted());
    };
    if modal::declared(&effects) {
        return Err(quoted());
    }
    let peeled = control::peel(&effects)?;
    if !peeled.is_plain() {
        return Err(quoted());
    }
    let typed: Vec<Sentence> = parts(&peeled.sentences)?
        .into_iter()
        .map(|p| match p {
            Part::Typed(s) => Ok(s),
            Part::Table(_) | Part::Revealed(_) | Part::Reflexive(..) | Part::Departure(_) => {
                Err(quoted())
            }
        })
        .collect::<Result<_, _>>()?;
    let mana =
        lower_mana_ability(&costs, &typed, None, block, kind, &|_| false)?.ok_or_else(quoted)?;
    Ok(QuotedAbility::Mana(mana))
}

fn parts(effects: &[serde_json::Value]) -> Result<Vec<Part>, Failure> {
    if revealed::candidate(effects) {
        return Ok(vec![Part::Revealed(effects.to_vec())]);
    }
    effects
        .iter()
        .map(|sentence| {
            // A table sentence carries nothing but its steps: no condition,
            // no bindings, and every step claimed by a row.
            if let Some(object) = sentence.as_object()
                && object.len() == 1
                && let Some(steps) = object.get("steps").and_then(|s| s.as_array())
                && !steps.is_empty()
                && steps
                    .iter()
                    .all(|step| steps::claims(step) || resolution_triggers::is_delayed(step))
            {
                return Ok(Part::Table(steps.clone()));
            }
            decode_sentence(sentence).map(Part::Typed)
        })
        .collect()
}

/// Decode one effect sentence; evaluated actions (including discard and mill)
/// decode through the generated effect schema in `Step::deserialize`.
pub(crate) fn decode_sentence(raw: &serde_json::Value) -> Result<Sentence, Failure> {
    serde_json::from_value(control::counted_instruction(raw)).map_err(|e| mirror_gap(raw, &e))
}

/// A reviewed trigger lowers typed sentences only. A sentence of step table
/// rows (predefined tokens) is also a typed sentence the shared step
/// lowerer reads (CR 111.10); any other part needs a table event.
fn reviewed_sentence(part: Part) -> Result<Sentence, Failure> {
    let table = || unsupported("compose.step", "step table rows require a table event");
    match part {
        Part::Typed(sentence) => Ok(sentence),
        Part::Table(steps) => {
            serde_json::from_value(serde_json::json!({ "steps": steps })).map_err(|_| table())
        }
        Part::Revealed(_) | Part::Reflexive(..) | Part::Departure(_) => Err(table()),
    }
}

/// The complete typed AST (`crate::ast`) has already decoded this sentence;
/// the lowerer's narrower mirror does not hold one of its shapes. Name the
/// step verb (or the sentence) instead of an untagged-enum serde message.
fn mirror_gap(sentence: &serde_json::Value, error: &serde_json::Error) -> Failure {
    if !opaque(error) {
        return unsupported("compose.step", error.to_string());
    }
    let step = sentence["steps"]
        .as_array()
        .into_iter()
        .flatten()
        .find(|step| serde_json::from_value::<Step>((*step).clone()).is_err());
    match step.and_then(|s| s["effect"].as_str()) {
        Some(effect) => unsupported(
            "compose.step",
            format!("{effect} shape outside the lowering mirror"),
        ),
        None => unsupported("compose.step", "sentence shape outside the lowering mirror"),
    }
}

/// A serde failure that names no shape: an untagged-enum mismatch, a field
/// the mirror requires in another variant, or an operand the mirror types
/// differently (a map where it reads a string, a string where it reads a
/// counter).
fn opaque(error: &serde_json::Error) -> bool {
    opaque_message(&error.to_string())
}

fn opaque_message(message: &str) -> bool {
    message.contains("untagged enum")
        || message.starts_with("missing field")
        || message.starts_with("invalid type")
}

/// The block decoded as the complete typed AST but not as the lowering
/// mirror: name its kind and discriminant (CR 113.1 ability kinds).
fn block_mirror_gap(raw: &str, error: &serde_json::Error) -> Failure {
    if !opaque(error) {
        return unsupported("compose.block", error.to_string());
    }
    let block: serde_json::Value = serde_json::from_str(raw).unwrap_or_default();
    let tag = ["/effect/static", "/trigger/trigger", "/kind"]
        .iter()
        .find_map(|p| block.pointer(p).and_then(|v| v.as_str()))
        .unwrap_or("block");
    unsupported(
        "compose.block",
        format!("{tag} shape outside the lowering mirror"),
    )
}

/// Sentence conditions have no block table row; the shared step lowerer
/// would reject them later, but reject here with a stable reason. X
/// bindings ("where X is ...") lower in the shared step lowerer.
fn plain_sentences(effects: &[Sentence]) -> Result<(), Failure> {
    if effects
        .iter()
        .any(|s| s.condition.is_some() || s.condition_kind.is_some())
    {
        return Err(unsupported(
            "compose.effects",
            "conditional sentence outside the block tables",
        ));
    }
    Ok(())
}

/// Lower one block's sentences in printed order (CR 608.2c). A block made
/// only of typed sentences uses the shared lowerer once, as one whole
/// would. A block with table sentences lowers sentence by sentence; a typed
/// damage pronoun is then rejected because its antecedent could be a table
/// step the shared binder cannot see.
fn append_effects(
    program: &mut SpellProgram,
    parts: Vec<Part>,
    block: usize,
    slots: &mut BTreeMap<String, u32>,
    context: Context,
    damage: impl Fn(&[Sentence]) -> Result<(), Failure>,
    scope: impl Into<Scope>,
) -> Result<(), Failure> {
    let mut scope = scope.into();
    if parts.iter().all(|p| matches!(p, Part::Typed(_))) {
        let typed: Vec<Sentence> = parts
            .into_iter()
            .map(|p| match p {
                Part::Typed(s) => s,
                Part::Table(_) | Part::Revealed(_) | Part::Reflexive(..) | Part::Departure(_) => {
                    unreachable!("all parts are typed")
                }
            })
            .collect();
        plain_sentences(&typed)?;
        damage(&typed)?;
        return append_spell_effects(program, &typed, block, slots, scope, context);
    }
    let label = source_block(block)?;
    for part in parts {
        match part {
            Part::Typed(sentence) => {
                if sentence.steps.iter().any(|step| {
                    matches!(step, Step::Modal { .. })
                        || step.damage().is_some_and(|damage| {
                            matches!(damage.source.as_ref(), ast::ObjectRef::It(_))
                        })
                }) {
                    return Err(unsupported(
                        "compose.effects",
                        "pronoun source or modal step beside a table step",
                    ));
                }
                let sentence = [sentence];
                plain_sentences(&sentence)?;
                damage(&sentence)?;
                append_spell_effects(program, &sentence, block, slots, scope, context)?;
                for introduced in &sentence[0].introduces {
                    scope.prior = scope.prior.join(*introduced);
                }
            }
            Part::Table(steps) => {
                for step in &steps {
                    scope.prior = scope.prior.join(crate::ingest::Introduced::of_step(step));
                    let instruction = if resolution_triggers::is_delayed(step) {
                        resolution_triggers::delayed(step, program, context, label)?
                    } else {
                        steps::lower(step, context, label)?
                    };
                    program.instructions.push(instruction);
                }
            }
            Part::Revealed(effects) => revealed::lower(&effects, program, slots, context, label)?,
            Part::Reflexive(..) | Part::Departure(_) => {
                return Err(unsupported(
                    "compose.step",
                    "reflexive trigger requires a parent trigger",
                ));
            }
        }
    }
    Ok(())
}

fn activation(
    card: &Card<'_>,
    costs: &[SourceCost],
    parts: Vec<Part>,
    timing: SpellTiming,
    block: usize,
    printed: &str,
) -> Result<ActivatedAbility, Failure> {
    costs::printed(costs, printed)?;
    let (cost, functioning_zone, hybrid_mana, x_mana_cost) = match costs::exile_source(card, costs)?
    {
        Some((cost, zone)) => (cost, zone, vec![], false),
        None => {
            let (cost, hybrid, x) = costs::announced(card, costs)?;
            (cost, FunctioningZone::Battlefield, hybrid, x)
        }
    };
    // CR 608.2c: only sacrificing this exact source proves a first pronoun.
    let first_it_is_self = matches!(cost, ActivationCost::SacrificeSource)
        || matches!(&cost, ActivationCost::Composite { costs } if costs.sacrifice);
    let mut program = SpellProgram::instructions(timing);
    append_effects(
        &mut program,
        parts,
        block,
        &mut BTreeMap::new(),
        Context::Activated,
        |typed| {
            validate_damage_sources(typed, DamageSourceBinding::Invocation { first_it_is_self })
        },
        PlayerBinding::Unbound,
    )?;
    Ok(ActivatedAbility {
        x_mana_cost,
        hybrid_mana,
        modal: None,
        cost_changes: vec![],
        functioning_zone,
        restriction: None,
        cost,
        program,
    })
}

/// CR 603.2: "your/their Nth [filtered] spell or card each turn" qualifies
/// the event it is printed on. The event lowers without it, then the
/// ordinal is attached to the cast, draw or life-gain pattern it counts;
/// any other qualified event, period or count rejects by name.
fn trigger(
    card: &Card<'_>,
    raw: &serde_json::Value,
    parts: Vec<Part>,
    condition: Option<&serde_json::Value>,
    block: usize,
) -> Result<TriggeredAbility, Failure> {
    trigger_in(card, raw, parts, condition, block, false)
}

/// [`trigger`], where `names_source` records that a clause already stripped
/// from `raw` names the source apart from the event object (CR 201.5), so
/// the effects lower with the source as a candidate antecedent.
fn trigger_in(
    card: &Card<'_>,
    raw: &serde_json::Value,
    parts: Vec<Part>,
    condition: Option<&serde_json::Value>,
    block: usize,
    names_source: bool,
) -> Result<TriggeredAbility, Failure> {
    if raw["trigger"] == "final-chapter-ability" {
        return Err(unsupported(
            "compose.block",
            "trigger:final-chapter-ability",
        ));
    }
    // CR 102.1, 500.1: "[event] during your turn" lowers the event without
    // the window, then requires the controller to be the active player.
    if let Some(during) = raw.get("during") {
        if during != "your" {
            return Err(unsupported("compose.block", "trigger:window:during"));
        }
        let mut stripped = raw.clone();
        if let Some(fields) = stripped.as_object_mut() {
            fields.remove("during");
        }
        let mut ability = trigger_in(card, &stripped, parts, condition, block, names_source)?;
        let TriggerCondition::Event { your_turn, .. } = &mut ability.condition;
        *your_turn = true;
        return Ok(ability);
    }
    // CR 115.1, 601.2c: "a spell that targets this [permanent]" lowers the
    // cast event without the target clause, then names the source.
    if raw["trigger"] == "cast"
        && let Some(targets) = raw["what"].get("targets")
        && (*targets == serde_json::json!({"ref": "self"})
            || targets["ref"] == "this"
                && targets.as_object().map(|o| o.len()) == Some(2)
                && targets["noun"]
                    .as_str()
                    .is_some_and(|noun| card.is_self(noun)))
    {
        let mut stripped = raw.clone();
        if let Some(what) = stripped["what"].as_object_mut() {
            what.remove("targets");
        }
        let mut ability = trigger_in(card, &stripped, parts, condition, block, true)?;
        let TriggerCondition::Event { pattern, .. } = &mut ability.condition;
        let EventPattern::Cast { targets_source, .. } = pattern else {
            return Err(unsupported("compose.block", "trigger:targets:event"));
        };
        *targets_source = true;
        return Ok(ability);
    }
    // CR 115.1, 601.2c: "a spell that targets a [filtered permanent]"
    // lowers the cast event without the target clause, then reads some
    // permanent the spell targets as it became cast.
    if raw["trigger"] == "cast"
        && let Some(targets) = raw["what"].get("targets")
        && targets["ref"] == "filter"
        && targets.as_object().map(serde_json::Map::len) == Some(3)
        && targets["count"] == serde_json::json!({"amount": "fixed", "value": 1})
    {
        let filter: ast::ObjectFilter = events::typed_filter(&targets["filter"])?;
        let target = lower_event_filter(&filter)?;
        let mut stripped = raw.clone();
        if let Some(what) = stripped["what"].as_object_mut() {
            what.remove("targets");
        }
        let mut ability = trigger_in(card, &stripped, parts, condition, block, names_source)?;
        let TriggerCondition::Event { pattern, .. } = &mut ability.condition;
        let EventPattern::Cast { target: slot, .. } = pattern else {
            return Err(unsupported("compose.block", "trigger:targets:event"));
        };
        *slot = target;
        return Ok(ability);
    }
    let Some(occurrence) = raw.get("occurrence") else {
        return trigger_event(card, raw, parts, condition, block, names_source);
    };
    let reject =
        |detail: &str| unsupported("compose.block", format!("trigger:occurrence:{detail}"));
    let fields = occurrence.as_object().ok_or_else(|| reject("shape"))?;
    let nth = fields
        .get("ordinal")
        .and_then(serde_json::Value::as_u64)
        .and_then(|n| u32::try_from(n).ok())
        .filter(|n| (1..=4).contains(n))
        .ok_or_else(|| reject("ordinal"))?;
    let per_player = match fields.get("per") {
        None => false,
        Some(per) if per == "player" => true,
        Some(_) => return Err(reject("per")),
    };
    if fields.get("period") != Some(&serde_json::json!("turn"))
        || fields
            .keys()
            .any(|k| !matches!(k.as_str(), "ordinal" | "period" | "per"))
    {
        return Err(reject("period"));
    }
    let mut stripped = raw.clone();
    if let Some(object) = stripped.as_object_mut() {
        object.remove("occurrence");
    }
    let mut ability = trigger_event(card, &stripped, parts, condition, block, names_source)?;
    let TriggerCondition::Event { pattern, .. } = &mut ability.condition;
    // "your Nth" and "their Nth" count the event player's own events; only
    // "the Nth spell of a turn" (each player, no per-player count) counts
    // every player's. "An opponent's Nth" without a per-player count is
    // not a printed form.
    let ordinal = |player: &PlayerExpr, cast: bool| -> Result<kir::TurnOrdinal, Failure> {
        let any_player = match player {
            _ if per_player => false,
            PlayerExpr::EachPlayer if cast => true,
            p if p.is_you() => false,
            _ => return Err(reject("player")),
        };
        Ok(kir::TurnOrdinal { nth, any_player })
    };
    match pattern {
        EventPattern::Cast {
            caster, nth: slot, ..
        } => *slot = Some(ordinal(caster, true)?),
        EventPattern::Draws { player, nth: slot }
        | EventPattern::GainsLife { player, nth: slot } => *slot = Some(ordinal(player, false)?),
        // CR 702.29a: the turn's cycles, any card ("another" excludes none).
        EventPattern::Discards {
            player,
            nth: slot,
            cycled: true,
            card,
        } if !card.reads_characteristics() && !card.is_ref(kir::Ref::Source) => {
            *slot = Some(ordinal(player, false)?)
        }
        other => return Err(reject(other.reporting_key())),
    }
    Ok(ability)
}

fn trigger_event(
    card: &Card<'_>,
    raw: &serde_json::Value,
    parts: Vec<Part>,
    condition: Option<&serde_json::Value>,
    block: usize,
    names_source: bool,
) -> Result<TriggeredAbility, Failure> {
    // The reviewed lowerers decode the narrow trigger mirror. The event table
    // only adds self events the shared lowerer keys to one card type; the
    // shared trigger lowerer also binds event players, damage quantities and
    // intervening conditions. A trigger neither admits falls to the general
    // event table, which never changes a trigger the reviewed lowerers admit.
    let narrow = serde_json::from_value::<SourceTrigger>(raw.clone());
    let narrow_condition = condition
        .map(|c| serde_json::from_value::<SourceCondition>(c.clone()))
        .transpose();
    let table = narrow.as_ref().ok().and_then(|t| events::lower(card, t));
    if let [Part::Typed(sentence)] = parts.as_slice()
        && let Some(ability) = events::object_effect(card, raw, condition, sentence, block)?
    {
        return Ok(ability);
    }
    let mut reviewed = None;
    // CR 601.2i: a cast event whose effects include step-table rows binds
    // the reviewed cast pattern here and lowers its rows below, as the
    // general event table's events do.
    let cast_table = matches!(narrow, Ok(SourceTrigger::Cast { .. }))
        && parts.iter().any(|part| matches!(part, Part::Table(_)));
    let reviewed_effects = !cast_table
        && !parts.iter().any(|part| match part {
            Part::Typed(_) => false,
            Part::Table(steps) => steps.iter().any(resolution_triggers::is_delayed),
            Part::Revealed(_) | Part::Reflexive(..) | Part::Departure(_) => true,
        });
    if table.is_none()
        && reviewed_effects
        && let (Ok(t), Ok(c)) = (&narrow, &narrow_condition)
    {
        let typed: Vec<Sentence> = parts
            .clone()
            .into_iter()
            .map(reviewed_sentence)
            .collect::<Result<_, _>>()?;
        match lower_trigger(t, &typed, c.as_ref(), block, names_source) {
            Ok(ability) => return Ok(ability),
            Err(failure) => reviewed = Some(failure),
        }
    }
    // An intervening clause the reviewed lowerer does not read: lower the
    // ability without it, then add the audited clause.
    if table.is_none()
        && reviewed_effects
        && let (Ok(t), Some(raw_condition)) = (&narrow, condition)
        && reviewed
            .as_ref()
            .is_none_or(|f| f.path == "abilities.condition")
    {
        let typed: Vec<Sentence> = parts
            .clone()
            .into_iter()
            .map(reviewed_sentence)
            .collect::<Result<_, _>>()?;
        match lower_trigger(t, &typed, None, block, names_source) {
            Ok(mut ability) => {
                let intervening = events::intervening(raw_condition, ability.condition.pattern())?;
                let TriggerCondition::Event { intervening_if, .. } = &mut ability.condition;
                *intervening_if = Some(intervening.condition());
                return Ok(ability);
            }
            Err(failure) => {
                reviewed.get_or_insert(failure);
            }
        }
    }
    let event = match (table, &narrow) {
        (Some(event), _) => event,
        (
            None,
            Ok(SourceTrigger::Cast {
                who,
                what,
                not_from,
            }),
        ) if cast_table => cast_event(who, what, not_from.as_ref())?,
        (None, _) => match events::general(card, raw)? {
            Some(event) => event,
            None => {
                return Err(reviewed.unwrap_or_else(|| {
                    let error = narrow
                        .err()
                        .or_else(|| narrow_condition.err())
                        .map(|e| e.to_string())
                        .unwrap_or_default();
                    if opaque_message(&error) {
                        unsupported("compose.block", "trigger shape outside the lowering mirror")
                    } else {
                        unsupported("compose.block", error)
                    }
                }));
            }
        },
    };
    // CR 603.4: the reviewed intervening clauses, against this event.
    let intervening = match (condition, narrow_condition) {
        (None, _) => None,
        (Some(raw_condition), narrow_condition) => {
            match narrow_condition
                .ok()
                .flatten()
                .map(|c| intervening_condition(narrow.as_ref().ok(), &event, Some(&c)))
            {
                Some(Ok(Some(reviewed))) => Some(reviewed),
                _ => Some(events::intervening(raw_condition, &event)?),
            }
        }
    };
    let mut program = SpellProgram::instructions(SpellTiming::Instant);
    let condition = TriggerCondition::Event {
        pattern: event,
        intervening_if: intervening.map(|i| i.condition()),
        once_each_turn: false,
        your_turn: false,
    };
    if let [Part::Departure(parent)] = parts.as_slice() {
        program = parent.clone();
    } else if let [Part::Reflexive(payment, sentence)] = parts.as_slice() {
        program
            .instructions
            .push(resolution_triggers::lower_reflexive(
                payment.clone(),
                sentence,
                &condition,
                source_block(block)?,
            )?);
    } else {
        append_effects(
            &mut program,
            parts,
            block,
            &mut BTreeMap::new(),
            Context::Triggered,
            |typed| validate_trigger_damage_sources(&condition, typed),
            trigger_scope_naming_source(&condition, names_source),
        )?;
    }
    // CR 702.29c: a discard of this card triggers from the graveyard.
    let functioning_zone = if condition.pattern().discards_source() {
        FunctioningZone::Graveyard
    } else {
        FunctioningZone::Battlefield
    };
    Ok(TriggeredAbility {
        modal: None,
        functioning_zone,
        condition,
        program,
    })
}

/// "This ability triggers only once each turn" (CR 603.2): the one printed
/// limit, at most one triggering per source incarnation each turn. Other
/// counts or periods reject by name.
fn trigger_limit(limit: Option<&serde_json::Value>) -> Result<bool, Failure> {
    match limit {
        None => Ok(false),
        Some(limit) if *limit == serde_json::json!({"maximum": 1, "period": "turn"}) => Ok(true),
        Some(_) => Err(unsupported("compose.block", "trigger:limit:period")),
    }
}

/// CR 603.2c: "Whenever one or more [objects] ..." triggers once per event
/// however many objects it affects. Limited to once each turn, it triggers
/// exactly when the first occurrence of the turn would trigger the
/// per-object form, so it lowers as that form when nothing in the ability
/// refers to the event's objects or players and no intervening condition
/// could tell the occurrences apart.
fn grouped_once(
    event: Box<serde_json::Value>,
    once_each_turn: bool,
) -> (Box<serde_json::Value>, bool) {
    if !once_each_turn || event.get("grouping") != Some(&serde_json::json!("one-or-more")) {
        return (event, false);
    }
    let mut event = event;
    if let Some(fields) = event.as_object_mut() {
        fields.remove("grouping");
    }
    (event, true)
}

fn references_trigger_event(value: &serde_json::Value) -> bool {
    match value {
        serde_json::Value::String(value) => {
            matches!(value.as_str(), "event_object" | "event_player")
        }
        serde_json::Value::Array(values) => values.iter().any(references_trigger_event),
        serde_json::Value::Object(fields) => fields.values().any(references_trigger_event),
        _ => false,
    }
}

fn limit_trigger(ability: &mut TriggeredAbility, once: bool, grouped: bool) -> Result<(), Failure> {
    if grouped {
        let program = serde_json::to_value(ability.programs().collect::<Vec<_>>())
            .map_err(|e| unsupported("compose.block", e.to_string()))?;
        if ability.condition.intervening_if().is_some() || references_trigger_event(&program) {
            return Err(unsupported(
                "compose.block",
                "trigger:grouping:event-reference",
            ));
        }
    }
    let TriggerCondition::Event { once_each_turn, .. } = &mut ability.condition;
    *once_each_turn = once;
    Ok(())
}

/// Every number printed in a block, outside mana symbols, reminder text and
/// quoted abilities, must occur in that block's AST. A changed amount whose
/// printed number the AST no longer carries is rejected rather than lowered
/// against its text.
fn numbers_printed(line: &str, ast: &str) -> Result<(), Failure> {
    fn runs(text: impl Iterator<Item = char>) -> BTreeSet<String> {
        let (mut found, mut run) = (BTreeSet::new(), String::new());
        for c in text.chain([' ']) {
            if c.is_ascii_digit() {
                run.push(c);
            } else if !run.is_empty() {
                found.insert(std::mem::take(&mut run));
            }
        }
        found
    }
    let (mut reminder, mut braces, mut quoted) = (0usize, 0usize, false);
    let printed = runs(line.chars().map(|c| {
        let outside = reminder == 0 && braces == 0 && !quoted;
        match c {
            '(' if !quoted => reminder += 1,
            ')' if !quoted => reminder = reminder.saturating_sub(1),
            '"' if reminder == 0 => quoted = !quoted,
            '{' => braces += 1,
            '}' => braces = braces.saturating_sub(1),
            _ if outside => return c,
            _ => {}
        }
        ' '
    }));
    if !printed.is_subset(&runs(ast.chars())) {
        return Err(unsupported(
            "compose.effects",
            "a printed number is absent from the AST",
        ));
    }
    Ok(())
}

/// The kernel scope a block's control nodes are attributed under. X is
/// admitted here; definition admission decides it (CR 107.3).
fn control_env<'a>(
    context: Context,
    targets: &'a [TargetSpec],
    event: Option<&'a TriggerCondition>,
) -> kir::Env<'a> {
    kir::Env {
        context,
        targets,
        event,
        announced_x: true,
        single_sacrifice_cost: false,
    }
}

/// What keeps one record out of composition, for the gap report.
#[derive(Default)]
pub(super) struct Blockers {
    /// Every card-wide reference-graph failure, in the order `lower`
    /// enforces them before any block is inspected.
    pub bindings: Vec<Failure>,
    /// The first failure of every printed block that does not lower.
    pub blocks: Vec<(usize, Failure)>,
    /// A failure outside the blocks once binding checks are deferred: an
    /// integrity check before them or a card-wide step after them.
    pub card: Option<Failure>,
}

/// The sentinel a deferred composition returns once some block failed.
const BLOCKS_FAILED: &str = "blocks failed";

pub(super) fn lower(
    archive: &AcceptedArchive,
    record: &str,
    evidence: Option<&evidence::CardEvidence>,
) -> Result<Definition, Failure> {
    compose(archive, record, evidence, None)
}

/// Every blocker the gap report should rank, without changing what lowers:
/// the card-wide binding checks are recorded instead of returned, so the
/// first block that also fails is named rather than hidden behind them. The
/// definition built under deferred checks is discarded; only `lower` admits.
pub(super) fn blockers(
    archive: &AcceptedArchive,
    record: &str,
    evidence: Option<&evidence::CardEvidence>,
) -> Blockers {
    let mut found = Blockers::default();
    if let Err(failure) = compose(archive, record, evidence, Some(&mut found))
        && !(failure.path == "compose" && failure.reason == BLOCKS_FAILED)
    {
        found.card = Some(failure);
    }
    found
}

/// A card-wide reference-graph check: enforced by `lower`, recorded (every
/// failure, in the enforced order) by `blockers`.
fn binding_gate(
    check: Result<(), Failure>,
    deferred: &mut Option<&mut Blockers>,
) -> Result<(), Failure> {
    match (check, deferred) {
        (Err(failure), Some(found)) => {
            found.bindings.push(failure);
            Ok(())
        }
        (check, _) => check,
    }
}

fn compose(
    archive: &AcceptedArchive,
    record: &str,
    evidence: Option<&evidence::CardEvidence>,
    mut deferred: Option<&mut Blockers>,
) -> Result<Definition, Failure> {
    let mut card = ingest::read_card(archive, record)?;
    // Select the whole-card authority before decoding any resolution verdict,
    // projection or segment. A compound ability keeps its existing authority.
    if let Some(claim) = source_authority::select(archive, &card)? {
        return source_authority::compose(archive, &mut card, claim, evidence);
    }
    let r = &card.record;
    let mut resolution =
        resolution::Resolution::read(r.resolution.as_deref(), r.resolution_sha256.as_deref())?;
    if resolution.face_count() != card.faces.len() {
        return Err(invalid("resolution", "missing or duplicated resolved face"));
    }
    for (index, (_, face)) in card.faces.iter().enumerate() {
        resolution.cover_spend_riders(index, face.abilities.get())?;
        resolution.cover_control(index, face.abilities.get())?;
        if face.abilities.get().contains("\"final-chapter-ability\"") {
            let abilities: Vec<serde_json::Value> = decode(face.abilities.get(), "abilities")?;
            if abilities
                .iter()
                .any(|a| a["trigger"]["trigger"] == "final-chapter-ability")
            {
                return Err(unsupported(
                    "compose.block",
                    "trigger:final-chapter-ability",
                ));
            }
        }
    }
    binding_gate(resolution.complete(), &mut deferred)?;
    let mut lowered = Vec::new();
    for (index, face) in std::mem::take(&mut card.faces).into_iter().enumerate() {
        resolution.select_face(index)?;
        let result = compose_face(
            archive,
            &card,
            index,
            face,
            &resolution,
            deferred.as_deref_mut(),
        );
        match result {
            Ok(face) => lowered.push(face),
            Err(failure)
                if deferred.is_some()
                    && failure.path == "compose"
                    && failure.reason == BLOCKS_FAILED => {}
            Err(failure) => return Err(failure),
        }
    }
    if deferred
        .as_ref()
        .is_some_and(|found| !found.blocks.is_empty())
    {
        return Err(unsupported("compose", BLOCKS_FAILED));
    }
    let back = if lowered.len() == 2 {
        lowered.pop()
    } else {
        None
    };
    let mut front = lowered
        .pop()
        .ok_or_else(|| invalid("faces", "no lowered face"))?;
    if let Some(mut back) = back {
        let keywords: Vec<_> = front
            .keywords
            .iter()
            .chain(&back.keywords)
            .copied()
            .collect();
        keywords::printed(&card.shape, &keywords)?;
        match card.layout {
            // CR 202.3b, 712.8e: the back face's mana value uses the front
            // face's mana cost.
            FaceLayout::Transforming => {
                back.characteristics.front_face_mana_value =
                    Some(front.characteristics.printed_mana_value());
            }
            // CR 202.2: the card's printed colors are its normal part's.
            FaceLayout::Adventurer => {
                let printed: BTreeSet<_> = card.printed_colors().into_iter().flatten().collect();
                if front.characteristics.colors.iter().collect::<BTreeSet<_>>() != printed {
                    return Err(invalid(
                        "source.colors",
                        "adventurer colors disagree with its mana cost",
                    ));
                }
            }
            FaceLayout::Split => {
                let colors: BTreeSet<_> = front
                    .characteristics
                    .colors
                    .iter()
                    .chain(&back.characteristics.colors)
                    .collect();
                if colors != card.printed_colors().into_iter().flatten().collect() {
                    return Err(unsupported(
                        "source.colors",
                        "split colors disagree with the combined mana cost",
                    ));
                }
            }
            FaceLayout::Single | FaceLayout::ModalDoubleFaced => {}
        }
        front.layout = card.layout;
        front.back_face = Some(Box::new(back));
        front.capabilities = front.requirements();
    } else {
        keywords::printed(&card.shape, &front.keywords)?;
    }
    front
        .validate()
        .map_err(|e| unsupported("composition", e))?;
    Ok(front)
}

fn compose_face(
    archive: &AcceptedArchive,
    card_source: &CardSource,
    part: usize,
    (source, result): (SourceFace, ParseResult),
    resolution: &resolution::Resolution,
    mut deferred: Option<&mut Blockers>,
) -> Result<Definition, Failure> {
    let printed: serde_json::Value = decode(result.abilities.get(), "abilities")?;
    // The generated AST and the lowering mirror both read an explicit `null`
    // as an absent term; the archive never prints one, so a `null` anywhere
    // (an object member or an array element) rejects under its field's name
    // before either decoder can drop it.
    if let Some(field) = null_field(&printed, "abilities") {
        return Err(unsupported("operand", format!("operand:ast:null:{field}")));
    }
    // The complete typed AST: an unknown shape is named, never an opaque
    // untagged-enum failure further down.
    let typed_blocks: Vec<ast::Ability> = decode(result.abilities.get(), "abilities")?;
    {
        binding_gate(
            resolution.references_with_types(&printed, &typed_blocks),
            &mut deferred,
        )?;
        // Every unaccounted step is its own binding blocker; `lower`
        // rejects at the first.
        match resolution.unaccounted_steps(&printed) {
            Err(failure) => binding_gate(Err(failure), &mut deferred)?,
            Ok(steps) => {
                for (_, _, effect) in steps {
                    binding_gate(
                        Err(unsupported("resolution", format!("step:{effect}"))),
                        &mut deferred,
                    )?;
                }
            }
        }
        binding_gate(resolution.referenced(&printed), &mut deferred)?;
        binding_gate(resolution.validate_delayed_returns(&printed), &mut deferred)?;
    }
    let types = type_line::parse(
        &source.type_line,
        card_source.layout == FaceLayout::Adventurer && part == 1,
    )?;
    let kind = types.kind;
    let noun = match kind {
        CardKind::Creature => "creature",
        CardKind::Planeswalker => "planeswalker",
        CardKind::Artifact => "artifact",
        CardKind::Enchantment => "enchantment",
        CardKind::Land => "land",
        CardKind::Instant | CardKind::Sorcery => "spell",
    };
    let canonical = resolution.canonical_abilities(result.abilities.get(), noun)?;
    // The normalized source must be exactly the parsed blocks, and each line
    // must carry the same AST as the aggregate; nothing may be unaccounted.
    let normalized = source_lines(&source.oracle_text, &source.name, &source.type_line)?;
    let parsed: Vec<String> = result
        .lines
        .iter()
        .map(|l| normalization::literal_spelling(&l.text))
        .collect();
    if normalized != parsed {
        // The same printed lines grouped into different blocks (the parser
        // keeps a rider line such as an ability-word "instead" with the block
        // it modifies) are accounted for, but outside the block mirror.
        return Err(if normalized.join("\n") == parsed.join("\n") {
            unsupported("lines", "parsed block grouping outside the block mirror")
        } else {
            invalid(
                "lines",
                "source contains unaccounted or mismatched normalized blocks",
            )
        });
    }
    let original: Vec<Box<RawValue>> = decode(result.abilities.get(), "abilities")?;
    let blocks: Vec<Box<RawValue>> = decode(&canonical, "abilities")?;
    if original.len() != result.lines.len() || blocks.len() != original.len() {
        return Err(invalid("lines", "unaccounted archived ability block"));
    }
    for (line, ability) in result.lines.iter().zip(&original) {
        let a: serde_json::Value = decode(line.ability.get(), "lines.ability")?;
        let b: serde_json::Value = decode(ability.get(), "abilities")?;
        if a != b || line.text.is_empty() {
            return Err(invalid("lines", "line and aggregate ability disagree"));
        }
    }
    let block_offset = resolution.block_offset();
    let spell_kind = matches!(kind, CardKind::Instant | CardKind::Sorcery);
    let stars = [&source.power, &source.toughness].map(|stat| {
        stat.as_deref()
            .and_then(|text| assemble::printed_stat(text).ok())
            .and_then(|(_, star)| star)
    });
    let (mut definition, mut labels) =
        assemble::shell(archive, card_source, part, source, &result, types)?;
    let additional_types = definition.characteristics.additional_types.clone();
    let subtypes = definition.characteristics.subtypes.clone();
    let card = Card {
        kind,
        additional_types: &additional_types,
        subtypes: &subtypes,
    };
    // CR 115.3: each printed block announces its own target occurrences.
    let mut slots = BTreeMap::new();
    for (block, raw) in blocks.iter().enumerate() {
        // `lower` stops at the first failing block; `blockers` records the
        // first failure of each block and goes on to the next.
        let lowered = (|| -> Result<(), Failure> {
            let label = source_block(block)?;
            let line = &result.lines[block].text;
            resolution.spans(block, line, &printed[block])?;
            let bound = if line.contains("you may have ") || line.contains("You may have ") {
                printed::bind_evaluated_choosers(raw.get(), line, &resolution.segments(block)?)?
            } else {
                None
            };
            let lowering = bound.as_deref().unwrap_or(raw.get());
            // CR 702.21a: ward instances lower from the typed AST; the
            // keyword mirror reads the block's other keywords.
            let ward = keywords::split_ward(lowering)?;
            let ward_names: Vec<Option<String>> = match &ward {
                Some(_) => serde_json::from_str::<serde_json::Value>(lowering)
                    .ok()
                    .and_then(|b| b["keywords"].as_array().cloned())
                    .unwrap_or_default()
                    .iter()
                    .map(|k| {
                        let name = k["keyword"].as_str().unwrap_or_default();
                        (name != "ward").then(|| name.replace('-', " "))
                    })
                    .collect(),
                None => vec![],
            };
            let (lowering, wards) = match &ward {
                Some((rest, wards)) => (rest.as_str(), wards.as_slice()),
                None => (lowering, [].as_slice()),
            };
            let ability: Block = if !wards.is_empty() && ward_names.iter().all(Option::is_none) {
                Block::Keywords {
                    ability_word: None,
                    keywords: vec![],
                }
            } else {
                serde_json::from_str(lowering).map_err(|e| block_mirror_gap(lowering, &e))?
            };
            numbers_printed(line, raw.get())?;
            let (activated, triggered) = (definition.activated.len(), definition.triggered.len());
            // A spell's printed blocks share one program; each announces its own
            // targets after the earlier blocks' (CR 601.2c).
            let spell_targets = definition.spell.as_ref().map_or(0, |p| p.targets.len());
            match ability {
                Block::Keywords {
                    ability_word: word,
                    keywords,
                } => {
                    ability_word(&word, &result.lines[block].text)?;
                    let text = &result.lines[block].text;
                    match keywords.as_slice() {
                        // CR 702.21a: each ward instance is its own
                        // triggered ability beside the listed keywords.
                        _ if !wards.is_empty() => {
                            if word.is_some() || keywords.iter().any(keywords::is_triggered) {
                                return Err(unsupported("keyword", "keyword:ward:printed"));
                            }
                            if !keywords.is_empty() {
                                definition
                                    .keywords
                                    .extend(keywords::lower_triggered_companion(
                                        card.kind, &keywords,
                                    )?);
                            }
                            keywords::ward_printed(&ward_names, wards, text)?;
                            for ward in wards {
                                definition
                                    .triggered
                                    .push(keywords::ward(&card, ward, label)?);
                                labels.triggered.push(label);
                            }
                        }
                        [one]
                            if matches!(
                                one.keyword.as_str(),
                                "delve" | "convoke" | "improvise"
                            ) =>
                        {
                            let expected = match one.keyword.as_str() {
                                "delve" => "Delve",
                                "convoke" => "Convoke",
                                _ => "Improvise",
                            };
                            if text != expected
                                || one.cost.is_some()
                                || one.filter.is_some()
                                || one.protection_from.is_some()
                                || one.amount.is_some()
                            {
                                return Err(unsupported(
                                    "keyword",
                                    format!("keyword:{}:parameters", one.keyword),
                                ));
                            }
                            match expected {
                                "Delve" => definition.delve = true,
                                "Convoke" => definition.convoke = true,
                                _ => definition.improvise = true,
                            }
                        }
                        [one] if one.keyword == "affinity" => {
                            definition
                                .static_abilities
                                .push(keywords::affinity(one, text)?);
                            labels.statics.push(label);
                        }
                        [one] if one.keyword == "crew" => {
                            definition
                                .activated
                                .push(keywords::crew(&card, one, label, text)?);
                            labels.activated.push(label);
                        }
                        [one] if one.keyword == "equip" => {
                            definition
                                .activated
                                .push(keywords::equip(&card, one, label, text)?);
                            labels.activated.push(label);
                        }
                        [one] if one.keyword == "cycling" => {
                            definition
                                .activated
                                .push(keywords::cycling(one, label, text)?);
                            labels.activated.push(label);
                        }
                        [one] if one.keyword == "typecycling" => {
                            definition
                                .activated
                                .push(keywords::typecycling(one, label, text)?);
                            labels.activated.push(label);
                        }
                        // CR 702.33a: kicker is an optional additional cost
                        // the caster declares while casting this spell.
                        [one] if one.keyword == "kicker" => {
                            let spell = definition
                                .spell
                                .as_mut()
                                .filter(|s| {
                                    spell_kind
                                        && !s
                                            .optional_additional_costs
                                            .iter()
                                            .any(|c| c.identity == keywords::KICKER)
                                })
                                .ok_or_else(|| unsupported("keyword", "keyword:kicker:source"))?;
                            spell
                                .optional_additional_costs
                                .push(keywords::kicker(one, label, text)?);
                        }
                        // CR 702.27a: buyback uses the same announcement as kicker.
                        [one] if one.keyword == mtg_contracts::BUYBACK_COST => {
                            let spell = definition
                                .spell
                                .as_mut()
                                .filter(|s| {
                                    spell_kind
                                        && !s
                                            .optional_additional_costs
                                            .iter()
                                            .any(|c| c.identity == mtg_contracts::BUYBACK_COST)
                                })
                                .ok_or_else(|| unsupported("keyword", "keyword:buyback:source"))?;
                            spell
                                .optional_additional_costs
                                .push(keywords::buyback(one, label, text)?);
                        }
                        // CR 702.74a: evoke is an alternative cost of a
                        // permanent spell plus the entering sacrifice trigger.
                        [one] if one.keyword == mtg_contracts::EVOKE_COST => {
                            if spell_kind
                                || definition.spell.is_none()
                                || definition.alternative_cost.is_some()
                            {
                                return Err(unsupported("keyword", "keyword:evoke:source"));
                            }
                            definition.alternative_cost = Some(keywords::evoke(one, label, text)?);
                            definition
                                .triggered
                                .push(TriggeredAbility::evoke_sacrifice(label));
                            labels.triggered.push(label);
                        }
                        // CR 702.109a: dash is an alternative cost of a
                        // creature spell; the engine creates its return.
                        [one] if one.keyword == mtg_contracts::DASH_COST => {
                            if spell_kind
                                || definition.spell.is_none()
                                || definition.alternative_cost.is_some()
                                || definition.characteristics.kind != CardKind::Creature
                            {
                                return Err(unsupported("keyword", "keyword:dash:source"));
                            }
                            definition.alternative_cost = Some(keywords::dash(one, label, text)?);
                        }
                        // CR 702.84a: unearth is a graveyard ability of a
                        // permanent card.
                        [one] if one.keyword == "unearth" => {
                            if spell_kind || definition.spell.is_none() {
                                return Err(unsupported("keyword", "keyword:unearth:source"));
                            }
                            definition
                                .activated
                                .push(keywords::unearth(one, label, text)?);
                            labels.activated.push(label);
                        }
                        // CR 702.141a: encore is a graveyard ability of a
                        // creature card.
                        [one] if one.keyword == "encore" => {
                            if spell_kind
                                || definition.spell.is_none()
                                || definition.characteristics.kind != CardKind::Creature
                            {
                                return Err(unsupported("keyword", "keyword:encore:source"));
                            }
                            definition
                                .activated
                                .push(keywords::encore(one, label, text)?);
                            labels.activated.push(label);
                        }
                        // CR 702.30a: echo is a triggered ability of a permanent.
                        [one] if one.keyword == "echo" => {
                            if spell_kind || definition.spell.is_none() {
                                return Err(unsupported("keyword", "keyword:echo:source"));
                            }
                            definition.triggered.push(keywords::echo(one, label, text)?);
                            labels.triggered.push(label);
                        }
                        // CR 702.138a: escape casts the card from its owner's
                        // graveyard for an alternative cost.
                        [one] if one.keyword == "escape" => {
                            if definition.spell.is_none()
                                || definition.escape.is_some()
                                || definition.retrace
                                || definition.flashback.is_some()
                            {
                                return Err(unsupported("keyword", "keyword:escape:source"));
                            }
                            definition.escape = Some(keywords::escape(one, text)?);
                        }
                        // CR 702.81a: retrace lets an instant or sorcery be
                        // cast from its owner's graveyard for a land discard.
                        [one] if one.keyword == "retrace" => {
                            if !spell_kind
                                || definition.retrace
                                || definition.flashback.is_some()
                                || definition.escape.is_some()
                            {
                                return Err(unsupported("keyword", "keyword:retrace:source"));
                            }
                            definition.retrace = true;
                        }
                        // CR 702.34a: flashback is an instant's or sorcery's
                        // alternative cost from its owner's graveyard.
                        [one] if one.keyword == "flashback" => {
                            if !spell_kind
                                || definition.flashback.is_some()
                                || definition.retrace
                                || definition.escape.is_some()
                            {
                                return Err(unsupported("keyword", "keyword:flashback:source"));
                            }
                            definition.flashback = Some(keywords::flashback(one, text)?);
                        }
                        // CR 702.37a-b: morph and megamorph let this
                        // permanent card be cast face down (CR 708.2a).
                        [one] if matches!(one.keyword.as_str(), "morph" | "megamorph") => {
                            if spell_kind || definition.face_down.is_some() {
                                return Err(unsupported("keyword", "keyword:morph:source"));
                            }
                            definition.face_down = Some(keywords::face_down(one, text)?);
                        }
                        [one] if one.keyword == "enchant" => {
                            let spell = definition
                                .spell
                                .as_mut()
                                .filter(|s| {
                                    s.targets.is_empty()
                                        && definition.characteristics.subtypes == ["Aura"]
                                })
                                .ok_or_else(|| unsupported("keyword", "keyword:enchant:source"))?;
                            spell.targets.push(keywords::enchant(one, text)?);
                        }
                        // CR 702.73a: changeling is a characteristic-defining
                        // ability, "This object is every creature type"; it
                        // functions in every zone (CR 604.3).
                        _ if keywords.iter().any(|k| k.keyword == "changeling") => {
                            let (changeling, rest): (Vec<_>, Vec<_>) = keywords
                                .iter()
                                .cloned()
                                .partition(|k| k.keyword == "changeling");
                            if changeling.len() != 1
                                || changeling[0] != keywords::changeling()
                                || kind != CardKind::Creature
                            {
                                return Err(unsupported("keyword", "keyword:changeling:source"));
                            }
                            definition.characteristics.all_creature_types = true;
                            if !rest.is_empty() {
                                definition
                                    .keywords
                                    .extend(keywords::lower(card.kind, &rest, text)?);
                            }
                        }
                        // CR 702.108a, 702.45a, 702.135a, 702.46a:
                        // a triggered keyword is the trigger its rule
                        // defines; each printed instance is its own ability.
                        _ if keywords.iter().any(keywords::is_triggered) => {
                            let (triggers, rest): (Vec<_>, Vec<_>) =
                                keywords.iter().cloned().partition(keywords::is_triggered);
                            if !rest.is_empty() {
                                definition
                                    .keywords
                                    .extend(keywords::lower_triggered_companion(card.kind, &rest)?);
                            }
                            keywords::triggered_printed(&keywords, text, &card_source.shape)?;
                            for keyword in &triggers {
                                definition
                                    .triggered
                                    .push(keywords::triggered(&card, keyword, label)?);
                                labels.triggered.push(label);
                                // CR 702.25a: "a creature without flanking"
                                // reads this marker.
                                if keyword.keyword == "flanking"
                                    && !definition.keywords.contains(&KeywordAbility::Flanking)
                                {
                                    definition.keywords.push(KeywordAbility::Flanking);
                                }
                            }
                        }
                        _ => definition
                            .keywords
                            .extend(keywords::lower(card.kind, &keywords, text)?),
                    }
                }
                Block::Static {
                    ability_word: word,
                    effect,
                } => {
                    ability_word(&word, &result.lines[block].text)?;
                    if let Some(cda) = rules::characteristic_pt(&effect) {
                        definition.static_abilities.push(cda?);
                        labels.statics.push(label);
                        return Ok(());
                    }
                    if let Some(cda) = rules::every_creature_type_cda(&effect) {
                        cda?;
                        if kind != CardKind::Creature {
                            return Err(unsupported("static", "static:cda-types:noncreature"));
                        }
                        definition.characteristics.all_creature_types = true;
                        return Ok(());
                    }
                    let quoted = |raw: &serde_json::Value| quoted_ability(raw, block, kind);
                    let abilities = lower_static(&effect, &StaticSite { quoted: &quoted })?;
                    labels.statics.extend(abilities.iter().map(|_| label));
                    definition.static_abilities.extend(abilities);
                }
                Block::Spell {
                    ability_word: word,
                    effects,
                } if spell_kind => {
                    ability_word(&word, &result.lines[block].text)?;
                    slots.clear();
                    let site = control::Site {
                        resolution,
                        index: block,
                        printed: &printed[block],
                        context: Context::Spell,
                    };
                    if modal::declared(&effects) {
                        if definition.modal_spell.is_some() {
                            return Err(unsupported(
                                "compose.effects",
                                "multiple modal declarations",
                            ));
                        }
                        let program = definition.spell.as_ref().expect("spell program");
                        definition.modal_spell = Some(modal::lower(
                            &effects,
                            program.timing,
                            &result.lines[block].text,
                            &site,
                        )?);
                        return printed::choices_printed(
                            &result.lines[block].text,
                            &printed[block],
                            &resolution.segments(block)?,
                        );
                    }
                    let program = definition.spell.as_mut().expect("spell program");
                    if !steps::sacrifice_return(&effects, &site, program, &mut slots, label)?
                        && !steps::declared_shuffle(&effects, &site, program, &mut slots, label)?
                    {
                        let peeled = control::peel(&effects)?;
                        let (before, before_slots) = (program.clone(), slots.clone());
                        let lower =
                            |program: &mut SpellProgram,
                             slots: &mut BTreeMap<String, u32>,
                             sentences: &[serde_json::Value]| {
                                append_effects(
                                    program,
                                    parts(sentences)?,
                                    block,
                                    slots,
                                    Context::Spell,
                                    |typed| {
                                        validate_damage_sources(typed, DamageSourceBinding::Spell)
                                    },
                                    PlayerBinding::Unbound,
                                )
                            };
                        lower(program, &mut slots, &peeled.sentences)?;
                        let base = before.instructions.len();
                        control::apply(
                            &peeled,
                            &site,
                            &mut program.instructions,
                            base,
                            |prefix| {
                                let (mut scratch, mut scratch_slots) =
                                    (before.clone(), before_slots.clone());
                                lower(&mut scratch, &mut scratch_slots, prefix)?;
                                Ok(scratch.instructions.len() - base)
                            },
                        )?;
                        control::admitted(
                            &peeled,
                            &program.instructions[base..],
                            &control_env(Context::Spell, &program.targets, None),
                        )?;
                    }
                }
                Block::Loyalty { cost, effects } => {
                    // CR 606.4: fixed printed loyalty change is the sole cost.
                    let text = &result.lines[block].text;
                    let printed_cost = match (cost.sign, cost.amount) {
                        (1, amount) if amount > 0 => format!("+{amount}"),
                        (-1, amount) if amount > 0 => format!("-{amount}"),
                        (0, 0) => "0".to_owned(),
                        _ => return Err(unsupported("compose.cost", "unsupported loyalty cost")),
                    };
                    if kind != CardKind::Planeswalker
                        || text.split_once(": ").map(|(cost, _)| cost)
                            != Some(printed_cost.as_str())
                    {
                        return Err(unsupported(
                            "compose.cost",
                            "printed loyalty cost differs from the AST",
                        ));
                    }
                    let mut program = SpellProgram::instructions(SpellTiming::MainPhase);
                    append_effects(
                        &mut program,
                        parts(&effects)?,
                        block,
                        &mut BTreeMap::new(),
                        Context::Activated,
                        |typed| {
                            validate_damage_sources(
                                typed,
                                DamageSourceBinding::Invocation {
                                    first_it_is_self: false,
                                },
                            )
                        },
                        PlayerBinding::Unbound,
                    )?;
                    definition.activated.push(ActivatedAbility {
                        x_mana_cost: false,
                        hybrid_mana: vec![],
                        modal: None,
                        cost_changes: vec![],
                        functioning_zone: FunctioningZone::Battlefield,
                        restriction: None,
                        cost: ActivationCost::Loyalty {
                            change: Quantity::from(cost.sign * cost.amount),
                        },
                        program,
                    });
                    labels.activated.push(label);
                }
                Block::Activated {
                    ability_word: word,
                    restriction,
                    costs,
                    effects,
                    cost_modifications,
                } => {
                    ability_word(&word, &result.lines[block].text)?;
                    let cost_changes = cost_modifications
                        .iter()
                        .flatten()
                        .map(activation_cost_change)
                        .collect::<Result<Vec<_>, _>>()?;
                    let (timing, restriction) = activation_restrictions::lower(
                        restriction,
                        &result.lines[block].text,
                        &|path| {
                            resolution.control_player(
                                block,
                                &printed[block],
                                path,
                                Context::Activated,
                            )
                        },
                    )?;
                    let mana_restriction = restriction
                        .as_ref()
                        .filter(|r| r.once_each_turn && r.window.is_none())
                        .map(|_| SourceActivationRestriction::OnceEachTurn {});
                    // CR 602.2b, 700.2: modes are chosen as the ability is
                    // activated; a mana ability has no modes here.
                    if modal::declared(&effects) {
                        modal::reject_mana_modes(&effects)?;
                        if !cost_changes.is_empty() {
                            return Err(unsupported(
                                "compose.block",
                                "modal activation cost change",
                            ));
                        }
                        let text = &result.lines[block].text;
                        let site = control::Site {
                            resolution,
                            index: block,
                            printed: &printed[block],
                            context: Context::Activated,
                        };
                        let (mut ability, modal) = modal::lower_ability(
                            &effects,
                            text,
                            modal::Header::Activated,
                            &site,
                            |sentences| {
                                activation(&card, &costs, parts(sentences)?, timing, block, text)
                            },
                            |a: &mut ActivatedAbility| &mut a.program,
                        )?;
                        if spell_kind {
                            return Err(unsupported(
                                "compose.block",
                                "battlefield activation on a spell card",
                            ));
                        }
                        ability.restriction = restriction;
                        ability.modal = Some(modal);
                        labels.activated.push(label);
                        definition.activated.push(ability);
                        return printed::choices_printed(
                            text,
                            &printed[block],
                            &resolution.segments(block)?,
                        );
                    }
                    let peeled = control::peel(&effects)?;
                    let core = parts(&peeled.sentences)?;
                    let parts = core;
                    // CR 605.1a: a mana ability is recognised from its complete
                    // typed cost, production and restriction, never a table row.
                    let typed: Option<Vec<Sentence>> = parts
                        .iter()
                        .map(|p| match p {
                            Part::Typed(s) => Some(s.clone()),
                            Part::Table(_)
                            | Part::Revealed(_)
                            | Part::Reflexive(..)
                            | Part::Departure(_) => None,
                        })
                        .collect();
                    // A mana ability has no resolution-time control (CR 605.3b).
                    let mana = match &typed {
                        Some(_) if !peeled.is_plain() => None,
                        Some(typed) => lower_mana_ability(
                            &costs,
                            typed,
                            mana_restriction.as_ref(),
                            block,
                            kind,
                            &|noun| card.is_self(noun),
                        )?,
                        None => None,
                    };
                    if let Some(mut mana) = mana {
                        if !cost_changes.is_empty() {
                            return Err(unsupported("compose.block", "mana ability cost change"));
                        }
                        if spell_kind
                            || timing != SpellTiming::Instant
                            || restriction
                                .as_ref()
                                .is_some_and(|r| r.window.is_some() || r.times_each_turn.is_some())
                        {
                            return Err(unsupported(
                                "restriction",
                                "timing restriction on a mana ability",
                            ));
                        }
                        // CR 602.5, 605.3a: "Activate only if" gates the
                        // mana ability as its activation begins.
                        mana.condition = restriction.and_then(|r| r.condition);
                        labels.mana.push(label);
                        definition.mana_abilities.push(mana);
                    } else {
                        let lower = |parts| {
                            activation(
                                &card,
                                &costs,
                                parts,
                                timing,
                                block,
                                &result.lines[block].text,
                            )
                        };
                        let mut ability = lower(parts)?;
                        if spell_kind && ability.functioning_zone == FunctioningZone::Battlefield {
                            return Err(unsupported(
                                "compose.block",
                                "battlefield activation on a spell card",
                            ));
                        }
                        ability.restriction = restriction;
                        ability.cost_changes = cost_changes;
                        let site = control::Site {
                            resolution,
                            index: block,
                            printed: &printed[block],
                            context: Context::Activated,
                        };
                        control::apply(
                            &peeled,
                            &site,
                            &mut ability.program.instructions,
                            0,
                            |prefix| Ok(lower(self::parts(prefix)?)?.program.instructions.len()),
                        )?;
                        control::admitted(
                            &peeled,
                            &ability.program.instructions,
                            &control_env(Context::Activated, &ability.program.targets, None),
                        )?;
                        labels.activated.push(label);
                        definition.activated.push(ability);
                    }
                }
                Block::AdditionalCost {
                    ability_word: word,
                    costs,
                    optional,
                    cost_modifications,
                    variable_constraints,
                } => {
                    ability_word(&word, &result.lines[block].text)?;
                    for (field, value) in [
                        ("optional", optional),
                        ("cost-modifications", cost_modifications),
                        ("variable-constraints", variable_constraints),
                    ] {
                        if value.is_some() {
                            return Err(unsupported(
                                "additional-cost",
                                format!("kind:additional-cost:{field}"),
                            ));
                        }
                    }
                    let spell = definition
                        .spell
                        .as_mut()
                        .ok_or_else(|| unsupported("additional-cost", "source"))?;
                    costs::spell_additional(&costs, &result.lines[block].text, spell)?;
                }
                Block::Chapter {
                    chapters,
                    ability_word,
                    effects,
                } => {
                    let site = control::Site {
                        resolution,
                        index: block,
                        printed: &printed[block],
                        context: Context::Triggered,
                    };
                    let abilities = sagas::chapters(
                        &card,
                        chapters,
                        &ability_word,
                        &effects,
                        &result.lines[block].text,
                        &site,
                    )?;
                    labels
                        .triggered
                        .extend(std::iter::repeat_n(label, abilities.len()));
                    definition.triggered.extend(abilities);
                }
                Block::Triggered {
                    ability_word: word,
                    condition,
                    trigger: event,
                    effects,
                    limit,
                } if !spell_kind => {
                    let once_each_turn = trigger_limit(limit.as_ref())?;
                    let (event, grouped) = grouped_once(event, once_each_turn);
                    ability_word(&word, &result.lines[block].text)?;
                    printed::intervening_if(
                        &result.lines[block].text,
                        &printed[block],
                        &resolution.segments(block)?,
                    )?;
                    // CR 603.3c, 700.2: modes are chosen as the trigger is
                    // put on the stack.
                    if modal::declared(&effects) {
                        let text = &result.lines[block].text;
                        let site = control::Site {
                            resolution,
                            index: block,
                            printed: &printed[block],
                            context: Context::Triggered,
                        };
                        let (mut ability, modal) = modal::lower_ability(
                            &effects,
                            text,
                            modal::Header::Triggered,
                            &site,
                            |sentences| {
                                trigger(&card, &event, parts(sentences)?, condition.as_ref(), block)
                            },
                            |a: &mut TriggeredAbility| &mut a.program,
                        )?;
                        ability.modal = Some(modal);
                        limit_trigger(&mut ability, once_each_turn, grouped)?;
                        labels.triggered.push(label);
                        definition.triggered.push(ability);
                        return printed::choices_printed(
                            text,
                            &printed[block],
                            &resolution.segments(block)?,
                        );
                    }
                    let site = control::Site {
                        resolution,
                        index: block,
                        printed: &printed[block],
                        context: Context::Triggered,
                    };
                    let ability = if let Some(parent) =
                        resolution_triggers::departure(&effects, &site, &result.lines[block].text)?
                    {
                        trigger(
                            &card,
                            &event,
                            vec![Part::Departure(parent)],
                            condition.as_ref(),
                            block,
                        )?
                    } else if let Some(part) =
                        resolution_triggers::reflexive(&effects, &site, &result.lines[block].text)?
                    {
                        trigger(&card, &event, vec![part], condition.as_ref(), block)?
                    } else {
                        let peeled = control::peel(&effects)?;
                        let lower = |sentences: &[serde_json::Value]| {
                            trigger(&card, &event, parts(sentences)?, condition.as_ref(), block)
                        };
                        let mut ability = lower(&peeled.sentences)?;
                        control::apply(
                            &peeled,
                            &site,
                            &mut ability.program.instructions,
                            0,
                            |prefix| Ok(lower(prefix)?.program.instructions.len()),
                        )?;
                        control::admitted(
                            &peeled,
                            &ability.program.instructions,
                            &control_env(
                                Context::Triggered,
                                &ability.program.targets,
                                Some(&ability.condition),
                            ),
                        )?;
                        ability
                    };
                    let mut ability = ability;
                    limit_trigger(&mut ability, once_each_turn, grouped)?;
                    labels.triggered.push(label);
                    definition.triggered.push(ability);
                }
                _ => {
                    return Err(unsupported(
                        "compose.kind",
                        "ability kind outside the block tables",
                    ));
                }
            }
            // Every block that lowered, keyword blocks included, must also
            // print exactly the choices, limits and conditions its AST
            // carries, sentence by sentence (`printed`). Checked once lowering
            // succeeded, so a block a table cannot lower keeps that blocker.
            printed::choices_printed(
                &result.lines[block].text,
                &printed[block],
                &resolution.segments(block)?,
            )?;
            // CR 601.2c: the block's announced targets are the resolution's
            // block-relative slots.
            {
                let (program, first) = if definition.activated.len() > activated {
                    (definition.activated.last().map(|a| &a.program), 0)
                } else if definition.triggered.len() > triggered {
                    (definition.triggered.last().map(|t| &t.program), 0)
                } else if spell_kind {
                    (definition.spell.as_ref(), spell_targets)
                } else {
                    (None, 0)
                };
                let first = u32::try_from(first).map_err(|_| invalid("targets", "slot range"))?;
                let targets: Vec<TargetSpec> = program
                    .map(|p| {
                        p.targets
                            .iter()
                            .filter(|t| t.slot >= first)
                            .map(|t| TargetSpec {
                                slot: t.slot - first,
                                ..t.clone()
                            })
                            .collect()
                    })
                    .unwrap_or_default();
                let reflexive = program
                    .and_then(|p| p.instructions.last())
                    .and_then(|i| match i {
                        Instruction::Kernel {
                            node: kir::Node::Reflexive { program, .. },
                            source_block,
                        } if *source_block == label => Some(program.targets.as_slice()),
                        _ => None,
                    });
                resolution.target_slots_with_reflexive(
                    block,
                    &printed[block],
                    &targets,
                    reflexive,
                )?;
            }
            Ok(())
        })();
        if let Err(failure) = lowered {
            match deferred.as_deref_mut() {
                Some(found) => found.blocks.push((block_offset + block, failure)),
                None => return Err(failure),
            }
        }
    }
    if deferred.as_ref().is_some_and(|found| {
        found
            .blocks
            .last()
            .is_some_and(|(index, _)| *index >= block_offset)
    }) {
        return Err(unsupported("compose", BLOCKS_FAILED));
    }
    // Checked once every block lowered, so a block that fails keeps its own
    // first blocker.
    let lines: Vec<String> = result.lines.iter().map(|l| l.text.clone()).collect();
    keywords::triggered_instances_printed(&lines, &printed)?;
    assemble::stars_defined(&definition.static_abilities, &stars)?;
    Ok(assemble::finish(definition, labels))
}
