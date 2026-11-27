//! The typed lowering mirror of archived AST blocks: complete shapes with
//! unknown fields rejected. Lowerers consume these, never raw strings.
use super::*;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct SourceKeyword {
    pub(crate) keyword: String,
    pub(crate) cost: Option<Vec<SourceCost>>,
    pub(crate) filter: Option<serde_json::Value>,
    pub(crate) protection_from: Option<Vec<SourceProtectionQuality>>,
    /// The printed N of a parameterized keyword ("Bushido 2", CR 702.45a).
    pub(crate) amount: Option<u32>,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(tag = "scope", rename_all = "kebab-case", deny_unknown_fields)]
pub(crate) enum SourceProtectionQuality {
    Color { color: String },
    AllColors {},
}

impl<'de> Deserialize<'de> for SourceKeyword {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        use serde::de::Error;
        let value = serde_json::Value::deserialize(deserializer)?;
        let object = value
            .as_object()
            .ok_or_else(|| D::Error::custom("keyword must be an object"))?;
        let keyword = object
            .get("keyword")
            .and_then(|k| k.as_str())
            .ok_or_else(|| D::Error::custom("missing keyword"))?
            .to_owned();
        let keyword = match keyword.as_str() {
            "first-strike" => "first strike".to_owned(),
            "double-strike" => "double strike".to_owned(),
            _ => keyword,
        };
        if !matches!(
            keyword.as_str(),
            "bestow"
                | "equip"
                | "enchant"
                | "cascade"
                | "kicker"
                | "buyback"
                | "evoke"
                | "echo"
                | "unearth"
                | "encore"
                | "retrace"
                | "dash"
                | "escape"
                | "affinity"
                | "delve"
                | "convoke"
                | "improvise"
                | "cycling"
                | "typecycling"
                | "flashback"
                | "morph"
                | "megamorph"
                | "protection"
                | "changeling"
                | "landwalk"
                | "prowess"
                | "crew"
                | "bushido"
                | "afterlife"
                | "soulshift"
                | "exalted"
                | "myriad"
                | "dethrone"
                | "training"
                | "afflict"
                | "rampage"
                | "melee"
                | "battle cry"
                | "flanking"
                | "annihilator"
                | "mentor"
        ) && serde_json::from_value::<KeywordAbility>(serde_json::Value::String(keyword.clone()))
            .is_err()
        {
            return Err(D::Error::custom(format!("keyword:{keyword}")));
        }
        let allowed = match keyword.as_str() {
            "bestow" | "equip" | "kicker" | "buyback" | "evoke" | "echo" | "unearth" | "encore"
            | "dash" | "escape" | "cycling" | "flashback" | "morph" | "megamorph" => {
                ["keyword", "cost"].as_slice()
            }
            "enchant" | "affinity" | "landwalk" => ["keyword", "filter"].as_slice(),
            "typecycling" => ["keyword", "cost", "filter"].as_slice(),
            "protection" => ["keyword", "from"].as_slice(),
            "crew" | "bushido" | "afterlife" | "soulshift" | "afflict" | "rampage"
            | "annihilator" => ["keyword", "amount"].as_slice(),
            _ => ["keyword"].as_slice(),
        };
        if object.len() != allowed.len() || object.keys().any(|k| !allowed.contains(&k.as_str())) {
            let field = object
                .keys()
                .find(|k| !allowed.contains(&k.as_str()))
                .map(String::as_str)
                .unwrap_or("missing_parameter");
            return Err(D::Error::custom(format!("keyword:{keyword}:field:{field}")));
        }
        let cost = object
            .get("cost")
            .map(|c| serde_json::from_value(c.clone()).map_err(D::Error::custom))
            .transpose()?;
        let filter = object.get("filter").cloned();
        // A fixed positive N; "X" and other amounts reject by name.
        let amount = object
            .get("amount")
            .map(|amount| {
                amount
                    .as_u64()
                    .and_then(|n| u32::try_from(n).ok())
                    .filter(|n| *n >= 1)
                    .ok_or_else(|| D::Error::custom(format!("keyword:{keyword}:amount")))
            })
            .transpose()?;
        let protection_from = object
            .get("from")
            .map(|from| {
                serde_json::from_value(from.clone())
                    .map_err(|_| D::Error::custom("keyword:protection:quality"))
            })
            .transpose()?;
        Ok(Self {
            keyword,
            cost,
            filter,
            protection_from,
            amount,
        })
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
#[serde(untagged)]
pub(crate) enum ModeCount {
    Exact {
        exactly: u32,
    },
    Minimum {
        #[serde(rename = "atLeast")]
        minimum: u32,
    },
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ModeOption {
    pub(crate) effects: Vec<Sentence>,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
#[serde(tag = "condition", rename_all = "kebab-case")]
pub(crate) enum SourceCondition {
    WasCast {
        what: serde_json::Value,
        by: PlayerSelector,
    },
    Control {
        who: SourceControlPlayer,
        what: TriggerObject,
    },
    CardsInHand {
        who: PlayerSelector,
        comparison: SourceComparison,
    },
    Status {
        what: TriggerObject,
        status: TapStatus,
    },
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct SourceComparison {
    pub(crate) op: String,
    pub(crate) value: Count,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(tag = "trigger", rename_all = "kebab-case", deny_unknown_fields)]
pub(crate) enum SourceTrigger {
    DealsDamage {
        what: SourceDamageObject,
        #[serde(default, deserialize_with = "present_bool_field")]
        combat: Option<bool>,
        to: serde_json::Value,
        #[serde(default, deserialize_with = "present_ast_field")]
        grouping: Option<String>,
    },
    Attacks {
        what: SourceAttacker,
    },
    Cast {
        who: PlayerSelector,
        what: SourceTypeFilter,
        #[serde(default, rename = "notFrom", deserialize_with = "present_json_operand")]
        not_from: Option<serde_json::Value>,
    },
    Draws {
        who: PlayerSelector,
    },
    AttacksAndIsntBlocked {
        what: SourceAttacker,
    },
    Phase {
        phase: String,
        whose: serde_json::Value,
    },
    PutIntoGraveyard {
        what: TriggerObject,
        from: String,
        to: serde_json::Value,
    },
    Enters {
        what: TriggerObject,
    },
    BecomesTapped {
        what: TriggerObject,
    },
    TurnedFaceUp {
        what: TriggerObject,
    },
    Dies {
        what: TriggerObject,
    },
    AnyOf {
        events: Vec<SourceTrigger>,
    },
}

// Self and complete creature filters have distinct meanings. Event-pronoun
// sources remain unsupported. Named source references require archive-bound
// canonicalization to "this" before this strict mirror is decoded.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(tag = "ref", rename_all = "kebab-case", deny_unknown_fields)]
pub(crate) enum SourceDamageObject {
    Equipped {
        noun: String,
    },
    Enchanted {
        noun: String,
    },
    This {
        noun: String,
    },
    Filter {
        filter: SourceDamageFilter,
        count: Count,
    },
}

// This creature-domain mirror consumes every source filter field. Requiring
// "creature" in root types avoids narrowing a printed artifact-only source.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct SourceDamageFilter {
    pub(crate) types: Vec<String>,
    #[serde(default, rename = "nonTypes", deserialize_with = "present_ast_field")]
    pub(crate) non_types: Option<Vec<String>>,
    #[serde(default, rename = "allOf", deserialize_with = "present_ast_field")]
    pub(crate) all_of: Option<Vec<SourceTypeFilter>>,
    #[serde(default, rename = "anyOf", deserialize_with = "present_ast_field")]
    pub(crate) any_of: Option<Vec<SourceTypeFilter>>,
    #[serde(default, deserialize_with = "present_ast_field")]
    pub(crate) control: Option<String>,
    #[serde(default, deserialize_with = "present_ast_field")]
    pub(crate) properties: Option<Vec<serde_json::Value>>,
}

pub(crate) fn present_ast_field<'de, D, T>(deserializer: D) -> Result<Option<T>, D::Error>
where
    D: serde::Deserializer<'de>,
    T: Deserialize<'de>,
{
    T::deserialize(deserializer).map(Some)
}

// Raw JSON operands still require presence to survive decoding. Their lowering
// operators validate supported shapes; explicit null must never mean omission.
pub(crate) fn present_json_operand<'de, D>(
    deserializer: D,
) -> Result<Option<serde_json::Value>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let value = serde_json::Value::deserialize(deserializer)?;
    if value.is_null() {
        return Err(serde::de::Error::custom("expected a non-null JSON operand"));
    }
    Ok(Some(value))
}

// ObjectFilter semantics: fields/arrays conjunctive, explicit anyOf disjunctive.
// packages/oracle-parser/src/ast.ts ObjectFilter; CR 205.2b, 601.2i.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct SourceTypeFilter {
    #[serde(default, deserialize_with = "present_ast_field")]
    pub(crate) subtypes: Option<Vec<String>>,
    #[serde(default, deserialize_with = "present_ast_field")]
    pub(crate) colors: Option<Vec<String>>,
    #[serde(default, deserialize_with = "present_ast_field")]
    pub(crate) cls: Option<String>,
    #[serde(default, deserialize_with = "present_ast_field")]
    pub(crate) types: Option<Vec<String>>,
    #[serde(default, rename = "nonTypes", deserialize_with = "present_ast_field")]
    pub(crate) non_types: Option<Vec<String>>,
    #[serde(default, rename = "allOf", deserialize_with = "present_ast_field")]
    pub(crate) all_of: Option<Vec<SourceTypeFilter>>,
    #[serde(default, rename = "anyOf", deserialize_with = "present_ast_field")]
    pub(crate) any_of: Option<Vec<SourceTypeFilter>>,
}

