//! Archive R4 resolution: the parser's authoritative, block-relative account
//! of every reference (closed `Ref` vocabulary), every printed step (its event
//! or structural scope) and every printed token (the span certificate). The
//! compositional lowerer reads these instead of rebuilding or pattern-matching
//! the diagnostic reference graph, and instead of counting printed punctuation.
//! Each check below rejects with the exact residual, role or field it found.
//!
//! CR 102.1, 109.5, 113.7a, 201.5, 601.2c, 603.2, 608.2c, 608.2h, 700.2 (pinned 2026-09-25).
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;
use crate::ast::{
    BlockResolution, BlockResolutionSpans, CardResolution, ControllerRole, RefEntry, ResolvedRef,
    SpanSegment, StepEntry,
};
use serde_json::Value;

/// The verified resolution of a single-face record's blocks.
pub(super) struct Resolution {
    card: CardResolution,
}

impl Resolution {
    /// Decode and hash-check the record's resolution. Its absence means the
    /// archive predates R4; composition never reads the diagnostic graph.
    pub(super) fn read(raw: Option<&RawValue>, sha: Option<&str>) -> Result<Self, Failure> {
        let (Some(raw), Some(sha)) = (raw, sha) else {
            return Err(unsupported(
                "resolution",
                "composition requires an archive R4 resolution",
            ));
        };
        if sha256(raw.get().as_bytes()) != sha {
            return Err(invalid("resolution", "resolution hash mismatch"));
        }
        let card: CardResolution = decode(raw.get(), "resolution")?;
        let residual_free = card.residuals.is_empty()
            && card.faces.iter().all(|f| {
                f.blocks
                    .as_ref()
                    .is_some_and(|b| b.iter().all(|b| b.residuals.is_empty()))
            });
        if card.complete != residual_free {
            return Err(invalid(
                "resolution",
                "completeness disagrees with residuals",
            ));
        }
        Ok(Self { card })
    }

    /// The blocks of the one parsed face, in printed order.
    fn blocks(&self) -> Result<&[BlockResolution], Failure> {
        match self.card.faces.as_slice() {
            [face] if face.source_face_index == 0 => face
                .blocks
                .as_deref()
                .ok_or_else(|| unsupported("resolution", "face:unparsed")),
            _ => Err(invalid("resolution", "expected exactly one resolved face")),
        }
    }

    /// Every reference resolved in the closed vocabulary, every step
    /// accounted for and every token attributed; otherwise the first exact
    /// residual (card-level first, then by block).
    pub(super) fn complete(&self) -> Result<(), Failure> {
        let first = self.card.residuals.first().or_else(|| {
            self.card
                .faces
                .iter()
                .flat_map(|f| f.blocks.iter().flatten())
                .find_map(|b| b.residuals.first())
        });
        match first {
            Some(residual) => Err(unsupported("resolution", residual)),
            None => Ok(()),
        }
    }

