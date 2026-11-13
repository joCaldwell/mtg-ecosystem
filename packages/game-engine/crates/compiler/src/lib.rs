//! Offline archive-3 adapter and the first complete-card lowering subset.
use mtg_contracts::*;
use serde::Deserialize;
use serde_json::value::RawValue;
use std::collections::{BTreeMap, BTreeSet};

pub mod fixtures {
    include!(concat!(env!("OUT_DIR"), "/fixtures.rs"));
}
pub mod catalog;
pub mod coverage;
mod diagnostics;
mod normalization;
mod self_reference;
mod type_line;
pub use catalog::*;
use normalization::source_lines;

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(tag = "status", rename_all = "snake_case", deny_unknown_fields)]
pub enum CompileResult {
    Lowered { definition: Box<Definition> },
    Unsupported { path: String, reason: String },
    Invalid { path: String, reason: String },
}
#[derive(Debug)]
struct Failure {
    unsupported: bool,
    path: String,
    reason: String,
}
fn invalid(path: &str, reason: impl Into<String>) -> Failure {
    Failure {
        unsupported: false,
        path: path.into(),
        reason: reason.into(),
    }
}
fn unsupported(path: &str, reason: impl Into<String>) -> Failure {
    Failure {
        unsupported: true,
        path: path.into(),
        reason: reason.into(),
    }
}
fn decode<T: serde::de::DeserializeOwned>(s: &str, path: &str) -> Result<T, Failure> {
    strict_read(s).map_err(|e| invalid(path, e))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Record {
    record_type: String,
    source_index: u32,
    oracle_identity: String,
    source_sha256: String,
    source: Box<RawValue>,
    status: String,
    faces: Vec<Face>,
    ast_sha256: Option<String>,
    binding: Box<RawValue>,
    binding_sha256: String,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Face {
    source_face_index: u32,
    status: String,
    result: Box<RawValue>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ParseResult {
    name: String,
    ok: bool,
    lines: Vec<SourceLine>,
    abilities: Box<RawValue>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SourceLine {
    text: String,
    ok: bool,
    ability: Box<RawValue>,
}

// Unconsumed Scryfall metadata stays in the original RawValue. Additional
// metadata is allowed; every consumed field is typed and validated. Field
// meanings: https://scryfall.com/docs/api/cards
#[derive(Deserialize)]
struct Source {
    id: String,
    oracle_id: String,
    name: String,
    layout: String,
    oracle_text: String,
    mana_cost: String,
    type_line: String,
    colors: Vec<Color>,
    color_identity: Vec<Color>,
    power: Option<String>,
    toughness: Option<String>,
    card_faces: Option<Box<RawValue>>,
    loyalty: Option<Box<RawValue>>,
    defense: Option<Box<RawValue>>,
}
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum Ability {
    #[serde(rename = "additional-cost")]
    AdditionalCost {
        #[serde(rename = "abilityWord")]
        ability_word: Option<String>,
        costs: Vec<SourceCost>,
    },
    Keywords {
        #[serde(rename = "abilityWord")]
        ability_word: Option<String>,
        keywords: Vec<SourceKeyword>,
    },
    Static {
        #[serde(rename = "abilityWord")]
        ability_word: Option<String>,
        effect: SourceStaticEffect,
    },
    Triggered {
        #[serde(rename = "abilityWord")]
        ability_word: Option<String>,
        condition: Option<SourceCondition>,
        trigger: Box<SourceTrigger>,
        effects: Vec<Sentence>,
    },
    Spell {
        #[serde(rename = "abilityWord")]
        ability_word: Option<String>,
        effects: Vec<Sentence>,
    },
    Activated {
        #[serde(rename = "abilityWord")]
        ability_word: Option<String>,
        costs: Vec<SourceCost>,
        effects: Vec<Sentence>,
    },
}
#[derive(Clone, Debug, PartialEq, Eq)]
struct SourceKeyword {
    keyword: String,
    cost: Option<Vec<SourceCost>>,
    filter: Option<serde_json::Value>,
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
        if !matches!(keyword.as_str(), "equip" | "enchant")
            && serde_json::from_value::<KeywordAbility>(serde_json::Value::String(keyword.clone()))
                .is_err()
        {
            return Err(D::Error::custom(format!("keyword:{keyword}")));
        }
        let allowed = match keyword.as_str() {
            "equip" => ["keyword", "cost"].as_slice(),
            "enchant" => ["keyword", "filter"].as_slice(),
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
        Ok(Self {
            keyword,
            cost,
            filter,
        })
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
struct ModeCount {
    exactly: u32,
}
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
struct ModeOption {
    effects: Vec<Sentence>,
}
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
#[serde(tag = "condition", rename_all = "kebab-case")]
enum SourceCondition {
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
struct SourceComparison {
    op: String,
    value: Count,
}
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(tag = "trigger", rename_all = "kebab-case", deny_unknown_fields)]
enum SourceTrigger {
    DealsDamage {
        what: SourceDamageObject,
        #[serde(default, deserialize_with = "damage_combat_qualifier")]
        combat: Option<bool>,
        to: serde_json::Value,
        #[serde(default, deserialize_with = "present_damage_filter_field")]
        grouping: Option<String>,
    },
    Attacks {
        what: SourceAttacker,
    },
    Cast {
        who: PlayerSelector,
        what: SourceTypeFilter,
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
enum SourceDamageObject {
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
struct SourceDamageFilter {
    types: Vec<String>,
    #[serde(
        default,
        rename = "nonTypes",
        deserialize_with = "present_damage_filter_field"
    )]
    non_types: Option<Vec<String>>,
    #[serde(
        default,
        rename = "allOf",
        deserialize_with = "present_damage_filter_field"
    )]
    all_of: Option<Vec<SourceTypeFilter>>,
    #[serde(
        default,
        rename = "anyOf",
        deserialize_with = "present_damage_filter_field"
    )]
    any_of: Option<Vec<SourceTypeFilter>>,
    #[serde(default, deserialize_with = "present_damage_filter_field")]
    control: Option<String>,
    #[serde(default, deserialize_with = "present_damage_filter_field")]
    properties: Option<Vec<serde_json::Value>>,
}
fn present_damage_filter_field<'de, D, T>(deserializer: D) -> Result<Option<T>, D::Error>
where
    D: serde::Deserializer<'de>,
    T: Deserialize<'de>,
{
    T::deserialize(deserializer).map(Some)
}
fn lower_damage_subject(what: &SourceDamageObject) -> Result<DamageTriggerSource, Failure> {
    match what {
        SourceDamageObject::This { noun } if noun == "creature" => {
            Ok(DamageTriggerSource::Source {})
        }
        SourceDamageObject::Filter { filter, count }
            if fixed_one(count)
                && filter.types.iter().any(|t| t == "creature")
                && filter.properties.is_none() =>
        {
            let controller = match filter.control.as_deref() {
                Some("you") => DamageSourceRelation::Controller,
                _ => {
                    return Err(unsupported(
                        "trigger.what.filter.control",
                        "unsupported damage source controller",
                    ));
                }
            };
            Ok(DamageTriggerSource::CreatureFilter {
                controller,
                types: lower_type_predicate(
                    &SourceTypeFilter {
                        cls: None,
                        types: Some(filter.types.clone()),
                        non_types: filter.non_types.clone(),
                        all_of: filter.all_of.clone(),
                        any_of: filter.any_of.clone(),
                    },
                    None,
                    true,
                    0,
                )?,
            })
        }
        _ => Err(unsupported(
            "trigger.what",
            "unsupported complete creature damage source/cardinality",
        )),
    }
}
// ObjectFilter semantics: fields/arrays conjunctive, explicit anyOf disjunctive.
// packages/oracle-parser/src/ast.ts ObjectFilter; CR 205.2b, 601.2i.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
struct SourceTypeFilter {
    cls: Option<String>,
    types: Option<Vec<String>>,
    #[serde(rename = "nonTypes")]
    non_types: Option<Vec<String>>,
    #[serde(rename = "allOf")]
    all_of: Option<Vec<SourceTypeFilter>>,
    #[serde(rename = "anyOf")]
    any_of: Option<Vec<SourceTypeFilter>>,
}
fn lower_type_predicate(
    f: &SourceTypeFilter,
    class: Option<&str>,
    root: bool,
    depth: usize,
) -> Result<TypePredicate, Failure> {
    if depth > 8
        || (root && f.cls.as_deref() != class)
        || (f.cls.is_some() && f.cls.as_deref() != class)
    {
        return Err(unsupported("trigger.what", "unsupported spell class/depth"));
    }
    let kind = |s: &str| match s {
        "creature" => Ok(CardKind::Creature),
        "artifact" => Ok(CardKind::Artifact),
        "enchantment" => Ok(CardKind::Enchantment),
        "instant" => Ok(CardKind::Instant),
        "sorcery" => Ok(CardKind::Sorcery),
        "land" => Ok(CardKind::Land),
        _ => Err(unsupported("trigger.what.types", "unsupported spell type")),
    };
    let mut terms = vec![];
    for (values, negative) in [(&f.types, false), (&f.non_types, true)] {
        if let Some(values) = values {
            if values.is_empty() {
                return Err(unsupported("trigger.what", "empty type predicate"));
            }
            for t in values {
                let kind = kind(t)?;
                terms.push(if negative {
                    TypePredicate::NotType { kind }
                } else {
                    TypePredicate::Type { kind }
                });
            }
        }
    }
    for (children, disjunction) in [(&f.all_of, false), (&f.any_of, true)] {
        if let Some(children) = children {
            if children.is_empty() {
                return Err(unsupported("trigger.what", "empty boolean predicate"));
            }
            let children = children
                .iter()
                .map(|c| lower_type_predicate(c, class, false, depth + 1))
                .collect::<Result<Vec<_>, _>>()?;
            terms.push(if disjunction {
                TypePredicate::AnyOf { terms: children }
            } else {
                TypePredicate::All { terms: children }
            });
        }
    }
    let p = match terms.len() {
        0 => TypePredicate::Any {},
        1 => terms.remove(0),
        _ => TypePredicate::All { terms },
    };
    p.validate().map_err(|e| unsupported("trigger.what", &e))?;
    Ok(p)
}
// Separate attack subject mirror: no death-only fields, status, ownership,
// grouping or qualifiers can silently disappear. A filter's arrays are AND.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(tag = "ref", rename_all = "kebab-case", deny_unknown_fields)]
enum SourceAttacker {
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
struct SourceAttackerFilter {
    types: Option<Vec<String>>,
    #[serde(rename = "nonTypes")]
    non_types: Option<Vec<String>>,
    #[serde(rename = "allOf")]
    all_of: Option<Vec<SourceTypeFilter>>,
    #[serde(rename = "anyOf")]
    any_of: Option<Vec<SourceTypeFilter>>,
    control: Option<String>,
}
fn lower_attack_subject(what: &SourceAttacker) -> Result<AttackSubject, Failure> {
    match what {
        SourceAttacker::This { noun } if noun == "creature" => Ok(AttackSubject::Source {}),
        SourceAttacker::Filter { filter, count } if fixed_one(count) => {
            let controller = match filter.control.as_deref() {
                None => AttackerRelation::AnyPlayer,
                Some("you") => AttackerRelation::Controller,
                Some("opponent") => AttackerRelation::Opponent,
                _ => {
                    return Err(unsupported(
                        "trigger.what.filter.control",
                        "unsupported attacker controller",
                    ));
                }
            };
            Ok(AttackSubject::Filter {
                controller,
                types: lower_type_predicate(
                    &SourceTypeFilter {
                        cls: None,
                        types: filter.types.clone(),
                        non_types: filter.non_types.clone(),
                        all_of: filter.all_of.clone(),
                        any_of: filter.any_of.clone(),
                    },
                    None,
                    true,
                    0,
                )?,
            })
        }
        _ => Err(unsupported(
            "trigger.what",
            "unsupported complete attacker subject/cardinality",
        )),
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(tag = "ref", rename_all = "kebab-case", deny_unknown_fields)]
enum TriggerObject {
    This { noun: String },
    Filter { filter: TriggerFilter, count: Count },
}
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
struct TriggerFilter {
    #[serde(default)]
    other: bool,
    types: Vec<String>,
    control: Option<String>,
    #[serde(default, rename = "nonToken")]
    non_token: bool,
    #[serde(default, rename = "nonSubtypes")]
    non_subtypes: Vec<String>,
}
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
struct Sentence {
    #[serde(default)]
    bindings: Vec<serde_json::Value>,
    steps: Vec<Step>,
}
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
#[serde(tag = "effect", rename_all = "kebab-case")]
enum Step {
    CreateReplacement {
        replacement: serde_json::Value,
        duration: SourceDuration,
    },
    AddMana {
        who: PlayerSelector,
        mana: SourceMana,
    },
    Exile {
        what: serde_json::Value,
    },
    Destroy {
        what: serde_json::Value,
        optional: Option<bool>,
        #[serde(rename = "regenerationForbidden")]
        regeneration_forbidden: Option<bool>,
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
    },
    CreateToken {
        who: Option<serde_json::Value>,
        count: Count,
        token: SourceToken,
    },
    PutCounters {
        on: SourceCounterRecipient,
        counter: SourceCounter,
        count: Count,
    },
    Modal {
        count: ModeCount,
        options: Vec<ModeOption>,
    },
    Pump {
        what: serde_json::Value,
        power: SignedAmount,
        toughness: SignedAmount,
        duration: SourceDuration,
    },
    PreventDamage {
        prevention: SourcePrevention,
        duration: SourceDuration,
    },
    Counter {
        what: CounterTarget,
    },
    Damage {
        source: DamageSource,
        amount: serde_json::Value,
        to: DamageDestination,
        optional: Option<bool>,
    },
    GainLife {
        who: PlayerSelector,
        amount: serde_json::Value,
        #[serde(rename = "forEach")]
        for_each: Option<serde_json::Value>,
    },
    LoseLife {
        who: PlayerSelector,
        amount: serde_json::Value,
    },
    ReturnToHand {
        what: serde_json::Value,
        to: String,
        optional: Option<bool>,
    },
    MoveZone {
        what: serde_json::Value,
        to: serde_json::Value,
        tapped: Option<bool>,
    },
    Sacrifice {
        who: PlayerSelector,
        what: FilterObject,
        #[serde(rename = "chosenBy")]
        chosen_by: PlayerSelector,
    },
    Draw {
        who: PlayerSelector,
        amount: DrawAmount,
        #[serde(default)]
        optional: bool,
    },
    Discard {
        who: PlayerSelector,
        what: FilterObject,
    },
}
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
struct SourceToken {
    colors: Vec<String>,
    supertypes: Vec<String>,
    types: Vec<String>,
    subtypes: Vec<String>,
    keywords: Vec<serde_json::Value>,
    power: Count,
    toughness: Count,
}
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(untagged)]
enum SourceCounterRecipient {
    Target(Target),
    Source(ThisObject),
}
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
struct SourceCounter {
    r#type: String,
}
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields, tag = "cost", rename_all = "kebab-case")]
enum SourceCost {
    Sacrifice { what: serde_json::Value },
    TapSelf {},
    Mana { symbols: Vec<String> },
}
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(tag = "ref", rename_all = "kebab-case", deny_unknown_fields)]
enum DamageSource {
    It {},
    #[serde(rename = "self")]
    SelfReference {},
    This {
        noun: String,
    },
}
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(tag = "to", rename_all = "kebab-case", deny_unknown_fields)]
enum DamageDestination {
    Object {
        #[serde(rename = "ref")]
        reference: serde_json::Value,
    },
    Each {
        refs: Vec<serde_json::Value>,
    },
    Player {
        #[serde(rename = "ref")]
        reference: serde_json::Value,
    },
    AnyTarget {
        #[serde(rename = "selectionId")]
        selection_id: String,
    },
}
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
struct ThisObject {
    #[serde(rename = "ref")]
    reference: String,
    noun: String,
}
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct PlayerSelector {
    player: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    selection_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    of: Option<serde_json::Value>,
}
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
struct FilterObject {
    #[serde(rename = "ref")]
    reference: String,
    filter: ObjectFilter,
    count: Count,
}
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(untagged)]
enum ObjectFilter {
    Types(Filter),
    Class(CardFilter),
}
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
struct CardFilter {
    cls: String,
}
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Target {
    #[serde(rename = "ref")]
    reference: String,
    filter: Filter,
    count: Count,
    selection_id: String,
}
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct CounterTarget {
    #[serde(rename = "ref")]
    reference: String,
    filter: CounterFilter,
    count: Count,
    selection_id: String,
}
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(tag = "cls", rename_all = "snake_case", deny_unknown_fields)]
enum CounterFilter {
    Spell {
        #[serde(rename = "allOf")]
        all_of: Option<Vec<serde_json::Value>>,
    },
    Ability {
        #[serde(rename = "anyOf")]
        any_of: Vec<AbilityKindFilter>,
    },
}
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct AbilityKindFilter {
    ability_kind: String,
}
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
struct Filter {
    types: Vec<String>,
}
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
struct Count {
    amount: String,
    value: u32,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(tag = "amount", rename_all = "kebab-case", deny_unknown_fields)]
enum DrawAmount {
    Fixed { value: u32 },
    ThatMuch {},
}
impl DrawAmount {
    fn fixed_one(&self) -> bool {
        matches!(self, Self::Fixed { value: 1 })
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
struct SourceDuration {
    duration: String,
}
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
struct SignedAmount {
    sign: i8,
    amount: Count,
}
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct SourcePrevention {
    event_scope: SourceEventScope,
    shield: SourceShield,
    kind: String,
    to: Option<Vec<AnyTargetReference>>,
}
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
struct SourceEventScope {
    sources: String,
    recipients: String,
}
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(tag = "shield", rename_all = "kebab-case", deny_unknown_fields)]
enum SourceShield {
    Events {
        occurrence: String,
        prevent: SourcePrevent,
    },
    DamageBudget {
        amount: Count,
    },
}
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(tag = "prevent", rename_all = "kebab-case", deny_unknown_fields)]
enum SourcePrevent {
    All {},
}
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(tag = "ref", rename_all = "kebab-case", deny_unknown_fields)]
enum AnyTargetReference {
    AnyTarget {
        #[serde(rename = "selectionId")]
        selection_id: String,
    },
}
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(tag = "static", rename_all = "kebab-case", deny_unknown_fields)]
enum SourceStaticEffect {
    Prohibit {
        prohibition: serde_json::Value,
    },
    EntersTapped {
        what: serde_json::Value,
        #[serde(rename = "conditionKind")]
        condition_kind: Option<serde_json::Value>,
        condition: Option<serde_json::Value>,
    },
    CostModification {
        scope: serde_json::Value,
        change: serde_json::Value,
        multiplier: serde_json::Value,
    },
    Cant {
        what: serde_json::Value,
        action: String,
    },
    Modify {
        what: serde_json::Value,
        modifiers: Vec<serde_json::Value>,
    },
    Replacement {
        replacement: SourceReplacement,
    },
}
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
struct SourceReplacement {
    event: SourceDamageEvent,
    occurrence: String,
    replace: SourceReplacementAction,
}
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(tag = "event", rename_all = "kebab-case", deny_unknown_fields)]
enum SourceDamageEvent {
    Damage {
        kind: String,
        source: ReplacementDamageSource,
        to: Vec<ReplacementRecipient>,
    },
}
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
struct ReplacementDamageSource {
    #[serde(rename = "ref")]
    reference: String,
    filter: serde_json::Value,
    count: Count,
}
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(untagged)]
enum ReplacementRecipient {
    Permanent(FilterObject),
    Player(PlayerSelector),
}
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(tag = "replacement", rename_all = "kebab-case", deny_unknown_fields)]
enum SourceReplacementAction {
    ModifyQuantity { change: SourceQuantityChange },
}
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
struct SourceQuantityChange {
    operation: String,
    factor: u32,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(tag = "mana", rename_all = "kebab-case", deny_unknown_fields)]
enum SourceMana {
    Fixed {
        symbols: Vec<Color>,
    },
    AnyColor {
        amount: Count,
    },
    CommanderColor {
        amount: Count,
        whose: PlayerSelector,
    },
}
fn mana_cost(text: &str) -> Result<ManaCost, Failure> {
    let mut cost = ManaCost {
        generic: Quantity::zero(),
        colored: vec![],
    };
    let mut rest = text;
    while !rest.is_empty() {
        let token = rest
            .strip_prefix('{')
            .and_then(|s| s.split_once('}'))
            .ok_or_else(|| invalid("source.mana_cost", "malformed mana symbol"))?;
        match token.0 {
            "W" => cost.colored.push(Color::W),
            "U" => cost.colored.push(Color::U),
            "B" => cost.colored.push(Color::B),
            "R" => cost.colored.push(Color::R),
            "G" => cost.colored.push(Color::G),
            "C" => cost.colored.push(Color::C),
            digits if !digits.is_empty() && digits.bytes().all(|b| b.is_ascii_digit()) => {
                let q: Quantity = digits.parse().map_err(|e| invalid("source.mana_cost", e))?;
                cost.generic.add(&q);
            }
            _ => {
                return Err(unsupported(
                    "source.mana_cost",
                    "variable/hybrid/Phyrexian/special mana is not supported",
                ));
            }
        }
        rest = token.1;
    }
    Ok(cost)
}

fn read_abilities(raw: &str) -> Result<Vec<Ability>, Failure> {
    // Keep the duplicate-key pass before serde's internally tagged mirrors.
    // serde_path_to_error reports context without accepting unknown fields.
    let nodes: Vec<Box<RawValue>> = decode(raw, "abilities")?;
    nodes
        .iter()
        .enumerate()
        .map(|(i, node)| {
            serde_json::from_str::<Ability>(node.get())
                .map_err(|_| diagnostics::ability(node.get(), &format!("abilities[{i}]")))
        })
        .collect()
}

pub fn compile(manifest: &str, record: &str) -> CompileResult {
    match lower(manifest, record) {
        Ok(definition) => CompileResult::Lowered {
            definition: Box::new(definition),
        },
        Err(e) if e.unsupported => CompileResult::Unsupported {
            path: e.path,
            reason: e.reason,
        },
        Err(e) => CompileResult::Invalid {
            path: e.path,
            reason: e.reason,
        },
    }
}
fn lower(manifest: &str, record: &str) -> Result<Definition, Failure> {
    let archive = accepted_manifest(manifest).map_err(|e| invalid("manifest", e))?;
    let r: Record = decode(record, "record")?;
    if r.record_type != "card" {
        return Err(invalid("recordType", "expected card record"));
    }
    if r.status != "parsed" {
        return Err(unsupported("status", "complete-card parsing is required"));
    }
    // Archive hashes use JS JSON.stringify. RawValue keeps each exact compact
    // subvalue rather than reordering keys or changing Unicode/number spellings.
    // https://docs.rs/serde_json/latest/serde_json/value/struct.RawValue.html
    if sha256(r.source.get().as_bytes()) != r.source_sha256
        || sha256(r.binding.get().as_bytes()) != r.binding_sha256
    {
        return Err(invalid(
            "record",
            "source or diagnostic binding hash mismatch",
        ));
    }
    // Multi-face Scryfall records intentionally lack single-face oracle_text,
    // mana_cost, etc. Classify the layout before decoding that adapter. Hashes
    // above still bind every source byte; no default fills missing fields.
    let shape: serde_json::Value = decode(r.source.get(), "source")?;
    if shape.get("card_faces").is_some_and(|v| !v.is_null()) {
        let layout = shape["layout"]
            .as_str()
            .ok_or_else(|| invalid("source.layout", "missing layout"))?;
        return Err(unsupported(
            "source.layout",
            format!("multi_face_layout:{layout}"),
        ));
    }
    let source: Source = decode(r.source.get(), "source")?;
    if source.oracle_id != r.oracle_identity || source.id.is_empty() || source.name.is_empty() {
        return Err(invalid("source", "missing or mismatched card identity"));
    }
    if source.loyalty.is_some() {
        return Err(unsupported("source.type_line.card_type", "Planeswalker"));
    }
    if source.defense.is_some() {
        return Err(unsupported("source.type_line.card_type", "Battle"));
    }
    if source.layout != "normal"
        || source.card_faces.is_some()
        || source.loyalty.is_some()
        || source.defense.is_some()
    {
        return Err(unsupported(
            "source.layout",
            "only complete, single-face normal cards in the initial subset are supported",
        ));
    }
    if r.faces.len() != 1 || r.faces[0].source_face_index != 0 || r.faces[0].status != "attempted" {
        return Err(invalid("faces", "missing or duplicated source face"));
    }
    let result: ParseResult = decode(r.faces[0].result.get(), "faces[0].result")?;
    if !result.ok || result.name != source.name || result.lines.iter().any(|l| !l.ok) {
        return Err(invalid("faces[0].result", "incomplete or mismatched parse"));
    }
    let ast_sha256 = r
        .ast_sha256
        .ok_or_else(|| invalid("astSha256", "missing successful AST hash"))?;
    if sha256(format!("[{}]", result.abilities.get()).as_bytes()) != ast_sha256 {
        return Err(invalid("astSha256", "successful AST hash mismatch"));
    }
    // CR 201.5: canonicalize only exact archive-bound references to this source.
    // Source and AST integrity checks above still cover the original bytes.
    let types = type_line::parse(&source.type_line)?;
    let noun = match types.kind {
        CardKind::Creature => "creature",
        CardKind::Artifact => "artifact",
        CardKind::Enchantment if types.subtypes == ["Aura"] => "aura",
        CardKind::Enchantment => "enchantment",
        CardKind::Land => "land",
        CardKind::Instant | CardKind::Sorcery => "spell",
    };
    let binding = decode(r.binding.get(), "binding")?;
    let canonical_abilities = self_reference::canonicalize(
        result.abilities.get(),
        &binding,
        &r.oracle_identity,
        noun,
        "/faces/0/abilities",
    )?;
    let abilities: Vec<Ability> = read_abilities(&canonical_abilities)?;
    // CR 207.2c: labels carry no rules meaning. All underlying fields must
    // still decode and lower, including conditions, costs and player relations.
    // https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
    for ability in &abilities {
        let (Ability::AdditionalCost { ability_word, .. }
        | Ability::Keywords { ability_word, .. }
        | Ability::Static { ability_word, .. }
        | Ability::Spell { ability_word, .. }
        | Ability::Activated { ability_word, .. }
        | Ability::Triggered { ability_word, .. }) = ability;
        if ability_word.as_ref().is_some_and(|s| s.trim().is_empty()) {
            return Err(unsupported("abilities.abilityWord", "empty annotation"));
        }
    }
    // Only understood whole-card ASTs reach source normalization. Unsupported
    // nodes are rejected first; every candidate for admission still proves the
    // exact complete normalized source and line/aggregate mirror below.
    if source_lines(&source.oracle_text, &source.name, &source.type_line)?
        != result
            .lines
            .iter()
            .map(|l| normalization::literal_spelling(&l.text))
            .collect::<Vec<_>>()
    {
        return Err(invalid(
            "lines",
            "source contains unaccounted or mismatched normalized blocks",
        ));
    }
    if result.lines.len() != abilities.len() {
        return Err(invalid("lines", "unaccounted ability block"));
    }
    let original: Vec<Box<RawValue>> = decode(result.abilities.get(), "abilities")?;
    for (line, ability) in result.lines.iter().zip(&original) {
        let a: serde_json::Value = decode(line.ability.get(), "lines.ability")?;
        let b: serde_json::Value = decode(ability.get(), "abilities")?;
        if a != b || line.text.is_empty() {
            return Err(invalid("lines", "line and aggregate ability disagree"));
        }
    }
    let kind = types.kind;
    let basic = types.basic;
    let power = source
        .power
        .map(|s| s.parse::<Quantity>())
        .transpose()
        .map_err(|e| unsupported("source.power", e))?;
    let toughness = source
        .toughness
        .map(|s| s.parse::<Quantity>())
        .transpose()
        .map_err(|e| unsupported("source.toughness", e))?;
    if source.mana_cost.matches("{X}").count() > 1
        || (source.mana_cost.contains("{X}")
            && !matches!(kind, CardKind::Instant | CardKind::Sorcery))
    {
        return Err(unsupported("mana_cost", "unsupported X cost"));
    }
    let mut definition = Definition {
        x_mana_cost: source.mana_cost.contains("{X}"),
        oracle_id: r.oracle_identity,
        name: source.name,
        characteristics: Characteristics {
            kind,
            additional_types: types.additional_types,
            basic,
            legendary: types.legendary,
            snow: types.snow,
            subtypes: types.subtypes,
            mana_cost: mana_cost(&source.mana_cost.replace("{X}", ""))?,
            colors: source.colors,
            color_identity: source.color_identity,
            power,
            toughness,
        },
        intrinsic_zone: if kind == CardKind::Land {
            Some(FunctioningZone::Battlefield)
        } else {
            None
        },
        spell: None,
        activated: vec![],
        mana_abilities: vec![],
        triggered: vec![],
        static_abilities: vec![],
        keywords: vec![],
        capabilities: BTreeSet::new(),
        provenance: SourceProvenance {
            printing_id: source.id,
            source_index: r.source_index,
            source_sha256: r.source_sha256,
            ast_sha256,
            binding_sha256: r.binding_sha256,
            archive_manifest_sha256: archive.manifest_sha256.clone(),
            parser_sha256: archive.parser_source_sha256.clone(),
            source_json: r.source.get().into(),
            abilities_json: result.abilities.get().into(),
        },
    };
    match kind {
        CardKind::Land | CardKind::Creature | CardKind::Artifact | CardKind::Enchantment => {
            if kind == CardKind::Land && basic && !abilities.is_empty() {
                return Err(unsupported(
                    "abilities",
                    "nonempty land abilities are not supported",
                ));
            }
            if kind == CardKind::Creature && abilities.is_empty() && !source.oracle_text.is_empty()
            {
                return Err(invalid(
                    "source.oracle_text",
                    "empty AST for nonempty creature behavior",
                ));
            }
            if kind != CardKind::Land {
                // CR 302.1–2 / 608.3a: casting a creature and activating one of
                // its battlefield abilities are distinct executable programs.
                // https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
                definition.spell = Some(SpellProgram {
                    additional_sacrifice: None,
                    timing: SpellTiming::MainPhase,
                    resolution: SpellResolution::Permanent,
                    functioning_zone: FunctioningZone::Stack,
                    targets: vec![],
                    instructions: vec![],
                    modes: vec![],
                });
            }
            for (block, ability) in abilities.iter().enumerate() {
                match ability {
                    Ability::Static { effect, .. } => {
                        definition.static_abilities.push(lower_static(effect)?)
                    }
                    Ability::Keywords { keywords, .. } => {
                        if keywords.is_empty() {
                            return Err(unsupported("abilities.keywords", "empty keyword block"));
                        }
                        for keyword in keywords {
                            if keyword.keyword == "equip"
                                && kind == CardKind::Artifact
                                && definition.characteristics.subtypes == ["Equipment"]
                                && keyword.filter.is_none()
                            {
                                let costs = keyword
                                    .cost
                                    .as_ref()
                                    .ok_or_else(|| unsupported("equip", "missing equip cost"))?;
                                let c = lower_permanent_costs(costs, kind)?;
                                if c.tap || c.sacrifice || c.mana.is_none() {
                                    return Err(unsupported(
                                        "equip",
                                        "equip requires fixed mana only",
                                    ));
                                }
                                definition.activated.push(ActivatedAbility {
                                    functioning_zone: FunctioningZone::Battlefield,
                                    cost: ActivationCost::Mana {
                                        cost: c.mana.unwrap(),
                                    },
                                    program: SpellProgram {
                                        additional_sacrifice: None,
                                        timing: SpellTiming::MainPhase,
                                        resolution: SpellResolution::Instructions,
                                        functioning_zone: FunctioningZone::Stack,
                                        targets: vec![TargetSpec {
                                            slot: 0,
                                            selection_id: "equip-creature".into(),
                                            chooser: Chooser::Controller,
                                            stage: EvaluationStage::Announcement,
                                            filter: None,
                                            domain: TargetDomain::ControlledBattlefieldCreature,
                                        }],
                                        instructions: vec![Instruction::AttachEquipment {
                                            target_slot: 0,
                                            source_block: block as u32,
                                        }],
                                        modes: vec![],
                                    },
                                });
                                continue;
                            }
                            if keyword.keyword == "enchant"
                                && kind == CardKind::Enchantment
                                && definition.characteristics.subtypes == ["Aura"]
                                && keyword.cost.is_none()
                                && keyword.filter == Some(serde_json::json!({"types":["creature"]}))
                            {
                                let spell = definition.spell.as_mut().unwrap();
                                if !spell.targets.is_empty() {
                                    return Err(unsupported("enchant", "duplicate enchant"));
                                }
                                spell.targets.push(TargetSpec {
                                    slot: 0,
                                    selection_id: "enchanted-creature".into(),
                                    chooser: Chooser::Controller,
                                    stage: EvaluationStage::Announcement,
                                    filter: None,
                                    domain: TargetDomain::BattlefieldCreature,
                                });
                                continue;
                            }
                            if keyword.cost.is_some() || keyword.filter.is_some() {
                                return Err(unsupported(
                                    "keyword",
                                    format!("keyword:{}:parameters", keyword.keyword),
                                ));
                            }
                            match keyword.keyword.as_str() {
                                "flash" => definition.keywords.push(KeywordAbility::Flash),
                                "defender" => definition.keywords.push(KeywordAbility::Defender),
                                "trample" => definition.keywords.push(KeywordAbility::Trample),
                                "first strike" => {
                                    definition.keywords.push(KeywordAbility::FirstStrike)
                                }
                                "double strike" => {
                                    definition.keywords.push(KeywordAbility::DoubleStrike)
                                }
                                "menace" => definition.keywords.push(KeywordAbility::Menace),
                                "hexproof" => definition.keywords.push(KeywordAbility::Hexproof),
                                "shroud" => definition.keywords.push(KeywordAbility::Shroud),
                                "flying" => definition.keywords.push(KeywordAbility::Flying),
                                "reach" => definition.keywords.push(KeywordAbility::Reach),
                                "vigilance" => definition.keywords.push(KeywordAbility::Vigilance),
                                "haste" => definition.keywords.push(KeywordAbility::Haste),
                                "lifelink" => definition.keywords.push(KeywordAbility::Lifelink),
                                "deathtouch" => {
                                    definition.keywords.push(KeywordAbility::Deathtouch)
                                }
                                _ => {
                                    return Err(unsupported(
                                        "abilities.keywords",
                                        format!("keyword:{}", keyword.keyword),
                                    ));
                                }
                            }
                        }
                    }
                    Ability::Activated { costs, effects, .. } => {
                        if let Some(mana) = lower_mana_ability(costs, effects, block, kind)? {
                            definition.mana_abilities.push(mana);
                        } else {
                            definition
                                .activated
                                .push(lower_activation(costs, effects, block, kind)?);
                        }
                    }
                    Ability::Triggered {
                        trigger,
                        effects,
                        condition,
                        ability_word: _,
                    } => {
                        definition.triggered.push(lower_trigger(
                            trigger,
                            effects,
                            condition.as_ref(),
                            block,
                        )?);
                    }
                    _ => {
                        return Err(unsupported(
                            "abilities.kind",
                            "unsupported complete creature ability kind",
                        ));
                    }
                }
            }
        }
        CardKind::Instant | CardKind::Sorcery => {
            if source.oracle_text.is_empty() {
                return Err(invalid("source.oracle_text", "spell has no source text"));
            }
            let mut program = SpellProgram {
                additional_sacrifice: None,
                // CR 304.1 / 307.1: ordinary spell timing follows card type.
                // https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
                timing: if kind == CardKind::Instant {
                    SpellTiming::Instant
                } else {
                    SpellTiming::MainPhase
                },
                resolution: SpellResolution::Instructions,
                functioning_zone: FunctioningZone::Stack,
                targets: vec![],
                instructions: vec![],
                modes: vec![],
            };
            let mut slots = BTreeMap::new();
            for (block, ability) in abilities.iter().enumerate() {
                if let Ability::AdditionalCost { costs, .. } = ability {
                    if block != 0 || program.additional_sacrifice.is_some() {
                        return Err(unsupported(
                            "cost",
                            "duplicate or misplaced additional cost",
                        ));
                    }
                    let [SourceCost::Sacrifice { what }] = costs.as_slice() else {
                        return Err(unsupported("cost", "unsupported additional cost"));
                    };
                    program.additional_sacrifice = Some(lower_sacrifice_filter(what)?);
                    continue;
                }
                if let Ability::Static { effect, .. } = ability {
                    let a = lower_static(effect)?;
                    if a.functioning_zone != FunctioningZone::Stack {
                        return Err(unsupported("static", "spell static must function on stack"));
                    }
                    definition.static_abilities.push(a);
                    continue;
                }
                let Ability::Spell { effects, .. } = ability else {
                    return Err(unsupported(
                        "abilities.kind",
                        "nonspell ability on instant/sorcery",
                    ));
                };
                if effects.iter().flat_map(|s| &s.steps).any(|step| matches!(step,Step::Damage { source,.. } if !matches!(source,DamageSource::SelfReference {}))) {
                    return Err(unsupported("damage.source","a spell must refer to its own spell source"));
                }
                // Separate source blocks introduce independent occurrences (CR115.3).
                // Reusing a selection ID across blocks must not alias targets.
                slots.clear();
                append_spell_effects(
                    &mut program,
                    effects,
                    block,
                    &mut slots,
                    PlayerBinding::Unbound,
                )?;
            }
            definition.spell = Some(program);
        }
    }
    if definition.keywords.contains(&KeywordAbility::Flash) {
        definition
            .spell
            .as_mut()
            .ok_or_else(|| unsupported("keyword", "flash requires castable spell"))?
            .timing = SpellTiming::Instant;
    }
    if definition.triggered.iter().any(|a| {
        a.condition != TriggerCondition::OpponentCreatureDies
            && a.program
                .instructions
                .iter()
                .any(|i| matches!(i, Instruction::LoseLifeDeathController { .. }))
    }) {
        return Err(unsupported("trigger", "unbound death controller"));
    }
    definition.capabilities = definition.requirements();
    definition
        .validate()
        .map_err(|e| unsupported("definition", e))?;
    Ok(definition)
}

