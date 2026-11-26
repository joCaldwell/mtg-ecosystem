use crate::engine::*;
/// A permitted observation cannot supply a trusted rules decision.
///
/// ```compile_fail
/// use mtg_core::{Decision, Rules, Viewer};
/// fn trusted(input: Decision<Rules>) {}
/// fn submit(observed: Decision<Viewer>) { trusted(observed); }
/// ```
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Rules {}
impl Space for Rules {
    type Id = u64;
    type Object = ObjectId;
    type Stack = StackId;
    type Trigger = TriggerId;
    type Damage = DamageId;
    type Effect = EffectKey;
    type Entity = TargetEntity;
    type Commander = CardId;
    type PreviousCommanderChoice = (ObjectId, bool);
}
