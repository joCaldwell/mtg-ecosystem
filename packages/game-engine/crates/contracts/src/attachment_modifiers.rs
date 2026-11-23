//! Signed stat and fixed-color protection grants from continuous attachments.
//! CR 301.5f, 303.4m, 611.3a, 613.1f, 613.4c, 702.11b and 702.16.
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;
use serde_json::{Value, json};

pub fn advanced(effect: &StaticEffect) -> bool {
    matches!(effect, StaticEffect::ModifyCreatures { scope: CreatureScope::Equipped | CreatureScope::Enchanted, power, toughness, keywords, .. }
        if power.is_negative() || toughness.is_negative() || keywords.iter().any(|k| k.fixed_protection_color().is_some()))
}

pub fn grants_protection(effect: &StaticEffect) -> bool {
    matches!(effect, StaticEffect::ModifyCreatures { scope: CreatureScope::Equipped | CreatureScope::Enchanted, keywords, .. }
        if keywords.iter().any(|k| k.fixed_protection_color().is_some()))
}

fn signed(text: &str) -> Option<(Quantity, Value)> {
    let (sign, n) = if let Some(n) = text.strip_prefix('+') {
        (1, n)
    } else {
        (-1, text.strip_prefix('-')?)
    };
    let n = n
        .parse::<u32>()
        .ok()
        .filter(|n| n.to_string() == text[1..])?;
    Some((
        (i64::from(n) * sign).into(),
        json!({"sign":sign,"amount":{"amount":"fixed","value":n}}),
    ))
}

const LEGENDARY_GRANT: &str = "As long as equipped creature is legendary, it has hexproof.";

fn legendary_grant() -> StaticAbility {
    StaticAbility {
        functioning_zone: FunctioningZone::Battlefield,
        effect: StaticEffect::ModifyCreatures {
            scope: CreatureScope::EquippedLegendary,
            power: Quantity::zero(),
            toughness: Quantity::zero(),
            keywords: vec![KeywordAbility::Hexproof],
            restrictions: vec![],
        },
    }
}

pub(super) fn scope_valid(effect: &StaticEffect, equipment: bool) -> bool {
    !matches!(
        effect,
        StaticEffect::ModifyCreatures {
            scope: CreatureScope::EquippedLegendary,
            ..
        }
    ) || (equipment && *effect == legendary_grant().effect)
}

fn printed_conditional(line: &str) -> Option<(StaticAbility, Value)> {
    if line != LEGENDARY_GRANT
        && line.strip_suffix(
            " (It can't be the target of spells or abilities your opponents control.)",
        ) != Some(LEGENDARY_GRANT)
    {
        return None;
    }
    Some((
        legendary_grant(),
        json!({"kind":"static","effect":{"static":"conditional",
            "condition":{"condition":"is-filter","what":{"ref":"equipped","noun":"creature"},
                "filter":{"supertypes":["legendary"]}},
            "effect":{"static":"modify","what":{"ref":"it"},"modifiers":[
                {"modifier":"abilities","abilities":[{"gained":"keyword","keyword":{"keyword":"hexproof"}}]}
            ]}
        }}),
    ))
}

pub fn printed(line: &str) -> Option<(StaticAbility, Value)> {
    if let Some(grant) = printed_conditional(line) {
        return Some(grant);
    }
    let (scope, reference, tail) = if let Some(tail) = line.strip_prefix("Equipped creature ") {
        (CreatureScope::Equipped, "equipped", tail)
    } else {
        (
            CreatureScope::Enchanted,
            "enchanted",
            line.strip_prefix("Enchanted creature ")?,
        )
    };
    let tail = tail.strip_suffix('.')?;
    let (stats, protection) = if let Some(rest) = tail.strip_prefix("gets ") {
        let (stats, grant) = rest
            .split_once(" and has ")
            .map_or((rest, None), |(s, g)| (s, Some(g)));
        (Some(stats), grant)
    } else {
        (None, Some(tail.strip_prefix("has ")?))
    };
    let mut modifiers = Vec::new();
    let (power, toughness) = if let Some(pt) = stats {
        let (p, t) = pt.split_once('/')?;
        let (power, pa) = signed(p)?;
        let (toughness, ta) = signed(t)?;
        modifiers.push(json!({"modifier":"pt","power":pa,"toughness":ta}));
        (power, toughness)
    } else {
        (Quantity::zero(), Quantity::zero())
    };
    let mut keywords = Vec::new();
    if let Some(grant) = protection {
        let mut from = Vec::new();
        for color in grant.strip_prefix("protection from ")?.split(" and from ") {
            let keyword = fixed_color_protection::keyword_for_color(color)?;
            if keywords.contains(&keyword) {
                return None;
            }
            from.push(json!({"scope":"color","color":color}));
            keywords.push(keyword);
        }
        modifiers.push(json!({"modifier":"abilities","abilities":[{"gained":"keyword","keyword":{"keyword":"protection","from":from}}]}));
    }
    let effect = StaticEffect::ModifyCreatures {
        scope,
        power,
        toughness,
        keywords,
        restrictions: vec![],
    };
    if !advanced(&effect) {
        return None;
    }
    Some((
        StaticAbility {
            functioning_zone: FunctioningZone::Battlefield,
            effect,
        },
        json!({"kind":"static","effect":{"static":"modify","what":{"ref":reference,"noun":"creature"},"modifiers":modifiers}}),
    ))
}
