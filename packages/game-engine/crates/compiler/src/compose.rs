//! Compositional lowering: one ability block at a time.
//!
//! The family matchers in `lib.rs` recognise whole cards. This pipeline is the
//! fallback when no family claims a record: every printed block is lowered
//! independently from its complete typed AST by small tables (`costs`,
//! `events`, `keywords`) plus the shared step lowerer, and the resulting
//! definition is admitted only by `mtg_contracts::composition`.
//!
//! Invariants kept from the family path: the exact archived source, AST and
//! binding hashes; the normalized line/aggregate mirror; complete typed
//! decoding with unknown fields rejected; and an explicit rejection whenever a
//! clause, operand or reference is not consumed. Nothing here reads a card
//! name, an Oracle identity or a profile.
//!
//! CR 113.1-113.3, 602.1, 603.1, 608.2 (pinned 2026-09-25).
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;

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
        effect: SourceStaticEffect,
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
                .map_err(|e| unsupported("compose.step", e.to_string()))
        })
        .collect()
}

/// Sentence conditions and X bindings belong to reviewed families; the shared
/// step lowerer would reject them later, but reject here with a stable reason.
fn plain_sentences(effects: &[Sentence]) -> Result<(), Failure> {
    if effects
        .iter()
        .any(|s| !s.bindings.is_empty() || s.condition.is_some() || s.condition_kind.is_some())
    {
        return Err(unsupported(
            "compose.effects",
            "conditional or bound sentence requires a reviewed family",
        ));
    }
    Ok(())
}

