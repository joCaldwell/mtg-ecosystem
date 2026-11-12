//! CR 205.1, 205.2, 205.3, 205.4: type-line components are independent.
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;
pub(super) struct ParsedTypes {
    pub kind: CardKind,
    pub additional_types: Vec<CardKind>,
    pub basic: bool,
    pub legendary: bool,
    pub snow: bool,
    pub subtypes: Vec<String>,
}
pub(super) fn parse(line: &str) -> Result<ParsedTypes, Failure> {
    let (head, tail) = line.split_once(" — ").unwrap_or((line, ""));
    let mut types = BTreeSet::new();
    let mut supertypes = BTreeSet::new();
    for word in head.split_whitespace() {
        let kind = match word {
            "Basic" | "Legendary" | "Snow" => {
                if !supertypes.insert(word) {
                    return Err(invalid("source.type_line", "duplicate supertype"));
                }
                continue;
            }
            "World" => return Err(unsupported("source.type_line.supertype", word)),
            "Artifact" => CardKind::Artifact,
            "Creature" => CardKind::Creature,
            "Enchantment" => CardKind::Enchantment,
            "Land" => CardKind::Land,
            "Instant" => CardKind::Instant,
            "Sorcery" => CardKind::Sorcery,
            _ => return Err(unsupported("source.type_line.card_type", word)),
        };
        if !types.insert(kind) {
            return Err(invalid("source.type_line", "duplicate card type"));
        }
    }
    let kind = [
        CardKind::Creature,
        CardKind::Land,
        CardKind::Artifact,
        CardKind::Enchantment,
        CardKind::Instant,
        CardKind::Sorcery,
    ]
    .into_iter()
    .find(|k| types.contains(k))
    .ok_or_else(|| invalid("source.type_line", "missing card type"))?;
    if (types.len() > 1
        && (types.contains(&CardKind::Instant) || types.contains(&CardKind::Sorcery)))
        || (types.contains(&CardKind::Creature) && types.contains(&CardKind::Land))
    {
        return Err(unsupported(
            "source.type_line",
            "unsupported type combination",
        ));
    }
    if supertypes.contains("Snow") && kind != CardKind::Creature {
        return Err(unsupported(
            "source.type_line.supertype",
            "snow support requires a non-mana creature",
        ));
    }
    if supertypes.contains("Basic") && kind != CardKind::Land {
        return Err(invalid("source.type_line", "basic requires land"));
    }
    for subtype in tail.split_whitespace() {
        if unsupported_rules_subtype(subtype) {
            return Err(unsupported(
                "source.type_line.subtypes",
                format!("subtype:{}", subtype.to_lowercase()),
            ));
        }
    }
    types.remove(&kind);
    Ok(ParsedTypes {
        kind,
        additional_types: types.into_iter().collect(),
        basic: supertypes.contains("Basic"),
        legendary: supertypes.contains("Legendary"),
        snow: supertypes.contains("Snow"),
        subtypes: tail.split_whitespace().map(str::to_owned).collect(),
    })
}
