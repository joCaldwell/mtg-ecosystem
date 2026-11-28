//! Evidence for opt-in Rust families. Only R6 intake constructs an R6 view.
//! The resolver declares its obligations independently of the citations it finds.
use super::{
    VerifiedFace, facts, nodes,
    quotes::{BlockRegions, Quotes},
    text,
};
use crate::{Failure, unsupported};
use mtg_oracle_ast::printed::*;
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};

/// A view of a validated card. Excluded records have no view.
pub(crate) struct CardEvidence(Option<VerifiedCard>);

struct VerifiedCard {
    printed: PrintedCard,
    faces: Vec<Option<VerifiedFace>>,
    sha256: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Check {
    RequiresR6,
    P11,
    P12,
}

/// These boundaries come from the R6 punctuation facts
/// (`oracle-parser/src/printed/facts.ts`) and the productions that consume them.
/// See `docs/game-engine-catalogs.md#rust-printed-evidence`.
#[derive(Clone, Copy)]
pub(crate) enum StructuralPunctuation {
    SentenceTerminator,
    ClauseComma,
    CostListComma,
    CostEffectColon,
}

#[derive(Debug)]
pub(crate) struct EvidenceError {
    pub check: Check,
    pub face: usize,
    pub block: usize,
    pub obligation: String,
    pub range: Option<[u32; 2]>,
}

impl From<EvidenceError> for Failure {
    fn from(error: EvidenceError) -> Self {
        let code = match error.check {
            Check::RequiresR6 => "evidence:requires-r6",
            Check::P11 => "evidence:missing:P11",
            Check::P12 => "evidence:unconsumed:P12",
        };
        let path = format!("printed/faces/{}/blocks/{}", error.face, error.block);
        let reason = match error.range {
            Some([start, end]) => format!("{code}:{}@O[{start},{end})", error.obligation),
            None => format!("{code}:{}", error.obligation),
        };
        unsupported(&path, reason)
    }
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Basis<'a> {
    Recorded,
    Alias {
        target: &'a str,
        default: Option<&'a ImplicitReason>,
    },
    Implicit(&'a ImplicitReason),
}

/// For defaults, the range anchors the diagnostic at the governing
/// production. The default itself claims no tokens.
#[derive(Debug)]
pub(crate) struct Located<'a, T> {
    pub range: [u32; 2],
    pub basis: Basis<'a>,
    pub owner: &'a str,
    pub value: T,
}

#[derive(Debug)]
pub(crate) struct BlockEvidence<'a> {
    face_index: usize,
    block_index: usize,
    face: &'a PrintedFaceRead,
    normalized: &'a str,
    ast: &'a Value,
    quotes: &'a Quotes,
    block_regions: &'a BlockRegions,
    block: &'a PrintedBlockRead,
    nodes: BTreeMap<&'a str, &'a PrintedNode>,
}

impl CardEvidence {
    pub(in crate::ingest) fn verified(
        printed: PrintedCard,
        faces: Vec<Option<VerifiedFace>>,
        sha256: String,
    ) -> Self {
        Self(Some(VerifiedCard {
            printed,
            faces,
            sha256,
        }))
    }

    pub(in crate::ingest) fn legacy() -> Self {
        Self(None)
    }

    pub(crate) fn sha256(&self) -> Option<&str> {
        self.0.as_ref().map(|card| card.sha256.as_str())
    }

