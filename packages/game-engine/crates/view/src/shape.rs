//! The public legality contract. Every index refers to this decision's ordered
//! candidate table, except for slot and bucket references within joint rules.
use mtg_contracts::{Color, ManaCost, Quantity};
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

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ManaPool {
    pub unrestricted: BTreeMap<Color, Quantity>,
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
}