    /// Every resolved reference of every block, looked up in the reference
    /// table ([`reference_row`]): its role, read where it is printed, either
    /// has a lowering or is rejected under `ref:<role>:<block kind>`.
    pub(super) fn references(&self, abilities: &Value) -> Result<(), Failure> {
        for (index, block) in self.blocks()?.iter().enumerate() {
            let ability = &abilities[index];
            let kind = BlockKind::of(ability);
            for entry in &block.refs {
                let (path, scope, role) = match entry {
                    RefEntry::Ref(r) => (&r.path, r.scope.as_deref(), Role::Resolved(&r.r#ref)),
                    RefEntry::Derived(d) => (&d.path, d.scope.as_deref(), Role::Unbound),
                    RefEntry::Evaluated(e) => (&e.path, e.scope.as_deref(), Role::Unbound),
                    RefEntry::Unresolved(u) => {
                        return Err(unsupported("resolution", format!("ref:{}", u.unresolved)));
                    }
                };
                let site = Site {
                    kind,
                    ability,
                    block,
                    path,
                    scope,
                    operand: ability.pointer(path),
                };
                if !reference_row(role, &site) {
                    return Err(unsupported(
                        "resolution",
                        format!("ref:{}:{}", describe(entry), kind.name()),
                    ));
                }
            }
        }
        Ok(())
    }

    /// CR 608.2c: every reference operand the AST prints (`{"ref":..}` or
    /// `{"player":..}`) carries its resolution entry; a deleted entry would
    /// otherwise leave the lowerer reading the operand by shape alone.
    pub(super) fn referenced(&self, abilities: &Value) -> Result<(), Failure> {
        for (index, block) in self.blocks()?.iter().enumerate() {
            let resolved: BTreeSet<&str> = block.refs.iter().map(entry_path).collect();
            let mut operands = Vec::new();
            collect_operands(&abilities[index], String::new(), &mut operands);
            if let Some(path) = operands.iter().find(|p| !resolved.contains(p.as_str())) {
                return Err(unsupported(
                    "resolution",
                    format!("unresolved reference operand {path}"),
                ));
            }
        }
        Ok(())
    }

    /// CR 601.2c, 608.2b: each printed target of block `index` is the slot
    /// the resolution assigns it, and a pronoun resolved to a target names
    /// the block's sole announced target, the only one the lowerer binds a
    /// pronoun to. `targets` are the block's lowered announcements.
    pub(super) fn target_slots(
        &self,
        index: usize,
        ability: &Value,
        targets: &[TargetSpec],
    ) -> Result<(), Failure> {
        let block = self
            .blocks()?
            .get(index)
            .ok_or_else(|| invalid("resolution", "missing block"))?;
        for entry in &block.refs {
            let RefEntry::Ref(r) = entry else { continue };
            let ResolvedRef::Target(t) = &*r.r#ref else {
                continue;
            };
            let expression = ability.pointer(&r.path);
            let announced = match expression.and_then(|e| e["selectionId"].as_str()) {
                Some(selection) => targets.iter().find(|s| s.selection_id == selection),
                None => match targets {
                    [sole] => Some(sole),
                    _ => None,
                },
            };
            if announced.is_none_or(|s| i64::from(s.slot) != t.slot) {
                return Err(unsupported(
                    "resolution",
                    format!(
                        "target reference {} disagrees with the announced slots",
                        r.path
                    ),
                ));
            }
        }
        Ok(())
    }

    /// The resolution and the AST describe the same clauses: every
    /// reference addresses an AST operand (or an implicit operand of an
    /// existing object), and the printed steps are exactly the step entries,
    /// each under its own AST discriminant. A clause removed, added or
    /// changed after resolution leaves a dangling or missing entry. Returns
    /// every step the resolution does not account for as (block, path,
    /// effect); composition rejects at the first and the gap report names
    /// them all.
    pub(super) fn unaccounted_steps(
        &self,
        abilities: &Value,
    ) -> Result<Vec<(usize, String, String)>, Failure> {
        let mut unaccounted = Vec::new();
        let blocks = self.blocks()?;
        let printed = abilities
            .as_array()
            .ok_or_else(|| invalid("abilities", "expected blocks"))?;
        if printed.len() != blocks.len() {
            return Err(unsupported(
                "resolution",
                "resolution does not cover the AST blocks",
            ));
        }
        for (index, (ability, block)) in printed.iter().zip(blocks).enumerate() {
            let mismatch = |what: &str, path: &str| {
                unsupported(
                    "resolution",
                    format!("{what} {path} does not cover the AST"),
                )
            };
            let mut seen = BTreeSet::new();
            for entry in &block.refs {
                let path = entry_path(entry);
                let parent = path.rsplit_once('/').map_or("", |(parent, _)| parent);
                if !seen.insert(path)
                    || (ability.pointer(path).is_none()
                        && !ability.pointer(parent).is_some_and(Value::is_object))
                {
                    return Err(mismatch("reference", path));
                }
            }
            let mut steps = BTreeMap::new();
            collect_steps(ability, String::new(), &mut steps);
            if steps.len() != block.steps.len() {
                return Err(mismatch("step map", ""));
            }
            for entry in &block.steps {
                let (path, effect, accounted) = match entry {
                    StepEntry::Event(e) => (
                        &e.path,
                        e.ast_effect.as_str(),
                        e.event.as_str() == e.ast_effect.as_str(),
                    ),
                    StepEntry::Scope(s) => (&s.path, s.ast_effect.as_str(), true),
                    StepEntry::Unaccounted(u) => (&u.path, u.ast_effect.as_str(), false),
                };
                if steps.get(path.as_str()).map(String::as_str) != Some(effect) {
                    return Err(mismatch("step", path));
                }
                if !accounted {
                    unaccounted.push((index, path.clone(), effect.to_string()));
                }
            }
        }
        Ok(unaccounted)
    }

    /// CR 201.5: an archived `{"ref":"self"}` resolved to this exact source
    /// becomes the lowerer's `{"ref":"this","noun":N}`. A damage source's
    /// `self` keeps its encoding; its operator lowers it to this incarnation.
    pub(super) fn canonical_abilities(&self, raw: &str, noun: &str) -> Result<String, Failure> {
        let blocks = self.blocks()?;
        let list: Vec<Box<RawValue>> = decode(raw, "abilities")?;
        let mut out = Vec::with_capacity(list.len());
        for (index, ability) in list.iter().enumerate() {
            let block = blocks.get(index).ok_or_else(|| {
                unsupported("resolution", "resolution does not cover the AST blocks")
            })?;
            out.push(canonical(ability.get(), "", block, noun)?);
        }
        Ok(format!("[{}]", out.join(",")))
    }

    /// The certified segments of block `index`: the AST fields each printed
    /// byte range produced, in printed order. Trivia (a modal bullet) names
    /// no field. [`Resolution::spans`] has checked the tiling.
    pub(super) fn segments(
        &self,
        index: usize,
    ) -> Result<Vec<(Vec<String>, usize, usize)>, Failure> {
        let block = self
            .blocks()?
            .get(index)
            .ok_or_else(|| invalid("resolution", "missing block"))?;
        let BlockResolutionSpans::Certificate(c) = &*block.spans else {
            return Err(unsupported("resolution", "spans:uncertified"));
        };
        let offset = |n: i64| usize::try_from(n).map_err(|_| invalid("resolution.spans", "offset"));
        c.certificate
            .segments
            .iter()
            .filter_map(|segment| match segment {
                SpanSegment::Fields(f) => Some((f.fields.clone(), f.start, f.end)),
                SpanSegment::Trivia(_) => None,
            })
            .map(|(fields, start, end)| Ok((fields, offset(start)?, offset(end)?)))
            .collect()
    }

    /// Whether block `index` records any reference entry at `path`.
    pub(super) fn binds(&self, index: usize, path: &str) -> Result<bool, Failure> {
        let block = self
            .blocks()?
            .get(index)
            .ok_or_else(|| invalid("resolution", "missing block"))?;
        Ok(block.refs.iter().any(|entry| entry_path(entry) == path))
    }

    /// CR 109.5, 603.2, 608.2d: the kernel player a printed player operand
    /// of block `index` at `path` resolves to: the block's own controller
    /// ("you") or a kernel event player of its own trigger. Any other
    /// resolution, or none, is no control chooser or payer.
    pub(super) fn control_player(
        &self,
        index: usize,
        ability: &Value,
        path: &str,
        context: kir::Context,
    ) -> Result<Option<kir::PlayerExpr>, Failure> {
        let block = self
            .blocks()?
            .get(index)
            .ok_or_else(|| invalid("resolution", "missing block"))?;
        let kind = BlockKind::of(ability);
        Ok(block.refs.iter().find_map(|entry| match entry {
            RefEntry::Ref(r) if r.path == path && r.scope.is_none() => match &*r.r#ref {
                ResolvedRef::Controller(c) if Some(c.role) == kind.controller() => {
                    Some(kir::PlayerExpr::you(context))
                }
                ResolvedRef::EventPlayer(e) if kind == BlockKind::Triggered => {
                    kernel_event_player(&e.role)
                        .filter(|(_, trigger)| ability["trigger"]["trigger"] == *trigger)
                        .map(|(role, _)| kir::PlayerExpr::event(role))
                }
                _ => None,
            },
            _ => None,
        }))
    }

