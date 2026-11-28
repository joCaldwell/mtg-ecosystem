use super::*;
use mtg_contracts::kir::{
    CharacteristicSet, CombatProhibitionScope, CombatRequirement, DamageKindFilter, Pred,
};
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(transparent)]
pub struct EffectHandle(pub String);
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(transparent)]
pub struct DamageHandle(pub String);
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum DamageEffectKind {
    PreventDamage,
    ModifyDamage,
    ProtectionFromEachColor,
    ProtectionFromEverything,
    ProtectionFromColor,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct VisibleDamageEffectCandidate {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    pub effect: EffectHandle,
    pub damage: DamageHandle,
    pub kind: DamageEffectKind,
    pub effect_source: VisibleTarget,
    pub effect_oracle_id: String,
    pub effect_controller: PlayerId,
    pub damage_source: VisibleTarget,
    pub damage_oracle_id: String,
    pub amount_before: Quantity,
    pub amount_after: Quantity,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(rename = "VisibleEffectSource")]
#[serde(bound(deserialize = "O: Deserialize<'de>"))]
pub struct VisibleEffectSource<O = ObjectHandle> {
    pub source: VisibleTarget<O>,
    pub oracle_id: String,
    pub controller: PlayerId,
}
impl<O> VisibleEffectSource<O> {
    fn convert<T: for<'a> From<&'a O>>(self) -> VisibleEffectSource<T> {
        VisibleEffectSource {
            source: self.source.convert(),
            oracle_id: self.oracle_id,
            controller: self.controller,
        }
    }
}
// Declare each effect once. Stored rows keep the issued integer while the
// public projection exposes its viewer-scoped text.
macro_rules! effect_field {
    ($value:ident) => {
        $value
    };
    ($value:ident, nested) => {
        $value.convert()
    };
    ($value:ident, optional) => {
        $value.map(|row| row.convert())
    };
    ($value:ident, targets) => {
        $value.into_iter().map(|row| row.convert()).collect()
    };
    ($value:ident, handles) => {
        $value.iter().map(P::from).collect()
    };
    ($value:ident, combat_scope) => {
        match $value {
            CombatProhibitionScope::Target { target } => CombatProhibitionScope::Target {
                target: target.convert(),
            },
            CombatProhibitionScope::Creatures {} => CombatProhibitionScope::Creatures {},
            CombatProhibitionScope::CreaturesWithoutFlying {} => {
                CombatProhibitionScope::CreaturesWithoutFlying {}
            }
        }
    };
}
macro_rules! visible_effects {
    ($( $variant:ident $(($handle:ident))? {
        $( $(#[$attr:meta])* $field:ident: $ty:ty $(=> $conversion:ident)?, )*
    }, )*) => {
        #[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
        #[serde(tag = "effect", rename_all = "snake_case", deny_unknown_fields)]
        #[schemars(rename = "VisibleEffect")]
        pub enum VisibleEffect<H = EffectHandle, O = ObjectHandle> {
            $( $variant { $( $handle: H, )? $( $(#[$attr])* $field: $ty, )* }, )*
        }
        impl<H,O> VisibleEffect<H,O> {
            pub fn map_handle<T, P: for<'a> From<&'a O>>(self, convert: impl FnOnce(H) -> T) -> VisibleEffect<T,P> {
                match self {
                    $( Self::$variant { $( $handle, )? $( $field, )* } =>
                        VisibleEffect::$variant { $( $handle: convert($handle), )? $( $field: effect_field!($field $(, $conversion)?), )* }, )*
                }
            }
        }
    }
}

visible_effects! {
    RegenerationShield(handle) {
        origin: VisibleEffectSource<O> => nested,
        target: VisibleTarget<O> => nested,
        expires_in_cleanup_of_turn: u64,
    },
    ProtectControllerUntilNextTurn(handle) {
        origin: VisibleEffectSource<O> => nested,
    },
    RemoveIndestructibleSet(handle) {
        origin: VisibleEffectSource<O> => nested,
        targets: Vec<VisibleTarget<O>> => targets,
        expires_in_cleanup_of_turn: u64,
    },
    GrantKeywordsSet(handle) {
        origin: VisibleEffectSource<O> => nested,
        targets: Vec<VisibleTarget<O>> => targets,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        modifier: Option<CreatureStatModifier>,
        keywords: Vec<KeywordAbility>,
        expires_in_cleanup_of_turn: u64,
    },
    PumpSet(handle) {
        origin: VisibleEffectSource<O> => nested,
        targets: Vec<VisibleTarget<O>> => targets,
        power: Quantity,
        toughness: Quantity,
        expires_in_cleanup_of_turn: u64,
    },
    ExileDamaged(handle) {
        origin: VisibleEffectSource<O> => nested,
        targets: Vec<VisibleTarget<O>> => targets,
        expires_in_cleanup_of_turn: u64,
    },
    GrantKeywords(handle) {
        origin: VisibleEffectSource<O> => nested,
        target: VisibleTarget<O> => nested,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        modifier: Option<CreatureStatModifier>,
        keywords: Vec<KeywordAbility>,
        expires_in_cleanup_of_turn: u64,
    },
    CombatProhibition(handle) {
        origin: VisibleEffectSource<O> => nested,
        affects: CombatProhibitionScope<VisibleTarget<O>> => combat_scope,
        restriction: CombatRequirement,
        expires_in_cleanup_of_turn: u64,
    },
    GainControl(handle) {
        origin: VisibleEffectSource<O> => nested,
        targets: Vec<VisibleTarget<O>> => targets,
        controller: PlayerId,
        while_source_remains: bool,
        expires_in_cleanup_of_turn: Option<u64>,
    },
    SetCharacteristics(handle) {
        origin: VisibleEffectSource<O> => nested,
        targets: Vec<VisibleTarget<O>> => targets,
        values: CharacteristicSet,
        expires_in_cleanup_of_turn: Option<u64>,
        expires_at_controller_next_turn: bool,
        while_source_remains: bool,
    },
    ModifyPowerToughness(handle) {
        origin: VisibleEffectSource<O> => nested,
        target: VisibleTarget<O> => nested,
        power: Quantity,
        toughness: Quantity,
        expires_in_cleanup_of_turn: u64,
    },
    // CR 615.1, 615.7: a resolved prevention effect; `remaining` is the
    // shield left of "the next N damage".
    PreventDamage(handle) {
        origin: VisibleEffectSource<O> => nested,
        kind: DamageKindFilter,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        to_player: Option<PlayerId>,
        #[serde(skip_serializing_if = "Option::is_none")]
        to_object: Option<VisibleTarget<O>> => optional,
        #[serde(skip_serializing_if = "Option::is_none")]
        by_object: Option<VisibleTarget<O>> => optional,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        by_filter: Option<Pred>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        remaining: Option<Quantity>,
        expires_in_cleanup_of_turn: u64,
    },
    AdditionalLandPlays(handle) {
        origin: VisibleEffectSource<O> => nested,
        count: Quantity,
        expires_in_cleanup_of_turn: u64,
    },
}
