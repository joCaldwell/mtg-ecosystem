//! Archive R4 resolution: the parser's authoritative, block-relative account
//! of every reference (closed `Ref` vocabulary), every printed step (its event
//! or structural scope) and every printed token (the span certificate). The
//! compositional lowerer reads these instead of rebuilding or pattern-matching
//! the diagnostic reference graph, and instead of counting printed punctuation.
//! Each check below rejects with the exact residual, role or field it found.
//!
//! CR 109.5, 113.7a, 201.5, 601.2c, 608.2c, 608.2h, 700.2 (pinned 2026-09-25).
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
            return Err(invalid("resolution", "completeness disagrees with residuals"));
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

    /// The shared step lowerer reads references the ordinary way: "you" is
    /// the block's controller (CR 109.5), "this" the source (CR 201.5), a
    /// target one of the block's announced targets (CR 601.2c), and another
    /// pronoun an announced target or a result of the block's own steps
    /// (CR 608.2c, 608.2h). Sets, filters and the resolving object itself are
    /// consumed operand by operand, and a damage source's pronoun is proven by
    /// its operator. Any other resolved role is named and rejected here.
    pub(super) fn ordinary_antecedents(&self, abilities: &Value) -> Result<(), Failure> {
        for (index, block) in self.blocks()?.iter().enumerate() {
            let ability = &abilities[index];
            let controller = match ability["kind"].as_str() {
                Some("spell") => Some(ControllerRole::SpellController),
                Some("activated" | "triggered") => Some(ControllerRole::AbilityController),
                Some("static") => Some(ControllerRole::StaticController),
                _ => None,
            };
            for entry in &block.refs {
                let (path, scope, resolved) = match entry {
                    RefEntry::Ref(r) => (&r.path, r.scope.as_deref(), Some(&*r.r#ref)),
                    RefEntry::Derived(d) => (&d.path, d.scope.as_deref(), None),
                    RefEntry::Evaluated(e) => (&e.path, e.scope.as_deref(), None),
                    RefEntry::Unresolved(u) => {
                        return Err(unsupported(
                            "resolution",
                            format!("ref:{}", u.unresolved),
                        ));
                    }
                };
                let leaves = || {
                    unsupported(
                        "resolution",
                        format!("ref {} outside the ordinary antecedents", describe(entry)),
                    )
                };
                let expression = ability.pointer(path);
                let effects_operand = path.starts_with("/effects/") && !path.ends_with("/of");
                // CR 109.5: every player resolved as a controller of this block,
                // printed "you" or an implicit payer, is the block's controller.
                if let Some(ResolvedRef::Controller(c)) = resolved
                    && scope.is_none()
                    && controller.is_some()
                    && Some(c.role) != controller
                {
                    return Err(leaves());
                }
                // CR 601.2c: a printed target (object or player) carries its
                // selection identity and resolves only to an announced target.
                if expression.is_some_and(|e| e.get("selectionId").is_some())
                    && !matches!(resolved, Some(ResolvedRef::Target(_)))
                {
                    return Err(leaves());
                }
                if expression == Some(&serde_json::json!({"player":"you"})) {
                    match resolved {
                        Some(ResolvedRef::Controller(c)) if Some(c.role) == controller => continue,
                        _ => return Err(leaves()),
                    }
                }
                let Some(kind) = expression.and_then(|e| e["ref"].as_str()) else {
                    continue;
                };
                let ordinary = match (kind, resolved) {
                    ("this", Some(ResolvedRef::Source(_))) => scope.is_none(),
                    ("this", _) => false,
                    ("target", Some(ResolvedRef::Target(_))) => scope.is_none(),
                    ("target", _) => false,
                    ("self" | "filter" | "each" | "all", _) => true,
                    _ if path.ends_with("/source") => true,
                    (_, Some(ResolvedRef::Target(_))) => {
                        scope.is_none_or(|s| s.starts_with("/effects/"))
                    }
                    (_, Some(ResolvedRef::StepResult(s))) => {
                        effects_operand && s.step.starts_with("/effects/")
                    }
                    (_, Some(ResolvedRef::BoundVar(v))) => {
                        effects_operand && v.id.starts_with("/effects/")
                    }
                    (_, Some(ResolvedRef::LinkedPool(p))) => {
                        effects_operand && p.id.starts_with("/effects/")
                    }
                    (_, None) => effects_operand,
                    _ => false,
                };
                if !ordinary {
                    return Err(leaves());
                }
            }
        }
        Ok(())
    }

    /// The resolution and the AST describe the same clauses: every
    /// reference addresses an AST operand (or an implicit operand of an
    /// existing object), and the printed steps are exactly the step entries,
    /// each under its own AST discriminant. A clause removed, added or
    /// changed after resolution leaves a dangling or missing entry.
    pub(super) fn covers(&self, abilities: &Value) -> Result<(), Failure> {
        let blocks = self.blocks()?;
        let printed = abilities
            .as_array()
            .ok_or_else(|| invalid("abilities", "expected blocks"))?;
        if printed.len() != blocks.len() {
            return Err(unsupported("resolution", "resolution does not cover the AST blocks"));
        }
        for (ability, block) in printed.iter().zip(blocks) {
            let mismatch = |what: &str, path: &str| {
                unsupported("resolution", format!("{what} {path} does not cover the AST"))
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
                    StepEntry::Event(e) => {
                        (&e.path, e.ast_effect.as_str(), e.event.as_str() == e.ast_effect.as_str())
                    }
                    StepEntry::Scope(s) => (&s.path, s.ast_effect.as_str(), true),
                    StepEntry::Unaccounted(u) => (&u.path, u.ast_effect.as_str(), false),
                };
                if steps.get(path.as_str()).map(String::as_str) != Some(effect) {
                    return Err(mismatch("step", path));
                }
                if !accounted {
                    return Err(unsupported("resolution", format!("step:{effect}")));
                }
            }
        }
        Ok(())
    }

    /// CR 201.5: an archived `{"ref":"self"}` resolved to this exact source
    /// becomes the lowerer's `{"ref":"this","noun":N}`. A damage source's
    /// `self` keeps its encoding; its operator lowers it to this incarnation.
    pub(super) fn canonical_abilities(&self, raw: &str, noun: &str) -> Result<String, Failure> {
        let blocks = self.blocks()?;
        let list: Vec<Box<RawValue>> = decode(raw, "abilities")?;
        let mut out = Vec::with_capacity(list.len());
        for (index, ability) in list.iter().enumerate() {
            let block = blocks
                .get(index)
                .ok_or_else(|| unsupported("resolution", "resolution does not cover the AST blocks"))?;
            out.push(canonical(ability.get(), "", block, noun)?);
        }
        Ok(format!("[{}]", out.join(",")))
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
                return Err(unsupported("resolution", format!("spans:{}", u.unsupported)));
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
                return Err(wrong("spans are unsorted, empty, overlapping or out of range"));
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
            return Err(wrong("a printed sentence and an AST sentence do not correspond"));
        }
        Ok(())
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
fn canonical(raw: &str, path: &str, block: &BlockResolution, noun: &str) -> Result<String, Failure> {
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

/// `cfg(test)` synthetic rows only: every non-damage-source `self` is this
/// source, as an archived resolution would certify for a single-face card.
pub(super) fn synthetic_canonical(raw: &str, noun: &str) -> String {
    let mut value: Value = serde_json::from_str(raw).expect("synthetic abilities");
    fn visit(value: &mut Value, path: &str, noun: &str) {
        if *value == serde_json::json!({"ref":"self"}) && !path.ends_with("/source") {
            *value = serde_json::json!({"ref":"this","noun":noun});
        } else if let Some(items) = value.as_array_mut() {
            for (i, item) in items.iter_mut().enumerate() {
                visit(item, &format!("{path}/{i}"), noun);
            }
        } else if let Some(fields) = value.as_object_mut() {
            for (key, child) in fields.iter_mut() {
                visit(child, &format!("{path}/{key}"), noun);
            }
        }
    }
    visit(&mut value, "", noun);
    value.to_string()
}