    /// The span certificate tiles the normalized block text: segments are
    /// sorted, disjoint UTF-8 ranges on character boundaries, separated only
    /// by whitespace; every field they name exists in the block AST; every
    /// top-level sentence has a segment of its own; a bullet mark is the only
    /// trivia; an ability word is printed as `Word —`.
    pub(super) fn spans(&self, index: usize, text: &str, ability: &Value) -> Result<(), Failure> {
        let block = self
            .blocks()?
            .get(index)
            .ok_or_else(|| unsupported("resolution", "resolution does not cover the AST blocks"))?;
        let certificate = match &*block.spans {
            BlockResolutionSpans::Certificate(c) => &c.certificate,
            BlockResolutionSpans::Unsupported(u) => {
                return Err(unsupported(
                    "resolution",
                    format!("spans:{}", u.unsupported),
                ));
            }
        };
        let wrong = |why: &str| unsupported("resolution.spans", why.to_owned());
        if usize::try_from(certificate.bytes).ok() != Some(text.len()) {
            return Err(wrong("certificate length differs from the block text"));
        }
        let mut cursor = 0usize;
        let mut sentences = BTreeSet::new();
        // The top-level block fields some span names ("" names them all).
        let mut printed_fields = BTreeSet::new();
        for segment in &certificate.segments {
            let (start, end) = match segment {
                SpanSegment::Trivia(t) => (t.start, t.end),
                SpanSegment::Fields(f) => (f.start, f.end),
            };
            let (Ok(start), Ok(end)) = (usize::try_from(start), usize::try_from(end)) else {
                return Err(wrong("negative span offset"));
            };
            if start < cursor || end <= start || end > text.len() {
                return Err(wrong(
                    "spans are unsorted, empty, overlapping or out of range",
                ));
            }
            let (Some(gap), Some(printed)) = (text.get(cursor..start), text.get(start..end)) else {
                return Err(wrong("span offset splits a character"));
            };
            if !gap.chars().all(char::is_whitespace) {
                return Err(wrong("printed text outside every span"));
            }
            match segment {
                SpanSegment::Trivia(_) => {
                    if !matches!(printed, "•" | "+") {
                        return Err(wrong("trivia that is not a modal bullet"));
                    }
                }
                SpanSegment::Fields(f) => {
                    if f.fields.is_empty() {
                        return Err(wrong("span names no AST field"));
                    }
                    for field in &f.fields {
                        if ability.pointer(field).is_none() {
                            return Err(wrong("span names a missing AST field"));
                        }
                        printed_fields.insert(field.split('/').nth(1).unwrap_or("").to_owned());
                    }
                    if let [field] = f.fields.as_slice() {
                        if let Some(i) = field
                            .strip_prefix("/effects/")
                            .filter(|i| i.bytes().all(|b| b.is_ascii_digit()))
                        {
                            sentences.insert(i.parse::<usize>().map_err(|_| wrong("index"))?);
                        }
                        if field == "/abilityWord"
                            && ability["abilityWord"]
                                .as_str()
                                .is_none_or(|word| printed != format!("{word} —"))
                        {
                            return Err(wrong("ability word span differs from the AST"));
                        }
                    }
                }
            }
            cursor = end;
        }
        if !text[cursor..].chars().all(char::is_whitespace) {
            return Err(wrong("printed text outside every span"));
        }
        // A block field no printed token produced (an added restriction, a
        // condition) is not the printed block.
        if !printed_fields.contains("")
            && ability.as_object().is_some_and(|fields| {
                fields
                    .keys()
                    .any(|key| key != "kind" && !printed_fields.contains(key))
            })
        {
            return Err(wrong("an AST field has no printed text"));
        }
        let effects = ability["effects"].as_array().map_or(0, Vec::len);
        if sentences != (0..effects).collect() {
            return Err(wrong(
                "a printed sentence and an AST sentence do not correspond",
            ));
        }
        Ok(())
    }
}