fn fixed_one(count: &Count) -> bool {
    count.amount == "fixed" && count.value == 1
}
fn source_block(block: usize) -> Result<u32, Failure> {
    u32::try_from(block).map_err(|_| invalid("abilities", "structural block limit"))
}
fn bind_target(
    program: &mut SpellProgram,
    slots: &mut BTreeMap<String, u32>,
    id: &str,
    domain: TargetDomain,
) -> Result<u32, Failure> {
    if id.is_empty() || program.targets.iter().any(|target| target.domain != domain) {
        return Err(unsupported(
            "targets",
            "empty selection or mixed creature/player target domains",
        ));
    }
    if let Some(slot) = slots.get(id) {
        return Ok(*slot);
    }
    let slot = u32::try_from(program.targets.len())
        .map_err(|_| invalid("targets", "structural slot limit"))?;
    program.targets.push(TargetSpec {
        slot,
        selection_id: id.into(),
        chooser: Chooser::Controller,
        stage: EvaluationStage::Announcement,
        domain,
        filter: None,
    });
    slots.insert(id.into(), slot);
    Ok(slot)
}
#[derive(Clone, Copy, PartialEq, Eq)]
enum PlayerBinding {
    Unbound,
    EventPlayer,
    DamageRecipient,
    DeathController,
}
fn append_spell_effects(
    program: &mut SpellProgram,
    effects: &[Sentence],
    block: usize,
    slots: &mut BTreeMap<String, u32>,
    player_binding: PlayerBinding,
) -> Result<(), Failure> {
    if let [Sentence { steps, .. }] = effects
        && let [Step::Modal { count, options }] = steps.as_slice()
    {
        if count.exactly != 1
            || options.len() < 2
            || !program.modes.is_empty()
            || !program.targets.is_empty()
            || !program.instructions.is_empty()
        {
            return Err(unsupported(
                "effects.modal",
                "only a complete standalone choose-one program is supported",
            ));
        }
        for option in options {
            let mut child = SpellProgram {
                additional_sacrifice: None,
                timing: program.timing,
                resolution: program.resolution,
                functioning_zone: program.functioning_zone,
                targets: vec![],
                instructions: vec![],
                modes: vec![],
            };
            append_spell_effects(
                &mut child,
                &option.effects,
                block,
                &mut BTreeMap::new(),
                player_binding,
            )?;
            if !child.modes.is_empty() {
                return Err(unsupported("effects.modal", "nested modes unsupported"));
            }
            program.modes.push(SpellMode { program: child });
        }
        return Ok(());
    }
    if !program.modes.is_empty() {
        return Err(unsupported(
            "effects.modal",
            "mixed modal and ordinary root instructions",
        ));
    }
    if effects.is_empty() {
        return Err(unsupported("abilities.effects", "empty spell effects"));
    }
    let source_block = source_block(block)?;
    for sentence in effects {
        let counted = if sentence.bindings.is_empty() {
            false
        } else if sentence.bindings
            == [
                serde_json::json!({"variable":"x","value":{"amount":"count","of":{"types":["creature"],"zone":{"zone":"battlefield"}}}}),
            ]
        {
            true
        } else {
            return Err(unsupported("bindings", "unsupported complete X binding"));
        };
        if counted
            && !matches!(sentence.steps.as_slice(),[Step::Damage {amount,..}] if *amount==serde_json::json!({"amount":"x"}))
        {
            return Err(unsupported("bindings", "unconsumed count binding"));
        }
        if let [
            Step::Search { who, zones, find },
            Step::MoveZone {
                what,
                to,
                tapped: Some(true),
            },
            Step::Shuffle {
                who: shuffler,
                library,
            },
        ] = sentence.steps.as_slice()
        {
            let maximum = find["count"]["value"].as_u64().unwrap_or(0);
            let up_to = maximum == 2;
            let expected = if up_to {
                serde_json::json!({"ref":"filter","filter":{"supertypes":["basic"],"types":["land"],"cls":"card"},"count":{"amount":"fixed","value":2},"upTo":true})
            } else {
                serde_json::json!({"ref":"filter","filter":{"supertypes":["basic"],"types":["land"],"cls":"card"},"count":{"amount":"fixed","value":1}})
            };
            // This instruction searches and shuffles only its controller's
            // library. Consume the complete actor, including object qualifiers.
            let you = PlayerSelector {
                player: "you".into(),
                selection_id: None,
                of: None,
            };
            if *who != you
                || shuffler != who
                || *zones
                    != serde_json::json!({"selection":"all","of":[{"zone":"library","owner":"your"}]})
                || *find != expected
                || !(1..=2).contains(&maximum)
                || *what
                    != if up_to {
                        serde_json::json!({"ref":"them"})
                    } else {
                        serde_json::json!({"ref":"that","filter":{"cls":"card"}})
                    }
                || *to != serde_json::json!({"zone":"battlefield"})
                || *library != serde_json::json!({"zone":"library","owner":{"player":"you"}})
            {
                return Err(unsupported(
                    "effects.search",
                    "unsupported complete search/move/shuffle sequence",
                ));
            }
            program.instructions.push(Instruction::SearchBasicLands {
                maximum: maximum as u32,
                source_block,
            });
            continue;
        }
        if sentence.steps.is_empty() {
            return Err(invalid("effects.steps", "empty instruction group"));
        }
        for step in &sentence.steps {
            if player_binding == PlayerBinding::Unbound
                && matches!(step, Step::LoseLife { who, .. } if who.player == "that-player")
            {
                return Err(unsupported("trigger", "unbound death controller"));
            }
            if let Some(i) = lower_wipes_step(
                step,
                counted,
                source_block,
                player_binding == PlayerBinding::DeathController,
            )? {
                program.instructions.push(i);
                continue;
            }
            if counted {
                return Err(unsupported("bindings", "unconsumed count binding"));
            }
            let instruction = match step {
                Step::CreateReplacement { .. } => {
                    return Err(unsupported("replacement", "unsupported replacement"));
                }
                Step::Exile { what } => Instruction::ExilePermanent {
                    target_slot: bind_permanent_target(program, slots, what)?,
                    source_block,
                },
                Step::Destroy {
                    what,
                    optional,
                    regeneration_forbidden,
                } => {
                    if what["ref"] == "all" && regeneration_forbidden.is_some() {
                        return Err(unsupported(
                            "regeneration",
                            "regeneration-forbidden mass destruction is outside this slice",
                        ));
                    }
                    if optional.is_none()
                        && regeneration_forbidden.is_none()
                        && *what == serde_json::json!({"ref":"all","filter":{"types":["creature"]}})
                    {
                        Instruction::DestroyAllCreatures { source_block }
                    } else if optional.is_none()
                        && regeneration_forbidden.is_none()
                        && what["filter"] == serde_json::json!({"types":["creature"]})
                    {
                        let t: Target = serde_json::from_value(what.clone()).map_err(|_| {
                            unsupported("effects.destroy", "unsupported destroy target")
                        })?;
                        Instruction::DestroyTargetCreature {
                            target_slot: bind_creature_target(program, slots, &t)?,
                            source_block,
                        }
                    } else {
                        if regeneration_forbidden.is_some() && *regeneration_forbidden != Some(true)
                        {
                            return Err(unsupported(
                                "regeneration",
                                "only cannot-be-regenerated destroy is supported",
                            ));
                        }
                        Instruction::DestroyPermanent {
                            target_slot: bind_permanent_target(program, slots, what)?,
                            optional: optional.unwrap_or(false),
                            regeneration_forbidden: regeneration_forbidden.unwrap_or(false),
                            source_block,
                        }
                    }
                }
                Step::ReturnToHand { what, to, optional } if to == "your" => {
                    let id = what["selectionId"].as_str().unwrap_or("");
                    if *what
                        != serde_json::json!({"ref":"target","filter":{"types":["creature"],"cls":"card","zone":{"zone":"graveyard","owner":"your"}},"count":{"amount":"fixed","value":1},"selectionId":id})
                    {
                        return Err(unsupported(
                            "effects.return",
                            "unsupported graveyard target",
                        ));
                    }
                    Instruction::ReturnOwnGraveyardCreatureToHand {
                        target_slot: bind_target(
                            program,
                            slots,
                            id,
                            TargetDomain::OwnGraveyardCreatureCard,
                        )?,
                        optional: optional.unwrap_or(false),
                        source_block,
                    }
                }
                Step::CreateToken {
                    count,
                    token,
                    who: Some(who),
                } => Instruction::CreateTokensFor {
                    recipient: removal_recipient(program, who)?,
                    count: if count.amount == "fixed" && count.value > 0 {
                        count.value
                    } else {
                        return Err(unsupported("token", "unsupported token count"));
                    },
                    token: lower_creature_token(token)?,
                    source_block,
                },
                Step::CreateToken {
                    count,
                    token,
                    who: None,
                } if count.amount == "fixed" && count.value > 0 => {
                    let token = lower_creature_token(token)?;
                    Instruction::CreateCreatureTokens {
                        count: count.value,
                        token,
                        source_block,
                    }
                }
                Step::PutCounters { on, counter, count } if counter.r#type == "+1/+1" => {
                    let on = match on {
                        SourceCounterRecipient::Target(t) => CounterRecipient::Target {
                            target_slot: bind_creature_target(program, slots, t)?,
                        },
                        SourceCounterRecipient::Source(t)
                            if t.reference == "this" && t.noun == "creature" =>
                        {
                            CounterRecipient::Source
                        }
                        _ => {
                            return Err(unsupported(
                                "effects.counters.on",
                                "unsupported counter recipient",
                            ));
                        }
                    };
                    Instruction::PutPlusOneCounters {
                        on,
                        amount: fixed_positive(count)?,
                        source_block,
                    }
                }
                Step::Pump {
                    what,
                    power,
                    toughness,
                    duration,
                } if *what == serde_json::json!({"ref":"this","noun":"creature"})
                    && duration.duration == "end-of-turn" =>
                {
                    Instruction::ModifySourcePowerToughness {
                        power: signed_fixed(power)?,
                        toughness: signed_fixed(toughness)?,
                        duration: Duration::UntilEndOfTurn,
                        source_block,
                    }
                }
                Step::Pump {
                    what,
                    power,
                    toughness,
                    duration,
                } if duration.duration == "end-of-turn" => Instruction::ModifyPowerToughness {
                    target_slot: bind_creature_target(
                        program,
                        slots,
                        &serde_json::from_value(what.clone())
                            .map_err(|_| unsupported("pump", "unsupported target"))?,
                    )?,
                    power: signed_fixed(power)?,
                    toughness: signed_fixed(toughness)?,
                    duration: Duration::UntilEndOfTurn,
                    source_block,
                },
                Step::PreventDamage {
                    prevention,
                    duration,
                } => lower_prevention(program, slots, prevention, duration, source_block)?,
                Step::Damage {
                    source,
                    amount,
                    to,
                    optional,
                } if !matches!(source, DamageSource::SelfReference {})
                    || optional.is_some()
                    || amount["amount"] != "fixed" =>
                {
                    lower_sacrifice_damage(
                        program,
                        slots,
                        source,
                        amount,
                        to,
                        *optional,
                        source_block,
                    )?
                }
                Step::Damage {
                    optional: None,
                    source: DamageSource::SelfReference {},
                    amount,
                    to: DamageDestination::Player { reference },
                } => {
                    let EffectRecipient::TargetController { target_slot } =
                        removal_recipient(program, reference)?
                    else {
                        return Err(unsupported("damage", "requires target controller"));
                    };
                    Instruction::DamageTargetController {
                        target_slot,
                        amount: fixed_positive_value(amount)?,
                        source_block,
                    }
                }
                Step::Damage {
                    optional: None,
                    source: DamageSource::SelfReference {},
                    amount,
                    to,
                } => {
                    let DamageDestination::AnyTarget { selection_id } = to else {
                        return Err(unsupported("damage", "unsupported damage destination"));
                    };
                    Instruction::DealDamage {
                        target_slot: bind_target(
                            program,
                            slots,
                            selection_id,
                            TargetDomain::AnyDamageTarget,
                        )?,
                        amount: fixed_positive_value(amount)?,
                        source_block,
                    }
                }
                Step::GainLife { who, amount, .. } | Step::LoseLife { who, amount }
                    if who.of.is_some() || amount["amount"] == "attribute" =>
                {
                    let recipient = removal_recipient(
                        program,
                        &serde_json::to_value(who).map_err(|_| invalid("life", "recipient"))?,
                    )?;
                    Instruction::ChangeLifeFromTarget {
                        recipient,
                        amount: removal_amount(program, amount)?,
                        gain: matches!(step, Step::GainLife { .. }),
                        source_block,
                    }
                }
                Step::GainLife { who, amount, .. } => Instruction::GainLife {
                    recipient: life_recipient(
                        program,
                        slots,
                        who,
                        player_binding == PlayerBinding::EventPlayer,
                    )?,
                    amount: fixed_positive(
                        &serde_json::from_value(amount.clone())
                            .map_err(|_| unsupported("amount", "unsupported fixed amount"))?,
                    )?,
                    source_block,
                },
                Step::LoseLife { who, amount } => Instruction::LoseLife {
                    recipient: life_recipient(
                        program,
                        slots,
                        who,
                        player_binding == PlayerBinding::EventPlayer,
                    )?,
                    amount: fixed_positive(
                        &serde_json::from_value(amount.clone())
                            .map_err(|_| unsupported("amount", "unsupported fixed amount"))?,
                    )?,
                    source_block,
                },
                Step::Discard { who, what }
                    if player_binding == PlayerBinding::DamageRecipient
                        && matches!(who.player.as_str(), "that-player" | "you")
                        && who.selection_id.is_none()
                        && who.of.is_none()
                        && what.reference == "filter"
                        && fixed_one(&what.count)
                        && what.filter
                            == ObjectFilter::Class(CardFilter { cls: "card".into() }) =>
                {
                    Instruction::DiscardCards {
                        recipient: if who.player == "that-player" {
                            Some(ActionRecipient::EventPlayer)
                        } else {
                            None
                        },
                        count: Quantity::from(1),
                        source_block,
                    }
                }
                Step::Draw {
                    who,
                    amount,
                    optional: true,
                } => {
                    if who.player != "you"
                        || who.selection_id.is_some()
                        || who.of.is_some()
                        || !amount.fixed_one()
                    {
                        return Err(unsupported(
                            "effects.draw",
                            "optional draw requires one card for the unqualified controller",
                        ));
                    }
                    Instruction::OptionalDrawCards { source_block }
                }
                Step::Draw {
                    who,
                    amount: DrawAmount::ThatMuch {},
                    optional: false,
                } if player_binding == PlayerBinding::DamageRecipient
                    && who.player == "you"
                    && who.selection_id.is_none()
                    && who.of.is_none() =>
                {
                    // CR 608.2c/608.2h: bind this quantity to the actual triggering
                    // damage event, not to current power, life change or another effect.
                    // The archive's reference graph remains diagnostic/nonexecutable.
                    Instruction::DrawDamageAmount { source_block }
                }
                Step::Draw {
                    who,
                    amount: DrawAmount::Fixed { value },
                    optional: false,
                } => {
                    let recipient = match life_recipient(
                        program,
                        slots,
                        who,
                        player_binding == PlayerBinding::EventPlayer,
                    )? {
                        LifeRecipient::Controller => None,
                        r => Some(r),
                    };
                    if !(1..=2).contains(value) {
                        return Err(unsupported("effects.draw", "unsupported fixed draw count"));
                    }
                    // CR 121.2: multiple draws are performed one at a time.
                    for _ in 1..*value {
                        program.instructions.push(Instruction::DrawCards {
                            recipient: recipient.clone(),
                            count: Quantity::from(1),
                            source_block,
                        });
                    }
                    Instruction::DrawCards {
                        recipient,
                        count: Quantity::from(1),
                        source_block,
                    }
                }
                Step::Counter { what } if what.reference == "target" && fixed_one(&what.count) => {
                    let domain = match &what.filter {
                        CounterFilter::Spell { .. } => TargetDomain::StackSpell,
                        CounterFilter::Ability { any_of }
                            if any_of.len() == 2
                                && any_of
                                    .iter()
                                    .map(|filter| filter.ability_kind.as_str())
                                    .collect::<BTreeSet<_>>()
                                    == BTreeSet::from(["activated", "triggered"]) =>
                        {
                            TargetDomain::StackActivatedOrTriggeredAbility
                        }
                        _ => {
                            return Err(unsupported(
                                "targets",
                                "counter requires the complete spell or activated-or-triggered domain",
                            ));
                        }
                    };
                    let slot = bind_target(program, slots, &what.selection_id, domain)?;
                    if let CounterFilter::Spell {
                        all_of: Some(all_of),
                    } = &what.filter
                    {
                        let f = lower_permanent_filter(&serde_json::json!({"allOf":all_of}))?;
                        if f.types.is_empty()
                            || !f.non_types.is_empty()
                            || !f.non_colors.is_empty()
                            || f.opponent
                        {
                            return Err(unsupported("counter", "unsupported typed spell filter"));
                        }
                        program.targets[slot as usize].filter = Some(f);
                    }
                    Instruction::CounterStackObject {
                        target_slot: slot,
                        source_block,
                    }
                }
                Step::ReturnToHand {
                    what,
                    to,
                    optional: None,
                } if to == "owner" => {
                    let t: Target = serde_json::from_value(what.clone())
                        .map_err(|_| unsupported("effects.return", "unsupported target"))?;
                    let target_slot = bind_creature_target(program, slots, &t)?;
                    Instruction::ReturnToOwnerHand {
                        target_slot,
                        source_block,
                    }
                }
                Step::MoveZone {
                    what,
                    to,
                    tapped: None,
                } if *to
                    == serde_json::json!({"zone":"library","owner":"its-owner","position":"top"}) =>
                {
                    let t: Target = serde_json::from_value(what.clone())
                        .map_err(|_| unsupported("effects.move", "unsupported target"))?;
                    Instruction::PutOnTopOwnerLibrary {
                        target_slot: bind_creature_target(program, slots, &t)?,
                        source_block,
                    }
                }
                Step::Sacrifice {
                    who,
                    what,
                    chosen_by,
                } if player_binding == PlayerBinding::DamageRecipient
                    && who.player == "that-player"
                    && who.selection_id.is_none()
                    && who.of.is_none()
                    && who == chosen_by
                    && what.reference == "filter"
                    && fixed_one(&what.count)
                    && what.filter
                        == ObjectFilter::Types(Filter {
                            types: vec!["creature".into()],
                        }) =>
                {
                    Instruction::SacrificeCreature {
                        recipient: ActionRecipient::EventPlayer,
                        source_block,
                    }
                }
                Step::Sacrifice {
                    who,
                    what,
                    chosen_by,
                } if who.player == "each-opponent"
                    && who.selection_id.is_none()
                    && who.of.is_none()
                    && who == chosen_by
                    && what.reference == "filter"
                    && fixed_one(&what.count)
                    && what.filter
                        == ObjectFilter::Types(Filter {
                            types: vec!["creature".into()],
                        }) =>
                {
                    Instruction::EachOpponentSacrifice { source_block }
                }
                Step::Sacrifice {
                    who,
                    what,
                    chosen_by,
                } if who.player == "target-opponent"
                    && who.of.is_none()
                    && who == chosen_by
                    && what.reference == "filter"
                    && fixed_one(&what.count)
                    && what.filter
                        == ObjectFilter::Types(Filter {
                            types: vec!["creature".into()],
                        }) =>
                {
                    // CR 601.2c / 608.2d / 701.21: opponent target is bound at
                    // announcement; that player selects a controlled creature
                    // only when the sacrifice instruction resolves.
                    // https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
                    let target_slot = bind_target(
                        program,
                        slots,
                        who.selection_id.as_deref().unwrap_or(""),
                        TargetDomain::OpponentPlayer,
                    )?;
                    Instruction::TargetPlayerSacrificeCreature {
                        target_slot,
                        source_block,
                    }
                }
                _ => {
                    return Err(unsupported(
                        "effects.steps",
                        "unsupported complete spell operator, destination, chooser or cardinality",
                    ));
                }
            };
            program.instructions.push(instruction);
        }
    }
    Ok(())
}
// Pinned CR 115, 608.2b/h and 202.3a: bind the complete finite filter and
// target reference; no unparsed attribute or controller clause is discarded.
/// Closed source-backed sets: never silently discard filter fields (CR 608.2h).
fn lower_object_set(v: &serde_json::Value) -> Result<ObjectSet, Failure> {
    use serde_json::json;
    if v["ref"] != "each" && v["ref"] != "all" {
        return Err(unsupported("set", "expected all/each set"));
    }
    if v.as_object().is_none_or(|o| o.len() != 2) {
        return Err(unsupported("set", "extra set fields"));
    }
    let f = &v["filter"];
    let set = if *f == json!({"types":["creature"]}) {
        ObjectSet::Creatures
    } else if *f == json!({"nonTypes":["land"],"cls":"permanent"}) {
        ObjectSet::NonlandPermanents
    } else if *f == json!({"types":["enchantment"]}) {
        ObjectSet::Enchantments
    } else if *f == json!({"types":["creature"],"status":["attacking"]}) {
        ObjectSet::AttackingCreatures
    } else if *f == json!({"types":["creature"],"control":"opponent"}) {
        ObjectSet::OpponentCreatures
    } else if *f == json!({"types":["creature"],"withAbility":["flying"]}) {
        ObjectSet::FlyingCreatures
    } else if *f == json!({"types":["creature"],"withoutAbility":["flying"]}) {
        ObjectSet::NonflyingCreatures
    } else {
        return Err(unsupported("set", "unsupported complete filter"));
    };
    Ok(set)
}
fn lower_wipes_step(
    step: &Step,
    counted: bool,
    source_block: u32,
    death_controller: bool,
) -> Result<Option<Instruction>, Failure> {
    use serde_json::json;
    Ok(Some(match step {
        Step::Exile { what } if what["ref"] == "all" || what["ref"] == "each" => {
            Instruction::MoveSet {
                set: lower_object_set(what)?,
                action: SetMove::Exile,
                source_block,
            }
        }
        Step::Destroy {
            what,
            optional: None,
            regeneration_forbidden: None,
        } if (what["ref"] == "all" || what["ref"] == "each")
            && *what != json!({"ref":"all","filter":{"types":["creature"]}}) =>
        {
            Instruction::MoveSet {
                set: lower_object_set(what)?,
                action: SetMove::Destroy,
                source_block,
            }
        }
        Step::ReturnToHand {
            what,
            to,
            optional: None,
        } if to == "owner" && (what["ref"] == "all" || what["ref"] == "each") => {
            Instruction::MoveSet {
                set: lower_object_set(what)?,
                action: SetMove::Hand,
                source_block,
            }
        }
        Step::Pump {
            what,
            power,
            toughness,
            duration,
        } if (what["ref"] == "all" || what["ref"] == "each")
            && duration.duration == "end-of-turn" =>
        {
            Instruction::PumpSet {
                set: lower_object_set(what)?,
                power: signed_fixed(power)?,
                toughness: signed_fixed(toughness)?,
                source_block,
            }
        }
        Step::Damage {
            source: DamageSource::SelfReference {},
            amount,
            to,
            optional: None,
        } if matches!(
            to,
            DamageDestination::Object { .. } | DamageDestination::Each { .. }
        ) =>
        {
            let refs = match to {
                DamageDestination::Object { reference } => vec![reference.clone()],
                DamageDestination::Each { refs } => refs.clone(),
                _ => unreachable!(),
            };
            let mut sets = vec![];
            let mut each_player = false;
            for r in refs {
                if r == json!({"player":"each-player"}) && !each_player {
                    each_player = true;
                } else {
                    sets.push(lower_object_set(&r)?);
                }
            }
            let amount = if *amount == json!({"amount":"x"}) {
                if counted {
                    WipeAmount::CreatureCount
                } else {
                    WipeAmount::AnnouncedX
                }
            } else {
                if counted {
                    return Err(unsupported("binding", "unused X"));
                }
                WipeAmount::Fixed {
                    value: fixed_positive_value(amount)?,
                }
            };
            Instruction::DamageSet {
                sets,
                each_player,
                amount,
                source_block,
            }
        }
        Step::GainLife {
            who,
            amount,
            for_each: Some(f),
        } if who.player == "you"
            && who.selection_id.is_none()
            && who.of.is_none()
            && *amount == json!({"amount":"fixed","value":1})
            && *f
                == json!({"types":["creature"],"relation":{"relation":"event-result","event":"destroy","scope":"this-way"}}) =>
        {
            Instruction::GainLifeDestroyed { source_block }
        }
        Step::GainLife {
            for_each: Some(_), ..
        } => return Err(unsupported("forEach", "unsupported event result")),
        Step::LoseLife { who, amount }
            if death_controller
                && who.player == "that-player"
                && who.selection_id.is_none()
                && who.of.is_none() =>
        {
            Instruction::LoseLifeDeathController {
                amount: fixed_positive_value(amount)?,
                source_block,
            }
        }
        Step::Sacrifice {
            who,
            what,
            chosen_by,
        } if who.player == "each-player"
            && who == chosen_by
            && who.of.is_none()
            && who.selection_id.is_none()
            && what.reference == "filter"
            && matches!(&what.filter,ObjectFilter::Types(f) if f.types==["creature"])
            && fixed_one(&what.count) =>
        {
            Instruction::EachPlayerSacrifice { source_block }
        }
        Step::CreateReplacement {
            replacement,
            duration,
        } if duration.duration == "this-turn"
            && *replacement
                == json!({"event":{"event":"move-zone","what":{"ref":"filter","filter":{"types":["creature"],"damageHistory":{"role":"recipient","window":{"event":"this-way"}}},"count":{"amount":"fixed","value":1}},"from":{"zone":"battlefield"},"to":{"zone":"graveyard"}},"occurrence":"each","replace":{"replacement":"effects","effects":[{"steps":[{"effect":"exile","what":{"ref":"replacement-event-object","role":"affected"}}]}]}}) =>
        {
            Instruction::ExileDamagedThisTurn { source_block }
        }
        _ => return Ok(None),
    }))
}