// Separate attack subject mirror: no death-only fields, status, ownership,
// grouping or qualifiers can silently disappear. A filter's arrays are AND.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(tag = "ref", rename_all = "kebab-case", deny_unknown_fields)]
pub(crate) enum SourceAttacker {
    Equipped {
        noun: String,
    },
    Enchanted {
        noun: String,
    },
    This {
        noun: String,
    },
    Filter {
        filter: SourceAttackerFilter,
        count: Count,
    },
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct SourceAttackerFilter {
    #[serde(default, deserialize_with = "present_ast_field")]
    pub(crate) types: Option<Vec<String>>,
    #[serde(default, rename = "nonTypes", deserialize_with = "present_ast_field")]
    pub(crate) non_types: Option<Vec<String>>,
    #[serde(default, rename = "allOf", deserialize_with = "present_ast_field")]
    pub(crate) all_of: Option<Vec<SourceTypeFilter>>,
    #[serde(default, rename = "anyOf", deserialize_with = "present_ast_field")]
    pub(crate) any_of: Option<Vec<SourceTypeFilter>>,
    #[serde(default, deserialize_with = "present_ast_field")]
    pub(crate) control: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(tag = "ref", rename_all = "kebab-case", deny_unknown_fields)]
pub(crate) enum TriggerObject {
    Equipped { noun: String },
    Enchanted { noun: String },
    This { noun: String },
    Filter { filter: TriggerFilter, count: Count },
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct TriggerFilter {
    #[serde(default, deserialize_with = "present_ast_field")]
    pub(crate) properties: Option<[SourceProperty; 1]>,
    #[serde(default)]
    pub(crate) other: bool,
    pub(crate) types: Vec<String>,
    pub(crate) control: Option<String>,
    #[serde(default, rename = "nonToken")]
    pub(crate) non_token: bool,
    #[serde(default, rename = "nonSubtypes")]
    pub(crate) non_subtypes: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct SourceProperty {
    pub(crate) property: String,
    pub(crate) comparison: SourceComparison,
}

// CR 113.10, 611.2a/c and 613.1f: gained keyword abilities share the
// ordinary captured-target grant operation. The complete program is validated
// by contracts; this decoder must never discard an unreviewed ability or field.
// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(try_from = "serde_json::Value")]
pub(crate) struct Sentence {
    pub(crate) condition: Option<serde_json::Value>,
    pub(crate) condition_kind: Option<String>,
    pub(crate) bindings: Vec<serde_json::Value>,
    pub(crate) steps: Vec<Step>,
    /// CR 107.3: which steps read the sentence's variable X, in step order.
    pub(crate) reads_x: Vec<bool>,
    /// CR 608.2c: the objects each step names that a later pronoun could
    /// mean, in step order.
    pub(crate) introduces: Vec<Introduced>,
}

/// The objects one printed step names besides its own pronouns and the
/// source (CR 201.5, 700.7, 608.2c). A later "it" or "that <noun>" may mean any of
/// them, so the antecedent rule counts them as candidates.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct Introduced {
    /// The step announces an object target (CR 601.2c).
    pub(crate) object_target: bool,
    /// The step names an untargeted object: a chosen, created, revealed,
    /// attached or set object, or a step result (CR 608.2c, 608.2h).
    pub(crate) other: bool,
    /// The step names the source itself, as "this <type>" or by name
    /// (CR 201.5, 700.7): a later bare "it" may mean it.
    pub(crate) source: bool,
}

impl Introduced {
    pub(crate) fn join(self, other: Self) -> Self {
        Self {
            object_target: self.object_target || other.object_target,
            other: self.other || other.other,
            source: self.source || other.source,
        }
    }

