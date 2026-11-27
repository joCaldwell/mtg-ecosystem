//! Quotation regions reconstructed from the tokens replayed against Oracle text.
use super::{text, tokens::Line};
use mtg_oracle_ast::printed::Edit;
use std::collections::BTreeMap;

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
}

#[derive(Debug, Default)]
pub(super) struct BlockRegions(BTreeMap<String, Option<Region>>);

impl BlockRegions {
    pub(super) fn get(&self, path: &str) -> Option<Region> {
        self.0.get(path).copied().unwrap_or(Some(Region::Outside))
    }

    pub(super) fn enter(&mut self, path: String, region: Option<Region>) {
        self.0.insert(path, region);
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
            for index in order {
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

    // Removed reminder citations have only original-byte ranges.
    pub(super) fn intersects_original(&self, [start, end]: [u32; 2]) -> bool {
        self.openings.values().any(|entry| {
            entry.original_start < end && entry.original_end.is_some_and(|close| start < close)
        })
    }
}