fn lower_creature_token(token: &SourceToken) -> Result<CreatureToken, Failure> {
    if !token.supertypes.is_empty()
        || token.types != ["creature"]
        || token.power.amount != "fixed"
        || token.toughness.amount != "fixed"
    {
        return Err(unsupported(
            "effects.token",
            "unsupported token characteristic shape",
        ));
    }
    let colors = token
        .colors
        .iter()
        .map(|c| match c.as_str() {
            "white" => Ok(Color::W),
            "blue" => Ok(Color::U),
            "black" => Ok(Color::B),
            "red" => Ok(Color::R),
            "green" => Ok(Color::G),
            _ => Err(unsupported("effects.token.colors", "unsupported color")),
        })
        .collect::<Result<Vec<_>, _>>()?;
    let keywords = token
        .keywords
        .iter()
        .map(|k| {
            let obj = k
                .as_object()
                .ok_or_else(|| unsupported("effects.token.keywords", "invalid keyword"))?;
            if obj.len() != 1 {
                return Err(unsupported(
                    "effects.token.keywords",
                    "parameterized keyword",
                ));
            }
            serde_json::from_value::<KeywordAbility>(k["keyword"].clone())
                .map_err(|_| unsupported("effects.token.keywords", "unsupported keyword"))
        })
        .collect::<Result<Vec<_>, _>>()?;
    let token = CreatureToken {
        colors,
        subtypes: token.subtypes.clone(),
        power: i64::from(token.power.value).into(),
        toughness: i64::from(token.toughness.value).into(),
        keywords,
    };
    token
        .validate()
        .map_err(|e| unsupported("effects.token", e))?;

    Ok(token)
}
fn lower_permanent_filter(v: &serde_json::Value) -> Result<PermanentFilter, Failure> {
    let kind = |v: &serde_json::Value| -> Result<CardKind, Failure> {
        serde_json::from_value(v.clone())
            .map_err(|_| unsupported("filter", "unsupported card type"))
    };
    let mut types = vec![];
    let obj = v
        .as_object()
        .ok_or_else(|| unsupported("filter", "not an object"))?;
    if obj.keys().any(|k| {
        !["cls", "types", "allOf", "nonTypes", "nonColors", "control"].contains(&k.as_str())
    }) {
        return Err(unsupported("filter", "unsupported filter term"));
    }
    if let Some(a) = v.get("types") {
        for t in a.as_array().ok_or_else(|| unsupported("filter", "types"))? {
            types.push(kind(t)?);
        }
    }
    if let Some(a) = v.get("allOf") {
        if !types.is_empty() {
            return Err(unsupported("filter", "mixed type forms"));
        }
        let a = a.as_array().ok_or_else(|| unsupported("filter", "allOf"))?;
        if a.len() != 1 || a[0].as_object().is_none_or(|o| o.len() != 1) {
            return Err(unsupported("filter", "unsupported conjunction"));
        }
        let alternatives = a[0]["anyOf"]
            .as_array()
            .ok_or_else(|| unsupported("filter", "anyOf"))?;
        if alternatives.len() < 2 {
            return Err(unsupported("filter", "empty alternatives"));
        }
        for a in alternatives {
            if a.as_object().is_none_or(|o| o.len() != 1)
                || a["types"].as_array().is_none_or(|a| a.len() != 1)
            {
                return Err(unsupported("filter", "unsupported alternative"));
            }
            types.push(kind(&a["types"][0])?);
        }
    }
    if let Some(cls) = v.get("cls")
        && (cls != "permanent" || !types.is_empty())
    {
        return Err(unsupported("filter", "unsupported class"));
    }
    if types.is_empty() && v.get("cls").is_none() {
        return Err(unsupported("filter", "missing type/class"));
    }
    let non_types = v
        .get("nonTypes")
        .map(|a| {
            a.as_array()
                .ok_or_else(|| unsupported("filter", "nonTypes"))
                .and_then(|a| a.iter().map(kind).collect())
        })
        .transpose()?
        .unwrap_or_default();
    let non_colors = v
        .get("nonColors")
        .map(|a| {
            a.as_array()
                .ok_or_else(|| unsupported("filter", "nonColors"))
                .and_then(|a| {
                    a.iter()
                        .map(|c| match c.as_str() {
                            Some("black") => Ok(Color::B),
                            _ => Err(unsupported("filter", "unsupported noncolor")),
                        })
                        .collect()
                })
        })
        .transpose()?
        .unwrap_or_default();
    let opponent = match v.get("control") {
        None => false,
        Some(c) if c == "opponent" => true,
        _ => return Err(unsupported("filter", "unsupported controller")),
    };
    Ok(PermanentFilter {
        types,
        non_types,
        non_colors,
        opponent,
    })
}
fn bind_permanent_target(
    p: &mut SpellProgram,
    slots: &mut BTreeMap<String, u32>,
    v: &serde_json::Value,
) -> Result<u32, Failure> {
    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase", deny_unknown_fields)]
    struct T {
        #[serde(rename = "ref")]
        reference: String,
        filter: serde_json::Value,
        count: Count,
        selection_id: String,
    }
    let t: T = serde_json::from_value(v.clone())
        .map_err(|_| unsupported("target", "unsupported complete permanent target"))?;
    if t.reference != "target" || !fixed_one(&t.count) {
        return Err(unsupported("target", "requires one target"));
    }
    let f = lower_permanent_filter(&t.filter)?;
    let slot = bind_target(
        p,
        slots,
        &t.selection_id,
        TargetDomain::BattlefieldPermanent,
    )?;
    if p.targets[slot as usize]
        .filter
        .as_ref()
        .is_some_and(|old| old != &f)
    {
        return Err(unsupported("filter", "conflicting target filter"));
    }
    p.targets[slot as usize].filter = Some(f);
    Ok(slot)
}
fn target_reference(p: &SpellProgram, v: &serde_json::Value) -> Result<u32, Failure> {
    if p.targets.len() != 1
        || !matches!(
            p.targets[0].domain,
            TargetDomain::BattlefieldPermanent | TargetDomain::StackSpell
        )
    {
        return Err(unsupported("reference", "requires bound removal target"));
    }
    let valid = *v == serde_json::json!({"ref":"it"})
        || *v == serde_json::json!({"ref":"that","filter":{"cls":"permanent"}})
        || (*v == serde_json::json!({"ref":"that","filter":{"types":["artifact"]}})
            && p.targets[0]
                .filter
                .as_ref()
                .is_some_and(|f| f.types == [CardKind::Artifact]));
    if !valid {
        return Err(unsupported(
            "reference",
            "unsupported complete target reference",
        ));
    }
    Ok(0)
}
fn removal_recipient(p: &SpellProgram, v: &serde_json::Value) -> Result<EffectRecipient, Failure> {
    if *v == serde_json::json!({"player":"you"}) {
        return Ok(EffectRecipient::Controller);
    }
    if v.as_object().is_none_or(|o| o.len() != 2) || v["player"] != "controller" {
        return Err(unsupported("recipient", "unsupported target controller"));
    }
    Ok(EffectRecipient::TargetController {
        target_slot: target_reference(p, &v["of"])?,
    })
}
fn removal_amount(p: &SpellProgram, v: &serde_json::Value) -> Result<TargetAmount, Failure> {
    if v["amount"] == "attribute" && v.as_object().is_some_and(|o| o.len() == 3) {
        let attribute = match v["attribute"].as_str() {
            Some("power") => TargetAttribute::Power,
            Some("mana-value") => TargetAttribute::ManaValue,
            _ => return Err(unsupported("amount", "unsupported attribute")),
        };
        return Ok(TargetAmount::Attribute {
            target_slot: target_reference(p, &v["of"])?,
            attribute,
        });
    }
    Ok(TargetAmount::Fixed {
        amount: fixed_positive(
            &serde_json::from_value(v.clone())
                .map_err(|_| unsupported("amount", "unsupported amount"))?,
        )?,
    })
}
fn signed_fixed(value: &SignedAmount) -> Result<Quantity, Failure> {
    if value.amount.amount != "fixed" || !matches!(value.sign, -1 | 1) {
        return Err(unsupported(
            "effects.pump",
            "unsupported signed fixed amount",
        ));
    }
    Ok(Quantity::from(
        i64::from(value.amount.value) * i64::from(value.sign),
    ))
}
fn lower_prevention(
    program: &mut SpellProgram,
    slots: &mut BTreeMap<String, u32>,
    prevention: &SourcePrevention,
    duration: &SourceDuration,
    source_block: u32,
) -> Result<Instruction, Failure> {
    // CR 615.7: the finite shield budget is shared across simultaneous sources.
    // The IR retains its target and duration; allocation is a runtime choice.
    // https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
    if duration.duration != "this-turn"
        || prevention.event_scope.sources != "simultaneous"
        || prevention.event_scope.recipients != "simultaneous"
    {
        return Err(unsupported(
            "effects.prevention",
            "unsupported event scope or duration",
        ));
    }
    match &prevention.shield {
        SourceShield::Events {
            occurrence,
            prevent: SourcePrevent::All {},
        } if occurrence == "each" && prevention.kind == "combat" && prevention.to.is_none() => {
            Ok(Instruction::PreventCombatDamage {
                duration: Duration::UntilEndOfTurn,
                source_block,
            })
        }
        SourceShield::DamageBudget { amount } if prevention.kind == "all" => {
            let Some(targets) = &prevention.to else {
                return Err(unsupported(
                    "effects.prevention.to",
                    "missing complete any-target scope",
                ));
            };
            let [AnyTargetReference::AnyTarget { selection_id }] = targets.as_slice() else {
                return Err(unsupported(
                    "effects.prevention.to",
                    "unsupported recipient scope",
                ));
            };
            Ok(Instruction::PreventNextDamage {
                target_slot: bind_target(
                    program,
                    slots,
                    selection_id,
                    TargetDomain::AnyDamageTarget,
                )?,
                amount: fixed_positive(amount)?,
                duration: Duration::UntilEndOfTurn,
                source_block,
            })
        }
        _ => Err(unsupported(
            "effects.prevention",
            "unsupported complete prevention family",
        )),
    }
}
fn lower_static(effect: &SourceStaticEffect) -> Result<StaticAbility, Failure> {
    if let SourceStaticEffect::Prohibit { prohibition } = effect {
        // CR 101.2/109.5/601.3/611.3b: the current battlefield controller
        // defines opponents and whose turn. Consume the whole action scope;
        // archived unsupported-scope diagnostics are not executable authority.
        if *prohibition
            != serde_json::json!({"prohibition":"cast","who":{"player":"each-opponent"},
            "what":{"ref":"each","filter":{"cls":"spell"}},"when":{"during":"turn","whose":{"player":"you"}}})
        {
            return Err(unsupported(
                "static.prohibition",
                "unsupported complete casting prohibition scope",
            ));
        }
        return Ok(StaticAbility {
            functioning_zone: FunctioningZone::Battlefield,
            effect: StaticEffect::OpponentsCantCastDuringControllerTurn,
        });
    }

    // CR 614.1c/614.12: only unconditional entry of this source is supported.
    // https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
    if let SourceStaticEffect::EntersTapped {
        what,
        condition_kind,
        condition,
    } = effect
    {
        if condition_kind.is_some() || condition.is_some() {
            return Err(unsupported("static", "static:enters-tapped:conditional"));
        }
        if ![
            "permanent",
            "creature",
            "artifact",
            "enchantment",
            "aura",
            "land",
        ]
        .iter()
        .any(|noun| *what == serde_json::json!({"ref":"this","noun":noun}))
        {
            return Err(unsupported(
                "static",
                "static:enters-tapped:unsupported_source",
            ));
        }
        return Ok(StaticAbility {
            functioning_zone: FunctioningZone::Battlefield,
            effect: StaticEffect::EntersTapped,
        });
    }
    let special = match effect {
        SourceStaticEffect::Cant { what, action }
            if *what == serde_json::json!({"ref":"this","noun":"spell"})
                && action == "be-countered" =>
        {
            Some(StaticEffect::CantBeCountered)
        }
        SourceStaticEffect::CostModification {
            scope,
            change,
            multiplier,
        } if *scope
            == serde_json::json!({"action":"cast","spells":{"ref":"this","noun":"spell"}})
            && *change == serde_json::json!({"direction":"reduce","symbols":["1"]})
            && *multiplier
                == serde_json::json!({"amount":"count","of":{"types":["creature"],"zone":{"zone":"battlefield"}}}) =>
        {
            Some(StaticEffect::ReduceGenericByCreatureCount)
        }
        SourceStaticEffect::CostModification { .. } => {
            return Err(unsupported("static", "unsupported cost modification"));
        }
        _ => None,
    };
    if let Some(effect) = special {
        return Ok(StaticAbility {
            functioning_zone: FunctioningZone::Stack,
            effect,
        });
    }
    if matches!(effect, SourceStaticEffect::Cant { what, action } if *what == serde_json::json!({"ref":"this","noun":"creature"}) && action == "block")
    {
        return Ok(StaticAbility {
            functioning_zone: FunctioningZone::Battlefield,
            effect: StaticEffect::ModifyCreatures {
                scope: CreatureScope::Source,
                power: Quantity::zero(),
                toughness: Quantity::zero(),
                keywords: vec![],
                restrictions: vec![CreatureRestriction::CantBlock],
            },
        });
    }
    // CR 611.3a/613.1f: a live battlefield source continuously grants the
    // keyword to its current attacking creatures. Consume the entire AST shape;
    // neither status/ownership qualifiers nor mixed modifiers are discarded.
    if let SourceStaticEffect::Modify { what, modifiers } = effect
        && *what
            == serde_json::json!({"ref":"each","filter":{"status":["attacking"],"types":["creature"],"control":"you"}})
    {
        if modifiers.len() != 1
            || modifiers[0]
                != serde_json::json!({"modifier":"abilities","abilities":[{"gained":"keyword","keyword":{"keyword":"deathtouch"}}]})
        {
            return Err(unsupported(
                "static.modifier",
                "attacking creature grant requires exactly deathtouch",
            ));
        }
        return Ok(StaticAbility {
            functioning_zone: FunctioningZone::Battlefield,
            effect: StaticEffect::ModifyCreatures {
                scope: CreatureScope::AttackingController,
                power: Quantity::zero(),
                toughness: Quantity::zero(),
                keywords: vec![KeywordAbility::Deathtouch],
                restrictions: vec![],
            },
        });
    }
    let scope = |what: &serde_json::Value| {
        if *what == serde_json::json!({"ref":"equipped","noun":"creature"}) {
            Some(CreatureScope::Equipped)
        } else if *what == serde_json::json!({"ref":"enchanted","noun":"creature"}) {
            Some(CreatureScope::Enchanted)
        } else if *what
            == serde_json::json!({"ref":"each","filter":{"types":["creature"],"control":"you"}})
        {
            Some(CreatureScope::Controller)
        } else {
            None
        }
    };
    if let SourceStaticEffect::Cant { what, action } = effect {
        if scope(what) != Some(CreatureScope::Enchanted) || action != "attack-or-block" {
            return Err(unsupported("static.cant", "unsupported prohibition"));
        }
        return Ok(StaticAbility {
            functioning_zone: FunctioningZone::Battlefield,
            effect: StaticEffect::ModifyCreatures {
                scope: CreatureScope::Enchanted,
                power: Quantity::zero(),
                toughness: Quantity::zero(),
                keywords: vec![],
                restrictions: vec![CreatureRestriction::CantAttackOrBlock],
            },
        });
    }
    if let SourceStaticEffect::Modify { what, modifiers } = effect
        && (scope(what) != Some(CreatureScope::Controller)
            || modifiers.len() != 1
            || modifiers[0]["modifier"] != "pt"
            || modifiers[0]["power"] != modifiers[0]["toughness"]
            || modifiers[0]["power"]["amount"]["value"] == 0)
    {
        let scope =
            scope(what).ok_or_else(|| unsupported("static.scope", "unsupported modifier scope"))?;
        let mut power = Quantity::zero();
        let mut toughness = Quantity::zero();
        let mut keywords = vec![];
        let mut restrictions = vec![];
        let mut pt = false;
        for modifier in modifiers {
            #[derive(Deserialize)]
            #[serde(tag = "modifier", rename_all = "kebab-case", deny_unknown_fields)]
            enum Modifier {
                Pt {
                    power: SignedAmount,
                    toughness: SignedAmount,
                },
                Abilities {
                    abilities: Vec<serde_json::Value>,
                },
                Cant {
                    action: String,
                },
            }
            match serde_json::from_value(modifier.clone())
                .map_err(|e| unsupported("static.modifier", format!("{e}")))?
            {
                Modifier::Pt {
                    power: p,
                    toughness: t,
                } => {
                    if pt {
                        return Err(unsupported("static.modifier", "duplicate pt"));
                    }
                    pt = true;
                    power = signed_fixed(&p)?;
                    toughness = signed_fixed(&t)?;
                    if power.is_negative() || toughness.is_negative() {
                        return Err(unsupported("static.modifier", "negative static pt"));
                    }
                }
                Modifier::Abilities { abilities } => {
                    if abilities.is_empty() {
                        return Err(unsupported("static.modifier", "empty grant"));
                    }
                    for a in abilities {
                        if a.as_object().is_none_or(|o| o.len() != 2)
                            || a["gained"] != "keyword"
                            || a["keyword"].as_object().is_none_or(|o| o.len() != 1)
                        {
                            return Err(unsupported("static.modifier", "unsupported grant"));
                        }
                        let k: KeywordAbility =
                            serde_json::from_value(a["keyword"]["keyword"].clone())
                                .map_err(|e| unsupported("static.keyword", e.to_string()))?;
                        if keywords.contains(&k) {
                            return Err(unsupported("static.keyword", "duplicate grant"));
                        }
                        keywords.push(k);
                    }
                }
                Modifier::Cant { action } => {
                    let restriction = match action.as_str() {
                        "be-blocked" => CreatureRestriction::CantBeBlocked,
                        "attack-or-block" => CreatureRestriction::CantAttackOrBlock,
                        _ => return Err(unsupported("static.cant", "unsupported restriction")),
                    };
                    if restrictions.contains(&restriction) {
                        return Err(unsupported("static.cant", "duplicate restriction"));
                    }
                    restrictions.push(restriction);
                }
            }
        }
        return Ok(StaticAbility {
            functioning_zone: FunctioningZone::Battlefield,
            effect: StaticEffect::ModifyCreatures {
                scope,
                power,
                toughness,
                keywords,
                restrictions,
            },
        });
    }
    if let SourceStaticEffect::Modify { what, modifiers } = effect {
        if *what
            != serde_json::json!({"ref":"each","filter":{"types":["creature"],"control":"you"}})
            || modifiers.len() != 1
        {
            return Err(unsupported("abilities.static", "unsupported anthem scope"));
        }
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Pt {
            modifier: String,
            power: SignedAmount,
            toughness: SignedAmount,
        }
        let m: Pt = serde_json::from_value(modifiers[0].clone())
            .map_err(|_| unsupported("abilities.static", "unsupported anthem modifier"))?;
        let power = signed_fixed(&m.power)?;
        let toughness = signed_fixed(&m.toughness)?;
        if m.modifier != "pt" || power <= Quantity::zero() || power != toughness {
            return Err(unsupported(
                "abilities.static",
                "anthem must give fixed +N/+N",
            ));
        }
        return Ok(StaticAbility {
            functioning_zone: FunctioningZone::Battlefield,
            effect: StaticEffect::ControllerCreatureAnthem { power, toughness },
        });
    }
    let SourceStaticEffect::Replacement { replacement } = effect else {
        unreachable!()
    };
    let SourceDamageEvent::Damage { kind, source, to } = &replacement.event;
    let SourceReplacementAction::ModifyQuantity { change } = &replacement.replace;
    let permanent = |recipient: &ReplacementRecipient| {
        matches!(recipient,
        ReplacementRecipient::Permanent(FilterObject { reference, filter: ObjectFilter::Class(CardFilter { cls }), count })
        if reference == "filter" && cls == "permanent" && fixed_one(count))
    };
    let player = |recipient: &ReplacementRecipient| {
        matches!(recipient,
        ReplacementRecipient::Player(p) if p.player == "a-player" && p.selection_id.is_none() && p.of.is_none())
    };
    if kind != "all"
        || source.reference != "filter"
        || !fixed_one(&source.count)
        || replacement.occurrence != "each"
        || change.operation != "multiply"
        || change.factor != 2
        || to.len() != 2
        || to.iter().filter(|r| permanent(r)).count() != 1
        || to.iter().filter(|r| player(r)).count() != 1
    {
        return Err(unsupported(
            "abilities.static",
            "unsupported complete damage replacement scope",
        ));
    }
    // Consume the complete source predicate. The old global source remains
    // exactly empty; no controller, property or type qualifier may be dropped.
    let effect = if source.filter == serde_json::json!({}) {
        StaticEffect::DoubleDamageToPermanentOrPlayer
    } else if source.filter == serde_json::json!({"types":["creature"],"control":"you"}) {
        StaticEffect::DoubleControllerCreatureDamageToPermanentOrPlayer
    } else {
        return Err(unsupported(
            "abilities.static",
            "unsupported complete damage replacement source",
        ));
    };
    // CR 611.3b/614.5: battlefield-only static rule, once per damage lineage.
    // https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
    Ok(StaticAbility {
        functioning_zone: FunctioningZone::Battlefield,
        effect,
    })
}

fn fixed_positive_value(value: &serde_json::Value) -> Result<Quantity, Failure> {
    fixed_positive(
        &serde_json::from_value(value.clone())
            .map_err(|_| unsupported("amount", "unsupported damage amount"))?,
    )
}
fn lower_sacrifice_filter(what: &serde_json::Value) -> Result<SacrificeFilter, Failure> {
    let types = if *what
        == serde_json::json!({"ref":"filter","filter":{"types":["creature"]},"count":{"amount":"fixed","value":1}})
    {
        vec![CardKind::Creature]
    } else if *what
        == serde_json::json!({"ref":"filter","filter":{"allOf":[{"anyOf":[{"types":["artifact"]},{"types":["creature"]}]}]},"count":{"amount":"fixed","value":1}})
    {
        vec![CardKind::Artifact, CardKind::Creature]
    } else if *what
        == serde_json::json!({"ref":"filter","filter":{"subtypes":["Goblin"]},"count":{"amount":"fixed","value":1}})
    {
        return Ok(SacrificeFilter {
            types: vec![],
            subtypes: vec!["Goblin".into()],
        });
    } else {
        return Err(unsupported("cost", "unsupported complete sacrifice filter"));
    };
    Ok(SacrificeFilter {
        types,
        subtypes: vec![],
    })
}
fn lower_sacrifice_damage(
    p: &mut SpellProgram,
    slots: &mut BTreeMap<String, u32>,
    source: &DamageSource,
    amount: &serde_json::Value,
    to: &DamageDestination,
    optional: Option<bool>,
    source_block: u32,
) -> Result<Instruction, Failure> {
    if !(matches!(source, DamageSource::SelfReference {} | DamageSource::It {})
        || matches!(source, DamageSource::This { noun } if noun == "creature" || noun == "enchantment"))
    {
        return Err(unsupported("damage", "unsupported complete damage source"));
    }
    let amount = if *amount
        == serde_json::json!({"amount":"attribute","of":{"ref":"related","number":"singular","filter":{"types":["creature"]},"relation":{"relation":"event-result","event":"sacrifice","scope":"contextual"}},"attribute":"power"})
    {
        if !matches!(source, DamageSource::SelfReference {}) || p.additional_sacrifice.is_none() {
            return Err(unsupported("damage", "unbound sacrificed power"));
        }
        SacrificeDamageAmount::SacrificedPower
    } else {
        SacrificeDamageAmount::Fixed {
            value: fixed_positive_value(amount)?,
        }
    };
    let (target_slot, recipient) = match to {
        DamageDestination::AnyTarget { selection_id } => (
            Some(bind_target(
                p,
                slots,
                selection_id,
                TargetDomain::AnyDamageTarget,
            )?),
            None,
        ),
        DamageDestination::Player { reference }
            if *reference == serde_json::json!({"player":"you"}) =>
        {
            (None, Some(LifeRecipient::Controller))
        }
        DamageDestination::Player { reference }
            if *reference == serde_json::json!({"player":"each-opponent"}) =>
        {
            (None, Some(LifeRecipient::EachOpponent))
        }
        _ => {
            return Err(unsupported(
                "damage",
                "unsupported complete damage recipient",
            ));
        }
    };
    Ok(Instruction::SacrificeDamage {
        target_slot,
        recipient,
        amount,
        optional: optional.unwrap_or(false),
        source_block,
    })
}
fn fixed_positive(count: &Count) -> Result<Quantity, Failure> {
    if count.amount != "fixed" || count.value == 0 {
        return Err(unsupported(
            "amount",
            "only positive fixed quantities are supported",
        ));
    }
    Ok(Quantity::from(i64::from(count.value)))
}
fn life_recipient(
    program: &mut SpellProgram,
    slots: &mut BTreeMap<String, u32>,
    who: &PlayerSelector,
    event_player: bool,
) -> Result<LifeRecipient, Failure> {
    if who.of.is_some() {
        return Err(unsupported("who", "unexpected object reference"));
    }
    match (who.player.as_str(), who.selection_id.as_deref()) {
        ("you", None) => Ok(LifeRecipient::Controller),
        ("that-player", None) if event_player => Ok(LifeRecipient::EventPlayer),
        ("each-opponent", None) => Ok(LifeRecipient::EachOpponent),
        ("target-player", Some(id)) => Ok(LifeRecipient::Target {
            target_slot: bind_target(program, slots, id, TargetDomain::AnyPlayer)?,
        }),
        _ => Err(unsupported(
            "who",
            "unsupported life recipient or unbound target",
        )),
    }
}
fn bind_creature_target(
    program: &mut SpellProgram,
    slots: &mut BTreeMap<String, u32>,
    target: &Target,
) -> Result<u32, Failure> {
    if target.reference != "target"
        || target.filter.types != ["creature"]
        || !fixed_one(&target.count)
    {
        return Err(unsupported(
            "targets",
            "unsupported creature target scope/cardinality",
        ));
    }
    bind_target(
        program,
        slots,
        &target.selection_id,
        TargetDomain::BattlefieldCreature,
    )
}
fn is_source_creature(subject: &TriggerObject) -> bool {
    matches!(subject,TriggerObject::This {noun} if noun=="creature")
}
fn is_other_creature(subject: &TriggerObject) -> bool {
    matches!(subject,TriggerObject::Filter {filter,count} if filter.other && filter.types==["creature"] && filter.control.is_none() && !filter.non_token && filter.non_subtypes.is_empty() && fixed_one(count))
}
// Omission denotes any damage kind. An explicit null is not that AST shape.
fn damage_combat_qualifier<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> Result<Option<bool>, D::Error> {
    bool::deserialize(deserializer).map(Some)
}
fn lower_trigger(
    trigger: &SourceTrigger,
    effects: &[Sentence],
    intervening: Option<&SourceCondition>,
    block: usize,
) -> Result<TriggeredAbility, Failure> {
    if let SourceTrigger::PutIntoGraveyard { what, from, to } = trigger {
        if !matches!(what, TriggerObject::This {noun} if noun == "aura")
            || from != "battlefield"
            || *to != serde_json::json!({"zone":"graveyard","owner":"any"})
            || intervening.is_some()
            || !matches!(effects, [Sentence {steps, ..}] if matches!(steps.as_slice(), [Step::ReturnToHand {what,to,optional:None}] if *what == serde_json::json!({"ref":"it"}) && to == "owner"))
        {
            return Err(unsupported("trigger", "unsupported Aura return"));
        }
        return Ok(TriggeredAbility {
            death_filter: None,
            functioning_zone: FunctioningZone::Battlefield,
            condition: TriggerCondition::SelfAuraToGraveyard,
            intervening_if: None,
            program: SpellProgram {
                additional_sacrifice: None,
                timing: SpellTiming::Instant,
                resolution: SpellResolution::Instructions,
                functioning_zone: FunctioningZone::Stack,
                targets: vec![],
                instructions: vec![Instruction::ReturnAuraFromGraveyard {
                    source_block: block as u32,
                }],
                modes: vec![],
            },
        });
    }
    // The archive reference-binding object is diagnostic/nonexecutable. This
    // Rust semantic binder consumes the complete typed AST instead: "other"
    // excludes this exact source incarnation (CR 201.5), and the exact disjoint
    // union [this creature dies OR another creature dies] is every creature
    // death, once per actual move. It does not bind an unused event pronoun.
    // ETB checks the resulting battlefield; dies checks the prior battlefield
    // (CR 603.6a/603.10/700.4). Runtime captures those event/source facts.
    // https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
    let filtered = |what: &TriggerObject| -> Option<DeathFilter> {
        match what {
            TriggerObject::Filter { filter: f, count }
                if fixed_one(count)
                    && f.types == ["creature"]
                    && f.control.as_deref() == Some("you")
                    && f.non_subtypes.iter().all(|s| s == "Spirit")
                    && f.non_subtypes.len() <= 1 =>
            {
                Some(DeathFilter {
                    controlled: true,
                    other: f.other,
                    non_token: f.non_token,
                    non_subtypes: f.non_subtypes.clone(),
                })
            }
            _ => None,
        }
    };
    let death_filter = match trigger {
        SourceTrigger::Dies { what } => filtered(what),
        SourceTrigger::AnyOf { events } if matches!(events.as_slice(), [SourceTrigger::Dies { what }, SourceTrigger::Dies { .. }] if is_source_creature(what)) =>
        {
            let SourceTrigger::Dies { what } = &events[1] else {
                unreachable!()
            };
            if let Some(mut f) = filtered(what) {
                if !f.other || f.non_token || !f.non_subtypes.is_empty() {
                    return Err(unsupported("trigger", "non-disjoint dies union"));
                }
                f.other = false;
                Some(f)
            } else {
                None
            }
        }
        _ => None,
    };
    fn has_draw_quantity(effects: &[Sentence]) -> bool {
        effects
            .iter()
            .flat_map(|s| &s.steps)
            .any(|step| match step {
                Step::Draw {
                    amount: DrawAmount::ThatMuch {},
                    ..
                } => true,
                Step::Modal { options, .. } => {
                    options.iter().any(|o| has_draw_quantity(&o.effects))
                }
                _ => false,
            })
    }
    if has_draw_quantity(effects)
        && !matches!(effects, [Sentence {bindings, steps}] if bindings.is_empty()
            && matches!(steps.as_slice(), [Step::Draw {amount: DrawAmount::ThatMuch {}, ..}]))
    {
        // Prior effects may produce another quantity. This finite binder only
        // proves the standalone trigger-quantity reference; it never guesses
        // the antecedent of "that much" in an arbitrary instruction sequence.
        return Err(unsupported(
            "effects.draw",
            "damage quantity requires a standalone draw sentence",
        ));
    }
    let condition=match trigger {
        SourceTrigger::DealsDamage { what:SourceDamageObject::Filter {filter,count},combat:Some(true),to,grouping:Some(grouping) }
            if grouping == "one-or-more" && *to == serde_json::json!("any")
                && fixed_one(count) && filter.types == ["creature"]
                && filter.control.as_deref() == Some("you") && filter.non_types.is_none()
                && filter.all_of.is_none() && filter.any_of.is_none()
                && filter.properties.as_deref() == Some(&[serde_json::json!({"property":"power","comparison":{"op":"gt","value":{"amount":"attribute","of":{"ref":"it"},"attribute":"base-power"}}})]) => {
            // CR 208.4b/603.2c/603.10: the property's "it" is EACH candidate
            // creature, not the observing source. Diagnostic references remain
            // unchanged even when their earlier binder chose another object.
            TriggerCondition::GroupedDamageDealt {
                source:DamageTriggerSource::CreatureFilter {controller:DamageSourceRelation::Controller,types:TypePredicate::Type {kind:CardKind::Creature}},
                kind:TriggerDamageKind::Combat,recipient:DamageTriggerRecipient::Player,source_power:DamageSourcePowerPredicate::GreaterThanBase,
            }
        },
        SourceTrigger::DealsDamage { what, combat: Some(true), to, grouping:None }
            if *to == serde_json::json!("any") => {
            TriggerCondition::DamageDealt { source: lower_damage_subject(what)?,
                kind: TriggerDamageKind::Combat, recipient: DamageTriggerRecipient::Player }
        },
        SourceTrigger::DealsDamage { what, combat: None, to, grouping:None }
            if *to == serde_json::json!({"player":"an-opponent"}) => {
            TriggerCondition::DamageDealt { source: lower_damage_subject(what)?,
                kind: TriggerDamageKind::Any, recipient: DamageTriggerRecipient::OpponentPlayer }
        },
        SourceTrigger::Attacks { what } => {
            let attacker = lower_attack_subject(what)?;
            // CR 201.5/508.3a: the event's "it" is the attacker, while the
            // executable damage source is this invocation's source. Equality
            // is proved only for the exact self-attacker pattern.
            if !matches!(attacker, AttackSubject::Source {})
                && effects.iter().flat_map(|s| &s.steps).any(|s|
                    matches!(s, Step::Damage { source: DamageSource::It {}, .. }))
            {
                return Err(unsupported("abilities.effects.source", "event-attacker damage source is unsupported"));
            }
            TriggerCondition::CreatureAttacked { attacker }
        },
        SourceTrigger::Cast { who, what } => {
            if who.selection_id.is_some() || who.of.is_some() { return Err(unsupported("trigger.who", "unsupported caster qualifier")); }
            let caster = match who.player.as_str() {
                "you" => CasterRelation::Controller,
                "a-player" => CasterRelation::AnyPlayer,
                "an-opponent" => CasterRelation::Opponent,
                _ => return Err(unsupported("trigger.who", "unsupported caster relation")),
            };
            TriggerCondition::SpellCast { caster, spell: lower_type_predicate(what, Some("spell"), true, 0)? }
        },
        SourceTrigger::Phase { phase, whose } => {
            let step = match phase.as_str() {
                "upkeep" => BeginningStep::Upkeep,
                "draw-step" => BeginningStep::Draw,
                "end-step" => BeginningStep::End,
                _ => return Err(unsupported("abilities.trigger.phase", "unsupported beginning step")),
            };
            let turn_relation = match whose.as_str() {
                Some("your") => TurnRelation::Controller,
                Some("each-player") => TurnRelation::AnyPlayer,
                Some("each-opponent") => TurnRelation::Opponent,
                _ => return Err(unsupported("abilities.trigger.whose", "unsupported turn relation")),
            };
            TriggerCondition::StepBegan { step, turn_relation }
        },
        SourceTrigger::Dies { what: TriggerObject::Filter { filter, count } } if fixed_one(count) && filter.types == ["creature"] && filter.control.as_deref() == Some("opponent") && !filter.other && !filter.non_token && filter.non_subtypes.is_empty() => TriggerCondition::OpponentCreatureDies,
        _ if death_filter.is_some() => TriggerCondition::AnyCreatureDies,
        SourceTrigger::Enters { what: TriggerObject::This { noun } } if noun == "enchantment" => TriggerCondition::SelfEntersBattlefield,
        SourceTrigger::Dies {what} if is_source_creature(what)=>TriggerCondition::SelfDies,
        SourceTrigger::Enters {what} if is_source_creature(what)=>TriggerCondition::SelfEntersBattlefield,
        SourceTrigger::Enters {what} if is_other_creature(what)=>TriggerCondition::OtherCreatureEntersBattlefield,
        SourceTrigger::AnyOf {events} if events.len()==2
            && events.iter().filter(|event|matches!(event,SourceTrigger::Dies {what} if is_source_creature(what))).count()==1
            && events.iter().filter(|event|matches!(event,SourceTrigger::Dies {what} if is_other_creature(what))).count()==1 =>TriggerCondition::AnyCreatureDies,
        _=>return Err(unsupported("abilities.trigger","unsupported complete event/source/other/alternative trigger binding")),
    };
    if matches!(condition, TriggerCondition::GroupedDamageDealt { .. })
        && (intervening.is_some()
            || !matches!(effects, [Sentence {bindings,steps}] if bindings.is_empty()
            && matches!(steps.as_slice(), [Step::Draw {who,amount:DrawAmount::Fixed {value:1},optional:false}]
                if who.player == "you" && who.of.is_none() && who.selection_id.is_none())))
    {
        return Err(unsupported(
            "effects.grouped_damage",
            "grouped damage requires one standalone mandatory controller draw",
        ));
    }
    let mut program = SpellProgram {
        additional_sacrifice: None,
        timing: SpellTiming::Instant,
        resolution: SpellResolution::Instructions,
        functioning_zone: FunctioningZone::Stack,
        targets: vec![],
        instructions: vec![],
        modes: vec![],
    };
    append_spell_effects(
        &mut program,
        effects,
        block,
        &mut BTreeMap::new(),
        if matches!(
            condition,
            TriggerCondition::StepBegan { .. } | TriggerCondition::SpellCast { .. }
        ) {
            PlayerBinding::EventPlayer
        } else if matches!(condition, TriggerCondition::DamageDealt { .. }) {
            PlayerBinding::DamageRecipient
        } else if condition == TriggerCondition::OpponentCreatureDies {
            PlayerBinding::DeathController
        } else {
            PlayerBinding::Unbound
        },
    )?;
    let intervening_if = match intervening {
        None => None,
        Some(SourceCondition::CardsInHand { who, comparison })
            if (condition == TriggerCondition::SelfDies
                || matches!(condition, TriggerCondition::StepBegan { .. }))
                && who.player == "you"
                && who.selection_id.is_none()
                && who.of.is_none()
                && comparison.op == "eq"
                && comparison.value.amount == "fixed"
                && comparison.value.value == 0 =>
        {
            Some(InterveningCondition::ControllerHandEmpty)
        }
        Some(SourceCondition::Status {
            what: TriggerObject::This { noun },
            status,
        }) if matches!(condition, TriggerCondition::StepBegan { .. })
            && matches!(
                noun.as_str(),
                "artifact" | "creature" | "enchantment" | "permanent"
            ) =>
        {
            Some(InterveningCondition::SourceStatus { status: *status })
        }
        _ => {
            return Err(unsupported(
                "abilities.condition",
                "unsupported complete intervening-if condition",
            ));
        }
    };
    Ok(TriggeredAbility {
        death_filter,
        intervening_if,
        functioning_zone: FunctioningZone::Battlefield,
        condition,
        program,
    })
}

