//! Quotation regions reconstructed from the tokens replayed against Oracle text.
use super::{
    text,
    tokens::{Line, Token},
};
use mtg_oracle_ast::printed::Edit;
use std::collections::BTreeMap;
use std::ops::Range;

pub(super) type Occurrence = (usize, usize);

#[derive(Clone, Copy, Debug, Default, Eq, Ord, PartialEq, PartialOrd)]
pub(super) enum Region {
    #[default]
    Outside,
    Inside(Occurrence),
}

#[derive(Debug)]
struct Opening {
    parent: Region,
    end: Option<Occurrence>,
    original_start: u32,
    original_end: Option<u32>,
    term: Option<TermClause>,
}

#[derive(Debug, PartialEq, Eq)]
pub(super) enum Term {
    LegendRule,
    BandsWithOther,
}

#[derive(Debug)]
pub(super) struct TermClause {
    pub kind: Term,
    pub tokens: Vec<Occurrence>,
}

fn term_clause(
    line: usize,
    tokens: &[Token<'_>],
    order: &[usize],
    open: usize,
    close: usize,
) -> Option<TermClause> {
    // Mirror parser/legend-rule.ts and parser/ability-grants.ts. Match in
    // grammar order and leave AST ownership to the block verifier. Like
    // parseGainedAbilities, allow the nested apostrophe delimiters here.
    let delimiter = tokens[open].punct()?;
    let start = tokens[open].grammar_index.checked_sub(1)?;
    let end = tokens[close].grammar_index;
    let (kind, words) = match end - start {
        4 => (
            Term::LegendRule,
            &["the", "\"", "legend", "rule", "\"", "doesn't", "apply"][..],
        ),
        5 => (
            Term::BandsWithOther,
            &["all", "\"", "bands", "with", "other", "\"", "abilities"][..],
        ),
        _ => return None,
    };
    let clause = order.get(start..start + words.len())?;
    clause
        .iter()
        .zip(words)
        .all(|(&index, &word)| {
            let token = &tokens[index];
            if word == "\"" {
                token.punct() == Some(delimiter)
            } else {
                token.word() == word && !token.possessive()
            }
        })
        .then(|| TermClause {
            kind,
            tokens: clause.iter().map(|&index| (line, index)).collect(),
        })
}

#[derive(Debug, Default)]
pub(super) struct BlockRegions {
    nodes: BTreeMap<String, Option<Region>>,
    pub terms: BTreeMap<Occurrence, String>,
}

impl BlockRegions {
    pub(super) fn get(&self, path: &str) -> Option<Region> {
        self.nodes
            .get(path)
            .copied()
            .unwrap_or(Some(Region::Outside))
    }

    pub(super) fn enter(&mut self, path: String, region: Option<Region>) {
        self.nodes.insert(path, region);
    }

    pub(super) fn allows_fact(&self, path: &str, region: Region) -> bool {
        self.get(path) == Some(region)
            || matches!(region, Region::Inside(open) if self.terms.get(&open).is_some_and(|owner| owner == path))
    }
}

#[derive(Debug, Default)]
pub(super) struct Quotes {
    openings: BTreeMap<Occurrence, Opening>,
    enclosing: BTreeMap<Occurrence, Region>,
}

impl Quotes {
    pub(super) fn new(lines: &[Line<'_>], edits: &[Edit]) -> Self {
        let mut quotes = Self::default();
        for (line, data) in lines.iter().enumerate() {
            let tokens = &data.tokens;
            if !tokens.iter().any(|token| token.punct() == Some("\"")) {
                continue;
            }
            // P3 proves the spelling after normalize.ts / printed/text.ts.
            // P4/P5 re-lex the physical tokens and replay lexer.ts's closing
            // punctuation moves. Names and possessive apostrophes stay in
            // their own tokens. A free apostrophe within double quotes nests.
            let mut order = vec![0; tokens.len()];
            for (index, token) in tokens.iter().enumerate() {
                order[token.grammar_index] = index;
            }
            let mut stack = Vec::new();
            for &index in &order {
                let occurrence = (line, index);
                let token = &tokens[index];
                let mark = token.punct().filter(|mark| matches!(*mark, "\"" | "'"));
                // Delimiter tokens belong to the enclosing production. Only
                // the tokens between a matched pair are in its child region.
                if let Some(mark) = mark
                    && let Some(pos) = stack.iter().rposition(|(open, _)| *open == mark)
                {
                    let (_, opening) = stack[pos];
                    let entry = quotes.openings.get_mut(&opening).unwrap();
                    entry.end = Some(occurrence);
                    entry.original_end = Some(text::hull(token.n, edits)[0]);
                    entry.term = term_clause(line, tokens, &order, opening.1, index);
                    stack.truncate(pos);
                    if let Some((_, parent)) = stack.last() {
                        quotes.enclosing.insert(occurrence, Region::Inside(*parent));
                    }
                    continue;
                }
                let parent = stack
                    .last()
                    .map_or(Region::Outside, |(_, open)| Region::Inside(*open));
                if parent != Region::Outside {
                    quotes.enclosing.insert(occurrence, parent);
                }
                if let Some(mark) = mark {
                    quotes.openings.insert(
                        occurrence,
                        Opening {
                            parent,
                            end: None,
                            original_start: text::hull(token.n, edits)[1],
                            original_end: None,
                            term: None,
                        },
                    );
                    stack.push((mark, occurrence));
                }
            }
        }
        quotes
    }

    pub(super) fn region(&self, occurrence: Occurrence) -> Region {
        self.enclosing.get(&occurrence).copied().unwrap_or_default()
    }

    pub(super) fn enclosure(&self, open: Occurrence) -> Option<(Region, Occurrence)> {
        let entry = self.openings.get(&open)?;
        Some((entry.parent, entry.end?))
    }

    pub(super) fn term(&self, open: Occurrence) -> Option<&TermClause> {
        self.openings.get(&open)?.term.as_ref()
    }

    pub(super) fn matched(&self, block: Range<usize>) -> impl Iterator<Item = Occurrence> + '_ {
        self.openings
            .range((block.start, 0)..(block.end, 0))
            .filter_map(|(&open, entry)| entry.end.map(|_| open))
    }

    // Removed reminder citations have only original-byte ranges.
    pub(super) fn intersects_original(&self, [start, end]: [u32; 2]) -> bool {
        self.openings.values().any(|entry| {
            entry.original_start < end && entry.original_end.is_some_and(|close| start < close)
        })
    }
}
