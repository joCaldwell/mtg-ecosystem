//! Versioned executable contracts. No rules execution or parser dependency.
use num_bigint::BigInt;
use num_traits::Zero;
use schemars::{JsonSchema, Schema, SchemaGenerator, json_schema};
use serde::{Deserialize, Serialize, de};
use sha2::{Digest, Sha256};
use std::{collections::BTreeSet, fmt, str::FromStr};

pub const RULES_EDITION: &str = "2026-09-25";
pub const RULES_SHA256: &str = "8d860e451f20f38865b725b42d82feb714c725373dd8f3b32b8652b3eeb070ca";
pub const PARSER_SHA256: &str = "98980a967a70d57008847ce0952edf0cc277af9ad26b8cb8739a555d97082b8d";
pub const ARCHIVE_MANIFEST_SHA256: &str =
    "b20406acec7b9af4610324af8848c76a2966315f1bb4043de2b03d53aa1a6f1b";
pub const SCENARIO_PROFILE: &str = "land-return-departure-scenario-v2";

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Default)]
pub struct Quantity(BigInt);
impl Quantity {
    pub fn zero() -> Self {
        Self(BigInt::zero())
    }
    pub fn is_negative(&self) -> bool {
        self.0 < BigInt::zero()
    }
    pub fn add(&mut self, other: &Self) {
        self.0 += &other.0;
    }
    pub fn subtract(&mut self, other: &Self) {
        self.0 -= &other.0;
    }
}
impl From<i64> for Quantity {
    fn from(value: i64) -> Self {
        Self(BigInt::from(value))
    }
}
impl FromStr for Quantity {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, String> {
        let value = BigInt::from_str(s).map_err(|_| "invalid exact quantity".to_string())?;
        if value.to_string() != s {
            return Err("quantity must use canonical decimal spelling".into());
        }
        Ok(Self(value))
    }
}
impl fmt::Display for Quantity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}
impl Serialize for Quantity {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&self.to_string())
    }
}
impl<'de> Deserialize<'de> for Quantity {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        String::deserialize(d)?.parse().map_err(de::Error::custom)
    }
}
impl JsonSchema for Quantity {
    fn schema_name() -> std::borrow::Cow<'static, str> {
        "ExactQuantity".into()
    }
    fn json_schema(_: &mut SchemaGenerator) -> Schema {
        json_schema!({"type":"string","pattern":"^(0|[1-9][0-9]*|-[1-9][0-9]*)$"})
    }
}

#[derive(
    Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
