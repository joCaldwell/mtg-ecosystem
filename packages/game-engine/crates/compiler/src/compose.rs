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

mod control;
mod costs;
mod events;
mod keywords;
mod steps;
#[cfg(test)]
mod tests;

/// Facts about the printed card that block tables may consult. A table never
/// sees sibling blocks, so a row cannot depend on the rest of the card.
pub(super) struct Card<'a> {
    pub kind: CardKind,
    pub additional_types: &'a [CardKind],
}
impl Card<'_> {
    fn has_type(&self, kind: CardKind) -> bool {
        self.kind == kind || self.additional_types.contains(&kind)
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
        condition: Option<SourceCondition>,
        trigger: Box<SourceTrigger>,
        effects: Vec<serde_json::Value>,
    },
    Spell {
        #[serde(rename = "abilityWord")]
        ability_word: Option<String>,
        effects: Vec<serde_json::Value>,
    },
    Activated {
        restriction: Option<serde_json::Value>,
        #[serde(rename = "abilityWord")]
        ability_word: Option<String>,
        costs: Vec<SourceCost>,
        effects: Vec<serde_json::Value>,
    },
}

/// One printed sentence: either the shared typed mirror or step-table rows.
enum Part {
    Typed(Sentence),
    Table(Vec<serde_json::Value>),
}