    /// Scan one archived step. Pronouns and the trigger's own event object
    /// name nothing new, and "this"/"self" names only the source; every
    /// other object reference, and every step that creates, reveals or
    /// moves cards it does not target, introduces a candidate. A "this
    /// <type>" inside a quoted granted ability names the object that has
    /// that ability, not this block's source (CR 113.10, 700.7), so it never
    /// counts as naming the source.
    pub(crate) fn of_step(step: &serde_json::Value) -> Self {
        fn walk(value: &serde_json::Value, quoted: bool, out: &mut Introduced) {
            match value {
                serde_json::Value::Object(object) => {
                    let quoted =
                        quoted || object.get("gained").and_then(|g| g.as_str()) == Some("quoted");
                    match object.get("ref").and_then(|r| r.as_str()) {
                        Some("it" | "that" | "them" | "trigger-event-object") => {}
                        Some("this" | "self") => out.source |= !quoted,
                        Some("target" | "any-target") => out.object_target = true,
                        Some(_) => out.other = true,
                        None => {}
                    }
                    if object.get("to").and_then(|t| t.as_str()) == Some("any-target") {
                        out.object_target = true;
                    }
                    if object
                        .get("effect")
                        .and_then(|e| e.as_str())
                        .is_some_and(|effect| {
                            matches!(
                                effect,
                                "create-token"
                                    | "copy-card"
                                    | "copy-stack-object"
                                    | "manifest"
                                    | "manifest-dread"
                                    | "cloak"
                                    | "amass"
                                    | "incubate"
                                    | "create-role"
                                    | "discover"
                                    | "explore"
                                    | "search"
                                    | "reveal"
                                    | "look-at"
                                    | "choose-object"
                                    | "draw"
                                    | "discard"
                                    | "mill"
                                    | "surveil"
                                    | "connive"
                                    | "learn"
                            )
                        })
                    {
                        out.other = true;
                    }
                    object.values().for_each(|v| walk(v, quoted, out));
                }
                serde_json::Value::Array(items) => items.iter().for_each(|v| walk(v, quoted, out)),
                _ => {}
            }
        }
        let mut out = Self::default();
        walk(step, false, &mut out);
        out
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SentenceFields {
    #[serde(default, deserialize_with = "present_ast_field")]
    condition: Option<serde_json::Value>,
    #[serde(
        default,
        rename = "conditionKind",
        deserialize_with = "present_ast_field"
    )]
    condition_kind: Option<String>,
    #[serde(default)]
    bindings: Vec<serde_json::Value>,
    steps: Vec<Step>,
}

impl TryFrom<serde_json::Value> for Sentence {
    type Error = serde_json::Error;
    fn try_from(value: serde_json::Value) -> Result<Self, Self::Error> {
        /// An `{"amount": "x"}` operand anywhere inside the step.
        fn reads_x(value: &serde_json::Value) -> bool {
            match value {
                serde_json::Value::Object(object) => {
                    object.get("amount").and_then(|a| a.as_str()) == Some("x")
                        || object.values().any(reads_x)
                }
                serde_json::Value::Array(items) => items.iter().any(reads_x),
                _ => false,
            }
        }
        let reads = value["steps"]
            .as_array()
            .map(|steps| steps.iter().map(reads_x).collect())
            .unwrap_or_default();
        let introduces = value["steps"]
            .as_array()
            .map(|steps| steps.iter().map(Introduced::of_step).collect())
            .unwrap_or_default();
        let SentenceFields {
            condition,
            condition_kind,
            bindings,
            steps,
        } = serde_json::from_value(value)?;
        Ok(Self {
            condition,
            condition_kind,
            bindings,
            steps,
            reads_x: reads,
            introduces,
        })
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields, remote = "Self")]
#[serde(tag = "effect", rename_all = "kebab-case")]
pub(crate) enum Step {
    GainControl {
        who: Box<ast::PlayerRef>,
        what: Box<ast::ObjectRef>,
        #[serde(default, deserialize_with = "present_json_operand")]
        duration: Option<serde_json::Value>,
    },
    Become {
        what: Box<ast::ObjectRef>,
        spec: Box<ast::BecomeSpec>,
        duration: Box<ast::Duration>,
    },
    ModifyCharacteristics {
        what: Box<ast::ObjectRef>,
        modifiers: Vec<ast::ResolvingCharacteristicModifier>,
        snapshot: ast::LitEffectCreated,
        #[serde(rename = "statedAs")]
        stated_as: Option<ast::EffectModifyCharacteristicsStatedAs>,
        duration: Box<ast::Duration>,
    },
    LoseAbilities {
        what: serde_json::Value,
        removal: serde_json::Value,
        duration: SourceDuration,
    },
    PlayCard {
        instruction: serde_json::Value,
        optional: bool,
    },
    LoseGame {
        who: PlayerSelector,
    },
    AdditionalLandPlays {
        allowance: serde_json::Value,
        duration: SourceDuration,
    },
    LookAt {
        who: serde_json::Value,
        what: serde_json::Value,
    },
    Simultaneous {
        steps: Vec<serde_json::Value>,
    },
    ShuffleIntoLibrary {
        who: PlayerSelector,
        what: Vec<serde_json::Value>,
        library: serde_json::Value,
    },
    Scry {
        amount: serde_json::Value,
    },
    Surveil {
        amount: serde_json::Value,
    },
    Mill {
        who: serde_json::Value,
        amount: serde_json::Value,
    },
    CreateReplacement {
        replacement: serde_json::Value,
        duration: SourceDuration,
    },
    ChooseValue {
        who: PlayerSelector,
        category: String,
    },
    AddMana {
        who: PlayerSelector,
        mana: SourceMana,
        #[serde(default, rename = "forEach", deserialize_with = "present_json_operand")]
        for_each: Option<serde_json::Value>,
        #[serde(
            default,
            rename = "restriction",
            deserialize_with = "present_json_operand"
        )]
        spend_restriction: Option<serde_json::Value>,
        #[serde(default, deserialize_with = "present_json_operand")]
        riders: Option<serde_json::Value>,
    },
    Exile {
        what: serde_json::Value,
    },
    Untap {
        what: serde_json::Value,
    },
    Tap {
        what: serde_json::Value,
    },
    /// CR 701.27.
    Transform {
        what: serde_json::Value,
        who: PlayerSelector,
        #[serde(default)]
        optional: bool,
    },
    Destroy {
        what: serde_json::Value,
        #[serde(default, deserialize_with = "present_bool_field")]
        optional: Option<bool>,
        #[serde(
            default,
            rename = "regenerationForbidden",
            deserialize_with = "present_bool_field"
        )]
        regeneration_forbidden: Option<bool>,
    },
    Reveal {
        who: PlayerSelector,
        what: serde_json::Value,
    },
    Search {
        who: PlayerSelector,
        zones: serde_json::Value,
        #[serde(rename = "for")]
        find: serde_json::Value,
    },
    Shuffle {
        who: PlayerSelector,
        library: serde_json::Value,
        #[serde(default, deserialize_with = "present_bool_field")]
        optional: Option<bool>,
    },
    /// CR 111.1-111.4, 111.10: the token recipe stays raw here; the token
    /// lowerer decodes predefined and specified recipes and rejects every
    /// other recipe (copies) or modifier by name.
    CreateToken {
        #[serde(default, deserialize_with = "present_json_operand")]
        who: Option<serde_json::Value>,
        count: serde_json::Value,
        token: serde_json::Value,
        #[serde(default, rename = "forEach", deserialize_with = "present_json_operand")]
        for_each: Option<serde_json::Value>,
    },
    KeywordAction {
        action: String,
        who: serde_json::Value,
        #[serde(default, deserialize_with = "present_json_operand")]
        times: Option<serde_json::Value>,
    },
    Fight {
        a: serde_json::Value,
        b: serde_json::Value,
    },
    GetCounters {
        who: PlayerSelector,
        counter: SourceCounter,
        count: serde_json::Value,
    },
    RemoveCounters {
        from: serde_json::Value,
        counter: SourceCounter,
        count: serde_json::Value,
    },
    Modal {
        count: ModeCount,
        options: Vec<ModeOption>,
    },
    GainAbilities {
        what: serde_json::Value,
        abilities: Vec<serde_json::Value>,
        duration: SourceDuration,
    },
    Cant {
        what: Box<ast::ObjectRef>,
        action: Box<ast::CantAction>,
        duration: SourceDuration,
    },
    CombatRule {
        what: Box<ast::ObjectRef>,
        rule: Box<ast::CombatRule>,
        duration: SourceDuration,
    },
    Pump {
        what: serde_json::Value,
        power: ast::SignedAmount,
        toughness: ast::SignedAmount,
        duration: SourceDuration,
        #[serde(default, rename = "forEach", deserialize_with = "present_json_operand")]
        for_each: Option<serde_json::Value>,
    },
    Regenerate {
        what: serde_json::Value,
    },
    PreventDamage {
        prevention: Box<ast::DamagePrevention>,
        duration: SourceDuration,
    },
    Counter {
        what: CounterTarget,
    },
    #[serde(skip)]
    Evaluated(Box<ast::Effect>),
    ReturnToHand {
        what: serde_json::Value,
        to: String,
        #[serde(default, deserialize_with = "present_bool_field")]
        optional: Option<bool>,
    },
    MoveZone {
        what: serde_json::Value,
        to: serde_json::Value,
        #[serde(default, deserialize_with = "present_ast_field")]
        order: Option<String>,
        #[serde(default, deserialize_with = "present_bool_field")]
        tapped: Option<bool>,
        #[serde(default, deserialize_with = "present_ast_field")]
        controller: Option<String>,
        /// The player who puts the cards (CR 110.2).
        #[serde(default, deserialize_with = "present_json_operand")]
        who: Option<serde_json::Value>,
    },
    Discard(DiscardStep),
}