/// Lower one block's sentences in printed order (CR 608.2c). A block made
/// only of typed sentences uses the shared lowerer once, exactly as a family
/// would. A block with table sentences lowers sentence by sentence; a typed
/// damage pronoun is then rejected because its antecedent could be a table
/// step the shared binder cannot see.
fn append_effects(
    program: &mut SpellProgram,
    parts: Vec<Part>,
    block: usize,
    slots: &mut BTreeMap<String, u32>,
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
        return append_spell_effects(program, &typed, block, slots, PlayerBinding::Unbound);
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
                append_spell_effects(program, &sentence, block, slots, PlayerBinding::Unbound)?;
            }
            Part::Table(steps) => {
                for step in &steps {
                    let instruction = steps::lower(step, program, slots, label)?;
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
    append_effects(&mut program, parts, block, &mut BTreeMap::new(), |typed| {
        validate_damage_sources(typed, DamageSourceBinding::Invocation { first_it_is_self })
    })?;
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
    // The event table only adds self events the family lowerer keys to one
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
    append_effects(&mut program, parts, block, &mut BTreeMap::new(), |typed| {
        validate_trigger_damage_sources(&event, typed)
    })?;
    Ok(TriggeredAbility {
        functioning_zone: FunctioningZone::Battlefield,
        condition: event,
        intervening_if: None,
        death_filter: None,
        program,
    })
}

/// The reference binder's own certificate that every reference, number and
/// scope in the AST resolved without a diagnostic. Its `engineFeatures` list
/// names semantics a path needs from the runtime; that is decided node by
/// node below, where each step, static and event either lowers to an existing
/// runtime operation or rejects.
fn references_bound(binding: &serde_json::Value) -> bool {
    binding["referencesBound"] == true
        && binding["diagnostics"] == serde_json::json!([])
        && binding["counts"]["unresolved"] == 0
}

/// The lowerer reads each reference the ordinary way: "you" is the ability's
/// own controller (CR 109.5), "this" is the source (CR 201.5), a target is one
/// of the block's announced targets (CR 601.2c), and an "its controller" or
/// "its power" operand is the program's one announced target (CR 608.2h).
/// Any other pronoun is consumed only inside an exact multi-step sequence
/// (search then put, look then choose). Require the archived reference graph
/// to say the same: every reference is bound to an existing node; direct
/// references resolve to those ordinary antecedents; an operand-of reference
/// resolves to a target of its own block; and any other derived reference
/// resolves to such a target or to an earlier step of the same block. A
/// damage-source pronoun is the one exception; its antecedent is proven
/// separately.
fn references_resolve_in_block(
    binding: &serde_json::Value,
    identity: &str,
    abilities: &serde_json::Value,
) -> Result<(), Failure> {
    let unbound = || unsupported("binding", "reference leaves its ordinary antecedent");
    let mut nodes = BTreeMap::new();
    for node in binding["nodes"].as_array().ok_or_else(unbound)? {
        // A repeated node identity would make every edge to it ambiguous.
        if nodes
            .insert(node["id"].as_str().ok_or_else(unbound)?, node)
            .is_some()
        {
            return Err(unbound());
        }
    }
    for reference in binding["references"].as_array().ok_or_else(unbound)? {
        let target = reference["target"].as_str().ok_or_else(unbound)?;
        let Some(node) = nodes.get(target) else {
            return Err(unbound());
        };
        if reference["binding"] != "bound" {
            return Err(unbound());
        }
        let path = reference["path"].as_str().ok_or_else(unbound)?;
        // A reference outside the ability list is not an operand of any block.
        let Some(block) = path
            .strip_prefix("/faces/0/abilities/")
            .and_then(|rest| rest.split('/').next())
        else {
            continue;
        };
        let ability = format!("card:{identity}/faces/0/abilities/{block}");
        let names_target = target.starts_with(&format!("{ability}#ability/targets/"));
        let expression = &reference["expression"];
        if *expression == serde_json::json!({"player":"you"}) {
            // CR 109.5: "you" is whoever controls this kind of ability.
            let role = match abilities
                .get(block.parse::<usize>().map_err(|_| unbound())?)
                .and_then(|a| a["kind"].as_str())
            {
                Some("spell") => "spell-controller",
                Some("activated") => "activator",
                Some("triggered") => "trigger-controller",
                Some("static") => "static-source-controller",
                _ => return Err(unbound()),
            };
            if !(target.starts_with(&ability)
                && target.ends_with("#you")
                && node["node"] == "runtime-player"
                && node["role"] == role)
            {
                return Err(unbound());
            }
            continue;
        }
        let Some(kind) = expression["ref"].as_str() else {
            continue;
        };
        let ordinary = match kind {
            "this" => target == format!("card:{identity}/source#object"),
            "target" => names_target,
            // The resolving object itself, and sets and filters, are consumed
            // operand by operand.
            "self" | "filter" | "each" | "all" => true,
            _ if path.ends_with("/source") => true,
            _ => {
                names_target
                    || (target.starts_with(&format!("{ability}/effects/"))
                        && !path.ends_with("/of"))
            }
        };
        if !ordinary {
            return Err(unbound());
        }
    }
    Ok(())
}

/// Every printed sentence of a block must be an AST sentence. The parser
/// spells a timing restriction and a regeneration prohibition as fields of
/// the preceding clause; everything else is one `effects` entry per printed
/// sentence. Reminder text and quoted abilities are not sentences of the
/// block. A row whose text carries a clause the AST lacks is rejected rather
/// than lowered without it.
fn sentences_accounted(
    line: &str,
    effects: &[serde_json::Value],
    restricted: bool,
) -> Result<(), Failure> {
    let mut printed = 0usize;
    let mut rules_text = String::new();
    let (mut reminder, mut quoted) = (0usize, false);
    let mut characters = line.chars().peekable();
    while let Some(c) = characters.next() {
        match c {
            '(' if !quoted => reminder += 1,
            ')' if !quoted => reminder = reminder.saturating_sub(1),
            '"' if reminder == 0 => quoted = !quoted,
            '.' | '!' | '?'
                if reminder == 0
                    && !quoted
                    && characters.peek().is_none_or(|next| next.is_whitespace()) =>
            {
                printed += 1;
                rules_text.push(' ');
            }
            _ if reminder == 0 && !quoted => rules_text.extend(c.to_lowercase()),
            _ => {}
        }
    }
    // A printed choice, limit or condition must be carried by the AST, and an
    // AST choice must be printed. The parser spells "may" as `optional`, "up
    // to" as `upTo` and "if"/"unless" as a condition; a block whose text and
    // AST disagree would otherwise lower as mandatory, optional, exact or
    // unconditional against its printed text.
    let words: Vec<&str> = rules_text
        .split(|c: char| !c.is_alphabetic())
        .filter(|w| !w.is_empty())
        .collect();
    let ast = serde_json::to_string(effects).unwrap_or_default();
    let has = |word: &str| words.contains(&word);
    if (has("may") != ast.contains("\"optional\":true"))
        || (words.windows(2).any(|w| w == ["up", "to"]) && !ast.contains("\"upTo\":true"))
        || ((has("if") || has("unless")) && !ast.contains("\"condition"))
    {
        return Err(unsupported(
            "compose.effects",
            "printed choice, limit or condition disagrees with the AST",
        ));
    }
    let prohibitions = effects
        .iter()
        .flat_map(|e| e["steps"].as_array().into_iter().flatten())
        .filter(|step| step["regenerationForbidden"] == true)
        .count();
    if printed != effects.len() + prohibitions + usize::from(restricted) {
        return Err(unsupported(
            "compose.effects",
            "printed sentences differ from the AST sentences",
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

/// The archived reference graph and the AST must describe the same clauses.
/// Every bound reference and node addresses an existing AST location (or an
/// implicit operand of one); a reference to an operand object carries that
/// exact operand; and every printed step carries the binder's own event node
/// for the same operation. A clause or operand removed, added or changed
/// after binding leaves a dangling, missing or unequal entry, so the record
/// is rejected instead of lowering something other than what was bound.
fn binding_covers_ast(
    binding: &serde_json::Value,
    abilities: &serde_json::Value,
) -> Result<(), Failure> {
    const ROOT: &str = "/faces/0/abilities";
    let mismatch = || unsupported("binding", "reference graph does not cover the AST");
    let nodes = binding["nodes"].as_array().ok_or_else(mismatch)?;
    let references = binding["references"].as_array().ok_or_else(mismatch)?;
    let mut events = BTreeSet::new();
    for (item, is_node) in nodes
        .iter()
        .map(|n| (n, true))
        .chain(references.iter().map(|r| (r, false)))
    {
        let path = item["path"].as_str().ok_or_else(mismatch)?;
        if is_node && item["node"] == "event" {
            events.insert((path, item["operation"].as_str().ok_or_else(mismatch)?));
        }
        let Some(pointer) = path.strip_prefix(ROOT) else {
            if matches!(path, "/source" | "/faces/0") {
                continue;
            }
            return Err(mismatch());
        };
        let parent = pointer.rsplit_once('/').map_or("", |(parent, _)| parent);
        match abilities.pointer(pointer) {
            // A string operand ("you", "your") is bound in normalized form,
            // an implicit relation has no spelled operand, and an any-target
            // destination is spelled as a destination rather than a reference.
            Some(operand)
                if !is_node
                    && operand.is_object()
                    && item["expression"].get("implicit").is_none()
                    && item["expression"]["ref"] != "any-target" =>
            {
                if *operand != item["expression"] {
                    return Err(mismatch());
                }
            }
            Some(_) => {}
            // An implicit operand hangs off an existing object, never off a
            // missing element of a list.
            None if abilities.pointer(parent).is_some_and(|v| v.is_object()) => {}
            None => return Err(mismatch()),
        }
    }
    for (block, ability) in abilities
        .as_array()
        .ok_or_else(mismatch)?
        .iter()
        .enumerate()
    {
        let Some(effects) = ability.get("effects").and_then(|e| e.as_array()) else {
            continue;
        };
        for (sentence, effect) in effects.iter().enumerate() {
            let Some(steps) = effect.get("steps").and_then(|s| s.as_array()) else {
                continue;
            };
            for (index, step) in steps.iter().enumerate() {
                let operation = step["effect"].as_str().ok_or_else(mismatch)?;
                // Grouping steps are bound through their member steps.
                if matches!(operation, "modal" | "simultaneous" | "optional-group") {
                    continue;
                }
                let path = format!("{ROOT}/{block}/effects/{sentence}/steps/{index}");
                if !events.contains(&(path.as_str(), operation)) {
                    return Err(mismatch());
                }
            }
        }
    }
    Ok(())
}

/// What keeps one record out of composition, for the gap report.
#[derive(Default)]
pub(super) struct Blockers {
    /// The first card-wide reference-graph failure. `lower` reports it before
    /// any block is inspected.
    pub binding: Option<Failure>,
    /// The first failure once those checks are deferred: an integrity check,
    /// the first printed block that does not lower, or a card-wide step after
    /// the blocks. `None` when every block lowers.
    pub lowering: Option<Failure>,
    /// The printed block `lowering` failed in, when it failed inside one.
    pub block: Option<usize>,
}

pub(super) fn lower(archive: &AcceptedArchive, record: &str) -> Result<Definition, Failure> {
    compose(archive, record, None)
}

/// Every blocker the gap report should rank, without changing what lowers:
/// the card-wide binding checks are recorded instead of returned, so the
/// first block that also fails is named rather than hidden behind them. The
/// definition built under deferred checks is discarded; only `lower` admits.
pub(super) fn blockers(archive: &AcceptedArchive, record: &str) -> Blockers {
    let mut found = Blockers::default();
    let lowered = compose(archive, record, Some(&mut found));
    found.lowering = lowered.err();
    found
}

/// A card-wide reference-graph check: enforced by `lower`, recorded (first
/// failure only, matching the enforced order) by `blockers`.
fn binding_gate(
    check: Result<(), Failure>,
    deferred: &mut Option<&mut Blockers>,
) -> Result<(), Failure> {
    match (check, deferred) {
        (Err(failure), Some(found)) => {
            found.binding.get_or_insert(failure);
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
    // Integrity: the same checks, in the same order, as the family path.
    let r: Record = decode(record, "record")?;
    if r.record_type != "card" {
        return Err(invalid("recordType", "expected card record"));
    }
    if r.status != "parsed" {
        return Err(unsupported("status", "complete-card parsing is required"));
    }
    if sha256(r.source.get().as_bytes()) != r.source_sha256
        || sha256(r.binding.get().as_bytes()) != r.binding_sha256
    {
        return Err(invalid(
            "record",
            "source or diagnostic binding hash mismatch",
        ));
    }
    let shape: serde_json::Value = decode(r.source.get(), "source")?;
    if shape.get("card_faces").is_some_and(|v| !v.is_null()) {
        return Err(unsupported("source.layout", "multi-face layout"));
    }
    let source: Source = decode(r.source.get(), "source")?;
    if source.oracle_id != r.oracle_identity || source.id.is_empty() || source.name.is_empty() {
        return Err(invalid("source", "missing or mismatched card identity"));
    }
    if source.layout != "normal"
        || source.card_faces.is_some()
        || source.loyalty.is_some()
        || source.defense.is_some()
    {
        return Err(unsupported(
            "source.layout",
            "composition requires a single-face normal card",
        ));
    }
    if r.faces.len() != 1 || r.faces[0].source_face_index != 0 || r.faces[0].status != "attempted" {
        return Err(invalid("faces", "missing or duplicated source face"));
    }
    let result: ParseResult = decode(r.faces[0].result.get(), "faces[0].result")?;
    if !result.ok || result.name != source.name || result.lines.iter().any(|l| !l.ok) {
        return Err(invalid("faces[0].result", "incomplete or mismatched parse"));
    }
    let ast_sha256 = r
        .ast_sha256
        .clone()
        .ok_or_else(|| invalid("astSha256", "missing successful AST hash"))?;
    if sha256(format!("[{}]", result.abilities.get()).as_bytes()) != ast_sha256 {
        return Err(invalid("astSha256", "successful AST hash mismatch"));
    }
    let binding: serde_json::Value = decode(r.binding.get(), "binding")?;
    binding_gate(
        if references_bound(&binding) {
            Ok(())
        } else {
            Err(unsupported(
                "binding",
                "composition requires a completely bound reference graph",
            ))
        },
        &mut deferred,
    )?;
    let printed: serde_json::Value = decode(result.abilities.get(), "abilities")?;
    binding_gate(
        references_resolve_in_block(&binding, &r.oracle_identity, &printed),
        &mut deferred,
    )?;
    binding_gate(binding_covers_ast(&binding, &printed), &mut deferred)?;
    let types = type_line::parse_source(&source.type_line, &source.layout)?;
    let kind = types.kind;
    let noun = match kind {
        CardKind::Creature => "creature",
        CardKind::Artifact => "artifact",
        CardKind::Enchantment => "enchantment",
        CardKind::Land => "land",
        CardKind::Instant | CardKind::Sorcery => "spell",
    };
    let canonical = self_reference::canonicalize(
        result.abilities.get(),
        &binding,
        &r.oracle_identity,
        noun,
        "/faces/0/abilities",
    )?;
    // The normalized source must be exactly the parsed blocks, and each line
    // must carry the same AST as the aggregate; nothing may be unaccounted.
    if source_lines(&source.oracle_text, &source.name, &source.type_line)?
        != result
            .lines
            .iter()
            .map(|l| normalization::literal_spelling(&l.text))
            .collect::<Vec<_>>()
    {
        return Err(invalid(
            "lines",
            "source contains unaccounted or mismatched normalized blocks",
        ));
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
    // CR 118.6: an absent mana cost is not {0}. Hybrid and X costs keep their
    // reviewed families.
    if kind != CardKind::Land && source.mana_cost.is_empty() {
        return Err(unsupported(
            "source.mana_cost",
            "nonland without a mana cost requires an explicit absent-cost representation",
        ));
    }
    if source.mana_cost.contains("{X}") || color_hybrid_cost(&source.mana_cost).is_some() {
        return Err(unsupported(
            "source.mana_cost",
            "variable or hybrid costs require a reviewed family",
        ));
    }
    let power = source
        .power
        .map(|s| s.parse::<Quantity>())
        .transpose()
        .map_err(|e| unsupported("source.power", e))?;
    let toughness = source
        .toughness
        .map(|s| s.parse::<Quantity>())
        .transpose()
        .map_err(|e| unsupported("source.toughness", e))?;
    let mut labels = SourceLabels {
        text: source.oracle_text.clone(),
        blocks: result.lines.iter().map(|l| l.text.clone()).collect(),
        ..SourceLabels::default()
    };
    let spell_kind = matches!(kind, CardKind::Instant | CardKind::Sorcery);
    let mut definition = Definition {
        modal_land: None,
        transform_faces: None,
        source_labels: None,
        composed: true,
        x_mana_cost: false,
        oracle_id: r.oracle_identity,
        name: source.name,
        characteristics: Characteristics {
            kind,
            additional_types: types.additional_types,
            basic: types.basic,
            legendary: types.legendary,
            snow: types.snow,
            subtypes: types.subtypes,
            hybrid_mana: vec![],
            mana_cost: mana_cost(&source.mana_cost)?,
            colors: source.colors,
            color_identity: source.color_identity,
            power,
            toughness,
        },
        intrinsic_zone: (kind == CardKind::Land).then_some(FunctioningZone::Battlefield),
        spell: match kind {
            CardKind::Land => None,
            // CR 304.1, 307.1: ordinary timing follows the card type.
            CardKind::Instant => Some(SpellProgram::instructions(SpellTiming::Instant)),
            CardKind::Sorcery => Some(SpellProgram::instructions(SpellTiming::MainPhase)),
            // CR 302.1, 608.3: a permanent spell resolves onto the battlefield.
            _ => Some(SpellProgram::permanent(SpellTiming::MainPhase)),
        },
        activated: vec![],
        mana_abilities: vec![],
        triggered: vec![],
        static_abilities: vec![],
        keywords: vec![],
        capabilities: BTreeSet::new(),
        provenance: SourceProvenance {
            printing_id: source.id,
            source_index: r.source_index,
            source_sha256: r.source_sha256,
            ast_sha256,
            binding_sha256: r.binding_sha256,
            archive_manifest_sha256: archive.manifest_sha256.clone(),
            parser_sha256: archive.parser_source_sha256.clone(),
            source_json: r.source.get().into(),
            abilities_json: result.abilities.get().into(),
        },
    };
    let additional_types = definition.characteristics.additional_types.clone();
    let card = Card {
        kind,
        additional_types: &additional_types,
    };
    // CR 115.3: each printed block announces its own target occurrences.
    let mut slots = BTreeMap::new();
    for (block, raw) in blocks.iter().enumerate() {
        if let Some(found) = deferred.as_deref_mut() {
            found.block = Some(block);
        }
        let ability: Block = serde_json::from_str(raw.get())
            .map_err(|e| unsupported("compose.block", e.to_string()))?;
        let label = source_block(block)?;
        numbers_printed(&result.lines[block].text, raw.get())?;
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
                labels.statics.push(label);
                definition
                    .static_abilities
                    .push(lower_static(&effect, label)?);
            }
            Block::Spell {
                ability_word: word,
                effects,
            } if spell_kind => {
                ability_word(&word, &result.lines[block].text)?;
                sentences_accounted(&result.lines[block].text, &effects, false)?;
                slots.clear();
                append_effects(
                    definition.spell.as_mut().expect("spell program"),
                    parts(&effects)?,
                    block,
                    &mut slots,
                    |typed| validate_damage_sources(typed, DamageSourceBinding::Spell),
                )?;
            }
            Block::Activated {
                ability_word: word,
                restriction,
                costs,
                effects,
            } if !spell_kind => {
                ability_word(&word, &result.lines[block].text)?;
                sentences_accounted(&result.lines[block].text, &effects, restriction.is_some())?;
                let (timing, restriction) = timing(restriction)?;
                let parts = parts(&effects)?;
                // CR 605.1a: a mana ability is recognised from its complete
                // typed cost, production and restriction, never a table row.
                let typed: Option<Vec<Sentence>> = parts
                    .iter()
                    .map(|p| match p {
                        Part::Typed(s) => Some(s.clone()),
                        Part::Table(_) => None,
                    })
                    .collect();
                let mana = match &typed {
                    Some(typed) => {
                        lower_mana_ability(&costs, typed, restriction.as_ref(), block, kind)?
                    }
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
                        "activation restriction requires a reviewed family",
                    ));
                } else {
                    labels.activated.push(label);
                    definition.activated.push(activation(
                        &card,
                        &costs,
                        parts,
                        timing,
                        block,
                        &result.lines[block].text,
                    )?);
                }
            }
            Block::Triggered {
                ability_word: word,
                condition,
                trigger: event,
                effects,
            } if !spell_kind => {
                ability_word(&word, &result.lines[block].text)?;
                sentences_accounted(&result.lines[block].text, &effects, false)?;
                labels.triggered.push(label);
                definition.triggered.push(trigger(
                    &card,
                    &event,
                    parts(&effects)?,
                    condition.as_ref(),
                    block,
                )?);
            }
            _ => {
                return Err(unsupported(
                    "compose.kind",
                    "ability kind requires a reviewed family",
                ));
            }
        }
    }
    if let Some(found) = deferred {
        found.block = None;
    }
    keywords::printed(&shape, &definition.keywords)?;
    if definition.keywords.contains(&KeywordAbility::Flash)
        && let Some(spell) = definition.spell.as_mut()
    {
        spell.timing = SpellTiming::Instant;
    }
    definition.source_labels = Some(labels);
    definition.capabilities = definition.requirements();
    definition
        .validate()
        .map_err(|e| unsupported("composition", e))?;
    Ok(definition)
}