    /// Locates an obligation even when the legacy cut has no byte evidence.
    pub(crate) fn block(
        &self,
        face: usize,
        block: usize,
        obligation: &str,
    ) -> Result<BlockEvidence<'_>, EvidenceError> {
        let missing = |check, range| EvidenceError {
            check,
            face,
            block,
            obligation: obligation.into(),
            range,
        };
        let VerifiedCard { printed, faces, .. } = self
            .0
            .as_ref()
            .ok_or_else(|| missing(Check::RequiresR6, None))?;
        let Some(PrintedFace::Read(read)) = printed.faces.get(face) else {
            return Err(missing(Check::P11, None));
        };
        let block_data = match read.blocks.get(block) {
            Some(PrintedBlock::Read(block)) => block,
            Some(PrintedBlock::Failed(block)) => {
                return Err(missing(
                    Check::P11,
                    Some(text::hull(block.n.0, &read.edits)),
                ));
            }
            None => return Err(missing(Check::P11, Some([0, read.original.bytes]))),
        };
        let verified = faces[face].as_ref().unwrap();
        let block_verified = verified.blocks[block].as_ref().unwrap();
        Ok(BlockEvidence {
            face_index: face,
            block_index: block,
            face: read,
            normalized: &verified.normalized,
            ast: &block_verified.ast,
            quotes: &verified.quotes,
            block_regions: &block_verified.quotes,
            block: block_data,
            nodes: block_data
                .nodes
                .iter()
                .map(|node| (nodes::path(node), node))
                .collect(),
        })
    }
}

fn runs(node: &PrintedNode) -> impl Iterator<Item = &TokenRun> {
    let (span, folds) = match node {
        PrintedNode::Recorded(node) => (
            node.span.as_slice(),
            node.folds.as_deref().unwrap_or_default(),
        ),
        PrintedNode::Hull(node) => (node.span.as_slice(), &[][..]),
        _ => (&[][..], &[][..]),
    };
    span.iter().chain(folds)
}

fn contains(node: &PrintedNode, [line, start, end]: [u32; 3]) -> bool {
    // Runs are canonical within each list, but a fact can straddle a recorded
    // run and an adjacent fold of the same node.
    (start..end).all(|index| {
        runs(node).any(|run| {
            let [l, s, e] = run.0;
            l == line && (s..e).contains(&index)
        })
    })
}

fn token_range(token: &PrintedToken) -> [u32; 2] {
    match token {
        PrintedToken::CardName(v) => v.o.0,
        PrintedToken::Number(v) => v.o.0,
        PrintedToken::Punct(v) => v.o.0,
        PrintedToken::Selfref(v) => v.o.0,
        PrintedToken::Symbol(v) => v.o.0,
        PrintedToken::Word(v) => v.o.0,
    }
}

fn normalized_range(token: &PrintedToken) -> [u32; 2] {
    match token {
        PrintedToken::CardName(v) => v.n.0,
        PrintedToken::Number(v) => v.n.0,
        PrintedToken::Punct(v) => v.n.0,
        PrintedToken::Selfref(v) => v.n.0,
        PrintedToken::Symbol(v) => v.n.0,
        PrintedToken::Word(v) => v.n.0,
    }
}

/// Each scalar, scalar list and empty container is an obligation. For
/// composite values, the descendants carry the obligations independently.
fn fields(value: &Value, path: &str, owner: &str, out: &mut BTreeMap<String, String>) {
    match value {
        Value::Object(object) if !object.is_empty() => {
            for (key, value) in object {
                fields(value, &nodes::pointer(path, key), path, out);
            }
        }
        Value::Array(items) if items.iter().any(|item| item.is_object() || item.is_array()) => {
            for (index, value) in items.iter().enumerate() {
                fields(value, &nodes::pointer(path, &index.to_string()), owner, out);
            }
        }
        _ => {
            out.insert(path.to_owned(), owner.to_owned());
        }
    }
}

fn linked_field(fact: &PrintedFact) -> Option<&str> {
    match fact {
        PrintedFact::ReminderCitation(cite) => Some(&cite.field),
        _ => match facts::location(fact).unwrap().2 {
            Nullable::Value(path) => Some(path),
            Nullable::Null => None,
        },
    }
}

impl<'a> BlockEvidence<'a> {
    pub(crate) fn single_line(&self) -> Option<(u32, &'a str)> {
        let [first, end] = self.block.lines.0;
        let PrintedLine::Lexed(line) = self.face.lines.get(first as usize)? else {
            return None;
        };
        let [start, stop] = line.n.0;
        (end == first + 1).then(|| (first, &self.normalized[start as usize..stop as usize]))
    }

    pub(crate) fn missing(&self, path: &str) -> EvidenceError {
        self.error(Check::P11, path, self.container(path))
    }

