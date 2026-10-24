//! Offline archive-3 adapter and the first complete-card lowering subset.
use mtg_contracts::*;
use serde::Deserialize;
use serde_json::value::RawValue;
use std::collections::{BTreeMap, BTreeSet};

pub mod fixtures {
    pub const STAPLES_RECORDS: [(&str, &str); 9] = [
        (
            "day-of-judgment",
            include_str!("../../../tests/fixtures/day-of-judgment.json"),
        ),
        (
            "glorious-anthem",
            include_str!("../../../tests/fixtures/glorious-anthem.json"),
        ),
        (
            "murder",
            include_str!("../../../tests/fixtures/murder.json"),
        ),
        (
            "raise-dead",
            include_str!("../../../tests/fixtures/raise-dead.json"),
        ),
        (
            "gravedigger",
            include_str!("../../../tests/fixtures/gravedigger.json"),
        ),
        (
            "nights-whisper",
            include_str!("../../../tests/fixtures/nights-whisper.json"),
        ),
        (
            "sign-in-blood",
            include_str!("../../../tests/fixtures/sign-in-blood.json"),
        ),
        (
            "rampant-growth",
            include_str!("../../../tests/fixtures/rampant-growth.json"),
        ),
        (
            "explosive-vegetation",
            include_str!("../../../tests/fixtures/explosive-vegetation.json"),
        ),
    ];
    pub const MANIFEST: &str = include_str!("../../../tests/fixtures/archive-manifest.json");
    pub const RECORDS: [(&str, &str); 6] = [
        (
            "forest",
            include_str!("../../../tests/fixtures/forest.json"),
        ),
        (
            "grizzly-bears",
            include_str!("../../../tests/fixtures/grizzly-bears.json"),
        ),
        (
            "island",
            include_str!("../../../tests/fixtures/island.json"),
        ),
        (
            "unsummon",
            include_str!("../../../tests/fixtures/unsummon.json"),
        ),
        (
            "isamaru",
            include_str!("../../../tests/fixtures/isamaru.json"),
        ),
        (
            "cruel-edict",
            include_str!("../../../tests/fixtures/cruel-edict.json"),
        ),
    ];
    pub const COMMANDER_RECORDS: [(&str, &str); 7] = [
        RECORDS[0],
        RECORDS[1],
        RECORDS[2],
        RECORDS[3],
        RECORDS[4],
        RECORDS[5],
        (
            "plains",
            include_str!("../../../tests/fixtures/plains.json"),
        ),
    ];
    pub const LIBRARY_RECORDS: [(&str, &str); 8] = [
        COMMANDER_RECORDS[0],
        COMMANDER_RECORDS[1],
        COMMANDER_RECORDS[2],
        COMMANDER_RECORDS[3],
        COMMANDER_RECORDS[4],
        COMMANDER_RECORDS[5],
        COMMANDER_RECORDS[6],
        (
            "time-ebb",
            include_str!("../../../tests/fixtures/time-ebb.json"),
        ),
    ];
    pub const DAMAGE_RECORDS: [(&str, &str); 8] = [
        COMMANDER_RECORDS[0],
        COMMANDER_RECORDS[1],
        COMMANDER_RECORDS[2],
        COMMANDER_RECORDS[3],
        COMMANDER_RECORDS[4],
        COMMANDER_RECORDS[5],
        COMMANDER_RECORDS[6],
        (
            "yargle-and-multani",
            include_str!("../../../tests/fixtures/yargle-and-multani.json"),
        ),
    ];
    pub const ACTION_RECORDS: [(&str, &str); 12] = [
        COMMANDER_RECORDS[0],
        COMMANDER_RECORDS[1],
        COMMANDER_RECORDS[2],
        COMMANDER_RECORDS[3],
        COMMANDER_RECORDS[4],
        COMMANDER_RECORDS[5],
        COMMANDER_RECORDS[6],
        DAMAGE_RECORDS[7],
        LIBRARY_RECORDS[7],
        (
            "hapless-researcher",
            include_str!("../../../tests/fixtures/hapless-researcher.json"),
        ),
        ("swamp", include_str!("../../../tests/fixtures/swamp.json")),
        (
            "blood-artist",
            include_str!("../../../tests/fixtures/blood-artist.json"),
        ),
    ];
    pub const STACK_RECORDS: [(&str, &str); 16] = [
        ACTION_RECORDS[0],
        ACTION_RECORDS[1],
        ACTION_RECORDS[2],
        ACTION_RECORDS[3],
        ACTION_RECORDS[4],
        ACTION_RECORDS[5],
        ACTION_RECORDS[6],
        ACTION_RECORDS[7],
        ACTION_RECORDS[8],
        ACTION_RECORDS[9],
        ACTION_RECORDS[10],
        ACTION_RECORDS[11],
        (
            "counterspell",
            include_str!("../../../tests/fixtures/counterspell.json"),
        ),
        (
            "stifle",
            include_str!("../../../tests/fixtures/stifle.json"),
        ),
        (
            "merfolk-looter",
            include_str!("../../../tests/fixtures/merfolk-looter.json"),
        ),
        (
            "azure-mage",
            include_str!("../../../tests/fixtures/azure-mage.json"),
        ),
    ];
    pub const DAMAGE_LIFE_RECORDS: [(&str, &str); 23] = [
        STACK_RECORDS[0],
        STACK_RECORDS[1],
        STACK_RECORDS[2],
        STACK_RECORDS[3],
        STACK_RECORDS[4],
        STACK_RECORDS[5],
        STACK_RECORDS[6],
        STACK_RECORDS[7],
        STACK_RECORDS[8],
        STACK_RECORDS[9],
        STACK_RECORDS[10],
        STACK_RECORDS[11],
        STACK_RECORDS[12],
        STACK_RECORDS[13],
        STACK_RECORDS[14],
        STACK_RECORDS[15],
        (
            "lightning-bolt",
            include_str!("../../../tests/fixtures/lightning-bolt.json"),
        ),
        (
            "prodigal-sorcerer",
            include_str!("../../../tests/fixtures/prodigal-sorcerer.json"),
        ),
        (
            "soul-feast",
            include_str!("../../../tests/fixtures/soul-feast.json"),
        ),
        (
            "healing-hands",
            include_str!("../../../tests/fixtures/healing-hands.json"),
        ),
        (
            "sacred-nectar",
            include_str!("../../../tests/fixtures/sacred-nectar.json"),
        ),
        (
            "mountain",
            include_str!("../../../tests/fixtures/mountain.json"),
        ),
        (
            "flame-rift",
            include_str!("../../../tests/fixtures/flame-rift.json"),
        ),
    ];
    pub const TRIGGER_RECORDS: [(&str, &str); 26] = [
        DAMAGE_LIFE_RECORDS[0],
        DAMAGE_LIFE_RECORDS[1],
        DAMAGE_LIFE_RECORDS[2],
        DAMAGE_LIFE_RECORDS[3],
        DAMAGE_LIFE_RECORDS[4],
        DAMAGE_LIFE_RECORDS[5],
        DAMAGE_LIFE_RECORDS[6],
        DAMAGE_LIFE_RECORDS[7],
        DAMAGE_LIFE_RECORDS[8],
        DAMAGE_LIFE_RECORDS[9],
        DAMAGE_LIFE_RECORDS[10],
        DAMAGE_LIFE_RECORDS[11],
        DAMAGE_LIFE_RECORDS[12],
        DAMAGE_LIFE_RECORDS[13],
        DAMAGE_LIFE_RECORDS[14],
        DAMAGE_LIFE_RECORDS[15],
        DAMAGE_LIFE_RECORDS[16],
        DAMAGE_LIFE_RECORDS[17],
        DAMAGE_LIFE_RECORDS[18],
        DAMAGE_LIFE_RECORDS[19],
        DAMAGE_LIFE_RECORDS[20],
        DAMAGE_LIFE_RECORDS[21],
        DAMAGE_LIFE_RECORDS[22],
        (
            "elvish-visionary",
            include_str!("../../../tests/fixtures/elvish-visionary.json"),
        ),
        (
            "essence-warden",
            include_str!("../../../tests/fixtures/essence-warden.json"),
        ),
        (
            "deathgreeter",
            include_str!("../../../tests/fixtures/deathgreeter.json"),
        ),
    ];
    pub const EFFECT_RECORDS: [(&str, &str); 35] = [
        TRIGGER_RECORDS[0],
        TRIGGER_RECORDS[1],
        TRIGGER_RECORDS[2],
        TRIGGER_RECORDS[3],
        TRIGGER_RECORDS[4],
        TRIGGER_RECORDS[5],
        TRIGGER_RECORDS[6],
        TRIGGER_RECORDS[7],
        TRIGGER_RECORDS[8],
        TRIGGER_RECORDS[9],
        TRIGGER_RECORDS[10],
        TRIGGER_RECORDS[11],
        TRIGGER_RECORDS[12],
        TRIGGER_RECORDS[13],
        TRIGGER_RECORDS[14],
        TRIGGER_RECORDS[15],
        TRIGGER_RECORDS[16],
        TRIGGER_RECORDS[17],
        TRIGGER_RECORDS[18],
        TRIGGER_RECORDS[19],
        TRIGGER_RECORDS[20],
        TRIGGER_RECORDS[21],
        TRIGGER_RECORDS[22],
        TRIGGER_RECORDS[23],
        TRIGGER_RECORDS[24],
        TRIGGER_RECORDS[25],
        (
            "giant-growth",
            include_str!("../../../tests/fixtures/giant-growth.json"),
        ),
        (
            "disfigure",
            include_str!("../../../tests/fixtures/disfigure.json"),
        ),
        ("fog", include_str!("../../../tests/fixtures/fog.json")),
        (
            "samite-healer",
            include_str!("../../../tests/fixtures/samite-healer.json"),
        ),
        (
            "furnace-of-rath",
            include_str!("../../../tests/fixtures/furnace-of-rath.json"),
        ),
        (
            "yoked-ox",
            include_str!("../../../tests/fixtures/yoked-ox.json"),
        ),
        (
            "righteousness",
            include_str!("../../../tests/fixtures/righteousness.json"),
        ),
        (
            "healing-salve",
            include_str!("../../../tests/fixtures/healing-salve.json"),
        ),
        (
            "gratuitous-violence",
            include_str!("../../../tests/fixtures/gratuitous-violence.json"),
        ),
    ];
    pub const CHOICE_RECORDS: [(&str, &str); 39] = [
        EFFECT_RECORDS[0],
        EFFECT_RECORDS[1],
        EFFECT_RECORDS[2],
        EFFECT_RECORDS[3],
        EFFECT_RECORDS[4],
        EFFECT_RECORDS[5],
        EFFECT_RECORDS[6],
        EFFECT_RECORDS[7],
        EFFECT_RECORDS[8],
        EFFECT_RECORDS[9],
        EFFECT_RECORDS[10],
        EFFECT_RECORDS[11],
        EFFECT_RECORDS[12],
        EFFECT_RECORDS[13],
        EFFECT_RECORDS[14],
        EFFECT_RECORDS[15],
        EFFECT_RECORDS[16],
        EFFECT_RECORDS[17],
        EFFECT_RECORDS[18],
        EFFECT_RECORDS[19],
        EFFECT_RECORDS[20],
        EFFECT_RECORDS[21],
        EFFECT_RECORDS[22],
        EFFECT_RECORDS[23],
        EFFECT_RECORDS[24],
        EFFECT_RECORDS[25],
        EFFECT_RECORDS[26],
        EFFECT_RECORDS[27],
        EFFECT_RECORDS[28],
        EFFECT_RECORDS[29],
        EFFECT_RECORDS[30],
        EFFECT_RECORDS[31],
        EFFECT_RECORDS[32],
        EFFECT_RECORDS[33],
        EFFECT_RECORDS[34],
        (
            "seeds-of-strength",
            include_str!("../../../tests/fixtures/seeds-of-strength.json"),
        ),
        (
            "slaughterhouse-bouncer",
            include_str!("../../../tests/fixtures/slaughterhouse-bouncer.json"),
        ),
        (
            "boros-charm",
            include_str!("../../../tests/fixtures/boros-charm.json"),
        ),
        (
            "lavaborn-muse",
            include_str!("../../../tests/fixtures/lavaborn-muse.json"),
        ),
    ];
    pub const MATCH_RECORDS: [(&str, &str); 41] = [
        CHOICE_RECORDS[0],
        CHOICE_RECORDS[1],
        CHOICE_RECORDS[2],
        CHOICE_RECORDS[3],
        CHOICE_RECORDS[4],
        CHOICE_RECORDS[5],
        CHOICE_RECORDS[6],
        CHOICE_RECORDS[7],
        CHOICE_RECORDS[8],
        CHOICE_RECORDS[9],
        CHOICE_RECORDS[10],
        CHOICE_RECORDS[11],
        CHOICE_RECORDS[12],
        CHOICE_RECORDS[13],
        CHOICE_RECORDS[14],
        CHOICE_RECORDS[15],
        CHOICE_RECORDS[16],
        CHOICE_RECORDS[17],
        CHOICE_RECORDS[18],
        CHOICE_RECORDS[19],
        CHOICE_RECORDS[20],
        CHOICE_RECORDS[21],
        CHOICE_RECORDS[22],
        CHOICE_RECORDS[23],
        CHOICE_RECORDS[24],
        CHOICE_RECORDS[25],
        CHOICE_RECORDS[26],
        CHOICE_RECORDS[27],
        CHOICE_RECORDS[28],
        CHOICE_RECORDS[29],
        CHOICE_RECORDS[30],
        CHOICE_RECORDS[31],
        CHOICE_RECORDS[32],
        CHOICE_RECORDS[33],
        CHOICE_RECORDS[34],
        CHOICE_RECORDS[35],
        CHOICE_RECORDS[36],
        CHOICE_RECORDS[37],
        CHOICE_RECORDS[38],
        (
            "wall-of-wood",
            include_str!("../../../tests/fixtures/wall-of-wood.json"),
        ),
        (
            "wall-of-vines",
            include_str!("../../../tests/fixtures/wall-of-vines.json"),
        ),
    ];
    pub const TOKEN_RECORDS: [(&str, &str); 13] = [
        (
            "hornet-queen",
            include_str!("../../../tests/fixtures/hornet-queen.json"),
        ),
        (
            "satyr-grovedancer",
            include_str!("../../../tests/fixtures/satyr-grovedancer.json"),
        ),
        (
            "raise-the-alarm",
            include_str!("../../../tests/fixtures/raise-the-alarm.json"),
        ),
        (
            "queens-commission",
            include_str!("../../../tests/fixtures/queens-commission.json"),
        ),
        (
            "battlegrowth",
            include_str!("../../../tests/fixtures/battlegrowth.json"),
        ),
        (
            "crested-herdcaller",
            include_str!("../../../tests/fixtures/crested-herdcaller.json"),
        ),
        (
            "seller-of-songbirds",
            include_str!("../../../tests/fixtures/seller-of-songbirds.json"),
        ),
        (
            "spore-swarm",
            include_str!("../../../tests/fixtures/spore-swarm.json"),
        ),
        (
            "ferocious-pup",
            include_str!("../../../tests/fixtures/ferocious-pup.json"),
        ),
        (
            "midnight-haunting",
            include_str!("../../../tests/fixtures/midnight-haunting.json"),
        ),
        (
            "attended-knight",
            include_str!("../../../tests/fixtures/attended-knight.json"),
        ),
        (
            "gallant-cavalry",
            include_str!("../../../tests/fixtures/gallant-cavalry.json"),
        ),
        (
            "ironpaw-aspirant",
            include_str!("../../../tests/fixtures/ironpaw-aspirant.json"),
        ),
    ];
    pub const COMBAT_KEYWORD_RECORDS: [(&str, &str); 9] = [
        (
            "youthful-knight",
            include_str!("../../../tests/fixtures/youthful-knight.json"),
        ),
        (
            "knight-of-meadowgrain",
            include_str!("../../../tests/fixtures/knight-of-meadowgrain.json"),
        ),
        (
            "fencing-ace",
            include_str!("../../../tests/fixtures/fencing-ace.json"),
        ),
        (
            "skyhunter-skirmisher",
            include_str!("../../../tests/fixtures/skyhunter-skirmisher.json"),
        ),
        (
            "gurmag-swiftwing",
            include_str!("../../../tests/fixtures/gurmag-swiftwing.json"),
        ),
        (
            "cursed-minotaur",
            include_str!("../../../tests/fixtures/cursed-minotaur.json"),
        ),
        (
            "alley-strangler",
            include_str!("../../../tests/fixtures/alley-strangler.json"),
        ),
        (
            "colossal-dreadmaw",
            include_str!("../../../tests/fixtures/colossal-dreadmaw.json"),
        ),
        (
            "elfhame-wurm",
            include_str!("../../../tests/fixtures/elfhame-wurm.json"),
        ),
    ];
    pub const KEYWORD_RECORDS: [(&str, &str); 16] = [
        MATCH_RECORDS[40],
        (
            "healers-hawk",
            include_str!("../../../tests/fixtures/healers-hawk.json"),
        ),
        (
            "ajanis-sunstriker",
            include_str!("../../../tests/fixtures/ajanis-sunstriker.json"),
        ),
        (
            "canopy-spider",
            include_str!("../../../tests/fixtures/canopy-spider.json"),
        ),
        (
            "angelic-wall",
            include_str!("../../../tests/fixtures/angelic-wall.json"),
        ),
        (
            "serra-angel",
            include_str!("../../../tests/fixtures/serra-angel.json"),
        ),
        (
            "banehound",
            include_str!("../../../tests/fixtures/banehound.json"),
        ),
        (
            "alpine-watchdog",
            include_str!("../../../tests/fixtures/alpine-watchdog.json"),
        ),
        (
            "deadly-recluse",
            include_str!("../../../tests/fixtures/deadly-recluse.json"),
        ),
        (
            "crazed-skirge",
            include_str!("../../../tests/fixtures/crazed-skirge.json"),
        ),
        (
            "greenwood-sentinel",
            include_str!("../../../tests/fixtures/greenwood-sentinel.json"),
        ),
        (
            "dragon-sniper",
            include_str!("../../../tests/fixtures/dragon-sniper.json"),
        ),
        (
            "child-of-night",
            include_str!("../../../tests/fixtures/child-of-night.json"),
        ),
        (
            "typhoid-rats",
            include_str!("../../../tests/fixtures/typhoid-rats.json"),
        ),
        (
            "giant-spider",
            include_str!("../../../tests/fixtures/giant-spider.json"),
        ),
        (
            "vampire-nighthawk",
            include_str!("../../../tests/fixtures/vampire-nighthawk.json"),
        ),
    ];
}

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
    Keywords {
        keywords: Vec<SourceKeyword>,
    },
    Static {
        effect: SourceStaticEffect,
    },
    Triggered {
        #[serde(rename = "abilityWord")]
        ability_word: Option<String>,
        condition: Option<SourceCondition>,
        trigger: SourceTrigger,
        effects: Vec<Sentence>,
    },
    Spell {
        effects: Vec<Sentence>,
    },
    Activated {
        costs: Vec<SourceCost>,
        effects: Vec<Sentence>,
    },
}
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
struct SourceKeyword {
    keyword: String,
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
struct SourceCondition {
    condition: String,
    who: PlayerSelector,
    comparison: SourceComparison,
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
    Enters { what: TriggerObject },
    Dies { what: TriggerObject },
    AnyOf { events: Vec<SourceTrigger> },
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
    other: bool,
    types: Vec<String>,
}
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
struct Sentence {
    steps: Vec<Step>,
}
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
#[serde(tag = "effect", rename_all = "kebab-case")]
enum Step {
    Destroy {
        what: serde_json::Value,
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
        what: Target,
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
        amount: Count,
        to: DamageDestination,
    },
    GainLife {
        who: PlayerSelector,
        amount: Count,
    },
    LoseLife {
        who: PlayerSelector,
        amount: Count,
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
        amount: Count,
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
    Sacrifice { what: ThisObject },
    TapSelf {},
    Mana { symbols: Vec<String> },
}
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(tag = "ref", rename_all = "kebab-case", deny_unknown_fields)]
enum DamageSource {
    #[serde(rename = "self")]
    SelfReference {},
    This {
        noun: String,
    },
}
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(tag = "to", rename_all = "kebab-case", deny_unknown_fields)]
enum DamageDestination {
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
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct PlayerSelector {
    player: String,
    selection_id: Option<String>,
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
    Spell {},
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
        source: AnyDamageSource,
        to: Vec<ReplacementRecipient>,
    },
}
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
struct EmptyFilter {}
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
struct AnyDamageSource {
    #[serde(rename = "ref")]
    reference: String,
    filter: EmptyFilter,
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

fn source_lines(text: &str, name: &str) -> Result<Vec<String>, Failure> {
    // Only the first subset's ASCII text/reminder normalization is consumed.
    // This checks source block association; it does not interpret rule bodies.
    // CR 207.2a, same pinned rules URL as the parser's normalize.ts.
    if text
        .chars()
        .any(|c| !c.is_ascii() && !matches!(c, '—' | '•'))
    {
        return Err(unsupported(
            "source.oracle_text",
            "normalization outside the first source adapter subset",
        ));
    }
    let mut normalized = String::new();
    let mut depth = 0_u32;
    for c in text.chars() {
        match c {
            '(' => {
                depth = depth
                    .checked_add(1)
                    .ok_or_else(|| invalid("source.oracle_text", "parenthesis depth limit"))?;
                if depth == 1 {
                    normalized.push(' ');
                }
            }
            ')' if depth > 0 => depth -= 1,
            ')' => return Err(invalid("source.oracle_text", "unbalanced reminder text")),
            '\n' => normalized.push('\n'),
            c if depth == 0 => normalized.push(c),
            _ => {}
        }
    }
    if depth != 0 {
        return Err(invalid("source.oracle_text", "unbalanced reminder text"));
    }
    let lines: Vec<String> = normalized
        .lines()
        .map(|line| {
            let line = line.split_whitespace().collect::<Vec<_>>().join(" ");
            // The reviewed archive normalizes a complete source name at the
            // start of a block to ~ (CR 201.5). Keep ASCII word boundaries;
            // literal-name and legendary-short-name grammar is not interpreted.
            let (prefix, body) = line
                .strip_prefix("• ")
                .map_or(("", line.as_str()), |body| ("• ", body));
            match body.strip_prefix(name) {
                Some(rest)
                    if rest
                        .chars()
                        .next()
                        .is_some_and(|c| !c.is_ascii_alphanumeric() && c != '_') =>
                {
                    format!("{prefix}~{rest}")
                }
                _ => line,
            }
        })
        .filter(|line| !line.is_empty())
        .collect();
    // The parser groups a complete choose/bullet source block into one line
    // entry. Preserve its newlines and account for every bullet, never discard
    // unknown alternatives. CR700.2: selection semantics are bound from AST.
    let mut blocks: Vec<String> = vec![];
    for line in lines {
        if line.starts_with("• ") {
            let Some(previous) = blocks.last_mut().filter(|b| b.starts_with("Choose ")) else {
                return Err(invalid("lines", "orphan modal source option"));
            };
            previous.push('\n');
            previous.push_str(&line);
        } else {
            blocks.push(line);
        }
    }
    Ok(blocks)
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
    // Pin the adapter to the exact audited archive header, not just its version.
    let _: serde_json::Value = decode(manifest, "manifest")?;
    if sha256(manifest.trim_end().as_bytes()) != ARCHIVE_MANIFEST_SHA256 {
        return Err(invalid(
            "manifest",
            "unsupported archive version, source identity, or rules edition",
        ));
    }
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
    let source: Source = decode(r.source.get(), "source")?;
    if source.oracle_id != r.oracle_identity || source.id.is_empty() || source.name.is_empty() {
        return Err(invalid("source", "missing or mismatched card identity"));
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
    if source_lines(&source.oracle_text, &source.name)?
        != result
            .lines
            .iter()
            .map(|l| l.text.clone())
            .collect::<Vec<_>>()
    {
        return Err(invalid(
            "lines",
            "source contains unaccounted or mismatched normalized blocks",
        ));
    }
    let ast_sha256 = r
        .ast_sha256
        .ok_or_else(|| invalid("astSha256", "missing successful AST hash"))?;
    if sha256(format!("[{}]", result.abilities.get()).as_bytes()) != ast_sha256 {
        return Err(invalid("astSha256", "successful AST hash mismatch"));
    }
    let abilities: Vec<Ability> =
        strict_read(result.abilities.get()).map_err(|e| unsupported("abilities", e))?;
    if result.lines.len() != abilities.len() {
        return Err(invalid("lines", "unaccounted ability block"));
    }
    for (line, ability) in result.lines.iter().zip(&abilities) {
        let a: Ability =
            strict_read(line.ability.get()).map_err(|e| unsupported("lines.ability", e))?;
        if &a != ability || line.text.is_empty() {
            return Err(invalid("lines", "line and aggregate ability disagree"));
        }
    }
    let (types, subtypes) = source
        .type_line
        .split_once(" — ")
        .unwrap_or((&source.type_line, ""));
    let (kind, basic) = match types {
        "Basic Land" => (CardKind::Land, true),
        "Creature" => (CardKind::Creature, false),
        "Legendary Creature" => (CardKind::Creature, false),
        "Instant" => (CardKind::Instant, false),
        "Sorcery" => (CardKind::Sorcery, false),
        "Enchantment" => (CardKind::Enchantment, false),
        _ => {
            return Err(unsupported(
                "source.type_line",
                "unsupported intrinsic type/supertype family",
            ));
        }
    };
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
    let mut definition = Definition {
        oracle_id: r.oracle_identity,
        name: source.name,
        characteristics: Characteristics {
            kind,
            basic,
            legendary: types == "Legendary Creature",
            subtypes: subtypes.split_whitespace().map(str::to_owned).collect(),
            mana_cost: mana_cost(&source.mana_cost)?,
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
            archive_manifest_sha256: ARCHIVE_MANIFEST_SHA256.into(),
            parser_sha256: PARSER_SHA256.into(),
            source_json: r.source.get().into(),
            abilities_json: result.abilities.get().into(),
        },
    };
    match kind {
        CardKind::Enchantment => {
            if source.oracle_text.is_empty() || abilities.is_empty() {
                return Err(unsupported("abilities", "empty enchantment behavior"));
            }
            definition.spell = Some(SpellProgram {
                timing: SpellTiming::MainPhase,
                resolution: SpellResolution::Permanent,
                functioning_zone: FunctioningZone::Stack,
                targets: vec![],
                instructions: vec![],
                modes: vec![],
            });
            for ability in &abilities {
                let Ability::Static { effect } = ability else {
                    return Err(unsupported(
                        "abilities.kind",
                        "unsupported enchantment ability",
                    ));
                };
                definition.static_abilities.push(lower_static(effect)?);
            }
        }
        CardKind::Land | CardKind::Creature => {
            if kind == CardKind::Land && !abilities.is_empty() {
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
            if kind == CardKind::Creature {
                // CR 302.1–2 / 608.3a: casting a creature and activating one of
                // its battlefield abilities are distinct executable programs.
                // https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
                definition.spell = Some(SpellProgram {
                    timing: SpellTiming::MainPhase,
                    resolution: SpellResolution::Permanent,
                    functioning_zone: FunctioningZone::Stack,
                    targets: vec![],
                    instructions: vec![],
                    modes: vec![],
                });
                for (block, ability) in abilities.iter().enumerate() {
                    match ability {
                        Ability::Keywords { keywords } => {
                            if keywords.is_empty() {
                                return Err(unsupported(
                                    "abilities.keywords",
                                    "empty keyword block",
                                ));
                            }
                            for keyword in keywords {
                                match keyword.keyword.as_str() {
                                    "defender" => {
                                        definition.keywords.push(KeywordAbility::Defender)
                                    }
                                    "trample" => definition.keywords.push(KeywordAbility::Trample),
                                    "first strike" => {
                                        definition.keywords.push(KeywordAbility::FirstStrike)
                                    }
                                    "double strike" => {
                                        definition.keywords.push(KeywordAbility::DoubleStrike)
                                    }
                                    "menace" => definition.keywords.push(KeywordAbility::Menace),
                                    "flying" => definition.keywords.push(KeywordAbility::Flying),
                                    "reach" => definition.keywords.push(KeywordAbility::Reach),
                                    "vigilance" => {
                                        definition.keywords.push(KeywordAbility::Vigilance)
                                    }
                                    "haste" => definition.keywords.push(KeywordAbility::Haste),
                                    "lifelink" => {
                                        definition.keywords.push(KeywordAbility::Lifelink)
                                    }
                                    "deathtouch" => {
                                        definition.keywords.push(KeywordAbility::Deathtouch)
                                    }
                                    _ => {
                                        return Err(unsupported(
                                            "abilities.keywords",
                                            "unsupported whole-card keyword family",
                                        ));
                                    }
                                }
                            }
                        }
                        Ability::Activated { costs, effects } => definition
                            .activated
                            .push(lower_activation(costs, effects, block)?),
                        Ability::Triggered {
                            trigger,
                            effects,
                            condition,
                            ability_word,
                        } => {
                            if ability_word.as_ref().is_some_and(|word| {
                                word.is_empty()
                                    || !word.chars().all(|c| c.is_ascii_alphabetic() || c == ' ')
                            }) {
                                return Err(unsupported(
                                    "abilities.abilityWord",
                                    "unsupported annotation",
                                ));
                            }
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
        }
        CardKind::Instant | CardKind::Sorcery => {
            if source.oracle_text.is_empty() {
                return Err(invalid("source.oracle_text", "spell has no source text"));
            }
            let mut program = SpellProgram {
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
                let Ability::Spell { effects } = ability else {
                    return Err(unsupported(
                        "abilities.kind",
                        "nonspell ability on instant/sorcery",
                    ));
                };
                // Separate source blocks introduce independent occurrences (CR115.3).
                // Reusing a selection ID across blocks must not alias targets.
                slots.clear();
                append_spell_effects(&mut program, effects, block, &mut slots)?;
            }
            definition.spell = Some(program);
        }
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
    });
    slots.insert(id.into(), slot);
    Ok(slot)
}
fn append_spell_effects(
    program: &mut SpellProgram,
    effects: &[Sentence],
    block: usize,
    slots: &mut BTreeMap<String, u32>,
) -> Result<(), Failure> {
    if let [Sentence { steps }] = effects
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
                timing: program.timing,
                resolution: program.resolution,
                functioning_zone: program.functioning_zone,
                targets: vec![],
                instructions: vec![],
                modes: vec![],
            };
            append_spell_effects(&mut child, &option.effects, block, &mut BTreeMap::new())?;
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
            if who.player != "you"
                || who.selection_id.is_some()
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
            let instruction = match step {
                Step::Destroy {
                    regeneration_forbidden: Some(_),
                    ..
                } => {
                    return Err(unsupported(
                        "effects.destroy.regenerationForbidden",
                        "regeneration and cannot-be-regenerated clauses are outside this slice",
                    ));
                }
                Step::Destroy {
                    what,
                    regeneration_forbidden: None,
                } => {
                    if *what == serde_json::json!({"ref":"all","filter":{"types":["creature"]}}) {
                        Instruction::DestroyAllCreatures { source_block }
                    } else {
                        let t: Target = serde_json::from_value(what.clone()).map_err(|_| {
                            unsupported("effects.destroy", "unsupported destroy filter")
                        })?;
                        Instruction::DestroyTargetCreature {
                            target_slot: bind_creature_target(program, slots, &t)?,
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
                Step::CreateToken { count, token }
                    if count.amount == "fixed" && count.value > 0 =>
                {
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
                            let obj = k.as_object().ok_or_else(|| {
                                unsupported("effects.token.keywords", "invalid keyword")
                            })?;
                            if obj.len() != 1 {
                                return Err(unsupported(
                                    "effects.token.keywords",
                                    "parameterized keyword",
                                ));
                            }
                            serde_json::from_value::<KeywordAbility>(k["keyword"].clone()).map_err(
                                |_| unsupported("effects.token.keywords", "unsupported keyword"),
                            )
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
                } if duration.duration == "end-of-turn" => Instruction::ModifyPowerToughness {
                    target_slot: bind_creature_target(program, slots, what)?,
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
                    source: DamageSource::SelfReference {},
                    amount,
                    to,
                } => {
                    let DamageDestination::AnyTarget { selection_id } = to;
                    Instruction::DealDamage {
                        target_slot: bind_target(
                            program,
                            slots,
                            selection_id,
                            TargetDomain::AnyDamageTarget,
                        )?,
                        amount: fixed_positive(amount)?,
                        source_block,
                    }
                }
                Step::GainLife { who, amount } => Instruction::GainLife {
                    recipient: life_recipient(program, slots, who)?,
                    amount: fixed_positive(amount)?,
                    source_block,
                },
                Step::LoseLife { who, amount } => Instruction::LoseLife {
                    recipient: life_recipient(program, slots, who)?,
                    amount: fixed_positive(amount)?,
                    source_block,
                },
                Step::Draw { who, amount } => {
                    let recipient = match life_recipient(program, slots, who)? {
                        LifeRecipient::Controller => None,
                        r => Some(r),
                    };
                    if amount.amount != "fixed" || !(1..=2).contains(&amount.value) {
                        return Err(unsupported("effects.draw", "unsupported fixed draw count"));
                    }
                    // CR 121.2: multiple draws are performed one at a time.
                    for _ in 1..amount.value {
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
                        CounterFilter::Spell {} => TargetDomain::StackSpell,
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
                    Instruction::CounterStackObject {
                        target_slot: bind_target(program, slots, &what.selection_id, domain)?,
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
                } if who.player == "target-opponent"
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
        ReplacementRecipient::Player(p) if p.player == "a-player" && p.selection_id.is_none())
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
    // CR 611.3b/614.5: battlefield-only static rule, once per damage lineage.
    // https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
    Ok(StaticAbility {
        functioning_zone: FunctioningZone::Battlefield,
        effect: StaticEffect::DoubleDamageToPermanentOrPlayer,
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
) -> Result<LifeRecipient, Failure> {
    match (who.player.as_str(), who.selection_id.as_deref()) {
        ("you", None) => Ok(LifeRecipient::Controller),
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
    matches!(subject,TriggerObject::Filter {filter,count} if filter.other && filter.types==["creature"] && fixed_one(count))
}
fn lower_trigger(
    trigger: &SourceTrigger,
    effects: &[Sentence],
    intervening: Option<&SourceCondition>,
    block: usize,
) -> Result<TriggeredAbility, Failure> {
    // The archive reference-binding object is diagnostic/nonexecutable. This
    // Rust semantic binder consumes the complete typed AST instead: "other"
    // excludes this exact source incarnation (CR 201.5), and the exact disjoint
    // union [this creature dies OR another creature dies] is every creature
    // death, once per actual move. It does not bind an unused event pronoun.
    // ETB checks the resulting battlefield; dies checks the prior battlefield
    // (CR 603.6a/603.10/700.4). Runtime captures those event/source facts.
    // https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
    let condition=match trigger {
        SourceTrigger::Dies {what} if is_source_creature(what)=>TriggerCondition::SelfDies,
        SourceTrigger::Enters {what} if is_source_creature(what)=>TriggerCondition::SelfEntersBattlefield,
        SourceTrigger::Enters {what} if is_other_creature(what)=>TriggerCondition::OtherCreatureEntersBattlefield,
        SourceTrigger::AnyOf {events} if events.len()==2
            && events.iter().filter(|event|matches!(event,SourceTrigger::Dies {what} if is_source_creature(what))).count()==1
            && events.iter().filter(|event|matches!(event,SourceTrigger::Dies {what} if is_other_creature(what))).count()==1 =>TriggerCondition::AnyCreatureDies,
        _=>return Err(unsupported("abilities.trigger","unsupported complete event/source/other/alternative trigger binding")),
    };
    let mut program = SpellProgram {
        timing: SpellTiming::Instant,
        resolution: SpellResolution::Instructions,
        functioning_zone: FunctioningZone::Stack,
        targets: vec![],
        instructions: vec![],
        modes: vec![],
    };
    append_spell_effects(&mut program, effects, block, &mut BTreeMap::new())?;
    let intervening_if = match intervening {
        None => None,
        Some(c)
            if condition == TriggerCondition::SelfDies
                && c.condition == "cards-in-hand"
                && c.who.player == "you"
                && c.who.selection_id.is_none()
                && c.comparison.op == "eq"
                && c.comparison.value.amount == "fixed"
                && c.comparison.value.value == 0 =>
        {
            Some(InterveningCondition::ControllerHandEmpty)
        }
        _ => {
            return Err(unsupported(
                "abilities.condition",
                "unsupported complete intervening-if condition",
            ));
        }
    };
    Ok(TriggeredAbility {
        intervening_if,
        functioning_zone: FunctioningZone::Battlefield,
        condition,
        program,
    })
}

fn lower_activation(
    costs: &[SourceCost],
    effects: &[Sentence],
    block: usize,
) -> Result<ActivatedAbility, Failure> {
    let cost = match costs {
        [SourceCost::Sacrifice { what }] if what.reference == "this" && what.noun == "creature" => {
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
        _ => {
            return Err(unsupported(
                "abilities.costs",
                "unsupported complete activation cost or combined costs",
            ));
        }
    };
    let source_block = source_block(block)?;
    let mut program = SpellProgram {
        timing: SpellTiming::Instant,
        resolution: SpellResolution::Instructions,
        functioning_zone: FunctioningZone::Stack,
        targets: vec![],
        instructions: vec![],
        modes: vec![],
    };
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
            };
            let instruction = match step {
                Step::PreventDamage {
                    prevention,
                    duration,
                } => {
                    lower_prevention(&mut program, &mut slots, prevention, duration, source_block)?
                }
                Step::Damage {
                    source: DamageSource::This { noun },
                    amount,
                    to,
                } if noun == "creature" => {
                    let DamageDestination::AnyTarget { selection_id } = to;
                    Instruction::DealDamage {
                        target_slot: bind_target(
                            &mut program,
                            &mut slots,
                            selection_id,
                            TargetDomain::AnyDamageTarget,
                        )?,
                        amount: fixed_positive(amount)?,
                        source_block,
                    }
                }
                Step::Draw { who, amount } if *who == you && fixed_one(amount) => {
                    Instruction::DrawCards {
                        recipient: None,
                        count: Quantity::from(1),
                        source_block,
                    }
                }
                Step::Discard { who, what }
                    if *who == you
                        && what.reference == "filter"
                        && fixed_one(&what.count)
                        && what.filter
                            == ObjectFilter::Class(CardFilter { cls: "card".into() }) =>
                {
                    Instruction::DiscardCards {
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

pub fn fixture_catalog() -> Result<Catalog, String> {
    catalog_from_records(&fixtures::RECORDS[..5], 3)
}

/// Exact extended source pool for the Commander casting/lifecycle scenarios.
/// Existing profiles retain their original catalog and its provenance digest.
pub fn commander_fixture_catalog() -> Result<Catalog, String> {
    let records = fixtures::COMMANDER_RECORDS
        .iter()
        .filter(|(name, _)| *name != "cruel-edict")
        .copied()
        .collect::<Vec<_>>();
    catalog_from_records(&records, 3)
}

/// Exact IR-v4 source pool for reviewed library-top replacement scenarios.
pub fn library_fixture_catalog() -> Result<Catalog, String> {
    let records = fixtures::LIBRARY_RECORDS
        .iter()
        .filter(|(name, _)| *name != "cruel-edict")
        .copied()
        .collect::<Vec<_>>();
    catalog_from_records(&records, 4)
}

/// Exact unchanged-vocabulary pool for natural Commander damage scenarios.
pub fn damage_fixture_catalog() -> Result<Catalog, String> {
    let records = fixtures::DAMAGE_RECORDS
        .iter()
        .filter(|(name, _)| *name != "cruel-edict")
        .copied()
        .collect::<Vec<_>>();
    catalog_from_records(&records, 3)
}

/// Exact complete-card action pool; prior pools still select their own definitions.
pub fn action_fixture_catalog() -> Result<Catalog, String> {
    let records = fixtures::ACTION_RECORDS
        .iter()
        .filter(|(name, _)| *name != "blood-artist")
        .copied()
        .collect::<Vec<_>>();
    catalog_from_records(&records, 5)
}

/// Exact IR-v6 stack targeting and activation-cost pool.
pub fn stack_fixture_catalog() -> Result<Catalog, String> {
    let records = fixtures::STACK_RECORDS
        .iter()
        .filter(|(name, _)| *name != "blood-artist")
        .copied()
        .collect::<Vec<_>>();
    catalog_from_records(&records, 6)
}

/// Exact IR-v7 source pool with paired triggered and aggregate negatives.
pub fn damage_life_fixture_catalog() -> Result<Catalog, String> {
    let records = fixtures::DAMAGE_LIFE_RECORDS
        .iter()
        .filter(|(name, _)| !matches!(*name, "blood-artist" | "flame-rift"))
        .copied()
        .collect::<Vec<_>>();
    catalog_from_records(&records, 7)
}

/// Exact IR-v8 pool; historical selected definitions stay byte-equivalent.
pub fn trigger_fixture_catalog() -> Result<Catalog, String> {
    let records = fixtures::TRIGGER_RECORDS
        .iter()
        .filter(|(name, _)| !matches!(*name, "flame-rift" | "deathgreeter"))
        .copied()
        .collect::<Vec<_>>();
    catalog_from_records(&records, 8)
}

pub fn effects_fixture_catalog() -> Result<Catalog, String> {
    let records = fixtures::EFFECT_RECORDS
        .iter()
        .filter(|(name, _)| {
            !matches!(
                *name,
                "flame-rift"
                    | "deathgreeter"
                    | "righteousness"
                    | "healing-salve"
                    | "gratuitous-violence"
            )
        })
        .copied()
        .collect::<Vec<_>>();
    catalog_from_records(&records, 9)
}

pub fn choices_fixture_catalog() -> Result<Catalog, String> {
    let records = fixtures::CHOICE_RECORDS
        .iter()
        .filter(|(name, _)| {
            !matches!(
                *name,
                "flame-rift"
                    | "deathgreeter"
                    | "righteousness"
                    | "gratuitous-violence"
                    | "boros-charm"
                    | "lavaborn-muse"
            )
        })
        .copied()
        .collect::<Vec<_>>();
    catalog_from_records(&records, 10)
}

pub fn match_fixture_catalog() -> Result<Catalog, String> {
    let records = fixtures::MATCH_RECORDS
        .iter()
        .filter(|(name, _)| {
            !matches!(
                *name,
                "flame-rift"
                    | "deathgreeter"
                    | "righteousness"
                    | "gratuitous-violence"
                    | "boros-charm"
                    | "lavaborn-muse"
                    | "wall-of-vines"
            )
        })
        .copied()
        .collect::<Vec<_>>();
    catalog_from_records(&records, 11)
}

/// Separate IR-v12 closure; do not expand the historical v11 catalog.
pub fn keywords_fixture_catalog() -> Result<Catalog, String> {
    let mut catalog = match_fixture_catalog()?;
    catalog.schema_version = 12;
    catalog
        .definitions
        .extend(catalog_from_records(&fixtures::KEYWORD_RECORDS, 12)?.definitions);
    catalog
        .definitions
        .sort_by(|a, b| a.oracle_id.cmp(&b.oracle_id));
    catalog.validate()?;
    Ok(catalog)
}

/// Separate IR-v13 closure, preserving every earlier profile definition.
pub fn combat_keywords_fixture_catalog() -> Result<Catalog, String> {
    let mut catalog = keywords_fixture_catalog()?;
    catalog.schema_version = 13;
    catalog
        .definitions
        .extend(catalog_from_records(&fixtures::COMBAT_KEYWORD_RECORDS, 13)?.definitions);
    catalog
        .definitions
        .sort_by(|a, b| a.oracle_id.cmp(&b.oracle_id));
    catalog.validate()?;
    Ok(catalog)
}

/// Separate IR-v14 closure; earlier profile definitions remain byte-identical.
pub fn tokens_fixture_catalog() -> Result<Catalog, String> {
    let mut catalog = combat_keywords_fixture_catalog()?;
    catalog.schema_version = 14;
    catalog
        .definitions
        .extend(catalog_from_records(&fixtures::TOKEN_RECORDS, 14)?.definitions);
    catalog
        .definitions
        .sort_by(|a, b| a.oracle_id.cmp(&b.oracle_id));
    catalog.validate()?;
    Ok(catalog)
}

/// Separate IR-v15 closure; historical catalogs remain unchanged.
pub fn staples_fixture_catalog() -> Result<Catalog, String> {
    let mut catalog = tokens_fixture_catalog()?;
    catalog.schema_version = 15;
    catalog
        .definitions
        .extend(catalog_from_records(&fixtures::STAPLES_RECORDS, 15)?.definitions);
    catalog
        .definitions
        .sort_by(|a, b| a.oracle_id.cmp(&b.oracle_id));
    catalog.validate()?;
    Ok(catalog)
}

fn catalog_from_records(records: &[(&str, &str)], schema_version: u32) -> Result<Catalog, String> {
    let mut definitions = vec![];
    for (_, r) in records {
        match compile(fixtures::MANIFEST, r) {
            CompileResult::Lowered { definition } => definitions.push(*definition),
            result => return Err(format!("unexpected fixture compile result: {result:?}")),
        }
    }
    definitions.sort_by(|a, b| a.oracle_id.cmp(&b.oracle_id));
    let catalog = Catalog {
        schema_version,
        rules_edition: RULES_EDITION.into(),
        rules_sha256: RULES_SHA256.into(),
        definitions,
    };
    catalog.validate()?;
    Ok(catalog)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn staples_v15_source_pairs_and_historical_identity() {
        let catalog = staples_fixture_catalog().unwrap();
        assert_eq!(
            (catalog.schema_version, catalog.definitions.len()),
            (15, 81)
        );
        assert_eq!(
            catalog,
            Catalog::read(include_str!(
                "../../../tests/fixtures/expected-staples-ir.json"
            ))
            .unwrap()
        );
        for d in tokens_fixture_catalog().unwrap().definitions {
            assert_eq!(
                catalog
                    .definitions
                    .iter()
                    .find(|x| x.oracle_id == d.oracle_id),
                Some(&d)
            );
        }
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
            if name != "nights-whisper" {
                let mut bad = catalog.clone();
                bad.schema_version = 14;
                bad.definitions = vec![*definition];
                assert!(bad.validate().is_err(), "{name}");
            }
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
    fn changed_record(index: usize, change: impl FnOnce(&mut serde_json::Value)) -> String {
        changed_source(fixtures::LIBRARY_RECORDS[index].1, change)
    }
    fn changed_source(raw: &str, change: impl FnOnce(&mut serde_json::Value)) -> String {
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
    fn tokens_v14_sources_and_rejection_pairs() {
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
        let catalog = tokens_fixture_catalog().unwrap();
        assert_eq!(
            (catalog.schema_version, catalog.definitions.len()),
            (14, 72)
        );
        assert_eq!(
            catalog,
            Catalog::read(include_str!(
                "../../../tests/fixtures/expected-tokens-ir.json"
            ))
            .unwrap()
        );
        for old in combat_keywords_fixture_catalog().unwrap().definitions {
            assert_eq!(
                catalog
                    .definitions
                    .iter()
                    .find(|d| d.oracle_id == old.oracle_id),
                Some(&old)
            );
        }
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
            for version in 3..=13 {
                let mut bad = catalog.clone();
                bad.schema_version = version;
                bad.definitions = vec![*definition.clone()];
                assert!(bad.validate().is_err(), "{name} v{version}");
            }
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
    fn combat_keywords_v13_sources_and_downgrade_pairs() {
        let catalog = combat_keywords_fixture_catalog().unwrap();
        assert_eq!(
            (catalog.schema_version, catalog.definitions.len()),
            (13, 59)
        );
        assert_eq!(
            catalog,
            Catalog::read(include_str!(
                "../../../tests/fixtures/expected-combat-keywords-ir.json"
            ))
            .unwrap()
        );
        for old in keywords_fixture_catalog().unwrap().definitions {
            assert_eq!(
                catalog
                    .definitions
                    .iter()
                    .find(|d| d.oracle_id == old.oracle_id),
                Some(&old)
            );
        }
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
            for version in 3..=12 {
                let mut bad = catalog.clone();
                bad.schema_version = version;
                bad.definitions = vec![*definition.clone()];
                assert!(bad.validate().is_err(), "{name} v{version}");
            }
        }
    }
    #[test]
    fn keywords_v12_complete_sources_and_downgrade_pairs() {
        let catalog = keywords_fixture_catalog().unwrap();
        assert_eq!(catalog.schema_version, 12);
        assert_eq!(catalog.definitions.len(), 50);
        for old in match_fixture_catalog().unwrap().definitions {
            assert_eq!(
                catalog
                    .definitions
                    .iter()
                    .find(|d| d.oracle_id == old.oracle_id),
                Some(&old)
            );
        }
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
            for version in 3..=11 {
                let mut bad = catalog.clone();
                bad.schema_version = version;
                bad.definitions = vec![*definition.clone()];
                assert!(bad.validate().is_err(), "{name} v{version}");
            }
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
    fn defender_lowers_complete_keyword_without_widening_previous_catalog() {
        let c = match_fixture_catalog().unwrap();
        assert_eq!(
            (
                c.schema_version,
                c.definitions.len(),
                fixtures::MATCH_RECORDS.len()
            ),
            (11, 34, 41)
        );
        assert_eq!(
            c,
            Catalog::read(include_str!(
                "../../../tests/fixtures/expected-match-ir.json"
            ))
            .unwrap()
        );
        for previous in choices_fixture_catalog().unwrap().definitions {
            assert_eq!(
                c.definitions
                    .iter()
                    .find(|d| d.oracle_id == previous.oracle_id),
                Some(&previous)
            );
        }
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
            (|a: &mut serde_json::Value| a["keywords"][0]["keyword"] = "hexproof".into())
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
        let c = match_fixture_catalog().unwrap();
        let wall = c
            .definitions
            .iter()
            .find(|d| !d.keywords.is_empty())
            .unwrap();
        for v in 3..11 {
            let mut bad = c.clone();
            bad.schema_version = v;
            bad.definitions = vec![wall.clone()];
            assert!(bad.validate().is_err());
            bad.definitions[0].capabilities.clear();
            assert!(bad.validate().is_err());
        }
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
    fn complete_choices_sources_preserve_occurrences_modes_conditions_and_old_catalogs() {
        let c = choices_fixture_catalog().unwrap();
        assert_eq!(
            c,
            Catalog::read(include_str!(
                "../../../tests/fixtures/expected-choices-ir.json"
            ))
            .unwrap()
        );
        assert_eq!(
            (
                c.schema_version,
                c.definitions.len(),
                fixtures::CHOICE_RECORDS.len()
            ),
            (10, 33, 39)
        );
        for d in effects_fixture_catalog().unwrap().definitions {
            assert_eq!(
                c.definitions.iter().find(|n| n.oracle_id == d.oracle_id),
                Some(&d)
            );
        }
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
            effects_fixture_catalog()
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
    fn choices_ir_rejects_downgrades_nested_modes_and_inconsistent_children() {
        let c = choices_fixture_catalog().unwrap();
        for name in ["Healing Salve", "Slaughterhouse Bouncer"] {
            let d = c.definitions.iter().find(|d| d.name == name).unwrap();
            for v in 3..10 {
                let mut bad = c.clone();
                bad.schema_version = v;
                bad.definitions = vec![d.clone()];
                assert!(bad.validate().is_err());
                bad.definitions[0].capabilities.clear();
                assert!(bad.validate().is_err());
            }
        }
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
    fn complete_effects_catalog_binds_all_six_sources_and_preserves_historical_definitions() {
        let c = effects_fixture_catalog().unwrap();
        assert_eq!(
            (
                c.schema_version,
                c.definitions.len(),
                fixtures::EFFECT_RECORDS.len()
            ),
            (9, 30, 35)
        );
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
        let old = trigger_fixture_catalog().unwrap();
        for d in old.definitions {
            assert_eq!(
                c.definitions.iter().find(|n| n.oracle_id == d.oracle_id),
                Some(&d)
            );
        }
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
        for name in [
            "righteousness",
            "gratuitous-violence",
            "flame-rift",
            "deathgreeter",
        ] {
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
    fn effects_vocabulary_and_capabilities_cannot_downgrade_into_historical_ir() {
        let c = effects_fixture_catalog().unwrap();
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
            for version in 3..9 {
                let mut old = c.clone();
                old.schema_version = version;
                old.definitions = vec![d.clone()];
                assert!(old.validate().is_err());
                old.definitions[0].capabilities.clear();
                assert!(old.validate().is_err());
            }
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
        let c = effects_fixture_catalog().unwrap();
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
        future.schema_version = 16;
        assert!(future.validate().is_err());
    }
    #[test]
    fn complete_trigger_pool_binds_exact_source_roles_and_preserves_historical_catalogs() {
        let catalog = trigger_fixture_catalog().unwrap();
        assert_eq!(catalog.schema_version, 8);
        assert_eq!(catalog.definitions.len(), 24);
        assert_eq!(
            Catalog::read(include_str!(
                "../../../tests/fixtures/expected-trigger-ir.json"
            ))
            .unwrap(),
            catalog
        );
        for (old, expected) in [
            (
                fixture_catalog().unwrap(),
                include_str!("../../../tests/fixtures/expected-ir.json"),
            ),
            (
                commander_fixture_catalog().unwrap(),
                include_str!("../../../tests/fixtures/expected-commander-ir.json"),
            ),
            (
                damage_fixture_catalog().unwrap(),
                include_str!("../../../tests/fixtures/expected-damage-ir.json"),
            ),
            (
                library_fixture_catalog().unwrap(),
                include_str!("../../../tests/fixtures/expected-library-ir.json"),
            ),
            (
                action_fixture_catalog().unwrap(),
                include_str!("../../../tests/fixtures/expected-action-ir.json"),
            ),
            (
                stack_fixture_catalog().unwrap(),
                include_str!("../../../tests/fixtures/expected-stack-ir.json"),
            ),
            (
                damage_life_fixture_catalog().unwrap(),
                include_str!("../../../tests/fixtures/expected-damage-life-ir.json"),
            ),
        ] {
            assert_eq!(
                canonical(&old).unwrap(),
                canonical(&Catalog::read(expected).unwrap()).unwrap()
            );
            for d in old.definitions {
                assert!(d.triggered.is_empty());
                assert_eq!(
                    catalog
                        .definitions
                        .iter()
                        .find(|new| new.oracle_id == d.oracle_id),
                    Some(&d)
                );
            }
        }
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
                intervening_if: None,
                functioning_zone: FunctioningZone::Battlefield,
                condition: TriggerCondition::SelfEntersBattlefield,
                program: SpellProgram {
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
            ("optional instruction", 23, |r| {
                r["faces"][0]["result"]["abilities"][0]["effects"][0]["steps"][0]["optional"] =
                    true.into()
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
    fn triggered_ir_requires_v8_exact_capability_closure_and_mandatory_stack_programs() {
        let c = trigger_fixture_catalog().unwrap();
        for name in ["Elvish Visionary", "Essence Warden", "Blood Artist"] {
            for version in 3..=7 {
                let mut old = c.clone();
                old.schema_version = version;
                old.definitions.retain(|d| d.name == name);
                assert!(
                    Catalog::read(&canonical(&old).unwrap()).is_err(),
                    "{name} v{version}"
                );
            }
        }
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
        bad.schema_version = 16;
        assert!(bad.validate().is_err());
    }

    #[test]
    fn damage_life_pool_retains_all_source_blocks_targets_quantities_and_printed_costs() {
        let c = damage_life_fixture_catalog().unwrap();
        assert_eq!(c.schema_version, 7);
        assert_eq!(c.definitions.len(), 21);
        assert_eq!(
            Catalog::read(include_str!(
                "../../../tests/fixtures/expected-damage-life-ir.json"
            ))
            .unwrap(),
            c
        );
        for prior in stack_fixture_catalog().unwrap().definitions {
            assert_eq!(
                c.definitions
                    .iter()
                    .find(|d| d.oracle_id == prior.oracle_id),
                Some(&prior)
            );
        }
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
    fn damage_life_programs_require_ir7_and_strict_target_recipient_quantity_closure() {
        let catalog = damage_life_fixture_catalog().unwrap();
        for name in [
            "Lightning Bolt",
            "Prodigal Sorcerer",
            "Soul Feast",
            "Healing Hands",
            "Sacred Nectar",
        ] {
            for version in 3..=6 {
                let mut bad = catalog.clone();
                bad.definitions.retain(|d| d.name == name);
                bad.schema_version = version;
                assert!(
                    Catalog::read(&canonical(&bad).unwrap()).is_err(),
                    "{name} v{version}"
                );
            }
        }
        let mut bad = catalog.clone();
        bad.schema_version = 16;
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
    fn legacy_spell_and_activation_shapes_cannot_hide_ir7_relaxations() {
        let mut c = fixture_catalog().unwrap();
        c.schema_version = 7;
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
        for v in 3..=6 {
            let mut old = c.clone();
            old.schema_version = v;
            assert!(old.validate().is_err());
        }
        let mut c = damage_life_fixture_catalog().unwrap();
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
    fn stack_pool_retains_complete_counter_domains_costs_and_prior_definitions() {
        let catalog = stack_fixture_catalog().unwrap();
        assert_eq!(catalog.schema_version, 6);
        assert_eq!(catalog.definitions.len(), 15);
        assert_eq!(
            Catalog::read(include_str!(
                "../../../tests/fixtures/expected-stack-ir.json"
            ))
            .unwrap(),
            catalog
        );
        for previous in action_fixture_catalog().unwrap().definitions {
            assert_eq!(
                catalog
                    .definitions
                    .iter()
                    .find(|d| d.oracle_id == previous.oracle_id),
                Some(&previous)
            );
        }
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
    fn stack_vocabulary_requires_ir6_and_rejects_bad_binding_and_capability_closure() {
        let catalog = stack_fixture_catalog().unwrap();
        for version in [3, 4, 5, 16] {
            let mut bad = catalog.clone();
            bad.schema_version = version;
            assert!(Catalog::read(&canonical(&bad).unwrap()).is_err());
        }
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
        let c = fixture_catalog().unwrap();
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
    fn opponent_sacrifice_lowers_as_resolution_choice_without_widening_prior_catalogs() {
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
        for catalog in [
            fixture_catalog().unwrap(),
            commander_fixture_catalog().unwrap(),
            damage_fixture_catalog().unwrap(),
            library_fixture_catalog().unwrap(),
        ] {
            assert!(
                !catalog
                    .definitions
                    .iter()
                    .any(|d| d.oracle_id == definition.oracle_id)
            );
        }
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
        let c = fixture_catalog().unwrap();
        let raw = canonical(&c).unwrap();
        for bad in [
            raw.replace("\"schema_version\":3", "\"schema_version\":16"),
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
        let catalog = fixture_catalog().unwrap();
        assert_eq!(catalog.schema_version, 3);
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
                CardKind::Sorcery | CardKind::Enchantment => {
                    panic!("legacy catalog cannot contain sorceries/enchantments")
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
        let original = fixture_catalog().unwrap();
        let catalog = commander_fixture_catalog().unwrap();
        assert_eq!(catalog.schema_version, 3);
        assert_eq!(catalog.definitions.len(), original.definitions.len() + 1);
        for definition in &original.definitions {
            assert!(catalog.definitions.contains(definition));
        }
        let plains = catalog
            .definitions
            .iter()
            .find(|definition| definition.oracle_id == "bc71ebf6-2056-41f7-be35-b2e5c34afa99")
            .unwrap();
        assert_eq!(plains.name, "Plains");
        assert_eq!(plains.provenance.source_index, 28278);
        assert_eq!(
            digest(plains).unwrap(),
            "a72923bdb28920f11bacc3fb63f30f2eef3b26dbd08b61946c4f781edc0236e6"
        );
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
        let previous = commander_fixture_catalog().unwrap();
        let catalog = library_fixture_catalog().unwrap();
        assert_eq!(catalog.schema_version, 4);
        assert_eq!(catalog.definitions.len(), previous.definitions.len() + 1);
        for definition in &previous.definitions {
            assert!(catalog.definitions.contains(definition));
        }
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
                modes: vec![],
                timing: SpellTiming::MainPhase,
                resolution: SpellResolution::Instructions,
                functioning_zone: FunctioningZone::Stack,
                targets: vec![TargetSpec {
                    slot: 0,
                    selection_id: "line:0:1".into(),
                    chooser: Chooser::Controller,
                    stage: EvaluationStage::Announcement,
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
        let previous = commander_fixture_catalog().unwrap();
        let catalog = damage_fixture_catalog().unwrap();
        assert_eq!(catalog.schema_version, 3);
        assert_eq!(catalog.definitions.len(), previous.definitions.len() + 1);
        for definition in &previous.definitions {
            assert!(catalog.definitions.contains(definition));
        }
        let commander = catalog
            .definitions
            .iter()
            .find(|d| d.oracle_id == "980e1721-3717-4425-95a4-938dbf2ac661")
            .unwrap();
        assert_eq!(commander.name, "Yargle and Multani");
        assert_eq!(commander.provenance.source_index, 22862);
        assert_eq!(
            digest(commander).unwrap(),
            "ab78799601f1a0f903ebaa63b1a4ce9681fe85b22bb1df3bb73b91a28df6f41e"
        );
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
                "Hexproof",
                serde_json::json!({"kind":"keywords","keywords":[{"keyword":"hexproof"}]}),
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
        let catalog = action_fixture_catalog().unwrap();
        assert_eq!(catalog.schema_version, 5);
        assert_eq!(catalog.definitions.len(), 11);
        for prior in [
            fixture_catalog().unwrap(),
            commander_fixture_catalog().unwrap(),
            damage_fixture_catalog().unwrap(),
            library_fixture_catalog().unwrap(),
        ] {
            for definition in prior.definitions {
                assert!(catalog.definitions.contains(&definition));
            }
        }
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
            |a| a["effects"][0]["steps"][0]["optional"] = true.into(),
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
    fn action_ir_version_shapes_and_target_domains_are_strict() {
        let catalog = action_fixture_catalog().unwrap();
        for version in [3, 4, 16] {
            let mut bad = catalog.clone();
            bad.schema_version = version;
            assert!(Catalog::read(&canonical(&bad).unwrap()).is_err());
        }
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
    fn library_vocabulary_requires_ir_v4_and_valid_program_shape() {
        let catalog = library_fixture_catalog().unwrap();
        let mut old = catalog.clone();
        old.schema_version = 3;
        assert!(old.validate().is_err());
        let raw = canonical(&catalog).unwrap();
        for bad in [
            raw.replace("\"schema_version\":4", "\"schema_version\":16"),
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
        // The operation is independently versioned from casting timing: even
        // an instant-form synthetic library instruction cannot enter IR v3.
        let mut synthetic = catalog.clone();
        let d = synthetic
            .definitions
            .iter_mut()
            .find(|d| d.characteristics.kind == CardKind::Sorcery)
            .unwrap();
        d.characteristics.kind = CardKind::Instant;
        d.spell.as_mut().unwrap().timing = SpellTiming::Instant;
        d.capabilities = d.requirements();
        synthetic.validate().unwrap();
        synthetic.schema_version = 3;
        assert!(synthetic.validate().is_err());
    }

    #[test]
    fn creature_lowering_rejects_unrepresented_or_unsupported_complete_behavior() {
        type Mutation = fn(&mut serde_json::Value);
        let mutations: Vec<Mutation> = vec![
            |r| {
                r["source"]["oracle_text"] = "(Flying)".into();
            },
            |r| {
                r["source"]["type_line"] = "Artifact Creature — Bear".into();
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
        let catalog = fixture_catalog().unwrap();
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
        assert_eq!(catalog.schema_version, 3);

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
                "Hexproof",
                serde_json::json!({"kind":"keywords","keywords":[{"keyword":"hexproof"}]}),
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
        let catalog = fixture_catalog().unwrap();
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
            raw.replace("\"schema_version\":3", "\"schema_version\":2"),
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
