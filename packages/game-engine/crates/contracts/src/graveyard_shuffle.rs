//! Announcement operands for a bounded group of cards in one targeted player's
//! graveyard. CR 115.3, 601.2c, 608.2b and 701.20.
use super::*;
use serde_json::{Value, json};

/// Scope and cardinality are bound independently of the selected runtime values.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GraveyardShuffle {
    pub maximum: u8,
    pub player_selection: String,
    pub cards_selection: String,
}

impl GraveyardShuffle {
    pub fn from_clause(text: &str, step: &Value) -> Option<Self> {
        let (word, noun) = text
            .strip_prefix("Target player shuffles up to ")?
            .split_once(' ')?;
        let maximum = [
            "one", "two", "three", "four", "five", "six", "seven", "eight", "nine", "ten",
        ]
        .iter()
        .position(|n| *n == word)? as u8
            + 1;
        if noun
            != if maximum == 1 {
                "target card from their graveyard into their library."
            } else {
                "target cards from their graveyard into their library."
            }
        {
            return None;
        }
        let player_selection = step["who"]["selectionId"].as_str()?;
        let cards_selection = step["what"][0]["selectionId"].as_str()?;
        if player_selection.is_empty()
            || cards_selection.is_empty()
            || player_selection == cards_selection
        {
            return None;
        }
        let expected = json!({"effect":"shuffle-into-library","who":{"player":"target-player","selectionId":player_selection},"what":[{"ref":"target","filter":{"cls":"card","zone":{"zone":"graveyard","owner":"their"}},"count":{"amount":"fixed","value": maximum},"upTo":true,"selectionId":cards_selection}],"library":{"zone":"library","owner":"their"}});
        (step == &expected).then(|| Self {
            maximum,
            player_selection: player_selection.to_owned(),
            cards_selection: cards_selection.to_owned(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn variable_graveyard_occurrence_binds_count_and_both_owners() {
        let source = "Target player shuffles up to two target cards from their graveyard into their library.";
        let step = json!({"effect":"shuffle-into-library","who":{"player":"target-player","selectionId":"chooser/seat"},"what":[{"ref":"target","filter":{"cls":"card","zone":{"zone":"graveyard","owner":"their"}},"count":{"amount":"fixed","value":2},"upTo":true,"selectionId":"chooser/cards"}],"library":{"zone":"library","owner":"their"}});
        assert_eq!(
            GraveyardShuffle::from_clause(source, &step)
                .unwrap()
                .maximum,
            2
        );
        for (pointer, value) in [
            ("/who/player", json!("you")),
            ("/what/0/upTo", json!(false)),
            ("/what/0/filter/zone/owner", json!("you")),
            ("/library/owner", json!("you")),
            ("/what/0/count/value", json!(3)),
            ("/what/0/selectionId", json!("chooser/seat")),
        ] {
            let mut altered = step.clone();
            *altered.pointer_mut(pointer).unwrap() = value;
            assert!(GraveyardShuffle::from_clause(source, &altered).is_none());
        }
    }
}
