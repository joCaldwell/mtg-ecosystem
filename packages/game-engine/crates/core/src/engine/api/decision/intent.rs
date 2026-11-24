use crate::engine::*;
impl mtg_view::TestRulesSpace for Rules {
    fn card_id(object: &ObjectId) -> CardId {
        object.card
    }
    fn target_candidate(entity: TargetEntity) -> Candidate<Self> {
        match entity {
            TargetEntity::Object { object } => Candidate::Object { object },
            TargetEntity::Player { player } => Candidate::Player { player },
        }
    }
}
