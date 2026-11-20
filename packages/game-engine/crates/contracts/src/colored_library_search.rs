//! CR 608.2d, 701.23b. A quality-constrained hidden search can find no card
//! for a slot. A multicolored card may fill exactly one matching slot.
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::Color;
use std::collections::BTreeSet;

/// Ordered, distinct colored slots of a single library search.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ColoredLibrarySearch {
    slots: Vec<Color>,
}

/// An assigned slot; unassigned slots represent cards the player did not find.
/// Callers supply their own incarnation or scoped handle type.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ColoredSearchAssignment<T> {
    pub slot: Color,
    pub card: T,
}

impl ColoredLibrarySearch {
    pub fn new(slots: &[Color]) -> Result<Self, &'static str> {
        let distinct: BTreeSet<_> = slots.iter().copied().collect();
        if slots.is_empty() || slots.contains(&Color::C) || distinct.len() != slots.len() {
            return Err("search requires distinct colored slots");
        }
        Ok(Self {
            slots: slots.to_vec(),
        })
    }

    pub fn slots(&self) -> &[Color] {
        &self.slots
    }

    /// Validate the submitted partial allocation against current candidates.
    /// The caller verifies library membership and current card color together
    /// in `eligible`. The returned aggregate follows the printed slot order
    /// regardless of the order of the submitted assignments.
    pub fn found<'a, T: Ord>(
        &self,
        assignments: &'a [ColoredSearchAssignment<T>],
        mut eligible: impl FnMut(&T, Color) -> bool,
    ) -> Result<Vec<&'a T>, &'static str> {
        if assignments.len() > self.slots.len() {
            return Err("too many colored search assignments");
        }
        let mut occupied = vec![None; self.slots.len()];
        let mut cards = BTreeSet::new();
        for assignment in assignments {
            let index = self
                .slots
                .iter()
                .position(|color| *color == assignment.slot)
                .ok_or("foreign colored search slot")?;
            if occupied[index].is_some() || !cards.insert(&assignment.card) {
                return Err("duplicate colored search slot or card");
            }
            if !eligible(&assignment.card, assignment.slot) {
                return Err("ineligible colored search card");
            }
            occupied[index] = Some(&assignment.card);
        }
        Ok(occupied.into_iter().flatten().collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn colored_search_preserves_partial_distinct_assignments() {
        let search = ColoredLibrarySearch::new(&[Color::R, Color::G, Color::W]).unwrap();
        let eligible = |id: &u8, slot| match *id {
            1 => matches!(slot, Color::R | Color::G),
            2 => slot == Color::R,
            _ => false,
        };
        let assign = |slot, card| ColoredSearchAssignment { slot, card };
        let selected = [assign(Color::G, 1), assign(Color::R, 2)];
        assert_eq!(search.found(&selected, eligible).unwrap(), [&2, &1]);
        assert!(search.found(&[], eligible).unwrap().is_empty());
        for bad in [
            vec![assign(Color::R, 1), assign(Color::G, 1)],
            vec![assign(Color::R, 1), assign(Color::R, 2)],
            vec![assign(Color::W, 1)],
            vec![assign(Color::U, 1)],
            vec![assign(Color::G, 3)],
        ] {
            assert!(search.found(&bad, eligible).is_err());
        }
        assert!(ColoredLibrarySearch::new(&[Color::C]).is_err());
        assert!(ColoredLibrarySearch::new(&[Color::R, Color::R]).is_err());
    }
}

use super::{
    CardKind, Definition, FunctioningZone, Instruction, SpellProgram, SpellResolution, SpellTiming,
    StaticEffect, strict_read,
};
use serde_json::{Value, json};

pub fn colored_library_source(text: &str) -> Option<(Vec<Color>, Value)> {
    let list = text
        .strip_prefix("Search your library for ")?
        .strip_suffix(". Reveal those cards, put them into your hand, then shuffle.")?;
    let (first, last) = list.rsplit_once(" and ")?;
    let preceding = first.strip_suffix(',').unwrap_or(first);
    let mut names = vec![];
    let mut colors = vec![];
    for item in preceding.split(", ").chain(std::iter::once(last)) {
        let name = item.strip_prefix("a ")?.strip_suffix(" card")?;
        colors.push(match name {
            "white" => Color::W,
            "blue" => Color::U,
            "black" => Color::B,
            "red" => Color::R,
            "green" => Color::G,
            _ => return None,
        });
        names.push(name);
    }
    ColoredLibrarySearch::new(&colors).ok()?;
    let cards: Vec<_> = names.iter().map(|n| format!("a {n} card")).collect();
    let (last, preceding) = cards.split_last()?;
    let canonical = if preceding.len() == 1 {
        format!("{} and {last}", preceding[0])
    } else {
        format!("{}, and {last}", preceding.join(", "))
    };
    if list != canonical {
        return None;
    }
    let members: Vec<_> = names
        .iter()
        .map(|n| json!({"filter":{"colors":[n],"cls":"card"},"count":{"amount":"fixed","value":1}}))
        .collect();
    let you = json!({"player":"you"});
    let ast = json!([{"kind":"spell","effects":[{"steps":[{"effect":"search","who":you,"zones":{"selection":"all","of":[{"zone":"library","owner":"your"}]},"for":{"ref":"card-extraction-group","members":members,"combine":"all","allocation":"distinct-objects"}}]},{"steps":[{"effect":"reveal","who":you,"what":{"ref":"that","filter":{"cls":"card"},"plural":true}},{"effect":"move-zone","what":{"ref":"them"},"to":{"zone":"hand","owner":"your"}},{"effect":"shuffle","who":you,"library":{"zone":"library","owner":you}}]}]}]);
    Some((colors, ast))
}

pub fn colored_library_program(colors: Vec<Color>) -> SpellProgram {
    SpellProgram {
        functioning_zone: FunctioningZone::Stack,
        timing: SpellTiming::MainPhase,
        resolution: SpellResolution::Instructions,
        targets: vec![],
        instructions: vec![Instruction::SearchColoredLibrary {
            colors,
            source_block: 0,
        }],
        modes: vec![],
        additional_sacrifice: None,
        optional_additional_costs: vec![],
    }
}

fn reserved(p: &SpellProgram) -> bool {
    p.instructions
        .iter()
        .any(|i| matches!(i, Instruction::SearchColoredLibrary { .. }))
        || p.modes.iter().any(|m| reserved(&m.program))
}

pub(super) fn validate_definition(d: &Definition) -> Result<(), String> {
    let raw = &d.provenance.source_json;
    if d.activated.iter().any(|a|reserved(&a.program))||d.triggered.iter().any(|a|reserved(&a.program))||d.static_abilities.iter().any(|a|matches!(&a.effect,StaticEffect::GrantSpellCascade{program,..} if reserved(program))){return Err("colored search requires an ordinary spell".into())}
    let has = d.spell.as_ref().is_some_and(reserved);
    if !has && !raw.contains("Reveal those cards") && !raw.contains(r"\u") {
        return Ok(());
    }
    let source: Value = strict_read(raw)?;
    let text = source["oracle_text"].as_str().unwrap_or_default();
    let Some((colors, ast)) = colored_library_source(text) else {
        return if has {
            Err("missing colored library search source".into())
        } else {
            Ok(())
        };
    };
    let c = &d.characteristics;
    let mut mana = String::new();
    if c.mana_cost.generic != super::Quantity::zero() || c.mana_cost.colored.is_empty() {
        mana.push_str(&format!("{{{}}}", c.mana_cost.generic));
    }
    for color in &c.mana_cost.colored {
        mana.push_str(&format!("{{{color:?}}}"));
    }
    if d.spell.as_ref() != Some(&colored_library_program(colors))
        || strict_read::<Value>(&d.provenance.abilities_json)? != ast
        || source["name"] != d.name
        || source["id"] != d.provenance.printing_id
        || source["oracle_id"] != d.oracle_id
        || source["layout"] != "normal"
        || source["type_line"] != "Sorcery"
        || source["mana_cost"] != mana
        || source["colors"] != json!(c.colors)
        || source["color_identity"] != json!(c.color_identity)
        || ["card_faces", "power", "toughness", "loyalty", "defense"]
            .iter()
            .any(|k| !source[*k].is_null())
        || c.kind != CardKind::Sorcery
        || c.basic
        || c.snow
        || c.legendary
        || !c.additional_types.is_empty()
        || !c.subtypes.is_empty()
        || d.x_mana_cost
        || !d.activated.is_empty()
        || !d.triggered.is_empty()
        || !d.static_abilities.is_empty()
        || !d.keywords.is_empty()
        || !d.mana_abilities.is_empty()
        || d.source_labels.as_ref().is_some_and(|l| {
            l.text != text
                || !matches!(l.blocks.as_slice(),[b] if b==text)
                || !l.statics.is_empty()
                || !l.triggered.is_empty()
                || !l.activated.is_empty()
                || !l.mana.is_empty()
                || !l.spell_modes.is_empty()
        })
    {
        return Err("incomplete colored library search program or source".into());
    }
    Ok(())
}

pub(super) fn extend_schema(schema: &mut Value) {
    let free = json!({"$ref":"#/$defs/ColoredSearchFreeProgram"});
    schema["$defs"]["ColoredSearchFreeProgram"] = json!({"properties":{"instructions":{"items":{"not":{"required":["op"],"properties":{"op":{"const":"search_colored_library"}}}}},"modes":{"items":{"properties":{"program":free}}}}});
    let capability = super::Capability::ColoredLibrarySearchV1;
    schema["$defs"]["Definition"]["allOf"].as_array_mut().unwrap().push(json!({
      "properties":{"activated":{"items":{"properties":{"program":free}}},"triggered":{"items":{"properties":{"program":free}}},
       "static_abilities":{"items":{"properties":{"effect":{"properties":{"grant_spell_cascade":{"properties":{"program":free}}}}}}}},
      "if":{"anyOf":[{"properties":{"spell":{"type":"object","not":free}}},
          {"properties":{"provenance":{"properties":{"source_json":{"pattern":r#"Search your library for a (?:white|blue|black|red|green) card[^"]+\. Reveal those cards, put them into your hand, then shuffle\."#}}}}}]},
      "then":{"properties":{"spell":{"type":"object","properties":{"instructions":{"minItems":1,"maxItems":1,"items":{"required":["op","colors","source_block"],"properties":{"op":{"const":"search_colored_library"},"colors":{"uniqueItems":true,"minItems":2,"maxItems":5,"items":{"enum":["W","U","B","R","G"]}},"source_block":{"const":0}}}},
          "targets":{"maxItems":0},"modes":{"maxItems":0},"additional_sacrifice":{"type":"null"},"optional_additional_costs":{"maxItems":0},"timing":{"const":"main_phase"},"functioning_zone":{"const":"stack"},"resolution":{"const":"instructions"}}},
          "characteristics":{"properties":{"kind":{"const":"sorcery"},"basic":{"const":false},"legendary":{"const":false},"snow":{"const":false},"subtypes":{"maxItems":0},"additional_types":{"maxItems":0}}},
          "activated":{"maxItems":0},"triggered":{"maxItems":0},"static_abilities":{"maxItems":0},"mana_abilities":{"maxItems":0},"keywords":{"maxItems":0},"x_mana_cost":{"const":false},
          "capabilities":{"uniqueItems":true,"minItems":3,"maxItems":3,"items":{"enum":[super::Capability::NormalLayoutV1,super::Capability::CastSorceryV1,capability]}}}},
      "else":{"properties":{"capabilities":{"not":{"contains":{"const":capability}}}}}
    }));
}
