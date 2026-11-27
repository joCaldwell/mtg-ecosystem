use super::{Failure, invalid};
use mtg_contracts::{R6Pins, Sha256Sink};
use mtg_oracle_ast::printed::*;
use regex::Regex;
use serde_json::Value;
use std::{collections::BTreeMap, sync::LazyLock};

fn pattern(expression: &str) -> Regex {
    Regex::new(expression).expect("valid Oracle normalization pattern")
}

static BOUNDARY: LazyLock<Regex> = LazyLock::new(|| pattern(r"^[\p{L}\p{M}\p{N}_]$"));
static LITERAL_SLOT: LazyLock<Regex> =
    LazyLock::new(|| pattern(r"(?i)\b(named|partner\s+with|creates?)\s+$"));
static DIFFERENTLY_NAMED: LazyLock<Regex> =
    LazyLock::new(|| pattern(r"(?i)\bdifferently\s+named\s+$"));
static NAMED: LazyLock<Regex> = LazyLock::new(|| pattern(r#"(?i)\bnamed\s+"?$"#));
static CREATE_ARTICLE: LazyLock<Regex> = LazyLock::new(|| pattern(r"^,\s+(?:a|an)\s+"));
static EPITHET: LazyLock<Regex> = LazyLock::new(|| pattern(r"^(.+?) (?:the|of) ."));
static TYPE_SPLIT: LazyLock<Regex> = LazyLock::new(|| pattern(r"—|\s-\s"));
static HER_FOLLOWER: LazyLock<Regex> = LazyLock::new(|| pattern(r"^\s+([\p{L}'-]+)"));

#[derive(Default)]
pub(in crate::ingest) struct NameCatalog(BTreeMap<String, u8>);

const CARD: u8 = 1;
const TOKEN: u8 = 2;

#[derive(Default)]
pub(in crate::ingest) struct NameClaims {
    positive: Vec<(String, u8)>,
    absent: Vec<(String, u8)>,
}

impl NameCatalog {
    pub(in crate::ingest) fn add_source(&mut self, source: &Value) -> Result<(), &'static str> {
        let kinds = match source["layout"].as_str().ok_or("missing source layout")? {
            "token" | "double_faced_token" => TOKEN,
            "art_series" | "emblem" => 0,
            _ => CARD,
        };
        let faces = source["card_faces"].as_array();
        self.add(
            source,
            if faces.is_some_and(|f| !f.is_empty()) {
                0
            } else {
                kinds
            },
        )?;
        for face in faces.into_iter().flatten() {
            self.add(face, kinds)?;
        }
        Ok(())
    }

    fn add(&mut self, source: &Value, kind: u8) -> Result<(), &'static str> {
        let name = source["name"]
            .as_str()
            .filter(|name| !name.trim().is_empty() && !name.contains(['\r', '\n']))
            .ok_or("invalid source name catalog entry")?;
        *self.0.entry(name.to_owned()).or_default() |= kind;
        Ok(())
    }

    pub(in crate::ingest) fn verify(
        self,
        claims: &NameClaims,
        pins: &R6Pins,
    ) -> Result<(), String> {
        let mut hash = Sha256Sink::new();
        for name in self.0.keys() {
            hash.update(name.as_bytes());
            hash.update(b"\n");
        }
        if self.0.len() != pins.name_catalog_names
            || hash.finalize_hex() != pins.name_catalog_sha256
        {
            return Err("R6 name catalog count or digest mismatch".into());
        }
        let mut canonical = BTreeMap::new();
        for (name, kind) in &self.0 {
            canonical
                .entry(punctuation(name))
                .or_insert_with(Vec::new)
                .push((name, *kind));
        }
        for (name, kinds) in &claims.positive {
            let mut matches = canonical
                .get(&punctuation(name))
                .into_iter()
                .flatten()
                .filter(|(_, kind)| kind & kinds != 0);
            if matches.next().is_none_or(|(found, _)| *found != name) || matches.next().is_some() {
                return Err(format!(
                    "printed.P3: literal name {name:?} has no unique eligible catalog spelling"
                ));
            }
        }
        for (name, kinds) in &claims.absent {
            if canonical
                .get(&punctuation(name))
                .into_iter()
                .flatten()
                .filter(|(_, kind)| kind & kinds != 0)
                .count()
                == 1
            {
                return Err(format!(
                    "printed.P4: failed lex claims known literal name {name:?}"
                ));
            }
        }
        Ok(())
    }
}