    fn error(&self, check: Check, obligation: impl Into<String>, range: [u32; 2]) -> EvidenceError {
        EvidenceError {
            check,
            face: self.face_index,
            block: self.block_index,
            obligation: obligation.into(),
            range: Some(range),
        }
    }

    fn tokens(&self, line: u32) -> &'a [PrintedToken] {
        match &self.face.lines[line as usize] {
            PrintedLine::Lexed(line) => &line.tokens,
            PrintedLine::Failed(_) => &[],
        }
    }

    /// The normalized text of a nonempty run of one line's tokens.
    fn text(&self, [line, start, end]: [u32; 3]) -> &'a str {
        let tokens = self.tokens(line);
        let start = normalized_range(&tokens[start as usize])[0] as usize;
        let end = normalized_range(&tokens[end as usize - 1])[1] as usize;
        &self.normalized[start..end]
    }

    fn range(&self, runs: impl Iterator<Item = [u32; 3]>) -> Option<[u32; 2]> {
        runs.flat_map(|[line, start, end]| &self.tokens(line)[start as usize..end as usize])
            .map(token_range)
            .reduce(|[a, b], [c, d]| [a.min(c), b.max(d)])
    }

    /// A container span may include gaps. Use it for scoping and diagnostics.
    pub(crate) fn container(&self, path: &str) -> [u32; 2] {
        let mut path = path;
        loop {
            if let Some(node) = self.nodes.get(path)
                && let Some(range) = self.range(runs(node).map(|run| run.0))
            {
                return range;
            }
            match path.rsplit_once('/') {
                Some((parent, _)) => path = parent,
                None => return text::hull(self.block.n.0, &self.face.edits),
            }
        }
    }

    pub(crate) fn node(&self, path: &str) -> Result<Located<'a, &'a PrintedNode>, EvidenceError> {
        let Some((&owner, &node)) = self.nodes.get_key_value(path) else {
            return Err(self.error(Check::P11, path, self.container(path)));
        };
        let (owner, node, basis) = match node {
            PrintedNode::Recorded(_) => (owner, node, Basis::Recorded),
            PrintedNode::Implicit(value) => (owner, node, Basis::Implicit(&value.reason)),
            PrintedNode::Alias(alias) => {
                let target = self.nodes[alias.of.as_str()];
                let default = match target {
                    PrintedNode::Recorded(_) => None,
                    PrintedNode::Implicit(value) => Some(&value.reason),
                    _ => return Err(self.error(Check::P11, path, self.container(&alias.of))),
                };
                (
                    alias.of.as_str(),
                    target,
                    Basis::Alias {
                        target: &alias.of,
                        default,
                    },
                )
            }
            _ => return Err(self.error(Check::P11, path, self.container(path))),
        };
        Ok(Located {
            range: self.container(owner),
            basis,
            owner,
            value: node,
        })
    }

    pub(crate) fn facts(&self) -> impl Iterator<Item = (usize, &'a PrintedFact)> {
        self.block.facts.iter().enumerate()
    }

    pub(crate) fn fact(&self, index: usize) -> Result<Located<'a, &'a PrintedFact>, EvidenceError> {
        let Some(fact) = self.block.facts.get(index) else {
            return Err(self.error(Check::P11, format!("fact/{index}"), self.container("")));
        };
        let (owner, range) = match fact {
            PrintedFact::ReminderCitation(cite) => {
                // P8 and P10 verify this original-only citation against its exact field.
                (cite.field.rsplit_once('/').unwrap().0, cite.o.0)
            }
            _ => {
                let (owner, run, _) = facts::location(fact).unwrap();
                let range = self.range(std::iter::once(run)).unwrap();
                if !self
                    .nodes
                    .get(owner)
                    .is_some_and(|node| contains(node, run))
                {
                    return Err(self.error(Check::P11, format!("fact/{index}"), range));
                }
                (owner, range)
            }
        };
        Ok(Located {
            range,
            basis: Basis::Recorded,
            owner,
            value: fact,
        })
    }

    pub(crate) fn token(
        &self,
        line: u32,
        index: u32,
    ) -> Result<Located<'a, &'a PrintedToken>, EvidenceError> {
        let ref_name = || format!("lines/{line}/tokens/{index}");
        let [first, end] = self.block.lines.0;
        let token = (first..end)
            .contains(&line)
            .then(|| self.tokens(line))
            .and_then(|tokens| tokens.get(index as usize))
            .ok_or_else(|| self.error(Check::P11, ref_name(), self.container("")))?;
        let range = token_range(token);
        let (&owner, _) = self
            .nodes
            .iter()
            .filter(|(_, node)| contains(node, [line, index, index + 1]))
            .max_by_key(|(path, _)| path.len())
            .ok_or_else(|| self.error(Check::P11, ref_name(), range))?;
        Ok(Located {
            range,
            basis: Basis::Recorded,
            owner,
            value: token,
        })
    }

    fn primary_span(&self, path: &str) -> &[TokenRun] {
        match self.nodes.get(path) {
            Some(PrintedNode::Recorded(node)) => &node.span,
            _ => &[],
        }
    }

    fn punctuation_boundary(&self, role: StructuralPunctuation, fact: &PrintedFactPunct) -> bool {
        let [line, index, after] = fact.tok.0;
        let owner = fact.at.as_str();
        let region = self.quotes.region((line as usize, index as usize));
        if self.block_regions.get(owner) != Some(region) {
            return false;
        }
        let Some(production) = self.ast.pointer(owner) else {
            return false;
        };
        let ends_at = |path: &str, end| {
            self.primary_span(path)
                .last()
                .is_some_and(|run| run.0[0] == line && run.0[2] == end)
        };
        let between = |left: &str, right: &str| {
            ends_at(left, index)
                && self
                    .primary_span(right)
                    .first()
                    .is_some_and(|run| run.0[..2] == [line, after])
        };
        let cost_count = || {
            production["costs"]
                .as_array()
                .filter(|_| production["kind"] == "activated")
                .map_or(0, Vec::len)
        };
        let between_children = |left: &str, right: &str| {
            between(&format!("{owner}{left}"), &format!("{owner}{right}"))
        };
        match role {
            // parseSentences and SentenceRecorder delimit the primary sentence;
            // a later folded continuation may have its own sentence boundary.
            StructuralPunctuation::SentenceTerminator => {
                fact.role == PrintedFactPunctRole::SentenceEnd
                    && production["steps"].is_array()
                    && ends_at(owner, after)
            }
            // parser/index.ts and parser/for-each.ts consume the comma between
            // these heads and their effects.
            StructuralPunctuation::ClauseComma => {
                fact.role == PrintedFactPunctRole::Comma
                    && (production["kind"] == "triggered"
                        && between_children("/trigger", "/effects/0")
                        || production["effect"] == "for-each"
                            && between_children("/each", "/effects/0"))
            }
            // parseActivationCostList and the activated production in
            // parser/index.ts separate adjacent costs and the cost from its effect.
            StructuralPunctuation::CostListComma => {
                fact.role == PrintedFactPunctRole::Comma
                    && (1..cost_count()).any(|next| {
                        between_children(&format!("/costs/{}", next - 1), &format!("/costs/{next}"))
                    })
            }
            StructuralPunctuation::CostEffectColon => {
                fact.role == PrintedFactPunctRole::Colon
                    && cost_count().checked_sub(1).is_some_and(|last| {
                        between_children(&format!("/costs/{last}"), "/effects/0")
                    })
            }
        }
    }

    /// A family covers one printed block at a time. Multiple checked meanings
    /// may share a citation. Querying a node does not discharge its contents.
    pub(crate) fn check_family<T>(
        &self,
        resolve: impl FnOnce(&mut Coverage<'_, 'a>) -> Result<T, EvidenceError>,
    ) -> Result<T, EvidenceError> {
        let mut obligations = BTreeMap::new();
        fields(self.ast, "", "", &mut obligations);
        let mut coverage = Coverage {
            obligations,
            meanings: BTreeMap::new(),
            block: self,
            facts: BTreeSet::new(),
            tokens: BTreeSet::new(),
        };
        let checked = resolve(&mut coverage)?;
        coverage.finish()?;
        Ok(checked)
    }

    /// Located facts directly owned by this node, or by its resolved alias target.
    pub(crate) fn node_facts(
        &self,
        path: &str,
    ) -> Result<
        impl Iterator<Item = Result<(usize, Located<'a, &'a PrintedFact>), EvidenceError>>,
        EvidenceError,
    > {
        let node = self.node(path)?;
        Ok(self.facts().filter_map(move |(index, fact)| {
            let owner = match fact {
                PrintedFact::ReminderCitation(cite) => cite.field.rsplit_once('/').unwrap().0,
                _ => facts::location(fact).unwrap().0,
            };
            (owner == node.owner).then(|| self.fact(index).map(|fact| (index, fact)))
        }))
    }
}

