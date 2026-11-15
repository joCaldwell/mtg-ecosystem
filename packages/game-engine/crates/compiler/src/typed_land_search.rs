//! Consume the complete typed-search/move/shuffle sentence. Pinned CR 701.23b,
//! 701.24a/e, 305.6: https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;
use serde_json::{Value, json};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Find {
    #[serde(rename = "ref")]
    reference: String,
    filter: Value,
    count: Count,
    #[serde(rename = "upTo")]
    up_to: Option<bool>,
}
fn selector(filter: &Value) -> Option<Vec<BasicLandType>> {
    let types = [BasicLandType::Plains, BasicLandType::Island, BasicLandType::Swamp, BasicLandType::Mountain, BasicLandType::Forest];
    for t in types {
        if *filter == json!({"cls":"card","subtypes":[t.subtype()]}) {
            return Some(vec![t]);
        }
    }
    let leaves = filter.get("allOf")?.as_array()?;
    let [alternatives] = leaves.as_slice() else { return None };
    let alternatives = alternatives.get("anyOf")?.as_array()?;
    if *filter != json!({"cls":"card","allOf":[{"anyOf":alternatives}]}) || alternatives.is_empty() {
        return None;
    }
    let mut result = Vec::new();
    for leaf in alternatives {
        let t = types.into_iter().find(|t| *leaf == json!({"subtypes":[t.subtype()]}))?;
        if result.contains(&t) { return None; }
        result.push(t);
    }
    result.sort();
    Some(result)
}
pub(super) fn lower(steps: &[Step], source_block: u32) -> Result<Option<Instruction>, Failure> {
    let [Step::Search { who, zones, find }, Step::MoveZone { what, to, tapped }, Step::Shuffle { who: shuffler, library }] = steps else {
        return Ok(None);
    };
    // Basic/split search keeps its own complete-source recognizer unchanged.
    if find["filter"].get("supertypes").is_some() { return Ok(None); }
    let bad = || unsupported("effects.search", "unsupported complete typed land search/move/shuffle sequence");
    let original = find;
    let find: Find = serde_json::from_value(find.clone()).map_err(|_| bad())?;
    let any_basic_land_types = selector(&find.filter).ok_or_else(bad)?;
    let singular = find.count.value == 1;
    let mut expected = json!({"ref":"filter","filter":find.filter,"count":{"amount":"fixed","value":find.count.value}});
    if !singular { expected["upTo"] = json!(true); }
    let you = PlayerSelector { player: "you".into(), selection_id: None, of: None };
    let bound = if singular {
        *what == json!({"ref":"it"}) || *what == json!({"ref":"that","filter":{"cls":"card"}})
    } else {
        *what == json!({"ref":"them"})
    };
    if *original != expected || *who != you || *shuffler != you
        || *zones != json!({"selection":"all","of":[{"zone":"library","owner":"your"}]})
        || find.reference != "filter" || find.count.amount != "fixed" || find.count.value == 0
        || (singular && find.up_to.is_some()) || (!singular && find.up_to != Some(true))
        || !bound || *to != json!({"zone":"battlefield"}) || *tapped == Some(false)
        || *library != json!({"zone":"library","owner":{"player":"you"}})
    { return Err(bad()); }
    Ok(Some(Instruction::SearchTypedLands {
        search: TypedLandSearch { any_basic_land_types, maximum: Quantity::from(i64::from(find.count.value)), force_tapped: *tapped == Some(true) },
        source_block,
    }))
}

#[cfg(test)]
#[path = "typed_land_search_tests.rs"]
mod tests;