/// CR 102.1, 601.2, 602.2, 603.2: the kernel's event-player roles, each with
/// the one trigger discriminant whose event supplies it. The parser spells a
/// role in this vocabulary only for that trigger; any other event player
/// keeps its trigger's discriminant and is no kernel role.
pub(super) fn kernel_event_player(role: &str) -> Option<(kir::EventPlayerRole, &'static str)> {
    match role {
        "active-player" => Some((kir::EventPlayerRole::ActivePlayer, "phase")),
        "caster" => Some((kir::EventPlayerRole::Caster, "cast")),
        "activating-player" => Some((kir::EventPlayerRole::ActivatingPlayer, "activate")),
        _ => None,
    }
}

/// The printed block kind a reference sits in (CR 113.3).
#[derive(Clone, Copy, PartialEq, Eq)]
enum BlockKind {
    Spell,
    Activated,
    Triggered,
    Static,
    Other,
}

impl BlockKind {
    fn of(ability: &Value) -> Self {
        match ability["kind"].as_str() {
            Some("spell") => Self::Spell,
            Some("activated") => Self::Activated,
            Some("triggered") => Self::Triggered,
            Some("static") => Self::Static,
            _ => Self::Other,
        }
    }

    fn name(self) -> &'static str {
        match self {
            Self::Spell => "spell",
            Self::Activated => "activated",
            Self::Triggered => "triggered",
            Self::Static => "static",
            Self::Other => "keywords",
        }
    }

    /// CR 109.5: whose "you" a controller reference in this block means.
    fn controller(self) -> Option<ControllerRole> {
        match self {
            Self::Spell => Some(ControllerRole::SpellController),
            Self::Activated | Self::Triggered => Some(ControllerRole::AbilityController),
            Self::Static => Some(ControllerRole::StaticController),
            Self::Other => None,
        }
    }
}

