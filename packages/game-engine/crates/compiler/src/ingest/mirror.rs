//! The typed lowering mirror of archived AST blocks: complete shapes with
//! unknown fields rejected. Lowerers consume these, never raw strings.
use super::*;

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub(crate) enum Ability {
    #[serde(skip)]
    ComposedSpell(SpellProgram),
    #[serde(skip)]
    BoundTrigger(TriggeredAbility),
    #[serde(skip)]
    BoundActivation(ActivatedAbility),
    Chapter {
        chapters: Vec<u32>,
        effects: Vec<serde_json::Value>,
    },
    // The complete spell, cast trigger, and source graph are verified together.
    #[serde(skip)]
    Demonstrate,
    #[serde(skip)]
    RevealAndCast,
    // Constructed together after closing the printed creature and replacement graph.
    #[serde(skip)]
    PrintedProtectionFromEverything,
    #[serde(skip)]
    SelfGraveyardRevealShuffle,
    // The complete source and its exact graph are verified before emission.
    #[serde(skip)]
    ControlledCreatureSpellCopy,
    #[serde(skip)]
    LinkedHideaway,
    // Constructed only after validating the complete raw sentence and payer graph.
    #[serde(skip)]
    CounterUnlessGeneric { generic: Quantity },
    // Only the whole-source adapter can introduce this bound trigger.
    #[serde(skip)]
    CastPayerDraw { generic: Quantity },
    #[serde(skip)]
    DividedPlusOneCountersModal,
    #[serde(skip)]
    CommanderModalKeyword(Box<SpellProgram>),
    #[serde(skip)]
    PlayerGraveyardShuffle,
    #[serde(skip)]
    FixedTargetModes,
    #[serde(skip)]
    CreaturePlaneswalkerReturn,
    #[serde(skip)]
    HandCastCascade,
    #[serde(skip)]
    ControllerWindowBlock,
    #[serde(skip)]
    BoundKeywords(Vec<KeywordAbility>),
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
        restriction: Option<SourceActivationRestriction>,
        #[serde(rename = "abilityWord")]
        ability_word: Option<String>,
        costs: Vec<SourceCost>,
        effects: Vec<Sentence>,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct SourceKeyword {
    pub(crate) keyword: String,
    pub(crate) cost: Option<Vec<SourceCost>>,
    pub(crate) filter: Option<serde_json::Value>,
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
            "bestow" | "equip" | "enchant" | "cascade" | "kicker" | "affinity" | "cycling"
        ) && serde_json::from_value::<KeywordAbility>(serde_json::Value::String(keyword.clone()))
            .is_err()
        {
            return Err(D::Error::custom(format!("keyword:{keyword}")));
        }
        let allowed = match keyword.as_str() {
            "bestow" | "equip" | "kicker" | "cycling" => ["keyword", "cost"].as_slice(),
            "enchant" | "affinity" => ["keyword", "filter"].as_slice(),
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
#[derive(Deserialize)]
#[serde(tag = "gained", rename_all = "kebab-case", deny_unknown_fields)]
pub(crate) enum GainedAbility {
    Keyword { keyword: GainedKeyword },
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct GainedKeyword {
    pub(crate) keyword: KeywordAbility,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Sentence {
    #[serde(default, deserialize_with = "present_ast_field")]
    pub(crate) condition: Option<KickedCondition>,
    #[serde(
        default,
        rename = "conditionKind",
        deserialize_with = "present_ast_field"
    )]
    pub(crate) condition_kind: Option<String>,
    #[serde(default)]
    pub(crate) bindings: Vec<serde_json::Value>,
    pub(crate) steps: Vec<Step>,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
#[serde(tag = "effect", rename_all = "kebab-case")]
pub(crate) enum Step {
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
    AddMana {
        who: PlayerSelector,
        mana: SourceMana,
        #[serde(
            default,
            rename = "restriction",
            deserialize_with = "present_json_operand"
        )]
        spend_restriction: Option<serde_json::Value>,
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
    CreateToken {
        #[serde(default, deserialize_with = "present_json_operand")]
        who: Option<serde_json::Value>,
        count: Count,
        token: SourceToken,
    },
    PutCounters {
        on: SourceCounterRecipient,
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
    Pump {
        what: serde_json::Value,
        power: SignedAmount,
        toughness: SignedAmount,
        duration: SourceDuration,
    },
    Regenerate {
        what: serde_json::Value,
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
        #[serde(default, deserialize_with = "present_bool_field")]
        optional: Option<bool>,
    },
    GainLife {
        who: PlayerSelector,
        amount: serde_json::Value,
        #[serde(default, rename = "forEach", deserialize_with = "present_json_operand")]
        for_each: Option<serde_json::Value>,
    },
    LoseLife {
        who: PlayerSelector,
        amount: serde_json::Value,
    },
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
    Discard(DiscardStep),
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
    pub(crate) power: Count,
    pub(crate) toughness: Count,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(untagged)]
pub(crate) enum SourceCounterRecipient {
    Target(Target),
    Source(ThisObject),
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

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(tag = "ref", rename_all = "kebab-case", deny_unknown_fields)]
pub(crate) enum DamageSource {
    It {},
    #[serde(rename = "self")]
    SelfReference {},
    This {
        noun: String,
    },
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(tag = "to", rename_all = "kebab-case", deny_unknown_fields)]
pub(crate) enum DamageDestination {
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
pub(crate) struct ThisObject {
    #[serde(rename = "ref")]
    pub(crate) reference: String,
    pub(crate) noun: String,
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
pub(crate) struct FilterObject<A = Count> {
    #[serde(rename = "ref")]
    pub(crate) reference: String,
    pub(crate) filter: ObjectFilter,
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
pub(crate) struct Target {
    #[serde(rename = "ref")]
    pub(crate) reference: String,
    pub(crate) filter: Filter,
    pub(crate) count: Count,
    pub(crate) selection_id: String,
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
        // ObjectFilter.allOf is optional in the parser AST; present null is not omission.
        #[serde(default, rename = "allOf", deserialize_with = "present_ast_field")]
        all_of: Option<Vec<serde_json::Value>>,
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

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Filter {
    pub(crate) types: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
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

impl DrawAmount {
    pub(crate) fn fixed_one(&self) -> bool {
        matches!(self, Self::Fixed { value: 1 })
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct SourceDuration {
    pub(crate) duration: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct SignedAmount {
    pub(crate) sign: i8,
    pub(crate) amount: Count,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct SourcePrevention {
    pub(crate) event_scope: SourceEventScope,
    pub(crate) shield: SourceShield,
    pub(crate) kind: String,
    pub(crate) to: Option<Vec<AnyTargetReference>>,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct SourceEventScope {
    pub(crate) sources: String,
    pub(crate) recipients: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(tag = "shield", rename_all = "kebab-case", deny_unknown_fields)]
pub(crate) enum SourceShield {
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
pub(crate) enum SourcePrevent {
    All {},
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(tag = "ref", rename_all = "kebab-case", deny_unknown_fields)]
pub(crate) enum AnyTargetReference {
    AnyTarget {
        #[serde(rename = "selectionId")]
        selection_id: String,
    },
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(tag = "static", rename_all = "kebab-case", deny_unknown_fields)]
pub(crate) enum SourceStaticEffect {
    AdditionalLandPlays {
        #[serde(rename = "allowance")]
        _allowance: serde_json::Value,
    },
    PlayPermission {
        #[serde(rename = "permission")]
        _permission: serde_json::Value,
    },
    AsCascade {
        controller: serde_json::Value,
        choice: serde_json::Value,
    },
    OpeningHandAction {
        what: serde_json::Value,
        who: serde_json::Value,
        action: String,
    },
    CastSpellAbilities {
        scope: serde_json::Value,
        abilities: Vec<serde_json::Value>,
    },
    PlayerRule {
        rule: serde_json::Value,
    },
    Control {
        who: SourceControlPlayer,
        what: SourceControlObject,
    },
    Prohibit {
        prohibition: serde_json::Value,
    },
    EntryChoice {
        what: serde_json::Value,
        who: serde_json::Value,
        costs: Vec<conditional_lands::SourceEntryCost>,
        declined: serde_json::Value,
    },
    EntryCopy {
        what: serde_json::Value,
        model: serde_json::Value,
        optional: serde_json::Value,
    },
    EntersTapped {
        what: serde_json::Value,
        #[serde(
            default,
            rename = "conditionKind",
            deserialize_with = "present_json_operand"
        )]
        condition_kind: Option<serde_json::Value>,
        #[serde(default, deserialize_with = "present_json_operand")]
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
    CombatRule {
        what: serde_json::Value,
        rule: serde_json::Value,
    },
    Modify {
        what: serde_json::Value,
        modifiers: Vec<serde_json::Value>,
    },
    Conditional {
        #[serde(rename = "condition")]
        _condition: serde_json::Value,
        #[serde(rename = "effect")]
        _effect: serde_json::Value,
    },
    Replacement {
        replacement: SourceReplacement,
    },
}

// Unlike diagnostic binding.references, these are executable typed operands.
// No selector, qualifier, sentence binding or duration may be discarded.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct SourceControlPlayer {
    pub(crate) player: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct SourceControlObject {
    #[serde(rename = "ref")]
    pub(crate) reference: String,
    pub(crate) noun: AuraControlTarget,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub(crate) enum AuraControlTarget {
    Creature,
    Artifact,
    Land,
    Permanent,
}

impl AuraControlTarget {
    pub(crate) fn noun(self) -> &'static str {
        match self {
            Self::Creature => "creature",
            Self::Artifact => "artifact",
            Self::Land => "land",
            Self::Permanent => "permanent",
        }
    }
    pub(crate) fn kind(self) -> Option<CardKind> {
        match self {
            Self::Creature => Some(CardKind::Creature),
            Self::Artifact => Some(CardKind::Artifact),
            Self::Land => Some(CardKind::Land),
            Self::Permanent => None,
        }
    }
    pub(crate) fn from_filter(filter: &Option<serde_json::Value>) -> Option<Self> {
        [Self::Creature, Self::Artifact, Self::Land, Self::Permanent]
            .into_iter()
            .find(|t| {
                let exact = if *t == Self::Permanent {
                    serde_json::json!({"cls":"permanent"})
                } else {
                    serde_json::json!({"types":[t.noun()]})
                };
                *filter == Some(exact)
            })
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct SourceReplacement {
    pub(crate) event: SourceDamageEvent,
    pub(crate) occurrence: String,
    pub(crate) replace: SourceReplacementAction,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(tag = "event", rename_all = "kebab-case", deny_unknown_fields)]
pub(crate) enum SourceDamageEvent {
    PutCounters {
        on: Vec<CounterPlacementSourceRecipient>,
        counter: SourceCounter,
        cause: String,
        quantity: SourceCounterThreshold,
    },
    Damage {
        kind: String,
        source: ReplacementDamageSource,
        to: Vec<ReplacementRecipient>,
    },
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct CounterPlacementSourceRecipient {
    #[serde(rename = "ref")]
    pub(crate) reference: String,
    pub(crate) filter: CounterPlacementSourceFilter,
    pub(crate) count: Count,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct CounterPlacementSourceFilter {
    pub(crate) types: Vec<String>,
    pub(crate) control: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct SourceCounterThreshold {
    pub(crate) op: String,
    pub(crate) value: Count,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ReplacementDamageSource {
    #[serde(rename = "ref")]
    pub(crate) reference: String,
    pub(crate) filter: serde_json::Value,
    pub(crate) count: Count,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(untagged)]
pub(crate) enum ReplacementRecipient {
    Permanent(FilterObject),
    Player(PlayerSelector),
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(tag = "replacement", rename_all = "kebab-case", deny_unknown_fields)]
pub(crate) enum SourceReplacementAction {
    ModifyQuantity { change: SourceQuantityChange },
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct SourceQuantityChange {
    pub(crate) operation: String,
    pub(crate) factor: Option<u32>,
    pub(crate) amount: Option<Count>,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(tag = "mana", rename_all = "kebab-case", deny_unknown_fields)]
pub(crate) enum SourceMana {
    Choice {
        options: Vec<SourceMana>,
    },
    Fixed {
        symbols: Vec<Color>,
    },
    AnyColor {
        amount: Count,
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
