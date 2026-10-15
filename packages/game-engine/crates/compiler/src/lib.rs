//! Offline archive-3 adapter and the first complete-card lowering subset.
use mtg_contracts::*;
use serde::Deserialize;
use serde_json::value::RawValue;
use std::collections::{BTreeMap, BTreeSet};

pub mod fixtures {
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
struct Sentence {
    steps: Vec<Step>,
}
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
#[serde(tag = "effect", rename_all = "kebab-case")]
enum Step {
    ReturnToHand {
        what: Target,
        to: String,
    },
    MoveZone {
        what: Target,
        to: ZoneDestination,
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
struct SourceCost {
    cost: String,
    what: ThisObject,
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
#[serde(deny_unknown_fields)]
struct ZoneDestination {
    zone: String,
    owner: String,
    position: String,
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

fn source_lines(text: &str) -> Result<Vec<String>, Failure> {
    // Only the first subset's ASCII text/reminder normalization is consumed.
    // This checks source block association; it does not interpret rule bodies.
    // CR 207.2a, same pinned rules URL as the parser's normalize.ts.
    if !text.is_ascii() {
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
    Ok(normalized
        .lines()
        .map(|line| line.split_whitespace().collect::<Vec<_>>().join(" "))
        .filter(|line| !line.is_empty())
        .collect())
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
    if source_lines(&source.oracle_text)?
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
                });
                for (block, ability) in abilities.iter().enumerate() {
                    let Ability::Activated { costs, effects } = ability else {
                        return Err(unsupported(
                            "abilities.kind",
                            "only supported complete activated creature abilities can lower",
                        ));
                    };
                    definition
                        .activated
                        .push(lower_activation(costs, effects, block)?);
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
            };
            let mut slots = BTreeMap::new();
            for (block, ability) in abilities.iter().enumerate() {
                let Ability::Spell { effects } = ability else {
                    return Err(unsupported(
                        "abilities.kind",
                        "nonspell ability on instant/sorcery",
                    ));
                };
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
    if effects.is_empty() {
        return Err(unsupported("abilities.effects", "empty spell effects"));
    }
    let source_block = source_block(block)?;
    for sentence in effects {
        if sentence.steps.is_empty() {
            return Err(invalid("effects.steps", "empty instruction group"));
        }
        for step in &sentence.steps {
            let instruction = match step {
                Step::ReturnToHand { what, to } if to == "owner" => {
                    let target_slot = bind_creature_target(program, slots, what)?;
                    Instruction::ReturnToOwnerHand {
                        target_slot,
                        source_block,
                    }
                }
                Step::MoveZone { what, to }
                    if to.zone == "library" && to.owner == "its-owner" && to.position == "top" =>
                {
                    let target_slot = bind_creature_target(program, slots, what)?;
                    Instruction::PutOnTopOwnerLibrary {
                        target_slot,
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
fn lower_activation(
    costs: &[SourceCost],
    effects: &[Sentence],
    block: usize,
) -> Result<ActivatedAbility, Failure> {
    if costs
        != [SourceCost {
            cost: "sacrifice".into(),
            what: ThisObject {
                reference: "this".into(),
                noun: "creature".into(),
            },
        }]
    {
        return Err(unsupported(
            "abilities.costs",
            "only exact source-creature sacrifice costs are supported",
        ));
    }
    let source_block = source_block(block)?;
    let mut instructions = vec![];
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
            instructions.push(match step {
                Step::Draw { who, amount } if *who == you && fixed_one(amount) => {
                    Instruction::DrawCards {
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
            });
        }
    }
    if instructions.is_empty() {
        return Err(unsupported("abilities.effects", "empty activated effects"));
    }
    // CR 602.1–2 / 608.2c: the cost is paid while activating; the ordered
    // program resolves on the stack independently of its sacrificed source.
    // https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
    Ok(ActivatedAbility {
        functioning_zone: FunctioningZone::Battlefield,
        cost: ActivationCost::SacrificeSource,
        program: SpellProgram {
            timing: SpellTiming::Instant,
            resolution: SpellResolution::Instructions,
            functioning_zone: FunctioningZone::Stack,
            targets: vec![],
            instructions,
        },
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
                    serde_json::json!([{"kind":"keywords","keywords":[{"keyword":"flying"}]}]);
            }),
            (1, |r| {
                r["source"]["toughness"] = "*".into();
            }),
            (1, |r| {
                r["source"].as_object_mut().unwrap().remove("power");
            }),
            (2, |r| {
                r["source"]["oracle_text"] = "Flying".into();
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
            raw.replace("\"schema_version\":3", "\"schema_version\":9"),
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
                CardKind::Sorcery => panic!("legacy catalog cannot contain sorceries"),
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
                "Trample",
                serde_json::json!({"kind":"keywords","keywords":[{"keyword":"trample"}]}),
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
                    timing: SpellTiming::Instant,
                    resolution: SpellResolution::Instructions,
                    functioning_zone: FunctioningZone::Stack,
                    targets: vec![],
                    instructions: vec![
                        Instruction::DrawCards {
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
            let source: serde_json::Value = serde_json::from_str(raw).unwrap();
            if source["source"]["name"] == "Blood Artist" {
                assert!(matches!(
                    compile(fixtures::MANIFEST, raw),
                    CompileResult::Unsupported { .. }
                ));
            } else {
                assert!(matches!(
                    compile(fixtures::MANIFEST, raw),
                    CompileResult::Lowered { .. }
                ));
            }
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
        for version in [3, 4, 6] {
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
            raw.replace("\"schema_version\":4", "\"schema_version\":6"),
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
                "Flying",
                serde_json::json!({"kind":"keywords","keywords":[{"keyword":"flying"}]}),
            ),
            (
                "First strike",
                serde_json::json!({"kind":"keywords","keywords":[{"keyword":"first strike"}]}),
            ),
            (
                "Trample",
                serde_json::json!({"kind":"keywords","keywords":[{"keyword":"trample"}]}),
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
            CompileResult::Unsupported { .. }
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