pub enum Color {
    W,
    U,
    B,
    R,
    G,
    C,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ManaCost {
    pub generic: Quantity,
    pub colored: Vec<Color>,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum CardKind {
    Land,
    Creature,
    Instant,
    Sorcery,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Characteristics {
    pub kind: CardKind,
    pub subtypes: Vec<String>,
    pub basic: bool,
    pub legendary: bool,
    pub mana_cost: ManaCost,
    pub colors: Vec<Color>,
    pub color_identity: Vec<Color>,
    pub power: Option<Quantity>,
    pub toughness: Option<Quantity>,
}
#[derive(
    Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum Capability {
    NormalLayoutV1,
    BasicLandManaV1,
    LandPlayV1,
    BaseCreatureV1,
    CommanderIdentityV1,
    CastInstantV1,
    CastCreatureV1,
    CreatureTargetV1,
    OwnerHandMoveV1,
    CastSorceryV1,
    OwnerLibraryTopMoveV1,
    ActivatedAbilityV1,
    SacrificeSourceCostV1,
    ControllerDrawV1,
    ControllerDiscardV1,
    OpponentTargetV1,
    TargetPlayerSacrificeCreatureV1,
    StackSpellTargetV1,
    StackAbilityTargetV1,
    CounterStackObjectV1,
    TapSourceCostV1,
    ManaActivationCostV1,
    AnyDamageTargetV1,
    AnyPlayerTargetV1,
    NoncombatDamageV1,
    LifeGainV1,
    LifeLossV1,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum FunctioningZone {
    Battlefield,
    Stack,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Chooser {
    Controller,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum EvaluationStage {
    Announcement,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum TargetDomain {
    BattlefieldCreature,
    OpponentPlayer,
    StackSpell,
    StackActivatedOrTriggeredAbility,
    /// CR 115.4: creatures, players, planeswalkers and battles. The closed
    /// scenario admits only creature/player inhabitants, not a narrower domain.
    AnyDamageTarget,
    AnyPlayer,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct TargetSpec {
    pub slot: u32,
    pub selection_id: String,
    pub chooser: Chooser,
    pub stage: EvaluationStage,
    pub domain: TargetDomain,
}
/// Controller means the frozen invocation controller (CR 109.5), not the
/// current controller of its physical source. Target refers to AnyPlayer.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum LifeRecipient {
    Controller,
    Target { target_slot: u32 },
}
// Tagged operations are deliberate; unknown variants and fields reject.
// https://serde.rs/enum-representations.html
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "op", rename_all = "snake_case", deny_unknown_fields)]
pub enum Instruction {
    ReturnToOwnerHand {
        target_slot: u32,
        source_block: u32,
    },
    PutOnTopOwnerLibrary {
        target_slot: u32,
        source_block: u32,
    },
    TargetPlayerSacrificeCreature {
        target_slot: u32,
        source_block: u32,
    },
    DrawCards {
        count: Quantity,
        source_block: u32,
    },
    DiscardCards {
        count: Quantity,
        source_block: u32,
    },
    CounterStackObject {
        target_slot: u32,
        source_block: u32,
    },
    /// Noncombat damage from this invocation's physical source (CR 120.2b).
    DealDamage {
        target_slot: u32,
        #[schemars(with = "String", regex(pattern = r"^[1-9][0-9]*$"))]
        amount: Quantity,
        source_block: u32,
    },
    GainLife {
        recipient: LifeRecipient,
        #[schemars(with = "String", regex(pattern = r"^[1-9][0-9]*$"))]
        amount: Quantity,
        source_block: u32,
    },
    LoseLife {
        recipient: LifeRecipient,
        #[schemars(with = "String", regex(pattern = r"^[1-9][0-9]*$"))]
        amount: Quantity,
        source_block: u32,
    },
}
/// Supported announcement timing; permissions are checked by the runtime.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum SpellTiming {
    Instant,
    MainPhase,
}
/// Permanent resolution is distinct from an empty instruction program.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum SpellResolution {
    Instructions,
    Permanent,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SpellProgram {
    pub timing: SpellTiming,
    pub resolution: SpellResolution,
    pub functioning_zone: FunctioningZone,
    pub targets: Vec<TargetSpec>,
    pub instructions: Vec<Instruction>,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum ActivationCost {
    SacrificeSource,
    TapSource,
    Mana { cost: ManaCost },
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ActivatedAbility {
    pub functioning_zone: FunctioningZone,
    pub cost: ActivationCost,
    pub program: SpellProgram,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SourceProvenance {
    pub printing_id: String,
    pub source_index: u32,
    pub source_sha256: String,
    pub ast_sha256: String,
    pub binding_sha256: String,
    pub archive_manifest_sha256: String,
    pub parser_sha256: String,
    /// Exact compact archive subvalues, not executable text.
    pub source_json: String,
    pub abilities_json: String,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Definition {
    pub oracle_id: String,
    pub name: String,
    pub characteristics: Characteristics,
    pub intrinsic_zone: Option<FunctioningZone>,
    pub spell: Option<SpellProgram>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub activated: Vec<ActivatedAbility>,
    pub capabilities: BTreeSet<Capability>,
    pub provenance: SourceProvenance,
}
impl Definition {
    pub fn requirements(&self) -> BTreeSet<Capability> {
        let mut set = BTreeSet::from([Capability::NormalLayoutV1]);
        match self.characteristics.kind {
            CardKind::Land => {
                set.extend([Capability::BasicLandManaV1, Capability::LandPlayV1]);
            }
            CardKind::Creature => {
                set.extend([Capability::BaseCreatureV1, Capability::CastCreatureV1]);
                if self.characteristics.legendary {
                    set.insert(Capability::CommanderIdentityV1);
                }
            }
            CardKind::Instant | CardKind::Sorcery => {
                set.insert(if self.characteristics.kind == CardKind::Instant {
                    Capability::CastInstantV1
                } else {
                    Capability::CastSorceryV1
                });
            }
        }
        if let Some(program) = &self.spell {
            add_program_requirements(&mut set, program);
        }
        for ability in &self.activated {
            set.insert(Capability::ActivatedAbilityV1);
            set.insert(match ability.cost {
                ActivationCost::SacrificeSource => Capability::SacrificeSourceCostV1,
                ActivationCost::TapSource => Capability::TapSourceCostV1,
                ActivationCost::Mana { .. } => Capability::ManaActivationCostV1,
            });
            add_program_requirements(&mut set, &ability.program);
        }
        set
    }
    pub fn validate(&self) -> Result<(), String> {
        let c = &self.characteristics;
        if self.oracle_id.is_empty()
            || self.name.is_empty()
            || c.mana_cost.generic.is_negative()
            || c.colors.contains(&Color::C)
            || c.color_identity.contains(&Color::C)
            || self.capabilities != self.requirements()
        {
            return Err("invalid identity, characteristics, or capability closure".into());
        }
        if self.provenance.parser_sha256 != PARSER_SHA256
            || self.provenance.archive_manifest_sha256 != ARCHIVE_MANIFEST_SHA256
            || sha256(self.provenance.source_json.as_bytes()) != self.provenance.source_sha256
            || sha256(format!("[{}]", self.provenance.abilities_json).as_bytes())
                != self.provenance.ast_sha256
        {
            return Err("invalid source/AST provenance".into());
        }
        if !self.activated.is_empty() && c.kind != CardKind::Creature {
            return Err("activated programs require supported creature characteristics".into());
        }
        for ability in &self.activated {
            let p = &ability.program;
            if matches!(&ability.cost, ActivationCost::Mana { cost } if cost.generic.is_negative())
                || ability.functioning_zone != FunctioningZone::Battlefield
                || p.functioning_zone != FunctioningZone::Stack
                || p.timing != SpellTiming::Instant
                || p.resolution != SpellResolution::Instructions
                || p.instructions.is_empty()
                || if p.targets.is_empty() {
                    p.instructions.iter().any(|instruction| !matches!(instruction,
                        Instruction::DrawCards {count,..}|Instruction::DiscardCards {count,..} if *count==Quantity::from(1)))
                } else {
                    p.targets
                        .iter()
                        .any(|target| target.domain != TargetDomain::AnyDamageTarget)
                        || p.instructions.iter().any(|instruction| {
                            !matches!(instruction, Instruction::DealDamage { .. })
                        })
                }
            {
                return Err("unsupported activated cost/program shape".into());
            }
            validate_instruction_program(p)?;
        }
        match c.kind {
            CardKind::Land => {
                if c.legendary
                    || !c.basic
                    || c.subtypes.len() != 1
                    || basic_land_color(&c.subtypes[0]).is_none()
                    || c.power.is_some()
                    || c.toughness.is_some()
                    || self.spell.is_some()
                    || self.intrinsic_zone != Some(FunctioningZone::Battlefield)
                {
                    return Err("unsupported land characteristics".into());
                }
            }
            CardKind::Creature => {
                if c.basic
                    || c.power.is_none()
                    || c.toughness.as_ref().is_none_or(|n| n <= &Quantity::zero())
                    || self.intrinsic_zone.is_some()
                {
                    return Err("unsupported creature characteristics".into());
                }
                let p = self
                    .spell
                    .as_ref()
                    .ok_or("missing creature spell program")?;
                if p.functioning_zone != FunctioningZone::Stack
                    || p.timing != SpellTiming::MainPhase
                    || p.resolution != SpellResolution::Permanent
                    || !p.targets.is_empty()
                    || !p.instructions.is_empty()
                {
                    return Err("unsupported creature spell semantics".into());
                }
            }
            CardKind::Instant | CardKind::Sorcery => {
                if c.legendary
                    || c.basic
                    || !c.subtypes.is_empty()
                    || c.power.is_some()
                    || c.toughness.is_some()
                    || self.intrinsic_zone.is_some()
                {
                    return Err("unsupported instant/sorcery characteristics".into());
                }
                let p = self.spell.as_ref().ok_or("missing spell program")?;
                if p.functioning_zone != FunctioningZone::Stack
                    || p.timing
                        != if c.kind == CardKind::Instant {
                            SpellTiming::Instant
                        } else {
                            SpellTiming::MainPhase
                        }
                    || p.resolution != SpellResolution::Instructions
                    || p.instructions.is_empty()
                {
                    return Err("invalid spell functioning zone or empty program".into());
                }
                validate_instruction_program(p)?;
            }
        }
        Ok(())
    }
}
fn validate_instruction_program(p: &SpellProgram) -> Result<(), String> {
    let mut ids = BTreeSet::new();
    for (index, target) in p.targets.iter().enumerate() {
        if usize::try_from(target.slot).ok() != Some(index)
            || target.selection_id.is_empty()
            || !ids.insert(&target.selection_id)
        {
            return Err("unbound, duplicate, or noncontiguous target occurrence".into());
        }
    }
    if p.targets
        .windows(2)
        .any(|targets| targets[0].domain != targets[1].domain)
    {
        return Err("mixed target domains are unsupported".into());
    }
    let mut used = BTreeSet::new();
    for instruction in &p.instructions {
        let binding = match instruction {
            Instruction::ReturnToOwnerHand { target_slot, .. }
            | Instruction::PutOnTopOwnerLibrary { target_slot, .. } => {
                Some((*target_slot, TargetDomain::BattlefieldCreature))
            }
            Instruction::TargetPlayerSacrificeCreature { target_slot, .. } => {
                Some((*target_slot, TargetDomain::OpponentPlayer))
            }
            Instruction::CounterStackObject { target_slot, .. } => {
                let domain = p
                    .targets
                    .get(*target_slot as usize)
                    .map(|target| target.domain)
                    .filter(|domain| {
                        matches!(
                            domain,
                            TargetDomain::StackSpell
                                | TargetDomain::StackActivatedOrTriggeredAbility
                        )
                    })
                    .ok_or("counter requires a stack target domain")?;
                Some((*target_slot, domain))
            }
            Instruction::DealDamage {
                target_slot,
                amount,
                ..
            } => {
                if *amount <= Quantity::zero() {
                    return Err("damage quantity must be positive".into());
                }
                Some((*target_slot, TargetDomain::AnyDamageTarget))
            }
            Instruction::GainLife {
                recipient, amount, ..
            }
            | Instruction::LoseLife {
                recipient, amount, ..
            } => {
                if *amount <= Quantity::zero() {
                    return Err("life quantity must be positive".into());
                }
                match recipient {
                    LifeRecipient::Controller => None,
                    LifeRecipient::Target { target_slot } => {
                        Some((*target_slot, TargetDomain::AnyPlayer))
                    }
                }
            }
            Instruction::DrawCards { count, .. } | Instruction::DiscardCards { count, .. } => {
                if *count != Quantity::from(1) {
                    return Err("unsupported draw/discard cardinality".into());
                }
                None
            }
        };
        if let Some((slot, domain)) = binding {
            if p.targets
                .get(slot as usize)
                .is_none_or(|target| target.domain != domain)
            {
                return Err("unbound target slot/domain".into());
            }
            used.insert(slot);
        }
    }
    if used.len() != p.targets.len() {
        return Err("unused target occurrence".into());
    }
    Ok(())
}
fn add_program_requirements(set: &mut BTreeSet<Capability>, program: &SpellProgram) {
    for target in &program.targets {
        set.insert(match target.domain {
            TargetDomain::BattlefieldCreature => Capability::CreatureTargetV1,
            TargetDomain::OpponentPlayer => Capability::OpponentTargetV1,
            TargetDomain::StackSpell => Capability::StackSpellTargetV1,
            TargetDomain::StackActivatedOrTriggeredAbility => Capability::StackAbilityTargetV1,
            TargetDomain::AnyDamageTarget => Capability::AnyDamageTargetV1,
            TargetDomain::AnyPlayer => Capability::AnyPlayerTargetV1,
        });
    }
    for instruction in &program.instructions {
        set.insert(match instruction {
            Instruction::ReturnToOwnerHand { .. } => Capability::OwnerHandMoveV1,
            Instruction::PutOnTopOwnerLibrary { .. } => Capability::OwnerLibraryTopMoveV1,
            Instruction::TargetPlayerSacrificeCreature { .. } => {
                Capability::TargetPlayerSacrificeCreatureV1
            }
            Instruction::DrawCards { .. } => Capability::ControllerDrawV1,
            Instruction::DiscardCards { .. } => Capability::ControllerDiscardV1,
            Instruction::CounterStackObject { .. } => Capability::CounterStackObjectV1,
            Instruction::DealDamage { .. } => Capability::NoncombatDamageV1,
            Instruction::GainLife { .. } => Capability::LifeGainV1,
            Instruction::LoseLife { .. } => Capability::LifeLossV1,
        });
    }
}
pub fn basic_land_color(subtype: &str) -> Option<Color> {
    match subtype {
        "Plains" => Some(Color::W),
        "Island" => Some(Color::U),
        "Swamp" => Some(Color::B),
        "Mountain" => Some(Color::R),
        "Forest" => Some(Color::G),
        _ => None,
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Catalog {
    #[schemars(range(min = 3, max = 7))]
    pub schema_version: u32,
    pub rules_edition: String,
    pub rules_sha256: String,
    pub definitions: Vec<Definition>,
}
impl Catalog {
    pub fn validate(&self) -> Result<(), String> {
        if !matches!(self.schema_version, 3..=7)
            || self.rules_edition != RULES_EDITION
            || self.rules_sha256 != RULES_SHA256
        {
            return Err("unsupported IR version or rules snapshot".into());
        }
        let mut ids = BTreeSet::new();
        for d in &self.definitions {
            d.validate()?;
            if self.schema_version < 7
                && (d.capabilities.iter().any(|cap| {
                    matches!(
                        cap,
                        Capability::AnyDamageTargetV1
                            | Capability::AnyPlayerTargetV1
                            | Capability::NoncombatDamageV1
                            | Capability::LifeGainV1
                            | Capability::LifeLossV1
                    )
                }) || d
                    .activated
                    .iter()
                    .any(|ability| !ability.program.targets.is_empty())
                    || d.spell.as_ref().is_some_and(|program| {
                        program.resolution == SpellResolution::Instructions
                            && (program.targets.is_empty()
                                || program.instructions.iter().any(|instruction| {
                                    matches!(
                                        instruction,
                                        Instruction::DrawCards { .. }
                                            | Instruction::DiscardCards { .. }
                                    )
                                }))
                    }))
            {
                return Err("IR v7 is required for damage/life, targeted activations or untargeted spell instructions".into());
            }
            if self.schema_version < 6
                && (d
                    .activated
                    .iter()
                    .any(|ability| ability.cost != ActivationCost::SacrificeSource)
                    || d.spell.as_ref().is_some_and(|program| {
                        program.targets.iter().any(|target| {
                            matches!(
                                target.domain,
                                TargetDomain::StackSpell
                                    | TargetDomain::StackActivatedOrTriggeredAbility
                            )
                        }) || program.instructions.iter().any(|instruction| {
                            matches!(instruction, Instruction::CounterStackObject { .. })
                        })
                    }))
            {
                return Err(
                    "IR v6 is required for stack targets or tap/mana activation costs".into(),
                );
            }
            if self.schema_version < 5
                && (!d.activated.is_empty()
                    || d.spell.as_ref().is_some_and(|program| {
                        program
                            .targets
                            .iter()
                            .any(|target| target.domain == TargetDomain::OpponentPlayer)
                            || program.instructions.iter().any(|instruction| {
                                matches!(
                                    instruction,
                                    Instruction::TargetPlayerSacrificeCreature { .. }
                                        | Instruction::DrawCards { .. }
                                        | Instruction::DiscardCards { .. }
                                )
                            })
                    }))
            {
                return Err("IR v5 is required for activated or opponent-choice programs".into());
            }
            if self.schema_version == 3
                && (d.characteristics.kind == CardKind::Sorcery
                    || d.spell.as_ref().is_some_and(|program| {
                        program.instructions.iter().any(|instruction| {
                            matches!(instruction, Instruction::PutOnTopOwnerLibrary { .. })
                        })
                    }))
            {
                return Err("IR v3 does not support sorcery/library-top semantics".into());
            }
            if !ids.insert(&d.oracle_id) {
                return Err("duplicate Oracle identity".into());
            }
        }
        if !self
            .definitions
            .windows(2)
            .all(|w| w[0].oracle_id < w[1].oracle_id)
        {
            return Err("catalog is not canonically ordered".into());
        }
        Ok(())
    }
    pub fn read(json: &str) -> Result<Self, String> {
        let c: Self = strict_read(json)?;
        c.validate()?;
        Ok(c)
    }
}
pub fn sha256(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
/// IR v3/v4/v5/v6/v7 canonicalization (same algorithm as v1/v2): UTF-8 compact serde_json Value (sorted object keys),
/// ordered arrays, exact quantities as strings. Separate from JS archive hashes.
pub fn canonical<T: Serialize>(value: &T) -> Result<String, String> {
    serde_json::to_value(value)
        .and_then(|v| serde_json::to_string(&v))
        .map_err(|e| e.to_string())
}
pub fn digest<T: Serialize>(value: &T) -> Result<String, String> {
    Ok(sha256(canonical(value)?.as_bytes()))
}
pub fn catalog_schema() -> serde_json::Value {
    let mut schema = serde_json::to_value(schemars::schema_for!(Catalog)).expect("schema is JSON");
    // Catalog versions share unchanged definition shapes. Restrict the v3
    // vocabularies explicitly so schema validation cannot downgrade programs.
    // https://json-schema.org/understanding-json-schema/reference/conditionals
    schema["allOf"] = serde_json::json!([{
        "if": {"required": ["schema_version"], "properties": {"schema_version": {"const": 3}}},
        "then": {"properties": {"definitions": {"items": {"properties": {
            "characteristics": {"properties": {"kind": {"enum": [CardKind::Land, CardKind::Creature, CardKind::Instant]}}},
            "capabilities": {"items": {"enum": [Capability::NormalLayoutV1, Capability::BasicLandManaV1,
                Capability::LandPlayV1, Capability::BaseCreatureV1, Capability::CommanderIdentityV1,
                Capability::CastInstantV1, Capability::CastCreatureV1, Capability::CreatureTargetV1,
                Capability::OwnerHandMoveV1]}},
            "spell": {"anyOf": [{"type": "null"}, {"type": "object", "properties": {
                "instructions": {"items": {"properties": {"op": {"enum": ["return_to_owner_hand"]}}}}
            }}]}
        }}}}}
    }, {
        "if": {"required": ["schema_version"], "properties": {"schema_version": {"enum": [3,4]}}},
        "then": {"properties": {"definitions": {"items": {"properties": {
            "activated": {"maxItems": 0},
            "capabilities": {"items": {"enum": [Capability::NormalLayoutV1, Capability::BasicLandManaV1,
                Capability::LandPlayV1, Capability::BaseCreatureV1, Capability::CommanderIdentityV1,
                Capability::CastInstantV1, Capability::CastCreatureV1, Capability::CreatureTargetV1,
                Capability::OwnerHandMoveV1, Capability::CastSorceryV1, Capability::OwnerLibraryTopMoveV1]}},
            "spell": {"anyOf": [{"type":"null"}, {"type":"object", "properties": {
                "targets": {"items": {"properties": {"domain": {"const":"battlefield_creature"}}}},
                "instructions": {"items": {"properties": {"op": {"enum":["return_to_owner_hand","put_on_top_owner_library"]}}}}
            }}]}
        }}}}}
    }, {
        "if": {"required": ["schema_version"], "properties": {"schema_version": {"enum": [3,4,5]}}},
        "then": {"properties": {"definitions": {"items": {"properties": {
            "activated": {"items": {"properties": {"cost": {"const": "sacrifice_source"}}}},
            "capabilities": {"items": {"not": {"enum": [Capability::StackSpellTargetV1,
                Capability::StackAbilityTargetV1, Capability::CounterStackObjectV1,
                Capability::TapSourceCostV1, Capability::ManaActivationCostV1]}}},
            "spell": {"anyOf": [{"type":"null"}, {"type":"object", "properties": {
                "targets": {"items": {"properties": {"domain": {"enum": ["battlefield_creature", "opponent_player"]}}}},
                "instructions": {"items": {"properties": {"op": {"not": {"const": "counter_stack_object"}}}}}
            }}]}
        }}}}}
    }, {
        "if":{"required":["schema_version"],"properties":{"schema_version":{"enum":[3,4,5,6]}}},
        "then":{"properties":{"definitions":{"items":{
            "properties":{
                "capabilities":{"items":{"not":{"enum":[Capability::AnyDamageTargetV1,Capability::AnyPlayerTargetV1,Capability::NoncombatDamageV1,Capability::LifeGainV1,Capability::LifeLossV1]}}},
                "activated":{"items":{"properties":{"program":{"properties":{"targets":{"maxItems":0},"instructions":{"items":{"properties":{"op":{"enum":["draw_cards","discard_cards"]}}}}}}}}},
                "spell":{"anyOf":[{"type":"null"},{"type":"object","properties":{
                    "targets":{"items":{"properties":{"domain":{"enum":["battlefield_creature","opponent_player","stack_spell","stack_activated_or_triggered_ability"]}}}},
                    "instructions":{"items":{"properties":{"op":{"enum":["return_to_owner_hand","put_on_top_owner_library","target_player_sacrifice_creature","counter_stack_object"]}}}}
                }}]}
            },
            "allOf":[{"if":{"properties":{"characteristics":{"properties":{"kind":{"enum":["instant","sorcery"]}}}}},
                "then":{"properties":{"spell":{"properties":{"targets":{"minItems":1}}}}}}]
        }}}}
    }]);
    schema
}
pub fn strict_read<T: de::DeserializeOwned>(json: &str) -> Result<T, String> {
    let _: UniqueJson = serde_json::from_str(json).map_err(|e| e.to_string())?;
    serde_json::from_str(json).map_err(|e| e.to_string())
}
/// Reject duplicates before tagged enums or maps could discard a repeated field.
struct UniqueJson;
impl<'de> Deserialize<'de> for UniqueJson {
    fn deserialize<D: de::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        struct Visitor;
        impl<'de> de::Visitor<'de> for Visitor {
            type Value = UniqueJson;
            fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                f.write_str("JSON without duplicate object keys")
            }
            fn visit_map<M: de::MapAccess<'de>>(self, mut m: M) -> Result<UniqueJson, M::Error> {
                let mut keys = BTreeSet::new();
                while let Some(k) = m.next_key::<String>()? {
                    if !keys.insert(k) {
                        return Err(de::Error::custom("duplicate JSON key"));
                    }
                    m.next_value::<UniqueJson>()?;
                }
                Ok(UniqueJson)
            }
            fn visit_seq<A: de::SeqAccess<'de>>(self, mut a: A) -> Result<UniqueJson, A::Error> {
                while a.next_element::<UniqueJson>()?.is_some() {}
                Ok(UniqueJson)
            }
            fn visit_bool<E: de::Error>(self, _: bool) -> Result<UniqueJson, E> {
                Ok(UniqueJson)
            }
            fn visit_i64<E: de::Error>(self, _: i64) -> Result<UniqueJson, E> {
                Ok(UniqueJson)
            }
            fn visit_u64<E: de::Error>(self, _: u64) -> Result<UniqueJson, E> {
                Ok(UniqueJson)
            }
            fn visit_f64<E: de::Error>(self, _: f64) -> Result<UniqueJson, E> {
                Ok(UniqueJson)
            }
            fn visit_str<E: de::Error>(self, _: &str) -> Result<UniqueJson, E> {
                Ok(UniqueJson)
            }
            fn visit_unit<E: de::Error>(self) -> Result<UniqueJson, E> {
                Ok(UniqueJson)
            }
        }
        d.deserialize_any(Visitor)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn quantities_are_exact_canonical_strings() {
        let mut q: Quantity = "999999999999999999999999999999999999999".parse().unwrap();
        q.add(&Quantity::from(1));
        assert_eq!(q.to_string(), "1000000000000000000000000000000000000000");
        assert_eq!(
            serde_json::from_str::<Quantity>("\"-42\"").unwrap(),
            Quantity::from(-42)
        );
        for input in ["1", "\"01\"", "\"-0\"", "\"+1\"", "\"1.0\""] {
            assert!(serde_json::from_str::<Quantity>(input).is_err());
        }
    }
    #[test]
    fn rejects_duplicates_even_in_nested_untyped_json() {
        assert!(strict_read::<serde_json::Value>(r#"{"x":[{"a":1,"a":2}]}"#).is_err());
    }
}