#[must_use = "finish coverage before admitting a migrated block"]
pub(crate) struct Coverage<'block, 'a> {
    block: &'block BlockEvidence<'a>,
    obligations: BTreeMap<String, String>,
    meanings: BTreeMap<String, BTreeSet<(u32, u32)>>,
    facts: BTreeSet<usize>,
    tokens: BTreeSet<(u32, u32)>,
}

impl<'a> Coverage<'_, 'a> {
    fn missing(&self, field: &str) -> EvidenceError {
        self.block
            .error(Check::P11, field, self.block.container(field))
    }

    fn record(&mut self, field: &str, [line, start, end]: [u32; 3]) {
        let occurrences = (start..end).map(|index| (line, index));
        self.meanings
            .entry(field.to_owned())
            .or_default()
            .extend(occurrences.clone());
        self.tokens.extend(occurrences);
    }

    /// Discharge a field using the exact linkage checked by P9 or P10.
    pub(crate) fn field(
        &mut self,
        field: &str,
    ) -> Result<Located<'a, &'a PrintedFact>, EvidenceError> {
        let owner = self
            .obligations
            .get(field)
            .ok_or_else(|| self.missing(field))?;
        if self
            .block
            .ast
            .pointer(field)
            .and_then(Value::as_array)
            .is_some_and(|items| items.len() != 1)
        {
            return Err(self.missing(field));
        }
        let mut linked = self
            .block
            .facts()
            .filter(|(_, fact)| linked_field(fact) == Some(field));
        let (index, _) = linked.next().ok_or_else(|| self.missing(field))?;
        if linked.next().is_some() {
            return Err(self.missing(field));
        }
        let proof = self.block.fact(index)?;
        if let PrintedFact::Target(target) = proof.value {
            let [line, index, _] = target.tok.0;
            let region = self.block.quotes.region((line as usize, index as usize));
            if !self
                .block
                .block_regions
                .shares_region(proof.owner, owner, region)
            {
                return Err(self.block.error(Check::P11, field, proof.range));
            }
        }
        self.facts.insert(index);
        if let Some((_, run, _)) = facts::location(proof.value) {
            self.record(field, run);
        }
        self.meanings.entry(field.to_owned()).or_default();
        Ok(proof)
    }

    /// Check a production-specific field against exact tokens of its recorded
    /// owner. The family verifies the spelling, role and field value together.
    pub(crate) fn meaning(
        &mut self,
        field: &str,
        run @ [_, start, end]: [u32; 3],
        verify: impl FnOnce(&Value, &str) -> bool,
    ) -> Result<(), EvidenceError> {
        let owner = self
            .obligations
            .get(field)
            .ok_or_else(|| self.missing(field))?;
        let proof = self.block.node(owner)?;
        if start >= end || !contains(proof.value, run) {
            return Err(self.missing(field));
        }
        if !verify(self.block.ast.pointer(field).unwrap(), self.block.text(run)) {
            return Err(self.block.error(
                Check::P11,
                field,
                self.block.range(std::iter::once(run)).unwrap(),
            ));
        }
        self.record(field, run);
        Ok(())
    }

    /// Check a field of an unlocated object the parser synthesized inside its
    /// recorded parent production ("each other player" yields a filtered
    /// player whose `each-player` base has no span of its own). The run must
    /// be exactly the parent's single recorded span, with no folds, and the
    /// family verifies that whole printed phrase against the synthesized
    /// value. A recorded, aliased or implicit owner uses [`Self::meaning`]
    /// instead.
    pub(crate) fn synthesized(
        &mut self,
        field: &str,
        run @ [_, start, end]: [u32; 3],
        verify: impl FnOnce(&Value, &str) -> bool,
    ) -> Result<(), EvidenceError> {
        let owner = self
            .obligations
            .get(field)
            .ok_or_else(|| self.missing(field))?;
        let Some(PrintedNode::Unlocated(_)) = self.block.nodes.get(owner.as_str()) else {
            return Err(self.missing(field));
        };
        let (parent, _) = owner.rsplit_once('/').ok_or_else(|| self.missing(field))?;
        let Some(PrintedNode::Recorded(node)) = self.block.nodes.get(parent) else {
            return Err(self.missing(field));
        };
        let whole = matches!(node.span.as_slice(), [only] if only.0 == run)
            && node.folds.as_deref().is_none_or(<[_]>::is_empty);
        if start >= end || !whole {
            return Err(self.missing(field));
        }
        let text = self.block.text(run);
        if !verify(self.block.ast.pointer(field).unwrap(), text) {
            return Err(self.block.error(
                Check::P11,
                field,
                self.block.range(std::iter::once(run)).unwrap(),
            ));
        }
        self.record(field, run);
        Ok(())
    }

    /// Claim the printed words that introduce a composite child of a recorded
    /// production, such as "for each" before a for-each filter. The words lie
    /// in the parent's own runs, outside the child's, and end exactly where
    /// the child's first recorded run begins on the same line; the child's
    /// fields are discharged separately. Facts on the words bind to `child`.
    pub(crate) fn introducer(
        &mut self,
        child: &str,
        run @ [line, start, end]: [u32; 3],
        verify: impl FnOnce(&str) -> bool,
    ) -> Result<(), EvidenceError> {
        let (parent, _) = child.rsplit_once('/').ok_or_else(|| self.missing(child))?;
        if !self.block.ast.pointer(child).is_some_and(Value::is_object) {
            return Err(self.missing(child));
        }
        let proof = self.block.node(parent)?;
        let Some(PrintedNode::Recorded(node)) = self.block.nodes.get(child) else {
            return Err(self.missing(child));
        };
        let adjacent = node.span.first().is_some_and(|first| {
            let [l, s, _] = first.0;
            l == line && s == end
        });
        let inside_child =
            (start..end).any(|index| contains(self.block.nodes[child], [line, index, index + 1]));
        if proof.basis != Basis::Recorded
            || start >= end
            || !contains(proof.value, run)
            || !adjacent
            || inside_child
        {
            return Err(self.missing(child));
        }
        if !verify(self.block.text(run)) {
            return Err(self.block.error(
                Check::P11,
                child,
                self.block.range(std::iter::once(run)).unwrap(),
            ));
        }
        self.record(child, run);
        Ok(())
    }

    /// Attribute a verified fact to an already checked field over its own
    /// occurrence. An exact wire linkage cannot be reassigned.
    pub(crate) fn bind_fact(&mut self, index: usize, field: &str) -> Result<(), EvidenceError> {
        let proof = self.block.fact(index)?;
        let valid = facts::location(proof.value).is_some_and(|(_, [line, start, end], _)| {
            linked_field(proof.value).is_none_or(|linked| linked == field)
                && self
                    .meanings
                    .get(field)
                    .is_some_and(|tokens| (start..end).all(|index| tokens.contains(&(line, index))))
        });
        if !valid {
            return Err(self.block.error(Check::P11, field, proof.range));
        }
        self.facts.insert(index);
        Ok(())
    }

    /// Check a closed default at the governing object. The family proves its
    /// production-specific preconditions before claiming the default.
    pub(crate) fn implicit(
        &mut self,
        field: &str,
        reason: ImplicitReason,
    ) -> Result<(), EvidenceError> {
        let owner = self
            .obligations
            .get(field)
            .ok_or_else(|| self.missing(field))?;
        let property = field.strip_prefix(owner).unwrap();
        let allows = match reason {
            ImplicitReason::DefaultZone => property == "/zone",
            ImplicitReason::ImperativeController => property == "/player",
            ImplicitReason::ImpliedSingularTarget => matches!(property, "/amount" | "/value"),
        };
        let proof = self.block.node(owner)?;
        let recorded = match proof.basis {
            Basis::Implicit(reason)
            | Basis::Alias {
                default: Some(reason),
                ..
            } => Some(reason),
            _ => None,
        };
        if !allows || recorded != Some(&reason) {
            return Err(self.missing(field));
        }
        self.meanings.entry(field.to_owned()).or_default();
        Ok(())
    }

    /// Claim a delimiter at a verified production boundary. Operative
    /// punctuation is covered with `meaning` and `bind_fact`.
    pub(crate) fn punctuation(
        &mut self,
        line: u32,
        index: u32,
        role: StructuralPunctuation,
    ) -> Result<(), EvidenceError> {
        let proof = self.block.token(line, index)?;
        let missing = || {
            self.block.error(
                Check::P11,
                format!("lines/{line}/tokens/{index}"),
                proof.range,
            )
        };
        let PrintedToken::Punct(token) = proof.value else {
            return Err(missing());
        };
        let after = index + 1;
        let closes_sentence_quote = matches!(role, StructuralPunctuation::SentenceTerminator)
            && self.block.quotes.closes_region(
                (line as usize, index as usize),
                (line as usize, after as usize),
            );
        // A matching closing quote can touch the inner sentence's period.
        // Other touching tokens and moved punctuation need their own productions.
        if token.grammar_index != index
            || (!closes_sentence_quote
                && self
                    .block
                    .tokens(line)
                    .get(after as usize)
                    .is_some_and(|next| normalized_range(next)[0] == token.n.0[1]))
        {
            return Err(missing());
        }
        let mut matching = self
            .block
            .facts()
            .filter_map(|(fact_index, fact)| match fact {
                PrintedFact::Punct(punct) if punct.tok.0 == [line, index, after] => {
                    Some((fact_index, punct))
                }
                _ => None,
            });
        let (fact_index, fact) = matching.next().ok_or_else(missing)?;
        if matching.next().is_some() || !self.block.punctuation_boundary(role, fact) {
            return Err(missing());
        }
        self.tokens.insert((line, index));
        self.facts.insert(fact_index);
        Ok(())
    }

    pub(crate) fn fact(&self, index: usize) -> Result<Located<'a, &'a PrintedFact>, EvidenceError> {
        self.block.fact(index)
    }

    pub(crate) fn token(
        &self,
        line: u32,
        index: u32,
    ) -> Result<Located<'a, &'a PrintedToken>, EvidenceError> {
        self.block.token(line, index)
    }

    fn finish(self) -> Result<(), EvidenceError> {
        for (index, fact) in self.block.facts() {
            if !self.facts.contains(&index) {
                let range = match fact {
                    PrintedFact::ReminderCitation(cite) => cite.o.0,
                    _ => self
                        .block
                        .range(facts::location(fact).map(|(_, run, _)| run).into_iter())
                        .unwrap(),
                };
                let obligation = match facts::location(fact) {
                    Some((_, _, Nullable::Value(path))) => path.clone(),
                    _ => format!("fact/{index}"),
                };
                return Err(self.block.error(Check::P12, obligation, range));
            }
        }
        for field in self.obligations.keys() {
            if !self.meanings.contains_key(field) {
                return Err(self.missing(field));
            }
        }
        let [start, end] = self.block.block.lines.0;
        for line in start..end {
            for (index, token) in self.block.tokens(line).iter().enumerate() {
                if !self.tokens.contains(&(line, index as u32)) {
                    return Err(self.block.error(
                        Check::P12,
                        format!("lines/{line}/tokens/{index}"),
                        token_range(token),
                    ));
                }
            }
        }
        Ok(())
    }
}