/// A reference entry's role: resolved in the closed vocabulary, or a
/// derived/evaluated entry with no antecedent object of its own.
#[derive(Clone, Copy)]
enum Role<'a> {
    Resolved(&'a ResolvedRef),
    Unbound,
}

/// Where one reference is printed.
struct Site<'a> {
    kind: BlockKind,
    ability: &'a Value,
    block: &'a BlockResolution,
    path: &'a str,
    scope: Option<&'a str>,
    /// The AST operand at `path`, when it exists.
    operand: Option<&'a Value>,
}

impl Site<'_> {
    /// A resolving-effect operand, not the owner object of a relation.
    fn effects_operand(&self) -> bool {
        self.path.starts_with("/effects/") && !self.path.ends_with("/of")
    }
    /// The `ref` word the AST prints here.
    fn word(&self) -> Option<&str> {
        self.operand.and_then(|e| e["ref"].as_str())
    }
    /// A printed target carries its selection identity (CR 601.2c).
    fn selection(&self) -> bool {
        self.operand.is_some_and(|e| e.get("selectionId").is_some())
    }
    /// The bare printed player "you".
    fn you(&self) -> bool {
        self.operand.is_some_and(|e| {
            e.as_object()
                .is_some_and(|o| o.len() == 1 && o.get("player") == Some(&Value::from("you")))
        })
    }
}