fn parts(effects: &[serde_json::Value]) -> Result<Vec<Part>, Failure> {
    effects
        .iter()
        .map(|sentence| {
            // A table sentence carries nothing but its steps: no condition,
            // no bindings, and every step claimed by a row.
            if let Some(object) = sentence.as_object()
                && object.len() == 1
                && let Some(steps) = object.get("steps").and_then(|s| s.as_array())
                && !steps.is_empty()
                && steps.iter().all(steps::claims)
            {
                return Ok(Part::Table(steps.clone()));
            }
            serde_json::from_value::<Sentence>(sentence.clone())
                .map(Part::Typed)
                .map_err(|e| mirror_gap(sentence, &e))
        })
        .collect()
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
    let message = error.to_string();
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

/// Sentence conditions and X bindings have no block table row; the shared
/// step lowerer would reject them later, but reject here with a stable reason.
fn plain_sentences(effects: &[Sentence]) -> Result<(), Failure> {
    if effects
        .iter()
        .any(|s| !s.bindings.is_empty() || s.condition.is_some() || s.condition_kind.is_some())
    {
        return Err(unsupported(
            "compose.effects",
            "conditional or bound sentence outside the block tables",
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
) -> Result<(), Failure> {
    if parts.iter().all(|p| matches!(p, Part::Typed(_))) {
        let typed: Vec<Sentence> = parts
            .into_iter()
            .map(|p| match p {
                Part::Typed(s) => s,
                Part::Table(_) => unreachable!("all parts are typed"),
            })
            .collect();
        plain_sentences(&typed)?;
        damage(&typed)?;
        return append_spell_effects(
            program,
            &typed,
            block,
            slots,
            PlayerBinding::Unbound,
            context,
        );
    }
    let label = source_block(block)?;
    for part in parts {
        match part {
            Part::Typed(sentence) => {
                if sentence.steps.iter().any(|step| {
                    matches!(
                        step,
                        Step::Damage {
                            source: DamageSource::It {},
                            ..
                        } | Step::Modal { .. }
                    )
                }) {
                    return Err(unsupported(
                        "compose.effects",
                        "pronoun source or modal step beside a table step",
                    ));
                }
                let sentence = [sentence];
                plain_sentences(&sentence)?;
                damage(&sentence)?;
                append_spell_effects(
                    program,
                    &sentence,
                    block,
                    slots,
                    PlayerBinding::Unbound,
                    context,
                )?;
            }
            Part::Table(steps) => {
                for step in &steps {
                    let instruction = steps::lower(step, context, label)?;
                    program.instructions.push(instruction);
                }
            }
        }
    }
    Ok(())
}

/// CR 602.5d: "Activate only as a sorcery" restricts when the ability may be
/// activated; the runtime checks the program's announcement timing. Every
/// other restriction keeps the shared typed mirror.
fn timing(
    restriction: Option<serde_json::Value>,
) -> Result<(SpellTiming, Option<SourceActivationRestriction>), Failure> {
    match restriction {
        None => Ok((SpellTiming::Instant, None)),
        Some(value) if value == serde_json::json!({"restriction":"sorcery"}) => {
            Ok((SpellTiming::MainPhase, None))
        }
        Some(value) => serde_json::from_value(value)
            .map(|typed| (SpellTiming::Instant, Some(typed)))
            .map_err(|_| unsupported("restriction", "unsupported activation restriction")),
    }
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
    let cost = costs::lower(card, costs)?;
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
    )?;
    Ok(ActivatedAbility {
        functioning_zone: FunctioningZone::Battlefield,
        cost,
        program,
    })
}

fn trigger(
    card: &Card<'_>,
    trigger: &SourceTrigger,
    parts: Vec<Part>,
    condition: Option<&SourceCondition>,
    block: usize,
) -> Result<TriggeredAbility, Failure> {
    // The event table only adds self events the shared lowerer keys to one
    // card type; every other event keeps the shared trigger lowerer, which
    // also binds event players, damage quantities and intervening conditions.
    let Some(event) = events::lower(card, trigger) else {
        let typed: Vec<Sentence> = parts
            .into_iter()
            .map(|p| match p {
                Part::Typed(s) => Ok(s),
                Part::Table(_) => Err(unsupported(
                    "compose.step",
                    "step table rows require a table event",
                )),
            })
            .collect::<Result<_, _>>()?;
        return lower_trigger(trigger, &typed, condition, block);
    };
    if condition.is_some() {
        return Err(unsupported(
            "abilities.condition",
            "unsupported complete intervening-if condition",
        ));
    }
    let mut program = SpellProgram::instructions(SpellTiming::Instant);
    append_effects(
        &mut program,
        parts,
        block,
        &mut BTreeMap::new(),
        Context::Triggered,
        |typed| validate_trigger_damage_sources(&event, typed),
    )?;
    Ok(TriggeredAbility {
        functioning_zone: FunctioningZone::Battlefield,
        condition: event,
        intervening_if: None,
        death_filter: None,
        program,
    })
}

/// Every printed choice, limit and condition is carried by the block's AST
/// exactly once, with its polarity, and the AST carries no unprinted one.
/// Each printed word has a closed set of AST carriers, the parser's spelling
/// of exactly that clause anywhere in the block (effects, costs, intervening
/// condition, restriction, permission or static effect), and the two counts
/// must be equal:
///
/// * "may" (CR 608.2d): an `optional` step or `optional-group`; a play or
///   cast permission or additional land play (CR 305.2, 601.3); "you may
///   choose the same mode more than once" and "you may choose both instead"
///   of a modal step (CR 700.2d); "any player may activate" (CR 602.2).
/// * "if": a sentence condition of kind `if`, an intervening-if (CR 603.4)
///   or restriction (CR 602.5b) condition, a conditional mode count, the
///   conditional replacement of a countered spell (CR 701.6), a created
///   replacement (CR 614.1) and an "if able" requirement (CR 508.1d,
///   509.1c). In a static block a kindless condition is printed "if", "as
///   long as" or "during" (CR 611.3a), so those phrases count together.
/// * "unless": a condition of kind `unless`, so "if" and "unless" cannot
///   stand in for each other.
/// * "up to" (CR 601.2c, 700.2): an `upTo` operand or a chosen count whose
///   minimum is zero.
///
/// A block whose text and AST disagree would otherwise lower as mandatory,
/// optional, exact, inverted or unconditional against its printed text; a
/// carrier's own lowering decides whether its semantics are supported.
/// Reminder text and quoted abilities are not the block's own words. Which
/// printed clause produced which AST sentence is the resolution's span
/// certificate.
fn choices_printed(line: &str, ability: &serde_json::Value) -> Result<(), Failure> {
    use serde_json::Value;
    let mut rules_text = String::new();
    let (mut reminder, mut quoted) = (0usize, false);
    for c in line.chars() {
        match c {
            '(' if !quoted => reminder += 1,
            ')' if !quoted => reminder = reminder.saturating_sub(1),
            '"' if reminder == 0 => quoted = !quoted,
            _ if reminder == 0 && !quoted => rules_text.extend(c.to_lowercase()),
            _ => {}
        }
    }
    // Phrases inside one clause: "rounded up, to you" is not "up to".
    let phrases = |phrase: &[&str]| -> usize {
        rules_text
            .split(|c: char| !(c.is_alphabetic() || c == ' '))
            .map(|clause| {
                let clause: Vec<&str> = clause.split_whitespace().collect();
                clause
                    .windows(phrase.len())
                    .filter(|w| *w == phrase)
                    .count()
            })
            .sum()
    };
    let words = |word: &str| phrases(&[word]);
    #[derive(Default, PartialEq, Debug)]
    struct Counts {
        may: usize,
        condition: usize,
        unless: usize,
        limit: usize,
    }
    fn walk(value: &Value, found: &mut Counts) {
        match value {
            Value::Object(fields) => {
                let kind = fields.get("conditionKind").and_then(Value::as_str);
                for (key, child) in fields {
                    match (key.as_str(), child) {
                        ("optional", Value::Bool(true)) => found.may += 1,
                        ("effect" | "static", Value::String(tag)) => match tag.as_str() {
                            "optional-group"
                            | "grant-play-permission"
                            | "additional-land-plays" => found.may += 1,
                            "create-replacement" | "must-attack" | "must-block" => {
                                found.condition += 1
                            }
                            _ => {}
                        },
                        ("allowRepeat" | "permission", _) => found.may += 1,
                        ("conditionalCount", _) => {
                            found.may += 1;
                            found.condition += 1;
                        }
                        ("conditionKind", Value::String(k)) => match k.as_str() {
                            "if" => found.condition += 1,
                            "unless" => found.unless += 1,
                            _ => {}
                        },
                        ("condition", Value::Object(_)) if kind.is_none() => found.condition += 1,
                        ("countered", _) => found.condition += 1,
                        ("rule", Value::String(rule)) if rule.ends_with("-requirement") => {
                            found.condition += 1
                        }
                        ("upTo", Value::Bool(true)) => found.limit += 1,
                        ("minimum" | "min", Value::Number(n)) if n.as_u64() == Some(0) => {
                            found.limit += 1
                        }
                        _ => {}
                    }
                    walk(child, found);
                }
            }
            Value::Array(items) => items.iter().for_each(|item| walk(item, found)),
            _ => {}
        }
    }
    let statics = if ability["kind"] == "static" {
        phrases(&["as", "long", "as"]) + words("during")
    } else {
        0
    };
    let printed = Counts {
        may: words("may"),
        condition: words("if") + statics,
        unless: words("unless"),
        limit: phrases(&["up", "to"]),
    };
    let mut found = Counts::default();
    walk(ability, &mut found);
    if printed != found {
        return Err(unsupported(
            "compose.effects",
            "printed choice, limit or condition disagrees with the AST",
        ));
    }
    Ok(())
}

/// CR 603.4: "When/Whenever/At [event], if [condition], [effect]" is an
/// intervening-if clause, checked as the ability triggers and again on
/// resolution. It must be the block's own trigger condition; a sentence
/// condition would only be read on resolution (CR 608.2a). The clause is an
/// "if" printed immediately after the trigger event's comma.
fn intervening_if(line: &str, ability: &serde_json::Value) -> Result<(), Failure> {
    let text = match ability["abilityWord"].as_str() {
        Some(word) => line
            .strip_prefix(&format!("{word} \u{2014} "))
            .unwrap_or(line),
        None => line,
    };
    let text = text.to_lowercase();
    let first = text.split('.').next().unwrap_or("");
    let intervening = ["when ", "whenever ", "at "]
        .iter()
        .any(|event| first.starts_with(event))
        && first
            .split_once(',')
            .is_some_and(|(_, rest)| rest.trim_start().starts_with("if "));
    if intervening && ability.get("condition").is_none() {
        return Err(unsupported(
            "abilities.condition",
            "printed intervening if outside the trigger condition",
        ));
    }
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

pub(super) fn lower(archive: &AcceptedArchive, record: &str) -> Result<Definition, Failure> {
    compose(archive, record, None)
}

/// Every blocker the gap report should rank, without changing what lowers:
/// the card-wide binding checks are recorded instead of returned, so the
/// first block that also fails is named rather than hidden behind them. The
/// definition built under deferred checks is discarded; only `lower` admits.
pub(super) fn blockers(archive: &AcceptedArchive, record: &str) -> Blockers {
    let mut found = Blockers::default();
    if let Err(failure) = compose(archive, record, Some(&mut found))
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
    mut deferred: Option<&mut Blockers>,
) -> Result<Definition, Failure> {
    // Integrity: the exact record, source, parse and AST hashes.
    let SingleFace {
        record: r,
        shape,
        source,
        result,
        ast_sha256,
    } = ingest::read_single_face(record)?;
    // Archive R4: the parser's resolution, not the diagnostic graph, says how
    // every reference, step and printed token of each block is accounted for.
    let resolution =
        resolution::Resolution::read(r.resolution.as_deref(), r.resolution_sha256.as_deref())?;
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
    decode::<Vec<ast::Ability>>(result.abilities.get(), "abilities")?;
    {
        binding_gate(resolution.complete(), &mut deferred)?;
        binding_gate(resolution.references(&printed), &mut deferred)?;
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
    }
    let types = type_line::parse_source(&source.type_line, &source.layout)?;
    let kind = types.kind;
    let noun = match kind {
        CardKind::Creature => "creature",
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
    let spell_kind = matches!(kind, CardKind::Instant | CardKind::Sorcery);
    let (mut definition, mut labels) =
        assemble::shell(archive, r, source, &result, types, ast_sha256)?;
    let additional_types = definition.characteristics.additional_types.clone();
    let card = Card {
        kind,
        additional_types: &additional_types,
    };
    // CR 115.3: each printed block announces its own target occurrences.
    let mut slots = BTreeMap::new();
    for (block, raw) in blocks.iter().enumerate() {
        // `lower` stops at the first failing block; `blockers` records the
        // first failure of each block and goes on to the next.
        let lowered = (|| -> Result<(), Failure> {
            let ability: Block =
                serde_json::from_str(raw.get()).map_err(|e| block_mirror_gap(raw.get(), &e))?;
            let label = source_block(block)?;
            {
                resolution.spans(block, &result.lines[block].text, &printed[block])?;
            }
            numbers_printed(&result.lines[block].text, raw.get())?;
            let (activated, triggered) = (definition.activated.len(), definition.triggered.len());
            // A spell's printed blocks share one program; each announces its own
            // targets after the earlier blocks' (CR 601.2c).
            let spell_targets = definition.spell.as_ref().map_or(0, |p| p.targets.len());
            // Keyword blocks print only keywords and their reminder text.
            let guarded = !matches!(ability, Block::Keywords { .. });
            match ability {
                Block::Keywords {
                    ability_word: word,
                    keywords,
                } => {
                    ability_word(&word, &result.lines[block].text)?;
                    definition
                        .keywords
                        .extend(keywords::lower(&card, &keywords)?);
                }
                Block::Static {
                    ability_word: word,
                    effect,
                } => {
                    ability_word(&word, &result.lines[block].text)?;
                    let abilities = lower_static(&effect)?;
                    labels.statics.extend(abilities.iter().map(|_| label));
                    definition.static_abilities.extend(abilities);
                }
                Block::Spell {
                    ability_word: word,
                    effects,
                } if spell_kind => {
                    ability_word(&word, &result.lines[block].text)?;
                    slots.clear();
                    let peeled = control::peel(&effects)?;
                    let program = definition.spell.as_mut().expect("spell program");
                    let (before, before_slots) = (program.clone(), slots.clone());
                    let lower = |program: &mut SpellProgram,
                                 slots: &mut BTreeMap<String, u32>,
                                 sentences: &[serde_json::Value]| {
                        append_effects(
                            program,
                            parts(sentences)?,
                            block,
                            slots,
                            Context::Spell,
                            |typed| validate_damage_sources(typed, DamageSourceBinding::Spell),
                        )
                    };
                    lower(program, &mut slots, &peeled.sentences)?;
                    let site = control::Site {
                        resolution: &resolution,
                        index: block,
                        printed: &printed[block],
                        context: Context::Spell,
                    };
                    let base = before.instructions.len();
                    control::apply(&peeled, &site, &mut program.instructions, base, |prefix| {
                        let (mut scratch, mut scratch_slots) =
                            (before.clone(), before_slots.clone());
                        lower(&mut scratch, &mut scratch_slots, prefix)?;
                        Ok(scratch.instructions.len() - base)
                    })?;
                    control::admitted(
                        &peeled,
                        &program.instructions[base..],
                        &control_env(Context::Spell, &program.targets, None),
                    )?;
                }
                Block::Activated {
                    ability_word: word,
                    restriction,
                    costs,
                    effects,
                } if !spell_kind => {
                    ability_word(&word, &result.lines[block].text)?;
                    let (timing, restriction) = timing(restriction)?;
                    let peeled = control::peel(&effects)?;
                    let core = parts(&peeled.sentences)?;
                    let parts = core;
                    // CR 605.1a: a mana ability is recognised from its complete
                    // typed cost, production and restriction, never a table row.
                    let typed: Option<Vec<Sentence>> = parts
                        .iter()
                        .map(|p| match p {
                            Part::Typed(s) => Some(s.clone()),
                            Part::Table(_) => None,
                        })
                        .collect();
                    // A mana ability has no resolution-time control (CR 605.3b).
                    let mana = match &typed {
                        Some(_) if !peeled.is_plain() => None,
                        Some(typed) => lower_mana_ability(
                            &costs,
                            typed,
                            restriction.as_ref(),
                            block,
                            kind,
                            &|noun| card.is_self(noun),
                        )?,
                        None => None,
                    };
                    if let Some(mana) = mana {
                        if timing != SpellTiming::Instant {
                            return Err(unsupported(
                                "restriction",
                                "timing restriction on a mana ability",
                            ));
                        }
                        labels.mana.push(label);
                        definition.mana_abilities.push(mana);
                    } else if restriction.is_some() {
                        return Err(unsupported(
                            "restriction",
                            "activation restriction outside the block tables",
                        ));
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
                        let site = control::Site {
                            resolution: &resolution,
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
                Block::Triggered {
                    ability_word: word,
                    condition,
                    trigger: event,
                    effects,
                } if !spell_kind => {
                    ability_word(&word, &result.lines[block].text)?;
                    intervening_if(&result.lines[block].text, &printed[block])?;
                    let peeled = control::peel(&effects)?;
                    let lower = |sentences: &[serde_json::Value]| {
                        trigger(&card, &event, parts(sentences)?, condition.as_ref(), block)
                    };
                    let mut ability = lower(&peeled.sentences)?;
                    let site = control::Site {
                        resolution: &resolution,
                        index: block,
                        printed: &printed[block],
                        context: Context::Triggered,
                    };
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
            // A block that lowered must also print exactly the choices,
            // limits and conditions its AST carries. Checked once lowering
            // succeeded, so a block a table cannot lower keeps that blocker.
            if guarded {
                choices_printed(&result.lines[block].text, &printed[block])?;
            }
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
                resolution.target_slots(block, &printed[block], &targets)?;
            }
            Ok(())
        })();
        if let Err(failure) = lowered {
            match deferred.as_deref_mut() {
                Some(found) => found.blocks.push((block, failure)),
                None => return Err(failure),
            }
        }
    }
    if deferred
        .as_ref()
        .is_some_and(|found| !found.blocks.is_empty())
    {
        return Err(unsupported("compose", BLOCKS_FAILED));
    }
    keywords::printed(&shape, &definition.keywords)?;
    assemble::admit(definition, labels)
}
