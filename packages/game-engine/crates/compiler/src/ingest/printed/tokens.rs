use super::{Failure, NameClaims, invalid, text};
use mtg_oracle_ast::{
    printed::*,
    vocab::{CONTRACTIONS, PUNCT_CHARS},
};
use regex::Regex;
use std::sync::LazyLock;

static WORD: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^\p{L}+(?:['-]\p{L}+)*").unwrap());

#[derive(Debug, PartialEq, Eq)]
pub(super) enum Kind {
    Word {
        value: String,
        possessive: Option<&'static str>,
    },
    Number,
    Symbol,
    Selfref(bool),
    CardName,
    Punct,
}

pub(super) struct Token<'a> {
    pub kind: Kind,
    pub n: [u32; 2],
    pub grammar_index: usize,
    pub raw: &'a str,
}

impl Token<'_> {
    pub(super) fn word(&self) -> &str {
        match &self.kind {
            Kind::Word { value, .. } => value,
            _ => "",
        }
    }

    pub(super) fn possessive(&self) -> bool {
        matches!(
            self.kind,
            Kind::Word {
                possessive: Some(_),
                ..
            } | Kind::Selfref(true)
        )
    }

    pub(super) fn punct(&self) -> Option<&str> {
        (self.kind == Kind::Punct).then_some(self.raw)
    }
}

pub(super) struct Line<'a> {
    pub source_id: &'a str,
    pub tokens: Vec<Token<'a>>,
    failure: Option<u32>,
}

struct LexError<'a> {
    message: &'a str,
    at: u32,
}

enum Literal<'a> {
    Known,
    Absent(&'a str),
}

fn lex<'a>(
    line: &'a str,
    start: u32,
    literal: Literal<'_>,
    claims: &mut NameClaims,
) -> Result<Vec<Token<'a>>, LexError<'a>> {
    let mut tokens: Vec<Token<'a>> = Vec::new();
    let mut offset = 0;
    let mut quote = false;
    while offset < line.len() {
        let rest = &line[offset..];
        let ch = rest.chars().next().unwrap();
        let at = start + offset as u32;
        let err = |message| LexError { message, at };
        let previous = tokens.last();
        let previous_word = previous.filter(|t| !t.possessive()).map(Token::word);
        let partner = previous_word == Some("with")
            && tokens
                .iter()
                .rev()
                .nth(1)
                .is_some_and(|t| t.word() == "partner" && !t.possessive());
        let (kind, len) = match ch {
            ' ' | '\t' => {
                offset += 1;
                continue;
            }
            '{' => (
                Kind::Symbol,
                rest.find('}').ok_or_else(|| err("unclosed symbol brace"))? + 1,
            ),
            '~' => {
                let possessive = rest.starts_with("~'s");
                (Kind::Selfref(possessive), if possessive { 3 } else { 1 })
            }
            '"' if partner || matches!(previous_word, Some("named" | "create" | "creates")) => {
                let mut escaped = false;
                let end = rest[1..]
                    .char_indices()
                    .find_map(|(index, ch)| {
                        if escaped {
                            escaped = false;
                            return None;
                        }
                        if ch == '\\' {
                            escaped = true;
                            return None;
                        }
                        (ch == '"').then_some(index + 2)
                    })
                    .ok_or_else(|| err("unclosed literal card name"))?;
                let value: String = serde_json::from_str(&rest[..end])
                    .map_err(|_| err("invalid literal card name"))?;
                if value.trim().is_empty() {
                    return Err(err("empty literal card name"));
                }
                match literal {
                    Literal::Absent("literal name is absent from the source catalog") => {
                        claims.absent(value, partner);
                        return Err(err("literal name is absent from the source catalog"));
                    }
                    Literal::Known => claims.known(value, partner),
                    _ => {}
                }
                (Kind::CardName, end)
            }
            '0'..='9' => (
                Kind::Number,
                rest.bytes().take_while(u8::is_ascii_digit).count(),
            ),
            '∞' if tokens.is_empty() => (
                Kind::Word {
                    value: "∞".into(),
                    possessive: None,
                },
                ch.len_utf8(),
            ),
            _ if let Some(word) = WORD.find(rest) => {
                let mut len = word.end();
                let mut value = word.as_str().to_lowercase();
                let mut possessive = None;
                if !CONTRACTIONS.contains(&value.as_str()) {
                    if value.ends_with("'s") {
                        value.truncate(value.len() - 2);
                        possessive = Some("'s");
                    } else if rest[len..].starts_with('\'') && value.ends_with('s') {
                        len += 1;
                        possessive = Some("'");
                    }
                }
                (Kind::Word { value, possessive }, len)
            }
            '"' => {
                quote = !quote;
                (Kind::Punct, 1)
            }
            '\'' if quote => (Kind::Punct, 1),
            _ if PUNCT_CHARS.contains(&&*ch.encode_utf8(&mut [0; 4])) => {
                (Kind::Punct, ch.len_utf8())
            }
            _ => return Err(err("unexpected character")),
        };
        tokens.push(Token {
            kind,
            raw: &rest[..len],
            n: [at, at + len as u32],
            grammar_index: tokens.len(),
        });
        offset += len;
    }
    Ok(tokens)
}