/// The reference table: `(role, site) → lowering or rejection`. A row
/// returning true means the shared operand lowerers read the reference at
/// this site: "you" is the block's controller (CR 109.5), "this" the source
/// (CR 201.5), a target one of the block's announced targets (CR 601.2c),
/// another pronoun an announced target or a result of the block's own steps
/// (CR 608.2c, 608.2h); sets, filters and the resolving object itself are
/// consumed operand by operand and a damage source's pronoun is proven by
/// its operator. A role a wave opens changes its own row only.
fn reference_row(role: Role<'_>, at: &Site<'_>) -> bool {
    let resolved = match role {
        Role::Resolved(r) => Some(r),
        Role::Unbound => None,
    };
    // Rows every operand word shares: a printed selection is an announced
    // target, a bare "you" the block's controller, and a pronoun an
    // announced target or an earlier step's result (CR 601.2c, 608.2c).
    if at.selection() && !matches!(resolved, Some(ResolvedRef::Target(_))) {
        return false;
    }
    if at.you() {
        return matches!(resolved, Some(ResolvedRef::Controller(c)) if Some(c.role) == at.kind.controller())
            && controller_row(resolved, at);
    }
    let word = at.word();
    if word.is_some_and(|w| PRONOUNS.contains(&w))
        && !matches!(
            resolved,
            Some(ResolvedRef::Target(_) | ResolvedRef::StepResult(_))
        )
    {
        return false;
    }
    match resolved {
        None => word_row(word, at, |at| at.effects_operand()),
        Some(ResolvedRef::Source(_)) => match word {
            None => true,
            Some("this") => at.scope.is_none(),
            Some("target") => false,
            _ => shared_word(word, at),
        },
        Some(ResolvedRef::Target(_)) => match word {
            None => true,
            Some("this") => false,
            Some("target") => at.scope.is_none(),
            _ if shared_word(word, at) => true,
            _ => at.scope.is_none_or(|s| s.starts_with("/effects/")),
        },
        // CR 608.2c: "the rest" names exactly a remainder.
        Some(ResolvedRef::StepResult(s)) => {
            word.is_none_or(|w| (w == "rest") == (s.role.as_str() == "remainder"))
                && word_row(word, at, |at| {
                    at.effects_operand() && step_producer(at.block, at.ability, at.path, s)
                })
        }
        Some(ResolvedRef::BoundVar(v)) => word_row(word, at, |at| {
            at.effects_operand() && v.id.starts_with("/effects/")
        }),
        Some(ResolvedRef::LinkedPool(p)) => word_row(word, at, |at| {
            at.effects_operand() && p.id.starts_with("/effects/")
        }),
        Some(ResolvedRef::Controller(_)) => {
            controller_row(resolved, at) && word_row(word, at, |_| false)
        }
        Some(ResolvedRef::EventPlayer(e)) => {
            // CR 603.2: a kernel event player belongs to the block's own
            // trigger, the one trigger that supplies it.
            kernel_event_player(&e.role).is_none_or(|(_, trigger)| {
                at.scope.is_none()
                    && at.kind == BlockKind::Triggered
                    && at.ability["trigger"]["trigger"] == trigger
            }) && word_row(word, at, |_| false)
        }
        // Opened by their waves: attachments (W1), as-enters (W2) and
        // trigger event objects (W7) have no lowering yet; the announced X
        // is read by its amount operand only.
        Some(
            ResolvedRef::AttachedTo(_)
            | ResolvedRef::EventObject(_)
            | ResolvedRef::EnteringObject(_)
            | ResolvedRef::AnnouncedX(_),
        ) => word_row(word, at, |_| false),
    }
}

/// CR 109.5: a controller resolved with no scope in a block with a
/// controller is that block's controller.
fn controller_row(resolved: Option<&ResolvedRef>, at: &Site<'_>) -> bool {
    match resolved {
        Some(ResolvedRef::Controller(c)) if at.scope.is_none() => {
            at.kind.controller().is_none_or(|own| c.role == own)
        }
        _ => true,
    }
}

/// Words every role reads the same way: a set, filter or self operand, and
/// the source operand of a damage step.
fn shared_word(word: Option<&str>, at: &Site<'_>) -> bool {
    matches!(word, Some("self" | "filter" | "each" | "all")) || at.path.ends_with("/source")
}

/// A role read through its operand word: no word, or a shared word, is
/// read operand by operand; "this" and "target" name their own roles;
/// any other word follows the role's own `rest` rule.
fn word_row(word: Option<&str>, at: &Site<'_>, rest: impl Fn(&Site<'_>) -> bool) -> bool {
    match word {
        None => true,
        Some("this" | "target") => false,
        _ if shared_word(word, at) => true,
        _ => rest(at),
    }
}

fn entry_path(entry: &RefEntry) -> &str {
    match entry {
        RefEntry::Ref(r) => &r.path,
        RefEntry::Derived(d) => &d.path,
        RefEntry::Evaluated(e) => &e.path,
        RefEntry::Unresolved(u) => &u.path,
    }
}

