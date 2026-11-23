//! Complete source contract for the optional same-turn battlefield entry copy.
use super::*;
use serde_json::{Value, json};

const COPY_TEXT: &str =
    "You may have this creature enter as a copy of any permanent that entered this turn.";
const TEXT: &str = concat!(
    "Flash\n",
    "Cascade (When you cast this spell, exile cards from the top of your library until you exile a nonland card that costs less. ",
    "You may cast it without paying its mana cost. Put the exiled cards on the bottom in a random order.)\n",
    "You may have this creature enter as a copy of any permanent that entered this turn."
);

// The full archives carry the same reviewed source, AST, and ordered graph at
// their original source index. The compiler also checks the exact archive row.

fn abilities() -> Value {
    json!([
        {"kind":"keywords","keywords":[{"keyword":"flash"}]},
        {"kind":"keywords","keywords":[{"keyword":"cascade"}]},
        {"kind":"static","effect":{
            "static":"entry-copy","what":{"ref":"this","noun":"creature"},
            "model":{"model":"choose","who":{"player":"you"},
                "filter":{"cls":"permanent","zoneChangeHistory":{
                    "event":"enters","to":{"zone":"battlefield"},"during":"this-turn"
                }},"count":1,"when":"before-entry"},
            "optional":{"who":{"player":"you"}}
        }}
    ])
}

fn characteristics() -> Characteristics {
    Characteristics {
        kind: CardKind::Creature,
        additional_types: vec![],
        subtypes: vec!["Shapeshifter".into()],
        basic: false,
        legendary: false,
        snow: false,
        hybrid_mana: vec![],
        mana_cost: ManaCost {
            generic: 4.into(),
            colored: vec![Color::U, Color::U],
        },
        colors: vec![Color::U],
        color_identity: vec![Color::U],
        power: Some(3.into()),
        toughness: Some(1.into()),
    }
}

fn permanent_spell() -> SpellProgram {
    SpellProgram {
        optional_additional_costs: vec![],
        additional_sacrifice: None,
        timing: SpellTiming::Instant,
        resolution: SpellResolution::Permanent,
        functioning_zone: FunctioningZone::Stack,
        targets: vec![],
        instructions: vec![],
        modes: vec![],
    }
}

fn labels() -> SourceLabels {
    SourceLabels {
        text: TEXT.into(),
        blocks: vec!["Flash".into(), "Cascade".into(), COPY_TEXT.into()],
        triggered: vec![1],
        statics: vec![2],
        ..SourceLabels::default()
    }
}
