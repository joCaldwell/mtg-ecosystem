//! Evidence for opt-in Rust families. Only R6 intake constructs an R6 view.
//! The resolver declares its obligations independently of the citations it finds.
use super::{facts, nodes, text};
use crate::{Failure, unsupported};
use mtg_oracle_ast::printed::*;
use std::collections::{BTreeMap, BTreeSet};

/// A view of a validated card. Excluded records have no view.
pub(crate) struct CardEvidence(Option<PrintedCard>);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Check {
    RequiresR6,
    P11,
    P12,
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
    block: &'a PrintedBlockRead,
    nodes: BTreeMap<&'a str, &'a PrintedNode>,
}

impl CardEvidence {
    pub(in crate::ingest) fn verified(printed: PrintedCard) -> Self {
        Self(Some(printed))
    }

    pub(in crate::ingest) fn legacy() -> Self {
        Self(None)
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
        let printed = self
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
        Ok(BlockEvidence {
            face_index: face,
            block_index: block,
            face: read,
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

impl<'a> BlockEvidence<'a> {
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

    fn token(&self, line: u32, index: u32) -> Result<Located<'a, &'a PrintedToken>, EvidenceError> {
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

    /// A family covers one printed block at a time. Multiple checked meanings
    /// may share a citation. Querying a node does not discharge its contents.
    pub(crate) fn check_family<T>(
        &self,
        resolve: impl FnOnce(&mut Coverage<'_, 'a>) -> Result<T, EvidenceError>,
    ) -> Result<T, EvidenceError> {
        let mut coverage = Coverage {
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
    facts: BTreeSet<usize>,
    tokens: BTreeSet<(u32, u32)>,
}

impl<'a> Coverage<'_, 'a> {
    /// Cite a field that P9 or P10 matched to a fact. Inferred fields must
    /// cite their exact implicit node and check its reason instead.
    pub(crate) fn field(
        &mut self,
        field: &str,
    ) -> Result<Located<'a, &'a PrintedFact>, EvidenceError> {
        let index = self
            .block
            .facts()
            .find_map(|(index, fact)| {
                let linked = match fact {
                    PrintedFact::ReminderCitation(cite) => Some(cite.field.as_str()),
                    _ => match facts::location(fact).unwrap().2 {
                        Nullable::Value(path) => Some(path.as_str()),
                        Nullable::Null => None,
                    },
                };
                (linked == Some(field)).then_some(index)
            })
            .ok_or_else(|| {
                self.block
                    .error(Check::P11, field, self.block.container(field))
            })?;
        self.fact(index)
    }

    pub(crate) fn fact(
        &mut self,
        index: usize,
    ) -> Result<Located<'a, &'a PrintedFact>, EvidenceError> {
        let proof = self.block.fact(index)?;
        self.facts.insert(index);
        if let Some((_, [line, start, end], _)) = facts::location(proof.value) {
            self.tokens.extend((start..end).map(|token| (line, token)));
        }
        Ok(proof)
    }

    pub(crate) fn token(
        &mut self,
        line: u32,
        index: u32,
    ) -> Result<Located<'a, &'a PrintedToken>, EvidenceError> {
        let proof = self.block.token(line, index)?;
        self.tokens.insert((line, index));
        Ok(proof)
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