/// The resolved role, for a rejection that names it.
fn describe(entry: &RefEntry) -> String {
    match entry {
        RefEntry::Ref(r) => match &*r.r#ref {
            ResolvedRef::Source(_) => "source".into(),
            ResolvedRef::Controller(c) => format!("controller:{}", c.role.as_str()),
            ResolvedRef::Target(_) => "target".into(),
            ResolvedRef::AttachedTo(_) => "attached-to".into(),
            ResolvedRef::EventObject(e) => format!("event-object:{}", e.role),
            ResolvedRef::EventPlayer(e) => format!("event-player:{}", e.role),
            ResolvedRef::EnteringObject(_) => "entering-object".into(),
            ResolvedRef::StepResult(s) => format!("step-result:{}", s.role.as_str()),
            ResolvedRef::BoundVar(_) => "bound-var".into(),
            ResolvedRef::LinkedPool(_) => "linked-pool".into(),
            ResolvedRef::AnnouncedX(_) => "announced-x".into(),
        },
        RefEntry::Derived(d) => format!("derived:{}", d.derived.relation.as_str()),
        RefEntry::Evaluated(_) => "evaluated".into(),
        RefEntry::Unresolved(u) => format!("unresolved:{}", u.unresolved),
    }
}

/// Every step object (an object with an `effect` in a `steps` array), by its
/// block-relative path: the same rule the parser's resolution uses.
fn collect_steps(value: &Value, path: String, out: &mut BTreeMap<String, String>) {
    match value {
        Value::Array(items) => {
            for (i, item) in items.iter().enumerate() {
                collect_steps(item, format!("{path}/{i}"), out);
            }
        }
        Value::Object(fields) => {
            for (key, child) in fields {
                let child_path = format!("{path}/{}", key.replace('~', "~0").replace('/', "~1"));
                if key == "steps"
                    && let Some(steps) = child.as_array()
                {
                    for (i, step) in steps.iter().enumerate() {
                        if let Some(effect) = step.get("effect").and_then(Value::as_str) {
                            out.insert(format!("{child_path}/{i}"), effect.to_owned());
                        }
                    }
                }
                collect_steps(child, child_path, out);
            }
        }
        _ => {}
    }
}

