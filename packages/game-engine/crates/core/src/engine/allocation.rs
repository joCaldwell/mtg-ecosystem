//! CR 615.7 finite prevention allocation across simultaneous source portions.
//! Selection chooses a shield/group; the anchor does not reserve its source.
use super::*;
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DamageAllocationCandidate {
    pub damage: DamageId,
    pub amount: Quantity,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct VisibleDamageAllocationCandidate {
    pub damage: DamageHandle,
    pub source: VisibleTarget,
    pub oracle_id: String,
    pub amount: Quantity,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DamageAllocationRecord {
    pub effect: EffectKey,
    pub allocations: Vec<(DamageId, Quantity)>,
    pub application_start: u32,
    pub application_count: u32,
    pub budget_before: Quantity,
    pub budget_after: Quantity,
}
impl RulesTxn {}
impl RulesTxn {
    pub(super) fn damage_handle(
        &self,
        viewer: PlayerId,
        transaction: DamageTransactionId,
        damage: DamageId,
    ) -> Result<DamageHandle, String> {
        self.state
            .knowledge
            .visibility
            .get(viewer.0 as usize)
            .and_then(|k| {
                k.damage_handles
                    .iter()
                    .find(|(id, _)| *id == (transaction, damage))
                    .map(|(_, h)| h.clone())
            })
            .ok_or_else(|| "unknown damage handle".into())
    }
    pub(crate) fn resolve_damage_handle(
        &self,
        viewer: PlayerId,
        handle: &DamageHandle,
    ) -> Result<DamageId, String> {
        let id = self.damage_transaction()?.id;
        self.state
            .knowledge
            .visibility
            .get(viewer.0 as usize)
            .and_then(|k| {
                k.damage_handles
                    .iter()
                    .find(|((tx, _), h)| *tx == id && h == handle)
                    .map(|((_, d), _)| *d)
            })
            .ok_or_else(|| "foreign/stale damage handle".into())
    }
}
impl RulesTxn {}
