//! Evidence checks for the complete grouped-search production. The grammar
//! supplies the field, value and range independently of the archived citations.
use super::{Gap, Value, word_number};
use crate::ingest::evidence::{BlockEvidence, Coverage, EvidenceError, StructuralPunctuation};
use mtg_oracle_ast::printed::{ImplicitReason, Nullable, PrintedFact, PrintedFactPunctRole};
use regex::Regex;
use serde::Serialize;
use serde_json::json;
use std::collections::{BTreeMap, BTreeSet};
use std::sync::LazyLock;

static TOKEN: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"\{[A-Za-z0-9]+\}|[A-Za-z0-9]+(?:-[A-Za-z0-9]+)*|[.,:/]").unwrap()
});

/// A contiguous run of grammar tokens on the block's one line.
#[derive(Clone, Copy)]
pub(super) struct Run(u32, u32);

impl Run {
    pub(super) fn first(self, count: u32) -> Self {
        Self(self.0, self.0 + count)
    }

    pub(super) fn last(self, count: u32) -> Self {
        Self(self.1 - count, self.1)
    }

    pub(super) fn within(self, offset: u32, count: u32) -> Self {
        Self(self.0 + offset, self.0 + offset + count)
    }

    pub(super) fn join(self, other: Self) -> Self {
        Self(self.0, other.1)
    }
}

pub(super) struct Proof<'text, 'check, 'block, 'evidence> {
    ability: &'text Value,
    text: &'text str,
    words: Vec<regex::Match<'text>>,
    pos: u32,
    line: u32,
    coverage: Option<&'check mut Coverage<'block, 'evidence>>,
    meanings: BTreeMap<String, BTreeSet<u32>>,
    commas: BTreeMap<u32, String>,
    structural: BTreeSet<u32>,
}

impl<'text, 'check, 'block, 'evidence> Proof<'text, 'check, 'block, 'evidence> {
    pub(super) fn new(
        ability: &'text Value,
        text: &'text str,
        line: u32,
        coverage: Option<&'check mut Coverage<'block, 'evidence>>,
    ) -> Self {
        Self {
            ability,
            text,
            words: TOKEN.find_iter(text).collect(),
            pos: 0,
            line,
            coverage,
            meanings: BTreeMap::new(),
            commas: BTreeMap::new(),
            structural: BTreeSet::new(),
        }
    }

    pub(super) fn position(&self) -> u32 {
        self.pos
    }

    pub(super) fn since(&self, start: u32) -> Run {
        Run(start, self.pos)
    }

    pub(super) fn back(&self, count: u32) -> Run {
        Run(self.pos - count, self.pos)
    }

