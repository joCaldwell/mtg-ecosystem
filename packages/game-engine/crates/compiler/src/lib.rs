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
struct Ability {
    kind: String,
    effects: Vec<Sentence>,
}
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
struct Sentence {
    steps: Vec<Step>,
}
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
struct Step {
    effect: String,
    what: Target,
    to: String,
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
            if !abilities.is_empty() {
                return Err(unsupported(
                    "abilities",
                    "nonempty permanent abilities are not supported",
                ));
            }
            // Empty creature text and basic-land reminder-only text are reviewed
            // through the pinned parser contract; no text is interpreted here.
            if kind == CardKind::Creature && !source.oracle_text.is_empty() {
                return Err(invalid(
                    "source.oracle_text",
                    "empty AST for nonempty creature behavior",
                ));
            }
        }
        CardKind::Instant => {
            if source.oracle_text.is_empty() {
                return Err(invalid("source.oracle_text", "spell has no source text"));
            }
            let mut program = SpellProgram {
                functioning_zone: FunctioningZone::Stack,
                targets: vec![],
                instructions: vec![],
            };
            let mut slots = BTreeMap::new();
            for (block, ability) in abilities.iter().enumerate() {
                if ability.kind != "spell" || ability.effects.is_empty() {
                    return Err(unsupported(
                        "abilities.kind",
                        "only complete spell blocks are supported",
                    ));
                }
                for sentence in &ability.effects {
                    if sentence.steps.is_empty() {
                        return Err(invalid("effects.steps", "empty instruction group"));
                    }
                    for step in &sentence.steps {
                        let target = &step.what;
                        if step.effect != "return-to-hand"
                            || step.to != "owner"
                            || target.reference != "target"
                            || target.filter.types != ["creature"]
                            || target.count.amount != "fixed"
                            || target.count.value != 1
                            || target.selection_id.is_empty()
                        {
                            return Err(unsupported(
                                "effects.steps",
                                "unsupported operator, scope, cardinality, or unbound target occurrence",
                            ));
                        }
                        let slot = if let Some(slot) = slots.get(&target.selection_id) {
                            *slot
                        } else {
                            let slot = u32::try_from(program.targets.len())
                                .map_err(|_| invalid("targets", "structural slot limit"))?;
                            program.targets.push(TargetSpec {
                                slot,
                                selection_id: target.selection_id.clone(),
                                chooser: Chooser::Controller,
                                stage: EvaluationStage::Announcement,
                                domain: TargetDomain::BattlefieldCreature,
                            });
                            slots.insert(target.selection_id.clone(), slot);
                            slot
                        };
                        program.instructions.push(Instruction::ReturnToOwnerHand {
                            target_slot: slot,
                            source_block: u32::try_from(block)
                                .map_err(|_| invalid("abilities", "structural block limit"))?,
                        });
                    }
                }
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

pub fn fixture_catalog() -> Result<Catalog, String> {
    let mut definitions = vec![];
    for (_, r) in &fixtures::RECORDS[..5] {
        match compile(fixtures::MANIFEST, r) {
            CompileResult::Lowered { definition } => definitions.push(*definition),
            result => return Err(format!("unexpected fixture compile result: {result:?}")),
        }
    }
    definitions.sort_by(|a, b| a.oracle_id.cmp(&b.oracle_id));
    let catalog = Catalog {
        schema_version: 2,
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
        let mut r: serde_json::Value = serde_json::from_str(fixtures::RECORDS[index].1).unwrap();
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
    fn cross_player_choice_remains_unsupported() {
        assert!(matches!(
            compile(fixtures::MANIFEST, fixtures::RECORDS[5].1),
            CompileResult::Unsupported { .. }
        ));
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
            raw.replace("\"schema_version\":2", "\"schema_version\":9"),
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
}