impl NameClaims {
    pub(super) fn known(&mut self, name: String, partner: bool) {
        self.positive
            .push((name, if partner { CARD } else { CARD | TOKEN }));
    }

    pub(super) fn absent(&mut self, name: String, partner: bool) {
        self.absent
            .push((name, if partner { CARD } else { CARD | TOKEN }));
    }
}

fn punct(ch: char) -> char {
    match ch {
        '’' => '\'',
        '“' | '”' => '"',
        '−' => '-',
        '–' => '—',
        ch => ch,
    }
}

pub(super) fn punctuation(text: &str) -> String {
    text.chars().map(punct).collect()
}

pub(super) fn slice(text: &str, [start, end]: [u32; 2]) -> Option<&str> {
    text.get(start as usize..end as usize)
}

pub(super) fn ranges(edit: &Edit) -> ([u32; 2], [u32; 2]) {
    match edit {
        Edit::Gap(edit) => (edit.o.0, edit.n.0),
        Edit::LiteralName(edit) => (edit.o.0, edit.n.0),
        Edit::Pronoun(edit) => (edit.o.0, edit.n.0),
        Edit::Punct(edit) => (edit.o.0, edit.n.0),
        Edit::SelfName(edit) => (edit.o.0, edit.n.0),
    }
}

fn balanced(text: &str) -> bool {
    if !text.starts_with('(') || !text.ends_with(')') {
        return false;
    }
    let mut depth = 0usize;
    for (index, ch) in text.char_indices() {
        match ch {
            '(' => depth += 1,
            ')' => {
                depth -= 1;
                if depth == 0 && index + 1 != text.len() {
                    return false;
                }
            }
            _ => {}
        }
    }
    depth == 0
}

fn whitespace(text: &str) -> bool {
    text.chars()
        .all(|ch| (ch.is_whitespace() || ch == '\u{feff}') && ch != '\u{85}')
}

fn bounded(before: &str, after: &str, pronoun: bool) -> bool {
    let word = |ch: char| {
        BOUNDARY.is_match(ch.encode_utf8(&mut [0; 4])) || pronoun && matches!(ch, '\'' | '-')
    };
    !before.chars().next_back().is_some_and(word) && !after.chars().next().is_some_and(word)
}

fn type_parts(type_line: &str) -> (&str, &str) {
    let mut parts = TYPE_SPLIT.split(type_line);
    (parts.next().unwrap_or(""), parts.next().unwrap_or(""))
}

fn legendary(types: &str) -> bool {
    types.split_whitespace().any(|t| t == "Legendary")
        && types
            .split_whitespace()
            .any(|t| matches!(t, "Creature" | "Planeswalker"))
}

fn short_name<'a>(name: &'a str, type_line: &str) -> Option<(&'a str, EditSelfNameForm)> {
    use EditSelfNameForm::*;
    if let Some((before, _)) = name
        .split_once(',')
        .filter(|(before, _)| !before.is_empty())
    {
        return Some((before.trim(), BeforeComma));
    }
    let first = name.split(' ').next()?;
    let (types, subtypes) = type_parts(type_line);
    let subtype = subtypes.split_whitespace().any(|t| t == first);
    if types.split_whitespace().any(|t| t == "Planeswalker") {
        return (name != first && subtype).then_some((first, PlaneswalkerSubtype));
    }
    if !legendary(types) {
        return None;
    }
    if let Some(captures) = EPITHET.captures(name) {
        return Some((captures.get(1)?.as_str(), BeforeEpithet));
    }
    (name != first && !subtype && first.starts_with(char::is_uppercase))
        .then_some((first, FirstWord))
}

fn pronoun(printed: &str, suffix: &str) -> Option<EditPronounRole> {
    use EditPronounRole::*;
    match printed.to_ascii_lowercase().as_str() {
        "he" | "she" => Some(Subject),
        "him" => Some(Object),
        "his" => Some(Possessive),
        "he's" | "she's" => Some(Contraction),
        "himself" | "herself" => Some(Reflexive),
        "her" => match HER_FOLLOWER
            .captures(suffix)
            .map(|m| m[1].to_lowercase())
            .as_deref()
        {
            Some(
                "owner's" | "owners'" | "controller's" | "power" | "toughness" | "name" | "other"
                | "sneak" | "loyalty" | "additional" | "mana" | "base",
            ) => Some(Possessive),
            Some(
                "to" | "into" | "onto" | "and" | "for" | "from" | "deal" | "unless" | "this" | "is"
                | "equal",
            ) => Some(Object),
            None if suffix.trim_start().is_empty()
                || suffix.trim_start().starts_with(['.', ',', ';', ':', '"']) =>
            {
                Some(Object)
            }
            _ => None,
        },
        _ => None,
    }
}

