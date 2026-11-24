//! The decision protocol: what a player is asked and may answer.
use crate::engine::*;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AcceptedInput {
    pub decision_id: u64,
    pub player: PlayerId,
    pub answer: Answer,
}
