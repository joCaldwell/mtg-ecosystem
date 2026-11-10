//! Capture-authorized hypothetical roots. No real State, seed, hidden identity
//! or opponent Knowledge is accepted by the world builder. P7a is restricted to
//! the sealed P5 construction whitelist and fresh active precombat priority.
use super::*;
mod capture;
mod sample;
#[cfg(test)]
mod shared_history_tests;
#[cfg(test)]
mod tests;
mod world;
pub use capture::*;
pub use sample::*;
pub use world::*;

pub const SEARCH_RECONSTRUCTION: &str = "foundations-fresh-precombat-v1";
pub const SEARCH_INFORMATION: &str = "foundations-open-decklists-v1";
pub const SEARCH_SAMPLER: &str = "foundations-uniform-v2";
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RememberedCard {
    pub memory_ref: String,
    pub oracle_id: String,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "action", rename_all = "snake_case", deny_unknown_fields)]
pub enum OwnKnowledgeAction {
    OpeningRedraw,
    OpeningBottom { cards: Vec<RememberedCard> },
    Drawn { oracle_id: String },
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct KnownHandMemory {
    pub player: PlayerId,
    pub oracle_id: String,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PublicSearchProfile {
    pub reconstruction_schema: String,
    pub information_policy: String,
    pub engine_build_id: String,
    pub catalog_digest: String,
    pub rules_digest: String,
    pub commander_policy_digest: String,
    pub match_profile: String,
    pub decks: [Decklist; 4],
    pub starting_player: PlayerId,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CapturedSearchContext {
    pub observation: Observation,
    pub permitted_history: PermittedHistory,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SearchCaptureData {
    pub capture_version: u32,
    pub profile: PublicSearchProfile,
    pub context: CapturedSearchContext,
    pub own_knowledge: Vec<OwnKnowledgeAction>,
    pub known_bottom: Vec<RememberedCard>,
    pub known_hands: Vec<KnownHandMemory>,
}
// No Deserialize or public constructor: a DTO's own hash is not authority.
#[derive(Clone)]
pub struct SearchCapture {
    data: Arc<SearchCaptureData>,
    id: String,
    sampler_identity: String,
}
impl SearchCapture {
    pub fn data(&self) -> &SearchCaptureData {
        &self.data
    }
    pub fn context_id(&self) -> &str {
        &self.id
    }
    /// Permitted-information entropy identity excludes only executable identity.
    /// Proposals remain separately scoped to the complete current capture ID.
    pub fn sampler_identity(&self) -> &str {
        &self.sampler_identity
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SearchRejection {
    UnsupportedRoot(String),
    UnsupportedHistory(String),
    InvalidProposal(String),
    StaleWork,
    Invariant(String),
}
fn invalid(s: impl Into<String>) -> SearchRejection {
    SearchRejection::InvalidProposal(s.into())
}
fn invariant(s: impl Into<String>) -> SearchRejection {
    SearchRejection::Invariant(s.into())
}
fn unsupported(s: impl Into<String>) -> SearchRejection {
    SearchRejection::UnsupportedRoot(s.into())
}

impl Game {
    pub(super) fn remember_opening_redraw(&mut self, player: PlayerId) {
        self.state.knowledge.visibility[player.0 as usize]
            .own_knowledge
            .push(OwnKnowledgeAction::OpeningRedraw);
    }
    pub(super) fn remember_opening_bottom(
        &mut self,
        player: PlayerId,
        objects: &[ObjectId],
    ) -> Result<(), String> {
        let sequence = self.state.knowledge.visibility[player.0 as usize]
            .own_knowledge
            .len();
        let cards = objects
            .iter()
            .enumerate()
            .map(|(i, o)| {
                Ok(RememberedCard {
                    memory_ref: format!("memory-{sequence}-{i}"),
                    oracle_id: self
                        .state
                        .rules
                        .cards
                        .get(&o.card)
                        .ok_or("bottom memory source")?
                        .oracle_id
                        .clone(),
                })
            })
            .collect::<Result<_, String>>()?;
        self.state.knowledge.visibility[player.0 as usize]
            .own_knowledge
            .push(OwnKnowledgeAction::OpeningBottom { cards });
        Ok(())
    }
    pub(super) fn remember_draw(&mut self, owner: PlayerId, oracle_id: String) {
        self.state.knowledge.visibility[owner.0 as usize]
            .own_knowledge
            .push(OwnKnowledgeAction::Drawn { oracle_id });
    }
}

// Position proof uses completed owner actions, never current hidden library rows.
fn known_suffix(
    actions: &[OwnKnowledgeAction],
    current_library: usize,
) -> Result<Vec<RememberedCard>, SearchRejection> {
    let mut bottom: Vec<RememberedCard> = vec![];
    let mut draws: Vec<&str> = vec![];
    for action in actions {
        match action {
            OwnKnowledgeAction::OpeningRedraw => {
                if !draws.is_empty() {
                    return Err(SearchRejection::UnsupportedHistory(
                        "post-setup library permutation is outside this schema".into(),
                    ));
                }
                bottom.clear();
            }
            OwnKnowledgeAction::OpeningBottom { cards } => {
                if !draws.is_empty() || cards.len() > 7 {
                    return Err(SearchRejection::UnsupportedHistory(
                        "invalid opening action order".into(),
                    ));
                }
                bottom = cards.clone();
            }
            OwnKnowledgeAction::Drawn { oracle_id } => draws.push(oracle_id),
        }
    }
    let initial = current_library
        .checked_add(draws.len())
        .ok_or_else(|| invariant("draw count overflow"))?;
    if initial != 92 + bottom.len() {
        return Err(SearchRejection::UnsupportedHistory(
            "library count disagrees with completed own setup/draws".into(),
        ));
    }
    for (index, drawn) in draws.into_iter().enumerate() {
        let remaining = initial - index;
        if remaining <= bottom.len() && bottom[bottom.len() - remaining].oracle_id != drawn {
            return Err(SearchRejection::UnsupportedHistory(
                "known bottom draw disagrees with remembered order".into(),
            ));
        }
    }
    Ok(bottom[bottom.len() - bottom.len().min(current_library)..].to_vec())
}