fn pronoun_replacement(printed: &str, role: &EditPronounRole, form: &EditPronounAs) -> String {
    use EditPronounRole::*;
    let spelling = match (form, role) {
        (EditPronounAs::Name, Subject | Object | Reflexive) => "~",
        (EditPronounAs::Name, Possessive) => "~'s",
        (EditPronounAs::Name, Contraction) => "~ is",
        (EditPronounAs::Neuter, Subject | Object) => "it",
        (EditPronounAs::Neuter, Possessive) => "its",
        (EditPronounAs::Neuter, Contraction) => "it's",
        (EditPronounAs::Neuter, Reflexive) => "itself",
    };
    if *form == EditPronounAs::Neuter && printed != printed.to_ascii_lowercase() {
        format!("I{}", &spelling[1..])
    } else {
        spelling.into()
    }
}

pub(super) fn verify(
    original: &str,
    normalized: &str,
    edits: &[Edit],
    reminders: &[RemovedReminder],
    name: &str,
    type_line: &str,
    claims: &mut NameClaims,
) -> Result<(), Failure> {
    let check = |detail| invalid("P3", detail);
    if original.contains(['~', '\0']) {
        return Err(check("reserved character in mapped original text"));
    }
    let name = punctuation(name);
    let article = name.strip_prefix("The ").map(|rest| format!("the {rest}"));
    let mut removed = reminders.iter();
    let mut o_end = 0;
    let mut n_end = 0;
    for (index, edit) in edits.iter().enumerate() {
        let ([o_start, o_next], [n_start, n_next]) = ranges(edit);
        let fail = || invalid("P3", format!("inadmissible normalization edit {index}"));
        let (before, printed, after) = (
            slice(original, [0, o_start]).ok_or_else(fail)?,
            slice(original, [o_start, o_next])
                .filter(|s| !s.is_empty())
                .ok_or_else(fail)?,
            original.get(o_next as usize..).ok_or_else(fail)?,
        );
        let (prefix, replacement, suffix) = (
            slice(normalized, [0, n_start]).ok_or_else(fail)?,
            slice(normalized, [n_start, n_next]).ok_or_else(fail)?,
            normalized.get(n_next as usize..).ok_or_else(fail)?,
        );
        let copied = slice(original, [o_end, o_start]).ok_or_else(fail)?;
        if Some(copied) != slice(normalized, [n_end, n_start])
            || copied.chars().any(|ch| punct(ch) != ch)
        {
            return Err(fail());
        }
        let canonical = punctuation(printed);
        let admissible = match edit {
            Edit::Punct(_) => {
                printed.chars().count() == 1 && printed != canonical && replacement == canonical
            }
            Edit::Gap(gap) => {
                let mut from = o_start;
                for reminder in &gap.reminder {
                    let [start, end] = reminder.0;
                    if end > o_next
                        || !slice(original, [from, start]).is_some_and(whitespace)
                        || !slice(original, reminder.0).is_some_and(balanced)
                        || !removed.next().is_some_and(|cite| {
                            cite.edit_index as usize == index && cite.o == *reminder
                        })
                    {
                        return Err(fail());
                    }
                    from = end;
                }
                let expected = if before.is_empty() || after.is_empty() {
                    ""
                } else if printed.contains('\n') {
                    "\n"
                } else {
                    " "
                };
                slice(original, [from, o_next]).is_some_and(whitespace)
                    && expected == replacement
                    && printed != expected
            }
            Edit::SelfName(edit) => {
                let spelling = match edit.form {
                    EditSelfNameForm::Full => Some(name.as_str()),
                    EditSelfNameForm::Article => article.as_deref(),
                    _ => short_name(&name, type_line)
                        .filter(|(_, form)| *form == edit.form)
                        .map(|(spelling, _)| spelling),
                };
                spelling == Some(canonical.as_str())
                    && bounded(before, after, false)
                    && !NAMED.is_match(prefix)
                    && replacement == "~"
            }
            Edit::Pronoun(edit) => {
                let line_prefix = prefix.rsplit('\n').next().unwrap_or("");
                let line_suffix = suffix.split('\n').next().unwrap_or("");
                let anchor = line_prefix.to_ascii_lowercase();
                legendary(type_parts(type_line).0)
                    && !name.is_empty()
                    && (anchor.contains('~')
                        || anchor.contains("this card")
                        || edits[..index]
                            .iter()
                            .rev()
                            .take_while(|e| {
                                ranges(e).1[1] as usize > prefix.len() - line_prefix.len()
                            })
                            .any(|e| matches!(e, Edit::Pronoun(_))))
                    && bounded(&punctuation(before), &punctuation(after), true)
                    && pronoun(&canonical, line_suffix).as_ref() == Some(&edit.role)
                    && replacement == pronoun_replacement(&canonical, &edit.role, &edit.r#as)
            }
            Edit::LiteralName(edit) => {
                let output: String = serde_json::from_str(replacement).map_err(|_| fail())?;
                if serde_json::to_string(&output).map_err(|_| fail())? != replacement {
                    return Err(fail());
                }
                let quoted: Option<String> = serde_json::from_str(&canonical).ok();
                let spelling = quoted.as_deref().unwrap_or(&canonical);
                let context = LITERAL_SLOT
                    .captures(&punctuation(before))
                    .map(|m| m[1].to_ascii_lowercase());
                let slot = match context.as_deref() {
                    Some("named") => Some(EditLiteralNameSlot::Named),
                    Some("create" | "creates") => Some(EditLiteralNameSlot::Create),
                    Some(_) => Some(EditLiteralNameSlot::PartnerWith),
                    None => None,
                };
                if output.trim().is_empty() || punctuation(&output) != punctuation(spelling) {
                    return Err(fail());
                }
                match edit.via {
                    EditLiteralNameVia::OwnName => {
                        edit.slot == EditLiteralNameSlot::Named
                            && NAMED.is_match(prefix)
                            && quoted.is_none()
                            && (canonical == name || article.as_deref() == Some(&canonical))
                            && output == canonical
                    }
                    EditLiteralNameVia::Catalog => {
                        let partner = edit.slot == EditLiteralNameSlot::PartnerWith;
                        claims
                            .positive
                            .push((output, if partner { CARD } else { CARD | TOKEN }));
                        slot.as_ref() == Some(&edit.slot)
                            && (quoted.is_some() || bounded(before, after, false))
                            && (edit.slot != EditLiteralNameSlot::Create
                                || CREATE_ARTICLE.is_match(after))
                            && (edit.slot != EditLiteralNameSlot::Named
                                || !DIFFERENTLY_NAMED.is_match(before))
                    }
                }
            }
        };
        if !admissible {
            return Err(fail());
        }
        (o_end, n_end) = (o_next, n_next);
    }
    let tail = original
        .get(o_end as usize..)
        .ok_or_else(|| check("original byte boundary"))?;
    if Some(tail) != normalized.get(n_end as usize..)
        || tail.chars().any(|ch| punct(ch) != ch)
        || removed.next().is_some()
    {
        return Err(check("edit replay or reminder inventory differs"));
    }
    Ok(())
}

/// The hull includes the whole original span of any rewrite intersected by
/// the token. A zero-width deletion contributes no bytes by itself.
pub(super) fn hull(normalized: [u32; 2], edits: &[Edit]) -> [u32; 2] {
    let [start, end] = normalized;
    let mut o = 0;
    let mut n = 0;
    let mut first = None;
    let mut last = 0;
    let mut include = |from, to| {
        first.get_or_insert(from);
        last = to;
    };
    for edit in edits {
        let ([o_start, o_end], [n_start, n_end]) = ranges(edit);
        let from = start.max(n);
        let to = end.min(n_start);
        if from < to {
            include(o + (from - n), o + (to - n));
        }
        if start < n_end && n_start < end && n_start < n_end {
            include(o_start, o_end);
        }
        (o, n) = (o_end, n_end);
    }
    let from = start.max(n);
    if from < end {
        include(o + (from - n), o + (end - n));
    }
    [first.unwrap_or(0), last]
}
