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

/// The verified resolution of a card, with a selected face's printed blocks.
pub(super) struct Resolution {
    card: CardResolution,
    face: usize,
    spend_riders: BTreeSet<(usize, usize)>,
    control: BTreeMap<(usize, usize), ControlCoverage>,
}

#[derive(Default)]
struct ControlCoverage {
    references: BTreeSet<String>,
    steps: BTreeSet<String>,
}

const SPEND_RIDER_STEP: &str = "/effects/0/steps/0/riders/0/effects/0/steps/0";
const SPEND_RIDER_REF: &str = "/effects/0/steps/0/riders/0/effects/0/steps/0/what";

fn draw_players_after_draw(ability: &Value, path: &str) -> bool {
    let Some((steps, suffix)) = path.rsplit_once("/steps/") else {
        return false;
    };
    let Some(index) = suffix
        .strip_suffix("/who")
        .and_then(|i| i.parse::<usize>().ok())
        .and_then(|i| i.checked_sub(1))
    else {
        return false;
    };
    let Some(who) = ability.pointer(path) else {
        return false;
    };
    matches!(typed::<ast::PlayerRef>(who, "player"),
        Ok(ast::PlayerRef::Filtered(who))
            if matches!(&*who.base, ast::PlayerRef::EachPlayer(_))
                && who.random.is_none()
                && crate::rules::card_result::reads_draw_players(&who.filter))
        && ability
            .pointer(&format!("{steps}/steps/{index}"))
            .is_some_and(|step| step["effect"] == "draw" && step["who"]["player"] == "each-player")
}

fn counted_iteration_site(ability: &Value, path: &str, fields: &[&str]) -> bool {
    fields.iter().any(|field| {
        path.strip_suffix(field)
            .and_then(|parent| ability.pointer(parent))
            .and_then(super::compose::control::counted_step)
            .is_some()
    })
}