/// Rewrite one block's JSON in its original field order.
fn canonical(
    raw: &str,
    path: &str,
    block: &BlockResolution,
    noun: &str,
) -> Result<String, Failure> {
    #[derive(Default)]
    struct OrderedObject(Vec<(String, Box<RawValue>)>);
    impl<'de> Deserialize<'de> for OrderedObject {
        fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
            struct Visitor;
            impl<'de> serde::de::Visitor<'de> for Visitor {
                type Value = OrderedObject;
                fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                    f.write_str("object")
                }
                fn visit_map<M: serde::de::MapAccess<'de>>(
                    self,
                    mut map: M,
                ) -> Result<Self::Value, M::Error> {
                    let mut fields = vec![];
                    while let Some(field) = map.next_entry()? {
                        fields.push(field);
                    }
                    Ok(OrderedObject(fields))
                }
            }
            d.deserialize_map(Visitor)
        }
    }
    let node: Value = decode(raw, "abilities")?;
    if node == serde_json::json!({"ref":"self"}) {
        if path.ends_with("/source") {
            return Ok(raw.to_owned());
        }
        let source = block.refs.iter().any(|entry| {
            matches!(entry, RefEntry::Ref(r) if r.path == path && r.scope.is_none()
                && matches!(&*r.r#ref, ResolvedRef::Source(_)))
        });
        if !source {
            return Err(unsupported(path, "ref:self:unbound_source"));
        }
        return Ok(serde_json::json!({"ref":"this", "noun":noun}).to_string());
    }
    if node.is_array() {
        let items: Vec<Box<RawValue>> = decode(raw, "abilities")?;
        let values = items
            .iter()
            .enumerate()
            .map(|(i, v)| canonical(v.get(), &format!("{path}/{i}"), block, noun))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(format!("[{}]", values.join(",")))
    } else if node.is_object() {
        let object: OrderedObject = decode(raw, "abilities")?;
        let values = object
            .0
            .iter()
            .map(|(key, v)| {
                let child = format!("{path}/{}", key.replace('~', "~0").replace('/', "~1"));
                Ok(format!(
                    "{}:{}",
                    serde_json::to_string(key).expect("string key"),
                    canonical(v.get(), &child, block, noun)?
                ))
            })
            .collect::<Result<Vec<_>, Failure>>()?;
        Ok(format!("{{{}}}", values.join(",")))
    } else {
        Ok(raw.to_owned())
    }
}

/// AST reference words that name an antecedent rather than a set, the
/// source or a printed target.
const PRONOUNS: [&str; 4] = ["it", "that", "them", "rest"];

/// Every operand object that names a reference (`ref`) or a player.
fn collect_operands(value: &Value, path: String, out: &mut Vec<String>) {
    match value {
        Value::Object(object) => {
            if object.get("ref").is_some_and(Value::is_string)
                || object.get("player").is_some_and(Value::is_string)
            {
                out.push(path.clone());
            }
            for (key, child) in object {
                // Scalar player relations the archive always resolves: a
                // controlled or owned zone/object and an owner's destination.
                if matches!(
                    (key.as_str(), child.as_str()),
                    ("control", Some("you" | "opponent"))
                        | ("owner" | "whose", Some("your"))
                        | ("to", Some("owner"))
                ) {
                    out.push(format!("{path}/{key}"));
                }
                collect_operands(child, format!("{path}/{key}"), out);
            }
        }
        Value::Array(items) => {
            for (i, child) in items.iter().enumerate() {
                collect_operands(child, format!("{path}/{i}"), out);
            }
        }
        _ => {}
    }
}

/// CR 608.2c: the step result a pronoun names is the one the shared lowerer
/// binds it to. A selection is the found card of the nearest earlier search
/// (`<step>/for`), objects are the nearest earlier look; a remainder is the
/// rest of the consumer's own simultaneous member pool after an earlier
/// sibling. Any other producer or role is not one the lowerer reads.
fn step_producer(
    block: &BlockResolution,
    ability: &Value,
    consumer: &str,
    result: &crate::ast::ResolvedRefStepResult,
) -> bool {
    let order: Vec<&str> = block
        .steps
        .iter()
        .map(|entry| match entry {
            StepEntry::Event(e) => e.path.as_str(),
            StepEntry::Scope(s) => s.path.as_str(),
            StepEntry::Unaccounted(u) => u.path.as_str(),
        })
        .collect();
    // The consumer's own step: the longest step path enclosing the operand.
    let Some(own) = order
        .iter()
        .enumerate()
        .filter(|(_, p)| consumer.starts_with(&format!("{p}/")))
        .max_by_key(|(_, p)| p.len())
        .map(|(i, _)| i)
    else {
        return false;
    };
    let effect = |i: usize| ability.pointer(order[i]).and_then(|s| s["effect"].as_str());
    let (step, wanted) = match result.role.as_str() {
        "selection" => match result.step.strip_suffix("/for") {
            Some(step) => (step, "search"),
            None => return false,
        },
        "objects" => (result.step.as_str(), "look-at"),
        "remainder" => {
            // The rest of a pool another member of the same simultaneous step
            // took from first ("one of them ... and the rest").
            let Some(parent) = order[own].rfind("/steps/").map(|i| &order[own][..i]) else {
                return false;
            };
            let simultaneous =
                ability.pointer(parent).and_then(|s| s["effect"].as_str()) == Some("simultaneous");
            let earlier_pool = order[..own].iter().any(|p| {
                p.strip_prefix(parent)
                    .and_then(|rest| rest.strip_prefix("/steps/"))
                    .is_some_and(|rest| !rest.contains('/'))
                    && ability.pointer(&format!("{p}/what/ref")) == Some(&Value::from("from-pool"))
            });
            return result.step == order[own] && simultaneous && earlier_pool;
        }
        _ => return false,
    };
    let Some(producer) = order.iter().position(|p| *p == step) else {
        return false;
    };
    producer < own
        && effect(producer) == Some(wanted)
        && (producer + 1..own).all(|i| effect(i) != Some(wanted))
}