fn lower_permanent_costs(
    costs: &[SourceCost],
    kind: CardKind,
) -> Result<PermanentActivationCosts, Failure> {
    let mut result = PermanentActivationCosts {
        tap: false,
        mana: None,
        sacrifice: false,
    };
    for cost in costs {
        match cost {
            SourceCost::TapSelf {} if !result.tap => result.tap = true,
            SourceCost::Mana { symbols }
                if result.mana.is_none()
                    && !symbols.is_empty()
                    && symbols
                        .iter()
                        .all(|s| !s.is_empty() && !s.contains(['{', '}'])) =>
            {
                let m = mana_cost(
                    &symbols
                        .iter()
                        .map(|s| format!("{{{s}}}"))
                        .collect::<String>(),
                )?;
                if m.colored.contains(&Color::C) {
                    return Err(unsupported("cost", "colorless-specific costs excluded"));
                }
                result.mana = Some(m);
            }
            SourceCost::Sacrifice { what }
                if !result.sacrifice
                    && *what
                        == serde_json::json!({"ref":"this","noun":match kind {
                            CardKind::Artifact => "artifact",
                            CardKind::Land => "land",
                            _ => "creature",
                        }}) =>
            {
                result.sacrifice = true
            }
            _ => {
                return Err(unsupported(
                    "cost",
                    "unsupported complete permanent activation costs",
                ));
            }
        }
    }
    Ok(result)
}
fn lower_mana_ability(
    costs: &[SourceCost],
    effects: &[Sentence],
    block: usize,
    kind: CardKind,
) -> Result<Option<ManaAbility>, Failure> {
    if effects.iter().any(|s| !s.bindings.is_empty()) {
        return Err(unsupported(
            "bindings",
            "bindings outside spell instructions",
        ));
    }
    if !effects
        .iter()
        .flat_map(|s| &s.steps)
        .any(|s| matches!(s, Step::AddMana { .. }))
    {
        return Ok(None);
    }
    let [Sentence { steps, .. }] = effects else {
        return Err(unsupported("mana", "extra mana effects"));
    };
    let [Step::AddMana { who, mana }] = steps.as_slice() else {
        return Err(unsupported("mana", "extra mana steps"));
    };
    let you = PlayerSelector {
        player: "you".into(),
        selection_id: None,
        of: None,
    };
    if *who != you {
        return Err(unsupported("mana", "unsupported mana recipient"));
    }
    let production = match mana {
        SourceMana::Fixed { symbols } if !symbols.is_empty() => ManaProduction::Fixed {
            symbols: symbols.clone(),
        },
        SourceMana::AnyColor { amount } if fixed_one(amount) => ManaProduction::AnyColor,
        SourceMana::CommanderColor { amount, whose } if fixed_one(amount) && *whose == you => {
            ManaProduction::CommanderColor
        }
        _ => return Err(unsupported("mana", "unsupported complete mana production")),
    };
    let costs = lower_permanent_costs(costs, kind)?;
    if !costs.tap {
        return Err(unsupported("mana", "mana ability requires tap-self"));
    }
    Ok(Some(ManaAbility {
        costs,
        production,
        source_block: source_block(block)?,
    }))
}

