//! The public legality contract. Every index refers to this decision's ordered
//! candidate table, except for slot and bucket references within joint rules.
use mtg_contracts::{Color, ManaCost, Quantity, RestrictedMana};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "shape", rename_all = "snake_case", deny_unknown_fields)]
pub enum Shape {
    Pick {
        slots: Vec<Slot>,
        rules: Vec<JointRule>,
    },
    Order {
        items: Vec<u32>,
        min: u32,
        max: u32,
    },
    Distribute {
        total: Quantity,
        buckets: Vec<Bucket>,
        gates: Vec<Gate>,
    },
    CombatDistribute {
        groups: Vec<CombatDistribution>,
        lethal: Vec<CombatLethal>,
    },
    Number {
        min: Quantity,
        max: Option<Quantity>,
    },
    Pay {
        cost: ManaCost,
        pool: ManaPool,
        step: PayStep,
    },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Slot {
    pub role: SlotRole,
    pub subject: Option<u32>,
    pub allowed: Vec<u32>,
    pub min: u32,
    pub max: u32,
    pub repeat: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "role", rename_all = "snake_case", deny_unknown_fields)]
pub enum SlotRole {
    Choice,
    Target { spec: u32 },
    AttackWith,
    BlockWith,
    HybridSymbol { index: u32 },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "rule", rename_all = "snake_case", deny_unknown_fields)]
pub enum JointRule {
    Distinct {
        slots: Vec<u32>,
    },
    /// Each named candidate must be unpicked or picked at least this many times.
    PickedAtLeast {
        candidates: Vec<u32>,
        count: u32,
    },
    /// Each named candidate must be picked at most this many times.
    /// Part of the observation and saved-work decision formats.
    PickedAtMost {
        candidates: Vec<u32>,
        count: u32,
    },
    Total {
        min: u32,
        max: u32,
    },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Bucket {
    pub candidate: u32,
    pub min: Quantity,
    pub max: Quantity,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Gate {
    /// Bucket ordinal whose positive allocation depends on the requirements.
    pub bucket: u32,
    pub requires: Vec<(u32, Quantity)>,
}

/// All buckets of a source are contiguous in the answer, in group order.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CombatDistribution {
    pub source: u32,
    pub total: Quantity,
    pub recipients: Vec<u32>,
}

/// A positive player allocation requires lethal damage on this blocker
/// from the combined assignments of all attacking creatures.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CombatLethal {
    pub player_bucket: u32,
    pub damage_buckets: Vec<u32>,
    pub deathtouch_buckets: Vec<u32>,
    pub remaining: Quantity,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ManaPool {
    pub unrestricted: BTreeMap<Color, Quantity>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub restricted: Vec<RestrictedMana>,
}

impl ManaPool {
    pub fn available(&self) -> BTreeMap<Color, Quantity> {
        let mut total = self.unrestricted.clone();
        for unit in &self.restricted {
            total.entry(unit.color).or_default().add(&unit.amount);
        }
        total
    }

    /// Deterministic legal allocation for policies that do not choose buckets.
    pub fn spend(&self, cost: &ManaCost) -> Option<PayAnswer> {
        let mut remaining = mtg_contracts::mana_spend::spend(cost, &self.available())?;
        let mut restricted = vec![];
        for bucket in &self.restricted {
            let Some(needed) = remaining.get_mut(&bucket.color) else {
                continue;
            };
            let amount = bucket.amount.clone().min(needed.clone());
            if amount > Quantity::zero() {
                needed.subtract(&amount);
                restricted.push(RestrictedMana {
                    amount,
                    ..bucket.clone()
                });
            }
        }
        remaining.retain(|_, amount| *amount > Quantity::zero());
        Some(if restricted.is_empty() {
            PayAnswer::Spend {
                unrestricted: remaining,
            }
        } else {
            PayAnswer::SpendRestricted {
                unrestricted: remaining,
                restricted,
            }
        })
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "step", rename_all = "snake_case", deny_unknown_fields)]
pub enum PayStep {
    Produce { sources: Vec<u32> },
    Spend,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "answer", rename_all = "snake_case", deny_unknown_fields)]
pub enum Answer {
    Pick { picks: Vec<Vec<u32>> },
    Order { order: Vec<u32> },
    Distribute { amounts: Vec<Quantity> },
    Number { value: Quantity },
    Pay { pay: PayAnswer },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "step", rename_all = "snake_case", deny_unknown_fields)]
pub enum PayAnswer {
    Activate {
        source: u32,
    },
    Finish {
        undo_on_failure: bool,
    },
    Spend {
        unrestricted: BTreeMap<Color, Quantity>,
    },
    SpendRestricted {
        unrestricted: BTreeMap<Color, Quantity>,
        restricted: Vec<RestrictedMana>,
    },
}

impl PayAnswer {
    pub fn spending(&self) -> Option<(&BTreeMap<Color, Quantity>, &[RestrictedMana])> {
        match self {
            Self::Spend { unrestricted } => Some((unrestricted, &[])),
            Self::SpendRestricted {
                unrestricted,
                restricted,
            } if !restricted.is_empty() => Some((unrestricted, restricted)),
            _ => None,
        }
    }
}