// Mirror the lexer quote rule over physical tokens so a grammar permutation
// cannot move arbitrary words or punctuation.
fn settle(tokens: &mut [Token<'_>]) {
    let mut order: Vec<usize> = (0..tokens.len()).collect();
    let mut open = false;
    for index in 0..order.len() {
        if tokens[order[index]].punct() != Some("\"") {
            continue;
        }
        if open {
            let continues = order.get(index + 1).is_some_and(|&next| {
                let next = &tokens[next];
                match next.kind {
                    Kind::Word { .. } => {
                        next.raw
                            .chars()
                            .next()
                            .is_some_and(|ch| ch.to_lowercase().eq([ch]))
                            || next.word() == "then"
                    }
                    Kind::Punct => next.raw != "\"",
                    _ => false,
                }
            });
            let moves = |index: usize| match tokens[order[index]].punct() {
                Some(",") => true,
                Some(".") => !continues,
                _ => false,
            };
            if index >= 1 && moves(index - 1) {
                order.swap(index - 1, index);
            } else if index >= 2
                && tokens[order[index - 1]].punct() == Some("'")
                && moves(index - 2)
            {
                order[index - 2..=index].rotate_left(1);
            }
        }
        open = !open;
    }
    for (index, physical) in order.into_iter().enumerate() {
        tokens[physical].grammar_index = index;
    }
}

fn wire(token: &PrintedToken) -> (Kind, [u32; 2], [u32; 2], usize) {
    macro_rules! fields {
        ($item:ident, $kind:expr) => {
            ($kind, $item.n.0, $item.o.0, $item.grammar_index as usize)
        };
    }
    match token {
        PrintedToken::CardName(t) => fields!(t, Kind::CardName),
        PrintedToken::Number(t) => fields!(t, Kind::Number),
        PrintedToken::Punct(t) => fields!(t, Kind::Punct),
        PrintedToken::Selfref(t) => fields!(t, Kind::Selfref(t.possessive.is_some())),
        PrintedToken::Symbol(t) => fields!(t, Kind::Symbol),
        PrintedToken::Word(t) => fields!(
            t,
            Kind::Word {
                value: String::new(),
                possessive: t
                    .possessive
                    .as_ref()
                    .map(PrintedTokenWordPossessive::as_str),
            }
        ),
    }
}

pub(super) fn verify<'a>(
    original: &str,
    normalized: &'a str,
    printed: &'a [PrintedLine],
    edits: &[Edit],
    claims: &mut NameClaims,
) -> Result<Vec<Line<'a>>, Failure> {
    let physical: Vec<_> = if normalized.is_empty() {
        vec![]
    } else {
        normalized.split('\n').collect()
    };
    if printed.len() != physical.len() {
        return Err(invalid("P4", "printed lines do not tile normalized text"));
    }
    let mut start = 0u32;
    let mut verified = Vec::with_capacity(physical.len());
    for (index, (line, contents)) in printed.iter().zip(physical).enumerate() {
        let reject = |check, detail| invalid(check, format!("line {index}: {detail}"));
        let end = start + contents.len() as u32;
        let expected = [start, end];
        let line_id = format!("line:{index}");
        if contents.is_empty() || contents.bytes().any(|b| b == b'\t') {
            return Err(reject("P4", "invalid normalized line"));
        }
        match line {
            PrintedLine::Lexed(line) if line.n.0 == expected && line.source_id == line_id => {
                let mut tokens = lex(contents, start, Literal::Known, claims)
                    .map_err(|error| reject("P4", error.message))?;
                settle(&mut tokens);
                if tokens.len() != line.tokens.len() {
                    return Err(reject(
                        "P4",
                        "token inventory differs from normalized spelling",
                    ));
                }
                for (token, archived) in tokens.iter().zip(&line.tokens) {
                    let (kind, n, o, grammar_index) = wire(archived);
                    let matches_kind = match (&token.kind, &kind) {
                        (Kind::Word { possessive: a, .. }, Kind::Word { possessive: b, .. }) => {
                            a == b
                        }
                        (a, b) => a == b,
                    };
                    if !matches_kind
                        || token.n != n
                        || text::slice(original, o).is_none()
                        || text::hull(n, edits) != o
                    {
                        return Err(invalid(
                            "P4",
                            format!("line {index}, token {n:?}: invalid kind or byte span"),
                        ));
                    }
                    if token.grammar_index != grammar_index {
                        return Err(reject("P5", "incorrect quote punctuation ordering"));
                    }
                }
                verified.push(Line {
                    source_id: &line.source_id,
                    tokens,
                    failure: None,
                });
            }
            PrintedLine::Failed(line) if line.n.0 == expected && line.source_id == line_id => {
                let Err(error) = lex(contents, start, Literal::Absent(&line.error), claims) else {
                    return Err(reject("P4", "failed line can be lexed"));
                };
                let message = if error.message == "unexpected character" {
                    let ch = normalized[error.at as usize..].chars().next().unwrap();
                    format!(
                        "unexpected character {}",
                        serde_json::to_string(&ch).unwrap()
                    )
                } else {
                    error.message.to_owned()
                };
                if error.at != line.at || message != line.error {
                    return Err(reject("P4", "lexical failure location or reason disagrees"));
                }
                verified.push(Line {
                    source_id: &line.source_id,
                    tokens: vec![],
                    failure: Some(line.at),
                });
            }
            _ => return Err(reject("P4", "invalid physical line ID or normalized range")),
        }
        start = end + 1;
    }
    Ok(verified)
}