fn lower_activation(
    costs: &[SourceCost],
    effects: &[Sentence],
    block: usize,
    kind: CardKind,
) -> Result<ActivatedAbility, Failure> {
    if effects.iter().any(|s| !s.bindings.is_empty()) {
        return Err(unsupported(
            "bindings",
            "bindings outside spell instructions",
        ));
    }
    let cost = match costs {
        [SourceCost::Sacrifice { what }] if what["ref"] == "filter" => {
            ActivationCost::SacrificePermanent {
                filter: lower_sacrifice_filter(what)?,
                mana: None,
            }
        }
        [SourceCost::Mana { symbols }, SourceCost::Sacrifice { what }]
            if what["ref"] == "filter" =>
        {
            let mana = lower_permanent_costs(
                &[SourceCost::Mana {
                    symbols: symbols.clone(),
                }],
                kind,
            )?
            .mana;
            ActivationCost::SacrificePermanent {
                filter: lower_sacrifice_filter(what)?,
                mana,
            }
        }
        [SourceCost::Sacrifice { what }]
            if *what == serde_json::json!({"ref":"this","noun":"creature"}) =>
        {
            ActivationCost::SacrificeSource
        }
        [SourceCost::TapSelf {}] => ActivationCost::TapSource,
        [SourceCost::Mana { symbols }] if !symbols.is_empty() => {
            // Each parser symbol is one complete Scryfall mana symbol. Reject
            // injected braces rather than interpreting extra unparsed symbols.
            if symbols
                .iter()
                .any(|symbol| symbol.is_empty() || symbol.contains(['{', '}']))
            {
                return Err(unsupported(
                    "abilities.costs",
                    "invalid complete mana-cost symbol",
                ));
            }
            let text = symbols
                .iter()
                .map(|symbol| format!("{{{symbol}}}"))
                .collect::<String>();
            ActivationCost::Mana {
                cost: mana_cost(&text)?,
            }
        }
        _ if kind == CardKind::Artifact
            || matches!(costs, [SourceCost::Mana { .. }, SourceCost::Sacrifice { what }] if kind == CardKind::Creature && *what == serde_json::json!({"ref":"this","noun":"creature"})) =>
        {
            ActivationCost::Composite {
                costs: lower_permanent_costs(costs, kind)?,
            }
        }
        _ => {
            return Err(unsupported(
                "abilities.costs",
                "unsupported complete activation cost or combined costs",
            ));
        }
    };
    let source_block = source_block(block)?;
    let mut program = SpellProgram {
        additional_sacrifice: None,
        timing: SpellTiming::Instant,
        resolution: SpellResolution::Instructions,
        functioning_zone: FunctioningZone::Stack,
        targets: vec![],
        instructions: vec![],
        modes: vec![],
    };
    if matches!(cost, ActivationCost::SacrificePermanent { .. })
        || (matches!(cost, ActivationCost::SacrificeSource)
            && effects.iter().flat_map(|s| &s.steps).any(|i| {
                matches!(
                    i,
                    Step::Damage {
                        source: DamageSource::It {},
                        ..
                    } | Step::Search { .. }
                )
            }))
    {
        append_spell_effects(
            &mut program,
            effects,
            block,
            &mut BTreeMap::new(),
            PlayerBinding::Unbound,
        )?;
        return Ok(ActivatedAbility {
            functioning_zone: FunctioningZone::Battlefield,
            cost,
            program,
        });
    }
    if kind == CardKind::Creature && matches!(cost, ActivationCost::Composite { .. }) {
        append_spell_effects(
            &mut program,
            effects,
            block,
            &mut BTreeMap::new(),
            PlayerBinding::Unbound,
        )?;
        if !matches!(
            program.instructions.as_slice(),
            [Instruction::DestroyPermanent {
                optional: false,
                ..
            }]
        ) {
            return Err(unsupported(
                "activation",
                "unsupported targeted sacrifice program",
            ));
        }
        return Ok(ActivatedAbility {
            functioning_zone: FunctioningZone::Battlefield,
            cost,
            program,
        });
    }
    let mut slots = BTreeMap::new();
    for sentence in effects {
        if sentence.steps.is_empty() {
            return Err(invalid(
                "effects.steps",
                "empty activated instruction group",
            ));
        }
        for step in &sentence.steps {
            let you = PlayerSelector {
                player: "you".into(),
                selection_id: None,
                of: None,
            };
            let instruction = match step {
                Step::CreateReplacement { .. } => {
                    return Err(unsupported("replacement", "unsupported replacement"));
                }
                Step::PreventDamage {
                    prevention,
                    duration,
                } => {
                    lower_prevention(&mut program, &mut slots, prevention, duration, source_block)?
                }
                Step::Damage {
                    optional: None,
                    source: DamageSource::This { noun },
                    amount,
                    to,
                } if noun == "creature" => {
                    let DamageDestination::AnyTarget { selection_id } = to else {
                        return Err(unsupported("damage", "unsupported damage destination"));
                    };
                    Instruction::DealDamage {
                        target_slot: bind_target(
                            &mut program,
                            &mut slots,
                            selection_id,
                            TargetDomain::AnyDamageTarget,
                        )?,
                        amount: fixed_positive_value(amount)?,
                        source_block,
                    }
                }
                Step::Draw {
                    who,
                    amount,
                    optional: true,
                } if *who == you && amount.fixed_one() => {
                    Instruction::OptionalDrawCards { source_block }
                }
                Step::Draw {
                    who,
                    amount,
                    optional: false,
                } if *who == you && amount.fixed_one() => Instruction::DrawCards {
                    recipient: None,
                    count: Quantity::from(1),
                    source_block,
                },
                Step::Discard { who, what }
                    if *who == you
                        && what.reference == "filter"
                        && fixed_one(&what.count)
                        && what.filter
                            == ObjectFilter::Class(CardFilter { cls: "card".into() }) =>
                {
                    Instruction::DiscardCards {
                        recipient: None,
                        count: Quantity::from(1),
                        source_block,
                    }
                }
                _ => {
                    return Err(unsupported(
                        "effects.steps",
                        "unsupported activated operator, controller or cardinality",
                    ));
                }
            };
            program.instructions.push(instruction);
        }
    }
    if program.instructions.is_empty() {
        return Err(unsupported("abilities.effects", "empty activated effects"));
    }
    // CR 602.1–2 / 608.2c: the cost is paid while activating; the ordered
    // program resolves on the stack independently of its sacrificed source.
    // https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
    Ok(ActivatedAbility {
        functioning_zone: FunctioningZone::Battlefield,
        cost,
        program,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn strict_mirror_reports_specific_nodes_keywords_and_fields() {
        for (raw, expected) in [
            (
                r#"[{"kind":"future-ability","payload":{}}]"#,
                "kind:future-ability",
            ),
            (
                // Combat damage is supported; this broader player event is not.
                r#"[{"kind":"triggered","trigger":{"trigger":"player-dealt-damage"},"effects":[]}]"#,
                "trigger:player-dealt-damage",
            ),
            (
                r#"[{"kind":"spell","effects":[{"steps":[{"effect":"gain-abilities"}]}]}]"#,
                "effect:gain-abilities",
            ),
            (
                r#"[{"kind":"keywords","keywords":[{"keyword":"ward","cost":[]}]}]"#,
                "keyword:ward",
            ),
            (
                r#"[{"kind":"keywords","keywords":[{"keyword":"flying","extra":true}]}]"#,
                "keyword:flying:field:extra",
            ),
        ] {
            let failure = read_abilities(raw).unwrap_err();
            assert!(failure.unsupported);
            assert_eq!(failure.reason, expected);
        }
        let failure =
            read_abilities(r#"[{"kind":"spell","effects":[],"unknownClause":true}]"#).unwrap_err();
        assert!(failure.unsupported && failure.reason.contains("unknownClause"));
        let failure =
            read_abilities(r#"[{"kind":"keywords","keywords":[],"keywords":[]}]"#).unwrap_err();
        assert!(!failure.unsupported, "duplicates remain invalid");
        let raw = fixtures::COMBAT_KEYWORD_RECORDS
            .iter()
            .find(|(name, _)| *name == "youthful-knight")
            .unwrap()
            .1;
        let alias = changed_source(raw, |r| {
            r["faces"][0]["result"]["abilities"][0]["keywords"][0]["keyword"] =
                "first-strike".into();
            r["faces"][0]["result"]["lines"][0]["ability"] =
                r["faces"][0]["result"]["abilities"][0].clone();
        });
        let CompileResult::Lowered { definition } = compile(fixtures::MANIFEST, &alias) else {
            panic!("implemented keyword alias must lower");
        };
        assert_eq!(definition.keywords, [KeywordAbility::FirstStrike]);
    }

    #[test]
    fn combined_types_preserve_all_types_and_reject_missing_runtime_types() {
        for line in [
            "Artifact Creature — Bear",
            "Legendary Artifact Creature — Bear",
            "Enchantment Creature — Bear",
            "Legendary Enchantment Creature — Bear",
            "Artifact Enchantment Creature — Bear",
            "Creature Artifact — Bear",
        ] {
            let raw = changed_record(1, |r| {
                r["source"]["type_line"] = line.into();
            });
            let CompileResult::Lowered { definition } = compile(fixtures::MANIFEST, &raw) else {
                panic!("{line}");
            };
            definition.validate().unwrap();
            assert!(definition.characteristics.has_type(CardKind::Creature));
            assert_eq!(
                definition.characteristics.has_type(CardKind::Artifact),
                line.contains("Artifact")
            );
            assert_eq!(
                definition.characteristics.has_type(CardKind::Enchantment),
                line.contains("Enchantment")
            );
            assert_eq!(
                definition.characteristics.legendary,
                line.contains("Legendary")
            );
            let mut bad = *definition;
            bad.characteristics.additional_types.push(CardKind::Land);
            bad.capabilities = bad.requirements();
            assert!(bad.validate().is_err());
        }
        let raw = fixtures::EFFECT_RECORDS
            .iter()
            .find(|(name, _)| *name == "furnace-of-rath")
            .unwrap()
            .1;
        let raw = changed_source(raw, |r| {
            r["source"]["type_line"] = "Legendary Enchantment".into();
        });
        assert!(matches!(
            compile(fixtures::MANIFEST, &raw),
            CompileResult::Lowered { .. }
        ));
        // Spell subtypes are retained metadata. Their printed abilities still
        // have to lower; Arcane/Lesson do not add an implicit runtime action.
        for subtype in ["Arcane", "Lesson"] {
            let raw = changed_record(3, |r| {
                r["source"]["type_line"] = format!("Instant — {subtype}").into();
            });
            let CompileResult::Lowered { definition } = compile(fixtures::MANIFEST, &raw) else {
                panic!("inert spell subtype {subtype}");
            };
            assert_eq!(definition.characteristics.subtypes, [subtype]);
            definition.validate().unwrap();
        }
        for (line, reason) in [
            ("Planeswalker — Ada", "Planeswalker"),
            ("Battle — Siege", "Battle"),
            ("World Enchantment", "World"),
        ] {
            let raw = changed_record(1, |r| {
                r["source"]["type_line"] = line.into();
            });
            assert!(
                matches!(compile(fixtures::MANIFEST, &raw), CompileResult::Unsupported { reason: actual, .. } if actual == reason)
            );
        }
    }

    #[test]
    fn multi_face_layout_is_unsupported_without_single_face_defaults() {
        let raw = changed_record(1, |r| {
            r["source"]["layout"] = "transform".into();
            r["source"]["card_faces"] = serde_json::json!([{}, {}]);
            r["source"].as_object_mut().unwrap().remove("oracle_text");
            r["source"].as_object_mut().unwrap().remove("mana_cost");
        });
        assert!(
            matches!(compile(fixtures::MANIFEST, &raw), CompileResult::Unsupported { reason, .. } if reason == "multi_face_layout:transform")
        );
        let mut bad: serde_json::Value = serde_json::from_str(&raw).unwrap();
        bad["sourceSha256"] = "0".repeat(64).into();
        assert!(matches!(
            compile(fixtures::MANIFEST, &bad.to_string()),
            CompileResult::Invalid { .. }
        ));
        let raw = changed_record(1, |r| {
            r["source"].as_object_mut().unwrap().remove("oracle_text");
        });
        assert!(matches!(
            compile(fixtures::MANIFEST, &raw),
            CompileResult::Invalid { .. }
        ));
    }

    #[test]
    fn source_normalization_accounts_for_every_clause_and_literal_name() {
        assert_eq!(source_lines("When Aether Bear enters, put a counter on Aether Bear. Aether Bear's power is 2.\n(Outer (nested) reminder) Flying", "Aether Bear", "Creature").unwrap(), ["When ~ enters, put a counter on ~. ~'s power is 2.", "Flying"]);
        assert_eq!(
            source_lines(
                "When Ada enters, Ada, the Wise draws a card.",
                "Ada, the Wise",
                "Legendary Creature"
            )
            .unwrap(),
            ["When ~ enters, ~ draws a card."]
        );
        assert_eq!(
            source_lines("Fearsome fear Fear’s power is 1.", "Fear", "Creature").unwrap(),
            ["Fearsome fear ~'s power is 1."]
        );
        assert_eq!(
            source_lines(
                "Search for cards named Squadron Hawk.",
                "Squadron Hawk",
                "Creature"
            )
            .unwrap(),
            ["Search for cards named Squadron Hawk."]
        );
        assert_eq!(
            source_lines(
                "When Ada enters, target Ada planeswalker gains loyalty.",
                "Ada, the Wise",
                "Legendary Creature"
            )
            .unwrap(),
            ["When ~ enters, target Ada planeswalker gains loyalty."]
        );
        assert!(source_lines("Flying (unclosed", "Bear", "Creature").is_err());
        // The expected line is still byte-exact after normalization.
        let raw = changed_record(3, |r| {
            r["faces"][0]["result"]["lines"][0]["text"] = "When ~ enters, draw two cards.".into();
        });
        assert!(
            matches!(compile(fixtures::MANIFEST, &raw), CompileResult::Invalid { path, .. } if path == "lines")
        );
    }

    #[test]
    fn wipes_complete_source_pairs() {
        let c = embedded_catalog("wipes").unwrap();
        assert_eq!(c.schema_version, CURRENT_IR_VERSION);
        assert_eq!(
            c,
            Catalog::read(include_str!(
                "../../../tests/fixtures/expected-wipes-ir.json"
            ))
            .unwrap()
        );
        for (name, raw) in fixtures::WIPES_RECORDS {
            let CompileResult::Lowered { definition } = compile(fixtures::MANIFEST, raw) else {
                panic!("{name}")
            };
            assert!(c.definitions.contains(&definition), "{name}");
        }
        for raw in [
            include_str!("../../../tests/fixtures/austere-command.json"),
            include_str!("../../../tests/fixtures/fiery-confluence.json"),
            include_str!("../../../tests/fixtures/bontus-last-reckoning.json"),
            include_str!("../../../tests/fixtures/rolling-earthquake.json"),
        ] {
            assert!(!matches!(
                compile(fixtures::MANIFEST, raw),
                CompileResult::Lowered { .. }
            ));
        }
    }
    #[test]
    fn sacrifice_complete_sources_and_whole_card_negatives() {
        let c = embedded_catalog("sacrifice").unwrap();
        assert_eq!(c.schema_version, CURRENT_IR_VERSION);
        assert_eq!(
            c,
            Catalog::read(include_str!(
                "../../../tests/fixtures/expected-sacrifice-ir.json"
            ))
            .unwrap()
        );
        for (name, raw) in fixtures::SACRIFICE_RECORDS {
            let CompileResult::Lowered { definition } = compile(fixtures::MANIFEST, raw) else {
                panic!("{name}")
            };
            assert!(c.definitions.contains(&definition), "{name}");
        }
        for raw in [
            include_str!("../../../tests/fixtures/ashnods-altar.json"),
            include_str!("../../../tests/fixtures/morbid-opportunist.json"),
            include_str!("../../../tests/fixtures/young-wolf.json"),
            include_str!("../../../tests/fixtures/pitiless-plunderer.json"),
        ] {
            assert!(!matches!(
                compile(fixtures::MANIFEST, raw),
                CompileResult::Lowered { .. }
            ));
        }
    }
    #[test]
    fn sacrifice_complete_filters_amounts_and_optional_clauses_cannot_disappear() {
        for (name, path, value) in [
            (
                "village-rites",
                "/costs/0/what/filter/control",
                serde_json::json!("opponent"),
            ),
            (
                "siege-gang-commander",
                "/costs/1/what/filter/other",
                serde_json::json!(true),
            ),
            (
                "midnight-reaper",
                "/trigger/what/filter/nonColors",
                serde_json::json!(["black"]),
            ),
            (
                "goblin-arsonist",
                "/effects/0/steps/0/optionalExtra",
                serde_json::json!(true),
            ),
            (
                "fling",
                "/effects/0/steps/0/amount/attribute",
                serde_json::json!("toughness"),
            ),
        ] {
            let raw = fixtures::SACRIFICE_RECORDS
                .iter()
                .find(|r| r.0 == name)
                .unwrap()
                .1;
            let bad = changed_source(raw, |r| {
                let index = if matches!(name, "siege-gang-commander" | "fling") {
                    1
                } else {
                    0
                };
                let a = &mut r["faces"][0]["result"]["abilities"][index];
                let (parent, key) = path.rsplit_once('/').unwrap();
                a.pointer_mut(parent)
                    .unwrap()
                    .as_object_mut()
                    .unwrap()
                    .insert(key.into(), value);
                r["faces"][0]["result"]["lines"][index]["ability"] =
                    r["faces"][0]["result"]["abilities"][index].clone();
            });
            assert!(
                !matches!(
                    compile(fixtures::MANIFEST, &bad),
                    CompileResult::Lowered { .. }
                ),
                "{name}"
            );
        }
    }
    #[test]
    fn removal_complete_source_pairs() {
        let c = embedded_catalog("removal").unwrap();
        assert_eq!(c.schema_version, CURRENT_IR_VERSION);
        assert_eq!(
            c,
            Catalog::read(include_str!(
                "../../../tests/fixtures/expected-removal-ir.json"
            ))
            .unwrap()
        );
        for (name, raw) in fixtures::REMOVAL_RECORDS {
            let CompileResult::Lowered { definition } = compile(fixtures::MANIFEST, raw) else {
                panic!("{name}")
            };
            assert!(c.definitions.contains(&definition), "{name}");
        }
        for raw in [
            include_str!("../../../tests/fixtures/return-to-dust.json"),
            include_str!("../../../tests/fixtures/vandalblast.json"),
            include_str!("../../../tests/fixtures/banishing-light.json"),
        ] {
            assert!(!matches!(
                compile(fixtures::MANIFEST, raw),
                CompileResult::Lowered { .. }
            ));
        }
        let throat = c
            .definitions
            .iter()
            .find(|d| d.name == "Go for the Throat")
            .unwrap();
        assert_eq!(
            throat.spell.as_ref().unwrap().targets[0]
                .filter
                .as_ref()
                .unwrap()
                .non_types,
            [CardKind::Artifact]
        );
        let pongify = c.definitions.iter().find(|d| d.name == "Pongify").unwrap();
        assert!(matches!(
            pongify.spell.as_ref().unwrap().instructions[0],
            Instruction::DestroyPermanent {
                regeneration_forbidden: true,
                ..
            }
        ));
    }
    #[test]
    fn removal_whole_card_mutants_cannot_lose_filters_references_or_flags() {
        for (name, field, value) in [
            (
                "swords-to-plowshares",
                "/effects/0/steps/0/regenerationForbidden",
                serde_json::json!(true),
            ),
            (
                "disenchant",
                "/effects/0/steps/0/what/filter/power",
                serde_json::json!(4),
            ),
            (
                "doom-blade",
                "/effects/0/steps/0/what/filter/nonColors",
                serde_json::json!(["purple"]),
            ),
            (
                "feed-the-swarm",
                "/effects/1/steps/0/amount/attribute",
                serde_json::json!("toughness"),
            ),
            (
                "generous-gift",
                "/effects/1/steps/0/who/of",
                serde_json::json!({"ref":"them"}),
            ),
        ] {
            let raw = fixtures::REMOVAL_RECORDS
                .iter()
                .find(|r| r.0 == name)
                .unwrap()
                .1;
            let bad = changed_source(raw, |r| {
                let a = &mut r["faces"][0]["result"]["abilities"][0];
                // Paths include new fields, so use the object parent explicitly.
                let (parent, key) = field.rsplit_once('/').unwrap();
                a.pointer_mut(parent)
                    .unwrap()
                    .as_object_mut()
                    .unwrap()
                    .insert(key.into(), value);
                r["faces"][0]["result"]["lines"][0]["ability"] =
                    r["faces"][0]["result"]["abilities"][0].clone();
            });
            assert!(
                !matches!(
                    compile(fixtures::MANIFEST, &bad),
                    CompileResult::Lowered { .. }
                ),
                "{name}"
            );
        }
        let c = embedded_catalog("removal").unwrap();
        let mut raw = serde_json::to_value(
            c.definitions
                .iter()
                .find(|d| d.name == "Swords to Plowshares")
                .unwrap(),
        )
        .unwrap();
        raw["spell"]["instructions"][0]["regeneration_forbidden"] = true.into();
        assert!(serde_json::from_value::<Definition>(raw).is_err());
    }
    #[test]
    fn attachments_complete_sources_and_strict_keyword_shapes() {
        let c = embedded_catalog("attachments").unwrap();
        assert_eq!(c.schema_version, CURRENT_IR_VERSION);
        assert_eq!(
            c,
            Catalog::read(include_str!(
                "../../../tests/fixtures/expected-attachments-ir.json"
            ))
            .unwrap()
        );
        for (name, raw) in fixtures::ATTACHMENT_RECORDS {
            let CompileResult::Lowered { definition } = compile(fixtures::MANIFEST, raw) else {
                panic!("{name}")
            };
            assert!(c.definitions.contains(&definition), "{name}");
        }
        for raw in [
            include_str!("../../../tests/fixtures/blanchwood-armor.json"),
            include_str!("../../../tests/fixtures/gift-of-orzhova.json"),
            include_str!("../../../tests/fixtures/arrest.json"),
            include_str!("../../../tests/fixtures/mask-of-memory.json"),
        ] {
            assert!(!matches!(
                compile(fixtures::MANIFEST, raw),
                CompileResult::Lowered { .. }
            ));
        }
        let get = |name| c.definitions.iter().find(|d| d.name == name).unwrap();
        assert_eq!(
            get("Pacifism").characteristics.mana_cost.generic,
            Quantity::from(1)
        );
        assert_eq!(
            get("Spidersilk Armor").characteristics.mana_cost.generic,
            Quantity::from(2)
        );
        assert!(
            matches!(&get("Lightning Greaves").activated[0].cost, ActivationCost::Mana {cost} if cost.generic==Quantity::zero())
        );
        for keyword in [
            "defender",
            "flying",
            "reach",
            "vigilance",
            "haste",
            "lifelink",
            "deathtouch",
            "trample",
            "first strike",
            "double strike",
            "menace",
            "hexproof",
            "shroud",
        ] {
            let k: KeywordAbility = serde_json::from_value(serde_json::json!(keyword)).unwrap();
            let source = SourceStaticEffect::Modify {
                what: serde_json::json!({"ref":"equipped","noun":"creature"}),
                modifiers: vec![
                    serde_json::json!({"modifier":"abilities","abilities":[{"gained":"keyword","keyword":{"keyword":keyword}}]}),
                ],
            };
            assert!(
                matches!(lower_static(&source).unwrap().effect, StaticEffect::ModifyCreatures {keywords,..} if keywords == [k])
            );
            let mut intrinsic = get("Storm Crow").clone();
            intrinsic.keywords = vec![k];
            intrinsic.capabilities = intrinsic.requirements();
            intrinsic.validate().unwrap();
        }
        for keyword in [KeywordAbility::Hexproof, KeywordAbility::Shroud] {
            let mut token_spell = get("Raise the Alarm").clone();
            let Instruction::CreateCreatureTokens { token, .. } =
                &mut token_spell.spell.as_mut().unwrap().instructions[0]
            else {
                panic!("token program")
            };
            token.keywords.push(keyword);
            token_spell.capabilities = token_spell.requirements();
            let mut probe = c.clone();
            probe.definitions = vec![token_spell];
            probe.validate().unwrap();
        }
    }
    #[test]
    fn four_colors_complete_sources_and_whole_card_negative() {
        let c = embedded_catalog("four-colors").unwrap();
        assert_eq!(c.schema_version, CURRENT_IR_VERSION);
        assert_eq!(
            c,
            Catalog::read(include_str!(
                "../../../tests/fixtures/expected-four-colors-ir.json"
            ))
            .unwrap()
        );
        for (name, raw) in fixtures::FOUR_COLORS_RECORDS {
            let CompileResult::Lowered { definition } = compile(fixtures::MANIFEST, raw) else {
                panic!("{name}")
            };
            let source: serde_json::Value = serde_json::from_str(raw).unwrap();
            assert_eq!(source["source"]["legalities"]["commander"], "legal");
            assert_eq!(definition.name, source["source"]["name"]);
        }
        assert!(matches!(
            compile(
                fixtures::MANIFEST,
                include_str!("../../../tests/fixtures/hunted-dragon.json")
            ),
            CompileResult::Unsupported { .. }
        ));
        let find = |name: &str| c.definitions.iter().find(|d| d.name == name).unwrap();
        assert_eq!(
            find("Caelorna, Coral Tyrant").characteristics.power,
            Some(0.into())
        );
        assert_eq!(
            find("Caelorna, Coral Tyrant").characteristics.toughness,
            Some(8.into())
        );
        assert_eq!(
            find("Rorix Bladewing").keywords,
            vec![KeywordAbility::Flying, KeywordAbility::Haste]
        );
        assert_eq!(
            find("Wall of Air").keywords,
            vec![KeywordAbility::Defender, KeywordAbility::Flying]
        );
    }

    #[test]
    fn mana_complete_sources_and_negatives() {
        let catalog = embedded_catalog("mana").unwrap();
        assert_eq!(catalog.schema_version, CURRENT_IR_VERSION);
        assert_eq!(
            catalog,
            Catalog::read(include_str!(
                "../../../tests/fixtures/expected-mana-ir.json"
            ))
            .unwrap()
        );
        for (name, raw) in fixtures::MANA_RECORDS {
            let CompileResult::Lowered { definition } = compile(fixtures::MANIFEST, raw) else {
                panic!("{name}")
            };
            assert!(catalog.definitions.contains(&definition), "{name}");
            let bad = changed_source(raw, |r| {
                let result = &mut r["faces"][0]["result"];
                let i = result["abilities"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .position(|a| a["kind"] == "activated")
                    .unwrap();
                result["abilities"][i]["extraManaClause"] = true.into();
                result["lines"][i]["ability"] = result["abilities"][i].clone();
            });
            assert!(
                !matches!(
                    compile(fixtures::MANIFEST, &bad),
                    CompileResult::Lowered { .. }
                ),
                "{name}"
            );
        }
        for raw in [
            include_str!("../../../tests/fixtures/fellwar-stone.json"),
            include_str!("../../../tests/fixtures/talisman-of-hierarchy.json"),
            include_str!("../../../tests/fixtures/utopia-sprawl.json"),
        ] {
            assert!(matches!(
                compile(fixtures::MANIFEST, raw),
                CompileResult::Unsupported { .. }
            ));
        }
        let find = |name: &str| catalog.definitions.iter().find(|d| d.name == name).unwrap();
        let tower = find("Command Tower");
        assert!(!tower.characteristics.basic && tower.characteristics.subtypes.is_empty());
        assert!(tower.characteristics.color_identity.is_empty());
        assert_eq!(
            find("Sol Ring").mana_abilities[0].production,
            ManaProduction::Fixed {
                symbols: vec![Color::C, Color::C]
            }
        );
        assert_eq!(
            find("Golgari Signet").mana_abilities[0]
                .costs
                .mana
                .as_ref()
                .unwrap()
                .generic,
            Quantity::from(1)
        );
        assert_eq!(find("Mind Stone").activated.len(), 1);
        assert_eq!(
            find("Birds of Paradise").keywords,
            vec![KeywordAbility::Flying]
        );
        // Rehashed near misses must fail semantic lowering, not provenance.
        for (field, value) in [
            ("who", serde_json::json!({"player":"opponent"})),
            (
                "mana",
                serde_json::json!({"mana":"any-color","amount":{"amount":"fixed","value":2}}),
            ),
        ] {
            let raw = fixtures::MANA_RECORDS[1].1;
            let bad = changed_source(raw, |r| {
                let result = &mut r["faces"][0]["result"];
                result["abilities"][0]["effects"][0]["steps"][0][field] = value;
                result["lines"][0]["ability"] = result["abilities"][0].clone();
            });
            assert!(!matches!(
                compile(fixtures::MANIFEST, &bad),
                CompileResult::Lowered { .. }
            ));
        }
    }

    #[test]
    fn staples_complete_source_pairs() {
        let catalog = embedded_catalog("staples").unwrap();
        assert_eq!(catalog.schema_version, CURRENT_IR_VERSION);
        assert_eq!(
            catalog,
            Catalog::read(include_str!(
                "../../../tests/fixtures/expected-staples-ir.json"
            ))
            .unwrap()
        );
        for (name, raw) in fixtures::STAPLES_RECORDS {
            let CompileResult::Lowered { definition } = compile(fixtures::MANIFEST, raw) else {
                panic!("{name}")
            };
            assert_eq!(
                catalog
                    .definitions
                    .iter()
                    .find(|d| d.oracle_id == definition.oracle_id),
                Some(definition.as_ref())
            );
            let bad = changed_source(raw, |r| {
                let result = &mut r["faces"][0]["result"];
                result["abilities"][0]["unsupportedStaplesClause"] = true.into();
                result["lines"][0]["ability"] = result["abilities"][0].clone();
            });
            assert!(
                !matches!(
                    compile(fixtures::MANIFEST, &bad),
                    CompileResult::Lowered { .. }
                ),
                "{name}"
            );
        }
        let raw = include_str!("../../../tests/fixtures/wrath-of-god.json");
        assert!(
            matches!(compile(fixtures::MANIFEST,raw),CompileResult::Unsupported {reason,..} if reason.contains("regeneration"))
        );
        // Unsupported restrictions, missing shuffle, untapped entry, and extra
        // anthem grants reject complete cards, even with recomputed provenance.
        for (name, path, value) in [
            (
                "murder",
                "/faces/0/result/abilities/0/effects/0/steps/0/what/filter/colors",
                serde_json::json!(["nonblack"]),
            ),
            (
                "rampant-growth",
                "/faces/0/result/abilities/0/effects/0/steps/1/tapped",
                serde_json::json!(false),
            ),
            (
                "explosive-vegetation",
                "/faces/0/result/abilities/0/effects/0/steps/0/for/count/value",
                serde_json::json!(3),
            ),
            (
                "glorious-anthem",
                "/faces/0/result/abilities/0/effect/what/filter/control",
                serde_json::json!("opponent"),
            ),
        ] {
            let raw = fixtures::STAPLES_RECORDS
                .iter()
                .find(|(n, _)| *n == name)
                .unwrap()
                .1;
            let bad = changed_source(raw, |r| {
                if path.ends_with("/colors") {
                    r["faces"][0]["result"]["abilities"][0]["effects"][0]["steps"][0]["what"]["filter"]
                        ["colors"] = value;
                } else {
                    *r.pointer_mut(path).unwrap() = value;
                }
                r["faces"][0]["result"]["lines"][0]["ability"] =
                    r["faces"][0]["result"]["abilities"][0].clone();
            });
            assert!(
                !matches!(
                    compile(fixtures::MANIFEST, &bad),
                    CompileResult::Lowered { .. }
                ),
                "{name}"
            );
        }
    }
    #[test]
    fn basic_land_search_rejects_qualified_search_and_shuffle_players() {
        for (name, raw, maximum) in [
            (
                "Rampant Growth",
                include_str!("../../../tests/fixtures/rampant-growth.json"),
                1,
            ),
            (
                "Explosive Vegetation",
                include_str!("../../../tests/fixtures/explosive-vegetation.json"),
                2,
            ),
            (
                "Sakura-Tribe Elder",
                include_str!("../../../tests/fixtures/sakura-tribe-elder.json"),
                1,
            ),
        ] {
            let CompileResult::Lowered { definition } = compile(fixtures::MANIFEST, raw) else {
                panic!("complete archived source must lower: {name}")
            };
            let program = definition
                .activated
                .first()
                .map(|a| &a.program)
                .or(definition.spell.as_ref())
                .unwrap();
            assert!(
                matches!(program.instructions.as_slice(), [Instruction::SearchBasicLands { maximum: actual, .. }] if *actual == maximum),
                "{name}"
            );
            // These are synthetic AST negatives over unchanged exact source.
            // Qualifying both actors must not bypass the actor-equality check.
            for steps in [&[0, 2][..], &[0][..], &[2][..]] {
                for (field, value) in [
                    ("of", serde_json::json!({"ref":"it"})),
                    ("selectionId", serde_json::json!("qualified-player")),
                ] {
                    let bad = changed_source(raw, |r| {
                        let result = &mut r["faces"][0]["result"];
                        for &step in steps {
                            result["abilities"][0]["effects"][0]["steps"][step]["who"][field] =
                                value.clone();
                        }
                        result["lines"][0]["ability"] = result["abilities"][0].clone();
                    });
                    assert!(
                        matches!(compile(fixtures::MANIFEST, &bad), CompileResult::Unsupported { path, .. } if path == "effects.search"),
                        "{name}, {field}, steps {steps:?}"
                    );
                }
            }
        }
    }
    #[test]
    fn target_opponent_sacrifice_rejects_qualified_player_and_chooser() {
        let raw = include_str!("../../../tests/fixtures/cruel-edict.json");
        let CompileResult::Lowered { definition } = compile(fixtures::MANIFEST, raw) else {
            panic!("complete archived Cruel Edict source must lower")
        };
        let program = definition.spell.as_ref().unwrap();
        assert!(matches!(
            program.instructions.as_slice(),
            [Instruction::TargetPlayerSacrificeCreature { target_slot: 0, .. }]
        ));
        assert_eq!(program.targets[0].domain, TargetDomain::OpponentPlayer);
        // Synthetic AST negatives: identical qualifications on both actors
        // must reject just as a qualification on either actor alone does.
        for actors in [&["who", "chosenBy"][..], &["who"][..], &["chosenBy"][..]] {
            let bad = changed_source(raw, |r| {
                let result = &mut r["faces"][0]["result"];
                for &actor in actors {
                    result["abilities"][0]["effects"][0]["steps"][0][actor]["of"] =
                        serde_json::json!({"ref":"it"});
                }
                result["lines"][0]["ability"] = result["abilities"][0].clone();
            });
            assert!(
                matches!(compile(fixtures::MANIFEST, &bad), CompileResult::Unsupported { path, .. } if path == "effects.steps"),
                "qualified sacrifice actors: {actors:?}"
            );
        }
    }
    fn changed_record(index: usize, change: impl FnOnce(&mut serde_json::Value)) -> String {
        changed_source(fixtures::LIBRARY_RECORDS[index].1, change)
    }
    pub(super) fn changed_source(raw: &str, change: impl FnOnce(&mut serde_json::Value)) -> String {
        let mut r: serde_json::Value = serde_json::from_str(raw).unwrap();
        change(&mut r);
        r["sourceSha256"] = sha256(canonical(&r["source"]).unwrap().as_bytes()).into();
        r["bindingSha256"] = sha256(canonical(&r["binding"]).unwrap().as_bytes()).into();
        let abilities = serde_json::json!([r["faces"][0]["result"]["abilities"]]);
        r["astSha256"] = sha256(canonical(&abilities).unwrap().as_bytes()).into();
        canonical(&r).unwrap()
    }
    fn match_record(name: &str) -> &'static str {
        fixtures::MATCH_RECORDS
            .iter()
            .find(|(n, _)| *n == name)
            .unwrap()
            .1
    }
    #[test]
    fn tokens_sources_and_rejection_pairs() {
        fn changed_token_source(raw: &str, change: impl FnOnce(&mut serde_json::Value)) -> String {
            changed_source(raw, |r| {
                change(r);
                let result = &mut r["faces"][0]["result"];
                let abilities = result["abilities"].as_array().unwrap().clone();
                for (i, a) in abilities.into_iter().enumerate() {
                    result["lines"][i]["ability"] = a;
                }
            })
        }
        let catalog = embedded_catalog("tokens").unwrap();
        assert_eq!(catalog.schema_version, CURRENT_IR_VERSION);
        assert_eq!(
            catalog,
            Catalog::read(include_str!(
                "../../../tests/fixtures/expected-tokens-ir.json"
            ))
            .unwrap()
        );
        for (name, raw) in fixtures::TOKEN_RECORDS {
            let CompileResult::Lowered { definition } = compile(fixtures::MANIFEST, raw) else {
                panic!("{name}")
            };
            assert_eq!(
                catalog
                    .definitions
                    .iter()
                    .find(|d| d.oracle_id == definition.oracle_id),
                Some(definition.as_ref())
            );
            // A rejected extra sentence must never disappear behind a valid prefix.
            let bad = changed_token_source(raw, |r| {
                let a = r["faces"][0]["result"]["abilities"].as_array_mut().unwrap();
                let ability = a.iter_mut().find(|a| a.get("effects").is_some()).unwrap();
                ability["effects"][0]["steps"]
                    .as_array_mut()
                    .unwrap()
                    .push(serde_json::json!({"effect":"proliferate"}));
            });
            assert!(
                !matches!(
                    compile(fixtures::MANIFEST, &bad),
                    CompileResult::Lowered { .. }
                ),
                "{name}"
            );
        }
        let raw = include_str!("../../../tests/fixtures/spore-burst.json");
        assert!(!matches!(
            compile(fixtures::MANIFEST, raw),
            CompileResult::Lowered { .. }
        ));
        let raw = fixtures::TOKEN_RECORDS
            .iter()
            .find(|(n, _)| *n == "raise-the-alarm")
            .unwrap()
            .1;
        for change in [
            |v: &mut serde_json::Value| v["count"]["amount"] = "variable".into(),
            |v: &mut serde_json::Value| v["token"]["types"] = serde_json::json!(["artifact"]),
            |v: &mut serde_json::Value| {
                v["token"]["keywords"] = serde_json::json!([{"keyword":"protection"}])
            },
            |v: &mut serde_json::Value| {
                v["token"]["keywords"] = serde_json::json!([{"keyword":"flying","amount":1}])
            },
        ] {
            let bad = changed_token_source(raw, |r| {
                change(&mut r["faces"][0]["result"]["abilities"][0]["effects"][0]["steps"][0])
            });
            assert!(!matches!(
                compile(fixtures::MANIFEST, &bad),
                CompileResult::Lowered { .. }
            ));
        }
        let raw = fixtures::TOKEN_RECORDS
            .iter()
            .find(|(n, _)| *n == "battlegrowth")
            .unwrap()
            .1;
        for counter in ["-1/-1", "flying"] {
            let bad = changed_token_source(raw, |r| {
                r["faces"][0]["result"]["abilities"][0]["effects"][0]["steps"][0]["counter"]["type"] =
                    counter.into()
            });
            assert!(!matches!(
                compile(fixtures::MANIFEST, &bad),
                CompileResult::Lowered { .. }
            ));
        }
        let own = changed_token_source(raw, |r| {
            r["faces"][0]["result"]["abilities"][0]["effects"][0]["steps"][0]["on"] =
                serde_json::json!({"ref":"this","noun":"creature"})
        });
        // A spell has no creature source to receive "this creature" counters.
        assert!(!matches!(
            compile(fixtures::MANIFEST, &own),
            CompileResult::Lowered { .. }
        ));
    }
    #[test]
    fn combat_keywords_complete_sources() {
        let catalog = embedded_catalog("combat-keywords").unwrap();
        assert_eq!(catalog.schema_version, CURRENT_IR_VERSION);
        assert_eq!(
            catalog,
            Catalog::read(include_str!(
                "../../../tests/fixtures/expected-combat-keywords-ir.json"
            ))
            .unwrap()
        );
        for (name, raw) in fixtures::COMBAT_KEYWORD_RECORDS {
            let CompileResult::Lowered { definition } = compile(fixtures::MANIFEST, raw) else {
                panic!("{name}")
            };
            let source: serde_json::Value = serde_json::from_str(raw).unwrap();
            let expected = source["faces"][0]["result"]["abilities"]
                .as_array()
                .unwrap()
                .iter()
                .flat_map(|a| a["keywords"].as_array().unwrap())
                .map(|k| k["keyword"].as_str().unwrap())
                .collect::<Vec<_>>();
            assert_eq!(
                serde_json::to_value(&definition.keywords).unwrap(),
                serde_json::json!(expected)
            );
        }
    }
    #[test]
    fn keywords_complete_sources() {
        let catalog = embedded_catalog("keywords").unwrap();
        assert_eq!(catalog.schema_version, CURRENT_IR_VERSION);
        for (name, raw) in fixtures::KEYWORD_RECORDS {
            let CompileResult::Lowered { definition } = compile(fixtures::MANIFEST, raw) else {
                panic!("{name}")
            };
            let source: serde_json::Value = serde_json::from_str(raw).unwrap();
            let expected = source["faces"][0]["result"]["abilities"]
                .as_array()
                .unwrap()
                .iter()
                .flat_map(|a| a["keywords"].as_array().unwrap())
                .map(|k| k["keyword"].as_str().unwrap())
                .collect::<Vec<_>>();
            assert_eq!(
                serde_json::to_value(&definition.keywords).unwrap(),
                serde_json::json!(expected)
            );
            for keyword in [
                serde_json::json!({"keyword":"protection"}),
                serde_json::json!({"keyword":"haste","amount":1}),
            ] {
                let changed = changed_source(raw, |r| {
                    r["faces"][0]["result"]["abilities"][0]["keywords"][0] = keyword;
                    r["faces"][0]["result"]["lines"][0]["ability"] =
                        r["faces"][0]["result"]["abilities"][0].clone();
                });
                assert!(
                    !matches!(
                        compile(fixtures::MANIFEST, &changed),
                        CompileResult::Lowered { .. }
                    ),
                    "{name}"
                );
            }
        }
    }
    #[test]
    fn defender_lowers_complete_keyword() {
        let c = embedded_catalog("match").unwrap();
        assert_eq!(c.schema_version, CURRENT_IR_VERSION);
        assert_eq!(
            c,
            Catalog::read(include_str!(
                "../../../tests/fixtures/expected-match-ir.json"
            ))
            .unwrap()
        );
        let d = c
            .definitions
            .iter()
            .find(|d| d.name == "Wall of Wood")
            .unwrap();
        assert_eq!(d.keywords, vec![KeywordAbility::Defender]);
        assert!(d.capabilities.contains(&Capability::DefenderV1));
        assert_eq!(
            (
                d.characteristics.power.clone(),
                d.characteristics.toughness.clone()
            ),
            (Some(0.into()), Some(3.into()))
        );
        assert_eq!(
            d.characteristics.mana_cost,
            ManaCost {
                generic: 0.into(),
                colored: vec![Color::G]
            }
        );
        assert_eq!(d.characteristics.subtypes, ["Wall"]);
        assert!(d.activated.is_empty() && d.triggered.is_empty() && d.static_abilities.is_empty());
        assert_eq!(
            d.spell.as_ref().unwrap().resolution,
            SpellResolution::Permanent
        );
        assert!(matches!(
            compile(fixtures::MANIFEST, match_record("wall-of-vines")),
            CompileResult::Lowered { .. }
        ));
        let raw = changed_source(match_record("wall-of-wood"), |r| {
            r["source"]["name"] = "Source shape independent".into();
            r["faces"][0]["result"]["name"] = "Source shape independent".into();
        });
        assert!(matches!(
            compile(fixtures::MANIFEST, &raw),
            CompileResult::Lowered { .. }
        ));
    }
    #[test]
    fn defender_whole_source_and_ir_fields_reject_unknown_keyword_semantics() {
        type Mutation = fn(&mut serde_json::Value);
        for mutate in [
            (|a: &mut serde_json::Value| a["keywords"][0]["keyword"] = "indestructible".into())
                as Mutation,
            |a| a["keywords"][0]["condition"] = true.into(),
            |a| a["keywords"].as_array_mut().unwrap().clear(),
            |a| {
                a["keywords"]
                    .as_array_mut()
                    .unwrap()
                    .push(serde_json::json!({"keyword":"protection"}))
            },
        ] {
            let raw = changed_source(match_record("wall-of-wood"), |r| {
                mutate(&mut r["faces"][0]["result"]["abilities"][0]);
                r["faces"][0]["result"]["lines"][0]["ability"] =
                    r["faces"][0]["result"]["abilities"][0].clone();
            });
            assert!(!matches!(
                compile(fixtures::MANIFEST, &raw),
                CompileResult::Lowered { .. }
            ));
        }
        let c = embedded_catalog("match").unwrap();
        let wall = c
            .definitions
            .iter()
            .find(|d| !d.keywords.is_empty())
            .unwrap();
        let mut bad = wall.clone();
        bad.keywords.clear();
        assert!(bad.validate().is_err());
        let mut bad = wall.clone();
        bad.characteristics.kind = CardKind::Instant;
        bad.capabilities = bad.requirements();
        assert!(bad.validate().is_err());
        let mut raw = serde_json::to_value(wall).unwrap();
        raw["keywords"] = serde_json::json!(["protection"]);
        assert!(strict_read::<Definition>(&canonical(&raw).unwrap()).is_err());
        raw["keywords"] = serde_json::json!([{"defender":null}]);
        assert!(strict_read::<Definition>(&canonical(&raw).unwrap()).is_err());
    }
    fn choice_record(name: &str) -> &'static str {
        fixtures::CHOICE_RECORDS
            .iter()
            .find(|(n, _)| *n == name)
            .unwrap()
            .1
    }
    #[test]
    fn complete_choices_sources_preserve_occurrences_modes_and_conditions() {
        let c = embedded_catalog("choices").unwrap();
        assert_eq!(
            c,
            Catalog::read(include_str!(
                "../../../tests/fixtures/expected-choices-ir.json"
            ))
            .unwrap()
        );
        assert_eq!(c.schema_version, CURRENT_IR_VERSION);
        let seeds = c
            .definitions
            .iter()
            .find(|d| d.name == "Seeds of Strength")
            .unwrap();
        let p = seeds.spell.as_ref().unwrap();
        assert_eq!(
            p.targets
                .iter()
                .map(|t| (&*t.selection_id, t.slot))
                .collect::<Vec<_>>(),
            [("line:0:0", 0), ("line:1:0", 1), ("line:2:0", 2)]
        );
        for (i, op) in p.instructions.iter().enumerate() {
            assert!(
                matches!(op,Instruction::ModifyPowerToughness{target_slot,power,toughness,duration:Duration::UntilEndOfTurn,source_block}
                if *target_slot==i as u32 && *source_block==i as u32 && *power==Quantity::from(1) && *toughness==Quantity::from(1))
            );
        }
        let salve = c
            .definitions
            .iter()
            .find(|d| d.name == "Healing Salve")
            .unwrap();
        assert_eq!(
            salve.characteristics.mana_cost,
            ManaCost {
                generic: 0.into(),
                colored: vec![Color::W]
            }
        );
        let p = salve.spell.as_ref().unwrap();
        assert!(p.instructions.is_empty() && p.targets.is_empty());
        assert_eq!(p.modes.len(), 2);
        assert!(
            matches!(p.modes[0].program.instructions.as_slice(),[Instruction::GainLife{recipient:LifeRecipient::Target{target_slot:0},amount,source_block:0}] if *amount==Quantity::from(3))
        );
        assert_eq!(
            p.modes[0].program.targets[0].domain,
            TargetDomain::AnyPlayer
        );
        assert!(
            matches!(p.modes[1].program.instructions.as_slice(),[Instruction::PreventNextDamage{target_slot:0,amount,duration:Duration::UntilEndOfTurn,source_block:0}] if *amount==Quantity::from(3))
        );
        assert_eq!(
            p.modes[1].program.targets[0].domain,
            TargetDomain::AnyDamageTarget
        );
        for cap in [
            Capability::ModalChoiceV1,
            Capability::LifeGainV1,
            Capability::DamageBudgetPreventionV1,
            Capability::AnyPlayerTargetV1,
            Capability::AnyDamageTargetV1,
        ] {
            assert!(salve.capabilities.contains(&cap));
        }
        let b = c
            .definitions
            .iter()
            .find(|d| d.name == "Slaughterhouse Bouncer")
            .unwrap();
        assert_eq!(
            b.characteristics.mana_cost,
            ManaCost {
                generic: 4.into(),
                colored: vec![Color::B]
            }
        );
        assert_eq!(b.triggered[0].condition, TriggerCondition::SelfDies);
        assert_eq!(
            b.triggered[0].intervening_if,
            Some(InterveningCondition::ControllerHandEmpty)
        );
        assert!(
            matches!(b.triggered[0].program.instructions.as_slice(),[Instruction::ModifyPowerToughness{target_slot:0,power,toughness,duration:Duration::UntilEndOfTurn,source_block:0}] if *power==Quantity::from(-3) && *toughness==Quantity::from(-3))
        );
        assert_eq!(Catalog::read(&canonical(&c).unwrap()).unwrap(), c);
        for name in ["boros-charm", "lavaborn-muse"] {
            assert!(
                matches!(
                    compile(fixtures::MANIFEST, choice_record(name)),
                    CompileResult::Unsupported { .. }
                ),
                "{name}"
            );
        }
        assert!(
            embedded_catalog("effects")
                .unwrap()
                .definitions
                .iter()
                .all(|d| d.name != "Healing Salve")
        );
    }
    #[test]
    fn choices_rehashed_source_mutations_cannot_drop_modes_conditions_or_target_occurrences() {
        type Mutation = (&'static str, fn(&mut serde_json::Value));
        let cases: Vec<Mutation> = vec![
            ("healing-salve", |a| {
                a["effects"][0]["steps"][0]["count"]["exactly"] = 2.into()
            }),
            ("healing-salve", |a| {
                a["effects"][0]["steps"][0]["options"][1]["effects"][0]["steps"][0]["prevention"]
                    ["kind"] = "combat".into()
            }),
            ("healing-salve", |a| {
                a["effects"][0]["steps"][0]["options"][1]["effects"][0]["steps"][0]["prevention"]
                    ["shield"]["amount"]["value"] = 0.into()
            }),
            ("healing-salve", |a| {
                a["effects"][0]["steps"][0]["options"]
                    .as_array_mut()
                    .unwrap()
                    .truncate(1)
            }),
            ("healing-salve", |a| {
                a["effects"][0]["steps"][0]["options"][1]["extra"] = true.into()
            }),
            ("slaughterhouse-bouncer", |a| {
                a["condition"]["comparison"]["op"] = "le".into()
            }),
            ("slaughterhouse-bouncer", |a| {
                a["condition"]["comparison"]["value"]["value"] = 1.into()
            }),
            ("slaughterhouse-bouncer", |a| {
                a["condition"]["who"]["player"] = "owner".into()
            }),
            ("slaughterhouse-bouncer", |a| {
                a["condition"]["comparison"]["value"]["amount"] = "X".into()
            }),
            ("slaughterhouse-bouncer", |a| {
                a["condition"]["hidden"] = "lost".into()
            }),
            ("slaughterhouse-bouncer", |a| {
                a["trigger"]["what"]["ref"] = "other".into()
            }),
        ];
        for (name, mutate) in cases {
            let raw = changed_source(choice_record(name), |r| {
                mutate(&mut r["faces"][0]["result"]["abilities"][0]);
                r["faces"][0]["result"]["lines"][0]["ability"] =
                    r["faces"][0]["result"]["abilities"][0].clone();
            });
            assert!(
                !matches!(
                    compile(fixtures::MANIFEST, &raw),
                    CompileResult::Lowered { .. }
                ),
                "{name}"
            );
        }
        let alias = changed_source(choice_record("seeds-of-strength"), |r| {
            r["faces"][0]["result"]["abilities"][1]["effects"][0]["steps"][0]["what"]["selectionId"] =
                "line:0:0".into();
            r["faces"][0]["result"]["lines"][1]["ability"] =
                r["faces"][0]["result"]["abilities"][1].clone();
        });
        assert!(!matches!(
            compile(fixtures::MANIFEST, &alias),
            CompileResult::Lowered { .. }
        ));
        for name in ["healing-salve", "slaughterhouse-bouncer"] {
            let renamed = changed_source(choice_record(name), |r| {
                r["source"]["name"] = "Independent shape witness".into();
                r["faces"][0]["result"]["name"] = "Independent shape witness".into();
            });
            assert!(matches!(
                compile(fixtures::MANIFEST, &renamed),
                CompileResult::Lowered { .. }
            ));
        }
    }
    #[test]
    fn choices_ir_rejects_nested_modes_and_inconsistent_children() {
        let c = embedded_catalog("choices").unwrap();
        let salve = c
            .definitions
            .iter()
            .find(|d| d.name == "Healing Salve")
            .unwrap();
        let mut cases = vec![];
        let mut d = salve.clone();
        d.spell
            .as_mut()
            .unwrap()
            .instructions
            .push(Instruction::DrawCards {
                recipient: None,
                count: 1.into(),
                source_block: 0,
            });
        cases.push(d);
        let mut d = salve.clone();
        d.spell.as_mut().unwrap().modes.truncate(1);
        cases.push(d);
        let mut d = salve.clone();
        d.spell.as_mut().unwrap().modes[0].program.timing = SpellTiming::MainPhase;
        cases.push(d);
        let mut d = salve.clone();
        let root = d.spell.as_ref().unwrap().clone();
        d.spell.as_mut().unwrap().modes[0].program = root;
        cases.push(d);
        let mut d = salve.clone();
        d.spell.as_mut().unwrap().modes[0]
            .program
            .instructions
            .clear();
        cases.push(d);
        for mut d in cases {
            d.capabilities = d.requirements();
            assert!(d.validate().is_err());
        }
        let mut raw = serde_json::to_value(salve).unwrap();
        raw["spell"]["modes"][0]["unexpected"] = true.into();
        assert!(strict_read::<Definition>(&canonical(&raw).unwrap()).is_err());
    }
    fn effect_record(name: &str) -> &'static str {
        fixtures::EFFECT_RECORDS
            .iter()
            .find(|(n, _)| *n == name)
            .unwrap()
            .1
    }
    fn effect_mutant(name: &str, change: impl FnOnce(&mut serde_json::Value)) -> String {
        changed_source(effect_record(name), |r| {
            change(&mut r["faces"][0]["result"]["abilities"][0]);
            r["faces"][0]["result"]["lines"][0]["ability"] =
                r["faces"][0]["result"]["abilities"][0].clone();
        })
    }
    #[test]
    fn controlled_creature_replacement_lowers_complete_gratuitous_source_without_rewriting_furnace()
    {
        let raw = effect_record("gratuitous-violence");
        let source: serde_json::Value = serde_json::from_str(raw).unwrap();
        let CompileResult::Lowered { definition } = compile(fixtures::MANIFEST, raw) else {
            panic!("complete retained Gratuitous Violence must lower");
        };
        assert_eq!(definition.name, "Gratuitous Violence");
        assert_eq!(
            source["source"]["oracle_text"],
            "If a creature you control would deal damage to a permanent or player, it deals double that damage instead."
        );
        assert_eq!(definition.characteristics.kind, CardKind::Enchantment);
        assert_eq!(
            definition.characteristics.mana_cost,
            ManaCost {
                generic: 2.into(),
                colored: vec![Color::R; 3]
            }
        );
        assert_eq!(
            definition.spell.as_ref().unwrap().resolution,
            SpellResolution::Permanent
        );
        assert_eq!(
            definition.static_abilities,
            [StaticAbility {
                functioning_zone: FunctioningZone::Battlefield,
                effect: StaticEffect::DoubleControllerCreatureDamageToPermanentOrPlayer
            }]
        );
        assert!(
            definition.activated.is_empty()
                && definition.triggered.is_empty()
                && definition.keywords.is_empty()
        );
        assert!(
            definition
                .capabilities
                .contains(&Capability::ControlledCreatureDamageReplacementV1)
        );
        assert!(
            !definition
                .capabilities
                .contains(&Capability::StaticDamageReplacementV1)
        );
        assert_eq!(definition.capabilities, definition.requirements());
        assert_eq!(definition.provenance.source_sha256, source["sourceSha256"]);
        assert_eq!(definition.provenance.ast_sha256, source["astSha256"]);
        assert_eq!(
            definition.provenance.binding_sha256,
            source["bindingSha256"]
        );
        assert_eq!(source["binding"]["fullyBound"], false);
        assert!(
            !source["binding"]["diagnostics"]
                .as_array()
                .unwrap()
                .is_empty()
        );
        Catalog {
            schema_version: CURRENT_IR_VERSION,
            rules_edition: RULES_EDITION.into(),
            rules_sha256: RULES_SHA256.into(),
            definitions: vec![definition.as_ref().clone()],
        }
        .validate()
        .unwrap();
        for reversed in [false, true] {
            let synthetic = effect_mutant("gratuitous-violence", |a| {
                if reversed {
                    a["effect"]["replacement"]["event"]["to"]
                        .as_array_mut()
                        .unwrap()
                        .reverse();
                }
            });
            let CompileResult::Lowered { definition: order } =
                compile(fixtures::MANIFEST, &synthetic)
            else {
                panic!("equivalent recipient union rejected");
            };
            assert_eq!(order.static_abilities, definition.static_abilities);
            assert_eq!(order.characteristics, definition.characteristics);
        }
        // The global source retains its complete definition,
        // including original provenance and capabilities, without regeneration.
        let old = embedded_catalog("effects").unwrap();
        let expected = old
            .definitions
            .iter()
            .find(|d| d.name == "Furnace of Rath")
            .unwrap();
        let CompileResult::Lowered {
            definition: furnace,
        } = compile(fixtures::MANIFEST, effect_record("furnace-of-rath"))
        else {
            panic!("Furnace source");
        };
        assert_eq!(furnace.as_ref(), expected);
        assert!(
            !furnace
                .capabilities
                .contains(&Capability::ControlledCreatureDamageReplacementV1)
        );
    }

    #[test]
    fn controlled_creature_replacement_rejects_partial_sources_and_qualified_recipients() {
        type Mutation = fn(&mut serde_json::Value);
        for mutate in [
            (|r: &mut serde_json::Value| {
                r["event"]["source"]["filter"]["control"] = "opponent".into()
            }) as Mutation,
            |r| r["event"]["source"]["filter"]["control"] = serde_json::Value::Null,
            |r| {
                r["event"]["source"]["filter"]
                    .as_object_mut()
                    .unwrap()
                    .remove("control");
            },
            |r| r["event"]["source"]["filter"]["types"] = serde_json::json!(["artifact"]),
            |r| {
                r["event"]["source"]["filter"]["types"] =
                    serde_json::json!(["artifact", "creature"])
            },
            |r| {
                r["event"]["source"]["filter"]["types"] =
                    serde_json::json!(["creature", "creature"])
            },
            |r| r["event"]["source"]["filter"]["types"] = serde_json::json!([]),
            |r| r["event"]["source"]["filter"]["types"] = serde_json::Value::Null,
            |r| r["event"]["source"]["filter"]["owner"] = "you".into(),
            |r| r["event"]["source"]["filter"]["subtypes"] = serde_json::json!(["Goblin"]),
            |r| r["event"]["source"]["filter"]["other"] = true.into(),
            |r| r["event"]["source"]["filter"]["properties"] = serde_json::json!([]),
            |r| r["event"]["source"]["filter"]["status"] = serde_json::json!(["attacking"]),
            |r| r["event"]["source"]["filter"] = serde_json::Value::Null,
            |r| r["event"]["source"]["ref"] = "this".into(),
            |r| r["event"]["source"]["count"]["value"] = 0.into(),
            |r| r["event"]["source"]["count"]["value"] = 2.into(),
            |r| r["event"]["source"]["count"]["amount"] = "that-much".into(),
            |r| r["event"]["source"]["selectionId"] = "unbound".into(),
            |r| r["event"]["kind"] = "combat".into(),
            |r| r["event"]["to"][1]["of"] = serde_json::json!({"ref":"it"}),
            |r| r["event"]["to"][1]["selectionId"] = "unbound-player".into(),
            |r| r["event"]["to"][0]["filter"]["types"] = serde_json::json!(["creature"]),
            |r| r["event"]["to"] = serde_json::json!([{"player":"a-player"}]),
            |r| r["occurrence"] = "next".into(),
            |r| r["replace"]["change"]["factor"] = 3.into(),
            |r| r["replace"]["change"]["operation"] = "add".into(),
            |r| r["replace"]["change"]["optional"] = true.into(),
        ] {
            let raw = effect_mutant("gratuitous-violence", |a| {
                mutate(&mut a["effect"]["replacement"])
            });
            assert!(
                matches!(
                    compile(fixtures::MANIFEST, &raw),
                    CompileResult::Unsupported { .. }
                ),
                "unsupported complete replacement must reject"
            );
        }
    }
    #[test]
    fn furnace_replacement_rejects_qualified_player_scope_in_either_union_order() {
        let catalog = embedded_catalog("effects").unwrap();
        let expected = catalog
            .definitions
            .iter()
            .find(|d| d.name == "Furnace of Rath")
            .unwrap();
        let CompileResult::Lowered { definition } =
            compile(fixtures::MANIFEST, effect_record("furnace-of-rath"))
        else {
            panic!("complete frozen Furnace of Rath must lower");
        };
        assert_eq!(definition.as_ref(), expected);
        for reversed in [false, true] {
            let raw = effect_mutant("furnace-of-rath", |a| {
                if reversed {
                    a["effect"]["replacement"]["event"]["to"]
                        .as_array_mut()
                        .unwrap()
                        .reverse();
                }
            });
            let CompileResult::Lowered { definition } = compile(fixtures::MANIFEST, &raw) else {
                panic!("equivalent complete recipient union rejected: reversed={reversed}");
            };
            assert_eq!(definition.static_abilities, expected.static_abilities);
            assert_eq!(definition.characteristics, expected.characteristics);
            assert_eq!(definition.capabilities, expected.capabilities);
            for (field, value) in [
                ("of", serde_json::json!({"ref":"it"})),
                ("selectionId", serde_json::json!("unbound-player")),
            ] {
                // Synthetic near-miss of the exact archived Furnace source:
                // neither a qualified object relation nor a selected player is
                // represented by the all-permanent/player doubling instruction.
                let qualified = effect_mutant("furnace-of-rath", |a| {
                    let to = a["effect"]["replacement"]["event"]["to"]
                        .as_array_mut()
                        .unwrap();
                    let player = to.iter_mut().find(|r| r["player"] == "a-player").unwrap();
                    player[field] = value;
                    if reversed {
                        to.reverse();
                    }
                });
                assert!(
                    matches!(
                        compile(fixtures::MANIFEST, &qualified),
                        CompileResult::Unsupported { .. }
                    ),
                    "unsupported player {field} must not be dropped: reversed={reversed}"
                );
            }
        }
    }
    #[test]
    fn complete_effects_catalog_binds_all_six_sources() {
        let c = embedded_catalog("effects").unwrap();
        assert_eq!(c.schema_version, CURRENT_IR_VERSION);
        for (name, power, toughness) in [("Giant Growth", 3, 3), ("Disfigure", -2, -2)] {
            let d = c.definitions.iter().find(|d| d.name == name).unwrap();
            assert!(matches!(d.spell.as_ref().unwrap().instructions.as_slice(),
                [Instruction::ModifyPowerToughness { power:p,toughness:t,duration:Duration::UntilEndOfTurn,target_slot:0,source_block:0 }]
                if *p == Quantity::from(power) && *t == Quantity::from(toughness)));
            assert_eq!(
                d.spell.as_ref().unwrap().targets[0].domain,
                TargetDomain::BattlefieldCreature
            );
        }
        let fog = c.definitions.iter().find(|d| d.name == "Fog").unwrap();
        assert!(fog.spell.as_ref().unwrap().targets.is_empty());
        assert!(matches!(
            fog.spell.as_ref().unwrap().instructions.as_slice(),
            [Instruction::PreventCombatDamage {
                duration: Duration::UntilEndOfTurn,
                source_block: 0
            }]
        ));
        let healer = c
            .definitions
            .iter()
            .find(|d| d.name == "Samite Healer")
            .unwrap();
        assert_eq!(healer.activated[0].cost, ActivationCost::TapSource);
        assert_eq!(
            healer.activated[0].program.targets[0].domain,
            TargetDomain::AnyDamageTarget
        );
        assert!(
            matches!(healer.activated[0].program.instructions.as_slice(),[Instruction::PreventNextDamage { target_slot:0,amount,duration:Duration::UntilEndOfTurn,source_block:0 }] if *amount==Quantity::from(1))
        );
        let furnace = c
            .definitions
            .iter()
            .find(|d| d.name == "Furnace of Rath")
            .unwrap();
        assert_eq!(furnace.characteristics.kind, CardKind::Enchantment);
        assert_eq!(
            furnace.characteristics.mana_cost,
            ManaCost {
                generic: Quantity::from(1),
                colored: vec![Color::R; 3]
            }
        );
        assert_eq!(
            furnace.static_abilities,
            vec![StaticAbility {
                functioning_zone: FunctioningZone::Battlefield,
                effect: StaticEffect::DoubleDamageToPermanentOrPlayer
            }]
        );
        assert_eq!(
            furnace.spell.as_ref().unwrap().resolution,
            SpellResolution::Permanent
        );
        let ox = c.definitions.iter().find(|d| d.name == "Yoked Ox").unwrap();
        assert_eq!(
            (
                ox.characteristics.power.clone(),
                ox.characteristics.toughness.clone()
            ),
            (Some(Quantity::from(0)), Some(Quantity::from(4)))
        );
        assert_eq!(Catalog::read(&canonical(&c).unwrap()).unwrap(), c);
        assert_eq!(
            c,
            Catalog::read(include_str!(
                "../../../tests/fixtures/expected-effects-ir.json"
            ))
            .unwrap()
        );
        for (name, generic, colored) in [
            ("Giant Growth", 0, vec![Color::G]),
            ("Disfigure", 0, vec![Color::B]),
            ("Fog", 0, vec![Color::G]),
            ("Samite Healer", 1, vec![Color::W]),
            ("Yoked Ox", 0, vec![Color::W]),
        ] {
            assert_eq!(
                c.definitions
                    .iter()
                    .find(|d| d.name == name)
                    .unwrap()
                    .characteristics
                    .mana_cost,
                ManaCost {
                    generic: generic.into(),
                    colored
                }
            );
        }
    }
    #[test]
    fn complete_effects_sources_reject_filtered_modal_and_optional_near_misses() {
        // Gratuitous Violence's original source negative is now a complete
        // controlled-creature replacement positive above; other partial sources
        // remain unsupported; each complete source retains its exact expected program.
        for name in ["righteousness", "flame-rift", "deathgreeter"] {
            assert!(
                matches!(
                    compile(fixtures::MANIFEST, effect_record(name)),
                    CompileResult::Unsupported { .. }
                ),
                "{name}"
            );
        }
        for name in ["fog", "samite-healer", "furnace-of-rath"] {
            let raw = changed_source(effect_record(name), |r| {
                r["binding"]["fullyBound"] = true.into();
                r["source"]["name"] = "Name independent source witness".into();
                r["faces"][0]["result"]["name"] = "Name independent source witness".into();
            });
            assert!(
                matches!(
                    compile(fixtures::MANIFEST, &raw),
                    CompileResult::Lowered { .. }
                ),
                "{name}"
            );
        }
    }
    #[test]
    fn effects_require_exact_capability_closure() {
        let c = embedded_catalog("effects").unwrap();
        for d in c.definitions.iter().filter(|d| {
            d.capabilities.iter().any(|c| {
                matches!(
                    c,
                    Capability::CastEnchantmentV1
                        | Capability::TemporaryPowerToughnessV1
                        | Capability::CombatDamagePreventionV1
                        | Capability::DamageBudgetPreventionV1
                        | Capability::StaticDamageReplacementV1
                )
            })
        }) {
            for cap in &d.capabilities {
                let mut bad = d.clone();
                bad.capabilities.remove(cap);
                assert!(bad.validate().is_err());
            }
        }
    }
    #[test]
    fn rehashed_effect_scope_duration_and_allocation_mutants_fail_complete_lowering() {
        let cases: [(&str, &str, serde_json::Value); 16] = [
            (
                "giant-growth",
                "/effects/0/steps/0/power/sign",
                serde_json::json!(2),
            ),
            (
                "disfigure",
                "/effects/0/steps/0/duration/duration",
                serde_json::json!("permanent"),
            ),
            (
                "fog",
                "/effects/0/steps/0/prevention/kind",
                serde_json::json!("all"),
            ),
            (
                "fog",
                "/effects/0/steps/0/prevention/eventScope/sources",
                serde_json::json!("separate"),
            ),
            (
                "fog",
                "/effects/0/steps/0/prevention/eventScope/recipients",
                serde_json::json!("separate"),
            ),
            (
                "fog",
                "/effects/0/steps/0/prevention/shield/occurrence",
                serde_json::json!("next"),
            ),
            (
                "samite-healer",
                "/effects/0/steps/0/prevention/kind",
                serde_json::json!("combat"),
            ),
            (
                "samite-healer",
                "/effects/0/steps/0/prevention/shield/amount/value",
                serde_json::json!(0),
            ),
            (
                "samite-healer",
                "/effects/0/steps/0/prevention/to",
                serde_json::json!([]),
            ),
            (
                "samite-healer",
                "/effects/0/steps/0/duration/duration",
                serde_json::json!("end-of-turn"),
            ),
            (
                "furnace-of-rath",
                "/effect/replacement/event/kind",
                serde_json::json!("combat"),
            ),
            (
                "furnace-of-rath",
                "/effect/replacement/event/source/filter",
                serde_json::json!({"types":["creature"]}),
            ),
            (
                "furnace-of-rath",
                "/effect/replacement/event/to",
                serde_json::json!([{"player":"a-player"},{"player":"a-player"}]),
            ),
            (
                "furnace-of-rath",
                "/effect/replacement/occurrence",
                serde_json::json!("next"),
            ),
            (
                "furnace-of-rath",
                "/effect/replacement/replace/change/operation",
                serde_json::json!("add"),
            ),
            (
                "furnace-of-rath",
                "/effect/replacement/replace/change/factor",
                serde_json::json!(3),
            ),
        ];
        for (name, path, value) in cases {
            let raw = effect_mutant(name, |ability| {
                *ability.pointer_mut(path).expect(path) = value
            });
            assert!(
                matches!(
                    compile(fixtures::MANIFEST, &raw),
                    CompileResult::Unsupported { .. }
                ),
                "{name} {path}"
            );
        }
        let extra = effect_mutant("samite-healer", |a| {
            a["effects"][0]["steps"][0]["prevention"]["after"] =
                serde_json::json!({"effect":"draw"})
        });
        assert!(matches!(
            compile(fixtures::MANIFEST, &extra),
            CompileResult::Unsupported { .. }
        ));
    }
    #[test]
    fn effect_binder_consumes_complete_scope_fields_without_diagnostic_or_name_dispatch() {
        type Mutation = (&'static str, fn(&mut serde_json::Value));
        let cases: Vec<Mutation> = vec![
            ("giant-growth", |a| {
                a["effects"][0]["steps"][0]["what"]["filter"]["control"] = serde_json::json!("you")
            }),
            ("giant-growth", |a| {
                a["effects"][0]["steps"][0]["power"]["sign"] = serde_json::json!(0)
            }),
            ("giant-growth", |a| {
                a["effects"][0]["steps"][0]["power"]["amount"]["amount"] =
                    serde_json::json!("variable")
            }),
            ("giant-growth", |a| {
                a["effects"][0]["steps"][0]["duration"]["unknown"] = true.into()
            }),
            ("disfigure", |a| {
                a["effects"][0]["steps"][0]["toughness"]["amount"]["value"] = (-2).into()
            }),
            ("fog", |a| {
                a["effects"][0]["steps"][0]["prevention"]["to"] = serde_json::json!([])
            }),
            ("fog", |a| {
                a["effects"][0]["steps"][0]["prevention"]["by"] =
                    serde_json::json!({"player":"you"})
            }),
            ("fog", |a| {
                a["effects"][0]["steps"][0]["prevention"]["shield"]["prevent"]["optional"] =
                    true.into()
            }),
            ("samite-healer", |a| {
                a["costs"]
                    .as_array_mut()
                    .unwrap()
                    .push(serde_json::json!({"cost":"mana","symbols":["W"]}))
            }),
            ("samite-healer", |a| {
                a["effects"][0]["steps"][0]["prevention"]["eventScope"]["recipients"] =
                    "each".into()
            }),
            ("samite-healer", |a| {
                a["effects"][0]["steps"][0]["prevention"]["to"]
                    .as_array_mut()
                    .unwrap()
                    .push(serde_json::json!({"ref":"any-target","selectionId":"extra"}))
            }),
            ("samite-healer", |a| {
                a["effects"][0]["steps"][0]["prevention"]["to"][0]["ref"] = "it".into()
            }),
            ("samite-healer", |a| {
                a["effects"][0]["steps"][0]["optional"] = true.into()
            }),
            ("furnace-of-rath", |a| {
                a["effect"]["replacement"]["event"]["source"]["count"]["value"] = 2.into()
            }),
            ("furnace-of-rath", |a| {
                a["effect"]["replacement"]["event"]["to"][0]["filter"]["control"] = "you".into()
            }),
            ("furnace-of-rath", |a| {
                a["effect"]["replacement"]["event"]["to"][1]["player"] = "you".into()
            }),
            ("furnace-of-rath", |a| {
                a["effect"]["replacement"]["replace"]["change"]["extra"] = true.into()
            }),
        ];
        for (name, mutate) in cases {
            let raw = changed_source(effect_record(name), |r| {
                r["binding"]["fullyBound"] = true.into();
                mutate(&mut r["faces"][0]["result"]["abilities"][0]);
                r["faces"][0]["result"]["lines"][0]["ability"] =
                    r["faces"][0]["result"]["abilities"][0].clone();
            });
            assert!(
                matches!(
                    compile(fixtures::MANIFEST, &raw),
                    CompileResult::Unsupported { .. }
                ),
                "{name}"
            );
        }
        let reversed = effect_mutant("furnace-of-rath", |a| {
            a["effect"]["replacement"]["event"]["to"]
                .as_array_mut()
                .unwrap()
                .reverse()
        });
        let CompileResult::Lowered { definition } = compile(fixtures::MANIFEST, &reversed) else {
            panic!("equivalent recipient union rejected")
        };
        assert_eq!(
            definition.static_abilities,
            [StaticAbility {
                functioning_zone: FunctioningZone::Battlefield,
                effect: StaticEffect::DoubleDamageToPermanentOrPlayer
            }]
        );
        for name in ["furnace-of-rath", "samite-healer"] {
            let extra = changed_source(effect_record(name), |r| {
                let unknown =
                    serde_json::json!({"kind":"keywords","keywords":[{"keyword":"protection"}]});
                r["faces"][0]["result"]["abilities"]
                    .as_array_mut()
                    .unwrap()
                    .push(unknown.clone());
                r["faces"][0]["result"]["lines"]
                    .as_array_mut()
                    .unwrap()
                    .push(serde_json::json!({"text":"Protection from red","ok":true,"ability":unknown}));
                r["source"]["oracle_text"] = format!(
                    "{}\nProtection from red",
                    r["source"]["oracle_text"].as_str().unwrap()
                )
                .into();
            });
            assert!(
                matches!(
                    compile(fixtures::MANIFEST, &extra),
                    CompileResult::Unsupported { .. }
                ),
                "whole {name}"
            );
        }
    }
    #[test]
    fn effects_ir_rejects_unbound_targets_invalid_budgets_and_static_zones() {
        let c = embedded_catalog("effects").unwrap();
        type Mutation = (&'static str, fn(&mut serde_json::Value));
        let mutations: Vec<Mutation> = vec![
            ("Giant Growth", |d| {
                d["spell"]["instructions"][0]["target_slot"] = 1.into()
            }),
            ("Disfigure", |d| {
                d["spell"]["targets"][0]["domain"] = "any_damage_target".into()
            }),
            ("Disfigure", |d| {
                d["spell"]["instructions"][0]["power"] = "-0".into()
            }),
            ("Fog", |d| {
                d["spell"]["instructions"][0]["duration"] = "permanent".into()
            }),
            ("Fog", |d| {
                d["spell"]["instructions"][0]["target_slot"] = 0.into()
            }),
            ("Samite Healer", |d| {
                d["activated"][0]["program"]["instructions"][0]["amount"] = "0".into()
            }),
            ("Samite Healer", |d| {
                d["activated"][0]["program"]["instructions"][0]["amount"] = "-1".into()
            }),
            ("Samite Healer", |d| {
                d["activated"][0]["program"]["instructions"][0]["amount"] = "01".into()
            }),
            ("Samite Healer", |d| {
                d["activated"][0]["program"]["targets"][0]["domain"] = "battlefield_creature".into()
            }),
            ("Furnace of Rath", |d| {
                d["static_abilities"][0]["functioning_zone"] = "stack".into()
            }),
            ("Furnace of Rath", |d| {
                d["static_abilities"][0]["effect"] = "double_only_your_damage".into()
            }),
            ("Furnace of Rath", |d| {
                d["characteristics"]["subtypes"] = serde_json::json!(["Aura"])
            }),
            ("Furnace of Rath", |d| {
                d["spell"]["timing"] = "instant".into()
            }),
        ];
        for (name, mutate) in mutations {
            let mut value = serde_json::to_value(&c).unwrap();
            mutate(
                value["definitions"]
                    .as_array_mut()
                    .unwrap()
                    .iter_mut()
                    .find(|d| d["name"] == name)
                    .unwrap(),
            );
            assert!(
                Catalog::read(&canonical(&value).unwrap()).is_err(),
                "{name}"
            );
        }
        let mut future = c;
        future.schema_version = mtg_contracts::CURRENT_IR_VERSION + 1;
        assert!(future.validate().is_err());
    }
    #[test]
    fn complete_trigger_pool_binds_exact_source_roles() {
        let catalog = embedded_catalog("trigger").unwrap();
        assert_eq!(catalog.schema_version, CURRENT_IR_VERSION);
        assert_eq!(
            Catalog::read(include_str!(
                "../../../tests/fixtures/expected-trigger-ir.json"
            ))
            .unwrap(),
            catalog
        );
        let get = |name: &str| catalog.definitions.iter().find(|d| d.name == name).unwrap();
        let visionary = get("Elvish Visionary");
        assert_eq!(
            visionary.characteristics.mana_cost,
            ManaCost {
                generic: 1.into(),
                colored: vec![Color::G]
            }
        );
        assert_eq!(visionary.characteristics.power, Some(1.into()));
        assert_eq!(visionary.characteristics.toughness, Some(1.into()));
        assert_eq!(
            visionary.triggered,
            [TriggeredAbility {
                death_filter: None,
                intervening_if: None,
                functioning_zone: FunctioningZone::Battlefield,
                condition: TriggerCondition::SelfEntersBattlefield,
                program: SpellProgram {
                    additional_sacrifice: None,
                    modes: vec![],
                    functioning_zone: FunctioningZone::Stack,
                    timing: SpellTiming::Instant,
                    resolution: SpellResolution::Instructions,
                    targets: vec![],
                    instructions: vec![Instruction::DrawCards {
                        recipient: None,
                        count: 1.into(),
                        source_block: 0
                    }]
                }
            }]
        );
        let warden = get("Essence Warden");
        assert_eq!(
            warden.characteristics.mana_cost,
            ManaCost {
                generic: 0.into(),
                colored: vec![Color::G]
            }
        );
        assert_eq!(
            warden.triggered[0].condition,
            TriggerCondition::OtherCreatureEntersBattlefield
        );
        assert_eq!(
            warden.triggered[0].program.instructions,
            [Instruction::GainLife {
                recipient: LifeRecipient::Controller,
                amount: 1.into(),
                source_block: 0
            }]
        );
        let artist = get("Blood Artist");
        assert_eq!(
            artist.characteristics.mana_cost,
            ManaCost {
                generic: 1.into(),
                colored: vec![Color::B]
            }
        );
        assert_eq!(artist.characteristics.power, Some(0.into()));
        assert_eq!(artist.characteristics.toughness, Some(1.into()));
        assert_eq!(
            artist.triggered[0].condition,
            TriggerCondition::AnyCreatureDies
        );
        assert_eq!(
            artist.triggered[0].program.targets,
            [TargetSpec {
                slot: 0,
                selection_id: "line:0:8".into(),
                chooser: Chooser::Controller,
                stage: EvaluationStage::Announcement,
                filter: None,
                domain: TargetDomain::AnyPlayer
            }]
        );
        assert_eq!(
            artist.triggered[0].program.instructions,
            [
                Instruction::LoseLife {
                    recipient: LifeRecipient::Target { target_slot: 0 },
                    amount: 1.into(),
                    source_block: 0
                },
                Instruction::GainLife {
                    recipient: LifeRecipient::Controller,
                    amount: 1.into(),
                    source_block: 0
                }
            ]
        );
        for d in [artist, warden] {
            let raw = fixtures::TRIGGER_RECORDS
                .iter()
                .find(|(_, raw)| {
                    serde_json::from_str::<serde_json::Value>(raw).unwrap()["oracleIdentity"]
                        == d.oracle_id
                })
                .unwrap()
                .1;
            let record: serde_json::Value = serde_json::from_str(raw).unwrap();
            assert_eq!(record["binding"]["fullyBound"], false);
        }
        for index in [22, 25] {
            assert!(matches!(
                compile(fixtures::MANIFEST, fixtures::TRIGGER_RECORDS[index].1),
                CompileResult::Unsupported { .. }
            ));
        }
    }

    #[test]
    fn trigger_binder_canonicalizes_only_the_exact_disjoint_dies_partition() {
        let reversed = changed_source(fixtures::TRIGGER_RECORDS[11].1, |r| {
            r["faces"][0]["result"]["abilities"][0]["trigger"]["events"]
                .as_array_mut()
                .unwrap()
                .reverse();
            r["faces"][0]["result"]["lines"][0]["ability"] =
                r["faces"][0]["result"]["abilities"][0].clone();
        });
        let CompileResult::Lowered { definition } = compile(fixtures::MANIFEST, &reversed) else {
            panic!("order of disjoint event alternatives must not change their meaning")
        };
        assert_eq!(
            definition.triggered[0].condition,
            TriggerCondition::AnyCreatureDies
        );
        let diagnostic = changed_source(fixtures::TRIGGER_RECORDS[23].1, |r| {
            r["binding"]["fullyBound"] = false.into()
        });
        assert!(matches!(
            compile(fixtures::MANIFEST, &diagnostic),
            CompileResult::Lowered { .. }
        ));
        let renamed = changed_source(fixtures::TRIGGER_RECORDS[24].1, |r| {
            r["source"]["name"] = "Renamed complete event source".into();
            r["faces"][0]["result"]["name"] = "Renamed complete event source".into();
        });
        assert!(matches!(
            compile(fixtures::MANIFEST, &renamed),
            CompileResult::Lowered { .. }
        ));
    }

    #[test]
    fn trigger_lowering_rejects_unbound_alternatives_optional_state_mana_and_deferred_scopes() {
        type Mutation = (&'static str, usize, fn(&mut serde_json::Value));
        let mutations: Vec<Mutation> = vec![
            ("missing self alternative", 11, |r| {
                r["faces"][0]["result"]["abilities"][0]["trigger"]["events"]
                    .as_array_mut()
                    .unwrap()
                    .remove(0);
            }),
            ("duplicate self alternative", 11, |r| {
                r["faces"][0]["result"]["abilities"][0]["trigger"]["events"][1] =
                    r["faces"][0]["result"]["abilities"][0]["trigger"]["events"][0].clone()
            }),
            ("different event", 11, |r| {
                r["faces"][0]["result"]["abilities"][0]["trigger"]["events"][1]["trigger"] =
                    "enters".into()
            }),
            ("unknown event argument", 23, |r| {
                r["faces"][0]["result"]["abilities"][0]["trigger"]["from"] = "graveyard".into()
            }),
            ("unbound other subject", 24, |r| {
                r["faces"][0]["result"]["abilities"][0]["trigger"]["what"]["filter"]["other"] =
                    false.into()
            }),
            ("narrowed controller", 24, |r| {
                r["faces"][0]["result"]["abilities"][0]["trigger"]["what"]["filter"]["control"] =
                    "you".into()
            }),
            ("different subject type", 24, |r| {
                r["faces"][0]["result"]["abilities"][0]["trigger"]["what"]["filter"]["types"] =
                    serde_json::json!(["artifact"])
            }),
            ("nonunit event cardinality", 24, |r| {
                r["faces"][0]["result"]["abilities"][0]["trigger"]["what"]["count"]["value"] =
                    2.into()
            }),
            ("unbound effect pronoun", 11, |r| {
                r["faces"][0]["result"]["abilities"][0]["effects"][0]["steps"][0]["who"]["player"] =
                    "that-player".into()
            }),
            ("unsupported optional draw-two instruction", 23, |r| {
                r["faces"][0]["result"]["abilities"][0]["effects"][0]["steps"][0]["optional"] =
                    true.into();
                r["faces"][0]["result"]["abilities"][0]["effects"][0]["steps"][0]["amount"]["value"] =
                    2.into();
            }),
            ("optional trigger", 23, |r| {
                r["faces"][0]["result"]["abilities"][0]["optional"] = true.into()
            }),
            ("state trigger", 23, |r| {
                r["faces"][0]["result"]["abilities"][0]["trigger"] = serde_json::json!({"trigger":"state","condition":{"condition":"life-total","value":0}})
            }),
            ("mana trigger", 23, |r| {
                r["faces"][0]["result"]["abilities"][0]["effects"][0]["steps"][0] = serde_json::json!({"effect":"add-mana","who":{"player":"you"},"mana":{"mana":"fixed","symbols":["G"]}})
            }),
            ("intervening if", 23, |r| {
                r["faces"][0]["result"]["abilities"][0]["interveningIf"] =
                    serde_json::json!({"condition":"life-total","value":0})
            }),
            ("conditional effect", 23, |r| {
                r["faces"][0]["result"]["abilities"][0]["effects"][0]["conditionKind"] = "if".into()
            }),
            ("delayed trigger", 23, |r| {
                r["faces"][0]["result"]["abilities"][0]["kind"] = "delayed-triggered".into()
            }),
            ("reflexive trigger", 23, |r| {
                r["faces"][0]["result"]["abilities"][0]["kind"] = "reflexive-triggered".into()
            }),
        ];
        for (label, index, change) in mutations {
            let raw = changed_source(fixtures::TRIGGER_RECORDS[index].1, |r| {
                change(r);
                r["binding"]["fullyBound"] = true.into();
                r["faces"][0]["result"]["lines"][0]["ability"] =
                    r["faces"][0]["result"]["abilities"][0].clone();
            });
            let result = compile(fixtures::MANIFEST, &raw);
            assert!(
                matches!(result, CompileResult::Unsupported { .. }),
                "{label}: {result:?}"
            );
        }
        let extra = changed_source(fixtures::TRIGGER_RECORDS[23].1, |r| {
            let original = r["source"]["oracle_text"].as_str().unwrap();
            r["source"]["oracle_text"] = format!("{original}\nProtection from red").into();
            let ability =
                serde_json::json!({"kind":"keywords","keywords":[{"keyword":"protection"}]});
            r["faces"][0]["result"]["abilities"]
                .as_array_mut()
                .unwrap()
                .push(ability.clone());
            r["faces"][0]["result"]["lines"]
                .as_array_mut()
                .unwrap()
                .push(
                    serde_json::json!({"text":"Protection from red","ok":true,"ability":ability}),
                );
        });
        assert!(matches!(
            compile(fixtures::MANIFEST, &extra),
            CompileResult::Unsupported { .. }
        ));
    }

    #[test]
    fn triggered_ir_requires_exact_capability_closure_and_mandatory_stack_programs() {
        let c = embedded_catalog("trigger").unwrap();
        for change in [
            (|d: &mut Definition| d.triggered[0].functioning_zone = FunctioningZone::Stack)
                as fn(&mut Definition),
            |d| d.triggered[0].program.functioning_zone = FunctioningZone::Battlefield,
            |d| d.triggered[0].program.timing = SpellTiming::MainPhase,
            |d| d.triggered[0].program.resolution = SpellResolution::Permanent,
            |d| d.triggered[0].program.targets[0].domain = TargetDomain::OpponentPlayer,
            |d| {
                d.triggered[0].program.instructions[0] = Instruction::LoseLife {
                    recipient: LifeRecipient::Target { target_slot: 1 },
                    amount: 1.into(),
                    source_block: 0,
                }
            },
            |d| {
                d.capabilities.remove(&Capability::CreatureDiesV1);
            },
        ] {
            let mut bad = c.clone();
            change(
                bad.definitions
                    .iter_mut()
                    .find(|d| d.name == "Blood Artist")
                    .unwrap(),
            );
            assert!(Catalog::read(&canonical(&bad).unwrap()).is_err());
        }
        let raw = canonical(&c).unwrap();
        for bad in [
            raw.replace("any_creature_dies", "other_creature_dies"),
            raw.replace(
                "\"condition\":\"any_creature_dies\"",
                "\"condition\":{\"any_creature_dies\":{\"extra\":true}}",
            ),
            raw.replace(
                "\"condition\":\"any_creature_dies\"",
                "\"optional\":true,\"condition\":\"any_creature_dies\"",
            ),
        ] {
            assert_ne!(bad, raw);
            assert!(Catalog::read(&bad).is_err());
        }
        let mut bad = c;
        bad.schema_version = mtg_contracts::CURRENT_IR_VERSION + 1;
        assert!(bad.validate().is_err());
    }

    #[test]
    fn damage_life_pool_retains_all_source_blocks_targets_quantities_and_printed_costs() {
        let c = embedded_catalog("damage-life").unwrap();
        assert_eq!(c.schema_version, CURRENT_IR_VERSION);
        assert_eq!(
            Catalog::read(include_str!(
                "../../../tests/fixtures/expected-damage-life-ir.json"
            ))
            .unwrap(),
            c
        );
        let get = |name: &str| c.definitions.iter().find(|d| d.name == name).unwrap();
        let bolt = get("Lightning Bolt");
        let p = bolt.spell.as_ref().unwrap();
        assert_eq!(
            bolt.characteristics.mana_cost,
            ManaCost {
                generic: 0.into(),
                colored: vec![Color::R]
            }
        );
        assert_eq!(p.targets[0].domain, TargetDomain::AnyDamageTarget);
        assert_eq!(p.targets[0].selection_id, "line:0:6");
        assert_eq!(
            p.instructions,
            [Instruction::DealDamage {
                target_slot: 0,
                amount: 3.into(),
                source_block: 0
            }]
        );
        let sorcerer = get("Prodigal Sorcerer");
        let a = &sorcerer.activated[0];
        assert_eq!(
            sorcerer.characteristics.mana_cost,
            ManaCost {
                generic: 2.into(),
                colored: vec![Color::U]
            }
        );
        assert_eq!(sorcerer.characteristics.power, Some(1.into()));
        assert_eq!(sorcerer.characteristics.toughness, Some(1.into()));
        assert_eq!(a.cost, ActivationCost::TapSource);
        assert_eq!(a.program.timing, SpellTiming::Instant);
        assert_eq!(a.program.targets[0].domain, TargetDomain::AnyDamageTarget);
        assert_eq!(a.program.targets[0].selection_id, "line:0:9");
        assert_eq!(
            a.program.instructions,
            [Instruction::DealDamage {
                target_slot: 0,
                amount: 1.into(),
                source_block: 0
            }]
        );
        let soul = get("Soul Feast");
        let p = soul.spell.as_ref().unwrap();
        assert_eq!(
            soul.characteristics.mana_cost,
            ManaCost {
                generic: 3.into(),
                colored: vec![Color::B, Color::B]
            }
        );
        assert_eq!(p.targets[0].domain, TargetDomain::AnyPlayer);
        assert_eq!(
            p.instructions,
            [
                Instruction::LoseLife {
                    recipient: LifeRecipient::Target { target_slot: 0 },
                    amount: 4.into(),
                    source_block: 0
                },
                Instruction::GainLife {
                    recipient: LifeRecipient::Controller,
                    amount: 4.into(),
                    source_block: 0
                }
            ]
        );
        let hands = get("Healing Hands");
        let p = hands.spell.as_ref().unwrap();
        assert_eq!(
            hands.characteristics.mana_cost,
            ManaCost {
                generic: 2.into(),
                colored: vec![Color::W]
            }
        );
        assert_eq!(
            p.instructions,
            [
                Instruction::GainLife {
                    recipient: LifeRecipient::Target { target_slot: 0 },
                    amount: 4.into(),
                    source_block: 0
                },
                Instruction::DrawCards {
                    recipient: None,
                    count: 1.into(),
                    source_block: 1
                }
            ]
        );
        let nectar = get("Sacred Nectar");
        let p = nectar.spell.as_ref().unwrap();
        assert_eq!(
            nectar.characteristics.mana_cost,
            ManaCost {
                generic: 1.into(),
                colored: vec![Color::W]
            }
        );
        assert!(p.targets.is_empty());
        assert_eq!(p.timing, SpellTiming::MainPhase);
        assert_eq!(
            p.instructions,
            [Instruction::GainLife {
                recipient: LifeRecipient::Controller,
                amount: 4.into(),
                source_block: 0
            }]
        );
        let mountain = get("Mountain");
        assert_eq!(mountain.characteristics.subtypes, ["Mountain"]);
        assert_eq!(mountain.characteristics.color_identity, [Color::R]);
        assert_eq!(basic_land_color("Mountain"), Some(Color::R));
        assert!(matches!(
            compile(fixtures::MANIFEST, fixtures::DAMAGE_LIFE_RECORDS[22].1),
            CompileResult::Unsupported { .. }
        ));
    }

    #[test]
    fn complete_damage_life_sources_reject_unknown_scopes_sources_amounts_and_missing_blocks() {
        type Mutation = (usize, fn(&mut serde_json::Value));
        let mutations: Vec<Mutation> = vec![
            (16, |r| {
                r["faces"][0]["result"]["abilities"][0]["effects"][0]["steps"][0]["source"] =
                    serde_json::json!({"ref":"this","noun":"creature"})
            }),
            (16, |r| {
                r["faces"][0]["result"]["abilities"][0]["effects"][0]["steps"][0]["source"]["extra"] =
                    true.into()
            }),
            (16, |r| {
                r["faces"][0]["result"]["abilities"][0]["effects"][0]["steps"][0]["to"]["controller"] =
                    "you".into()
            }),
            (16, |r| {
                r["faces"][0]["result"]["abilities"][0]["effects"][0]["steps"][0]["amount"]["amount"] =
                    "x".into()
            }),
            (16, |r| {
                r["faces"][0]["result"]["abilities"][0]["effects"][0]["steps"][0]["amount"]["value"] =
                    0.into()
            }),
            (16, |r| {
                r["faces"][0]["result"]["abilities"][0]["effects"][0]["steps"][0]["to"]["selectionId"] =
                    "".into()
            }),
            (17, |r| {
                r["faces"][0]["result"]["abilities"][0]["effects"][0]["steps"][0]["source"] =
                    serde_json::json!({"ref":"self"})
            }),
            (17, |r| {
                r["faces"][0]["result"]["abilities"][0]["costs"]
                    .as_array_mut()
                    .unwrap()
                    .push(serde_json::json!({"cost":"mana","symbols":["U"]}))
            }),
            (18, |r| {
                r["faces"][0]["result"]["abilities"][0]["effects"][0]["steps"][0]["who"]["player"] =
                    "target-opponent".into()
            }),
            (18, |r| {
                r["faces"][0]["result"]["abilities"][0]["effects"][0]["steps"][1]["who"]["selectionId"] =
                    "fake-controller-target".into()
            }),
            (19, |r| {
                r["faces"][0]["result"]["abilities"][1]["effects"][0]["steps"][0]["amount"]["value"] =
                    3.into()
            }),
            (20, |r| {
                r["faces"][0]["result"]["abilities"][0]["effects"][0]["steps"][0]["who"]["player"] =
                    "each-player".into()
            }),
        ];
        for (index, change) in mutations {
            let raw = changed_source(fixtures::DAMAGE_LIFE_RECORDS[index].1, |r| {
                change(r);
                for block in 0..r["faces"][0]["result"]["abilities"]
                    .as_array()
                    .unwrap()
                    .len()
                {
                    r["faces"][0]["result"]["lines"][block]["ability"] =
                        r["faces"][0]["result"]["abilities"][block].clone();
                }
            });
            let result = compile(fixtures::MANIFEST, &raw);
            assert!(
                matches!(result, CompileResult::Unsupported { .. }),
                "index {index}: {result:?}"
            );
        }
        let raw = changed_source(fixtures::DAMAGE_LIFE_RECORDS[19].1, |r| {
            r["faces"][0]["result"]["abilities"]
                .as_array_mut()
                .unwrap()
                .pop();
            r["faces"][0]["result"]["lines"]
                .as_array_mut()
                .unwrap()
                .pop();
        });
        assert!(matches!(
            compile(fixtures::MANIFEST, &raw),
            CompileResult::Invalid { .. }
        ));
        let raw = changed_source(fixtures::DAMAGE_LIFE_RECORDS[16].1, |r| {
            r["source"]["name"] = "Renamed damage source".into();
            r["faces"][0]["result"]["name"] = "Renamed damage source".into();
            r["source"]["oracle_text"] =
                "Renamed damage source deals 3 damage to any target.".into();
        });
        assert!(matches!(
            compile(fixtures::MANIFEST, &raw),
            CompileResult::Lowered { .. }
        ));
    }

    #[test]
    fn damage_life_programs_require_strict_target_recipient_quantity_closure() {
        let catalog = embedded_catalog("damage-life").unwrap();
        let mut bad = catalog.clone();
        bad.schema_version = mtg_contracts::CURRENT_IR_VERSION + 1;
        assert!(Catalog::read(&canonical(&bad).unwrap()).is_err());
        for change in [
            (|d: &mut Definition| {
                d.spell.as_mut().unwrap().targets[0].domain = TargetDomain::BattlefieldCreature
            }) as fn(&mut Definition),
            |d| {
                d.spell.as_mut().unwrap().instructions[0] = Instruction::DealDamage {
                    target_slot: 1,
                    amount: 3.into(),
                    source_block: 0,
                }
            },
            |d| {
                d.spell.as_mut().unwrap().instructions[0] = Instruction::DealDamage {
                    target_slot: 0,
                    amount: Quantity::zero(),
                    source_block: 0,
                }
            },
            |d| {
                d.capabilities.remove(&Capability::NoncombatDamageV1);
            },
        ] {
            let mut bad = catalog.clone();
            change(
                bad.definitions
                    .iter_mut()
                    .find(|d| d.name == "Lightning Bolt")
                    .unwrap(),
            );
            assert!(Catalog::read(&canonical(&bad).unwrap()).is_err());
        }
        for change in [
            (|d: &mut Definition| {
                d.spell.as_mut().unwrap().instructions[0] = Instruction::LoseLife {
                    recipient: LifeRecipient::Controller,
                    amount: 4.into(),
                    source_block: 0,
                }
            }) as fn(&mut Definition),
            |d| {
                d.spell.as_mut().unwrap().instructions[0] = Instruction::LoseLife {
                    recipient: LifeRecipient::Target { target_slot: 99 },
                    amount: 4.into(),
                    source_block: 0,
                }
            },
            |d| {
                d.spell.as_mut().unwrap().instructions[0] = Instruction::LoseLife {
                    recipient: LifeRecipient::Target { target_slot: 0 },
                    amount: Quantity::from(-4),
                    source_block: 0,
                }
            },
        ] {
            let mut bad = catalog.clone();
            let d = bad
                .definitions
                .iter_mut()
                .find(|d| d.name == "Soul Feast")
                .unwrap();
            change(d);
            d.capabilities = d.requirements();
            assert!(Catalog::read(&canonical(&bad).unwrap()).is_err());
        }
        let raw = canonical(&catalog).unwrap();
        for (from, to) in [
            ("any_damage_target", "creature_or_player"),
            ("any_player", "opponent_only"),
            (
                "\"recipient\":\"controller\"",
                "\"recipient\":{\"controller\":{\"extra\":true}}",
            ),
        ] {
            let bad = raw.replace(from, to);
            assert_ne!(bad, raw);
            assert!(Catalog::read(&bad).is_err());
        }
    }

    #[test]
    fn mixed_spell_programs_and_activation_shapes_are_checked() {
        let mut c = embedded_catalog("base").unwrap();
        let d = c
            .definitions
            .iter_mut()
            .find(|d| d.name == "Unsummon")
            .unwrap();
        d.spell
            .as_mut()
            .unwrap()
            .instructions
            .push(Instruction::DrawCards {
                recipient: None,
                count: 1.into(),
                source_block: 1,
            });
        d.capabilities = d.requirements();
        c.validate().unwrap();
        let mut c = embedded_catalog("damage-life").unwrap();
        let d = c
            .definitions
            .iter_mut()
            .find(|d| d.name == "Prodigal Sorcerer")
            .unwrap();
        d.activated[0].program.instructions = vec![Instruction::DrawCards {
            recipient: None,
            count: 1.into(),
            source_block: 0,
        }];
        d.capabilities = d.requirements();
        assert!(c.validate().is_err());
    }

    #[test]
    fn stack_pool_retains_complete_counter_domains_and_costs() {
        let catalog = embedded_catalog("stack").unwrap();
        assert_eq!(catalog.schema_version, CURRENT_IR_VERSION);
        assert_eq!(
            Catalog::read(include_str!(
                "../../../tests/fixtures/expected-stack-ir.json"
            ))
            .unwrap(),
            catalog
        );
        for (name, domain, colored) in [
            (
                "Counterspell",
                TargetDomain::StackSpell,
                vec![Color::U, Color::U],
            ),
            (
                "Stifle",
                TargetDomain::StackActivatedOrTriggeredAbility,
                vec![Color::U],
            ),
        ] {
            let d = catalog.definitions.iter().find(|d| d.name == name).unwrap();
            assert_eq!(
                d.characteristics.mana_cost,
                ManaCost {
                    generic: Quantity::zero(),
                    colored
                }
            );
            let p = d.spell.as_ref().unwrap();
            assert_eq!(p.targets.len(), 1);
            assert_eq!(p.targets[0].domain, domain);
            assert_eq!(
                p.instructions,
                [Instruction::CounterStackObject {
                    target_slot: 0,
                    source_block: 0
                }]
            );
        }
        let looter = catalog
            .definitions
            .iter()
            .find(|d| d.name == "Merfolk Looter")
            .unwrap();
        assert_eq!(looter.activated[0].cost, ActivationCost::TapSource);
        assert_eq!(
            looter.activated[0].program.instructions,
            [
                Instruction::DrawCards {
                    recipient: None,
                    count: Quantity::from(1),
                    source_block: 0
                },
                Instruction::DiscardCards {
                    recipient: None,
                    count: Quantity::from(1),
                    source_block: 0
                },
            ]
        );
        let mage = catalog
            .definitions
            .iter()
            .find(|d| d.name == "Azure Mage")
            .unwrap();
        assert_eq!(
            mage.activated[0].cost,
            ActivationCost::Mana {
                cost: ManaCost {
                    generic: Quantity::from(3),
                    colored: vec![Color::U]
                }
            }
        );
        assert_eq!(
            mage.activated[0].program.instructions,
            [Instruction::DrawCards {
                recipient: None,
                count: Quantity::from(1),
                source_block: 0
            }]
        );
        for d in [looter, mage] {
            assert_eq!(
                d.characteristics.mana_cost,
                ManaCost {
                    generic: Quantity::from(1),
                    colored: vec![Color::U]
                }
            );
            assert_eq!(d.activated.len(), 1);
        }
    }

    #[test]
    fn complete_stack_sources_reject_narrowed_filters_combined_costs_and_extra_effects() {
        type Mutation = (usize, fn(&mut serde_json::Value));
        let mutations: Vec<Mutation> = vec![
            (12, |r| {
                r["faces"][0]["result"]["abilities"][0]["effects"][0]["steps"][0]["what"]["filter"]
                    ["controller"] = "you".into()
            }),
            (12, |r| {
                r["faces"][0]["result"]["abilities"][0]["effects"][0]["steps"][0]["what"]["count"]
                    ["value"] = 2.into()
            }),
            (13, |r| {
                r["faces"][0]["result"]["abilities"][0]["effects"][0]["steps"][0]["what"]["filter"]
                    ["anyOf"] = serde_json::json!([{"abilityKind":"activated"}])
            }),
            (13, |r| {
                r["faces"][0]["result"]["abilities"][0]["effects"][0]["steps"][0]["what"]["filter"]
                    ["anyOf"][1]["abilityKind"] = "mana".into()
            }),
            (13, |r| {
                r["faces"][0]["result"]["abilities"][0]["effects"][0]["steps"][0]["what"]["selectionId"] =
                    "".into()
            }),
            (14, |r| {
                r["faces"][0]["result"]["abilities"][0]["costs"]
                    .as_array_mut()
                    .unwrap()
                    .push(serde_json::json!({"cost":"mana","symbols":["U"]}))
            }),
            (14, |r| {
                r["faces"][0]["result"]["abilities"][0]["costs"][0]["what"] =
                    serde_json::json!({"ref":"another","noun":"creature"})
            }),
            (15, |r| {
                r["faces"][0]["result"]["abilities"][0]["costs"][0]["symbols"] =
                    serde_json::json!(["X", "U"])
            }),
            (15, |r| {
                r["faces"][0]["result"]["abilities"][0]["costs"][0]["symbols"] =
                    serde_json::json!(["3}{U"])
            }),
            (15, |r| {
                r["faces"][0]["result"]["abilities"][0]["costs"][0]["symbols"] =
                    serde_json::json!([])
            }),
            (15, |r| {
                r["faces"][0]["result"]["abilities"][0]["effects"][0]["steps"].as_array_mut().unwrap().push(serde_json::json!({"effect":"gain-life","amount":{"amount":"fixed","value":1},"who":{"player":"you"}}))
            }),
        ];
        for (index, change) in mutations {
            let raw = changed_source(fixtures::STACK_RECORDS[index].1, |r| {
                change(r);
                r["faces"][0]["result"]["lines"][0]["ability"] =
                    r["faces"][0]["result"]["abilities"][0].clone();
            });
            let result = compile(fixtures::MANIFEST, &raw);
            assert!(
                matches!(result, CompileResult::Unsupported { .. }),
                "index {index}: {result:?}"
            );
        }
        for (_, source) in &fixtures::STACK_RECORDS[12..] {
            let raw = changed_source(source, |r| {
                r["source"]["name"] = "Renamed complete source".into();
                r["faces"][0]["result"]["name"] = "Renamed complete source".into();
            });
            assert!(matches!(
                compile(fixtures::MANIFEST, &raw),
                CompileResult::Lowered { .. }
            ));
            let raw = changed_source(source, |r| {
                let original = r["source"]["oracle_text"].as_str().unwrap();
                r["source"]["oracle_text"] = format!("{original}\nMenace").into();
            });
            assert!(matches!(
                compile(fixtures::MANIFEST, &raw),
                CompileResult::Invalid { .. }
            ));
        }
    }

    #[test]
    fn stack_vocabulary_rejects_bad_binding_and_capability_closure() {
        let catalog = embedded_catalog("stack").unwrap();
        for change in [
            (|d: &mut Definition| {
                d.spell.as_mut().unwrap().targets[0].domain = TargetDomain::BattlefieldCreature
            }) as fn(&mut Definition),
            |d| {
                d.spell.as_mut().unwrap().instructions[0] = Instruction::CounterStackObject {
                    target_slot: 1,
                    source_block: 0,
                }
            },
            |d| {
                d.capabilities.remove(&Capability::CounterStackObjectV1);
            },
        ] {
            let mut bad = catalog.clone();
            let d = bad
                .definitions
                .iter_mut()
                .find(|d| d.name == "Counterspell")
                .unwrap();
            change(d);
            assert!(Catalog::read(&canonical(&bad).unwrap()).is_err());
        }
        let mut bad = catalog.clone();
        let d = bad
            .definitions
            .iter_mut()
            .find(|d| d.name == "Azure Mage")
            .unwrap();
        d.activated[0].cost = ActivationCost::Mana {
            cost: ManaCost {
                generic: Quantity::from(-1),
                colored: vec![],
            },
        };
        assert!(Catalog::read(&canonical(&bad).unwrap()).is_err());
        let raw = canonical(&catalog).unwrap();
        for bad in [
            raw.replace(
                "stack_activated_or_triggered_ability",
                "stack_activated_ability",
            ),
            raw.replace("\"cost\":{\"mana\":", "\"cost\":{\"unknown\":"),
            raw.replace("\"tap_source\"", "{\"tap_source\":{\"extra\":true}}"),
        ] {
            assert!(Catalog::read(&bad).is_err());
        }
    }

    #[test]
    fn rehashed_unsupported_semantics_are_not_hidden_by_hash_rejection() {
        type Mutation = (usize, fn(&mut serde_json::Value));
        let mutations: Vec<Mutation> = vec![
            (3, |r| {
                r["faces"][0]["result"]["abilities"][0]["effects"][0]["steps"][0]["unknown"] =
                    true.into();
            }),
            (3, |r| {
                r["faces"][0]["result"]["abilities"][0]["effects"][0]["steps"][0]["what"]["selectionId"] =
                    "".into();
                r["faces"][0]["result"]["lines"][0]["ability"] =
                    r["faces"][0]["result"]["abilities"][0].clone();
            }),
            (3, |r| {
                r["source"]["layout"] = "transform".into();
            }),
            (3, |r| {
                r["faces"][0]["result"]["abilities"] =
                    serde_json::json!([{"kind":"keywords","keywords":[{"keyword":"protection"}]}]);
            }),
            (1, |r| {
                r["source"]["toughness"] = "*".into();
            }),
            (1, |r| {
                r["source"].as_object_mut().unwrap().remove("power");
            }),
            (2, |r| {
                r["source"]["oracle_text"] = "Menace".into();
            }),
            (3, |r| {
                r["source"]["mana_cost"] = "{X}{U}".into();
            }),
            (3, |r| {
                r["source"]["oracle_text"] =
                    "Return target creature to its owner's hand.\nDraw a card.".into();
            }),
        ];
        for (index, mutation) in mutations {
            let result = compile(fixtures::MANIFEST, &changed_record(index, mutation));
            assert!(
                !matches!(result, CompileResult::Lowered { .. }),
                "unexpected lowering: {result:?}"
            );
        }
    }
    #[test]
    fn external_metadata_and_display_names_do_not_supply_semantics() {
        let r = changed_record(3, |r| {
            r["source"]["unconsumed_metadata"] = serde_json::json!({"unicode":"Δ","number":1.5});
            r["source"]["name"] = "Renamed source fixture".into();
            r["faces"][0]["result"]["name"] = "Renamed source fixture".into();
        });
        assert!(matches!(
            compile(fixtures::MANIFEST, &r),
            CompileResult::Lowered { .. }
        ));
    }
    #[test]
    fn lowers_intrinsics_and_complete_spell_without_name_dispatch() {
        let c = embedded_catalog("base").unwrap();
        let island = c.definitions.iter().find(|d| d.name == "Island").unwrap();
        assert!(island.spell.is_none());
        assert_eq!(island.intrinsic_zone, Some(FunctioningZone::Battlefield));
        let spell = c
            .definitions
            .iter()
            .find(|d| d.name == "Unsummon")
            .unwrap()
            .spell
            .as_ref()
            .unwrap();
        assert_eq!(spell.targets[0].selection_id, "line:0:1");
        assert_eq!(
            spell.instructions,
            [Instruction::ReturnToOwnerHand {
                target_slot: 0,
                source_block: 0
            }]
        );
        assert_eq!(Catalog::read(&canonical(&c).unwrap()).unwrap(), c);
    }
    #[test]
    fn opponent_sacrifice_lowers_as_resolution_choice() {
        let CompileResult::Lowered { definition } =
            compile(fixtures::MANIFEST, fixtures::RECORDS[5].1)
        else {
            panic!("complete opponent sacrifice source must lower")
        };
        let program = definition.spell.as_ref().unwrap();
        assert_eq!(definition.characteristics.kind, CardKind::Sorcery);
        assert_eq!(program.timing, SpellTiming::MainPhase);
        assert_eq!(
            program.targets,
            [TargetSpec {
                slot: 0,
                selection_id: "line:0:0".into(),
                chooser: Chooser::Controller,
                stage: EvaluationStage::Announcement,
                filter: None,
                domain: TargetDomain::OpponentPlayer
            }]
        );
        assert_eq!(
            program.instructions,
            [Instruction::TargetPlayerSacrificeCreature {
                target_slot: 0,
                source_block: 0
            }]
        );
        assert!(definition.activated.is_empty());
    }
    #[test]
    fn rejects_hash_version_unknown_field_and_incomplete_face() {
        let raw = fixtures::RECORDS[3].1;
        for bad in [
            raw.replace("\"status\":\"parsed\"", "\"status\":\"unsupported\""),
            raw.replacen(
                "\"sourceIndex\":",
                "\"extraSemanticField\":true,\"sourceIndex\":",
                1,
            ),
            raw.replace("Return target creature", "Destroy target creature"),
            raw.replace("\"sourceFaceIndex\":0", "\"sourceFaceIndex\":1"),
        ] {
            assert!(!matches!(
                compile(fixtures::MANIFEST, &bad),
                CompileResult::Lowered { .. }
            ));
        }
        let bad = fixtures::MANIFEST.replace("\"schemaVersion\":3", "\"schemaVersion\":4");
        assert!(matches!(compile(&bad, raw), CompileResult::Invalid { .. }));
    }
    #[test]
    fn semantic_ir_is_strict_and_capability_checked() {
        let c = embedded_catalog("base").unwrap();
        let raw = canonical(&c).unwrap();
        for bad in [
            raw.replace(
                "\"schema_version\":1",
                &format!(
                    "\"schema_version\":{}",
                    mtg_contracts::CURRENT_IR_VERSION + 1
                ),
            ),
            raw.replace("\"op\":\"return_to_owner_hand\"", "\"op\":\"execute_text\""),
            raw.replace("\"target_slot\":0", "\"target_slot\":99"),
            raw.replace("\"source_block\":0", "\"unknown\":true,\"source_block\":0"),
            raw.replace(
                "\"functioning_zone\":\"stack\"",
                "\"functioning_zone\":\"battlefield\"",
            ),
        ] {
            assert!(Catalog::read(&bad).is_err());
        }
    }
    #[test]
    fn vanilla_creature_spells_and_land_permissions_are_explicit() {
        let catalog = embedded_catalog("base").unwrap();
        assert_eq!(catalog.schema_version, CURRENT_IR_VERSION);
        for d in &catalog.definitions {
            match d.characteristics.kind {
                CardKind::Land => {
                    assert!(d.spell.is_none());
                    assert_eq!(
                        d.capabilities,
                        BTreeSet::from([
                            Capability::NormalLayoutV1,
                            Capability::BasicLandManaV1,
                            Capability::LandPlayV1,
                        ])
                    );
                }
                CardKind::Creature => {
                    let spell = d.spell.as_ref().unwrap();
                    assert_eq!(spell.timing, SpellTiming::MainPhase);
                    assert_eq!(spell.resolution, SpellResolution::Permanent);
                    assert_eq!(spell.functioning_zone, FunctioningZone::Stack);
                    assert!(spell.targets.is_empty());
                    assert!(spell.instructions.is_empty());
                    assert!(d.capabilities.contains(&Capability::CastCreatureV1));
                    assert_eq!(
                        d.capabilities.contains(&Capability::CommanderIdentityV1),
                        d.characteristics.legendary
                    );
                }
                CardKind::Instant => {
                    let spell = d.spell.as_ref().unwrap();
                    assert_eq!(spell.timing, SpellTiming::Instant);
                    assert_eq!(spell.resolution, SpellResolution::Instructions);
                    assert!(!spell.instructions.is_empty());
                }
                CardKind::Sorcery | CardKind::Enchantment | CardKind::Artifact => {
                    panic!("base fixture contains an unexpected card kind")
                }
            }
            for capability in &d.capabilities {
                let mut missing = d.clone();
                missing.capabilities.remove(capability);
                assert!(missing.validate().is_err(), "missing {capability:?}");
            }
        }
        // Same base semantics on a different source identity/display name must
        // lower; runtime exact-profile admission remains a separate check.
        let renamed = changed_record(1, |r| {
            r["source"]["name"] = "Renamed vanilla source".into();
            r["faces"][0]["result"]["name"] = "Renamed vanilla source".into();
        });
        assert!(matches!(
            compile(fixtures::MANIFEST, &renamed),
            CompileResult::Lowered { .. }
        ));
    }

    #[test]
    fn commander_pool_adds_only_exact_source_backed_white_mana() {
        let catalog = embedded_catalog("commander").unwrap();
        assert_eq!(catalog.schema_version, CURRENT_IR_VERSION);
        let plains = catalog
            .definitions
            .iter()
            .find(|definition| definition.oracle_id == "bc71ebf6-2056-41f7-be35-b2e5c34afa99")
            .unwrap();
        assert_eq!(plains.name, "Plains");
        assert_eq!(plains.provenance.source_index, 28278);
        assert_eq!(plains.characteristics.kind, CardKind::Land);
        assert_eq!(plains.characteristics.subtypes, ["Plains"]);
        assert!(plains.characteristics.basic);
        assert_eq!(plains.characteristics.colors, []);
        assert_eq!(plains.characteristics.color_identity, [Color::W]);
        assert_eq!(plains.characteristics.mana_cost.generic, Quantity::zero());
        assert!(plains.characteristics.mana_cost.colored.is_empty());
        assert_eq!(
            basic_land_color(&plains.characteristics.subtypes[0]),
            Some(Color::W)
        );
        assert_eq!(plains.intrinsic_zone, Some(FunctioningZone::Battlefield));
        assert!(plains.spell.is_none());
        assert_eq!(plains.provenance.abilities_json, "[]");

        let commander = catalog
            .definitions
            .iter()
            .find(|definition| definition.oracle_id == "cfb756dd-6fe1-430a-8f04-8b10cac62a86")
            .unwrap();
        assert!(commander.characteristics.legendary);
        assert_eq!(
            commander.characteristics.mana_cost.generic,
            Quantity::zero()
        );
        assert_eq!(commander.characteristics.mana_cost.colored, [Color::W]);
        assert_eq!(commander.characteristics.color_identity, [Color::W]);
        assert!(
            commander
                .capabilities
                .contains(&Capability::CommanderIdentityV1)
        );
        // Command-zone permission and the additional cost belong to game rules:
        // they do not rewrite source mana cost or the card's complete program.
        assert_eq!(
            commander.spell.as_ref().unwrap().timing,
            SpellTiming::MainPhase
        );
        assert_eq!(
            catalog,
            Catalog::read(include_str!(
                "../../../tests/fixtures/expected-commander-ir.json"
            ))
            .unwrap()
        );
        assert_eq!(
            Catalog::read(&canonical(&catalog).unwrap()).unwrap(),
            catalog
        );
    }

    #[test]
    fn white_mana_fixture_does_not_admit_additional_land_semantics() {
        type Mutation = fn(&mut serde_json::Value);
        let mutations: Vec<Mutation> = vec![
            |record| record["source"]["type_line"] = "Land — Plains Island".into(),
            |record| record["source"]["type_line"] = "Legendary Land — Plains".into(),
            |record| record["source"]["oracle_text"] = "{T}: Add {W}.".into(),
            |record| record["source"]["oracle_text"] = "When this land enters, draw a card.".into(),
            |record| record["source"]["card_faces"] = serde_json::json!([{}]),
        ];
        for mutation in mutations {
            let result = compile(fixtures::MANIFEST, &changed_record(6, mutation));
            assert!(
                !matches!(result, CompileResult::Lowered { .. }),
                "unsupported land semantics disappeared: {result:?}"
            );
        }
        let renamed = changed_record(6, |record| {
            record["source"]["name"] = "Renamed white basic source".into();
            record["faces"][0]["result"]["name"] = "Renamed white basic source".into();
        });
        let CompileResult::Lowered { definition } = compile(fixtures::MANIFEST, &renamed) else {
            panic!("intrinsic mana must follow basic subtype rather than card name");
        };
        assert_eq!(
            basic_land_color(&definition.characteristics.subtypes[0]),
            Some(Color::W)
        );
    }

    #[test]
    fn library_catalog_lowers_complete_source_with_explicit_top_and_sorcery_timing() {
        let catalog = embedded_catalog("library").unwrap();
        assert_eq!(catalog.schema_version, CURRENT_IR_VERSION);
        let ebb = catalog
            .definitions
            .iter()
            .find(|d| d.oracle_id == "30cc8f7b-3c28-40f5-8f8f-157e8212280b")
            .unwrap();
        assert_eq!(ebb.name, "Time Ebb");
        assert_eq!(ebb.provenance.source_index, 7282);
        assert_eq!(ebb.characteristics.kind, CardKind::Sorcery);
        assert_eq!(
            ebb.characteristics.mana_cost,
            ManaCost {
                generic: Quantity::from(2),
                colored: vec![Color::U]
            }
        );
        assert_eq!(ebb.characteristics.colors, [Color::U]);
        assert_eq!(ebb.characteristics.color_identity, [Color::U]);
        assert_eq!(
            ebb.spell,
            Some(SpellProgram {
                additional_sacrifice: None,
                modes: vec![],
                timing: SpellTiming::MainPhase,
                resolution: SpellResolution::Instructions,
                functioning_zone: FunctioningZone::Stack,
                targets: vec![TargetSpec {
                    slot: 0,
                    selection_id: "line:0:1".into(),
                    chooser: Chooser::Controller,
                    stage: EvaluationStage::Announcement,
                    filter: None,
                    domain: TargetDomain::BattlefieldCreature
                }],
                instructions: vec![Instruction::PutOnTopOwnerLibrary {
                    target_slot: 0,
                    source_block: 0
                }],
            })
        );
        assert_eq!(
            ebb.capabilities,
            BTreeSet::from([
                Capability::NormalLayoutV1,
                Capability::CastSorceryV1,
                Capability::CreatureTargetV1,
                Capability::OwnerLibraryTopMoveV1
            ])
        );
        assert_eq!(
            Catalog::read(include_str!(
                "../../../tests/fixtures/expected-library-ir.json"
            ))
            .unwrap(),
            catalog
        );
        assert_eq!(
            Catalog::read(&canonical(&catalog).unwrap()).unwrap(),
            catalog
        );
        for capability in &ebb.capabilities {
            let mut changed = ebb.clone();
            changed.capabilities.remove(capability);
            assert!(changed.validate().is_err());
        }
        let renamed = changed_record(7, |r| {
            r["source"]["name"] = "Renamed library-top sorcery".into();
            r["faces"][0]["result"]["name"] = "Renamed library-top sorcery".into();
        });
        let CompileResult::Lowered { definition } = compile(fixtures::MANIFEST, &renamed) else {
            panic!("library movement cannot depend on display names")
        };
        assert_eq!(definition.spell, ebb.spell);
    }

    #[test]
    fn damage_pool_is_exact_empty_text_legendary_without_new_ability_vocabulary() {
        let catalog = embedded_catalog("damage").unwrap();
        assert_eq!(catalog.schema_version, CURRENT_IR_VERSION);
        let commander = catalog
            .definitions
            .iter()
            .find(|d| d.oracle_id == "980e1721-3717-4425-95a4-938dbf2ac661")
            .unwrap();
        assert_eq!(commander.name, "Yargle and Multani");
        assert_eq!(commander.provenance.source_index, 22862);
        assert_eq!(commander.characteristics.kind, CardKind::Creature);
        assert!(commander.characteristics.legendary);
        assert_eq!(commander.characteristics.power, Some(Quantity::from(18)));
        assert_eq!(commander.characteristics.toughness, Some(Quantity::from(6)));
        assert_eq!(
            commander.characteristics.mana_cost,
            ManaCost {
                generic: Quantity::from(3),
                colored: vec![Color::B, Color::B, Color::G]
            }
        );
        assert_eq!(commander.characteristics.colors, [Color::B, Color::G]);
        assert_eq!(
            commander.characteristics.color_identity,
            [Color::B, Color::G]
        );
        assert_eq!(commander.provenance.abilities_json, "[]");
        assert_eq!(
            commander.capabilities,
            BTreeSet::from([
                Capability::NormalLayoutV1,
                Capability::BaseCreatureV1,
                Capability::CastCreatureV1,
                Capability::CommanderIdentityV1
            ])
        );
        let spell = commander.spell.as_ref().unwrap();
        assert_eq!(spell.resolution, SpellResolution::Permanent);
        assert!(spell.instructions.is_empty());
        assert_eq!(
            Catalog::read(include_str!(
                "../../../tests/fixtures/expected-damage-ir.json"
            ))
            .unwrap(),
            catalog
        );
        assert_eq!(
            Catalog::read(&canonical(&catalog).unwrap()).unwrap(),
            catalog
        );
        for (text, ability) in [
            (
                "Indestructible",
                serde_json::json!({"kind":"keywords","keywords":[{"keyword":"indestructible"}]}),
            ),
            (
                "When this creature attacks, draw a card.",
                serde_json::json!({"kind":"triggered","effects":[]}),
            ),
        ] {
            let bad = changed_source(fixtures::DAMAGE_RECORDS[7].1, |r| {
                r["source"]["oracle_text"] = text.into();
                r["faces"][0]["result"]["lines"] =
                    serde_json::json!([{"text":text,"ok":true,"ability":ability}]);
                r["faces"][0]["result"]["abilities"] = serde_json::json!([ability]);
            });
            assert!(matches!(
                compile(fixtures::MANIFEST, &bad),
                CompileResult::Unsupported { .. }
            ));
        }
        for mutate in [
            (|r: &mut serde_json::Value| r["source"]["power"] = "*".into())
                as fn(&mut serde_json::Value),
            |r| r["source"]["toughness"] = "0".into(),
            |r| r["source"]["mana_cost"] = "{X}{B}{B}{G}".into(),
        ] {
            let bad = changed_source(fixtures::DAMAGE_RECORDS[7].1, mutate);
            assert!(matches!(
                compile(fixtures::MANIFEST, &bad),
                CompileResult::Unsupported { .. }
            ));
        }
    }

    #[test]
    fn action_pool_retains_printed_costs_and_complete_distinct_activation_program() {
        let catalog = embedded_catalog("action").unwrap();
        assert_eq!(catalog.schema_version, CURRENT_IR_VERSION);
        let researcher = catalog
            .definitions
            .iter()
            .find(|d| d.name == "Hapless Researcher")
            .unwrap();
        assert_eq!(researcher.provenance.source_index, 5423);
        assert_eq!(
            researcher.characteristics.mana_cost,
            ManaCost {
                generic: Quantity::zero(),
                colored: vec![Color::U]
            }
        );
        assert_eq!(researcher.characteristics.power, Some(Quantity::from(1)));
        assert_eq!(
            researcher.characteristics.toughness,
            Some(Quantity::from(1))
        );
        assert_eq!(
            researcher.activated,
            [ActivatedAbility {
                functioning_zone: FunctioningZone::Battlefield,
                cost: ActivationCost::SacrificeSource,
                program: SpellProgram {
                    additional_sacrifice: None,
                    modes: vec![],
                    timing: SpellTiming::Instant,
                    resolution: SpellResolution::Instructions,
                    functioning_zone: FunctioningZone::Stack,
                    targets: vec![],
                    instructions: vec![
                        Instruction::DrawCards {
                            recipient: None,
                            count: Quantity::from(1),
                            source_block: 0
                        },
                        Instruction::DiscardCards {
                            recipient: None,
                            count: Quantity::from(1),
                            source_block: 0
                        }
                    ]
                }
            }]
        );
        let creature_spell = researcher.spell.as_ref().unwrap();
        assert_eq!(creature_spell.resolution, SpellResolution::Permanent);
        assert!(creature_spell.instructions.is_empty());
        assert_eq!(
            researcher.capabilities,
            BTreeSet::from([
                Capability::NormalLayoutV1,
                Capability::BaseCreatureV1,
                Capability::CastCreatureV1,
                Capability::ActivatedAbilityV1,
                Capability::SacrificeSourceCostV1,
                Capability::ControllerDrawV1,
                Capability::ControllerDiscardV1
            ])
        );
        let edict = catalog
            .definitions
            .iter()
            .find(|d| d.name == "Cruel Edict")
            .unwrap();
        assert_eq!(
            edict.characteristics.mana_cost,
            ManaCost {
                generic: Quantity::from(1),
                colored: vec![Color::B]
            }
        );
        assert!(edict.capabilities.contains(&Capability::OpponentTargetV1));
        assert!(!edict.capabilities.contains(&Capability::CreatureTargetV1));
        let swamp = catalog
            .definitions
            .iter()
            .find(|d| d.name == "Swamp")
            .unwrap();
        assert_eq!(swamp.characteristics.subtypes, ["Swamp"]);
        assert_eq!(
            basic_land_color(&swamp.characteristics.subtypes[0]),
            Some(Color::B)
        );
        assert_eq!(swamp.provenance.source_index, 12962);
        assert_eq!(
            Catalog::read(include_str!(
                "../../../tests/fixtures/expected-action-ir.json"
            ))
            .unwrap(),
            catalog
        );
        assert_eq!(
            Catalog::read(&canonical(&catalog).unwrap()).unwrap(),
            catalog
        );
        for (_, raw) in fixtures::ACTION_RECORDS {
            assert!(matches!(
                compile(fixtures::MANIFEST, raw),
                CompileResult::Lowered { .. }
            ));
        }
        for capability in &researcher.capabilities {
            let mut bad = researcher.clone();
            bad.capabilities.remove(capability);
            assert!(bad.validate().is_err());
        }
    }

    #[test]
    fn optional_draw_single_card_variants_preserve_existing_entry_and_activation_sequences() {
        fn program(d: &Definition) -> &SpellProgram {
            if let Some(a) = d.activated.first() {
                &a.program
            } else {
                &d.triggered[0].program
            }
        }
        // Synthetic optional-one-card AST variants of Elvish Visionary and
        // Hapless Researcher. CR 603.5/608.2d and the optional draw instruction admit
        // this choice; it does not make the following discard conditional.
        for raw in [
            fixtures::TRIGGER_RECORDS[23].1,
            fixtures::ACTION_RECORDS[9].1,
        ] {
            let CompileResult::Lowered {
                definition: original,
            } = compile(fixtures::MANIFEST, raw)
            else {
                panic!("original archived source")
            };
            let optional = changed_source(raw, |r| {
                r["faces"][0]["result"]["abilities"][0]["effects"][0]["steps"][0]["optional"] =
                    true.into();
                r["faces"][0]["result"]["lines"][0]["ability"] =
                    r["faces"][0]["result"]["abilities"][0].clone();
            });
            let CompileResult::Lowered { definition } = compile(fixtures::MANIFEST, &optional)
            else {
                panic!("optional controller draw")
            };
            assert_eq!(
                program(&definition).instructions[0],
                Instruction::OptionalDrawCards { source_block: 0 }
            );
            assert_eq!(
                program(&definition).instructions[1..],
                program(&original).instructions[1..]
            );
            assert!(
                definition
                    .capabilities
                    .contains(&Capability::OptionalControllerDrawV1)
            );
        }
    }

    #[test]
    fn complete_action_lowering_rejects_wrong_costs_choosers_cardinality_and_extra_clauses() {
        type Mutation = fn(&mut serde_json::Value);
        let activation_mutations: Vec<Mutation> = vec![
            |a| a["costs"][0]["cost"] = "tap".into(),
            |a| a["costs"][0]["what"]["ref"] = "target".into(),
            |a| a["costs"][0]["what"]["noun"] = "artifact".into(),
            |a| a["costs"] = serde_json::json!([]),
            |a| {
                let cost = a["costs"][0].clone();
                a["costs"].as_array_mut().unwrap().push(cost);
            },
            |a| a["effects"][0]["steps"][0]["who"]["player"] = "opponent".into(),
            |a| a["effects"][0]["steps"][0]["amount"]["value"] = 2.into(),
            |a| {
                // Optional controller draw supports exactly one card; optional draw-two
                // remains an unsupported complete shape.
                a["effects"][0]["steps"][0]["optional"] = true.into();
                a["effects"][0]["steps"][0]["amount"]["value"] = 2.into();
            },
            |a| a["effects"][0]["steps"][1]["what"]["filter"]["cls"] = "creature".into(),
            |a| a["effects"][0]["steps"][1]["what"]["count"]["value"] = 2.into(),
            |a| a["effects"][0]["steps"][1]["who"]["selectionId"] = "line:0:0".into(),
            |a| a["restrictions"] = serde_json::json!(["sorcery-speed"]),
        ];
        let edict_mutations: Vec<Mutation> = vec![
            |a| a["effects"][0]["steps"][0]["chosenBy"]["player"] = "you".into(),
            |a| {
                a["effects"][0]["steps"][0]["chosenBy"]["selectionId"] = "different-opponent".into()
            },
            |a| a["effects"][0]["steps"][0]["who"]["player"] = "target-player".into(),
            |a| a["effects"][0]["steps"][0]["what"]["count"]["value"] = 2.into(),
            |a| {
                a["effects"][0]["steps"][0]["what"]["filter"]["types"] =
                    serde_json::json!(["artifact"])
            },
            |a| a["effects"][0]["steps"][0]["optional"] = true.into(),
        ];
        for (raw, mutations) in [
            (fixtures::ACTION_RECORDS[9].1, activation_mutations),
            (fixtures::RECORDS[5].1, edict_mutations),
        ] {
            for mutate in mutations {
                let bad = changed_source(raw, |r| {
                    let ability = &mut r["faces"][0]["result"]["abilities"][0];
                    mutate(ability);
                    let changed = ability.clone();
                    r["faces"][0]["result"]["lines"][0]["ability"] = changed;
                });
                let result = compile(fixtures::MANIFEST, &bad);
                assert!(
                    matches!(result, CompileResult::Unsupported { .. }),
                    "{result:?}"
                );
            }
        }
        let bad = changed_source(fixtures::ACTION_RECORDS[9].1, |r| {
            r["source"]["oracle_text"] =
                "Sacrifice this creature: Draw a card, then discard a card. You gain 1 life.".into()
        });
        assert!(matches!(
            compile(fixtures::MANIFEST, &bad),
            CompileResult::Invalid { .. }
        ));
        let renamed = changed_source(fixtures::ACTION_RECORDS[9].1, |r| {
            r["source"]["name"] = "Renamed source-sacrifice looter".into();
            r["faces"][0]["result"]["name"] = "Renamed source-sacrifice looter".into();
        });
        assert!(matches!(
            compile(fixtures::MANIFEST, &renamed),
            CompileResult::Lowered { .. }
        ));
    }

    #[test]
    fn action_ir_shapes_and_target_domains_are_strict() {
        let catalog = embedded_catalog("action").unwrap();
        let researcher = catalog
            .definitions
            .iter()
            .find(|d| !d.activated.is_empty())
            .unwrap();
        for mutate in [
            (|d: &mut Definition| d.activated[0].functioning_zone = FunctioningZone::Stack)
                as fn(&mut Definition),
            |d| d.activated[0].program.functioning_zone = FunctioningZone::Battlefield,
            |d| d.activated[0].program.timing = SpellTiming::MainPhase,
            |d| {
                d.activated[0].program.instructions = vec![Instruction::DrawCards {
                    recipient: None,
                    count: Quantity::from(2),
                    source_block: 0,
                }]
            },
            |d| {
                d.activated[0].program.targets = vec![TargetSpec {
                    slot: 0,
                    selection_id: "unexpected".into(),
                    chooser: Chooser::Controller,
                    stage: EvaluationStage::Announcement,
                    filter: None,
                    domain: TargetDomain::BattlefieldCreature,
                }]
            },
            |d| {
                d.activated[0].program.instructions = vec![Instruction::ReturnToOwnerHand {
                    target_slot: 0,
                    source_block: 0,
                }]
            },
        ] {
            let mut bad = researcher.clone();
            mutate(&mut bad);
            bad.capabilities = bad.requirements();
            assert!(bad.validate().is_err());
        }
        let mut mixed = catalog
            .definitions
            .iter()
            .find(|d| d.name == "Cruel Edict")
            .unwrap()
            .clone();
        let p = mixed.spell.as_mut().unwrap();
        p.targets.push(TargetSpec {
            slot: 1,
            selection_id: "creature-slot".into(),
            chooser: Chooser::Controller,
            stage: EvaluationStage::Announcement,
            filter: None,
            domain: TargetDomain::BattlefieldCreature,
        });
        p.instructions.push(Instruction::ReturnToOwnerHand {
            target_slot: 1,
            source_block: 0,
        });
        mixed.capabilities = mixed.requirements();
        assert!(mixed.validate().is_err());
        let raw = canonical(&catalog).unwrap();
        for bad in [
            raw.replace("\"cost\":\"sacrifice_source\"", "\"cost\":\"tap_source\""),
            raw.replace("\"count\":\"1\"", "\"count\":\"01\""),
            raw.replace(
                "\"op\":\"draw_cards\"",
                "\"optional\":true,\"op\":\"draw_cards\"",
            ),
        ] {
            assert!(Catalog::read(&bad).is_err());
        }
    }

    #[test]
    fn complete_library_lowering_rejects_other_positions_owners_and_abilities() {
        type Mutation = fn(&mut serde_json::Value);
        let mutations: Vec<Mutation> = vec![
            |r| r["to"]["position"] = "bottom".into(),
            |r| r["to"]["position"] = "random".into(),
            |r| r["to"]["owner"] = "controller".into(),
            |r| r["to"]["zone"] = "hand".into(),
            |r| {
                r["to"].as_object_mut().unwrap().remove("position");
            },
            |r| r["to"]["shuffle"] = true.into(),
            |r| r["effect"] = "draw".into(),
            |r| r["optional"] = true.into(),
            |r| r["what"]["filter"]["types"] = serde_json::json!(["player"]),
            |r| r["what"]["count"]["value"] = 2.into(),
            |r| r["what"]["selectionId"] = "".into(),
        ];
        for mutate in mutations {
            let changed = changed_record(7, |record| {
                let ability = &mut record["faces"][0]["result"]["abilities"][0];
                mutate(&mut ability["effects"][0]["steps"][0]);
                let ability = ability.clone();
                record["faces"][0]["result"]["lines"][0]["ability"] = ability;
            });
            let result = compile(fixtures::MANIFEST, &changed);
            assert!(
                matches!(result, CompileResult::Unsupported { .. }),
                "{result:?}"
            );
        }
        let extra = changed_record(7, |r| {
            r["source"]["oracle_text"] =
                "Put target creature on top of its owner's library. Draw a card.".into();
        });
        assert!(matches!(
            compile(fixtures::MANIFEST, &extra),
            CompileResult::Invalid { .. }
        ));
    }

    #[test]
    fn library_vocabulary_requires_valid_program_shape() {
        let catalog = embedded_catalog("library").unwrap();
        let raw = canonical(&catalog).unwrap();
        for bad in [
            raw.replace(
                "\"schema_version\":1",
                &format!(
                    "\"schema_version\":{}",
                    mtg_contracts::CURRENT_IR_VERSION + 1
                ),
            ),
            raw.replace(
                "\"op\":\"put_on_top_owner_library\"",
                "\"op\":\"put_on_bottom_owner_library\"",
            ),
            raw.replace(
                "\"op\":\"put_on_top_owner_library\"",
                "\"unknown\":true,\"op\":\"put_on_top_owner_library\"",
            ),
        ] {
            assert!(Catalog::read(&bad).is_err());
        }
        let ebb = catalog
            .definitions
            .iter()
            .find(|d| d.characteristics.kind == CardKind::Sorcery)
            .unwrap();
        for mutate in [
            (|d: &mut Definition| d.spell.as_mut().unwrap().timing = SpellTiming::Instant)
                as fn(&mut Definition),
            |d| d.spell.as_mut().unwrap().resolution = SpellResolution::Permanent,
            |d| {
                d.spell.as_mut().unwrap().instructions = vec![Instruction::PutOnTopOwnerLibrary {
                    target_slot: 1,
                    source_block: 0,
                }]
            },
            |d| d.spell.as_mut().unwrap().targets.clear(),
        ] {
            let mut bad = ebb.clone();
            mutate(&mut bad);
            assert!(bad.validate().is_err());
        }
    }

    #[test]
    fn creature_lowering_rejects_unrepresented_or_unsupported_complete_behavior() {
        type Mutation = fn(&mut serde_json::Value);
        let mutations: Vec<Mutation> = vec![
            |r| {
                r["source"]["oracle_text"] = "(Flying)".into();
            },
            |r| {
                r["source"]["mana_cost"] = "{X}{G}".into();
            },
            |r| {
                r["source"]["toughness"] = "0".into();
            },
            |r| {
                r["source"]["power"] = "*".into();
            },
            |r| {
                r["source"]["oracle_text"] = "Return target creature to its owner's hand.".into();
                let instant: serde_json::Value =
                    serde_json::from_str(fixtures::RECORDS[3].1).unwrap();
                r["faces"][0]["result"]["lines"] = instant["faces"][0]["result"]["lines"].clone();
                r["faces"][0]["result"]["abilities"] =
                    instant["faces"][0]["result"]["abilities"].clone();
            },
        ];
        for mutation in mutations {
            let result = compile(fixtures::MANIFEST, &changed_record(1, mutation));
            assert!(
                !matches!(result, CompileResult::Lowered { .. }),
                "{result:?}"
            );
        }
        let empty_instant = changed_record(3, |r| {
            r["source"]["oracle_text"] = "(Reminder only)".into();
            r["faces"][0]["result"]["lines"] = serde_json::json!([]);
            r["faces"][0]["result"]["abilities"] = serde_json::json!([]);
        });
        assert!(matches!(
            compile(fixtures::MANIFEST, &empty_instant),
            CompileResult::Unsupported { .. }
        ));
    }

    #[test]
    fn ordinary_combat_pool_reuses_complete_vanilla_ir_and_rejects_extra_mechanics() {
        let catalog = embedded_catalog("base").unwrap();
        let bears = catalog
            .definitions
            .iter()
            .find(|definition| definition.oracle_id == "14c8f55d-d177-4c25-a931-ebeb9e6062a0")
            .unwrap();
        assert_eq!(bears.characteristics.power, Some(Quantity::from(2)));
        assert_eq!(bears.characteristics.toughness, Some(Quantity::from(2)));
        assert_eq!(bears.characteristics.mana_cost.generic, Quantity::from(1));
        assert_eq!(bears.characteristics.mana_cost.colored, vec![Color::G]);
        assert!(!bears.characteristics.legendary);
        assert_eq!(bears.provenance.abilities_json, "[]");
        assert_eq!(catalog.schema_version, CURRENT_IR_VERSION);

        // These are rehashed synthetic negative variants of the retained exact
        // source, not additional cards. Account for the complete source block
        // before verifying that unknown keyword/trigger/static semantics reject.
        // Combat rules remain core behavior; they must not make extra source
        // abilities disappear during complete-card lowering.
        for (text, ability) in [
            (
                "Protection from red",
                serde_json::json!({"kind":"keywords","keywords":[{"keyword":"protection"}]}),
            ),
            (
                "Indestructible",
                serde_json::json!({"kind":"keywords","keywords":[{"keyword":"indestructible"}]}),
            ),
            (
                "Indestructible",
                serde_json::json!({"kind":"keywords","keywords":[{"keyword":"indestructible"}]}),
            ),
            (
                "When this creature dies, draw a card.",
                serde_json::json!({"kind":"triggered","effects":[]}),
            ),
            (
                "Other creatures you control get +1/+1.",
                serde_json::json!({"kind":"static","effects":[]}),
            ),
        ] {
            let changed = changed_record(1, |record| {
                record["source"]["oracle_text"] = text.into();
                record["faces"][0]["result"]["lines"] = serde_json::json!([
                    {"text":text,"ok":true,"ability":ability}
                ]);
                record["faces"][0]["result"]["abilities"] = serde_json::json!([ability]);
            });
            assert!(matches!(
                compile(fixtures::MANIFEST, &changed),
                CompileResult::Unsupported { .. }
            ));
        }
        assert!(matches!(
            compile(fixtures::MANIFEST, fixtures::ACTION_RECORDS[11].1),
            CompileResult::Lowered { .. }
        ));
    }

    #[test]
    fn ir_reader_rejects_mismatched_spell_timing_resolution_and_shape() {
        type Mutation = fn(&mut Definition);
        let catalog = embedded_catalog("base").unwrap();
        let creature = catalog
            .definitions
            .iter()
            .find(|d| d.characteristics.kind == CardKind::Creature)
            .unwrap();
        let instant = catalog
            .definitions
            .iter()
            .find(|d| d.characteristics.kind == CardKind::Instant)
            .unwrap();
        let mutations: Vec<Mutation> = vec![
            |d| {
                d.spell = None;
            },
            |d| {
                d.spell.as_mut().unwrap().timing = SpellTiming::Instant;
            },
            |d| {
                d.spell.as_mut().unwrap().resolution = SpellResolution::Instructions;
            },
            |d| {
                d.spell.as_mut().unwrap().functioning_zone = FunctioningZone::Battlefield;
            },
            |d| {
                d.spell
                    .as_mut()
                    .unwrap()
                    .instructions
                    .push(Instruction::ReturnToOwnerHand {
                        target_slot: 0,
                        source_block: 0,
                    });
            },
            |d| {
                d.spell.as_mut().unwrap().targets.push(TargetSpec {
                    slot: 0,
                    selection_id: "forged".into(),
                    chooser: Chooser::Controller,
                    stage: EvaluationStage::Announcement,
                    filter: None,
                    domain: TargetDomain::BattlefieldCreature,
                });
            },
        ];
        for mutation in mutations {
            let mut bad = creature.clone();
            mutation(&mut bad);
            let mut malformed = catalog.clone();
            let target = malformed
                .definitions
                .iter_mut()
                .find(|d| d.oracle_id == bad.oracle_id)
                .unwrap();
            *target = bad;
            assert!(Catalog::read(&canonical(&malformed).unwrap()).is_err());
        }
        for mutation in [
            (|d: &mut Definition| {
                d.spell.as_mut().unwrap().timing = SpellTiming::MainPhase;
            }) as Mutation,
            |d| {
                d.spell.as_mut().unwrap().resolution = SpellResolution::Permanent;
            },
            |d| {
                d.spell.as_mut().unwrap().instructions.clear();
            },
        ] {
            let mut bad = instant.clone();
            mutation(&mut bad);
            assert!(bad.validate().is_err());
        }
        let raw = canonical(&catalog).unwrap();
        for malformed in [
            raw.replace("\"schema_version\":1", "\"schema_version\":2"),
            raw.replace("\"timing\":\"main_phase\"", "\"timing\":\"anytime\""),
            raw.replace(
                "\"resolution\":\"permanent\"",
                "\"resolution\":\"instructions\"",
            ),
            raw.replace(",\"timing\":\"main_phase\"", ""),
            raw.replace("\"resolution\":\"permanent\",", ""),
        ] {
            assert!(malformed != raw, "mutation must change the fixture");
            assert!(Catalog::read(&malformed).is_err());
        }
    }
}

#[cfg(test)]
mod d2b_tests;

#[cfg(test)]
mod d2c_tests;

#[cfg(test)]
mod d2cb_tests;

#[cfg(test)]
mod d2cc_tests;

#[cfg(test)]
mod d2cd_tests;

#[cfg(test)]
mod d2ce_tests;