/// CR 608.2d, 205.3m: the local creature type choice used by the next
/// instruction. The evaluator independently proves the type and scope.
fn local_creature_type_choice(site: &Site<'_>, result: &ast::ResolvedRefStepResult) -> bool {
    if result.role.as_str() != "choice"
        || !site.path.ends_with("/chosenType")
        || site.operand != Some(&serde_json::json!({"choice":"chosen-type"}))
        || !site.ability.pointer(&result.step).is_some_and(|producer| {
            producer["effect"] == "choose-value"
                && producer["category"] == "creature-type"
                && producer["who"] == serde_json::json!({"player":"you"})
        })
    {
        return false;
    }
    let Some((producer_sentence, index)) = result.step.rsplit_once("/steps/") else {
        return false;
    };
    let Some(index) = index.parse::<usize>().ok() else {
        return false;
    };
    if site
        .path
        .starts_with(&format!("{producer_sentence}/steps/{}/", index + 1))
    {
        return true;
    }
    let Some((effects, sentence)) = producer_sentence.rsplit_once('/') else {
        return false;
    };
    let Some(sentence) = sentence.parse::<usize>().ok() else {
        return false;
    };
    site.ability
        .pointer(&format!("{producer_sentence}/steps"))
        .and_then(Value::as_array)
        .is_some_and(|steps| index + 1 == steps.len())
        && site
            .path
            .starts_with(&format!("{effects}/{}/steps/0/", sentence + 1))
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
        Ok(Self {
            card,
            face: 0,
            spend_riders: BTreeSet::new(),
            control: BTreeMap::new(),
        })
    }

    /// CR 121.2, 608.2c/h: Rust binds actual draw participants and reduces
    /// complete instruction-wise counts. The frozen binder had no terms for
    /// those scopes. All other residuals still go through validation.
    pub(super) fn cover_control(&mut self, face: usize, raw: &str) -> Result<(), Failure> {
        let printed: Value = decode(raw, "abilities")?;
        let Some(printed) = printed.as_array() else {
            return Ok(());
        };
        let Some(blocks) = self.card.faces[face].blocks.as_ref() else {
            return Ok(());
        };
        for (index, (ability, block)) in printed.iter().zip(blocks).enumerate() {
            if block.residuals.is_empty() {
                continue;
            }
            let mut covered = ControlCoverage::default();
            let mut residuals = BTreeSet::new();
            for reference in &block.refs {
                if let RefEntry::Unresolved(entry) = reference
                    && entry.unresolved == "unsupported-scope"
                    && (draw_players_after_draw(ability, &entry.path)
                        || counted_iteration_site(
                            ability,
                            &entry.path,
                            &["/each", "/each/filter/control", "/effects/0/steps/0/who"],
                        ))
                {
                    residuals.extend([
                        "diagnostic:unsupported-scope".to_owned(),
                        "ref:unsupported-scope".to_owned(),
                    ]);
                    covered.references.insert(entry.path.clone());
                }
            }
            for step in &block.steps {
                if let StepEntry::Unaccounted(entry) = step
                    && counted_iteration_site(ability, &entry.path, &["/effects/0/steps/0"])
                {
                    residuals.insert(format!("step:{}", entry.ast_effect.as_str()));
                    covered.steps.insert(entry.path.clone());
                }
            }
            if block.residuals.iter().cloned().collect::<BTreeSet<_>>() == residuals {
                self.control.insert((face, index), covered);
            }
        }
        Ok(())
    }

    pub(super) fn face_count(&self) -> usize {
        self.card.faces.len()
    }

    pub(super) fn select_face(&mut self, index: usize) -> Result<(), Failure> {
        self.face = index;
        self.blocks().map(|_| ())
    }

    pub(super) fn block_offset(&self) -> usize {
        self.card.faces[..self.face]
            .iter()
            .filter_map(|face| face.blocks.as_ref())
            .map(Vec::len)
            .sum()
    }

    fn blocks(&self) -> Result<&[BlockResolution], Failure> {
        self.card
            .faces
            .get(self.face)
            .filter(|face| face.source_face_index as usize == self.face)
            .ok_or_else(|| invalid("resolution", "missing or unordered resolved face"))?
            .blocks
            .as_deref()
            .ok_or_else(|| unsupported("resolution", "face:unparsed"))
    }

    // R5 left the referent of these exact CR 106.6 riders unresolved.
    // The spend allocation binds it later, to the purchased spell. Require
    // the complete rider AST and both exact residual sites before discharging
    // the frozen diagnostics. Other references and steps still pass the gate.
    pub(super) fn cover_spend_riders(&mut self, face: usize, raw: &str) -> Result<(), Failure> {
        let printed: Value = decode(raw, "abilities")?;
        let Some(printed) = printed.as_array() else {
            // The ordinary composition guard owns malformed and null operands.
            return Ok(());
        };
        let Some(blocks) = self.card.faces[face].blocks.as_ref() else {
            return Ok(());
        };
        for (index, (ability, block)) in printed.iter().zip(blocks).enumerate() {
            let Some(step) = ability.pointer("/effects/0/steps/0") else {
                continue;
            };
            if ability["kind"] != "activated"
                || step["effect"] != "add-mana"
                || step.get("riders").is_none()
                || crate::rules::lower_spend_restriction(
                    step.get("restriction"),
                    step.get("riders"),
                )
                .is_err()
                || block.residuals
                    != [
                        "diagnostic:missing-antecedent",
                        "ref:missing-antecedent",
                        "step:cant",
                    ]
                || !block.refs.iter().any(|r| {
                    matches!(r, RefEntry::Unresolved(u)
                    if u.path == SPEND_RIDER_REF && u.unresolved == "missing-antecedent")
                })
                || !block.steps.iter().any(|entry| {
                    matches!(entry, StepEntry::Unaccounted(u)
                    if u.path == SPEND_RIDER_STEP && u.ast_effect.as_str() == "cant")
                })
            {
                continue;
            }
            self.spend_riders.insert((face, index));
        }
        Ok(())
    }

    /// Every reference resolved in the closed vocabulary, every step
    /// accounted for and every token attributed; otherwise the first exact
    /// residual (card-level first, then by block).
    pub(super) fn complete(&self) -> Result<(), Failure> {
        let first = self.card.residuals.first().or_else(|| {
            self.card
                .faces
                .iter()
                .enumerate()
                .flat_map(|(face, f)| {
                    f.blocks
                        .iter()
                        .flatten()
                        .enumerate()
                        .filter(move |(block, _)| {
                            !self.spend_riders.contains(&(face, *block))
                                && !self.control.contains_key(&(face, *block))
                        })
                })
                .find_map(|(_, b)| b.residuals.first())
        });
        match first {
            Some(residual) => Err(unsupported("resolution", residual)),
            None => Ok(()),
        }
    }

    /// Every resolved reference of every block, looked up in the reference
    /// table ([`reference_row`]): its role, read where it is printed, either
    /// has a lowering or is rejected under `ref:<role>:<block kind>`.
    #[cfg(test)]
    pub(super) fn references(&self, abilities: &Value) -> Result<(), Failure> {
        self.references_with_types(abilities, &[])
    }

    pub(super) fn references_with_types(
        &self,
        abilities: &Value,
        typed: &[ast::Ability],
    ) -> Result<(), Failure> {
        for (index, block) in self.blocks()?.iter().enumerate() {
            let ability = &abilities[index];
            let kind = BlockKind::of(ability);
            for entry in &block.refs {
                let (path, scope, role) = match entry {
                    RefEntry::Ref(r) => (&r.path, r.scope.as_deref(), Role::Resolved(&r.r#ref)),
                    // CR 607.2d: a player derived from another block's
                    // linked result has no per-object storage to read yet.
                    RefEntry::Derived(d)
                        if matches!(&*d.derived.of, ResolvedRef::LinkedResult(_)) =>
                    {
                        return Err(unsupported(
                            "resolution",
                            format!("ref:{}:linked-result:{}", describe(entry), kind.name()),
                        ));
                    }
                    RefEntry::Derived(d) => (&d.path, d.scope.as_deref(), Role::Unbound),
                    RefEntry::Evaluated(e) => (&e.path, e.scope.as_deref(), Role::Unbound),
                    RefEntry::Unresolved(u)
                        if u.path == SPEND_RIDER_REF
                            && u.unresolved == "missing-antecedent"
                            && self.spend_riders.contains(&(self.face, index)) =>
                    {
                        continue;
                    }
                    RefEntry::Unresolved(u)
                        if u.unresolved == "unsupported-scope"
                            && self
                                .control
                                .get(&(self.face, index))
                                .is_some_and(|covered| covered.references.contains(&u.path)) =>
                    {
                        continue;
                    }
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
                if let Role::Resolved(ResolvedRef::LinkedResult(link)) = role
                    && linked_entry_choice(abilities, &site, link)
                {
                    continue;
                }
                let evaluated = typed
                    .get(index)
                    .and_then(|block| evaluated_site(block, path));
                if matches!(role, Role::Resolved(ResolvedRef::StepResult(result))
                    if matches!(result.role, ast::StepResultRole::Selection | ast::StepResultRole::Remainder)
                        && super::compose::revealed::local_reference(ability, path))
                {
                    continue;
                }
                let admitted = match (evaluated, role) {
                    (Some(kind), Role::Resolved(reference)) => {
                        evaluated_reference(reference, kind, &site)
                    }
                    _ => reference_row(role, &site),
                };
                if !admitted
                    && !matches!(role, Role::Resolved(ResolvedRef::StepResult(r)) if local_creature_type_choice(&site, r))
                {
                    return Err(unsupported(
                        "resolution",
                        format!("ref:{}:{}", describe(entry), kind.name()),
                    ));
                }
            }
        }
        Ok(())
    }

    pub(super) fn validate_delayed_returns(&self, abilities: &Value) -> Result<(), Failure> {
        for (ability, block) in abilities
            .as_array()
            .into_iter()
            .flatten()
            .zip(self.blocks()?)
        {
            for entry in &block.steps {
                let StepEntry::Scope(scope) = entry else {
                    continue;
                };
                if scope.ast_effect.as_str() != "create-delayed-trigger" {
                    continue;
                }
                let path = format!("{}/effects/0/steps/0", scope.path);
                let Some(step) = ability.pointer(&path) else {
                    continue;
                };
                let departure = super::compose::resolution_triggers::is_departure_return(step);
                if !departure && !super::compose::resolution_triggers::is_owner_return(step) {
                    continue;
                }
                let what_path = format!("{path}/what");
                let owner_path = format!("{path}/controller");
                let from = block.refs.iter().find_map(|entry| match entry {
                    RefEntry::Ref(r) if r.path == what_path => match &*r.r#ref {
                        ResolvedRef::StepResult(result) => Some(result),
                        _ => None,
                    },
                    _ => None,
                });
                if !from.is_some_and(|from| {
                    (delayed_exile_producer(ability, &what_path, from)
                        || (departure && delayed_dies_producer(ability, &what_path, from)))
                        && block.refs.iter().any(|entry| {
                            matches!(entry, RefEntry::Derived(d)
                        if d.path == owner_path && d.derived.relation.as_str() == "owner"
                            && matches!(&*d.derived.of, ResolvedRef::StepResult(owner)
                                if owner.step == from.step && owner.role == from.role))
                        })
                }) {
                    return Err(unsupported(
                        "resolution",
                        "delayed return must bind the preceding exile successor and its owner",
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
        self.target_slots_with_reflexive(index, ability, targets, None)
    }

    pub(super) fn target_slots_with_reflexive(
        &self,
        index: usize,
        ability: &Value,
        targets: &[TargetSpec],
        reflexive: Option<&[TargetSpec]>,
    ) -> Result<(), Failure> {
        let block = self
            .blocks()?
            .get(index)
            .ok_or_else(|| invalid("resolution", "missing block"))?;
        for entry in &block.refs {
            let RefEntry::Ref(r) = entry else { continue };
            let expression = ability.pointer(&r.path);
            let ResolvedRef::Target(t) = &*r.r#ref else {
                // The typed operand check already authenticated the event subject.
                if matches!(&*r.r#ref, ResolvedRef::EventObject(_))
                    || matches!(&*r.r#ref, ResolvedRef::StepResult(result)
                        if local_movement_result(ability, &r.path, result))
                {
                    continue;
                }
                // CR 608.2b-c: other singular follow-up object effects
                // consume an announced antecedent.
                if expression.is_some_and(|e| matches!(e["ref"].as_str(), Some("it" | "that")))
                    && r.path.rsplit_once('/').is_some_and(|(step, operand)| {
                        matches!(operand, "what" | "on")
                            && ability.pointer(step).is_some_and(|s| {
                                matches!(
                                    s["effect"].as_str(),
                                    Some(
                                        "gain-abilities"
                                            | "gain-control"
                                            | "pump"
                                            | "put-counters"
                                            | "set-characteristics"
                                            | "untap"
                                    )
                                )
                            })
                    })
                {
                    return Err(unsupported(
                        "resolution",
                        format!("object follow-up {} needs an announced target", r.path),
                    ));
                }
                continue;
            };
            let targets = match (reflexive, r.scope.as_deref()) {
                (Some(nested), Some("/effects/1/steps/0"))
                    if r.path.starts_with("/effects/1/steps/0/effects/") =>
                {
                    nested
                }
                _ => targets,
            };
            let fight_antecedent = r.path.rsplit_once('/').is_some_and(|(parent, field)| {
                field == "a"
                    && ability.pointer(parent).is_some_and(|step| {
                        step["effect"] == "fight"
                            && matches!(step["a"]["ref"].as_str(), Some("it" | "that"))
                    })
            });
            let announced = match expression.and_then(|e| e["selectionId"].as_str()) {
                Some(selection) => targets.iter().find(|s| s.selection_id == selection),
                None => match targets {
                    [sole] => Some(sole),
                    // The choice-result of a declared group denotes its one
                    // printed selection (slot zero), not each synthetic member.
                    [first, rest @ ..]
                        if r.path == "/effects/2/steps/0/what"
                            && expression.is_some_and(|v| v["ref"] == "choice-result")
                            && ability
                                .pointer("/effects/0/steps/0/effect")
                                .is_some_and(|v| v == "declare-target")
                            && ability
                                .pointer("/effects/0/steps/0/what/selectionId")
                                .and_then(Value::as_str)
                                == Some(first.selection_id.as_str())
                            && rest.iter().enumerate().all(|(i, t)| {
                                t.selection_id == format!("{}/{}", first.selection_id, i + 1)
                            }) =>
                    {
                        Some(first)
                    }
                    // The fight lowerer bound the preceding creature effect's
                    // sole occurrence before it introduced the second fighter.
                    [first, _] if fight_antecedent => Some(first),
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
                if !accounted
                    && !(path == SPEND_RIDER_STEP
                        && effect == "cant"
                        && self.spend_riders.contains(&(self.face, index)))
                    && !self
                        .control
                        .get(&(self.face, index))
                        .is_some_and(|covered| covered.steps.contains(path))
                {
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

    /// An unscoped reference to the target announced by this block.
    pub(super) fn target_slot_at(&self, index: usize, path: &str) -> Result<Option<u32>, Failure> {
        let block = self
            .blocks()?
            .get(index)
            .ok_or_else(|| invalid("resolution", "missing block"))?;
        Ok(block.refs.iter().find_map(|entry| match entry {
            RefEntry::Ref(r) if r.path == path && r.scope.is_none() => match &*r.r#ref {
                ResolvedRef::Target(t) => u32::try_from(t.slot).ok(),
                _ => None,
            },
            _ => None,
        }))
    }

    pub(super) fn binds_target_player_selection(
        &self,
        index: usize,
        who: &str,
        implicit: &str,
        slot: u32,
    ) -> Result<bool, Failure> {
        let block = self
            .blocks()?
            .get(index)
            .ok_or_else(|| invalid("resolution", "missing block"))?;
        let of = format!("{who}/of");
        let selection = block.refs.iter().any(
            |entry| matches!(entry, RefEntry::Evaluated(e) if e.path == who && e.scope.is_none()),
        );
        let target = block.refs.iter().any(|entry| {
            matches!(entry, RefEntry::Ref(r) if r.path == of && r.scope.is_none()
                && matches!(&*r.r#ref, ResolvedRef::Target(t) if t.slot == i64::from(slot)))
        });
        let payer = block
            .refs
            .iter()
            .find(|entry| entry_path(entry) == implicit);
        Ok(selection
            && target
            && payer.is_some_and(|entry| {
                matches!(entry, RefEntry::Ref(r) if r.scope.is_none()
                && matches!(&*r.r#ref, ResolvedRef::StepResult(p)
                    if p.step == who && p.role == crate::ast::StepResultRole::Selection))
            }))
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
            // CR 608.2h: "its controller", the current or last-known
            // controller of one of the block's announced targets.
            RefEntry::Derived(d) if d.path == path && d.scope.is_none() => {
                match (&d.derived.relation, &*d.derived.of) {
                    (crate::ast::PlayerRelation::Controller, ResolvedRef::Target(t)) => {
                        u32::try_from(t.slot)
                            .ok()
                            .map(|slot| kir::PlayerExpr::ControllerOf {
                                object: kir::Ref::Target { slot },
                            })
                    }
                    _ => None,
                }
            }
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
        "draws" => Some((kir::EventPlayerRole::DrawingPlayer, "draws")),
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
            Some("activated" | "loyalty") => Self::Activated,
            Some("triggered" | "chapter") => Self::Triggered,
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

/// CR 607.2d: the consuming term reads the unique printed choice on this
/// face. Verify the frozen archive's producer against the actual source AST.
fn linked_entry_choice(
    abilities: &Value,
    site: &Site<'_>,
    link: &ast::ResolvedRefLinkedResult,
) -> bool {
    if site.scope.is_some() || link.role != ast::StepResultRole::Choice {
        return false;
    }
    let category = match (site.kind, site.path) {
        (
            BlockKind::Activated,
            "/effects/0/steps/0/restriction/uses/0/filter/chosenType"
            | "/effects/0/steps/0/restriction/uses/1/source/chosenType"
            | "/effects/0/steps/0/riders/0/spentOn/0/filter/chosenType",
        ) if site.ability["effects"][0]["steps"][0]["effect"] == "add-mana" => "creature-type",
        (BlockKind::Static, "/effect/what/filter/chosenType") => "creature-type",
        (BlockKind::Activated, "/effects/0/steps/0/mana/choice")
            if site.ability["effects"][0]["steps"][0]["mana"]["mana"] == "chosen-color" =>
        {
            "color"
        }
        (BlockKind::Static, "/effect/what/filter/chosenColor") => "color",
        _ => return false,
    };
    let Ok(index) = usize::try_from(link.block) else {
        return false;
    };
    let producer = &abilities[index];
    let path = "/effect/effects/0/steps/0";
    producer["kind"] == "static"
        && producer["effect"]["static"] == "as-enters"
        && link.step == path
        && producer
            .pointer(path)
            .is_some_and(|step| step["effect"] == "choose-value" && step["category"] == category)
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
    /// CR 614.12: the incoming controller binds the printed entry condition
    /// and the count its entry counters are multiplied by.
    fn controller(&self) -> Option<ControllerRole> {
        if self.kind == BlockKind::Static
            && ((self.path.starts_with("/effect/condition/")
                && self.ability["effect"]["static"] == "enters-tapped")
                || (matches!(
                    self.path,
                    "/effect/who"
                        | "/effect/costs/0/payer"
                        | "/effect/costs/0/what/filter/zone/owner"
                ) && self.ability["effect"]["static"] == "entry-choice")
                || (self.path.starts_with("/effect/multiplier/")
                    && self.ability["effect"]["static"] == "enters-with-counters")
                || self
                    .path
                    .strip_suffix("/effects/0/steps/0/who")
                    .is_some_and(|parent| {
                        self.ability
                            .pointer(parent)
                            .is_some_and(|effect| effect["static"] == "as-enters")
                    }))
        {
            Some(ControllerRole::EnteringController)
        } else if self.quoted_ability() {
            // CR 109.5, 113.10a: "you" in a granted activated ability is
            // the player who activates it, on the object that has it.
            Some(ControllerRole::AbilityController)
        } else {
            self.kind.controller()
        }
    }

    /// The reference is printed inside a static's quoted ability, and the
    /// archive scoped it to that ability.
    fn quoted_ability(&self) -> bool {
        self.kind == BlockKind::Static
            && self.scope.is_some_and(|scope| {
                self.path.starts_with(&format!("{scope}/"))
                    && scope
                        .strip_suffix("/ability")
                        .and_then(|grant| self.ability.pointer(grant))
                        .is_some_and(|grant| grant["gained"] == "quoted")
            })
    }

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

#[derive(Clone, Copy)]
enum SiteKind<'a> {
    Player(&'a ast::PlayerRef),
    Object(&'a ast::ObjectRef),
    Amount(&'a ast::Amount),
}

fn evaluated_site<'a>(block: &'a ast::Ability, path: &str) -> Option<SiteKind<'a>> {
    let effects = match block {
        ast::Ability::Activated(block) => &block.effects,
        ast::Ability::Triggered(block) => &block.effects,
        ast::Ability::Spell(block) => &block.effects,
        _ => return None,
    };
    let mut parts = path.strip_prefix("/effects/")?.split('/');
    let sentence = effects.get(parts.next()?.parse::<usize>().ok()?)?;
    match parts.next()? {
        "steps" => {}
        "bindings" => {
            let binding = sentence
                .bindings
                .as_ref()?
                .get(parts.next()?.parse::<usize>().ok()?)?;
            if parts.next()? != "value" {
                return None;
            };
            return operand_site(SiteKind::Amount(&binding.value), &mut parts);
        }
        _ => return None,
    }
    let action = sentence.steps.get(parts.next()?.parse::<usize>().ok()?)?;
    let field = parts.next()?;
    let site = match (action, field) {
        (ast::Effect::Discard(discard), "who") => SiteKind::Player(&discard.who),
        (ast::Effect::Discard(discard), "what") => match discard.what.as_ref() {
            ast::CostDiscardWhat::ObjectRef(object) => SiteKind::Object(object),
            ast::CostDiscardWhat::Hand => return None,
        },
        (ast::Effect::Mill(mill), "who") => SiteKind::Player(&mill.who),
        (ast::Effect::Mill(mill), "amount") => SiteKind::Amount(&mill.amount),
        (ast::Effect::Draw(draw), "who") => SiteKind::Player(&draw.who),
        (ast::Effect::Variant99(life), "who") => SiteKind::Player(&life.who),
        (ast::Effect::Sacrifice(sacrifice), "who" | "chosenBy") => {
            SiteKind::Player(if field == "who" {
                &sacrifice.who
            } else {
                sacrifice.chosen_by.as_deref()?
            })
        }
        (ast::Effect::Sacrifice(sacrifice), "what") => SiteKind::Object(&sacrifice.what),
        (ast::Effect::Draw(draw), "amount") => SiteKind::Amount(&draw.amount),
        (ast::Effect::Variant99(life), "amount") => SiteKind::Amount(&life.amount),
        (ast::Effect::PutCounters(counters), "on") => SiteKind::Object(&counters.on),
        (ast::Effect::PutCounters(counters), "count") => SiteKind::Amount(&counters.count),
        (ast::Effect::Damage(damage), "source") => SiteKind::Object(&damage.source),
        (ast::Effect::Damage(damage), "amount") => SiteKind::Amount(&damage.amount),
        (ast::Effect::Damage(damage), "to") => match (&*damage.to, parts.next()) {
            (ast::DamageTarget::Player(to), Some("ref")) => SiteKind::Player(&to.r#ref),
            (ast::DamageTarget::Object(to), Some("ref")) => SiteKind::Object(&to.r#ref),
            (ast::DamageTarget::Attacked(to), Some("by")) => SiteKind::Object(&to.by),
            (ast::DamageTarget::Each(to), Some("refs")) => {
                match to.refs.get(parts.next()?.parse::<usize>().ok()?)? {
                    ast::AmountCounterCountOn::PlayerRef(player) => SiteKind::Player(player),
                    ast::AmountCounterCountOn::ObjectRef(object) => SiteKind::Object(object),
                }
            }
            _ => return None,
        },
        _ => return None,
    };
    operand_site(site, &mut parts)
}

fn operand_site<'a>(
    site: SiteKind<'a>,
    parts: &mut std::str::Split<'_, char>,
) -> Option<SiteKind<'a>> {
    let Some(field) = parts.next() else {
        return Some(site);
    };
    let child = match (site, field) {
        (SiteKind::Player(ast::PlayerRef::Owner(who)), "of") => SiteKind::Object(&who.of),
        (SiteKind::Player(ast::PlayerRef::Controller(who)), "of") => SiteKind::Object(&who.of),
        (SiteKind::Object(ast::ObjectRef::Filter(selection)), "count") => {
            SiteKind::Amount(&selection.count)
        }
        (SiteKind::Amount(ast::Amount::Attribute(amount)), "of") => SiteKind::Object(&amount.of),
        (SiteKind::Amount(ast::Amount::LifeTotal(amount)), "of") => SiteKind::Player(&amount.of),
        (SiteKind::Amount(ast::Amount::Plus(amount)), "a") => SiteKind::Amount(&amount.a),
        (SiteKind::Amount(ast::Amount::Plus(amount)), "b") => SiteKind::Amount(&amount.b),
        (SiteKind::Amount(ast::Amount::Minus(amount)), "a") => SiteKind::Amount(&amount.a),
        (SiteKind::Amount(ast::Amount::Minus(amount)), "b") => SiteKind::Amount(&amount.b),
        (SiteKind::Amount(ast::Amount::Twice(amount)), "of") => SiteKind::Amount(&amount.of),
        (SiteKind::Amount(ast::Amount::Product(amount)), "factors") => {
            SiteKind::Amount(amount.factors.get(parts.next()?.parse::<usize>().ok()?)?)
        }
        _ => return None,
    };
    operand_site(child, parts)
}

pub(crate) fn sacrifice_cost_object(related: &ast::ObjectRefRelated) -> bool {
    related.number == ast::ObjectRefCopyResultNumber::Singular
        && matches!(related.relation.as_ref(), ast::ObjectRelation::EventResult(event)
            if event.event == ast::ObjectRelationEventResultEvent::Sacrifice
                && event.scope == ast::ObjectRelationEventResultScope::Contextual
                && event.period.is_none() && event.who.is_none())
        && present_fields(&related.filter) == ["types"]
        && related
            .filter
            .types
            .as_deref()
            .is_some_and(|types| types == ["creature"])
}

fn class_of(reference: &ResolvedRef, site: SiteKind<'_>) -> Option<kir::OperandClass> {
    use kir::OperandClass as Class;
    match (reference, site) {
        (ResolvedRef::Controller(_), SiteKind::Player(ast::PlayerRef::You(_))) => {
            Some(Class::Controller)
        }
        (
            ResolvedRef::EventPlayer(_),
            SiteKind::Player(
                ast::PlayerRef::ThatPlayer(_)
                | ast::PlayerRef::They(_)
                | ast::PlayerRef::DefendingPlayer(_)
                | ast::PlayerRef::ActivePlayer(_),
            ),
        ) => Some(Class::EventPlayer),
        (
            ResolvedRef::Target(_),
            SiteKind::Player(ast::PlayerRef::TargetPlayer(_) | ast::PlayerRef::TargetOpponent(_)),
        ) => Some(Class::Target),
        (
            ResolvedRef::Source(_),
            SiteKind::Object(
                ast::ObjectRef::This(_) | ast::ObjectRef::SelfObject(_) | ast::ObjectRef::It(_),
            ),
        ) => Some(Class::Source),
        (
            ResolvedRef::Target(_),
            SiteKind::Object(
                ast::ObjectRef::Target(_) | ast::ObjectRef::It(_) | ast::ObjectRef::That(_),
            ),
        ) => Some(Class::Target),
        (
            ResolvedRef::EventObject(_),
            SiteKind::Object(ast::ObjectRef::It(_) | ast::ObjectRef::That(_)),
        ) => Some(Class::EventObject),
        (ResolvedRef::AnnouncedX(_), SiteKind::Amount(ast::Amount::X(_))) => {
            Some(Class::AnnouncedX)
        }
        (ResolvedRef::BoundVar(_), SiteKind::Amount(ast::Amount::X(_))) => Some(Class::Bound),
        (ResolvedRef::StepResult(step), SiteKind::Amount(ast::Amount::ThatMuch(_)))
            if step.step == "/trigger" && step.role.as_str() == "quantity" =>
        {
            Some(Class::EventAmount)
        }
        (ResolvedRef::StepResult(step), SiteKind::Object(ast::ObjectRef::Related(related)))
            if step.role.as_str() == "selection" && sacrifice_cost_object(related) =>
        {
            Some(Class::CostResult)
        }
        _ => None,
    }
}

fn evaluated_reference(reference: &ResolvedRef, kind: SiteKind<'_>, at: &Site<'_>) -> bool {
    use kir::OperandClass as Class;
    if at.scope.is_some() {
        return false;
    }
    let trigger = at.ability["trigger"]["trigger"].as_str();
    match (class_of(reference, kind), reference) {
        (Some(Class::Controller), _) => controller_row(Some(reference), at),
        (Some(Class::EventPlayer), ResolvedRef::EventPlayer(event)) => {
            at.kind == BlockKind::Triggered
                && (kernel_event_player(&event.role)
                    .is_some_and(|(_, event)| trigger == Some(event))
                    || trigger == Some(event.role.as_str())
                    || (event.role == "defending-player"
                        && matches!(
                            trigger,
                            Some("attacks" | "attacks-unblocked" | "becomes-blocked")
                        )))
        }
        (Some(Class::EventObject), ResolvedRef::EventObject(event)) => {
            at.kind == BlockKind::Triggered && trigger == Some(event.role.as_str())
        }
        (Some(Class::CostResult), ResolvedRef::StepResult(step)) => {
            at.kind == BlockKind::Activated
                && step
                    .step
                    .strip_prefix("/costs/")
                    .and_then(|cost| cost.strip_suffix("/what"))
                    .and_then(|index| index.parse::<usize>().ok())
                    .is_some_and(|index| {
                        let costs = at.ability["costs"].as_array();
                        costs.is_some_and(|costs| {
                            costs
                                .get(index)
                                .is_some_and(|cost| cost["cost"] == "sacrifice")
                                && costs
                                    .iter()
                                    .filter(|cost| cost["cost"] == "sacrifice")
                                    .count()
                                    == 1
                        })
                    })
        }
        (Some(Class::EventAmount), _) => {
            at.kind == BlockKind::Triggered
                && matches!(
                    trigger,
                    Some("deals-damage" | "is-dealt-damage" | "gains-life")
                )
        }
        (Some(Class::Source | Class::Target | Class::AnnouncedX), _) => true,
        (Some(Class::Bound), ResolvedRef::BoundVar(var)) => var.id.starts_with("/effects/"),
        _ if matches!(kind, SiteKind::Amount(ast::Amount::ThatMuch(_))) => false,
        _ => reference_row(Role::Resolved(reference), at),
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
    // CR 608.2c, 700.5: both operands read the same preceding color choice.
    if at.kind == BlockKind::Activated
        && at.scope.is_none()
        && matches!(
            at.path,
            "/effects/1/steps/0/mana/choice" | "/effects/1/steps/0/mana/amount/colors"
        )
        && at.ability.pointer("/effects/0/steps/0/category") == Some(&Value::from("color"))
        && at.ability.pointer("/effects/1/steps/0/mana/mana") == Some(&Value::from("chosen-color"))
    {
        return matches!(resolved, Some(ResolvedRef::StepResult(result))
            if result.step == "/effects/0/steps/0" && result.role == crate::ast::StepResultRole::Choice
                && at.ability.pointer("/effects/0/steps/0/effect") == Some(&Value::from("choose-value")));
    }
    // Rows every operand word shares: a printed selection is an announced
    // target, a bare "you" the block's controller, and a pronoun an
    // announced target or an earlier step's result (CR 601.2c, 608.2c).
    if matches!(at.word(), Some("equipped" | "enchanted"))
        && !matches!(resolved, Some(ResolvedRef::AttachedTo(_)))
    {
        return false;
    }
    if at.selection() && !matches!(resolved, Some(ResolvedRef::Target(_))) {
        return false;
    }
    if at.you() {
        return matches!(resolved, Some(ResolvedRef::Controller(c)) if Some(c.role) == at.controller())
            && controller_row(resolved, at);
    }
    let word = at.word();
    if at.kind == BlockKind::Triggered
        && at.path == "/condition/what"
        && at.scope.is_none()
        && word == Some("commander")
        && at.ability["condition"]["condition"] == "control"
    {
        return resolved.is_none();
    }
    let reflexive = at.ability.pointer("/effects/1/steps/0/effect")
        == Some(&Value::from("create-reflexive-trigger"));
    if let Some(ResolvedRef::EventObject(event)) = resolved {
        // The unmigrated pump reads the exact invocation source.
        if at.kind == BlockKind::Triggered
            && at.scope.is_none()
            && word == Some("it")
            && matches!(event.role.as_str(), "enters" | "attacks")
            && at.ability["trigger"]["trigger"] == event.role
            && at.path == "/effects/0/steps/0/what"
            && at.ability["effects"][0]["steps"][0]["effect"] == "pump"
        {
            return true;
        }
        let self_event = at
            .ability
            .pointer("/trigger/what/ref")
            .and_then(Value::as_str)
            .is_some_and(|kind| matches!(kind, "this" | "self"));
        let reflexive_source = reflexive
            && at.path == "/effects/1/steps/0/effects/0/steps/0/source"
            && at.ability.pointer("/trigger/trigger") == Some(&Value::from(event.role.as_str()))
            && matches!(event.role.as_str(), "enters" | "dies" | "attacks")
            && self_event;
        let death_source = event.role.as_str() == "dies"
            && at.path == "/effects/0/steps/0/what"
            && delayed_dies_exile(at.ability);
        if word == Some("it") && (reflexive_source || death_source) {
            return true;
        }
    }
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
            Some("target") => {
                at.scope.is_none()
                    || (reflexive
                        && at.scope == Some("/effects/1/steps/0")
                        && at.path.starts_with("/effects/1/steps/0/effects/"))
            }
            _ if shared_word(word, at) => true,
            _ => at.scope.is_none_or(|s| s.starts_with("/effects/")),
        },
        // CR 608.2c: "the rest" names exactly a remainder.
        Some(ResolvedRef::StepResult(s)) if delayed_dies_producer(at.ability, at.path, s) => true,
        Some(ResolvedRef::StepResult(s)) => {
            word.is_none_or(|w| (w == "rest") == (s.role.as_str() == "remainder"))
                && word_row(word, at, |at| {
                    at.effects_operand()
                        && (local_movement_result(at.ability, at.path, s)
                            || step_producer(at.block, at.ability, at.path, s)
                            || delayed_exile_producer(at.ability, at.path, s))
                })
        }
        Some(ResolvedRef::BoundVar(v)) => word_row(word, at, |at| {
            at.effects_operand() && v.id.starts_with("/effects/")
        }),
        Some(ResolvedRef::LinkedPool(p)) => word_row(word, at, |at| {
            at.effects_operand() && p.id.starts_with("/effects/")
        }),
        // Only the typed entry-choice producers above bind across blocks.
        Some(ResolvedRef::LinkedResult(_)) => false,
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
        // CR 301.5 and 303.4: only host scopes and host event subjects bind here.
        Some(ResolvedRef::AttachedTo(_)) => {
            (at.kind == BlockKind::Static
                || (at.kind == BlockKind::Triggered && at.path == "/trigger/what"))
                && at.scope.is_none()
                && matches!(word, Some("equipped" | "enchanted"))
        }
        Some(
            ResolvedRef::EventObject(_)
            | ResolvedRef::EnteringObject(_)
            | ResolvedRef::AnnouncedX(_),
        ) => word_row(word, at, |_| false),
    }
}

/// R5's step locator is checked against the printed scope. The result
/// consumer resolves the noun, cardinality and public incarnation in KIR.
fn local_movement_result(ability: &Value, path: &str, result: &ast::ResolvedRefStepResult) -> bool {
    let Some((consumer, field)) = path.rsplit_once('/') else {
        return false;
    };
    if result.role != ast::StepResultRole::Objects
        || ability.pointer(consumer).is_none_or(|step| {
            !matches!(
                (step["effect"].as_str(), field),
                (Some("move-zone"), "what") | (Some("put-counters"), "on")
            )
        })
    {
        return false;
    }
    let Some(producer) = ability.pointer(&result.step) else {
        return false;
    };
    let zone = match producer["effect"].as_str() {
        Some("exile") => kir::ResultZone::Exile,
        Some("move-zone") if producer["to"]["zone"] == "battlefield" => {
            kir::ResultZone::Battlefield
        }
        _ => return false,
    };
    // The two steps share the same direct sequence, including a sequence
    // within a printed optional group or chosen mode.
    fn position(path: &str) -> Option<(&str, usize, usize)> {
        let (sentence_path, step) = path.rsplit_once("/steps/")?;
        let (effects_path, sentence) = sentence_path.rsplit_once('/')?;
        if let Ok(sentence) = sentence.parse() {
            Some((effects_path, sentence, step.parse().ok()?))
        } else {
            None
        }
    }
    let Some((producer_scope, sentence, step)) = position(&result.step) else {
        return false;
    };
    let Some((consumer_scope, next_sentence, next_step)) = position(consumer) else {
        return false;
    };
    producer_scope == consumer_scope
        && (sentence, step) < (next_sentence, next_step)
        && ability
            .pointer(path)
            .cloned()
            .and_then(|raw| serde_json::from_value(raw).ok())
            .is_some_and(|object| {
                crate::rules::card_result::movement_mention(&object, zone).is_some()
            })
}

/// CR 109.5: a controller resolved with no scope in a block with a
/// controller is that block's controller.
fn controller_row(resolved: Option<&ResolvedRef>, at: &Site<'_>) -> bool {
    match resolved {
        Some(ResolvedRef::Controller(c)) if at.scope.is_none() => {
            at.controller().is_none_or(|own| c.role == own)
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
            ResolvedRef::LinkedResult(_) => "linked-result".into(),
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
                || (object.get("choice").and_then(Value::as_str) == Some("chosen-value")
                    && (path.ends_with("/mana/choice") || path.ends_with("/mana/amount/colors")))
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
fn delayed_dies_exile(ability: &Value) -> bool {
    ability.pointer("/trigger/trigger") == Some(&Value::from("dies"))
        && ability
            .pointer("/trigger/what/ref")
            .and_then(Value::as_str)
            .is_some_and(|kind| matches!(kind, "this" | "self"))
        && ability
            .pointer("/effects/0/steps/0")
            .is_some_and(|step| step["effect"] == "exile" && step["what"]["ref"] == "it")
        && ability.pointer("/effects/1/steps/0").is_some_and(|step| {
            step["effect"] == "create-delayed-trigger" && step["trigger"]["trigger"] == "leaves"
        })
}

fn delayed_dies_producer(
    ability: &Value,
    consumer: &str,
    result: &crate::ast::ResolvedRefStepResult,
) -> bool {
    consumer == "/effects/1/steps/0/effects/0/steps/0/what"
        && result.role.as_str() == "objects"
        && result.step == "/effects/0/steps/0"
        && delayed_dies_exile(ability)
        && ability
            .pointer("/effects/1/steps/0/effects/0/steps/0")
            .is_some_and(super::compose::resolution_triggers::is_departure_return)
}

fn delayed_exile_producer(
    ability: &Value,
    consumer: &str,
    result: &crate::ast::ResolvedRefStepResult,
) -> bool {
    let Some((sequence, index)) = consumer
        .strip_prefix("/effects/")
        .and_then(|p| p.split_once('/'))
    else {
        return false;
    };
    let Some(previous) = sequence
        .parse::<usize>()
        .ok()
        .and_then(|index| index.checked_sub(1))
    else {
        return false;
    };
    index == "steps/0/effects/0/steps/0/what"
        && result.role.as_str() == "objects"
        && result.step == format!("/effects/{previous}/steps/0")
        && ability.pointer(&format!("/effects/{sequence}/steps/0/effect"))
            == Some(&Value::from("create-delayed-trigger"))
        && ability
            .pointer(&result.step)
            .is_some_and(|step| step["effect"] == "exile" && step["what"]["ref"] == "target")
}

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
        // The cards a search found are its objects too (CR 701.23a).
        "objects"
            if ability
                .pointer(&result.step)
                .is_some_and(|s| s["effect"] == "search") =>
        {
            (result.step.as_str(), "search")
        }
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

#[cfg(test)]
mod linked_result_tests {
    use super::*;
    use serde_json::json;

    /// One block whose single reference resolves to `entry`.
    fn block(entry: Value) -> Resolution {
        let card = json!({"artifactKind":"oracle-resolution","schemaVersion":1,"faces":[{"sourceFaceIndex":0,
            "blocks":[{"refs":[entry],"steps":[],"spans":{"unsupported":"synthetic"},"engineFeatures":[],
            "residuals":[]}]}],"residuals":[],"complete":true})
        .to_string();
        let raw = RawValue::from_string(card.clone()).unwrap();
        Resolution::read(Some(raw.as_ref()), Some(&sha256(card.as_bytes()))).unwrap()
    }

    /// CR 607.2d, 601.2h: another block's linked result decodes and is
    /// rejected by name, directly and as the operand of a derived player.
    #[test]
    fn linked_results_are_rejected_by_name() {
        let static_block = json!([{"kind":"static","effect":{"static":"modify","what":{"ref":"each",
            "filter":{"types":["creature"],"chosenColor":{"choice":"chosen-value","category":"color","which":"chosen"}}},
            "modifiers":[]}}]);
        let linked = json!({"ref":"linked-result","block":0,"step":"/effect/effects/0/steps/0","role":"choice"});
        let reason = |r: Result<(), Failure>| r.err().map(|f| f.reason);
        assert_eq!(
            reason(
                block(json!({"path":"/effect/what/filter/chosenColor","ref":linked.clone()}))
                    .references(&static_block)
            ),
            Some("ref:linked-result:static".into())
        );
        let derived = json!({"path":"/effect/what/filter/control","derived":{"relation":"controller","of":linked}});
        assert_eq!(
            reason(block(derived).references(&static_block)),
            Some("ref:derived:controller:linked-result:static".into())
        );
    }
}