pub(super) fn verify_failure(
    block: &PrintedBlockFailed,
    lines: &[Line<'_>],
) -> Result<(), Failure> {
    use PrintedBlockFailedLocation::*;
    let [first, end] = block.lines.0.map(|v| v as usize);
    let section = lines
        .get(first..end)
        .ok_or_else(|| invalid("P4", "invalid failed block range"))?;
    let token = block
        .error
        .strip_suffix(')')
        .and_then(|s| s.rsplit_once("(token "))
        .and_then(|(_, index)| index.parse::<usize>().ok());
    let expected = if let Some(at) = section.iter().find_map(|line| line.failure) {
        (LexError, at)
    } else if let Some(at) = token.filter(|_| section.len() == 1).and_then(|index| {
        section[0]
            .tokens
            .iter()
            .find(|t| t.grammar_index == index)
            .map(|t| t.n[0])
    }) {
        (Token, at)
    } else {
        (End, block.n.0[1])
    };
    if (block.location.clone(), block.at) != expected {
        return Err(invalid("P4", "invalid failed block diagnostic location"));
    }
    Ok(())
}

pub(super) fn selection_matches(
    line: &Line<'_>,
    index: usize,
    selection: &str,
    suffix: Option<usize>,
) -> bool {
    let Some((id, grammar)) = selection.rsplit_once(':') else {
        return false;
    };
    let Ok(grammar) = grammar.parse::<usize>() else {
        return false;
    };
    if id != line.source_id {
        return false;
    }
    let Some(start) = suffix else {
        return line.tokens[index].grammar_index == grammar;
    };
    if start > index {
        return false;
    }
    let mut body: Vec<_> = line.tokens[start..]
        .iter()
        .map(|token| Token {
            kind: match &token.kind {
                Kind::Punct => Kind::Punct,
                Kind::Word { value, possessive } => Kind::Word {
                    value: value.clone(),
                    possessive: *possessive,
                },
                _ => Kind::Symbol,
            },
            n: token.n,
            grammar_index: 0,
            raw: token.raw,
        })
        .collect();
    settle(&mut body);
    body[index - start].grammar_index == grammar
}
