//! CR 400.7, 701.20a, 701.23b. Quality-constrained searches may find fewer
//! cards; the aggregate is public only after the private allocation is accepted.
use super::*;
use mtg_contracts::{ColoredLibrarySearch, ColoredSearchAssignment};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ColoredCardCandidate {
    pub handle: ObjectHandle,
    pub oracle_id: String,
    pub slots: Vec<Color>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ColoredCardAssignment {
    pub slot: Color,
    pub handle: ObjectHandle,
}

impl RulesTxn {
    /// Publicly known identities in hand. No targetable handle or hand position
    /// is disclosed. A subsequent zone change invalidates that incarnation.
    pub fn publicly_searched_hand(
        &self,
        viewer: PlayerId,
    ) -> Result<Vec<(PlayerId, String)>, String> {
        let k = self
            .state
            .knowledge
            .visibility
            .get(viewer.0 as usize)
            .ok_or("invalid viewer")?;
        let mut cards = vec![];
        for id in &k.revealed_hand {
            let Some(Zone::Hand { owner }) = self.zone(*id) else {
                return Err("stale revealed hand memory".into());
            };
            cards.push((owner, self.definition(id.card)?.oracle_id.clone()));
        }
        cards.sort();
        Ok(cards)
    }
}