impl<'de> Deserialize<'de> for Step {
    fn deserialize<D: serde::Deserializer<'de>>(decoder: D) -> Result<Self, D::Error> {
        let raw = serde_json::Value::deserialize(decoder)?;
        if matches!(
            raw["effect"].as_str(),
            Some("draw" | "gain-life" | "lose-life" | "damage" | "put-counters" | "sacrifice")
        ) {
            if raw
                .as_object()
                .is_some_and(|fields| fields.values().any(serde_json::Value::is_null))
            {
                return Err(serde::de::Error::custom(
                    "explicit null on an evaluated action",
                ));
            }
            return serde_json::from_value(raw)
                .map(|effect| Self::Evaluated(Box::new(effect)))
                .map_err(serde::de::Error::custom);
        }
        Step::deserialize(raw).map_err(serde::de::Error::custom)
    }
}

impl Step {
    pub(crate) fn damage(&self) -> Option<&ast::EffectDamage> {
        match self.effect()? {
            ast::Effect::Damage(effect) => Some(effect),
            _ => None,
        }
    }

    pub(crate) fn effect(&self) -> Option<&ast::Effect> {
        match self {
            Self::Evaluated(effect) => Some(effect),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(untagged, deny_unknown_fields)]
pub(crate) enum DiscardStep {
    Chosen {
        who: PlayerSelector,
        what: FilterObject,
    },
    Random {
        who: PlayerSelector,
        what: FilterObject<DrawAmount>,
        random: bool,
    },
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct SourceToken {
    pub(crate) colors: Vec<String>,
    pub(crate) supertypes: Vec<String>,
    pub(crate) types: Vec<String>,
    pub(crate) subtypes: Vec<String>,
    pub(crate) keywords: Vec<serde_json::Value>,
    pub(crate) power: Option<serde_json::Value>,
    pub(crate) toughness: Option<serde_json::Value>,
    /// Printed modifiers the token lowerer reads or rejects by name.
    #[serde(default, deserialize_with = "present_json_operand")]
    pub(crate) name: Option<serde_json::Value>,
    #[serde(default, deserialize_with = "present_json_operand")]
    pub(crate) abilities: Option<serde_json::Value>,
    #[serde(default, deserialize_with = "present_bool_field")]
    pub(crate) tapped: Option<bool>,
    #[serde(default, deserialize_with = "present_bool_field")]
    pub(crate) attacking: Option<bool>,
    #[serde(
        default,
        rename = "attackingDefender",
        deserialize_with = "present_json_operand"
    )]
    pub(crate) attacking_defender: Option<serde_json::Value>,
    #[serde(default, deserialize_with = "present_json_operand")]
    pub(crate) blocking: Option<serde_json::Value>,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(tag = "restriction", rename_all = "kebab-case", deny_unknown_fields)]
pub(crate) enum SourceActivationRestriction {
    OnceEachTurn {},
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct SourceCounter {
    pub(crate) r#type: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields, tag = "cost", rename_all = "kebab-case")]
pub(crate) enum SourceCost {
    Discard {
        what: serde_json::Value,
        random: Option<bool>,
    },
    TapObjects {
        what: serde_json::Value,
    },
    Exile {
        what: serde_json::Value,
        from: Option<serde_json::Value>,
    },
    RemoveCounters {
        counter: SourceCounter,
        count: Count,
        from: serde_json::Value,
    },
    Sacrifice {
        what: serde_json::Value,
    },
    TapSelf {},
    PayLife {
        amount: serde_json::Value,
    },
    Mana {
        symbols: Vec<String>,
    },
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct PlayerSelector {
    pub(crate) player: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) selection_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) of: Option<serde_json::Value>,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct FilterObject<A = Count, F = ObjectFilter> {
    #[serde(rename = "ref")]
    pub(crate) reference: String,
    pub(crate) filter: F,
    pub(crate) count: A,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(untagged)]
pub(crate) enum ObjectFilter {
    Types(Filter),
    Class(CardFilter),
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct CardFilter {
    pub(crate) cls: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct CounterTarget {
    #[serde(rename = "ref")]
    pub(crate) reference: String,
    pub(crate) filter: CounterFilter,
    pub(crate) count: Count,
    pub(crate) selection_id: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(tag = "cls", rename_all = "snake_case", deny_unknown_fields)]
pub(crate) enum CounterFilter {
    Spell {
        #[serde(default, rename = "allOf", deserialize_with = "present_ast_field")]
        all_of: Option<Vec<ast::ObjectFilter>>,
        #[serde(default, deserialize_with = "present_ast_field")]
        types: Option<Vec<String>>,
        #[serde(default, rename = "nonTypes", deserialize_with = "present_ast_field")]
        non_types: Option<Vec<String>>,
        #[serde(default, deserialize_with = "present_ast_field")]
        properties: Option<Vec<ast::PropertyConstraint>>,
    },
    Ability {
        #[serde(rename = "anyOf")]
        any_of: Vec<AbilityKindFilter>,
    },
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct AbilityKindFilter {
    pub(crate) ability_kind: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, serde::Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Filter {
    pub(crate) types: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, serde::Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Count {
    pub(crate) amount: String,
    pub(crate) value: u32,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(tag = "amount", rename_all = "kebab-case", deny_unknown_fields)]
pub(crate) enum DrawAmount {
    X {},
    Fixed { value: u32 },
    ThatMuch {},
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, serde::Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct SourceDuration {
    pub(crate) duration: String,
}

// Unlike diagnostic binding.references, these are executable typed operands.
// No selector, qualifier, sentence binding or duration may be discarded.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct SourceControlPlayer {
    pub(crate) player: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(tag = "mana", rename_all = "kebab-case", deny_unknown_fields)]
pub(crate) enum SourceMana {
    ChosenColor {
        amount: serde_json::Value,
        choice: serde_json::Value,
    },
    Choice {
        options: Vec<SourceMana>,
    },
    Fixed {
        symbols: Vec<Color>,
    },
    AnyColor {
        amount: Count,
    },
    Combination {
        amount: serde_json::Value,
        options: serde_json::Value,
    },
    AnyOneColor {
        amount: serde_json::Value,
    },
    CommanderColor {
        amount: Count,
        whose: PlayerSelector,
    },
    CouldProduce {
        amount: Count,
        what: serde_json::Value,
        #[serde(rename = "colorOnly")]
        color_only: bool,
    },
    EachColorAmong {
        among: serde_json::Value,
    },
}

// Optional AST flags may be omitted, but a present value must be a boolean.
pub(crate) fn present_bool_field<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> Result<Option<bool>, D::Error> {
    bool::deserialize(deserializer).map(Some)
}