    pub(super) fn spelling(&self, run: Run) -> &'text str {
        &self.text[self.words[run.0 as usize].start()..self.words[run.1 as usize - 1].end()]
    }

    pub(super) fn peek(&self, token: &str) -> bool {
        self.words
            .get(self.pos as usize)
            .is_some_and(|m| m.as_str().eq_ignore_ascii_case(token))
    }

    pub(super) fn peek_phrase(&self, phrase: &str) -> bool {
        let count = TOKEN.find_iter(phrase).count() as u32;
        let run = Run(self.pos, self.pos + count);
        run.1 <= self.words.len() as u32 && self.spelling(run).eq_ignore_ascii_case(phrase)
    }

    pub(super) fn take(&mut self, phrase: &str, path: &str) -> Result<Run, Gap> {
        if !self.peek_phrase(phrase) {
            return Err(path.to_owned());
        }
        let start = self.pos;
        self.pos += TOKEN.find_iter(phrase).count() as u32;
        Ok(self.since(start))
    }

    pub(super) fn either(&mut self, first: &str, second: &str, path: &str) -> Result<Run, Gap> {
        self.take(if self.peek(first) { first } else { second }, path)
    }

    pub(super) fn word(&mut self, path: &str) -> Result<(String, Run), Gap> {
        let word = self
            .words
            .get(self.pos as usize)
            .ok_or_else(|| path.to_owned())?
            .as_str();
        let run = self.take(word, path)?;
        Ok((word.to_ascii_lowercase(), run))
    }

    pub(super) fn mana_symbol(&mut self) -> Option<&'text str> {
        let symbol = self
            .words
            .get(self.pos as usize)?
            .as_str()
            .strip_prefix('{')?
            .strip_suffix('}')?;
        if symbol.eq_ignore_ascii_case("t") {
            return None;
        }
        self.pos += 1;
        Some(symbol)
    }

    pub(super) fn mark(
        &mut self,
        field: &str,
        expected: impl Serialize,
        run: Run,
    ) -> Result<(), Gap> {
        let expected = json!(expected);
        let phrase = self.spelling(run);
        if self.ability.pointer(field) != Some(&expected) {
            return Err(field.into());
        }
        if let Some(coverage) = self.coverage.as_deref_mut() {
            coverage
                .meaning(field, [self.line, run.0, run.1], |value, text| {
                    value == &expected && text.eq_ignore_ascii_case(phrase)
                })
                .map_err(|e| e.obligation)?;
            self.meanings
                .entry(field.to_owned())
                .or_default()
                .extend(run.0..run.1);
        }
        Ok(())
    }

    fn implicit(&mut self, field: &str, value: &str, reason: ImplicitReason) -> Result<(), Gap> {
        if self.ability.pointer(field) != Some(&json!(value)) {
            return Err(field.to_owned());
        }
        if let Some(coverage) = self.coverage.as_deref_mut() {
            coverage.implicit(field, reason).map_err(|e| e.obligation)?;
        }
        Ok(())
    }

    pub(super) fn controller(&mut self, field: &str) -> Result<(), Gap> {
        self.implicit(field, "you", ImplicitReason::ImperativeController)
    }

    pub(super) fn zone_default(&mut self, field: &str, zone: &str) -> Result<(), Gap> {
        self.implicit(field, zone, ImplicitReason::DefaultZone)
    }

    pub(super) fn count(&mut self, path: &str, value: i64, run: Run) -> Result<(), Gap> {
        self.mark(&format!("{path}/amount"), "fixed", run)?;
        self.mark(&format!("{path}/value"), value, run)?;
        if let Some(coverage) = self.coverage.as_deref_mut() {
            let field = format!("{path}/value");
            let fact = coverage.field(&field).map_err(|e| e.obligation)?.value;
            if !matches!(fact, PrintedFact::Quantity(fact)
                if fact.tok.0 == [self.line, run.0, run.1]
                    && matches!(fact.value, Nullable::Value(n) if i64::from(n) == value))
            {
                return Err(field);
            }
        }
        Ok(())
    }

    pub(super) fn structure_run(&mut self, run: Run, _path: &str) -> Result<(), Gap> {
        // All three headers scope the whole one-line search production.
        let kind = self.ability["kind"].clone();
        self.mark("/kind", kind, run)
    }

    pub(super) fn structure(&mut self, token: &str, path: &str) -> Result<(), Gap> {
        let run = self.take(token, path)?;
        self.structure_run(run, path)?;
        self.commas.insert(run.0, path.to_owned());
        Ok(())
    }

    fn punctuation(
        &mut self,
        symbol: &str,
        path: &str,
        role: StructuralPunctuation,
    ) -> Result<(), Gap> {
        let run = self.take(symbol, path)?;
        if let Some(coverage) = self.coverage.as_deref_mut() {
            coverage
                .punctuation(self.line, run.0, role)
                .map_err(|e| e.obligation)?;
            self.structural.insert(run.0);
        }
        Ok(())
    }

    pub(super) fn period(&mut self, path: &str) -> Result<(), Gap> {
        self.punctuation(".", path, StructuralPunctuation::SentenceTerminator)
    }

    pub(super) fn cost_comma(&mut self, path: &str) -> Result<(), Gap> {
        self.punctuation(",", path, StructuralPunctuation::CostListComma)
    }

    pub(super) fn cost_colon(&mut self, path: &str) -> Result<(), Gap> {
        self.punctuation(":", path, StructuralPunctuation::CostEffectColon)
    }

    pub(super) fn clause_comma(&mut self, path: &str) -> Result<(), Gap> {
        self.punctuation(",", path, StructuralPunctuation::ClauseComma)
    }

    pub(super) fn complete(&self, expected: &Value) -> Result<(), Gap> {
        // The local tokenization never drops non-whitespace input.
        let mut last = 0;
        for token in &self.words {
            if !self.text[last..token.start()]
                .chars()
                .all(char::is_whitespace)
            {
                return Err(String::new());
            }
            last = token.end();
        }
        if self.pos as usize != self.words.len()
            || !self.text[last..].chars().all(char::is_whitespace)
            || self.ability != expected
        {
            return Err(String::new());
        }
        Ok(())
    }

    pub(super) fn cover_facts(&mut self, block: &BlockEvidence<'_>) -> Result<(), EvidenceError> {
        let mut bindings = Vec::new();
        let spelling = |[line, start, end]: [u32; 3]| {
            (line == self.line && start < end && end as usize <= self.words.len())
                .then(|| self.spelling(Run(start, end)))
        };
        let matches = |owner: &str, run: [u32; 3], linked: &Nullable<String>| {
            self.meanings.iter().find_map(|(path, tokens)| {
                (path
                    .strip_prefix(owner)
                    .is_some_and(|tail| tail.starts_with('/'))
                    && (run[1]..run[2]).all(|i| tokens.contains(&i))
                    && !matches!(linked, Nullable::Value(field) if field != path))
                .then_some(path.as_str())
            })
        };
        for (index, fact) in block.facts() {
            let field = match fact {
                PrintedFact::Punct(fact) if self.structural.contains(&fact.tok.0[1]) => continue,
                PrintedFact::Punct(fact)
                    if fact.role == PrintedFactPunctRole::Comma
                        && self.commas.get(&fact.tok.0[1]) == Some(&fact.at) =>
                {
                    Some("/kind")
                }
                PrintedFact::Punct(fact)
                    if fact.role == PrintedFactPunctRole::Other
                        && spelling(fact.tok.0) == Some("/") =>
                {
                    let field = format!("{}/combine", fact.at);
                    self.meanings
                        .get_key_value(&field)
                        .filter(|(_, tokens)| tokens.contains(&fact.tok.0[1]))
                        .map(|(path, _)| path.as_str())
                }
                PrintedFact::Mention(fact) if fact.possessive.is_none() => spelling(fact.tok.0)
                    .is_some_and(|text| text.eq_ignore_ascii_case(fact.form.as_str()))
                    .then(|| matches(&fact.at, fact.tok.0, &fact.field))
                    .flatten(),
                PrintedFact::Introducer(fact) => {
                    let Some(spelling) = spelling(fact.tok.0).map(str::to_ascii_lowercase) else {
                        continue;
                    };
                    let form = match spelling.as_str() {
                        "a" | "an" => "a",
                        "up to" => "up-to",
                        "the" => "the",
                        n if word_number(n).is_some() || n.parse::<u32>().is_ok() => "numeral",
                        _ => continue,
                    };
                    (form == fact.form.as_str())
                        .then(|| matches(&fact.at, fact.tok.0, &fact.field))
                        .flatten()
                }
                PrintedFact::Optional(fact) if fact.form.as_str() == "up-to" => {
                    matches(&fact.at, fact.tok.0, &fact.field)
                        .filter(|field| field.ends_with("/upTo"))
                }
                // Required and linked to its number field by count().
                PrintedFact::Quantity(_) => continue,
                _ => continue,
            };
            if let Some(field) = field {
                bindings.push((index, field));
            }
        }
        if let Some(coverage) = self.coverage.as_deref_mut() {
            for (index, field) in bindings {
                coverage.bind_fact(index, field)?;
            }
        }
        Ok(())
    }
}
